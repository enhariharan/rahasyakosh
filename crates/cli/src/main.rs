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
};
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
    Init,
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
    /// Generate a password
    Gen {
        #[arg(short, long, default_value_t = 20)]
        length: usize,
        #[arg(long)]
        no_symbols: bool,
        #[arg(long)]
        avoid_ambiguous: bool,
    },
    /// Change the master password
    Passwd,
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
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

fn new_password() -> Result<zeroize::Zeroizing<String>> {
    let p = read_password("New master password: ")?;
    if std::env::var("RK_PASSWORD").is_err() {
        let again = rpassword::prompt_password("Repeat: ")?;
        if *again != *p {
            bail!("passwords do not match");
        }
    }
    if p.chars().count() < 8 {
        bail!("master password must be at least 8 characters");
    }
    Ok(p)
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
            many.iter().map(|i| i.title.as_str()).collect::<Vec<_>>().join(", ")
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
        Cmd::Gen { length, no_symbols, avoid_ambiguous } => {
            let spec = PasswordSpec { length, symbols: !no_symbols, avoid_ambiguous, ..Default::default() };
            println!("{}", *generator::generate_password(&spec, &mut rng)?);
            return Ok(());
        }
        Cmd::Init => {
            if path.exists() {
                bail!("{} already exists", path.display());
            }
            let pw = new_password()?;
            let vault = UnlockedVault::create(&pw, KdfParams::DESKTOP, &mut rng)?;
            VaultFile { header: vault.header().clone(), records: vec![] }.save(&path)?;
            eprintln!("Created {}", path.display());
            return Ok(());
        }
        _ => {}
    }

    let mut vf = VaultFile::load(&path).with_context(|| format!("opening {}", path.display()))?;
    let pw = read_password("Master password: ")?;
    let mut vault = vf.header.unlock(&pw)?;
    let mut items: Vec<Item> = vf.records.iter().map(|r| vault.open_record(r)).collect::<Result<_, _>>()?;

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
            let f = if field == "notes" { None } else { item.field(&field) };
            match (field.as_str(), f) {
                ("notes", _) => println!("{}", item.notes),
                (_, Some(f)) => println!("{}", f.value),
                _ => bail!("no field '{field}' on '{}'", item.title),
            }
        }
        Cmd::Totp { query } => {
            let item = find(&items, &query)?;
            let f = item.field("totp").context("item has no TOTP field")?;
            let t = if f.value.starts_with("otpauth://") { Totp::from_uri(&f.value)? } else { Totp::from_base32(&f.value)? };
            let ts = now() as u64;
            println!("{}\t({}s left)", t.code_at(ts), t.seconds_remaining(ts));
        }
        Cmd::Add { title, username, url, password_stdin, generate, totp } => {
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
                if t.starts_with("otpauth://") { Totp::from_uri(&t)?; } else { Totp::from_base32(&t)?; }
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
        Cmd::Passwd => {
            let new = new_password()?;
            vault.change_password(&new, KdfParams::DESKTOP, &mut rng)?;
            vf.header = vault.header().clone();
            vf.save(&path)?;
            eprintln!("Master password changed");
        }
        Cmd::Init | Cmd::Gen { .. } => unreachable!(),
    }
    Ok(())
}
