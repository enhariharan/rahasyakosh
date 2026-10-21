# Glossary

Stuck on a word? This page explains every piece of jargon used in the book, in the program's messages and in the code. The first
part explains the **core ideas** in a few paragraphs each, because they build on one another. The second part is an **A–Z list** for
quick look-ups.

## Core ideas

### The vault

A **vault** is the place where all your saved logins live. Think of a safe-deposit box: the box holds everything, it is locked, and
only one key opens it (here, your **master password**). If someone steals the box, they hold a locked metal case, not your passwords.

In Rahasyakosh a vault is **one file on your disk**: by default the SQLite database `~/.local/share/rahasyakosh/vault.db`. Inside the file there are two kinds of thing:

```
 ┌──────────────────────── the vault (one file) ─────────────────────────┐
 │  HEADER   (readable, not secret)                                      │
 │     - which settings were used to turn your password into a key       │
 │     - the vault key, itself locked (it can only be opened with the    │
 │       key made from your password)                                    │
 │                                                                       │
 │  RECORDS  (one per saved item, all scrambled)                         │
 │     - "GitHub" login      →  ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                       │
 │     - "Bank" login        →  ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                   │
 │     - "Wi-Fi" note        →  ▒▒▒▒▒▒▒▒▒                                │
 └───────────────────────────────────────────────────────────────────────┘
```

Three things are worth knowing:

- **Everything useful is scrambled (encrypted).** Titles, usernames, URLs, notes, even the dates are inside the scrambled part. Someone who copies the
  file sees the header and a pile of random-looking bytes.
- **The program never keeps your master password.** It uses it to open the vault, then forgets it. The same password is the only way back in.
- **A vault is just a file**, so you can copy it, back it up, or keep it in a synced folder. You can also have more than one vault.

The *exported* vault (`rk export -f vault`) is a copy of this file: still locked, so safe to store anywhere. A *plaintext export* (CSV or JSON) is
the opposite: a readable list of every password, which is why it asks for your password again and warns you.

### Items and records: the same thing at two stages

An **item** is one thing you saved: a login, a note, a card. Its title, username, password, URLs, tags, notes and history are the item's *content*.
A **record** is that item after it has been encrypted and stamped with a little bookkeeping (its id, a revision number) so it can be stored.

Item (readable, only ever in memory while the vault is unlocked) → encrypt → record (what is written in the vault file) → decrypt → item.

### Keys, and why there are several

A **key** is a long random number that locks and unlocks data. Rahasyakosh uses a small chain of them:

1. Your **master password** is run through a deliberately slow function (**Argon2id**) to make a **master key**.
2. The master key is turned into a **key-encrypting key** whose only job is to unlock one thing: the **vault key**.
3. The **vault key** is a random key made when the vault was created. It is stored in the header, locked by the key from step 2. It is what actually protects your items (through a separate key per item).

Why not lock the items with the password's key directly? Because then changing your password would mean re-encrypting every item. With the
extra step, changing the password only re-locks the one small vault key.

### Locked and unlocked

The vault file is always scrambled on disk. **Unlocked** means a program has used your master password to work out the vault key and is holding it
in memory, so it can read your items. **Locked** means that key has been wiped. The `rk` command never stays unlocked: each command unlocks, does
its job, and exits (see [How a vault works](how-a-vault-works.md)).

### Store, backend and migration

The **store** is the part of the program that *saves and loads* records: it knows how to keep them, not how to read them (it never has a key).
Different **backends** are different ways of keeping them: in memory (for tests), as one file per item in a folder (for sync tools), or in a
**SQLite** database (the default). The rest of the program only talks to the store through one interface, so a backend can be swapped without changing anything else.

A **migration** is a safe upgrade of the storage layout when a new program version needs a different one. It must never lose data: it either completes
fully or leaves the file exactly as it was.

### Trash, purge, history

`rk rm` puts an item in the **trash**: it is marked deleted but still stored, so you can **restore** it. **Purging** deletes it for good (leaving a tiny
marker, a *tombstone*, so that other devices could later learn it was deleted). **History** keeps the previous contents of an item each time you edit it,
so an old password is not lost by accident, and you can go back to it.

## A–Z

**AAD (associated data)** — Extra information that is not hidden but *is* covered by the tamper check. We use it to tie a record to its vault, item and revision, so records cannot be swapped around. See [Security words](vocabulary.md).

