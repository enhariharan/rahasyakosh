//! Behaviour of `Vault`, checked against every storage backend.
use rahasyakosh_app::{Config, Error, Scope, Vault};
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::{Field, Item};
use rahasyakosh_store::{MemoryStore, Store, Uuid};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

const PW: &str = "correct horse battery staple";
const DAY: i64 = 86_400;
/// Cheapest parameters the library accepts when reading a header.
const KDF: KdfParams = KdfParams {
    m_kib: KdfParams::MIN_M_KIB,
    t: KdfParams::MIN_T,
    p: 1,
};

fn rng() -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(42)
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0000 + n)
}

fn login(n: u128, title: &str, user: &str, password: &str) -> Item {
    Item::login(title, user, password, 1_000, id(n))
}

fn new_vault<S: Store>(store: S) -> Vault<S> {
    Vault::create(store, PW, KDF, Config::default(), &mut rng()).unwrap()
}

/// Lock and unlock again, as a program restart would.
fn reopen<S: Store>(v: Vault<S>) -> Vault<S> {
    Vault::unlock(v.into_store(), PW, Config::default()).unwrap()
}

fn titles(items: Vec<&Item>) -> Vec<String> {
    items.iter().map(|i| i.title.clone()).collect()
}

fn password_of(v: &Item) -> String {
    v.field("password").unwrap().value.clone()
}

/// Every behaviour, run against one backend. `make` gives a new empty store.
fn behaves_like_a_vault<S: Store>(make: impl Fn() -> S) {
    unlock_and_persistence(make());
    adding(make());
    editing_and_history(make());
    trash_and_restore(make());
    purging(make());
    expiry(make());
    reverting(make());
    searching(make());
    resolving(make());
    changing_the_password(make());
}

fn unlock_and_persistence<S: Store>(store: S) {
    let mut v = new_vault(store);
    v.add(login(1, "GitHub", "alice", "pw1"), &mut rng())
        .unwrap();
    let store = v.into_store();
    assert!(matches!(
        Vault::unlock(store, "wrong password", Config::default()),
        Err(Error::Core(rahasyakosh_core::Error::Decrypt))
    ));
}

fn adding<S: Store>(store: S) {
    let mut v = new_vault(store);
    let a = v
        .add(login(1, "GitHub", "alice", "pw1"), &mut rng())
        .unwrap();
    assert_eq!(v.revision(a), Some(1));
    assert_eq!(password_of(v.get(a).unwrap()), "pw1");
    assert!(matches!(
        v.add(login(1, "Again", "x", "y"), &mut rng()),
        Err(Error::State(_))
    ));
    // A batch is all-or-nothing.
    assert!(v
        .add_many(
            vec![login(2, "B", "", "p"), login(1, "Dup", "", "p")],
            &mut rng()
        )
        .is_err());
    assert!(v.get(id(2)).is_none());
    v.add_many(
        vec![login(2, "B", "", "p"), login(3, "C", "", "p")],
        &mut rng(),
    )
    .unwrap();
    let v = reopen(v);
    assert_eq!(v.len(), 3);
    assert_eq!(password_of(v.get(a).unwrap()), "pw1");
}

