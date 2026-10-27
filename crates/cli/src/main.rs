//! `rk` — Rahasyakosh command-line password manager.
//!
//! Every command unlocks the vault, does its job and exits (the CLI keeps no session), so keys never outlive it.
//! The vault is a SQLite database; `rk export -f vault` writes (and `rk migrate` reads) a portable JSON bundle.
mod clipboard;
mod file;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use rahasyakosh_app::{Config, Scope, Vault};
use rahasyakosh_core::{
    crypto::KdfParams,
    generator::{self, PasswordSpec},
    item::{Field, Item, ItemVersion},
    totp::Totp,
    wordlist,
};
use rahasyakosh_store::{SqliteStore, Store};
use rahasyakosh_strength as strength;
use rahasyakosh_transfer as transfer;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "rk", version, about = "Rahasyakosh password manager")]
struct Cli {
    /// Vault file (default: $RK_VAULT or ~/.local/share/rahasyakosh/vault.db)
    #[arg(long, global = true, env = "RK_VAULT")]
    vault: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a new vault
    Init {
        #[command(flatten)]
        master: MasterOpts,
        /// Also lock the vault with a random Secret Key (shown once, kept apart from the vault file)
        #[arg(long)]
        secret_key: bool,
    },
    /// Manage the vault's Secret Key: a second secret, needed with the master password to open the vault
    SecretKey {
        #[command(subcommand)]
        action: SecretKeyCmd,
    },
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
        /// Pin the item: favourites are listed first
        #[arg(long, conflicts_with = "no_favourite")]
        favourite: bool,
        #[arg(long)]
        no_favourite: bool,
    },
    /// Show the earlier versions of an item
    History { query: String },
    /// Make an earlier version current again (the present one is kept in the history)
    Revert {
        query: String,
        /// Version number from `rk history`
        version: usize,
    },
    /// Load a JSON vault file (an old-format vault, or a backup from `rk export -f vault`) into a NEW vault
    Migrate {
        /// The JSON vault file to read. The new vault is created at --vault.
        file: PathBuf,
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
    /// Export the vault. csv, json and txt are PLAINTEXT and ask for the master password again.
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
    /// Plaintext, readable text file (every field of every login)
    Txt,
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

#[derive(Subcommand)]
enum SecretKeyCmd {
    /// Add a Secret Key to this vault (shown once; print it and keep it somewhere safe)
    Enable,
    /// Show the Secret Key again (to print a new recovery sheet)
    Show,
    /// Remove the Secret Key; the master password alone opens the vault again
    Disable,
}

/// How the new master password is chosen.
#[derive(clap::Args)]
struct MasterOpts {
    /// Generate a random master passphrase (EFF large wordlist) instead of typing a password
    #[arg(short, long)]
    passphrase: bool,
    /// Number of words in the generated passphrase (at least 5)
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
    Ok(PathBuf::from(home).join(".local/share/rahasyakosh/vault.db"))
}

/// Master password from $RK_PASSWORD (for scripting) or an interactive prompt.
fn read_password(prompt: &str) -> Result<zeroize::Zeroizing<String>> {
    if let Ok(p) = std::env::var("RK_PASSWORD") {
        return Ok(zeroize::Zeroizing::new(p));
    }
    Ok(zeroize::Zeroizing::new(rpassword::prompt_password(prompt)?))
}

/// Fewest words `--passphrase` will make for a master passphrase: 5 words of the EFF list are about 64 bits.
const MIN_PASSPHRASE_WORDS: usize = 5;

/// New master password: typed twice, or generated (shown once, then re-typed to confirm it was saved).
fn new_master(
    opts: &MasterOpts,
    rng: &mut impl rand::CryptoRng,
) -> Result<zeroize::Zeroizing<String>> {
    if !opts.passphrase {
        return new_password(opts.allow_weak);
    }
    if opts.words < MIN_PASSPHRASE_WORDS {
        bail!(
            "a master passphrase needs at least {MIN_PASSPHRASE_WORDS} words (about 64 bits); {} would be too easy to guess",
            opts.words
        );
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

/// A vault file can ask for a very expensive key derivation, and the header is read before anything is checked. Say so,
/// and when someone is at the terminal let them back out.
fn confirm_costly_unlock(kdf: &KdfParams) -> Result<()> {
    if !kdf.is_unusually_costly() {
        return Ok(());
    }
    eprintln!(
        "Note: this vault asks for {} MiB of memory and {} passes to unlock (this program's default is {} MiB and {}), so unlocking will be slow.",
        kdf.m_kib / 1024,
        kdf.t,
        KdfParams::DESKTOP.m_kib / 1024,
        KdfParams::DESKTOP.t
    );
    if std::env::var("RK_PASSWORD").is_err() {
        eprint!("Continue? [y/N] ");
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            bail!("cancelled");
        }
    }
    Ok(())
}

/// The Secret Key for the vault in `store`, if it needs one: from this device's key folder, `RK_SECRET_KEY`, or by
/// asking. The flag says whether it was typed (and so is worth keeping).
fn find_secret_key(
    store: &SqliteStore,
) -> Result<Option<(rahasyakosh_core::secret_key::SecretKey, bool)>> {
    use rahasyakosh_core::secret_key::SecretKey;
    let header = store.header()?;
    if !header.secret_key {
        return Ok(None);
    }
    if let Some(dir) = rahasyakosh_app::SecretKeyDir::default_dir() {
        if let Some(k) = dir.load(header.vault_id) {
            return Ok(Some((k, false)));
        }
    }
    let text = match std::env::var("RK_SECRET_KEY") {
        Ok(t) => zeroize::Zeroizing::new(t),
        Err(_) => zeroize::Zeroizing::new(
            rpassword::prompt_password("Secret Key (RK1-XXXX-...): ").context(
                "this vault needs its Secret Key; set RK_SECRET_KEY or run in a terminal",
            )?,
        ),
    };
    Ok(Some((SecretKey::parse(&text)?, true)))
}

/// Save the Secret Key on this device, and when `sheet` is set show it as a recovery sheet.
fn keep_secret_key(vault: Uuid, key: &rahasyakosh_core::secret_key::SecretKey, sheet: bool) {
    if let Some(dir) = rahasyakosh_app::SecretKeyDir::default_dir() {
        match dir.save(vault, key) {
            Ok(()) if !sheet => eprintln!("Secret Key kept on this device."),
            Ok(()) => {}
            Err(e) => eprintln!("Could not keep the Secret Key on this device: {e}"),
        }
    }
    if sheet {
        print_recovery_sheet(key);
    }
}

fn print_recovery_sheet(key: &rahasyakosh_core::secret_key::SecretKey) {
    eprintln!(
        "\nYour Secret Key (copy it down; print it and keep it somewhere safe):\n\n    {}\n\n\
         It is needed together with your master password on any new device, and it cannot be recovered from the vault.\n\
         This device keeps a copy apart from the vault file.",
        key.to_text().as_str()
    );
}

/// Ask for the master password again and check it against the vault header.
fn reauthenticate(
    header: &rahasyakosh_core::vault::VaultHeader,
    secret_key: Option<&rahasyakosh_core::secret_key::SecretKey>,
) -> Result<()> {
    let again = zeroize::Zeroizing::new(
        rpassword::prompt_password("Re-enter master password to continue: ")
            .context("re-entering the password needs an interactive terminal")?,
    );
    header
        .unlock_with(&again, secret_key)
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

/// "today", "1 day ago", "5 days ago".
fn days_ago(then: i64, now: i64) -> String {
    match ((now - then).max(0)) / 86_400 {
        0 => "today".into(),
        1 => "1 day ago".into(),
        n => format!("{n} days ago"),
    }
}

/// Open the vault at `path` and unlock it. Refuses files that are not a vault, and points old-format files at `rk migrate`.
fn open_vault(path: &Path) -> Result<Vault<SqliteStore>> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("no vault at {} (create one with `rk init`)", path.display()))?;
    let mut head = [0u8; 16];
    let n = file.read(&mut head)?;
    let head = &head[..n];
    if !head.is_empty() && head != b"SQLite format 3\0" {
        if head.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
            bail!(
                "{} is in the old vault file format. Convert it with: rk migrate {} --vault <NEW FILE>",
                path.display(),
                path.display()
            );
        }
        bail!("{} is not a Rahasyakosh vault", path.display());
    }
    drop(file);
    let store = SqliteStore::open(path)?;
    confirm_costly_unlock(&store.header()?.kdf)?;
    let pw = read_password("Master password: ")?;
    let secret_key = find_secret_key(&store)?;
    let mut highwater =
        rahasyakosh_app::HighWater::default_path().map(rahasyakosh_app::HighWater::at);
    let last_seen = highwater
        .as_ref()
        .and_then(|h| h.last_seen(store.header().ok()?.vault_id));
    let mut vault = Vault::try_unlock_full(
        store,
        &pw,
        secret_key.as_ref().map(|(k, _)| k),
        Config::default(),
        last_seen,
    )
    .map_err(|f| f.error)?;
    if let Some((key, true)) = &secret_key {
        // Typed in just now and it worked: keep it on this device so it is not asked for again.
        keep_secret_key(vault.vault_id(), key, false);
    }
    for w in vault.warnings() {
        // Warnings name item ids and numbers only, never text taken from the stored data.
        eprintln!("Warning: {w}.");
    }
    if let Some(h) = highwater.take() {
        vault.track(h);
    }
    if !vault.unreadable().is_empty() {
        // Only ids are printed: the parser's reason can quote text from the damaged or planted data.
        let ids: Vec<String> = vault
            .unreadable()
            .iter()
            .map(|s| s.id.map_or_else(|| s.source.clone(), |id| id.to_string()))
            .collect();
        eprintln!(
            "Warning: {} stored item(s) could not be read and were skipped (a sync conflict, a damaged file, or tampering). Your other items are shown. Skipped: {}",
            ids.len(),
            ids.join(", ")
        );
    }
    let purged = vault.purge_expired(now(), &mut rand::rng())?;
    if purged > 0 {
        eprintln!(
            "Deleted {purged} item(s) that had been in the trash for {} days or more.",
            vault.config().trash_retention_days
        );
    }
    Ok(vault)
}

