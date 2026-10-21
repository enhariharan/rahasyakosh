# Passwords and passphrases

The generator lives in `crates/core/src/generator.rs`.

## Random passwords

`generate_password` builds a password from character classes (lowercase, uppercase, digits,
symbols). It guarantees at least one character from each chosen class, fills the rest from the
combined pool, and then shuffles the result so the guaranteed characters are not always first.
An option excludes look-alike characters (`O 0 I l 1 |`).

### The "modulo bias" trap

Suppose you want a random number from 0 to 9 and your source gives numbers 0 to 255. A tempting
approach is `x % 10`. But 256 is not a multiple of 10, so the results 0–5 occur slightly more often
than 6–9. The password is subtly predictable. Real attacks exploit such bias.

The fix, in `generator::uniform`, is **rejection sampling**: throw away any random number that
falls in the uneven leftover zone and draw again. What remains is perfectly uniform. Always use
`uniform` when picking from a list; never use `%` on raw random output.

## Passphrases

A **passphrase** is a password made of ordinary words, for example
`correct-horse-battery-staple`. Words are far easier to remember and type than `x7#Qp9!z`, so they
are ideal for things you type by hand, such as your master password or a Wi-Fi key. They are strong
as long as the words are chosen **truly at random** from a **large known list**, and there are
**enough of them**.

The strength is simple arithmetic. If the list has 7,776 words, each word adds log₂(7776) ≈ 12.9 bits
of entropy:

| Words | Entropy | Verdict |
|---:|---:|---|
| 4 | ~52 bits | fine for low-value accounts |
| 5 | ~65 bits | good |
| 6 | ~77 bits | strong; recommended for a master password |
| 7 | ~90 bits | very strong |

Notice the attacker is assumed to know the list and the method. Security comes only from the
randomness of the choice. A phrase *you* make up ("my dog Rex loves cheese") is **not** random and
is far weaker than its length suggests.

### The EFF wordlist

The Electronic Frontier Foundation (EFF), a digital-rights non-profit, published a "large wordlist"
of exactly **7,776 words** (6⁵, so five rolls of a six-sided die pick one word — handy for
generating a passphrase with physical dice). The words were chosen to be common, easy to spell and
easy to tell apart, and no word is the beginning of another, so the phrase is unambiguous even
without separators. This is the list that user story **US-GEN-02** specifies.

`generate_passphrase` takes the word list as an argument rather than hard-wiring it. This keeps the
algorithm separate from the data, makes testing easy, and lets other lists be added later (for
example in other languages).

### The command line

```
rk gen                      # a random password
rk gen --passphrase         # a passphrase from the EFF list
rk gen --passphrase --words 7 --separator . --capitalize
```

Each run also prints the estimated entropy (for example `~78 bits of entropy`) on standard error,
so it does not pollute the passphrase when you pipe the output.

### Choosing a master passphrase

The master password is the one secret everything else depends on, so a random passphrase is the best
choice for it:

```
rk init --passphrase            # six random words (~77 bits)
rk passwd --passphrase -w 7     # change the master passphrase to seven words
```

The program shows the phrase **once**, then asks you to type it again. This proves you have written it
down before the vault is created or changed; if the second entry does not match, nothing is changed.
Remember: if you lose the master password, nobody, including us, can recover your vault.
`--passphrase` cannot be combined with the `RK_PASSWORD` environment variable.

### How it is wired up

- `crates/core/src/wordlists/eff_large.txt` is the list itself, compiled into the program with
  `include_str!`, so nothing is downloaded at run time and it works in WebAssembly too.
- `wordlist::eff_large()` returns the words. Tests pin the file's SHA-256 checksum and check that it has
  7,776 unique lowercase words, none the beginning of another. If you ever need to change the file, you
  must update the checksum on purpose.
- `generator::passphrase_entropy_bits` does the arithmetic from the table above.
- The list is licensed CC BY 3.0 US, which requires crediting EFF. That credit is in
  `crates/core/src/wordlists/README.md`; keep it.

### Rules for working on this code

- The randomness must always come from the injected CSPRNG, never from the clock or `rand::random`.
- Never log or print a generated secret except as the program's intended output.
- Generated secrets are returned as `Zeroizing<String>` so they are wiped when dropped.
- Any change to the word list changes the strength calculation. Keep the list exact and unmodified,
  and keep a test that it has 7,776 unique entries.

## The strength meter

`crates/strength/src/lib.rs` (the `rahasyakosh-strength` crate) answers "how guessable is this password?". The naive way is
"alphabet size ^ length", which is wrong for human-made passwords: `Password123!` uses four
character classes and 12 characters, yet every cracking tool tries it within seconds.

We use the `zxcvbn` library (originally from Dropbox), which looks for the patterns attackers
actually try: common passwords, dictionary words, names, keyboard walks like `qwerty`, repeats,
sequences, dates and "l33t" substitutions. It then estimates how many guesses an attacker would need.

`strength::estimate(password, user_inputs)` returns:

| Field | Meaning |
|---|---|
| `score` | 0 (very weak) to 4 (very strong) |
| `crack_time` | Time to crack offline against a slow hash, e.g. "centuries" |
| `warning`, `suggestions` | Plain-language hints, e.g. "This is a top-10 common password." |

`user_inputs` lets the caller pass words an attacker could guess (username, site name); passwords
containing them score lower.

Where it is used:

- `rk init` and `rk passwd` print the score, and **refuse a master password scoring below 2** (US-VLT-01).
  `--allow-weak` overrides this deliberately. It cannot be combined with `--passphrase`.
- `rk gen` prints the score of what it generated on standard error.

### Limits you should know about

- It is an **estimate**. A high score does not mean a password is safe if it has been leaked, or reused.
  Breach checking is a separate, later feature (US-GEN-05).
- It assumes the attacker knows common human habits but not you personally.
- The `zxcvbn` library reads the system clock internally (to recognise recent years) and, on WebAssembly,
  uses browser JavaScript APIs. `rahasyakosh-core` must never do either, so the meter lives in its **own
  crate**. Core stays free of those dependencies, and a problem in the estimator can never reach the code
  that handles keys. The meter is advisory: it never sees a key, and the password never leaves the process.
- Because it is separate, core cannot refuse a weak master password by itself. Each front-end (today
  the CLI) applies the rule using `Strength::MIN_MASTER_SCORE`.