fn editing_and_history<S: Store>(store: S) {
    let mut v = new_vault(store);
    let a = v
        .add(login(1, "GitHub", "alice", "pw1"), &mut rng())
        .unwrap();

    let changed = v
        .edit(a, 2_000, &mut rng(), |i| {
            i.set_field("password", "pw2", true)
        })
        .unwrap();
    assert!(changed);
    assert_eq!(v.revision(a), Some(2));
    let item = v.get(a).unwrap();
    assert_eq!(password_of(item), "pw2");
    assert_eq!(item.updated_at, 2_000);
    assert_eq!(item.history.len(), 1);
    assert_eq!(
        item.history[0].fields[1].value, "pw1",
        "the old password is in history"
    );
    assert_eq!(item.history[0].replaced_at, 2_000);

    // An edit that changes nothing writes nothing and adds no history.
    assert!(!v.edit(a, 3_000, &mut rng(), |_| {}).unwrap());
    assert!(!v
        .edit(a, 3_000, &mut rng(), |i| i
            .set_field("password", "pw2", true))
        .unwrap());
    assert_eq!(v.revision(a), Some(2));
    assert_eq!(v.get(a).unwrap().history.len(), 1);

    // The id cannot be changed, and a failed edit changes nothing.
    assert!(v.edit(a, 4_000, &mut rng(), |i| i.id = id(99)).is_err());
    assert_eq!(v.revision(a), Some(2));

    // History is capped, newest first.
    for n in 3..=14 {
        v.edit(a, 5_000 + n, &mut rng(), |i| {
            i.set_field("password", &format!("pw{n}"), true)
        })
        .unwrap();
    }
    let h = v.history(a).unwrap();
    assert_eq!(h.len(), 10, "default limit");
    assert_eq!(h[0].fields[1].value, "pw13");
    assert_eq!(h[9].fields[1].value, "pw4");

    let v = reopen(v);
    assert_eq!(
        v.history(a).unwrap().len(),
        10,
        "history survives a restart"
    );
    assert_eq!(password_of(v.get(a).unwrap()), "pw14");
    assert!(matches!(v.history(id(77)), Err(Error::NotFound(_))));
}

fn trash_and_restore<S: Store>(store: S) {
    let mut v = new_vault(store);
    let a = v
        .add(login(1, "GitHub", "alice", "pw"), &mut rng())
        .unwrap();
    v.add(login(2, "Bank", "me", "pw"), &mut rng()).unwrap();
    v.edit(a, 2_000, &mut rng(), |i| i.notes = "n".into())
        .unwrap();

    v.trash(a, 3_000, &mut rng()).unwrap();
    assert_eq!(titles(v.items(Scope::Live)), ["Bank"]);
    assert_eq!(titles(v.items(Scope::Trashed)), ["GitHub"]);
    assert_eq!(
        titles(v.items(Scope::Any)),
        ["Bank", "GitHub"],
        "live first"
    );
    assert!(
        titles(v.search("github")).is_empty(),
        "trashed items are not searched by default"
    );
    assert_eq!(titles(v.search_in("github", Scope::Trashed)), ["GitHub"]);
    assert!(matches!(
        v.trash(a, 3_100, &mut rng()),
        Err(Error::State(_))
    ));
    assert_eq!(
        v.get(a).unwrap().history.len(),
        1,
        "trashing does not touch history"
    );

    let v2 = reopen(v);
    assert_eq!(
        titles(v2.items(Scope::Trashed)),
        ["GitHub"],
        "trash survives a restart"
    );
    let mut v = v2;
    v.restore(a, 4_000, &mut rng()).unwrap();
    assert_eq!(titles(v.items(Scope::Live)), ["Bank", "GitHub"]);
    assert!(v.get(a).unwrap().deleted_at.is_none());
    assert!(matches!(
        v.restore(a, 4_100, &mut rng()),
        Err(Error::State(_))
    ));
    assert_eq!(
        v.revision(a),
        Some(4),
        "edit, trash, restore each bump the revision"
    );
}

fn purging<S: Store>(store: S) {
    let mut v = new_vault(store);
    let a = v
        .add(login(1, "GitHub", "alice", "pw"), &mut rng())
        .unwrap();
    v.add(login(2, "Bank", "me", "pw"), &mut rng()).unwrap();
    v.purge(a, &mut rng()).unwrap();
    assert!(v.get(a).is_none());
    assert!(v.search("github").is_empty());
    assert!(matches!(v.purge(a, &mut rng()), Err(Error::NotFound(_))));
    let tombs = v.store().tombstones().unwrap();
    assert_eq!(tombs.len(), 1);
    assert_eq!(tombs[0].id, a);
    let v = reopen(v);
    assert_eq!(titles(v.items(Scope::Any)), ["Bank"]);
}

