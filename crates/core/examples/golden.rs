//! Regenerates the golden vault in `spec/vectors/` deterministically (fixed seed, fixed ids, fixed times).
//!
//!     cargo run -p rahasyakosh-core --example golden -- spec/vectors
//!
//! The committed files are a compatibility contract: every future version must still open them. Only
//! regenerate when introducing a NEW format version, and then keep the old files and add new ones.
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::{Field, Item, ItemKind};
use rahasyakosh_core::vault::UnlockedVault;
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use std::{fs, path::PathBuf};
use uuid::Uuid;

pub const PASSWORD: &str = "रहस्यकोश-golden-1";
const T0: i64 = 1_700_000_000;

fn id(n: u128) -> Uuid {
    Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0000 + n)
}

fn items() -> Vec<(Item, u64)> {
    let mut github = Item::login(
        "GitHub",
        "alice@example.com",
        "gh-correct-horse-9!",
        T0,
        id(1),
    );
    github.urls.push("https://github.com/login".into());
    github.tags = vec!["work".into(), "dev".into()];
    github.favourite = true;
    github
        .fields
        .push(Field::secret("totp", "JBSWY3DPEHPK3PXP"));
    github.notes = "Recovery codes are in the safe.".into();

    let mut wifi = Item::new(ItemKind::Note, "Wi-Fi 🔐", T0 + 60, id(2));
    wifi.notes =
        "Network: HomeNet\nKey: रहस्य-kosh-2024\nRouter admin is on the back of the device.".into();

    let mut bank = Item::login(
        "बैंक (Bank)",
        "me",
        "bank p@ss \"quoted\" \\ slash",
        T0 + 120,
        id(3),
    );
    bank.urls.push("https://bank.example/login".into());
    bank.updated_at = T0 + 3600;

    let mut old = Item::login("Old site", "bob", "old-password", T0 + 180, id(4));
    old.deleted_at = Some(T0 + 100_000);

    let mut long = Item::new(ItemKind::Note, "Long note", T0 + 240, id(5));
    long.notes = "0123456789abcdef".repeat(40);
    long.fields.push(Field::text("custom", "visible value"));
    long.fields.push(Field::secret("pin", "4821"));

    // (item, revision): the bank login was edited once.
    vec![(github, 1), (wifi, 1), (bank, 2), (old, 1), (long, 1)]
}

fn main() {
    let dir: PathBuf = std::env::args()
        .nth(1)
        .expect("usage: golden <output dir>")
        .into();
    let mut rng = ChaCha20Rng::seed_from_u64(0x7261_6861_7379_6131);
    let vault = UnlockedVault::create(PASSWORD, KdfParams::DESKTOP, &mut rng).unwrap();
    let items = items();
    let records: Vec<_> = items
        .iter()
        .map(|(i, rev)| vault.seal_item(i, *rev, &mut rng).unwrap())
        .collect();

    let file = serde_json::json!({ "header": vault.header(), "records": records });
    let expected: Vec<&Item> = items.iter().map(|(i, _)| i).collect();
    fs::write(
        dir.join("golden-v1.rkv"),
        serde_json::to_string_pretty(&file).unwrap() + "\n",
    )
    .unwrap();
    fs::write(
        dir.join("golden-v1.expected.json"),
        serde_json::to_string_pretty(&expected).unwrap() + "\n",
    )
    .unwrap();
}
