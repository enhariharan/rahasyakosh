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

> **Status:** bundling the EFF list and the `--passphrase` option is the work for US-GEN-02. See
> `docs/06-roadmap.md` for current progress.

### Rules for working on this code

- The randomness must always come from the injected CSPRNG, never from the clock or `rand::random`.
- Never log or print a generated secret except as the program's intended output.
- Generated secrets are returned as `Zeroizing<String>` so they are wiped when dropped.
- Any change to the word list changes the strength calculation. Keep the list exact and unmodified,
  and keep a test that it has 7,776 unique entries.