fn expiry<S: Store>(store: S) {
    let mut v = new_vault(store);
    let t0 = 1_000_000;
    let old = v.add(login(1, "Old", "", "p"), &mut rng()).unwrap();
    let newer = v.add(login(2, "Newer", "", "p"), &mut rng()).unwrap();
    v.add(login(3, "Live", "", "p"), &mut rng()).unwrap();
    v.trash(old, t0, &mut rng()).unwrap();
    v.trash(newer, t0 + 10 * DAY, &mut rng()).unwrap();

    assert_eq!(
        v.purge_expired(t0 + 29 * DAY, &mut rng()).unwrap(),
        0,
        "29 days: nothing yet"
    );
    assert_eq!(
        v.purge_expired(t0 + 30 * DAY, &mut rng()).unwrap(),
        1,
        "30 days: the old one goes"
    );
    assert!(v.get(old).is_none() && v.get(newer).is_some());
    assert_eq!(v.purge_expired(t0 + 39 * DAY, &mut rng()).unwrap(), 0);
    assert_eq!(v.purge_expired(t0 + 40 * DAY, &mut rng()).unwrap(), 1);
    assert_eq!(
        titles(v.items(Scope::Any)),
        ["Live"],
        "live items are never expired"
    );
}

fn reverting<S: Store>(store: S) {
    let mut v = new_vault(store);
    let a = v
        .add(login(1, "Site", "alice", "first"), &mut rng())
        .unwrap();
    v.edit(a, 2_000, &mut rng(), |i| {
        i.set_field("password", "second", true)
    })
    .unwrap();
    v.edit(a, 3_000, &mut rng(), |i| {
        i.set_field("password", "third", true)
    })
    .unwrap();
    // history: [second, first]
    v.revert(a, 1, 4_000, &mut rng()).unwrap();
    let item = v.get(a).unwrap();
    assert_eq!(password_of(item), "first");
    assert_eq!(
        item.history.len(),
        3,
        "nothing is lost: 'third' was saved before reverting"
    );
    assert_eq!(item.history[0].fields[1].value, "third");
    assert_eq!(item.updated_at, 4_000);
    assert!(v.revert(a, 9, 5_000, &mut rng()).is_err());
    let v = reopen(v);
    assert_eq!(password_of(v.get(a).unwrap()), "first");
}

fn searching<S: Store>(store: S) {
    let mut v = new_vault(store);
    let mut gh = login(1, "GitHub", "alice@example.com", "TOPSECRET");
    gh.urls.push("https://github.com/login".into());
    gh.tags = vec!["work".into(), "dev".into()];
    gh.notes = "recovery phrase lives here".into();
    let mut fav = login(2, "Zeta mail", "bob", "pw");
    fav.favourite = true;
    fav.tags = vec!["personal".into()];
    let mut totp = login(3, "Alpha", "carol", "pw");
    totp.fields.push(Field::secret("totp", "JBSWY3DPEHPK3PXP"));
    v.add_many(vec![gh, fav, totp], &mut rng()).unwrap();

    assert_eq!(titles(v.search("github")), ["GitHub"]);
    assert_eq!(titles(v.search("GITHUB")), ["GitHub"], "case-insensitive");
    assert_eq!(titles(v.search("alice")), ["GitHub"], "username");
    assert_eq!(titles(v.search("github.com")), ["GitHub"], "URL");
    assert_eq!(titles(v.search("dev")), ["GitHub"], "tag");
    assert_eq!(
        titles(v.search("work dev")),
        ["GitHub"],
        "every term must match"
    );
    assert!(
        v.search("work personal").is_empty(),
        "terms are ANDed across fields of ONE item"
    );
    assert!(
        v.search("TOPSECRET").is_empty(),
        "concealed fields are never searched"
    );
    assert!(v.search("JBSWY3DPEHPK3PXP").is_empty());
    assert!(v.search("recovery").is_empty(), "notes are not searched");
    // An empty query lists everything; favourites first, then by title.
    assert_eq!(titles(v.search("")), ["Zeta mail", "Alpha", "GitHub"]);
    assert_eq!(titles(v.search("   ")), ["Zeta mail", "Alpha", "GitHub"]);

    // The index follows edits.
    v.edit(id(1), 2_000, &mut rng(), |i| i.title = "GitLab".into())
        .unwrap();
    assert!(v.search("github.com").len() == 1 && v.search("github")[0].title == "GitLab");
    assert_eq!(titles(v.search("gitlab")), ["GitLab"]);
    let v = reopen(v);
    assert_eq!(
        titles(v.search("gitlab")),
        ["GitLab"],
        "the index is rebuilt on unlock"
    );
    assert_eq!(
        v.tags(),
        [
            ("dev".into(), 1),
            ("personal".into(), 1),
            ("work".into(), 1)
        ]
    );
}

