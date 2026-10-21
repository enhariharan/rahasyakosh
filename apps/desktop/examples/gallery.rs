//! Development aid: open a screen of the app on sample data, already unlocked, to look at the design.
//!
//!     GALLERY=detail cargo run --example gallery        # detail | revealed | list | form | settings | trash | setup
//!
//! It uses an in-memory vault, never touches your real one, and is not part of the shipped program.
use rahasyakosh_app::{Session, Settings};
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::Item;
use rahasyakosh_desktop::model::{Model, SystemClipboard, View};
use rahasyakosh_store::{SqliteStore, Uuid};
use rand::SeedableRng;

fn main() {
    let view = std::env::var("GALLERY").unwrap_or_else(|_| "detail".into());
    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let settings = Settings {
        kdf_memory_mib: KdfParams::MIN_M_KIB.div_ceil(1024),
        kdf_passes: KdfParams::MIN_T,
        idle_lock_secs: 0,
        reveal_secs: 0,
        ..Settings::default()
    };
    let mut session = Session::create(
        SqliteStore::open_in_memory().unwrap(),
        "gallery",
        settings,
        now,
        &mut rng,
    )
    .unwrap();
    {
        let v = session.vault(now).unwrap();
        let mut gh = Item::login(
            "GitHub",
            "alice@example.com",
            "correct-horse-9!",
            now - 90 * 86_400,
            Uuid::from_u128(1),
        );
        gh.urls.push("https://github.com/login".into());
        gh.tags = vec!["work".into(), "dev".into()];
        gh.set_field("totp", "JBSWY3DPEHPK3PXP", true);
        gh.notes = "Recovery codes are in the safe.".into();
        gh.favourite = true;
        v.add(gh, &mut rng).unwrap();
        for (n, (t, u)) in [
            ("Bank of Examples", "me"),
            ("Mail", "me@example.org"),
            ("Wi-Fi at home", ""),
            ("Zeta shop", "alice"),
        ]
        .iter()
        .enumerate()
        {
            v.add(
                Item::login(t, u, "pw", now, Uuid::from_u128(n as u128 + 2)),
                &mut rng,
            )
            .unwrap();
        }
        v.edit(Uuid::from_u128(1), now - 5 * 86_400, &mut rng, |i| {
            i.set_field("password", "tr0ub4dor-and-3-horses", true)
        })
        .unwrap();
        v.edit(Uuid::from_u128(1), now - 86_400, &mut rng, |i| {
            i.notes = "Recovery codes are in the safe. Spare key in the drawer.".into()
        })
        .unwrap();
        v.trash(Uuid::from_u128(5), now - 3 * 86_400, &mut rng)
            .unwrap();
    }
    let mut m = Model::new(session, SystemClipboard, None);
    match view.as_str() {
        "list" => {}
        "revealed" => {
            m.select(Uuid::from_u128(1), now);
            m.reveal("password", now);
            m.reveal("notes", now);
        }
        "form" => {
            m.select(Uuid::from_u128(1), now);
            m.start_edit(now);
        }
        "settings" => m.show(View::Settings, now),
        "trash" => {
            m.show(View::Trash, now);
            m.select(Uuid::from_u128(5), now);
        }
        _ => m.select(Uuid::from_u128(1), now),
    }
    rahasyakosh_desktop::launch_with(Some(m));
}
