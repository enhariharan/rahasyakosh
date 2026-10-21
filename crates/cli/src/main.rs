//! `rk` — Rahasyakosh command-line password manager (M1b: single-file vault).
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
    /// List items (optionally filtered)
    List { query: Option<String> },
    /// Print a field of the best-matching item (default: password)
    Get {
        query: String,
        #[arg(short, long, default_value = "password")]
        field: String,
    },
    /// Print the current TOTP code
    Totp { query: String },
    /// Move an item to trash (or --purge to delete forever)
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

fn confirm(question: &str) -> Result<bool> {
    eprint!("{question} [y/N] ");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(matches!(line.trim(), "y" | "Y" | "yes"))
}

fn find<'a>(items: &'a [Item], q: &str) -> Result<&'a Item> {
    let live = || items.iter().filter(|i| i.deleted_at.is_none());
    if let Some(i) = live().find(|i| i.title.eq_ignore_ascii_case(q)) {
        return Ok(i);
    }
    let hits: Vec<_> = live().filter(|i| i.matches(q)).collect();
    match hits.as_slice() {
        [] => bail!("no item matches '{q}'"),
        [one] => Ok(one),
        many => bail!(
            "'{q}' is ambiguous: {}",
            many.iter()
                .map(|i| i.title.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
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
        Cmd::Get { query, field } => {
            let item = find(&items, &query)?;
            let f = if field == "notes" {
                None
            } else {
                item.field(&field)
            };
            match (field.as_str(), f) {
                ("notes", _) => println!("{}", item.notes),
                (_, Some(f)) => println!("{}", f.value),
                _ => bail!("no field '{field}' on '{}'", item.title),
            }
        }
        Cmd::Totp { query } => {
            let item = find(&items, &query)?;
            let f = item.field("totp").context("item has no TOTP field")?;
            let t = if f.value.starts_with("otpauth://") {
                Totp::from_uri(&f.value)?
            } else {
                Totp::from_base32(&f.value)?
            };
            let ts = now() as u64;
            println!("{}\t({}s left)", t.code_at(ts), t.seconds_remaining(ts));
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
                let mut s = String::new();
                std::io::stdin().read_line(&mut s)?;
                zeroize::Zeroizing::new(s.trim_end_matches(['\n', '\r']).to_string())
            } else {
                zeroize::Zeroizing::new(rpassword::prompt_password("Item password: ")?)
            };
            let mut item = Item::login(&title, &username, &password, now(), Uuid::now_v7());
            item.urls.extend(url);
            if let Some(t) = totp {
                if t.starts_with("otpauth://") {
                    Totp::from_uri(&t)?;
                } else {
                    Totp::from_base32(&t)?;
                }
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
        Cmd::Rm { query, purge } => {
            let id = find(&items, &query)?.id;
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
        Cmd::Init(_) | Cmd::Gen { .. } => unreachable!(),
    }
    Ok(())
}
