//! `rk` — Rahasyakosh command-line password manager (M1b: single-file vault).
mod clipboard;
mod file;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use file::VaultFile;
use rahasyakosh_core::{
    crypto::KdfParams,
    generator::{self, PasswordSpec},
    item::{Field, Item},
    totp::Totp,
    vault::UnlockedVault,
    wordlist,
};
use rahasyakosh_strength as strength;
use rahasyakosh_transfer as transfer;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "rk", version, about = "Rahasyakosh password manager")]
struct Cli {
    /// Vault file (default: $RK_VAULT or ~/.local/share/rahasyakosh/vault.rkv)
    #[arg(long, global = true, env = "RK_VAULT")]
    vault: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a new vault
    Init(MasterOpts),
    /// Add a login (password generated if --password is omitted and --generate is set)
    Add {
        title: String,
        #[arg(short, long, default_value = "")]
        username: String,
        #[arg(long)]
        url: Option<String>,
        /// Read the password from stdin instead of prompting
        #[arg(long)]
        password_stdin: bool,
        /// Generate a random password
        #[arg(short, long)]
        generate: bool,
        /// Base32 TOTP secret or otpauth:// URI
        #[arg(long)]
        totp: Option<String>,
    },
    /// Change fields of an item; only the options you pass are changed
    Edit {
        query: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(short, long)]
        username: Option<String>,
        /// Replace the item's URLs with this one (empty string clears them)
        #[arg(long)]
        url: Option<String>,
        #[arg(long)]
        notes: Option<String>,
        /// Prompt for a new password
        #[arg(long, group = "new_password")]
        password: bool,
        /// Read the new password from stdin
        #[arg(long, group = "new_password")]
        password_stdin: bool,
        /// Replace the password with a generated one (printed once)
        #[arg(short, long, group = "new_password")]
        generate: bool,
        /// Set the TOTP secret (base32 or otpauth:// URI)
        #[arg(long, conflicts_with = "remove_totp")]
        totp: Option<String>,
        #[arg(long)]
        remove_totp: bool,
        /// Add a tag (repeatable)
        #[arg(long)]
        tag: Vec<String>,
        /// Remove a tag (repeatable)
        #[arg(long)]
        untag: Vec<String>,
    },
    /// List items (optionally filtered)
    List { query: Option<String> },
    /// Print a field of the best-matching item (default: password)
    Get {
        query: String,
        #[arg(short, long, default_value = "password")]
        field: String,
        #[command(flatten)]
        copy: CopyOpts,
    },
    /// Print the current TOTP code
    Totp {
        query: String,
        #[command(flatten)]
        copy: CopyOpts,
    },
    #[command(hide = true, name = "clipboard-clear")]
    ClipboardClear {
        #[arg(long)]
        after: u64,
    },
    /// List items in the trash
    Trash,
    /// Bring an item back from the trash
    Restore { query: String },
    /// Move an item to trash (or --purge to delete forever, also from the trash)
    Rm {
        query: String,
        #[arg(long)]
        purge: bool,
    },
    /// Generate a password, or a passphrase with --passphrase
    Gen {
        #[arg(short, long, default_value_t = 20)]
        length: usize,
        #[arg(long)]
        no_symbols: bool,
        #[arg(long)]
        avoid_ambiguous: bool,
        /// Generate a passphrase from the EFF large wordlist instead of a password
        #[arg(short, long)]
        passphrase: bool,
        /// Number of words in the passphrase
        #[arg(short, long, default_value_t = 6, requires = "passphrase")]
        words: usize,
        /// Separator between words
        #[arg(short, long, default_value = "-", requires = "passphrase")]
        separator: String,
        /// Capitalise each word
        #[arg(short, long, requires = "passphrase")]
        capitalize: bool,
    },
    /// Change the master password
    Passwd(MasterOpts),
    /// Import logins from a CSV file (generic, Chrome or Firefox export)
    Import {
        file: PathBuf,
        /// Map one of our fields to a column header, e.g. --map password=pwd (fields: title, url,
        /// username, password, notes, totp, tags)
        #[arg(long, value_name = "FIELD=COLUMN")]
        map: Vec<String>,
        /// Show what would be imported, then stop
        #[arg(long)]
        dry_run: bool,
        /// Do not ask for confirmation
        #[arg(short, long)]
        yes: bool,
    },
    /// Export the vault. csv and json are PLAINTEXT and ask for the master password again.
    Export {
        #[arg(short, long, value_enum, default_value_t = ExportFormat::Csv)]
        format: ExportFormat,
        /// Write to this file (default: standard output; not allowed for --format vault)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Overwrite the output file if it exists
        #[arg(long)]
        force: bool,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum ExportFormat {
    /// Plaintext CSV (lossy: logins only)
    Csv,
    /// Plaintext JSON (complete)
    Json,
    /// Encrypted copy of the vault file (safe to store; needs your master password to open)
    Vault,
}

/// Copy to the clipboard instead of printing.
#[derive(clap::Args)]
struct CopyOpts {
    /// Copy to the clipboard instead of printing it, and clear the clipboard afterwards
    #[arg(short, long)]
    copy: bool,
    /// Seconds before the clipboard is cleared (0 = never)
    #[arg(long, default_value_t = 20, requires = "copy", value_name = "SECONDS")]
    clear_after: u64,
}

/// Print `value`, or copy it when asked (then say so on stderr and keep it off the screen).
fn output(value: &str, what: &str, opts: &CopyOpts) -> Result<()> {
    if !opts.copy {
        println!("{value}");
        return Ok(());
    }
    clipboard::copy_and_schedule_clear(value, opts.clear_after)?;
    if opts.clear_after == 0 {
        eprintln!("Copied {what}. It will stay on the clipboard until you replace it.");
    } else {
        eprintln!(
            "Copied {what}; the clipboard clears in {}s.",
            opts.clear_after
        );
    }
    Ok(())
}

/// How the new master password is chosen.
#[derive(clap::Args)]
struct MasterOpts {
    /// Generate a random master passphrase (EFF large wordlist) instead of typing a password
    #[arg(short, long)]
    passphrase: bool,
    /// Number of words in the generated passphrase
    #[arg(short, long, default_value_t = 6, requires = "passphrase")]
    words: usize,
    /// Accept a master password the strength meter rates below "fair"
    #[arg(long, conflicts_with = "passphrase")]
    allow_weak: bool,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn default_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME not set; use --vault")?;
    Ok(PathBuf::from(home).join(".local/share/rahasyakosh/vault.rkv"))
}

/// Master password from $RK_PASSWORD (for scripting) or an interactive prompt.
fn read_password(prompt: &str) -> Result<zeroize::Zeroizing<String>> {
    if let Ok(p) = std::env::var("RK_PASSWORD") {
        return Ok(zeroize::Zeroizing::new(p));
    }
    Ok(zeroize::Zeroizing::new(rpassword::prompt_password(prompt)?))
}

/// New master password: typed twice, or generated (shown once, then re-typed to confirm it was saved).
fn new_master(
    opts: &MasterOpts,
    rng: &mut impl rand::CryptoRng,
) -> Result<zeroize::Zeroizing<String>> {
    if !opts.passphrase {
        return new_password(opts.allow_weak);
    }
    if std::env::var("RK_PASSWORD").is_ok() {
        bail!("--passphrase cannot be combined with RK_PASSWORD");
    }
    let list = wordlist::eff_large();
    let phrase = generator::generate_passphrase(&list, opts.words, "-", false, rng)?;
    eprintln!(
        "Your new master passphrase (write it down; it is shown only once):\n\n    {}\n",
        *phrase
    );
    eprintln!(
        "~{:.0} bits of entropy",
        generator::passphrase_entropy_bits(list.len(), opts.words)
    );
    let again = rpassword::prompt_password("Type it again to confirm you saved it: ")?;
    if *again != *phrase {
        bail!("passphrase did not match; nothing was changed");
    }
    Ok(phrase)
}

fn print_strength(s: &strength::Strength) {
    eprintln!(
        "Strength: {}/4 ({}); offline cracking time: {}",
        s.score,
        s.label(),
        s.crack_time
    );
    if let Some(w) = &s.warning {
        eprintln!("  {w}");
    }
    for hint in &s.suggestions {
        eprintln!("  - {hint}");
    }
}

fn new_password(allow_weak: bool) -> Result<zeroize::Zeroizing<String>> {
    let p = read_password("New master password: ")?;
    if std::env::var("RK_PASSWORD").is_err() {
        let again = rpassword::prompt_password("Repeat: ")?;
        if *again != *p {
            bail!("passwords do not match");
        }
    }
    let strength = strength::estimate(&p, &[]);
    print_strength(&strength);
    if strength.score < strength::Strength::MIN_MASTER_SCORE && !allow_weak {
        bail!("master password is too weak; choose a longer one, use --passphrase, or pass --allow-weak");
    }
    Ok(p)
}

/// Ask for the master password again and check it against the vault header.
fn reauthenticate(header: &rahasyakosh_core::vault::VaultHeader) -> Result<()> {
    let again = zeroize::Zeroizing::new(
        rpassword::prompt_password("Re-enter master password to continue: ")
            .context("re-entering the password needs an interactive terminal")?,
    );
    header
        .unlock(&again)
        .map(drop)
        .context("wrong master password")
}

/// Write `bytes` to `path` (owner-only on Unix), refusing to replace an existing file without `force`.
fn write_private(path: &std::path::Path, bytes: &[u8], force: bool) -> Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true);
    if force {
        opts.create(true).truncate(true);
    } else {
        opts.create_new(true);
    }
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts.open(path).with_context(|| {
        format!(
            "cannot write {} (use --force to overwrite an existing file)",
            path.display()
        )
    })?;
    f.write_all(bytes)?;
    f.sync_all()?;
    Ok(())
}

