//! The session state machine: auto-lock, backoff, reveal and clipboard bookkeeping. Time is passed in, so none of it sleeps.
use rahasyakosh_app::{
    backoff_secs, should_clear, Locked, Scope, Session, Settings, ThemeMode, ToolbarStyle,
    UnlockError, Vault,
};
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::Item;
use rahasyakosh_store::{MemoryStore, Uuid};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

const PW: &str = "correct horse battery staple";

/// Cheapest key settings the library accepts, so tests stay fast.
fn fast() -> Settings {
    Settings {
        kdf_memory_mib: KdfParams::MIN_M_KIB.div_ceil(1024),
        kdf_passes: KdfParams::MIN_T,
        ..Settings::default()
    }
}

fn rng() -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(5)
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0000 + n)
}

/// A locked session over a vault that already holds one item.
fn locked_session(settings: Settings) -> Session<MemoryStore> {
    let mut s = Session::create(MemoryStore::new(), PW, settings, 0, &mut rng()).unwrap();
    s.vault(0)
        .unwrap()
        .add(Item::login("GitHub", "alice", "pw", 1, id(1)), &mut rng())
        .unwrap();
    s.lock();
    s
}

#[test]
fn a_new_session_is_locked_and_gives_no_access() {
    let mut s = Session::new(MemoryStore::new(), Settings::default());
    assert!(!s.is_unlocked());
    assert!(s.peek().is_none());
    assert_eq!(s.vault(0).err(), Some(Locked));
    assert_eq!(s.reveal(id(1), "password", 0), Err(Locked));
    assert_eq!(s.seconds_until_lock(0), None);
}

#[test]
fn unlock_lock_unlock_keeps_the_data() {
    let mut s = locked_session(fast());
    assert!(!s.is_unlocked());
    s.unlock(PW, 10, &mut rng()).unwrap();
    assert_eq!(s.peek().unwrap().items(Scope::Live).len(), 1);
    s.vault(11)
        .unwrap()
        .add(Item::login("Bank", "me", "pw2", 2, id(2)), &mut rng())
        .unwrap();
    s.lock();
    assert!(s.peek().is_none(), "locking drops the vault");
    s.unlock(PW, 20, &mut rng()).unwrap();
    assert_eq!(
        s.peek().unwrap().items(Scope::Live).len(),
        2,
        "changes made while unlocked were saved"
    );
    assert!(matches!(
        s.unlock(PW, 21, &mut rng()),
        Err(UnlockError::AlreadyUnlocked)
    ));
}

#[test]
fn a_wrong_password_returns_the_store_so_the_right_one_still_works() {
    let mut s = locked_session(fast());
    let err = s.unlock("not it", 0, &mut rng()).unwrap_err();
    assert!(
        matches!(
            err,
            UnlockError::WrongPassword {
                failures: 1,
                retry_in_secs: 0
            }
        ),
        "{err:?}"
    );
    assert_eq!(s.failed_attempts(), 1);
    assert!(!s.is_unlocked());
    s.unlock(PW, 1, &mut rng()).unwrap();
    assert_eq!(s.failed_attempts(), 0, "success resets the counter");
}

#[test]
fn the_backoff_schedule() {
    let waits: Vec<i64> = (0..=14).map(backoff_secs).collect();
    assert_eq!(
        waits,
        [0, 0, 0, 1, 2, 4, 8, 16, 32, 64, 128, 256, 300, 300, 300]
    );
    assert_eq!(backoff_secs(u32::MAX), 300, "no overflow, capped");
}

#[test]
fn repeated_wrong_passwords_must_wait_and_the_password_is_not_even_checked_meanwhile() {
    let mut s = locked_session(fast());
    for n in 1..=3 {
        assert!(
            matches!(s.unlock("bad", 100, &mut rng()), Err(UnlockError::WrongPassword { failures, .. }) if failures == n)
        );
    }
    // After the third failure the next attempt must wait 1 s.
    assert_eq!(s.retry_in(100), 1);
    // Even the right password is refused during the wait, so waiting cannot be skipped by guessing faster.
    assert!(matches!(
        s.unlock(PW, 100, &mut rng()),
        Err(UnlockError::LockedOut { retry_in_secs: 1 })
    ));
    assert!(!s.is_unlocked());
    // Another wrong guess once allowed doubles the wait.
    assert!(matches!(
        s.unlock("bad", 101, &mut rng()),
        Err(UnlockError::WrongPassword {
            failures: 4,
            retry_in_secs: 2
        })
    ));
    assert!(matches!(
        s.unlock(PW, 102, &mut rng()),
        Err(UnlockError::LockedOut { retry_in_secs: 1 })
    ));
    s.unlock(PW, 103, &mut rng()).unwrap();
    assert_eq!(s.retry_in(103), 0);
}