fn resolving<S: Store>(store: S) {
    let mut v = new_vault(store);
    v.add_many(
        vec![
            login(1, "Mail", "a", "p"),
            login(2, "Mail work", "b", "p"),
            login(3, "Bank", "c", "p"),
            login(4, "Bank savings", "d", "p"),
        ],
        &mut rng(),
    )
    .unwrap();
    // An exact title beats a longer title that merely contains it.
    assert_eq!(v.resolve("mail", Scope::Live).unwrap().id, id(1));
    assert_eq!(v.resolve("savings", Scope::Live).unwrap().id, id(4));
    match v.resolve("ban", Scope::Live) {
        Err(Error::Ambiguous { titles, .. }) => assert_eq!(titles, ["Bank", "Bank savings"]),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        v.resolve("zzz", Scope::Live),
        Err(Error::NoMatch(_))
    ));

    v.trash(id(3), 2_000, &mut rng()).unwrap();
    assert!(
        v.resolve("bank", Scope::Live).is_ok(),
        "the live 'Bank savings' is the only live match"
    );
    assert_eq!(v.resolve("bank", Scope::Trashed).unwrap().id, id(3));
    assert_eq!(
        v.resolve("bank", Scope::Any).unwrap().id,
        id(3),
        "exact title wins in any scope"
    );
    assert!(matches!(
        v.resolve("savings", Scope::Trashed),
        Err(Error::NoMatch(_))
    ));
}

fn changing_the_password<S: Store>(store: S) {
    let mut v = new_vault(store);
    v.add(login(1, "GitHub", "alice", "pw"), &mut rng())
        .unwrap();
    v.change_password("a brand new password", KDF, &mut rng())
        .unwrap();
    let v = Vault::unlock(v.into_store(), "a brand new password", Config::default()).unwrap();
    assert_eq!(v.len(), 1, "items are untouched by the change");
    assert_eq!(password_of(v.get(id(1)).unwrap()), "pw");
    assert!(
        Vault::unlock(v.into_store(), PW, Config::default()).is_err(),
        "the old password no longer works"
    );
}

// ---- the backends --------------------------------------------------------------------------------

#[test]
fn memory_store() {
    behaves_like_a_vault(MemoryStore::new);
}

#[cfg(not(target_family = "wasm"))]
mod native {
    use super::*;
    use rahasyakosh_store::{DirectoryStore, SqliteStore};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp(name: &str) -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "app-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn directory_store() {
        behaves_like_a_vault(|| DirectoryStore::at(temp("dir").join("vault")));
    }

    #[test]
    fn sqlite_store() {
        behaves_like_a_vault(|| SqliteStore::open(temp("sqlite").join("vault.db")).unwrap());
    }

    #[test]
    fn a_second_session_cannot_overwrite_a_newer_change() {
        let path = temp("race").join("vault.db");
        let mut a = new_vault(SqliteStore::open(&path).unwrap());
        let id1 = a.add(login(1, "Site", "u", "v0"), &mut rng()).unwrap();
        let mut b =
            Vault::unlock(SqliteStore::open(&path).unwrap(), PW, Config::default()).unwrap();

        a.edit(id1, 2_000, &mut rng(), |i| {
            i.set_field("password", "from-a", true)
        })
        .unwrap();
        let lost = b.edit(id1, 2_001, &mut rng(), |i| {
            i.set_field("password", "from-b", true)
        });
        assert!(
            matches!(
                lost,
                Err(Error::Store(rahasyakosh_store::Error::StaleRevision { .. }))
            ),
            "{lost:?}"
        );
        // The loser's view is unchanged, and the winner's change is what is stored.
        assert_eq!(password_of(b.get(id1).unwrap()), "v0");
        let fresh =
            Vault::unlock(SqliteStore::open(&path).unwrap(), PW, Config::default()).unwrap();
        assert_eq!(password_of(fresh.get(id1).unwrap()), "from-a");
    }
}

