# What a password manager does

## The problem

A typical person has well over a hundred online accounts. Good security advice says every account
needs a **long, random, different** password. Nobody can memorise that. So people reuse passwords,
and when one website leaks, attackers try the same email-and-password pair everywhere else.

## The idea

A password manager stores all your passwords in one place and locks that place with **one** strong
master password. You remember one thing; the manager remembers the rest.

That only works if the place is truly locked. This leads to the central design question of the
whole project:

> *What if someone gets a copy of the vault file?*

The file might be stolen from your laptop, copied from a cloud-sync folder, or taken from a
hacked server. The answer must be: **they still learn nothing.** Everything useful inside is
**encrypted**, and the only way to decrypt it is the master password, which is never stored.

## Zero knowledge

"Zero knowledge" means that anything other than your own device — a sync server, a cloud folder,
the developers — only ever sees scrambled data. We could not read your vault even if we wanted to,
or if a court told us to. The same goes for an attacker who breaks in.

## Local first

Rahasyakosh works entirely offline. There is no account, and no company server is required.
Syncing between your devices is an optional extra that moves *already encrypted* data.

## What the program actually does

| Job | Where it is described |
|---|---|
| Turn a master password into encryption keys | [How a vault works](how-a-vault-works.md) |
| Encrypt and decrypt each saved item | [How a vault works](how-a-vault-works.md) |
| Generate strong passwords and passphrases | [Passwords and passphrases](passwords-and-passphrases.md) |
| Produce two-factor codes | [One-time codes](totp.md) |
| Wipe secrets from memory when you lock | [Writing code that handles secrets](safe-coding.md) |

## One core, many front-ends

All security-sensitive code lives in a single Rust library called `rahasyakosh-core`. The command
line tool `rk` is a thin layer on top. Later, desktop, web and Android apps will be thin layers
on the *same* library. The benefit is that there is only one place to review and audit, and every
platform behaves identically.
