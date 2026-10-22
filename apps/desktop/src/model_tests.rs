use crate::model::*;
use rahasyakosh_app::{Language, Scope, Session, Settings};
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::Item;
use rahasyakosh_store::{MemoryStore, Uuid};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use std::cell::RefCell;
use std::rc::Rc;

const PW: &str = "correct horse battery staple";

/// A clipboard held in memory that the test can look at and overwrite.
#[derive(Clone, Default)]
struct Fake(Rc<RefCell<Option<String>>>);

impl Clipboard for Fake {
    fn copy(&mut self, text: &str) -> Result<(), String> {
        *self.0.borrow_mut() = Some(text.to_string());
        Ok(())
    }
    fn clear_if_unchanged(&mut self, text: &str) -> Result<bool, String> {
        let mut c = self.0.borrow_mut();
        if c.as_deref() == Some(text) {
            *c = None;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

fn fast(s: Settings) -> Settings {
    Settings {
        kdf_memory_mib: KdfParams::MIN_M_KIB.div_ceil(1024),
        kdf_passes: KdfParams::MIN_T,
        ..s
    }
}

/// An unlocked model over a vault holding "GitHub" (alice, secret `gh-pw`, 2FA seed) and "Bank".
fn model(settings: Settings) -> (Model<MemoryStore, Fake>, Fake) {
    let clip = Fake::default();
    let mut rng = ChaCha20Rng::seed_from_u64(3);
    let mut session = Session::create(MemoryStore::new(), PW, fast(settings), 0, &mut rng).unwrap();
    let v = session.vault(0).unwrap();
    let mut gh = Item::login("GitHub", "alice", "gh-pw", 1, Uuid::from_u128(1));
    gh.set_field("totp", "JBSWY3DPEHPK3PXP", true);
    gh.notes = "recovery: abc-123".into();
    gh.urls.push("https://github.com".into());
    v.add(gh, &mut rng).unwrap();
    v.add(
        Item::login("Bank", "me", "bank-pw", 2, Uuid::from_u128(2)),
        &mut rng,
    )
    .unwrap();
    session.lock();
    let mut m = Model::new(session, clip.clone(), None);
    m.unlock(PW, 10);
    assert!(m.is_unlocked());
    (m, clip)
}

fn gh() -> Uuid {
    Uuid::from_u128(1)
}

#[test]
fn a_wrong_password_shows_a_message_and_the_wait() {
    let (mut m, _) = model(Settings::default());
    m.lock();
    m.unlock("nope", 100);
    assert!(!m.is_unlocked());
    assert_eq!(m.unlock_error.as_deref(), Some("Wrong password."));
    m.unlock("nope", 100);
    m.unlock("nope", 100);
    // The third consecutive failure starts the backoff, which the unlock screen shows as a countdown.
    assert_eq!(m.unlock_error.as_deref(), Some("Wrong password."));
    assert!(m.session.retry_in(100) > 0);
    m.unlock(PW, 100);
    assert!(m
        .unlock_error
        .as_deref()
        .unwrap()
        .contains("Too many wrong passwords"));
    m.unlock(PW, 102);
    assert!(m.is_unlocked() && m.unlock_error.is_none());
}

#[test]
fn the_list_searches_and_the_trash_view_lists_the_trash() {
    let (mut m, _) = model(Settings::default());
    assert_eq!(
        m.rows()
            .iter()
            .map(|r| r.title.as_str())
            .collect::<Vec<_>>(),
        ["Bank", "GitHub"]
    );
    m.query = "git".into();
    assert_eq!(m.rows().len(), 1);
    assert_eq!(m.rows()[0].subtitle, "alice");
    m.query = "alice".into();
    assert_eq!(m.rows().len(), 1, "usernames are searched");
    m.query = "gh-pw".into();
    assert!(m.rows().is_empty(), "passwords are not searched");
    m.query.clear();
    m.select(gh(), 20);
    m.trash_selected(21);
    assert_eq!(m.rows().len(), 1);
    assert_eq!(m.trash_count(), 1);
    m.show(View::Trash, 22);
    assert_eq!(m.rows()[0].title, "GitHub");
}

#[test]
fn secrets_are_masked_until_revealed_and_hide_again_on_time() {
    let (mut m, _) = model(Settings {
        reveal_secs: 10,
        idle_lock_secs: 0,
        ..Settings::default()
    });
    m.select(gh(), 20);
    let d = m.detail(20).unwrap();
    let pw = d.fields.iter().find(|f| f.name == "password").unwrap();
    assert!(pw.secret && pw.value.is_none(), "hidden by default");
    let user = d.fields.iter().find(|f| f.name == "username").unwrap();
    assert_eq!(user.value.as_deref(), Some("alice"));
    assert!(
        d.fields
            .iter()
            .find(|f| f.name == "notes")
            .unwrap()
            .value
            .is_none(),
        "notes are hidden too"
    );
    assert!(
        !d.fields.iter().any(|f| f.name == "totp"),
        "the 2FA seed is never listed as a field"
    );

    m.reveal("password", 30);
    let shown = |m: &Model<MemoryStore, Fake>, t| {
        m.detail(t)
            .unwrap()
            .fields
            .into_iter()
            .find(|f| f.name == "password")
            .unwrap()
            .value
    };
    assert_eq!(shown(&m, 30).as_deref(), Some("gh-pw"));
    assert_eq!(shown(&m, 39).as_deref(), Some("gh-pw"));
    assert_eq!(shown(&m, 40), None, "hidden again after the reveal time");
    m.reveal("password", 50);
    m.hide("password", 51);
    assert_eq!(shown(&m, 51), None);
}

#[test]
fn selecting_another_item_hides_what_was_revealed() {
    let (mut m, _) = model(Settings {
        idle_lock_secs: 0,
        ..Settings::default()
    });
    m.select(gh(), 20);
    m.reveal("password", 20);
    m.select(Uuid::from_u128(2), 21);
    m.select(gh(), 22);
    let pw = m
        .detail(22)
        .unwrap()
        .fields
        .into_iter()
        .find(|f| f.name == "password")
        .unwrap();
    assert!(pw.value.is_none());
}

#[test]
fn the_two_factor_code_is_live_and_its_seed_is_never_shown() {
    let (mut m, _) = model(Settings::default());
    m.select(gh(), 59);
    let t = m.detail(59).unwrap().totp.unwrap();
    assert_eq!(t.code.len(), 6);
    assert_eq!(
        t.seconds_left, 1,
        "at second 59 of a 30-second window... 59 % 30 = 29, so 1 left"
    );
    let next = m.detail(60).unwrap().totp.unwrap();
    assert_eq!(next.seconds_left, 30);
    assert!(m
        .detail(20)
        .unwrap()
        .fields
        .iter()
        .all(|f| f.value.as_deref() != Some("JBSWY3DPEHPK3PXP")));
}

#[test]
fn copying_clears_the_clipboard_on_time_but_not_if_it_was_replaced() {
    let (mut m, clip) = model(Settings {
        clipboard_clear_secs: 20,
        idle_lock_secs: 0,
        ..Settings::default()
    });
    m.select(gh(), 100);
    m.copy("password", 100);
    assert_eq!(clip.0.borrow().as_deref(), Some("gh-pw"));
    assert!(m.notice.as_ref().unwrap().text.contains("20 s"));
    m.tick(119);
    assert_eq!(clip.0.borrow().as_deref(), Some("gh-pw"), "not yet");
    m.tick(120);
    assert!(clip.0.borrow().is_none(), "cleared on time");

    m.copy("password", 200);
    *clip.0.borrow_mut() = Some("something the user copied".into());
    m.tick(230);
    assert_eq!(
        clip.0.borrow().as_deref(),
        Some("something the user copied"),
        "left alone"
    );
}

#[test]
fn copy_covers_notes_url_username_and_the_current_code() {
    let (mut m, clip) = model(Settings::default());
    m.select(gh(), 59);
    for (field, want) in [
        ("username", "alice"),
        ("notes", "recovery: abc-123"),
        ("url", "https://github.com"),
    ] {
        m.copy(field, 59);
        assert_eq!(clip.0.borrow().as_deref(), Some(want), "{field}");
    }
    m.copy("totp", 59);
    let code = clip.0.borrow().clone().unwrap();
    assert_eq!(code, m.detail(59).unwrap().totp.unwrap().code);
}

#[test]
fn locking_clears_a_pending_secret_wipes_the_screen_and_hides_everything() {
    let (mut m, clip) = model(Settings {
        clipboard_clear_secs: 600,
        ..Settings::default()
    });
    m.select(gh(), 100);
    m.reveal("password", 100);
    m.query = "git".into();
    m.copy("password", 101);
    m.lock();
    assert!(
        clip.0.borrow().is_none(),
        "the copied secret is cleared when the vault locks"
    );
    assert!(m.selected.is_none() && m.query.is_empty() && m.form.is_none());
    assert!(
        m.rows().is_empty() && m.detail(101).is_none(),
        "nothing is readable while locked"
    );
    m.unlock(PW, 200);
    m.select(gh(), 200);
    assert!(m
        .detail(200)
        .unwrap()
        .fields
        .iter()
        .find(|f| f.name == "password")
        .unwrap()
        .value
        .is_none());
}

#[test]
fn the_idle_timeout_locks_and_clears_the_clipboard() {
    let (mut m, clip) = model(Settings {
        idle_lock_secs: 60,
        clipboard_clear_secs: 600,
        ..Settings::default()
    });
    m.select(gh(), 100);
    m.copy("password", 100);
    m.tick(159);
    assert!(m.is_unlocked());
    m.activity(150);
    m.tick(209);
    assert!(m.is_unlocked(), "activity postponed the lock");
    m.tick(210);
    assert!(!m.is_unlocked());
    assert!(clip.0.borrow().is_none());
    assert!(m.selected.is_none());
}

#[test]
fn lock_on_blur_only_when_enabled() {
    let (mut m, _) = model(Settings {
        lock_on_blur: false,
        ..Settings::default()
    });
    m.window_blurred();
    assert!(m.is_unlocked());
    m.apply_settings(
        Settings {
            lock_on_blur: true,
            ..fast(Settings::default())
        },
        30,
    );
    m.window_blurred();
    assert!(!m.is_unlocked());
}

#[test]
fn forms_validate_and_create_and_edit_items() {
    let (mut m, _) = model(Settings {
        idle_lock_secs: 0,
        ..Settings::default()
    });
    m.start_new(20);
    assert_eq!(m.view, View::Form);
    m.save_form(21);
    assert!(
        m.notice.as_ref().unwrap().text.contains("title"),
        "a title is required"
    );
    {
        let f = m.form.as_mut().unwrap();
        f.title = "  Mail  ".into();
        f.username = "me@example.com".into();
        f.totp = "not base32!".into();
    }
    m.save_form(22);
    assert!(
        m.notice.as_ref().unwrap().text.contains("two-factor"),
        "{:?}",
        m.notice
    );
    assert_eq!(m.view, View::Form, "an invalid form stays open");
    {
        let f = m.form.as_mut().unwrap();
        f.totp = String::new();
        f.tags = "personal, mail ,Personal".into();
        f.url = "https://mail.example.com".into();
    }
    m.generate_password(24);
    let generated = m.form.as_ref().unwrap().password.clone();
    assert_eq!(generated.chars().count(), 24);
    m.save_form(23);
    assert_eq!(m.view, View::Items);
    let id = m.selected.expect("the new item is selected");
    let item = m.session.peek().unwrap().get(id).unwrap().clone();
    assert_eq!(item.title, "Mail", "trimmed");
    assert_eq!(
        item.tags,
        ["personal", "mail"],
        "tags are split, trimmed and de-duplicated ignoring case"
    );
    assert_eq!(item.field("password").unwrap().value, generated);
    assert!(
        item.field("password").unwrap().concealed && !item.field("username").unwrap().concealed
    );

    // Editing keeps the old password in history.
    m.start_edit(30);
    assert_eq!(
        m.form.as_ref().unwrap().password,
        generated,
        "the edit form shows the current values"
    );
    m.form.as_mut().unwrap().password = "new-secret".into();
    m.save_form(31);
    let d = m.detail(31).unwrap();
    assert_eq!(d.history.len(), 1);
    assert_eq!(d.history[0].changed, "password");
    assert_eq!(
        m.session.peek().unwrap().get(id).unwrap().history[0].fields[1].value,
        generated
    );
}

#[test]
fn an_edit_that_changes_nothing_adds_no_history() {
    let (mut m, _) = model(Settings {
        idle_lock_secs: 0,
        ..Settings::default()
    });
    m.select(gh(), 20);
    m.start_edit(21);
    m.save_form(22);
    assert!(m.detail(22).unwrap().history.is_empty());
}

#[test]
fn trash_restore_purge_favourite_and_revert() {
    let (mut m, _) = model(Settings {
        idle_lock_secs: 0,
        ..Settings::default()
    });
    m.select(gh(), 20);
    m.toggle_favourite(21);
    assert!(m.detail(21).unwrap().favourite);
    assert_eq!(m.rows()[0].title, "GitHub", "favourites come first");
    m.start_edit(22);
    m.form.as_mut().unwrap().notes = "changed".into();
    m.save_form(23);
    m.revert_selected(0, 24);
    assert_eq!(
        m.session.peek().unwrap().get(gh()).unwrap().notes,
        "recovery: abc-123"
    );

    m.trash_selected(30);
    assert!(m.selected.is_none());
    m.show(View::Trash, 31);
    m.select(gh(), 32);
    assert!(m.detail(32).unwrap().trashed);
    m.restore_selected(33);
    assert_eq!(m.trash_count(), 0);
    m.select(gh(), 34);
    m.trash_selected(35);
    m.select(gh(), 36);
    m.purge_selected(37);
    assert!(m.session.peek().unwrap().get(gh()).is_none());
    assert_eq!(m.session.peek().unwrap().items(Scope::Any).len(), 1);
}

#[test]
fn settings_are_applied_clamped_and_saved() {
    let saved: Rc<RefCell<Vec<Settings>>> = Rc::default();
    let sink = saved.clone();
    let clip = Fake::default();
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    let session = Session::create(
        MemoryStore::new(),
        PW,
        fast(Settings::default()),
        0,
        &mut rng,
    )
    .unwrap();
    let mut m = Model::new(
        session,
        clip,
        Some(Box::new(move |s: &Settings| {
            sink.borrow_mut().push(s.clone())
        })),
    );
    m.apply_settings(
        Settings {
            idle_lock_secs: u32::MAX,
            kdf_passes: 0,
            ..Settings::default()
        },
        5,
    );
    let s = m.session.settings();
    assert_eq!(s.idle_lock_secs, Settings::MAX_IDLE_SECS);
    assert!(s.kdf_params().validate().is_ok());
    assert_eq!(saved.borrow().len(), 1);
    assert_eq!(&saved.borrow()[0], m.session.settings());
}

#[test]
fn changing_the_master_password_checks_strength_and_match() {
    let (mut m, _) = model(Settings {
        idle_lock_secs: 0,
        ..Settings::default()
    });
    assert!(m
        .change_master_password("a", "b", 20)
        .unwrap_err()
        .contains("do not match"));
    assert!(m
        .change_master_password("password", "password", 20)
        .unwrap_err()
        .contains("too weak"));
    m.change_master_password(
        "a much longer and better passphrase",
        "a much longer and better passphrase",
        21,
    )
    .unwrap();
    m.lock();
    m.unlock(PW, 100);
    assert!(!m.is_unlocked(), "the old password no longer works");
    m.unlock("a much longer and better passphrase", 101);
    assert!(m.is_unlocked());
}

#[test]
fn notices_expire() {
    let (mut m, _) = model(Settings {
        idle_lock_secs: 0,
        ..Settings::default()
    });
    m.select(gh(), 20);
    m.copy("username", 20);
    assert!(m.notice.is_some());
    m.tick(23);
    assert!(m.notice.is_some());
    m.tick(24);
    assert!(m.notice.is_none());
}

#[test]
fn the_form_hides_secrets_in_debug_output() {
    let mut f = Form::default();
    f.password = "super-secret".into();
    f.notes = "also secret".into();
    let shown = format!("{f:?}");
    assert!(!shown.contains("secret"), "{shown}");
}

#[test]
fn age_text() {
    assert_eq!(days_ago(Language::English, 0, 100), "today");
    assert_eq!(days_ago(Language::English, 0, 86_400), "1 day ago");
    assert_eq!(days_ago(Language::English, 0, 5 * 86_400 + 5), "5 days ago");
    assert_eq!(
        days_ago(Language::English, 100, 0),
        "today",
        "a time in the future is not negative"
    );
}

#[test]
fn first_run_choices_are_validated() {
    // Generated passphrase: only once the user confirms they wrote it down.
    assert!(
        new_master_password(Language::English, false, "a-b-c", false, "", "")
            .unwrap_err()
            .contains("written")
    );
    assert_eq!(
        new_master_password(Language::English, false, "a-b-c", true, "", "").unwrap(),
        "a-b-c"
    );
    // Own password: must match and must not be weak.
    assert!(
        new_master_password(Language::English, true, "", false, "x", "y")
            .unwrap_err()
            .contains("do not match")
    );
    assert!(
        new_master_password(Language::English, true, "", false, "password", "password")
            .unwrap_err()
            .contains("too weak")
    );
    assert_eq!(
        new_master_password(
            Language::English,
            true,
            "",
            false,
            "an excellent long passphrase here",
            "an excellent long passphrase here"
        )
        .unwrap(),
        "an excellent long passphrase here"
    );
}

#[test]
fn messages_follow_the_language_setting() {
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    let session = Session::create(
        MemoryStore::new(),
        PW,
        fast(Settings {
            language: Language::Hindi,
            ..Settings::default()
        }),
        0,
        &mut rng,
    )
    .unwrap();
    let mut m = Model::new(session, Fake::default(), None);
    assert_eq!(m.lang(), Language::Hindi);
    m.lock();
    m.unlock("wrong password", 1);
    assert_eq!(m.unlock_error.as_deref(), Some("गलत पासवर्ड।"));
    assert_eq!(days_ago(Language::Urdu, 0, 3 * 86_400), "3 دن پہلے");
    assert!(
        new_master_password(Language::Tamil, false, "a-b", false, "", "")
            .unwrap_err()
            .contains("கடவுச்சொற்றொடரை")
    );
}
