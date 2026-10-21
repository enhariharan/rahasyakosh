# Fuzzing: attacking our own parsers

## The problem

A *parser* is code that reads data and makes sense of it: a vault file, a CSV from your browser, a two-factor link. Any parser that reads data from
outside the program is a door. If someone can hand it a file you did not write (a download, a shared folder, a backup copied from elsewhere), then a mistake in the parser can
crash the program, hang it, use up all your memory, or, in the worst case in other languages, let an attacker run their own code.

Rust removes the worst of these (it will not let a parser overwrite memory it does not own). But a parser can still **panic** (stop the program because an
assumption was wrong, such as "this number is never huge"), **hang**, or **allocate gigabytes**. For a password manager, a vault you cannot open because a bad file crashes the
program is a serious failure. So we test parsers against *hostile* input, not only well-formed input.

## What fuzzing is

**Fuzzing** means feeding a program a huge number of automatically made-up inputs and watching for crashes. A **coverage-guided fuzzer** (we use
[libFuzzer](https://llvm.org/docs/LibFuzzer.html), driven by `cargo fuzz`) is cleverer than random: it watches which lines of our code each input reaches, keeps
the inputs that reach something new, and mutates those (flip a bit, delete a byte, splice two inputs). Starting from a few valid files, it quickly explores strange variations that still get
deep into the parser. Millions of inputs per minute is normal.

Compare the kinds of test in this project:

| Kind | Question it answers | Example |
|---|---|---|
| Unit test | Does it work on this input? | Import this Chrome CSV |
| Known-answer test | Does it match the standard? | The RFC's TOTP codes |
| Property test | Does a rule hold for *typical* generated inputs? | Encrypt then decrypt gives back the input |
| **Fuzzing** | Can *any* input, however twisted, break it? | A header whose numbers are nonsense |

## What we fuzz

Ten **targets**, one for each place we read bytes we did not write. Each is a plain function in `crates/fuzzcheck/src/lib.rs` that must return normally for any input.

| Target | What it reads | Beyond "does not crash", it also checks |
|---|---|---|
| `vault_header` | The readable header of a vault | Write then read gives the same header; opening a header with cheap settings never panics |
| `record` | An encrypted record, as JSON or as raw fields | A made-up record **never opens** (nobody can forge one) |
| `payload` | The decrypted bytes inside a record | Padding and item parsing cope with any bytes, even from a buggy or newer program |
| `item_json` | An item's content | Stable under write and read; the item's `Debug` output never shows a concealed value |
| `csv_import` | A CSV file, with and without a column mapping | What we import can be exported and imported again |
| `totp` | A two-factor secret or `otpauth://` link | Codes always have the right number of digits |
| `bundle` | A portable vault file (`rk export -f vault`) | Write then read is stable; it loads into a store without panicking |
| `directory_store` | A folder whose files contain anything | Every operation returns a value or an error |
| `sqlite_store` | A SQLite file with any contents | Opening and reading never panics (this one also runs under AddressSanitizer, which catches memory errors in the C library) |
| `strength` | Any text as a password | The score is always between 0 and 4 |

The `payload` target needs a trick. Random bytes can never pass the tamper check, so they would never reach the code that reads a decrypted payload. A small hook in `core`
(`seal_raw_payload_for_fuzzing`, hidden from the documentation and never used by the programs) encrypts the fuzzer's bytes with a real key, so they arrive on the other side of the lock exactly
as a buggy program's output would.

## How to run it

You need a nightly Rust toolchain and `cargo install cargo-fuzz`. From the repository root:

```
cd fuzz
cargo +nightly fuzz list                                    # the ten targets
cargo +nightly fuzz run -s none record                      # run until you press Ctrl-C
cargo +nightly fuzz run -s none csv_import -- -max_total_time=60    # or for 60 seconds
cargo +nightly fuzz run sqlite_store                        # default sanitizer: AddressSanitizer
```

The output lines look like `#2942508 REDUCE cov: 1525 ft: 3874 corp: 1885/178Kb exec/s: 65389`: the number of inputs tried, how much code has been reached (`cov`), the size of the
collection of interesting inputs (`corp`, the **corpus**), and speed. If it finds a crash it stops and writes the input into `fuzz/artifacts/<target>/crash-…`.

GitHub also runs every target for a few minutes each, every Sunday (`.github/workflows/fuzz.yml`), and you can start it by hand from the Actions tab.

## What to do when the fuzzer finds a crash

1. **Reproduce.** `cargo +nightly fuzz run <target> fuzz/artifacts/<target>/crash-…` runs just that input and shows the panic.
2. **Shrink it** (optional): `cargo +nightly fuzz tmin <target> <crash file>` finds the smallest input that still fails.
3. **Decide who is wrong.** Usually the code is, and you fix it. Sometimes the *check* is too strict (this really happened, see below).
4. **Keep the input forever.** Copy it into `fuzz/regressions/<target>/` with a descriptive name. The ordinary test suite (`cargo test`) replays everything in `fuzz/regressions/` and `fuzz/corpus/` on every change, so the bug cannot come back unnoticed, and nobody needs nightly Rust for that.

## The corpus

`fuzz/corpus/<target>/` holds the **seeds**: small, valid starting inputs (the golden vault's header, a Chrome CSV, a real SQLite database…) made by
`cargo run -p rahasyakosh-fuzzcheck --example seed_corpus -- fuzz/corpus`. A fuzzer finds bugs much faster starting from valid data. Seeds are committed. Everything a
fuzzing run adds beyond the seeds is not committed, to keep the repository small.

## What fuzzing has found so far

- **Checks that were too strict, twice.** Within seconds the `item_json` fuzzer "found" a secret in `Debug` output. Both times nothing leaked: the fuzzer had built a concealed value that happened to equal a piece
  of something `Debug` legitimately prints, first the item's own id, then the *escaped* form of a field name (`Debug` doubles every backslash). The second one also showed a hole in our check: a real leak of a value
  containing a backslash or quote would have appeared escaped and slipped past a check that looked for the raw text. We made the check compare escaped forms in both directions and kept both inputs in
  `fuzz/regressions/item_json/`. Lesson: a fuzz check is code too, and fuzzing tests it as hard as it tests the program.
- **A 32-bit overflow, found by reading.** While wiring up the `payload` target we read the padding code and saw that `4 + length` could overflow on 32-bit systems such as WebAssembly (where the web app will run
  this exact code) when the stored length is near 4 GB. A 64-bit fuzzer can never reach that, so we fixed it with checked arithmetic. Fuzzing and careful reading complement each other.
- **A known limit, not a bug.** A hostile vault header may legitimately ask for the maximum key-derivation cost (1 GiB of memory, 20 passes) because the format allows it. Opening such a file is slow by design; the
  fuzz target only tries to unlock headers with cheap settings so it does not spend its time there.

The other targets ran millions of inputs each without a crash (in 45 seconds each: about 2.9 million for `vault_header`, 12.8 million for `record`, 15.5 million for `bundle`; `sqlite_store` ran 391,000 inputs under AddressSanitizer, and `item_json` 17.5 million after the check was fixed). That does not prove there are no
bugs, but it is strong evidence, and the weekly run keeps looking.

## Limits

- Fuzzing finds crashes and violated checks. It cannot tell whether a result is *correct* beyond the checks we wrote.
- It only explores what it can reach. Code behind a valid signature or key (like decrypted payloads) needs a hook such as the one above.
- The vault logic (adding, trashing, history) is covered by the model-based property test rather than a fuzz target, because each run would need a slow key derivation.