/// "title, notes": which parts of the content differ between two versions. Never shows values.
fn changed_parts(a: &ItemVersion, b: &ItemVersion) -> String {
    let mut parts: Vec<String> = Vec::new();
    if a.title != b.title {
        parts.push("title".into());
    }
    if a.urls != b.urls {
        parts.push("url".into());
    }
    if a.tags != b.tags {
        parts.push("tags".into());
    }
    if a.notes != b.notes {
        parts.push("notes".into());
    }
    if a.favourite != b.favourite {
        parts.push("favourite".into());
    }
    let mut names: Vec<&str> = a
        .fields
        .iter()
        .chain(&b.fields)
        .map(|f| f.name.as_str())
        .collect();
    names.sort_unstable();
    names.dedup();
    for name in names {
        let find = |v: &ItemVersion| {
            v.fields
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.value.clone())
        };
        if find(a) != find(b) {
            parts.push(name.to_string());
        }
    }
    if parts.is_empty() {
        "(no visible change)".into()
    } else {
        parts.join(", ")
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = match cli.vault {
        Some(p) => p,
        None => default_path()?,
    };
    let mut rng = rand::rng();

    // Commands that do not need an existing, unlocked vault.
    match &cli.cmd {
        Cmd::Gen {
            length,
            no_symbols,
            avoid_ambiguous,
            passphrase,
            words,
            separator,
            capitalize,
        } => {
            if *passphrase {
                let list = wordlist::eff_large();
                let phrase = generator::generate_passphrase(
                    &list,
                    *words,
                    separator,
                    *capitalize,
                    &mut rng,
                )?;
                println!("{}", *phrase);
                eprintln!(
                    "~{:.0} bits of entropy",
                    generator::passphrase_entropy_bits(list.len(), *words)
                );
                return Ok(());
            }
            let spec = PasswordSpec {
                length: *length,
                symbols: !no_symbols,
                avoid_ambiguous: *avoid_ambiguous,
                ..Default::default()
            };
            let password = generator::generate_password(&spec, &mut rng)?;
            println!("{}", *password);
            let s = strength::estimate(&password, &[]);
            eprintln!("Strength: {}/4 ({})", s.score, s.label());
            return Ok(());
        }
        Cmd::ClipboardClear { after } => return clipboard::run_clearer(*after),
        Cmd::Init {
            master: opts,
            secret_key,
        } => {
            if path.exists() {
                bail!("{} already exists", path.display());
            }
            let pw = new_master(opts, &mut rng)?;
            let mut vault = Vault::create(
                SqliteStore::open(&path)?,
                &pw,
                KdfParams::DESKTOP,
                Config::default(),
                &mut rng,
            )?;
            if *secret_key {
                let sk = vault.set_secret_key(&pw, &mut rng)?;
                keep_secret_key(vault.vault_id(), &sk, true);
            }
            eprintln!("Created {}", path.display());
            return Ok(());
        }
        Cmd::Migrate { file } => {
            if path.exists() {
                bail!(
                    "{} already exists; migrate into a new file (--vault)",
                    path.display()
                );
            }
            let bundle = file::load(file).with_context(|| format!("reading {}", file.display()))?;
            let pw = read_password("Master password of the old vault: ")?;
            // Copy first, then prove the copy opens with the password: a wrong password or damaged data must not
            // leave a half-built vault behind.
            let result = (|| -> Result<usize> {
                let mut store = SqliteStore::open(&path)?;
                store.create(&bundle.header)?;
                store.put_many(&bundle.records)?;
                Ok(Vault::unlock(store, &pw, Config::default())?.len())
            })();
            match result {
                Ok(count) => {
                    eprintln!("Migrated {count} item(s) into {}.", path.display());
                    eprintln!(
                        "Your master password is unchanged. Keep {} until you are sure everything is there.",
                        file.display()
                    );
                    return Ok(());
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&path);
                    for ext in ["-wal", "-shm"] {
                        let mut side = path.clone().into_os_string();
                        side.push(ext);
                        let _ = std::fs::remove_file(side);
                    }
                    return Err(e.context("migration failed; nothing was created"));
                }
            }
        }
        _ => {}
    }

    let mut vault = open_vault(&path)?;

    match cli.cmd {
        Cmd::List { query } => {
            for i in vault.search(query.as_deref().unwrap_or("")) {
                let user = i.field("username").map(|f| f.value.as_str()).unwrap_or("");
                if i.favourite {
                    println!("{}\t{}\t★", i.title, user);
                } else {
                    println!("{}\t{}", i.title, user);
                }
            }
        }
        Cmd::Get { query, field, copy } => {
            let item = vault.resolve(&query, Scope::Live)?;
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
            let item = vault.resolve(&query, Scope::Live)?;
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
            vault.add(item, &mut rng)?;
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
            favourite,
            no_favourite,
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
                || !untag.is_empty()
                || favourite
                || no_favourite;
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

            let id = vault.resolve(&query, Scope::Live)?.id;
            let mut had_no_totp = false;
            let changed = vault.edit(id, now(), &mut rng, |item| {
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
                if remove_totp {
                    had_no_totp = !item.remove_field("totp");
                }
                for t in tag {
                    if !item.tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
                        item.tags.push(t);
                    }
                }
                item.tags
                    .retain(|x| !untag.iter().any(|u| u.eq_ignore_ascii_case(x)));
                if favourite {
                    item.favourite = true;
                }
                if no_favourite {
                    item.favourite = false;
                }
            })?;
            let title = vault.get(id).map(|i| i.title.clone()).unwrap_or_default();
            if had_no_totp {
                eprintln!("note: '{title}' has no TOTP secret");
            }
            if generate {
                if let Some(p) = &new_password {
                    println!("{}", **p);
                }
            }
            if changed {
                eprintln!("Updated '{title}'");
            } else {
                eprintln!("No changes: '{title}' already has these values");
            }
        }
        Cmd::History { query } => {
            let item = vault.resolve(&query, Scope::Any)?;
            let t = now();
            let mut newer = item.snapshot(t);
            for (n, v) in item.history.iter().enumerate() {
                println!(
                    "{n}\treplaced {}\t{}",
                    days_ago(v.replaced_at, t),
                    changed_parts(v, &newer)
                );
                newer = v.clone();
            }
            eprintln!(
                "{} earlier version(s) of '{}'. `rk revert <item> <number>` makes one current again.",
                item.history.len(),
                item.title
            );
        }
        Cmd::Revert { query, version } => {
            let id = vault.resolve(&query, Scope::Live)?.id;
            vault.revert(id, version, now(), &mut rng)?;
            let title = vault.get(id).map(|i| i.title.clone()).unwrap_or_default();
            eprintln!("Reverted '{title}' to version {version} (the previous content is kept in its history)");
        }
        Cmd::Trash => {
            let mut trashed = vault.items(Scope::Trashed);
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
                "{} item(s) in trash; they are deleted automatically after {} days. `rk restore <item>` brings one back; `rk rm --purge <item>` deletes it now.",
                trashed.len(),
                vault.config().trash_retention_days
            );
        }
        Cmd::Restore { query } => {
            let id = vault
                .resolve(&query, Scope::Trashed)
                .context("nothing in the trash matches (see `rk trash`)")?
                .id;
            vault.restore(id, now(), &mut rng)?;
            let title = vault.get(id).map(|i| i.title.clone()).unwrap_or_default();
            eprintln!("Restored '{title}'");
        }
        Cmd::Rm { query, purge } => {
            let scope = if purge { Scope::Any } else { Scope::Live };
            let id = match vault.resolve(&query, scope) {
                Ok(i) => i.id,
                Err(e) => {
                    if !purge && vault.resolve(&query, Scope::Trashed).is_ok() {
                        bail!("'{query}' is already in the trash (`rk restore` it, or `rk rm --purge` to delete it forever)");
                    }
                    return Err(e.into());
                }
            };
            if purge {
                vault.purge(id, &mut rng)?;
            } else {
                vault.trash(id, now(), &mut rng)?;
            }
            eprintln!("{}", if purge { "Deleted" } else { "Moved to trash" });
        }
        Cmd::SecretKey { action } => match action {
            SecretKeyCmd::Enable => {
                if vault.has_secret_key() {
                    bail!("this vault already has a Secret Key (`rk secret-key show` prints it)");
                }
                let pw = read_password("Master password (to confirm): ")?;
                let sk = vault.set_secret_key(&pw, &mut rng)?;
                keep_secret_key(vault.vault_id(), &sk, true);
            }
            SecretKeyCmd::Show => {
                let Some(sk) = vault.secret_key() else {
                    bail!("this vault has no Secret Key");
                };
                reauthenticate(&vault.store().header()?, Some(sk))?;
                print_recovery_sheet(sk);
            }
            SecretKeyCmd::Disable => {
                if !vault.has_secret_key() {
                    bail!("this vault has no Secret Key");
                }
                let pw = read_password("Master password (to confirm): ")?;
                let vault_id = vault.vault_id();
                vault.clear_secret_key(&pw, &mut rng)?;
                if let Some(dir) = rahasyakosh_app::SecretKeyDir::default_dir() {
                    let _ = dir.remove(vault_id);
                }
                eprintln!(
                    "The Secret Key is removed. The master password alone opens this vault now."
                );
            }
        },
        Cmd::Passwd(opts) => {
            let new = new_master(&opts, &mut rng)?;
            vault.change_password(&new, KdfParams::DESKTOP, &mut rng)?;
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
            let existing: Vec<Item> = vault.items(Scope::Any).into_iter().cloned().collect();
            let report = transfer::import_csv(&data, &overrides, &existing, now(), Uuid::now_v7)?;
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
            let count = report.items.len();
            vault.add_many(report.items, &mut rng)?;
            eprintln!("Imported {count} item(s)");
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
                file::save(
                    &transfer::Bundle {
                        header: vault.store().header()?,
                        records: vault.store().all()?,
                    },
                    &out,
                )?;
                eprintln!("Wrote encrypted copy to {}", out.display());
                return Ok(());
            }
            eprintln!(
                "WARNING: this export is PLAINTEXT. Anyone who can read it can read every password in it."
            );
            reauthenticate(&vault.store().header()?, vault.secret_key())?;
            let items: Vec<Item> = vault.items(Scope::Any).into_iter().cloned().collect();
            let text = match format {
                ExportFormat::Csv => transfer::export_csv(&items)?,
                ExportFormat::Txt => transfer::export_txt(&items),
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
        Cmd::Init { .. } | Cmd::Gen { .. } | Cmd::ClipboardClear { .. } | Cmd::Migrate { .. } => {
            unreachable!()
        }
    }
    Ok(())
}
