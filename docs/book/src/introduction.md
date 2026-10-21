# Introduction

Rahasyakosh (रहस्यकोश, "treasury of secrets") is an open-source password manager written in Rust.
You keep your logins in an encrypted file called a **vault**. You remember one master password,
and the program does the rest.

This book explains how it works **in plain English**. It is written for two kinds of reader:

- **Newcomers** who want to understand what the project is and why it is built the way it is.
- **Future maintainers**, who may never have studied security or cryptography. Maintaining this
  repository is a good way to learn those subjects, and this book is meant to be your first teacher.

You do not need any security background. Every technical term is explained the first time it
appears, and again in the [Glossary](glossary.md). You do need to be comfortable reading basic Rust.

## How to read this book

If you are new to the subject, read the chapters in order. Each one builds on the one before.

| If you want to… | Read |
|---|---|
| Understand the big picture | [What a password manager does](what-is-a-password-manager.md) |
| Decode jargon in a code review | [Security words you will meet](vocabulary.md) and the [Glossary](glossary.md) |
| Know why we picked Argon2, XChaCha20 and friends | [The cryptography toolbox](crypto-toolbox.md) |
| Follow a password from keyboard to disk | [How a vault works](how-a-vault-works.md) |
| Move logins in or out safely | [Importing and exporting](import-export.md) |
| Work on the password generator | [Passwords and passphrases](passwords-and-passphrases.md) |
| Find your way around the code | [A tour of the repository](repo-tour.md) |
| Make your first change safely | [Maintainer's handbook](maintainers-handbook.md) |

## A word of caution

Security software fails quietly. A bug in a game shows up as a glitch; a bug in a password manager
may look perfectly fine while leaking secrets. So this project favours **boring, well-tested
building blocks** over clever ideas, and it is **not yet independently audited**. Do not store real
secrets in it until that has happened.

## Other documents in `docs/`

This book explains. The numbered documents in `docs/` *decide*: vision, backlog, architecture,
the formal security design, UX and the roadmap. When the two disagree, the numbered documents win;
please then fix the book.

## Building this book

The book is an [mdBook](https://rust-lang.github.io/mdBook/). From the repository root:

```
cargo install mdbook      # once
mdbook serve docs/book --open
```