// ---- random operations against a simple model -------------------------------------------------------

#[cfg(not(target_family = "wasm"))]
mod model {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeMap;

    #[derive(Debug, Clone)]
    enum Op {
        Add(String),
        Rename(usize, String),
        Trash(usize),
        Restore(usize),
        Purge(usize),
        Revert(usize),
    }

    fn ops() -> impl Strategy<Value = Vec<Op>> {
        let title = "[a-z]{1,6}";
        prop::collection::vec(
            prop_oneof![
                3 => title.prop_map(Op::Add),
                3 => (any::<usize>(), title).prop_map(|(i, t)| Op::Rename(i, t)),
                2 => any::<usize>().prop_map(Op::Trash),
                2 => any::<usize>().prop_map(Op::Restore),
                1 => any::<usize>().prop_map(Op::Purge),
                1 => any::<usize>().prop_map(Op::Revert),
            ],
            1..30,
        )
    }

    /// What a user can see: title and whether it is in the trash, per item.
    fn view<S: Store>(v: &Vault<S>) -> BTreeMap<Uuid, (String, bool)> {
        v.items(Scope::Any)
            .into_iter()
            .map(|i| (i.id, (i.title.clone(), i.deleted_at.is_some())))
            .collect()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]

        #[test]
        fn vault_matches_a_simple_model(ops in ops(), reopen_at in any::<prop::sample::Index>()) {
            let mut v = new_vault(MemoryStore::new());
            let mut model: BTreeMap<Uuid, (String, bool)> = BTreeMap::new();
            let mut next = 1u128;
            let reopen_at = reopen_at.index(ops.len());
            let mut r = rng();

            for (step, op) in ops.iter().enumerate() {
                let ids: Vec<Uuid> = model.keys().copied().collect();
                let pick = |n: usize| ids.get(n % ids.len().max(1)).copied();
                match op {
                    Op::Add(t) => {
                        v.add(login(next, t, "u", "p"), &mut r).unwrap();
                        model.insert(id(next), (t.clone(), false));
                        next += 1;
                    }
                    Op::Rename(n, t) => if let Some(x) = pick(*n) {
                        v.edit(x, 100 + step as i64, &mut r, |i| i.title = t.clone()).unwrap();
                        model.get_mut(&x).unwrap().0 = t.clone();
                    },
                    Op::Trash(n) => if let Some(x) = pick(*n) {
                        let r1 = v.trash(x, 100 + step as i64, &mut r);
                        prop_assert_eq!(r1.is_ok(), !model[&x].1);
                        if r1.is_ok() { model.get_mut(&x).unwrap().1 = true; }
                    },
                    Op::Restore(n) => if let Some(x) = pick(*n) {
                        let r1 = v.restore(x, 100 + step as i64, &mut r);
                        prop_assert_eq!(r1.is_ok(), model[&x].1);
                        if r1.is_ok() { model.get_mut(&x).unwrap().1 = false; }
                    },
                    Op::Purge(n) => if let Some(x) = pick(*n) {
                        v.purge(x, &mut rng()).unwrap();
                        model.remove(&x);
                    },
                    Op::Revert(n) => if let Some(x) = pick(*n) {
                        // Reverting to the newest history entry restores the previous title, if there is one.
                        let before = v.get(x).unwrap().history.first().map(|h| h.title.clone());
                        if let Some(prev) = before {
                            v.revert(x, 0, 100 + step as i64, &mut r).unwrap();
                            model.get_mut(&x).unwrap().0 = prev;
                        } else {
                            prop_assert!(v.revert(x, 0, 100, &mut r).is_err());
                        }
                    },
                }
                prop_assert_eq!(view(&v), model.clone(), "after step {} ({:?})", step, op);
                if step == reopen_at {
                    v = reopen(v);
                    prop_assert_eq!(view(&v), model.clone(), "after reopening at step {}", step);
                }
            }
            let v = reopen(v);
            prop_assert_eq!(view(&v), model);
        }
    }
}