**Accessibility (a11y)** — Making software usable by people with disabilities: using only a keyboard, a screen reader, high contrast or large text. "a11y" is short for "a, then 11 letters, then y". See [Accessibility](desktop-app.md#accessibility).

**AEAD** — Authenticated encryption with associated data: encrypting *and* detecting any change in one step. Ours is XChaCha20-Poly1305.

**Argon2id** — A deliberately slow, memory-hungry way to turn a password into a key, so that guessing passwords is expensive for an attacker.

**Atomic** — All-or-nothing. An atomic write either fully happens or does not happen at all; you never see half of it.

**Authenticator app** — A phone app (or `rk totp`) that shows the six-digit two-factor codes. See TOTP.

**Auto-purge** — Deleting items that have been in the trash for 30 days, done whenever the vault is unlocked.

**Backend** — One way of keeping stored data (memory, a folder of files, SQLite). See Store.

**Backoff** — Making each repeated failed attempt wait longer (1 s, 2 s, 4 s …). It slows guessing at the keyboard; it does not stop offline guessing against a copied file.

**Backup (encrypted)** — `rk export -f vault`: a copy of the vault that is still locked, so it can be stored anywhere.

**Base32 / Base64** — Ways of writing binary data using only letters and digits so it can sit in text files. TOTP secrets are base32; our JSON files use base64.

**Bit / byte** — A bit is a 0 or 1; a byte is 8 bits. A 256-bit key has 2²⁵⁶ possible values.

**Brute force** — Trying every possible password or key until one works. Strong random passwords and a slow key function make it impractical.

**Bucket (padding)** — Item sizes are rounded up to a multiple of 64 bytes, so the file size does not reveal how long a password is.

**Bundle (vault bundle)** — A vault written out as a single JSON file (header plus scrambled records). It is what `rk export -f vault` produces and `rk migrate` reads. Still encrypted, so safe to store.

**cargo-audit / cargo-deny** — Tools that check our dependencies: `cargo-audit` for known security vulnerabilities, `cargo-deny` for licences, banned crates and yanked versions. Both run in CI.

**Cargo / crate / workspace** — Cargo is Rust's build tool. A *crate* is one Rust package (a library or program). Our *workspace* is the group of crates in this repository.

**Checksum (SHA-256)** — A short fingerprint of a file. If even one bit changes, the fingerprint changes completely. Tests use it to make sure the golden files were not edited.

**CI (continuous integration)** — Automatic checks that run on GitHub for every change: formatting, lints, tests, security scans.

**Ciphertext / plaintext** — Scrambled data / readable data.

**CLI (command-line interface)** — A program you use by typing commands in a terminal, like `rk`.

**Clipboard helper** — The small background process started by `rk get --copy` that clears the clipboard after a delay.

**Clippy / rustfmt** — Rust's code checker (finds likely mistakes) and automatic formatter. CI fails if either complains.

**Conformance suite** — One set of tests that every storage backend must pass, so they all behave identically.

**Constant-time** — Code that takes the same time whatever the secret is, so timing cannot leak it.

**Corpus / seed** — The collection of inputs a fuzzer works from. Seeds are the small valid inputs it starts with.

**CSPRNG** — A random-number generator safe for secrets (cryptographically secure). Every key and nonce here comes from one.

**CSV** — A simple table in plain text (comma-separated values). Browsers export passwords this way. CSV exports are **unencrypted**.

**CSV injection** — A spreadsheet treating a cell that starts with `=`, `+`, `-` or `@` as a formula. Open exports in a text editor.

**Decrypt / encrypt** — Turn scrambled data back into readable data / the reverse, using a key.

**De-duplication** — Skipping an imported row that is identical to one already stored, so importing twice does not double your vault.

**Dependency** — A library our code uses. Every one is code we trust with your secrets, which is why we keep them few.

**Deterministic** — Always giving the same result from the same input. The golden vault is generated deterministically so it can be reproduced exactly.

**Dioxus** — The Rust framework we draw the desktop app with. It shows the interface in a web view and lets us write the interface in Rust.

**Downgrade attack** — Tricking software into using weaker settings than it should.

**EFF wordlist** — A list of 7,776 easy words published by the Electronic Frontier Foundation, used to make random passphrases.

**Entropy** — A measure of unpredictability in bits. The more bits, the harder to guess.

**Environment variable** — A named setting a program inherits from its surroundings. `RK_VAULT` chooses the vault file; `RK_PASSWORD` supplies the master password to scripts. **Do not set `RK_PASSWORD` permanently**: it keeps your master password in your environment.

**Fixture** — A saved sample used by a test. Ours (the golden vault, the schema database) are never changed.

**Fsync / full sync** — Asking the operating system to confirm that data has really reached the disk, not just a memory buffer. Slower, but a committed change survives a power cut.

**Fuzzing / fuzzer** — Testing by throwing junk at a program. A *fuzzer* is a tool that feeds the program millions of automatically made-up inputs per minute — empty files, truncated files, random bytes, valid files with one byte changed — and watches for crashes, hangs or broken promises. Normal tests check the cases a person thought of; a fuzzer finds the ones nobody did, which matters most for code that reads files from outside (a vault file, a CSV, a one-time-code link), because an attacker controls those. A *coverage-guided* fuzzer is smarter than random: it notices which inputs reach new parts of the code, keeps those, and mutates them further. Example: if opening a damaged vault file crashed the program, the fuzzer would hand us the exact bytes that did it. See [Fuzzing](fuzzing.md).

**Gallery** — A development aid that opens one screen of the desktop app on sample data in an in-memory vault, to look at the design without touching a real vault.

**Golden file** — A saved known-good file that the code must always be able to open. See [Testing with known answers and properties](testing-with-known-answers.md).

**Hash** — A one-way fingerprint of data. You cannot get the data back from it.

**Header (vault header)** — The readable, non-secret part of a vault: the key-derivation settings, the salt and the locked vault key.

**History** — Earlier versions of an item, kept inside it so they are as protected as the current one.

**HKDF** — A method that makes several unrelated keys out of one secret, using labels.

**HLC (hybrid logical clock)** — A kind of timestamp that keeps order across devices even if their clocks differ. Planned for sync (M4).

**HMAC** — A keyed fingerprint; the building block of TOTP.

**Idle timeout (auto-lock)** — Locking the vault automatically after a period with no activity, so a vault left open on an unattended computer closes itself.

**Index (search index)** — A temporary in-memory list built when you unlock, used to search quickly. It is never written to disk and is wiped when the program exits.

**Item** — One thing you saved: a login, note, card or identity. See the core ideas above.

**Journal / WAL** — SQLite's safety log. Writes go to the log first, so a crash cannot leave the database half-written.

**JSON** — A text format for structured data. Vault headers, records and exports use it.

**KAT (known-answer test)** — A test that checks output against a value published by someone else. See the testing chapter.

**KDF (key derivation function)** — Turns a password into a key. Ours is Argon2id.

**KEK (key-encrypting key)** — A key whose only job is to lock another key (the vault key).

**Key** — A secret number that locks and unlocks data.

**Locked / unlocked** — Whether the vault key is erased from memory or held for use. See the core ideas above.

**MAC** — A message authentication code: a keyed checksum that proves data was not altered.

**Master password (or passphrase)** — The one secret you remember. It is never stored; it is only used to open the vault. **There is no recovery.**

**mdBook** — The tool that turns the Markdown files in `docs/book/` into this book.

**Migration** — A safe, all-or-nothing upgrade of how data is stored. See the core ideas above.

**Mutation check** — Deliberately breaking the code to confirm the tests notice.

**Native / WebAssembly** — "Native" means an ordinary program for your computer. Some tests run only natively; the important ones also run as WebAssembly.

**Nightly Rust** — The experimental Rust toolchain. Only fuzzing needs it; everything else builds on stable.

**Nonce** — "Number used once": a value that must never repeat for the same key. Ours are 24 random bytes.

**OpenSSL / ring** — Widely used C and Rust cryptography libraries. We do not link them, to keep builds pure Rust and portable to WebAssembly. OpenSSL was used *once*, as an independent source for expected test values.

**otpauth URI** — The `otpauth://totp/...` link inside a two-factor QR code; it contains the secret and settings.

**Padding** — Extra filler added so all encrypted items fall into a few size groups. See Bucket.

**Panic** — Rust stopping a program because an assumption was wrong (an out-of-range index, an impossible value). In a parser, a panic on hostile input is a bug.

**Parser** — Code that reads data and makes sense of it (a vault file, a CSV, an `otpauth://` link). Any parser of data from outside is a door to attack.

**Passphrase** — A password made of several random words, such as `correct-horse-battery-staple`.

**Process group** — A way of grouping programs so closing the terminal does not stop a background helper.

**Property test** — A test that checks a rule for hundreds of generated inputs, then shrinks any failure to the smallest example.

**Purge** — Delete an item for good, as opposed to moving it to the trash.

**Python `cryptography`** — A separate, widely used Python cryptography library. Used once, together with OpenSSL, to compute expected test values independently.

**Re-authentication** — Asking for the master password again before a dangerous action (plaintext export).

**Record** — An encrypted item as it is stored, plus its id and revision. See the core ideas above.

**Redundant protection** — Two independent safeguards against the same attack. Deliberate: if one has a flaw, the other still holds.

**Regression test** — A test that keeps a fixed bug from coming back. Every input that ever crashed a fuzzer is replayed on each change.

**Rejection sampling** — Throwing away random numbers that would make a choice slightly uneven. Avoids "modulo bias".

**Reveal** — Showing a hidden secret on screen for a few seconds, then hiding it again.

**Revision (rev)** — A counter on each record that goes up with every change. A write that is not newer than what is stored is refused.

**RFC** — A published technical standard ("Request for Comments"). The test vectors come from them.

**Salt** — Random public data mixed into password hashing so identical passwords give different keys.

**Sanitizer (AddressSanitizer)** — Instrumentation that makes memory errors, such as reading freed memory, crash immediately. Useful for the C code inside SQLite.

**Schema** — The layout of a database (its tables and columns), or of an item's saved data. It has a version number.

**Screen reader** — Software (Orca on Linux) that reads the screen aloud or to a braille display. It relies on each control having a clear text name, which is why a button must say *what* it copies.

**Scrollback** — Everything your terminal remembers having shown. A password printed by `rk get` stays there; `--copy` avoids that.

**Secret Key** — A planned, optional extra random key kept only on your devices and combined with your password, so a stolen vault is safe even against a weak password. Not built yet.

**Secure delete** — Overwriting deleted data with zeros so it cannot be recovered from the file's free space.

**Session** — A period during which the vault stays unlocked, and the code that decides when it ends. The CLI has none; the desktop app does. See [Sessions](sessions.md).

**Shared secret** — The value that a website and your authenticator both know, from which the six-digit codes are computed. Treat it like a password.

**SQLite** — A small, very reliable database that lives in a single file. The default store.

**Standard input (stdin)** — The channel a program reads typed or piped text from. `--password-stdin` reads a password this way, so it never appears in the command line where other users could see it.

**Store** — The part of the program that saves and loads records. See the core ideas above.

**Story ID (`US-XXX-NN`)** — The name of a user story in `docs/02-epics-and-user-stories.md`, such as `US-GEN-02`. We quote them in commits and tests so every change traces back to a reason.

**Strength score** — A 0–4 rating of how guessable a password is (from the zxcvbn library).

**Supply chain** — All the third-party code a program depends on. Each dependency is trusted, so we keep few and scan them.

**Test vector** — A published input and its correct output, used to prove an implementation matches the standard.

**Threat model** — A written answer to "who are we defending against, and who are we not?"

**Tombstone** — A tiny marker left when an item is purged, so a deletion can later be shared with other devices.

**TOTP** — Time-based one-time password: the six-digit code that changes every 30 seconds. See [One-time codes](totp.md).

**Transaction** — A group of database changes that happen together or not at all. "Check the revision, then write" is one transaction, so two programs cannot slip in between.

**Trash** — Items marked deleted but still stored, so they can be restored.

**UUID** — A long random identifier (like `0190…0001`) used to name each item.

**Vault** — The encrypted file that holds all your saved items. See the core ideas above.

**Vault key (VK)** — The random key that protects your items. It is stored in the header, locked by the key made from your password.

**Wayland / X11** — The two ways Linux draws a desktop. The clipboard tools differ between them.

**WCAG** — The Web Content Accessibility Guidelines. Level AA is the usual target: text contrast of at least 4.5 to 1, 3 to 1 for control edges, everything usable by keyboard.

**WebAssembly (WASM / WASI)** — A format that lets the same code run in a browser or another sandbox. We build for it so the web app can share the code. `wasmtime` runs it in tests.

**WebKitGTK** — The web-page engine used by GNOME. The desktop app uses it as its drawing surface; it is a system library, not part of our code.

**Wrapped key** — A key that is stored encrypted under another key.

**Wrapped vault key** — The vault key as stored in the header: encrypted under the key made from your password. See Wrapped key.

**XChaCha20-Poly1305** — The cipher we use to encrypt and tamper-proof data.

**Zeroize** — Overwriting secret memory with zeros as soon as it is no longer needed.

**Zero knowledge** — A service that cannot read your data. Here, nothing outside your device ever sees plaintext.

**zxcvbn** — The library that estimates password strength by looking for common patterns.