#[test]
fn only_a_wrong_password_counts_as_a_failed_attempt() {
    // A store with no vault in it gives a different error, which must not feed the lockout.
    let mut s = Session::new(MemoryStore::new(), Settings::default());
    for _ in 0..5 {
        assert!(matches!(
            s.unlock("x", 0, &mut rng()),
            Err(UnlockError::Failed(_))
        ));
    }
    assert_eq!(s.failed_attempts(), 0);
}

#[test]
fn the_vault_locks_itself_after_the_idle_time() {
    let mut s = locked_session(Settings {
        idle_lock_secs: 60,
        ..fast()
    });
    s.unlock(PW, 1000, &mut rng()).unwrap();
    assert_eq!(s.seconds_until_lock(1000), Some(60));
    assert!(!s.tick(1059).locked);
    assert_eq!(s.seconds_until_lock(1059), Some(1));
    let t = s.tick(1060);
    assert!(t.locked, "locks exactly when the idle time is reached");
    assert!(!s.is_unlocked());
    assert_eq!(s.seconds_until_lock(1060), None);
}

#[test]
fn activity_postpones_the_lock_but_peeking_does_not() {
    let mut s = locked_session(Settings {
        idle_lock_secs: 60,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    let _ = s.peek(); // drawing a screen is not activity
    assert_eq!(s.seconds_until_lock(50), Some(10));
    s.touch(50);
    assert_eq!(s.seconds_until_lock(50), Some(60));
    s.vault(100).unwrap(); // a user action
    assert_eq!(s.seconds_until_lock(100), Some(60));
    assert!(!s.tick(159).locked);
    assert!(s.tick(160).locked);
}

#[test]
fn using_the_vault_after_the_timeout_locks_it_instead_of_serving_it() {
    let mut s = locked_session(Settings {
        idle_lock_secs: 60,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    // Nobody called tick; the next action finds the timeout has passed.
    assert_eq!(s.vault(61).err(), Some(Locked));
    assert!(!s.is_unlocked());
    assert!(s.peek().is_none());
}

#[test]
fn an_idle_time_of_zero_never_locks() {
    let mut s = locked_session(Settings {
        idle_lock_secs: 0,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    assert_eq!(s.seconds_until_lock(10), None);
    assert!(!s.tick(10_000_000).locked);
    assert!(s.vault(10_000_001).is_ok());
}

#[test]
fn trash_older_than_the_retention_is_purged_on_unlock() {
    let mut s = locked_session(Settings {
        trash_retention_days: 30,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    s.vault(1).unwrap().trash(id(1), 1, &mut rng()).unwrap();
    s.lock();
    let day = 86_400;
    assert_eq!(s.unlock(PW, 29 * day, &mut rng()).unwrap().purged, 0);
    s.lock();
    assert_eq!(s.unlock(PW, 31 * day, &mut rng()).unwrap().purged, 1);
    assert!(s.peek().unwrap().items(Scope::Any).is_empty());
}

#[test]
fn a_retention_of_zero_days_cannot_empty_the_trash_at_unlock() {
    // From a settings file (hand-edited, imported or forged): 0 is raised to the minimum.
    let from_file = Settings::from_json(br#"{"trash_retention_days": 0}"#);
    assert_eq!(from_file.trash_retention_days, Settings::MIN_RETENTION_DAYS);
    let mut s = locked_session(Settings {
        trash_retention_days: 0,
        ..fast()
    });
    assert_eq!(s.settings().trash_retention_days, 1);
    s.unlock(PW, 0, &mut rng()).unwrap();
    s.vault(1).unwrap().trash(id(1), 1, &mut rng()).unwrap();
    s.lock();
    assert_eq!(s.unlock(PW, 100, &mut rng()).unwrap().purged, 0);
    assert!(s.peek().unwrap().get(id(1)).is_some(), "still in the trash");
}

#[test]
fn the_vault_itself_also_keeps_a_day_whatever_its_config_says() {
    let mut s = locked_session(fast());
    s.unlock(PW, 0, &mut rng()).unwrap();
    let v = s.vault(1).unwrap();
    v.trash(id(1), 1, &mut rng()).unwrap();
    v.set_config(rahasyakosh_app::Config {
        trash_retention_days: 0,
        ..rahasyakosh_app::Config::default()
    });
    assert_eq!(v.purge_expired(2, &mut rng()).unwrap(), 0);
    assert_eq!(v.purge_expired(1 + 86_400, &mut rng()).unwrap(), 1);
}

// ---- secrets on screen ---------------------------------------------------------------------------

#[test]
fn secrets_are_hidden_until_revealed_and_hide_again() {
    let mut s = locked_session(Settings {
        reveal_secs: 10,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    assert!(!s.is_revealed(id(1), "password", 0), "hidden by default");
    s.reveal(id(1), "password", 100).unwrap();
    assert!(s.is_revealed(id(1), "password", 100));
    assert!(s.is_revealed(id(1), "password", 109));
    assert!(
        !s.is_revealed(id(1), "password", 110),
        "hides after the reveal time"
    );
    assert!(!s.is_revealed(id(1), "totp", 100), "per field");
    assert!(!s.is_revealed(id(2), "password", 100), "per item");
    s.reveal(id(1), "password", 200).unwrap();
    s.hide(id(1), "password");
    assert!(!s.is_revealed(id(1), "password", 200));
}

#[test]
fn revealing_again_restarts_the_timer_and_zero_means_until_hidden() {
    let mut s = locked_session(Settings {
        reveal_secs: 10,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    s.reveal(id(1), "password", 0).unwrap();
    s.reveal(id(1), "password", 8).unwrap();
    assert!(s.is_revealed(id(1), "password", 17));
    assert!(!s.is_revealed(id(1), "password", 18));

    s.set_settings(Settings {
        reveal_secs: 0,
        // No inactivity lock here, or it would (rightly) forget the revealed secret during the long wait.
        idle_lock_secs: 0,
        ..fast()
    });
    s.reveal(id(1), "password", 0).unwrap();
    s.tick(1_000_000);
    assert!(s.is_revealed(id(1), "password", 1_000_000));
    s.hide_all();
    assert!(!s.is_revealed(id(1), "password", 0));
}

#[test]
fn locking_forgets_what_was_revealed() {
    let mut s = locked_session(Settings {
        reveal_secs: 0,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    s.reveal(id(1), "password", 0).unwrap();
    s.lock();
    s.unlock(PW, 1, &mut rng()).unwrap();
    assert!(
        !s.is_revealed(id(1), "password", 1),
        "a new unlock starts with everything hidden"
    );
}

// ---- the clipboard ----------------------------------------------------------------------------------

#[test]
fn a_copied_secret_is_due_for_clearing_once_after_the_delay() {
    let mut s = locked_session(Settings {
        clipboard_clear_secs: 20,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    assert_eq!(s.copied("hunter2", 100), Some(120));
    assert!(s.has_pending_clipboard());
    assert!(s.clipboard_due(119).is_none());
    let due = s.clipboard_due(120).expect("due at 120");
    assert_eq!(&*due, "hunter2");
    assert!(
        s.clipboard_due(121).is_none(),
        "each secret is handed back once"
    );
    assert!(!s.has_pending_clipboard());
}

#[test]
fn a_newer_copy_replaces_the_pending_one_and_zero_disables_clearing() {
    let mut s = locked_session(Settings {
        clipboard_clear_secs: 20,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    s.copied("first", 0);
    s.copied("second", 10);
    assert!(
        s.clipboard_due(25).is_none(),
        "the first copy's deadline no longer applies"
    );
    assert_eq!(&*s.clipboard_due(30).unwrap(), "second");

    s.set_settings(Settings {
        clipboard_clear_secs: 0,
        ..fast()
    });
    assert_eq!(s.copied("third", 40), None);
    assert!(!s.has_pending_clipboard());
}

#[test]
fn locking_hands_back_the_secret_still_on_the_clipboard() {
    let mut s = locked_session(Settings {
        clipboard_clear_secs: 20,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    s.copied("hunter2", 5);
    let pending = s.lock().expect("a secret was waiting to be cleared");
    assert_eq!(&*pending, "hunter2");
    assert!(!s.has_pending_clipboard());
    assert!(s.lock().is_none());
}

#[test]
fn an_idle_lock_also_returns_the_clipboard_secret() {
    let mut s = locked_session(Settings {
        idle_lock_secs: 30,
        clipboard_clear_secs: 600,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    s.copied("hunter2", 1);
    let tick = s.tick(30);
    assert!(tick.locked);
    assert_eq!(&**tick.clear_now.as_ref().unwrap(), "hunter2");
}

#[test]
fn only_our_own_secret_is_cleared_from_the_clipboard() {
    assert!(should_clear("hunter2", Some("hunter2")));
    assert!(!should_clear(
        "hunter2",
        Some("something else the user copied")
    ));
    assert!(
        !should_clear("hunter2", None),
        "an empty clipboard needs nothing"
    );
}

// ---- settings --------------------------------------------------------------------------------------------

#[test]
fn settings_are_clamped_and_never_weaken_the_key_derivation() {
    let wild = Settings {
        idle_lock_secs: u32::MAX,
        clipboard_clear_secs: u32::MAX,
        reveal_secs: u32::MAX,
        trash_retention_days: u32::MAX,
        history_limit: usize::MAX,
        kdf_memory_mib: 0,
        kdf_passes: 0,
        ..Settings::default()
    }
    .validated();
    assert_eq!(wild.idle_lock_secs, Settings::MAX_IDLE_SECS);
    assert_eq!(wild.history_limit, Settings::MAX_HISTORY);
    let kdf = wild.kdf_params();
    assert!(kdf.validate().is_ok(), "{kdf:?}");
    assert!(kdf.m_kib >= KdfParams::MIN_M_KIB && kdf.t >= KdfParams::MIN_T);
    let huge = Settings {
        kdf_memory_mib: u32::MAX,
        kdf_passes: u32::MAX,
        ..Settings::default()
    };
    assert!(huge.kdf_params().validate().is_ok());
    assert_eq!(Settings::default().kdf_params(), KdfParams::DESKTOP);
}

#[test]
fn settings_files_tolerate_missing_unknown_and_broken_content() {
    assert_eq!(Settings::from_json(b"{}"), Settings::default());
    assert_eq!(Settings::from_json(b"not json"), Settings::default());
    assert_eq!(Settings::from_json(b""), Settings::default());
    let partial = Settings::from_json(br#"{"idle_lock_secs": 90, "a_future_option": true}"#);
    assert_eq!(partial.idle_lock_secs, 90);
    assert_eq!(
        partial.clipboard_clear_secs, 20,
        "missing fields take their defaults"
    );
    let round = Settings {
        idle_lock_secs: 42,
        lock_on_blur: true,
        ..Settings::default()
    };
    assert_eq!(Settings::from_json(&round.to_json()), round);
}

#[test]
fn changing_settings_updates_the_open_vault() {
    let mut s = locked_session(Settings {
        history_limit: 10,
        ..fast()
    });
    s.unlock(PW, 0, &mut rng()).unwrap();
    s.set_settings(Settings {
        history_limit: 1,
        ..fast()
    });
    assert_eq!(s.peek().unwrap().config().history_limit, 1);
    let v: &mut Vault<MemoryStore> = s.vault(1).unwrap();
    for n in 1..=4 {
        v.edit(id(1), 10 + n, &mut rng(), |i| i.notes = format!("v{n}"))
            .unwrap();
    }
    assert_eq!(v.history(id(1)).unwrap().len(), 1);
}

// ---- random sequences against a model ---------------------------------------------------------------

#[cfg(not(target_family = "wasm"))]
mod model {
    use super::*;
    use proptest::prelude::*;

    #[derive(Debug, Clone)]
    enum Op {
        UnlockRight,
        UnlockWrong,
        Lock,
        Touch,
        Vault,
        Wait(u16),
    }

    fn ops() -> impl Strategy<Value = Vec<Op>> {
        prop::collection::vec(
            prop_oneof![
                3 => Just(Op::UnlockRight),
                2 => Just(Op::UnlockWrong),
                1 => Just(Op::Lock),
                2 => Just(Op::Touch),
                2 => Just(Op::Vault),
                4 => (1u16..120).prop_map(Op::Wait),
            ],
            1..40,
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(24))]

        /// Whatever happens, the session agrees with a trivial model of "unlocked?" and of the wrong-password wait.
        #[test]
        fn the_session_follows_a_simple_model(ops in ops()) {
            let idle = 60i64;
            let mut s = locked_session(Settings { idle_lock_secs: idle as u32, ..fast() });
            let mut now = 1_000i64;
            let (mut unlocked, mut last, mut failures, mut next_ok) = (false, 0i64, 0u32, 0i64);

            for op in ops {
                // The model applies the idle rule whenever time passes, like `tick`.
                if unlocked && now - last >= idle { unlocked = false; }
                match op {
                    Op::Wait(secs) => { now += i64::from(secs); s.tick(now); if unlocked && now - last >= idle { unlocked = false; } }
                    Op::Lock => { s.lock(); unlocked = false; }
                    Op::Touch => { s.touch(now); if unlocked { last = now; } }
                    Op::Vault => {
                        let got = s.vault(now).is_ok();
                        if unlocked && now - last >= idle { unlocked = false; }
                        prop_assert_eq!(got, unlocked);
                        if unlocked { last = now; }
                    }
                    Op::UnlockRight | Op::UnlockWrong => {
                        let right = matches!(op, Op::UnlockRight);
                        let result = s.unlock(if right { PW } else { "wrong" }, now, &mut rng());
                        if unlocked {
                            prop_assert!(matches!(result, Err(UnlockError::AlreadyUnlocked)));
                        } else if now < next_ok {
                            prop_assert!(matches!(result, Err(UnlockError::LockedOut { .. })), "expected a lockout wait");
                        } else if right {
                            prop_assert!(result.is_ok());
                            unlocked = true; last = now; failures = 0; next_ok = 0;
                        } else {
                            prop_assert!(matches!(result, Err(UnlockError::WrongPassword { .. })), "expected a wrong-password error");
                            failures += 1;
                            next_ok = now + backoff_secs(failures);
                        }
                    }
                }
                prop_assert_eq!(s.is_unlocked(), unlocked);
                prop_assert_eq!(s.peek().is_some(), unlocked);
                prop_assert_eq!(s.failed_attempts(), failures);
            }
        }
    }
}

#[test]
fn toolbar_style_defaults_for_old_files_and_round_trips() {
    // A settings file written before the option existed.
    let old = Settings::from_json(br#"{"idle_lock_secs": 60}"#);
    assert_eq!(old.toolbar_style, ToolbarStyle::IconAndText);
    assert_eq!(old.idle_lock_secs, 60);
    for style in [
        ToolbarStyle::IconAndText,
        ToolbarStyle::TextOnly,
        ToolbarStyle::IconOnly,
    ] {
        let s = Settings {
            toolbar_style: style,
            ..Settings::default()
        };
        assert_eq!(Settings::from_json(&s.to_json()).toolbar_style, style);
    }
}

#[test]
fn theme_defaults_to_system_for_old_files_and_round_trips() {
    let old = Settings::from_json(br#"{"idle_lock_secs": 60}"#);
    assert_eq!(old.theme, ThemeMode::System);
    for theme in [ThemeMode::Light, ThemeMode::Dark, ThemeMode::System] {
        let s = Settings {
            theme,
            ..Settings::default()
        };
        assert_eq!(Settings::from_json(&s.to_json()).theme, theme);
    }
}

#[test]
fn language_defaults_to_english_for_old_files_and_round_trips() {
    use rahasyakosh_app::Language;
    let old = Settings::from_json(br#"{"idle_lock_secs": 60}"#);
    assert_eq!(old.language, Language::English);
    for (i, language) in Language::ALL.into_iter().enumerate() {
        assert_eq!(language.index(), i);
        let s = Settings {
            language,
            ..Settings::default()
        };
        assert_eq!(Settings::from_json(&s.to_json()).language, language);
    }
    assert!(Language::Urdu.is_rtl());
    assert!(Language::ALL.iter().filter(|l| l.is_rtl()).count() == 1);
}