/// One line from stdin with the trailing newline removed.
fn read_stdin_line() -> Result<zeroize::Zeroizing<String>> {
    let mut s = String::new();
    std::io::stdin().read_line(&mut s)?;
    Ok(zeroize::Zeroizing::new(
        s.trim_end_matches(['\n', '\r']).to_string(),
    ))
}

/// Check a TOTP secret (base32 or otpauth:// URI) is usable before storing it.
fn validate_totp(t: &str) -> Result<()> {
    if t.starts_with("otpauth://") {
        Totp::from_uri(t)?;
    } else {
        Totp::from_base32(t)?;
    }
    Ok(())
}

fn confirm(question: &str) -> Result<bool> {
    eprint!("{question} [y/N] ");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(matches!(line.trim(), "y" | "Y" | "yes"))
}

/// Which items a lookup may return.
#[derive(Clone, Copy, PartialEq)]
enum Scope {
    Live,
    Trashed,
    /// Live items first, then trashed ones.
    Any,
}

fn find<'a>(items: &'a [Item], q: &str) -> Result<&'a Item> {
    find_in(items, q, Scope::Live)
}

fn find_in<'a>(items: &'a [Item], q: &str, scope: Scope) -> Result<&'a Item> {
    let wanted = |i: &&Item| match scope {
        Scope::Live => i.deleted_at.is_none(),
        Scope::Trashed => i.deleted_at.is_some(),
        Scope::Any => true,
    };
    // Live items sort before trashed ones, so an exact title match prefers the live item.
    let mut pool: Vec<&Item> = items.iter().filter(wanted).collect();
    pool.sort_by_key(|i| i.deleted_at.is_some());
    if let Some(i) = pool.iter().find(|i| i.title.eq_ignore_ascii_case(q)) {
        return Ok(i);
    }
    let hits: Vec<_> = pool.into_iter().filter(|i| i.matches(q)).collect();
    match hits.as_slice() {
        [] => bail!("no item matches '{q}'"),
        [one] => Ok(one),
        many => bail!(
            "'{q}' is ambiguous: {}",
            many.iter()
                .map(|i| if i.deleted_at.is_some() {
                    format!("{} (in trash)", i.title)
                } else {
                    i.title.clone()
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// "today", "1 day ago", "5 days ago".
fn days_ago(then: i64, now: i64) -> String {
    match ((now - then).max(0)) / 86_400 {
        0 => "today".into(),
        1 => "1 day ago".into(),
        n => format!("{n} days ago"),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = match cli.vault {
        Some(p) => p,
        None => default_path()?,
    };
    let mut rng = rand::rng();

    match cli.cmd {
        Cmd::Gen {
            length,
            no_symbols,
            avoid_ambiguous,
            passphrase,
            words,
            separator,
            capitalize,
        } => {
            if passphrase {
                let list = wordlist::eff_large();
                let phrase =
                    generator::generate_passphrase(&list, words, &separator, capitalize, &mut rng)?;
                println!("{}", *phrase);
                eprintln!(
                    "~{:.0} bits of entropy",
                    generator::passphrase_entropy_bits(list.len(), words)
                );
                return Ok(());
            }
            let spec = PasswordSpec {
                length,
                symbols: !no_symbols,
                avoid_ambiguous,
                ..Default::default()
            };
            let password = generator::generate_password(&spec, &mut rng)?;
            println!("{}", *password);
            let s = strength::estimate(&password, &[]);
            eprintln!("Strength: {}/4 ({})", s.score, s.label());
            return Ok(());
        }
        Cmd::ClipboardClear { after } => return clipboard::run_clearer(after),
        Cmd::Init(opts) => {
            if path.exists() {
                bail!("{} already exists", path.display());
            }
            let pw = new_master(&opts, &mut rng)?;
            let vault = UnlockedVault::create(&pw, KdfParams::DESKTOP, &mut rng)?;
            VaultFile {
                header: vault.header().clone(),
                records: vec![],
            }
            .save(&path)?;
            eprintln!("Created {}", path.display());
            return Ok(());
        }
        _ => {}
    }

    let mut vf = VaultFile::load(&path).with_context(|| format!("opening {}", path.display()))?;
    let pw = read_password("Master password: ")?;
    let mut vault = vf.header.unlock(&pw)?;
    let mut items: Vec<Item> = vf
        .records
        .iter()
        .map(|r| vault.open_record(r))
        .collect::<Result<_, _>>()?;

    match cli.cmd {
        Cmd::List { query } => {
            let mut shown: Vec<_> = items
                .iter()
                .filter(|i| i.deleted_at.is_none() && query.as_deref().is_none_or(|q| i.matches(q)))
                .collect();
            shown.sort_by_key(|i| i.title.to_lowercase());
            for i in shown {
                let user = i.field("username").map(|f| f.value.as_str()).unwrap_or("");
                println!("{}\t{}", i.title, user);
            }
        }
        Cmd::Get { query, field, copy } => {
            let item = find(&items, &query)?;
            let f = if field == "notes" {
                None
            } else {
                item.field(&field)
            };
            let what = format!("{field} of '{}'", item.title);
            match (field.as_str(), f) {
                ("notes", _) => output(&item.notes, &what, &copy)?,
                (_, Some(f)) => output(&f.value, &what, &copy)?,
                _ => bail!("no field '{field}' on '{}'", item.title),
            }
        }
        Cmd::Totp { query, copy } => {
            let item = find(&items, &query)?;
            let f = item.field("totp").context("item has no TOTP field")?;
            let t = if f.value.starts_with("otpauth://") {
                Totp::from_uri(&f.value)?
            } else {
                Totp::from_base32(&f.value)?
            };
            let ts = now() as u64;
            if copy.copy {
                output(
                    &t.code_at(ts),
                    &format!("TOTP code for '{}'", item.title),
                    &copy,
                )?;
                eprintln!("The code is valid for {}s more.", t.seconds_remaining(ts));
            } else {
                println!("{}\t({}s left)", t.code_at(ts), t.seconds_remaining(ts));
            }
        }
        Cmd::Add {
            title,
            username,
            url,
            password_stdin,
            generate,
            totp,
        } => {
            let password = if generate {
                generator::generate_password(&PasswordSpec::default(), &mut rng)?
            } else if password_stdin {
                read_stdin_line()?
            } else {
                zeroize::Zeroizing::new(rpassword::prompt_password("Item password: ")?)
            };
            let mut item = Item::login(&title, &username, &password, now(), Uuid::now_v7());
            item.urls.extend(url);
            if let Some(t) = totp {
                validate_totp(&t)?;
                item.fields.push(Field::secret("totp", &t));
            }
            let rec = vault.seal_item(&item, 1, &mut rng)?;
            vf.records.push(rec);
            vf.save(&path)?;
            if generate {
                println!("{}", *password);
            }
            eprintln!("Added '{title}'");
        }
        Cmd::Edit {
            query,
            title,
            username,
            url,
            notes,
            password,
            password_stdin,
            generate,
            totp,
            remove_totp,
            tag,
            untag,
        } => {
            let changes = title.is_some()
                || username.is_some()
                || url.is_some()
                || notes.is_some()
                || password
                || password_stdin
                || generate
                || totp.is_some()
                || remove_totp
                || !tag.is_empty()
                || !untag.is_empty();
            if !changes {
                bail!("nothing to change; pass at least one option (see `rk edit --help`)");
            }
            if let Some(t) = &totp {
                validate_totp(t)?;
            }
            let new_password = if generate {
                Some(generator::generate_password(
                    &PasswordSpec::default(),
                    &mut rng,
                )?)
            } else if password_stdin {
                Some(read_stdin_line()?)
            } else if password {
                Some(zeroize::Zeroizing::new(rpassword::prompt_password(
                    "New item password: ",
                )?))
            } else {
                None
            };

            let id = find(&items, &query)?.id;
            let idx = items.iter().position(|i| i.id == id).unwrap();
            let item = &mut items[idx];
            if let Some(t) = title {
                item.title = t;
            }
            if let Some(u) = username {
                item.set_field("username", &u, false);
            }
            if let Some(u) = url {
                item.urls = if u.is_empty() { vec![] } else { vec![u] };
            }
            if let Some(n) = notes {
                item.notes = n;
            }
            if let Some(p) = &new_password {
                item.set_field("password", p, true);
            }
            if let Some(t) = totp {
                item.set_field("totp", &t, true);
            }
            if remove_totp && !item.remove_field("totp") {
                eprintln!("note: '{}' has no TOTP secret", item.title);
            }
            for t in tag {
                if !item.tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
                    item.tags.push(t);
                }
            }
            item.tags
                .retain(|x| !untag.iter().any(|u| u.eq_ignore_ascii_case(x)));
            item.updated_at = now();

            let old = vf.records.iter_mut().find(|r| r.item_id == id).unwrap();
            *old = vault.seal_item(item, old.rev + 1, &mut rng)?;
            let title = item.title.clone();
            vf.save(&path)?;
            if generate {
                if let Some(p) = &new_password {
                    println!("{}", **p);
                }
            }
            eprintln!("Updated '{title}'");
        }
        Cmd::Trash => {
            let mut trashed: Vec<_> = items.iter().filter(|i| i.deleted_at.is_some()).collect();
            trashed.sort_by_key(|i| std::cmp::Reverse(i.deleted_at));
            let t = now();
            for i in &trashed {
                let user = i.field("username").map(|f| f.value.as_str()).unwrap_or("");
                println!(
                    "{}\t{}\tdeleted {}",
                    i.title,
                    user,
                    days_ago(i.deleted_at.unwrap_or(t), t)
                );
            }
            eprintln!(
                "{} item(s) in trash. `rk restore <item>` brings one back; `rk rm --purge <item>` deletes it forever.",
                trashed.len()
            );
        }
        Cmd::Restore { query } => {
            let id = find_in(&items, &query, Scope::Trashed)
                .context("nothing in the trash matches (see `rk trash`)")?
                .id;
            let idx = items.iter().position(|i| i.id == id).unwrap();
            let item = &mut items[idx];
            item.deleted_at = None;
            item.updated_at = now();
            let old = vf.records.iter_mut().find(|r| r.item_id == id).unwrap();
            *old = vault.seal_item(item, old.rev + 1, &mut rng)?;
            let title = item.title.clone();
            vf.save(&path)?;
            eprintln!("Restored '{title}'");
        }
        Cmd::Rm { query, purge } => {
            let scope = if purge { Scope::Any } else { Scope::Live };
            let id = match find_in(&items, &query, scope) {
                Ok(i) => i.id,
                Err(e) => {
                    if !purge && find_in(&items, &query, Scope::Trashed).is_ok() {
                        bail!("'{query}' is already in the trash (`rk restore` it, or `rk rm --purge` to delete it forever)");
                    }
                    return Err(e);
                }
            };
            let idx = items.iter().position(|i| i.id == id).unwrap();
            if purge {
                vf.records.retain(|r| r.item_id != id);
            } else {
                let item = &mut items[idx];
                item.deleted_at = Some(now());
                item.updated_at = now();
                let old = vf.records.iter_mut().find(|r| r.item_id == id).unwrap();
                *old = vault.seal_item(item, old.rev + 1, &mut rng)?;
            }
            vf.save(&path)?;
            eprintln!("{}", if purge { "Deleted" } else { "Moved to trash" });
        }
        Cmd::Passwd(opts) => {
            let new = new_master(&opts, &mut rng)?;
            vault.change_password(&new, KdfParams::DESKTOP, &mut rng)?;
            vf.header = vault.header().clone();
            vf.save(&path)?;
            eprintln!("Master password changed");
        }
        Cmd::Import {
            file,
            map,
            dry_run,
            yes,
        } => {
            let overrides = map
                .iter()
                .map(|m| {
                    m.split_once('=')
                        .map(|(f, c)| (f.trim().to_string(), c.trim().to_string()))
                        .with_context(|| format!("--map expects FIELD=COLUMN, got '{m}'"))
                })
                .collect::<Result<Vec<_>>>()?;
            let data = zeroize::Zeroizing::new(
                std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?,
            );
            let report = transfer::import_csv(&data, &overrides, &items, now(), Uuid::now_v7)?;
            eprintln!(
                "Detected format: {}. {} to import, {} duplicate(s) skipped, {} empty row(s) skipped.",
                report.detected,
                report.items.len(),
                report.skipped_duplicates,
                report.skipped_empty
            );
            for w in &report.warnings {
                eprintln!("warning: {w}");
            }
            for i in &report.items {
                let user = i.field("username").map(|f| f.value.as_str()).unwrap_or("");
                println!(
                    "{}\t{}\t{}",
                    i.title,
                    user,
                    i.urls.first().map_or("", |u| u)
                );
            }
            if report.items.is_empty() || dry_run {
                return Ok(());
            }
            if !yes && !confirm(&format!("Import {} item(s)?", report.items.len()))? {
                bail!("cancelled; nothing was imported");
            }
            for item in &report.items {
                vf.records.push(vault.seal_item(item, 1, &mut rng)?);
            }
            vf.save(&path)?;
            eprintln!("Imported {} item(s)", report.items.len());
            eprintln!(
                "The CSV still holds your passwords in plain text. Delete it securely: {}",
                file.display()
            );
        }
        Cmd::Export {
            format,
            output,
            force,
        } => {
            if let ExportFormat::Vault = format {
                let out = output.context("--format vault needs --output FILE")?;
                if out == path {
                    bail!("output must differ from the vault file");
                }
                if out.exists() && !force {
                    bail!("{} exists (use --force to overwrite)", out.display());
                }
                vf.save(&out)?;
                eprintln!("Wrote encrypted copy to {}", out.display());
                return Ok(());
            }
            eprintln!(
                "WARNING: this export is PLAINTEXT. Anyone who can read it can read every password in it."
            );
            reauthenticate(&vf.header)?;
            let text = match format {
                ExportFormat::Csv => transfer::export_csv(&items)?,
                _ => transfer::export_json(&items)?,
            };
            match output {
                Some(out) => {
                    write_private(&out, text.as_bytes(), force)?;
                    eprintln!(
                        "Wrote {}. Delete it securely when you are done.",
                        out.display()
                    );
                }
                None => print!("{}", *text),
            }
        }
        Cmd::Init(_) | Cmd::Gen { .. } | Cmd::ClipboardClear { .. } => unreachable!(),
    }
    Ok(())
}
