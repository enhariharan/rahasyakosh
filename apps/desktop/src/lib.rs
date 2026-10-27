//! Rahasyakosh desktop app (library, so examples can launch the same screens). This file only draws screens and forwards clicks; the behaviour lives in `model.rs`
//! (tested without a window) and in the session of `rahasyakosh-app`.
pub mod i18n;
pub mod model;
#[cfg(test)]
mod model_tests;
mod platform;
mod profiles;
mod style;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use i18n::{t as tr, trf, Msg};
use model::{Model, NoticeKind, ReaderNotice, SystemClipboard, View};
use rahasyakosh_app::{
    new_profile_id, Language, Profile, ProfileError, ProfileList, Session, Settings, ThemeMode,
    ToolbarStyle, DEFAULT_FULL_NAME, DEFAULT_USERNAME,
};
use rahasyakosh_store::{SqliteStore, Uuid};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub type AppModel = Model<SqliteStore, SystemClipboard>;

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---- starting up -------------------------------------------------------------------------------------------

fn vault_path() -> PathBuf {
    if let Some(p) = std::env::var_os("RK_VAULT") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    home.join(".local/share/rahasyakosh/vault.db")
}

/// The folder holding the profile list and the vault files.
fn data_dir() -> PathBuf {
    vault_path()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

fn settings_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".config")
        });
    base.join("rahasyakosh/settings.json")
}

fn load_settings() -> Settings {
    std::fs::read(settings_path())
        .map(|b| Settings::from_json(&b))
        .unwrap_or_default()
}

fn save_settings(s: &Settings) {
    let path = settings_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, s.to_json());
}

pub fn make_model(session: Session<SqliteStore>) -> AppModel {
    Model::new(session, SystemClipboard, Some(Box::new(save_settings)))
}

enum Startup {
    /// A vault exists: show the unlock screen.
    Ready(Box<AppModel>),
    /// No vault yet: offer to create one.
    NeedsSetup,
    /// Something is wrong with the file; say what.
    Broken(String),
}

fn startup(path: &Path) -> Startup {
    use std::io::Read;
    let mut head = [0u8; 16];
    match std::fs::File::open(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Startup::NeedsSetup,
        Err(e) => return Startup::Broken(format!("Cannot open {}: {e}", path.display())),
        Ok(mut f) => {
            let n = f.read(&mut head).unwrap_or(0);
            if n == 0 {
                return Startup::NeedsSetup;
            }
            if &head[..n.min(15)] != b"SQLite format 3" {
                return Startup::Broken(
                    if head[..n].iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
                        format!(
                        "{} is in the old vault file format. Convert it first with:  rk migrate {} --vault <NEW FILE>",
                        path.display(),
                        path.display()
                    )
                    } else {
                        format!("{} is not a Rahasyakosh vault.", path.display())
                    },
                );
            }
        }
    }
    match SqliteStore::open(path) {
        Ok(store) => {
            let mut session = Session::new(store, load_settings());
            if let Some(p) = rahasyakosh_app::HighWater::default_path() {
                session = session.with_highwater(p);
            }
            Startup::Ready(Box::new(make_model(session)))
        }
        Err(e) => Startup::Broken(trf(load_settings().language, Msg::CannotOpenVault, &[&e])),
    }
}

/// Opens the vault of `profile` (locked), ready for its master password.
fn open_profile(profile: &Profile) -> Startup {
    let lang = load_settings().language;
    let Some(path) = profiles::vault_file(&data_dir(), profile) else {
        return Startup::Broken(trf(lang, Msg::ProfileFileMissing, &[&profile.file]));
    };
    if !path.exists() {
        return Startup::Broken(trf(lang, Msg::ProfileFileMissing, &[&path.display()]));
    }
    startup(&path)
}

// ---- shared state -----------------------------------------------------------------------------------------

#[derive(Clone)]
struct Ctx {
    model: Signal<Option<AppModel>>,
    /// Re-render trigger: set once a second.
    clock: Signal<i64>,
    /// Latest user input time, kept outside signals so moving the pointer does not re-render the window.
    last_input: Rc<Cell<i64>>,
    /// Set when the vault file is unusable.
    broken: Signal<Option<String>>,
    /// The language chosen on the first-run screens, before there is a vault with settings.
    setup_lang: Signal<Language>,
    /// Everyone who has a vault here, and which of them is open or being unlocked.
    profiles: Signal<ProfileList>,
    active: Signal<Option<Uuid>>,
    /// True while the "add a profile" page is showing.
    adding: Signal<bool>,
    /// Set while the "screen reader started or stopped" dialog is showing.
    reader_notice: Signal<Option<ReaderNotice>>,
}

thread_local! {
    /// A model to start with instead of reading the vault file (used by the development gallery).
    static INITIAL: std::cell::RefCell<Option<AppModel>> = const { std::cell::RefCell::new(None) };
}

/// Open the window. This is what `main` does.
pub fn launch() {
    launch_with(None);
}

/// Open the window, starting from `initial` if given (for the development gallery), else from the vault file.
pub fn launch_with(initial: Option<AppModel>) {
    INITIAL.with(|c| *c.borrow_mut() = initial);
    platform::init();
    let window = WindowBuilder::new()
        .with_title(tr(load_settings().language, Msg::AppName))
        .with_window_icon(window_icon())
        .with_inner_size(LogicalSize::new(1080.0, 720.0))
        .with_min_inner_size(LogicalSize::new(760.0, 520.0));
    dioxus::LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window).with_menu(None))
        .launch(app);
}

/// Seconds since 1970, for lining debug messages up with other logs.
fn debug_time() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// The keys the app acts on, for both ways they arrive (see `forward_page_keys`). Returns true if the key was used,
/// so the caller can stop the browser from also acting on it.
fn handle_key(
    key: &Key,
    mods: Modifiers,
    model: Signal<Option<AppModel>>,
    reader_notice: Signal<Option<ReaderNotice>>,
    mut adding: Signal<bool>,
) -> bool {
    let mut model = model;
    if reader_notice.peek().is_some() {
        // The dialog is modal: Esc is OK, Tab stays on its one button, and no shortcut reaches the page.
        match key {
            Key::Escape => acknowledge_reader_notice(model, reader_notice),
            Key::Tab => return true,
            _ => {}
        }
        return false;
    }
    let mut used = false;
    if *key == Key::Escape {
        // Esc backs out of the form, settings or trash page, as Cancel / Items would.
        if let Some(m) = model.write().as_mut() {
            match (m.is_unlocked(), m.view.clone()) {
                (true, View::Form) => m.cancel_form(now()),
                (true, View::Settings | View::Trash) => m.show(View::Items, now()),
                _ => {}
            }
        }
    }
    if mods.ctrl() && !mods.alt() {
        if let Key::Character(c) = key {
            // Ctrl+Z only means "undo moving to the trash" while that offer is showing.
            if c.eq_ignore_ascii_case("z") && !mods.shift() {
                if let Some(m) = model.write().as_mut() {
                    if m.notice.as_ref().is_some_and(|n| n.undo.is_some()) {
                        used = true;
                        m.undo_trash(now());
                    }
                }
            }
            let unlocked = model.read().as_ref().is_some_and(|m| m.is_unlocked());
            if let Some(shortcut) = Shortcut::from_key(c, mods.shift()).filter(|_| unlocked) {
                if let Some(m) = model.write().as_mut() {
                    if !shortcut.needs_item() || m.has_open_item() {
                        used = true;
                        shortcut.run(m, now());
                    }
                }
            }
        }
    }
    if mods.ctrl() && *key == Key::Character("k".into()) {
        used = true;
        spawn(async move {
            let _ = dioxus::document::eval("document.getElementById('search')?.focus()").await;
        });
    }
    // Ctrl+Shift+N: "Add a profile", only while the unlock screen is showing (a locked vault, not already adding).
    if mods.ctrl() && mods.shift() && !mods.alt() {
        if let Key::Character(c) = key {
            if c.eq_ignore_ascii_case("n") {
                let on_unlock = model.read().as_ref().is_some_and(|m| !m.is_unlocked());
                if on_unlock && !adding() {
                    used = true;
                    adding.set(true);
                }
            }
        }
    }
    used
}

/// Keys pressed while nothing in the window has focus (right after unlocking, or after the focused control was
/// removed, for example by clicking a list row) go to the page itself, which the window's own key handler cannot see.
/// This script catches those and the app handles them with `handle_key`, so shortcuts work whatever has focus.
/// The keys it stops the browser acting on (`preventDefault`) must match `Shortcut::from_key`.
const PAGE_KEYS_SCRIPT: &str = r#"
if (!window.__rkPageKeys) {
  window.__rkPageKeys = true;
  document.addEventListener('keydown', (e) => {
    if (e.target !== document.body && e.target !== document.documentElement) return;
    if (e.key !== 'Escape' && !e.ctrlKey) return;
    dioxus.send([e.key, e.ctrlKey, e.altKey, e.shiftKey]);
    const k = e.key.toLowerCase();
    if (e.ctrlKey && !e.altKey && (e.shiftKey ? 'ucwtsn' : '123klnpedz').includes(k) && k.length === 1) e.preventDefault();
  });
}
"#;

/// The key and modifiers described by a message from `PAGE_KEYS_SCRIPT`.
fn page_key(key: &str, ctrl: bool, alt: bool, shift: bool) -> Option<(Key, Modifiers)> {
    let mut mods = Modifiers::empty();
    mods.set(Modifiers::CONTROL, ctrl);
    mods.set(Modifiers::ALT, alt);
    mods.set(Modifiers::SHIFT, shift);
    Some((key.parse::<Key>().ok()?, mods))
}

/// Pass keys pressed with nothing focused to `handle_key`, for as long as the window lives.
async fn forward_page_keys(
    model: Signal<Option<AppModel>>,
    reader_notice: Signal<Option<ReaderNotice>>,
    adding: Signal<bool>,
    last_input: Rc<Cell<i64>>,
) {
    let mut events = dioxus::document::eval(PAGE_KEYS_SCRIPT);
    while let Ok((key, ctrl, alt, shift)) = events.recv::<(String, bool, bool, bool)>().await {
        last_input.set(now());
        if let Some((key, mods)) = page_key(&key, ctrl, alt, shift) {
            handle_key(&key, mods, model, reader_notice, adding);
        }
    }
}

/// OK on the "screen reader started or stopped" dialog: close it, and lock the vault if the dialog said it would.
/// Going back to the unlock screen is what makes a screen reader started late pick the window up.
fn acknowledge_reader_notice(
    mut model: Signal<Option<AppModel>>,
    mut notice: Signal<Option<ReaderNotice>>,
) {
    let Some(n) = *notice.peek() else { return };
    if let Some(m) = model.write().as_mut() {
        m.acknowledge_reader_notice(n);
    }
    notice.set(None);
    if !n.lock_first {
        // Already on the unlock screen: put the cursor back in the password box.
        spawn(async {
            let _ = dioxus::document::eval("document.getElementById('master')?.focus()").await;
        });
    }
}

/// The dialog shown when a screen reader starts or stops while the app is open.
#[component]
fn ReaderDialog() -> Element {
    let ctx = use_context::<Ctx>();
    let lang = lang_of(&ctx);
    let Some(n) = (ctx.reader_notice)() else {
        return rsx! {};
    };
    let (model, notice) = (ctx.model, ctx.reader_notice);
    rsx! {
        div { class: "modal-backdrop",
            div {
                class: "modal", role: "alertdialog", aria_modal: "true",
                aria_labelledby: "reader-title", aria_describedby: "reader-text",
                h2 { id: "reader-title", {tr(lang, Msg::ReaderNoticeTitle)} }
                p { id: "reader-text", {tr(lang, n.message())} }
                button {
                    id: "reader-ok", class: "primary", r#type: "button", autofocus: true,
                    // `autofocus` only works when the page loads; the dialog appears later.
                    onmounted: move |_| {
                        spawn(async {
                            let _ = dioxus::document::eval("document.getElementById('reader-ok')?.focus()").await;
                        });
                    },
                    onclick: move |_| acknowledge_reader_notice(model, notice),
                    {tr(lang, Msg::Ok)}
                }
            }
        }
    }
}

/// The taskbar / title-bar icon: the chest, pre-rendered to raw 64x64 RGBA (see `packaging/`).
fn window_icon() -> Option<dioxus::desktop::tao::window::Icon> {
    dioxus::desktop::tao::window::Icon::from_rgba(
        include_bytes!("../packaging/window-icon-64.rgba").to_vec(),
        64,
        64,
    )
    .ok()
}

/// The app icon (same drawing as `packaging/rahasyakosh-chest.svg`), shown above the title on the first-run and unlock screens.
#[component]
fn Logo() -> Element {
    let lang = lang_of(&use_context::<Ctx>());
    rsx! {
        svg { class: "logo", view_box: "0 0 128 128", width: "72", height: "72", role: "img", "aria-label": tr(lang, Msg::AppName),
            defs {
                linearGradient { id: "logo-bg", x1: "0", y1: "0", x2: "0", y2: "1",
                    stop { offset: "0", stop_color: "#2f3a8f" }
                    stop { offset: "1", stop_color: "#171a45" }
                }
                linearGradient { id: "logo-lid", x1: "0", y1: "0", x2: "0", y2: "1",
                    stop { offset: "0", stop_color: "#ffd36b" }
                    stop { offset: "1", stop_color: "#f2a900" }
                }
            }
            rect { width: "128", height: "128", rx: "26", fill: "url(#logo-bg)" }
            rect { x: "22", y: "62", width: "84", height: "44", rx: "7", fill: "#e08e0b" }
            path { d: "M22 66V56C22 40 40 28 64 28s42 12 42 28v10z", fill: "url(#logo-lid)" }
            rect { x: "36", y: "31", width: "7", height: "75", fill: "#a86400", opacity: ".55" }
            rect { x: "85", y: "31", width: "7", height: "75", fill: "#a86400", opacity: ".55" }
            rect { x: "22", y: "62", width: "84", height: "5", fill: "#a86400" }
            circle { cx: "64", cy: "70", r: "13", fill: "#171a45" }
            circle { cx: "64", cy: "67.5", r: "4.5", fill: "#ffd36b" }
            path { d: "M61.5 70h5l2 9h-9z", fill: "#ffd36b" }
        }
    }
}

/// The language to draw in: the open vault's setting, else the saved settings (the first-run and broken screens).
fn lang_of(ctx: &Ctx) -> Language {
    ctx.model
        .read()
        .as_ref()
        .map_or_else(|| (ctx.setup_lang)(), |m| m.lang())
}

fn app() -> Element {
    let mut model: Signal<Option<AppModel>> = use_signal(|| None);
    let mut broken: Signal<Option<String>> = use_signal(|| None);
    let clock = use_signal(now);
    let mut profiles: Signal<ProfileList> = use_signal(ProfileList::new);
    let mut active: Signal<Option<Uuid>> = use_signal(|| None);
    let adding = use_signal(|| false);
    let last_input = use_hook(|| Rc::new(Cell::new(now())));
    use_hook(|| {
        if let Some(m) = INITIAL.with(|c| c.borrow_mut().take()) {
            model.set(Some(m));
            return;
        }
        match profiles::load(&data_dir(), &vault_path()) {
            Err(msg) => broken.set(Some(msg)),
            Ok(list) => {
                if let Some(first) = list.all().first() {
                    match open_profile(first) {
                        Startup::Ready(m) => model.set(Some(*m)),
                        Startup::NeedsSetup => {}
                        Startup::Broken(msg) => broken.set(Some(msg)),
                    }
                    active.set(Some(first.id));
                }
                profiles.set(list);
            }
        };
    });
    let setup_lang = use_signal(|| load_settings().language);
    let reader_notice: Signal<Option<ReaderNotice>> = use_signal(|| None);
    let ctx = use_context_provider(|| Ctx {
        model,
        clock,
        last_input: last_input.clone(),
        broken,
        setup_lang,
        profiles,
        active,
        adding,
        reader_notice,
    });

    {
        let last_input = last_input.clone();
        use_future(move || forward_page_keys(model, reader_notice, adding, last_input.clone()));
    }

    // Once a second: pass on user input, apply the inactivity lock, clear the clipboard when due, and redraw.
    {
        let last_input = last_input.clone();
        let mut clock = clock;
        let mut reader_notice = reader_notice;
        use_future(move || {
            let last_input = last_input.clone();
            async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    let t = now();
                    if let Some(m) = model.write().as_mut() {
                        if last_input.get() > 0 {
                            m.activity(last_input.get().min(t));
                        }
                        m.tick(t);
                    }
                    clock.set(t);
                    if let Some(enabled) = platform::screen_reader_change() {
                        // Only once there is a vault screen to go back to; the first-run pages have nothing to lock.
                        let next = model
                            .read()
                            .as_ref()
                            .map(|m| m.screen_reader_changed(enabled, *reader_notice.peek()));
                        if next.is_some() {
                            reader_notice.set(next);
                        }
                    }
                }
            }
        });
    }

    // When the window gets focus back (Alt+Tab) while a screen reader runs, the web view may not tell the reader which
    // control has focus, and the reader then speaks every key, even in the password box. The platform code knows
    // what to do about it, if anything.
    let mut focus_gain = use_signal(|| 0u32);
    use_effect(move || {
        if focus_gain() == 0 || !platform::screen_reader_active() {
            return;
        }
        if let Some(script) = platform::script_after_window_focus() {
            spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                let _ = dioxus::document::eval(script).await;
            });
        }
    });

    // Lock when the window loses focus (if enabled), and wipe the clipboard secret when it is closed.
    dioxus::desktop::use_wry_event_handler(move |event, _| {
        use dioxus::desktop::tao::event::{Event, WindowEvent};
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::Focused(true) => {
                    if platform::debug() {
                        eprintln!("{:.3} window focus gained", debug_time());
                    }
                    focus_gain += 1;
                }
                WindowEvent::Focused(false) => {
                    if platform::debug() {
                        eprintln!("{:.3} window focus lost", debug_time());
                    }
                    if let Some(m) = model.write().as_mut() {
                        m.window_blurred();
                    }
                }
                WindowEvent::CloseRequested => {
                    if let Some(m) = model.write().as_mut() {
                        m.lock();
                    }
                }
                _ => {}
            }
        }
    });

    let (input, input_down, input_key) = (
        ctx.last_input.clone(),
        ctx.last_input.clone(),
        ctx.last_input.clone(),
    );
    let theme = model
        .read()
        .as_ref()
        .map_or(ThemeMode::System, |m| m.session.settings().theme);
    // On the first-run screen there is no vault yet, so the language lives in `setup_lang`.
    let lang = model
        .read()
        .as_ref()
        .map_or_else(|| (ctx.setup_lang)(), |m| m.lang());
    let dir = if lang.is_rtl() { "rtl" } else { "ltr" };
    use_effect(use_reactive!(|theme, lang| {
        let name = match theme {
            ThemeMode::Light => "light",
            ThemeMode::Dark => "dark",
            ThemeMode::System => "system",
        };
        dioxus::desktop::window().set_title(tr(lang, Msg::AppName));
        let (code, dir) = (lang.code(), if lang.is_rtl() { "rtl" } else { "ltr" });
        spawn(async move {
            let _ = dioxus::document::eval(&format!(
                "const d=document.documentElement;d.dataset.theme='{name}';d.lang='{code}';d.dir='{dir}'"
            ))
            .await;
        });
    }));
    let (polite, urgent) = model.read().as_ref().map_or_else(Default::default, |m| {
        let a = &m.announcement;
        if a.urgent {
            (String::new(), a.spoken())
        } else {
            (a.spoken(), String::new())
        }
    });
    rsx! {
        style { "{style::CSS}" }
        // Always on the page, so a screen reader is already listening when the text changes.
        div { class: "sr-only", role: "status", aria_live: "polite", aria_atomic: "true", "{polite}" }
        div { class: "sr-only", role: "alert", aria_live: "assertive", aria_atomic: "true", "{urgent}" }
        if reader_notice().is_some() { ReaderDialog {} }
        div {
            class: "app",
            dir: "{dir}",
            lang: "{lang.code()}",
            onpointermove: move |_| input.set(now()),
            onpointerdown: move |_| input_down.set(now()),
            onkeydown: move |e| {
                input_key.set(now());
                if handle_key(&e.key(), e.modifiers(), model, reader_notice, adding) {
                    e.prevent_default();
                }
            },
            { screen(ctx.clone()) }
        }
    }
}

fn screen(ctx: Ctx) -> Element {
    let _ = (ctx.clock)(); // redraw every second
    if let Some(msg) = (ctx.broken)() {
        return rsx! { main { class: "centre", h1 { {tr(lang_of(&ctx), Msg::AppName)} } p { class: "error", role: "alert", "{msg}" } } };
    }
    if (ctx.adding)() {
        return rsx! { Setup {} };
    }
    let state = {
        let m = ctx.model.read();
        m.as_ref().map(|m| m.is_unlocked())
    };
    match state {
        None => rsx! { Setup {} },
        Some(false) => rsx! { Unlock {} },
        Some(true) => rsx! { Main {} },
    }
}

// ---- first run ---------------------------------------------------------------------------------------------

#[component]
fn Setup() -> Element {
    let mut ctx = use_context::<Ctx>();
    let lang = lang_of(&ctx);
    let mut passphrase = use_signal(new_passphrase);
    // Adding a profile skips the welcome page; the first profile starts from the default name and username.
    let adding = (ctx.adding)();
    let mut started = use_signal(|| adding);
    let mut full_name = use_signal(|| {
        if adding {
            String::new()
        } else {
            DEFAULT_FULL_NAME.to_string()
        }
    });
    let mut username = use_signal(|| {
        if adding {
            String::new()
        } else {
            DEFAULT_USERNAME.to_string()
        }
    });
    let mut own = use_signal(|| false);
    let mut pw = use_signal(String::new);
    let mut pw2 = use_signal(String::new);
    let mut saved = use_signal(|| false);
    let mut copied = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let strength = rahasyakosh_strength::estimate(&pw.read(), &[]);

    let create = move |_| {
        let dir = data_dir();
        let mut list = ctx.profiles.read().clone();
        let id = new_profile_id(&mut rand::rng());
        // The first profile keeps the file name single-vault installations always used.
        let legacy = vault_path();
        let legacy_name = legacy
            .file_name()
            .and_then(|f| f.to_str())
            .map(str::to_string);
        let file = profiles::new_vault_file(
            list.is_empty().then_some(legacy_name).flatten().as_deref(),
            id,
        );
        let profile = match list.add(id, &full_name.read(), &username.read(), &file) {
            Ok(p) => p.clone(),
            Err(e) => {
                error.set(Some(tr(lang, profile_error_msg(e)).into()));
                return;
            }
        };
        let password = match model::new_master_password(
            lang,
            own(),
            &passphrase.read(),
            saved(),
            &pw.read(),
            &pw2.read(),
        ) {
            Ok(p) => p,
            Err(e) => {
                error.set(Some(e));
                return;
            }
        };
        let settings = Settings {
            language: lang,
            ..load_settings()
        };
        let Some(path) = profiles::vault_file(&dir, &profile) else {
            return;
        };
        profiles::prepare_folder(&path);
        match SqliteStore::open(&path)
            .map_err(|e| e.to_string())
            .and_then(|store| {
                Session::create(store, &password, settings, now(), &mut rand::rng())
                    .map_err(|e| e.to_string())
            }) {
            Ok(session) => {
                if let Err(e) = profiles::save(&dir, &list) {
                    error.set(Some(trf(lang, Msg::CouldNotCreateVault, &[&e])));
                    return;
                }
                save_settings(session.settings());
                ctx.profiles.set(list);
                ctx.active.set(Some(id));
                ctx.adding.set(false);
                ctx.model.set(Some(make_model(session)));
            }
            Err(e) => error.set(Some(trf(lang, Msg::CouldNotCreateVault, &[&e]))),
        }
    };

    if !started() {
        return rsx! {
            main { class: "centre landing",
                Logo {}
                h1 { {tr(lang, Msg::WelcomeTitle)} }
                p { class: "tagline spaced",
                    {tr(lang, Msg::TaglineStore)} br {}
                    {tr(lang, Msg::TaglineFile)} br {}
                    {tr(lang, Msg::TaglineProtected)}
                }
                LanguagePicker {}
                button { class: "primary", onclick: move |_| started.set(true), {tr(lang, Msg::CreatePassphraseButton)} }
            }
        };
    }

    // Copy the generated passphrase, and take it off the clipboard again after the usual delay if it is still there.
    let copy_passphrase = move |_| {
        let text = passphrase.read().clone();
        if model::Clipboard::copy(&mut SystemClipboard, &text).is_err() {
            error.set(Some(tr(lang, Msg::CouldNotCopy).into()));
            return;
        }
        error.set(None);
        copied.set(true);
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            copied.set(false);
        });
        let secs = load_settings().clipboard_clear_secs;
        if secs > 0 {
            spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(u64::from(secs))).await;
                let _ = model::Clipboard::clear_if_unchanged(&mut SystemClipboard, &text);
            });
        }
    };

    rsx! {
        main { class: "centre landing",
            h1 { {tr(lang, if adding { Msg::AddProfileTitle } else { Msg::CreatePassphraseTitle })} }
            if adding { p { class: "muted", {tr(lang, Msg::AddProfileHint)} } }
            section { aria_label: tr(lang, Msg::ProfileSection),
                label { r#for: "prf-name", {tr(lang, Msg::ProfileFullName)} }
                input { id: "prf-name", autocomplete: "off", value: "{full_name}", oninput: move |e| full_name.set(e.value()) }
                label { r#for: "prf-user", {tr(lang, Msg::ProfileUsername)} }
                input {
                    id: "prf-user", class: "ltr", dir: "ltr", autocomplete: "off", value: "{username}",
                    aria_describedby: if adding { "" } else { "prf-hint" },
                    oninput: move |e| username.set(e.value()),
                }
                if !adding { p { id: "prf-hint", class: "muted small", {tr(lang, Msg::ProfileKeepHint)} } }
            }
            p { class: "warn", {tr(lang, Msg::ShownOnce)} }
            if !own() {
                section { aria_label: tr(lang, Msg::GeneratedPassphrase),
                    p { class: "label", {tr(lang, Msg::YourNewPassphrase)} }
                    div { class: "passbox",
                        p { class: "passphrase", dir: "ltr", "{passphrase}" }
                        button {
                            class: "icon", r#type: "button",
                            aria_label: if copied() { tr(lang, Msg::PassphraseCopied) } else { tr(lang, Msg::CopyPassphrase) },
                            title: if copied() { tr(lang, Msg::Copied) } else { tr(lang, Msg::CopyToClipboard) },
                            onclick: copy_passphrase,
                            if copied() { CheckIcon {} } else { CopyIcon {} }
                        }
                    }
                    div { class: "row",
                        button { onclick: move |_| { passphrase.set(new_passphrase()); saved.set(false); }, {tr(lang, Msg::GenerateAnother)} }
                        button { class: "link", onclick: move |_| own.set(true), {tr(lang, Msg::ChooseOwn)} }
                    }
                    label { class: "check",
                        input { r#type: "checkbox", checked: saved(), onchange: move |e| saved.set(e.checked()) }
                        {tr(lang, Msg::WrittenDown)}
                    }
                }
            } else {
                section { aria_label: tr(lang, Msg::ChooseMasterPassword),
                    label { r#for: "pw", {tr(lang, Msg::MasterPassword)} }
                    input { id: "pw", class: "ltr", dir: "ltr", r#type: "password", value: "{pw}", oninput: move |e| pw.set(e.value()) }
                    label { r#for: "pw2", {tr(lang, Msg::RepeatIt)} }
                    input { id: "pw2", class: "ltr", dir: "ltr", r#type: "password", value: "{pw2}", oninput: move |e| pw2.set(e.value()) }
                    if !pw.read().is_empty() {
                        p { class: "meter", "{trf(lang, Msg::StrengthLine, &[&model::strength_label(lang, strength.score), &strength.crack_time])}" }
                        if let Some(w) = strength.warning.clone() { p { class: "hint", "{w}" } }
                    }
                    button { class: "link", onclick: move |_| own.set(false), {tr(lang, Msg::UseGenerated)} }
                }
            }
            if let Some(e) = error() { p { class: "error", role: "alert", "{e}" } }
            button { class: "primary", onclick: create, disabled: !own() && !saved(), {tr(lang, Msg::CreateVault)} }
            if adding { button { r#type: "button", onclick: move |_| ctx.adding.set(false), {tr(lang, Msg::CancelAdd)} } }
            p { class: "small", "{trf(lang, Msg::VaultLocation, &[&data_dir().display()])}" }
        }
    }
}

/// The message for a refused profile change.
fn profile_error_msg(e: ProfileError) -> Msg {
    match e {
        ProfileError::EmptyName => Msg::ProfileNameEmpty,
        ProfileError::EmptyUsername => Msg::ProfileUsernameEmpty,
        ProfileError::UsernameTaken => Msg::ProfileUsernameTaken,
        ProfileError::LastProfile => Msg::LastProfile,
        ProfileError::Unknown => Msg::ProfileFileMissing,
    }
}

/// A language chooser for the first-run screens, where there is no vault yet to keep a setting in.
#[component]
fn LanguagePicker() -> Element {
    let mut ctx = use_context::<Ctx>();
    let lang = lang_of(&ctx);
    rsx! {
        div { class: "lang-pick",
            label { class: "sr-only", r#for: "setup-lang", {tr(lang, Msg::SetLanguage)} }
            select { id: "setup-lang",
                onchange: move |e| {
                    if let Some(l) = Language::ALL.into_iter().find(|l| l.code() == e.value()) {
                        ctx.setup_lang.set(l);
                    }
                },
                for l in Language::ALL {
                    option { key: "{l.code()}", value: "{l.code()}", selected: l == lang, lang: "{l.code()}", "{l.native_name()}" }
                }
            }
        }
    }
}

#[component]
fn CopyIcon() -> Element {
    rsx! {
        svg { view_box: "0 0 24 24", width: "20", height: "20", fill: "none", stroke: "currentColor", stroke_width: "2",
            stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
            rect { x: "9", y: "9", width: "11", height: "11", rx: "2" }
            path { d: "M5 15V6a2 2 0 0 1 2-2h9" }
        }
    }
}

#[component]
fn CheckIcon() -> Element {
    rsx! {
        svg { view_box: "0 0 24 24", width: "20", height: "20", fill: "none", stroke: "currentColor", stroke_width: "2.5",
            stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
            path { d: "M5 12.5l4.5 4.5L19 7" }
        }
    }
}

/// A person with a "+": the "Add a profile" floating button.
#[component]
fn AddProfileIcon() -> Element {
    rsx! {
        svg { view_box: "0 0 24 24", width: "22", height: "22", fill: "none", stroke: "currentColor", stroke_width: "2",
            stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
            circle { cx: "9", cy: "7", r: "4" }
            path { d: "M2 21a7 7 0 0 1 14 0" }
            path { d: "M19 8v6M16 11h6" }
        }
    }
}

fn new_passphrase() -> String {
    let words = rahasyakosh_core::wordlist::eff_large();
    rahasyakosh_core::generator::generate_passphrase(&words, 6, "-", false, &mut rand::rng())
        .map(|p| p.to_string())
        .unwrap_or_default()
}

// ---- unlock ----------------------------------------------------------------------------------------------

#[component]
fn Unlock() -> Element {
    let mut ctx = use_context::<Ctx>();
    let lang = lang_of(&ctx);
    let mut pw = use_signal(String::new);
    let t = (ctx.clock)();
    let (err, wait) = {
        let m = ctx.model.read();
        let m = m.as_ref();
        (
            m.and_then(|m| m.unlock_error.clone()),
            m.map_or(0, |m| m.session.retry_in(t)),
        )
    };
    let has_err = err.is_some();
    let profile_list = ctx.profiles.read().all().to_vec();
    let active_id = (ctx.active)();
    let mut submit = move || {
        let t = now();
        if let Some(m) = ctx.model.write().as_mut() {
            m.unlock(&pw.read(), t);
        }
        pw.set(String::new());
        spawn(async move {
            let _ = dioxus::document::eval(
                // Leave and re-enter the box: a screen reader started after the app only notices the field
                // when focus really moves, and Enter alone leaves focus where it was.
                "const e=document.getElementById('master');if(e){e.value='';e.blur();setTimeout(()=>e.focus(),50)}",
            )
            .await;
        });
    };
    rsx! {
        main { class: "centre landing",
            Logo {}
            h1 { {tr(lang, Msg::AppName)} }
            p { class: "tagline spaced", {tr(lang, Msg::VaultLockedTag)} }
            if !profile_list.is_empty() {
                div { class: "profile-pick",
                    label { r#for: "profile", {tr(lang, Msg::ProfilePick)} }
                    select { id: "profile",
                        onchange: move |e| {
                            let chosen = profile_list.iter().find(|p| p.id.to_string() == e.value()).cloned();
                            let Some(p) = chosen else { return };
                            match open_profile(&p) {
                                Startup::Ready(m) => ctx.model.set(Some(*m)),
                                Startup::NeedsSetup => {}
                                Startup::Broken(msg) => ctx.broken.set(Some(msg)),
                            }
                            ctx.active.set(Some(p.id));
                        },
                        for p in profile_list.iter() {
                            option { key: "{p.id}", value: "{p.id}", selected: Some(p.id) == active_id, "{p.full_name}" }
                        }
                    }
                }
            }
            form {
                onsubmit: move |e| { e.prevent_default(); submit(); },
                label { class: "sr-only", r#for: "master", {tr(lang, Msg::MasterPassword)} }
                // Not bound to `value`: re-writing the typed text into the field on every key makes the screen
                // reader see new text and read it back. The box is emptied once, after a try, by `submit`.
                input {
                    id: "master", class: "ltr", dir: "ltr", r#type: "password", autofocus: true, autocomplete: "off",
                    aria_label: tr(lang, Msg::EnterMasterPassword),
                    aria_invalid: has_err, aria_describedby: if has_err { "unlock-error" } else { "" },
                    placeholder: tr(lang, Msg::EnterMasterPassword),
                    oninput: move |e| pw.set(e.value()),
                    // The focus given by `autofocus` can happen before the screen reader is listening; do it again.
                    onmounted: move |_| {
                        spawn(async move {
                            let _ = dioxus::document::eval(
                                "setTimeout(()=>{const e=document.getElementById('master');if(e){e.blur();e.focus()}},250)",
                            )
                            .await;
                        });
                    },
                }
                if let Some(e) = err { p { id: "unlock-error", class: "error", "{e}" } }
                if wait > 0 { p { class: "muted", "{trf(lang, Msg::TryAgainIn, &[&wait])}" } }
                button { class: "primary", r#type: "submit", disabled: wait > 0, {tr(lang, Msg::Unlock)} }
            }
        }
        button {
            class: "profile-fab", r#type: "button",
            aria_keyshortcuts: "Control+Shift+N",
            title: "{tr(lang, Msg::AddProfile)} (Ctrl+Shift+N)",
            aria_label: tr(lang, Msg::AddProfile),
            onclick: move |_| ctx.adding.set(true),
            AddProfileIcon {}
        }
    }
}

// ---- the main window ----------------------------------------------------------------------------------------

#[component]
fn Main() -> Element {
    let mut ctx = use_context::<Ctx>();
    let lang = lang_of(&ctx);
    let t = (ctx.clock)();
    let (view, query, left, notice, trash_count, style) = {
        let m = ctx.model.read();
        let m = m.as_ref().expect("unlocked");
        (
            m.view.clone(),
            m.query.clone(),
            m.session.seconds_until_lock(t),
            m.notice.clone(),
            m.trash_count(),
            m.session.settings().toolbar_style,
        )
    };
    let lock = move |_| {
        if let Some(m) = ctx.model.write().as_mut() {
            m.lock();
        }
    };
    let lock_text = match left {
        Some(s) if s < 60 => trf(lang, Msg::LocksInSecs, &[&s]),
        Some(s) => trf(lang, Msg::LocksInMins, &[&((s + 59) / 60)]),
        None => tr(lang, Msg::AutoLockOff).to_string(),
    };
    rsx! {
        header { class: "bar",
            h1 { {tr(lang, Msg::AppName)} }
            input {
                id: "search", r#type: "search", placeholder: tr(lang, Msg::SearchPlaceholder), aria_label: tr(lang, Msg::Search),
                value: "{query}",
                oninput: move |e| {
                    if let Some(m) = ctx.model.write().as_mut() {
                        m.query = e.value();
                        if m.view != View::Items { m.show(View::Items, now()); m.query = e.value(); }
                    }
                },
            }
            nav { aria_label: tr(lang, Msg::MainNav),
                for (sc, on, label) in [
                    (Shortcut::Items, view == View::Items || view == View::Form, tr(lang, Msg::Items).to_string()),
                    (Shortcut::Trash, view == View::Trash, trf(lang, Msg::TrashCount, &[&trash_count])),
                    (Shortcut::Settings, view == View::Settings, tr(lang, Msg::Settings).to_string()),
                ] {
                    button {
                        key: "{label}",
                        aria_current: if on { "page" } else { "false" },
                        aria_label: "{label}",
                        aria_keyshortcuts: sc.aria(),
                        title: "{label} ({sc.hint()})",
                        class: if on { "tab on" } else { "tab" },
                        onclick: move |_| { if let Some(m) = ctx.model.write().as_mut() { sc.run(m, now()); } },
                        NavContent { icon: sc.icon(), label: label.clone(), style }
                    }
                }
            }
            span { class: "muted small", "{lock_text}" }
            button {
                class: "danger", onclick: lock,
                aria_label: tr(lang, Msg::Lock), aria_keyshortcuts: Shortcut::Lock.aria(), title: "{tr(lang, Msg::Lock)} ({Shortcut::Lock.hint()})",
                NavContent { icon: Shortcut::Lock.icon(), label: tr(lang, Msg::Lock).to_string(), style }
            }
        }
        if let Some(n) = notice {
            // The text is spoken by the always-present regions at the top of the window, so it is hidden here.
            div {
                class: if n.kind == NoticeKind::Error { "toast error" } else { "toast" },
                span { aria_hidden: "true", "{n.text}" }
                if n.undo.is_some() {
                    button {
                        class: "undo", r#type: "button",
                        aria_keyshortcuts: "Control+Z",
                        onclick: move |_| { if let Some(m) = ctx.model.write().as_mut() { m.undo_trash(now()); } },
                        {tr(lang, Msg::Undo)}
                    }
                }
            }
        }
        match view {
            View::Settings => rsx! { SettingsView {} },
            View::Form => rsx! { FormView {} },
            _ => rsx! { Browser {} },
        }
    }
}

/// A top-bar action with its keyboard shortcut (Ctrl + the key).
#[derive(Clone, Copy, PartialEq)]
enum Shortcut {
    Items,
    Trash,
    Settings,
    Lock,
    NewItem,
    // The next ones act on the item open in the detail pane.
    Pin,
    Edit,
    MoveToTrash,
    CopyUsername,
    CopyPassword,
    CopyWebsite,
    CopyTotp,
    ShowPassword,
}

/// The "hide" version of the eye icon, for the Hide button.
const EYE_OFF_ICON: &str = "M3 3l18 18M10.6 6.1A9 9 0 0 1 12 5c6 0 10 7 10 7a17 17 0 0 1-3 3.7M6.6 6.6A17 17 0 0 0 2 12s4 7 10 7a9 9 0 0 0 4-1";

impl Shortcut {
    /// The shortcut for Ctrl + `c` (with Shift if `shift`).
    fn from_key(c: &str, shift: bool) -> Option<Self> {
        match (c, shift) {
            ("1", _) => Some(Self::Items),
            ("2", _) => Some(Self::Trash),
            ("3", _) => Some(Self::Settings),
            ("l" | "L", _) => Some(Self::Lock),
            ("n" | "N", _) => Some(Self::NewItem),
            ("p" | "P", false) => Some(Self::Pin),
            ("e" | "E", false) => Some(Self::Edit),
            ("d" | "D", false) => Some(Self::MoveToTrash),
            ("u" | "U", true) => Some(Self::CopyUsername),
            ("c" | "C", true) => Some(Self::CopyPassword),
            ("w" | "W", true) => Some(Self::CopyWebsite),
            ("t" | "T", true) => Some(Self::CopyTotp),
            ("s" | "S", true) => Some(Self::ShowPassword),
            _ => None,
        }
    }

    /// The shortcut that copies the field called `name`, if it has one.
    fn copy_for(name: &str) -> Option<Self> {
        match name {
            "username" => Some(Self::CopyUsername),
            "password" => Some(Self::CopyPassword),
            "url" => Some(Self::CopyWebsite),
            "totp" => Some(Self::CopyTotp),
            _ => None,
        }
    }

    /// Whether it needs an item to be open in the detail pane.
    fn needs_item(self) -> bool {
        !matches!(
            self,
            Self::Items | Self::Trash | Self::Settings | Self::Lock | Self::NewItem
        )
    }

    fn hint(self) -> &'static str {
        match self {
            Self::Items => "Ctrl+1",
            Self::Trash => "Ctrl+2",
            Self::Settings => "Ctrl+3",
            Self::Lock => "Ctrl+L",
            Self::NewItem => "Ctrl+N",
            Self::Pin => "Ctrl+P",
            Self::Edit => "Ctrl+E",
            Self::MoveToTrash => "Ctrl+D",
            Self::CopyUsername => "Ctrl+Shift+U",
            Self::CopyPassword => "Ctrl+Shift+C",
            Self::CopyWebsite => "Ctrl+Shift+W",
            Self::CopyTotp => "Ctrl+Shift+T",
            Self::ShowPassword => "Ctrl+Shift+S",
        }
    }

    fn aria(self) -> &'static str {
        match self {
            Self::Items => "Control+1",
            Self::Trash => "Control+2",
            Self::Settings => "Control+3",
            Self::Lock => "Control+L",
            Self::NewItem => "Control+N",
            Self::Pin => "Control+P",
            Self::Edit => "Control+E",
            Self::MoveToTrash => "Control+D",
            Self::CopyUsername => "Control+Shift+U",
            Self::CopyPassword => "Control+Shift+C",
            Self::CopyWebsite => "Control+Shift+W",
            Self::CopyTotp => "Control+Shift+T",
            Self::ShowPassword => "Control+Shift+S",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::Items => "M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01",
            Self::Trash => "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
            Self::Settings => "M4 7h10M18 7h2M4 17h2M10 17h10M16 4v6M8 14v6",
            Self::Lock => "M7 11V8a5 5 0 0 1 10 0v3M6 11h12v9H6z",
            Self::NewItem => "M12 5v14M5 12h14",
            Self::Pin => "M12 17v5M9 3h6l-1 7 3 3v2H7v-2l3-3z",
            Self::Edit => "M4 20h4L19 9l-4-4L4 16zM13 7l4 4",
            Self::MoveToTrash => "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3",
            Self::CopyUsername | Self::CopyPassword | Self::CopyWebsite | Self::CopyTotp => {
                "M9 9h11v11H9zM5 15V5h10"
            }
            Self::ShowPassword => {
                "M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z"
            }
        }
    }

    fn run(self, m: &mut AppModel, now: i64) {
        match self {
            Self::Items => m.show(View::Items, now),
            Self::Trash => m.show(View::Trash, now),
            Self::Settings => m.show(View::Settings, now),
            Self::Lock => m.lock(),
            Self::NewItem => m.start_new(now),
            Self::Pin => m.toggle_favourite(now),
            Self::Edit => m.start_edit(now),
            Self::MoveToTrash => m.trash_selected(now),
            Self::CopyUsername => m.copy("username", now),
            Self::CopyPassword => m.copy("password", now),
            Self::CopyWebsite => m.copy("url", now),
            Self::CopyTotp => m.copy("totp", now),
            Self::ShowPassword => m.toggle_reveal("password", now),
        }
    }
}

/// The inside of a top-bar button: an icon, the text, or both, as the setting says.
#[component]
fn NavContent(icon: &'static str, label: String, style: ToolbarStyle) -> Element {
    rsx! {
        if style != ToolbarStyle::TextOnly {
            svg { view_box: "0 0 24 24", width: "18", height: "18", fill: "none", stroke: "currentColor", stroke_width: "2",
                stroke_linecap: "round", stroke_linejoin: "round", "aria-hidden": "true",
                path { d: "{icon}" }
            }
        }
        if style != ToolbarStyle::IconOnly { span { "{label}" } }
    }
}

#[component]
fn Browser() -> Element {
    let ctx = use_context::<Ctx>();
    let mut model = ctx.model;
    let _ = (ctx.clock)();
    let lang = lang_of(&ctx);
    let (rows, selected, view, retention) = {
        let m = ctx.model.read();
        let m = m.as_ref().expect("unlocked");
        (
            m.rows(),
            m.selected,
            m.view.clone(),
            m.session.settings().trash_retention_days,
        )
    };
    let trash = view == View::Trash;
    rsx! {
        div { class: "split",
          div { class: "pane",
            aside { aria_label: if trash { tr(lang, Msg::DeletedItems) } else { tr(lang, Msg::Items) },
                if trash {
                    p { class: "muted small pad", "{trf(lang, Msg::TrashNote, &[&retention])}" }
                }
                if rows.is_empty() {
                    p { class: "muted pad", if trash { {tr(lang, Msg::TrashEmpty)} } else { {tr(lang, Msg::NothingHere)} } }
                }
                ul { class: "list", aria_label: tr(lang, Msg::ItemList),
                    for r in rows {
                        li { key: "{r.id}",
                            button {
                                class: if selected == Some(r.id) { "row on" } else { "row" },
                                aria_current: if selected == Some(r.id) { "true" } else { "false" },
                                onclick: move |_| select(model, r.id),
                                span { class: "title", if r.favourite { span { class: "sr-only", {tr(lang, Msg::Pinned)} } span { aria_hidden: "true", "★ " } } "{r.title}" }
                                span { class: "sub", "{r.subtitle}" }
                            }
                        }
                    }
                }
            }
            if !trash {
                button {
                    class: "primary fab",
                    aria_keyshortcuts: Shortcut::NewItem.aria(),
                    title: "{tr(lang, Msg::NewItem)} ({Shortcut::NewItem.hint()})",
                    onclick: move |_| { if let Some(m) = model.write().as_mut() { Shortcut::NewItem.run(m, now()); } },
                    svg { view_box: "0 0 24 24", width: "18", height: "18", fill: "none", stroke: "currentColor", stroke_width: "2.5",
                        stroke_linecap: "round", "aria-hidden": "true",
                        path { d: "{Shortcut::NewItem.icon()}" }
                    }
                    span { {tr(lang, Msg::NewItem)} }
                }
            }
          }
            main { class: "detail", aria_label: tr(lang, Msg::ItemDetails),
                { detail_pane(ctx.clone()) }
            }
        }
    }
}

fn select(mut model: Signal<Option<AppModel>>, id: Uuid) {
    if let Some(m) = model.write().as_mut() {
        m.select(id, now());
    }
}

fn detail_pane(ctx: Ctx) -> Element {
    let t = (ctx.clock)();
    let lang = lang_of(&ctx);
    let detail = ctx.model.read().as_ref().and_then(|m| m.detail(t));
    let Some(d) = detail else {
        return rsx! { p { class: "muted pad", {tr(lang, Msg::SelectItem)} } };
    };
    let id = d.id;
    // `act!(m, now => m.something(now))`: a click handler that runs `body` on the model with the current time.
    macro_rules! act {
        ($m:ident, $now:ident => $body:expr) => {{
            let mut ctx = ctx.clone();
            move |_| {
                if let Some($m) = ctx.model.write().as_mut() {
                    let $now = now();
                    $body;
                }
            }
        }};
    }
    // Hover text "Copy (Ctrl+Shift+C)" for a button that has a shortcut.
    let hint = |label: &str, sc: Option<Shortcut>| sc.map(|s| format!("{label} ({})", s.hint()));
    let copy_label = tr(lang, Msg::Copy);
    let item_btn = |sc: Shortcut, label: &'static str| {
        rsx! { NavContent { icon: sc.icon(), label: label.to_string(), style: ToolbarStyle::IconAndText } }
    };
    rsx! {
        div { class: "pad",
            div { class: "head",
                h2 { if d.favourite { "★ " } "{d.title}" }
                div { class: "row",
                    if d.trashed {
                        button { class: "primary", onclick: act!(m, now => m.restore_selected(now)), {tr(lang, Msg::Restore)} }
                        button { class: "danger", onclick: act!(m, now => m.purge_selected(now)), {tr(lang, Msg::DeleteForGood)} }
                    } else {
                        {
                            let label = if d.favourite { tr(lang, Msg::Unpin) } else { tr(lang, Msg::Pin) };
                            rsx! {
                                button {
                                    title: hint(label, Some(Shortcut::Pin)), aria_keyshortcuts: Shortcut::Pin.aria(),
                                    onclick: act!(m, now => m.toggle_favourite(now)),
                                    { item_btn(Shortcut::Pin, label) }
                                }
                            }
                        }
                        button {
                            title: hint(tr(lang, Msg::Edit), Some(Shortcut::Edit)), aria_keyshortcuts: Shortcut::Edit.aria(),
                            onclick: act!(m, now => m.start_edit(now)),
                            { item_btn(Shortcut::Edit, tr(lang, Msg::Edit)) }
                        }
                        button {
                            class: "danger",
                            title: hint(tr(lang, Msg::MoveToTrash), Some(Shortcut::MoveToTrash)), aria_keyshortcuts: Shortcut::MoveToTrash.aria(),
                            onclick: act!(m, now => m.trash_selected(now)),
                            { item_btn(Shortcut::MoveToTrash, tr(lang, Msg::MoveToTrash)) }
                        }
                    }
                }
            }
            dl {
                for f in d.fields.clone() {
                    {
                        let name = f.name.clone();
                        let (n1, n2, n3) = (name.clone(), name.clone(), name.clone());
                        let copy_sc = Shortcut::copy_for(&f.name);
                        let show_sc = (f.name == "password").then_some(Shortcut::ShowPassword);
                        rsx! {
                            div { class: "field", key: "{f.name}",
                                dt { "{f.label}" }
                                dd {
                                    if f.secret && f.value.is_none() { span { class: "mask", role: "img", aria_label: "{trf(lang, Msg::IsHidden, &[&f.label])}", "••••••••••" } }
                                    else if f.name == "notes" { span { class: "notes", "{f.value.clone().unwrap_or_default()}" } }
                                    else { span { class: if f.secret { "mono ltr" } else { "" }, dir: if f.secret { "ltr" } else { "auto" }, "{f.value.clone().unwrap_or_default()}" } }
                                    span { class: "row tight",
                                        if f.secret {
                                            if f.value.is_none() {
                                                button {
                                                    aria_label: "{trf(lang, Msg::ShowField, &[&f.label])}",
                                                    title: hint(tr(lang, Msg::Show), show_sc), aria_keyshortcuts: show_sc.map(Shortcut::aria),
                                                    onclick: act!(m, now => m.reveal(&n1, now)),
                                                    { item_btn(Shortcut::ShowPassword, tr(lang, Msg::Show)) }
                                                }
                                            } else {
                                                button {
                                                    aria_label: "{trf(lang, Msg::HideField, &[&f.label])}",
                                                    title: hint(tr(lang, Msg::Hide), show_sc), aria_keyshortcuts: show_sc.map(Shortcut::aria),
                                                    onclick: act!(m, now => m.hide(&n2, now)),
                                                    NavContent { icon: EYE_OFF_ICON, label: tr(lang, Msg::Hide).to_string(), style: ToolbarStyle::IconAndText }
                                                }
                                            }
                                        }
                                        button {
                                            aria_label: "{trf(lang, Msg::CopyField, &[&f.label])}",
                                            title: hint(copy_label, copy_sc), aria_keyshortcuts: copy_sc.map(Shortcut::aria),
                                            onclick: act!(m, now => m.copy(&n3, now)),
                                            { item_btn(Shortcut::CopyPassword, copy_label) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(t) = d.totp.clone() {
                    div { class: "field",
                        dt { {tr(lang, Msg::TwoFactorCode)} }
                        dd {
                            span { class: "mono big", dir: "ltr", "{t.code}" }
                            span { class: "muted small", " {t.seconds_left} s" }
                            span { class: "row tight",
                                button {
                                    aria_label: tr(lang, Msg::CopyTwoFactor),
                                    title: hint(copy_label, Some(Shortcut::CopyTotp)), aria_keyshortcuts: Shortcut::CopyTotp.aria(),
                                    onclick: act!(m, now => m.copy("totp", now)),
                                    { item_btn(Shortcut::CopyTotp, copy_label) }
                                }
                            }
                        }
                    }
                }
                for u in d.urls.clone() {
                    div { class: "field", key: "{u}",
                        dt { {tr(lang, Msg::Website)} }
                        dd { span { dir: "ltr", "{u}" } span { class: "row tight",
                            button {
                                aria_label: "{trf(lang, Msg::CopyWebsite, &[&u])}",
                                title: hint(copy_label, Some(Shortcut::CopyWebsite)), aria_keyshortcuts: Shortcut::CopyWebsite.aria(),
                                onclick: act!(m, now => m.copy("url", now)),
                                { item_btn(Shortcut::CopyWebsite, copy_label) }
                            }
                        } }
                    }
                }
                if !d.tags.is_empty() {
                    div { class: "field",
                        dt { {tr(lang, Msg::Tags)} }
                        dd { for tag in d.tags.clone() { span { class: "tag", key: "{tag}", "{tag}" } } }
                    }
                }
            }
            if !d.history.is_empty() {
                h3 { {tr(lang, Msg::EarlierVersions)} }
                ul { class: "history",
                    for h in d.history.clone() {
                        li { key: "{h.index}",
                            span { "{trf(lang, Msg::ChangedLine, &[&h.when, &h.changed])}" }
                            if !d.trashed {
                                button { aria_label: "{trf(lang, Msg::GoBackToVersion, &[&h.when])}", onclick: act!(m, now => m.revert_selected(h.index, now)), {tr(lang, Msg::GoBackToThis)} }
                            }
                        }
                    }
                }
            }
            { let _ = id; rsx! {} }
        }
    }
}

// ---- add / edit ----------------------------------------------------------------------------------------------

#[component]
fn FormView() -> Element {
    let mut ctx = use_context::<Ctx>();
    let _ = (ctx.clock)();
    let lang = lang_of(&ctx);
    let form = ctx.model.read().as_ref().and_then(|m| m.form.clone());
    let Some(f) = form else { return rsx! {} };
    let editing = f.id.is_some();
    let strength = rahasyakosh_strength::estimate(&f.password, &[&f.title, &f.username]);
    let form_error = ctx.model.read().as_ref().and_then(|m| m.form_error.clone());
    // `Some("true")` marks the box a form error is about; `describedby` ties the message to it.
    let invalid = |id: &str| form_error.as_ref().is_some_and(|e| e.field_id == id);
    let describedby = |id: &str| if invalid(id) { "f-error" } else { "" };
    let error_text = form_error.as_ref().map(|e| e.text.clone());
    macro_rules! field {
        ($name:ident) => {{
            let mut ctx = ctx.clone();
            move |e: FormEvent| {
                if let Some(m) = ctx.model.write().as_mut() {
                    if let Some(f) = &mut m.form {
                        f.$name = e.value();
                    }
                    m.activity(now());
                }
            }
        }};
    }
    rsx! {
        main { class: "form",
            h2 { if editing { {tr(lang, Msg::EditItem)} } else { {tr(lang, Msg::NewItem)} } }
            form {
                onsubmit: move |e| {
                    e.prevent_default();
                    // After a failed save, move focus to the box that is wrong so a screen reader reads it with its message.
                    let mut bad = None;
                    if let Some(m) = ctx.model.write().as_mut() {
                        m.save_form(now());
                        bad = m.form_error.as_ref().map(|e| e.field_id);
                    }
                    if let Some(id) = bad {
                        spawn(async move {
                            let _ = dioxus::document::eval(&format!("document.getElementById('{id}')?.focus()")).await;
                        });
                    }
                },
                label { r#for: "f-title", {tr(lang, Msg::FieldTitle)} }
                input {
                    id: "f-title", autofocus: true, value: "{f.title}", oninput: field!(title),
                    aria_invalid: invalid("f-title"), aria_describedby: describedby("f-title"), aria_required: "true",
                }
                if invalid("f-title") { if let Some(t) = &error_text { p { id: "f-error", class: "error", "{t}" } } }
                label { r#for: "f-user", {tr(lang, Msg::UsernameOrEmail)} }
                input { id: "f-user", dir: "auto", autocomplete: "off", value: "{f.username}", oninput: field!(username) }
                label { r#for: "f-pw", {tr(lang, Msg::FieldPassword)} }
                div { class: "row",
                    input { id: "f-pw", class: "grow mono ltr", dir: "ltr", autocomplete: "off", value: "{f.password}", oninput: field!(password) }
                    button {
                        r#type: "button",
                        onclick: move |_| { if let Some(m) = ctx.model.write().as_mut() { m.generate_password(20); } },
                        aria_label: tr(lang, Msg::GenerateRandom),
                        {tr(lang, Msg::Generate)}
                    }
                }
                if !f.password.is_empty() {
                    p { class: "meter", "{trf(lang, Msg::StrengthLine, &[&model::strength_label(lang, strength.score), &strength.crack_time])}" }
                }
                label { r#for: "f-url", {tr(lang, Msg::Website)} }
                input { id: "f-url", class: "ltr", dir: "ltr", value: "{f.url}", oninput: field!(url) }
                label { r#for: "f-totp", {tr(lang, Msg::TwoFactorSecret)} }
                input {
                    id: "f-totp", class: "ltr", dir: "ltr", autocomplete: "off", value: "{f.totp}", oninput: field!(totp),
                    aria_invalid: invalid("f-totp"), aria_describedby: describedby("f-totp"),
                }
                if invalid("f-totp") { if let Some(t) = &error_text { p { id: "f-error", class: "error", "{t}" } } }
                label { r#for: "f-tags", {tr(lang, Msg::TagsCommas)} }
                input { id: "f-tags", dir: "auto", value: "{f.tags}", oninput: field!(tags) }
                label { r#for: "f-notes", {tr(lang, Msg::Notes)} }
                textarea { id: "f-notes", dir: "auto", rows: "4", value: "{f.notes}", oninput: field!(notes) }
                div { class: "row",
                    button { class: "primary", r#type: "submit", {tr(lang, Msg::Save)} }
                    button {
                        r#type: "button",
                        onclick: move |_| { if let Some(m) = ctx.model.write().as_mut() { m.cancel_form(now()); } },
                        {tr(lang, Msg::Cancel)}
                    }
                }
            }
        }
    }
}

// ---- settings --------------------------------------------------------------------------------------------------

#[component]
fn SettingsView() -> Element {
    let mut ctx = use_context::<Ctx>();
    let lang = lang_of(&ctx);
    let current = ctx
        .model
        .read()
        .as_ref()
        .map(|m| m.session.settings().clone())
        .unwrap_or_default();
    let mut s = use_signal(|| current.clone());
    let mut new_pw = use_signal(String::new);
    let mut new_pw2 = use_signal(String::new);
    let mut pw_msg = use_signal(|| Option::<(bool, String)>::None);
    let mut export_pw = use_signal(String::new);
    let mut export_format = use_signal(model::ExportFormat::default);
    let mut export_path =
        use_signal(|| model::default_export_path(now(), model::ExportFormat::default()));
    let mut export_msg = use_signal(|| Option::<(bool, String)>::None);
    let me = (ctx.active)().and_then(|id| ctx.profiles.read().get(id).cloned());
    let mut pf_name = use_signal(|| me.as_ref().map(|p| p.full_name.clone()).unwrap_or_default());
    let mut pf_user = use_signal(|| me.as_ref().map(|p| p.username.clone()).unwrap_or_default());
    let mut pf_msg = use_signal(|| Option::<(bool, String)>::None);
    let mut del_pw = use_signal(String::new);
    let mut del_msg = use_signal(|| Option::<String>::None);
    let only_profile = ctx.profiles.read().len() < 2;
    macro_rules! num {
        ($name:ident, $ty:ty) => {
            move |e: FormEvent| {
                if let Ok(v) = e.value().trim().parse::<$ty>() {
                    s.write().$name = v;
                }
            }
        };
    }
    rsx! {
        main { class: "form",
            h2 { {tr(lang, Msg::SettingsTitle)} }
            form {
                onsubmit: move |e| { e.prevent_default(); let new = s.read().clone(); if let Some(m) = ctx.model.write().as_mut() { m.apply_settings(new, now()); } },
                label { r#for: "s-idle", {tr(lang, Msg::SetIdle)} }
                input { id: "s-idle", r#type: "number", min: "0", value: "{s.read().idle_lock_secs / 60}",
                    oninput: move |e| { if let Ok(v) = e.value().trim().parse::<u32>() { s.write().idle_lock_secs = v.saturating_mul(60); } } }
                label { class: "check",
                    input { r#type: "checkbox", checked: s.read().lock_on_blur, onchange: move |e| s.write().lock_on_blur = e.checked() }
                    {tr(lang, Msg::SetBlur)}
                }
                label { r#for: "s-clip", {tr(lang, Msg::SetClip)} }
                input { id: "s-clip", r#type: "number", min: "0", value: "{s.read().clipboard_clear_secs}", oninput: num!(clipboard_clear_secs, u32) }
                label { r#for: "s-reveal", {tr(lang, Msg::SetReveal)} }
                input { id: "s-reveal", r#type: "number", min: "0", value: "{s.read().reveal_secs}", oninput: num!(reveal_secs, u32) }
                label { r#for: "s-trash", {tr(lang, Msg::SetTrash)} }
                input { id: "s-trash", r#type: "number", min: "0", value: "{s.read().trash_retention_days}", oninput: num!(trash_retention_days, u32) }
                label { r#for: "s-hist", {tr(lang, Msg::SetHistory)} }
                input { id: "s-hist", r#type: "number", min: "0", value: "{s.read().history_limit}", oninput: num!(history_limit, usize) }
                label { r#for: "s-toolbar", {tr(lang, Msg::SetToolbar)} }
                select { id: "s-toolbar",
                    onchange: move |e| s.write().toolbar_style = match e.value().as_str() {
                        "text" => ToolbarStyle::TextOnly,
                        "icons" => ToolbarStyle::IconOnly,
                        _ => ToolbarStyle::IconAndText,
                    },
                    option { value: "both", selected: s.read().toolbar_style == ToolbarStyle::IconAndText, {tr(lang, Msg::OptIconsText)} }
                    option { value: "text", selected: s.read().toolbar_style == ToolbarStyle::TextOnly, {tr(lang, Msg::OptTextOnly)} }
                    option { value: "icons", selected: s.read().toolbar_style == ToolbarStyle::IconOnly, {tr(lang, Msg::OptIconsOnly)} }
                }
                label { r#for: "s-theme", {tr(lang, Msg::SetTheme)} }
                select { id: "s-theme",
                    onchange: move |e| s.write().theme = match e.value().as_str() {
                        "light" => ThemeMode::Light,
                        "dark" => ThemeMode::Dark,
                        _ => ThemeMode::System,
                    },
                    option { value: "system", selected: s.read().theme == ThemeMode::System, {tr(lang, Msg::OptSystem)} }
                    option { value: "light", selected: s.read().theme == ThemeMode::Light, {tr(lang, Msg::OptLight)} }
                    option { value: "dark", selected: s.read().theme == ThemeMode::Dark, {tr(lang, Msg::OptDark)} }
                }
                label { r#for: "s-lang", {tr(lang, Msg::SetLanguage)} }
                select { id: "s-lang",
                    onchange: move |e| {
                        if let Some(l) = Language::ALL.into_iter().find(|l| l.code() == e.value()) {
                            s.write().language = l;
                        }
                    },
                    for l in Language::ALL {
                        option { key: "{l.code()}", value: "{l.code()}", selected: s.read().language == l, lang: "{l.code()}", "{l.native_name()}" }
                    }
                }
                h3 { {tr(lang, Msg::GuardTitle)} }
                p { class: "muted small", {tr(lang, Msg::GuardNote)} }
                label { r#for: "s-mem", {tr(lang, Msg::SetMemory)} }
                input { id: "s-mem", r#type: "number", min: "19", value: "{s.read().kdf_memory_mib}", oninput: num!(kdf_memory_mib, u32) }
                label { r#for: "s-pass", {tr(lang, Msg::SetPasses)} }
                input { id: "s-pass", r#type: "number", min: "2", value: "{s.read().kdf_passes}", oninput: num!(kdf_passes, u32) }
                button { class: "primary", r#type: "submit", {tr(lang, Msg::SaveSettings)} }
            }
            h3 { {tr(lang, Msg::ChangeMasterTitle)} }
            form {
                onsubmit: move |e| {
                    e.prevent_default();
                    let (a, b) = (new_pw.read().clone(), new_pw2.read().clone());
                    let r = ctx.model.write().as_mut().map(|m| m.change_master_password(&a, &b, now()));
                    match r {
                        Some(Ok(())) => { new_pw.set(String::new()); new_pw2.set(String::new()); pw_msg.set(Some((true, tr(lang, Msg::MasterChanged).into()))); }
                        Some(Err(e)) => pw_msg.set(Some((false, e))),
                        None => {}
                    }
                },
                label { r#for: "p1", {tr(lang, Msg::NewMasterPassword)} }
                input { id: "p1", class: "ltr", dir: "ltr", r#type: "password", autocomplete: "off", value: "{new_pw}", oninput: move |e| new_pw.set(e.value()) }
                label { r#for: "p2", {tr(lang, Msg::RepeatIt)} }
                input { id: "p2", class: "ltr", dir: "ltr", r#type: "password", autocomplete: "off", value: "{new_pw2}", oninput: move |e| new_pw2.set(e.value()) }
                if let Some((ok, text)) = pw_msg() { p { class: if ok { "ok" } else { "error" }, role: "status", "{text}" } }
                button { class: "primary", r#type: "submit", {tr(lang, Msg::ChangeMasterButton)} }
            }
            h3 { {tr(lang, Msg::ExportTitle)} }
            p { class: "muted small", id: "export-warning", {tr(lang, Msg::ExportWarning)} }
            form {
                onsubmit: move |e| {
                    e.prevent_default();
                    let (pw, path) = (export_pw.read().clone(), export_path.read().clone());
                    let r = ctx.model.write().as_mut().map(|m| m.export(export_format(), &pw, &path, now()));
                    export_pw.set(String::new());
                    match r {
                        Some(Ok((n, file))) => export_msg.set(Some((true, trf(lang, Msg::ExportDone, &[&n, &file.display()])))),
                        Some(Err(e)) => export_msg.set(Some((false, e))),
                        None => {}
                    }
                },
                label { r#for: "x-format", {tr(lang, Msg::ExportFormatLabel)} }
                select { id: "x-format",
                    onchange: move |e| {
                        if let Some(f) = model::ExportFormat::from_extension(&e.value()) {
                            export_format.set(f);
                            let renamed = model::with_export_extension(&export_path.read(), f);
                            export_path.set(renamed);
                        }
                    },
                    for f in model::ExportFormat::ALL {
                        option { key: "{f.extension()}", value: "{f.extension()}", selected: f == export_format(), {tr(lang, f.label())} }
                    }
                }
                label { r#for: "x-path", {tr(lang, Msg::ExportPath)} }
                input { id: "x-path", class: "ltr mono", dir: "ltr", autocomplete: "off", value: "{export_path}", oninput: move |e| export_path.set(e.value()) }
                label { r#for: "x-pw", {tr(lang, Msg::ExportMasterPassword)} }
                input {
                    id: "x-pw", class: "ltr", dir: "ltr", r#type: "password", autocomplete: "off", value: "{export_pw}",
                    aria_describedby: "export-warning", oninput: move |e| export_pw.set(e.value()),
                }
                if let Some((ok, text)) = export_msg() { p { class: if ok { "ok" } else { "error" }, "{text}" } }
                button { class: "primary", r#type: "submit", {tr(lang, Msg::ExportButton)} }
            }
            if me.is_some() {
                h3 { {tr(lang, Msg::ProfileSection)} }
                form {
                    onsubmit: move |e| {
                        e.prevent_default();
                        let Some(id) = (ctx.active)() else { return };
                        let mut list = ctx.profiles.read().clone();
                        let result = list
                            .edit(id, &pf_name.read(), &pf_user.read())
                            .map_err(|e| tr(lang, profile_error_msg(e)).to_string())
                            .and_then(|()| profiles::save(&data_dir(), &list));
                        match result {
                            Ok(()) => {
                                if let Some(p) = list.get(id) {
                                    pf_name.set(p.full_name.clone());
                                    pf_user.set(p.username.clone());
                                }
                                ctx.profiles.set(list);
                                pf_msg.set(Some((true, tr(lang, Msg::ProfileSaved).to_string())));
                            }
                            Err(e) => pf_msg.set(Some((false, e))),
                        }
                    },
                    label { r#for: "pf-name", {tr(lang, Msg::ProfileFullName)} }
                    input { id: "pf-name", autocomplete: "off", value: "{pf_name}", oninput: move |e| pf_name.set(e.value()) }
                    label { r#for: "pf-user", {tr(lang, Msg::ProfileUsername)} }
                    input { id: "pf-user", class: "ltr", dir: "ltr", autocomplete: "off", value: "{pf_user}", oninput: move |e| pf_user.set(e.value()) }
                    if let Some((ok, text)) = pf_msg() { p { class: if ok { "ok" } else { "error" }, role: if ok { "status" } else { "alert" }, "{text}" } }
                    button { class: "primary", r#type: "submit", {tr(lang, Msg::ProfileSave)} }
                }
                h3 { {tr(lang, Msg::DeleteProfileTitle)} }
                if only_profile {
                    p { class: "muted small", {tr(lang, Msg::LastProfile)} }
                } else {
                    p { class: "muted small", id: "delete-warning", {tr(lang, Msg::DeleteProfileWarning)} }
                    form {
                        onsubmit: move |e| {
                            e.prevent_default();
                            let pw = del_pw.read().clone();
                            del_pw.set(String::new());
                            let Some(id) = (ctx.active)() else { return };
                            let mut list = ctx.profiles.read().clone();
                            let checked = ctx.model.write().as_mut().map(|m| m.confirm_password(&pw, now()));
                            match checked {
                                Some(Ok(())) => {}
                                Some(Err(e)) => { del_msg.set(Some(e)); return; }
                                None => return,
                            }
                            let gone = match list.remove(id) {
                                Ok(p) => p,
                                Err(e) => { del_msg.set(Some(tr(lang, profile_error_msg(e)).to_string())); return; }
                            };
                            if let Err(e) = profiles::save(&data_dir(), &list) {
                                del_msg.set(Some(e));
                                return;
                            }
                            // Open the next profile (locked) first, so the deleted vault is closed before its file goes.
                            if let Some(next) = list.all().first() {
                                match open_profile(next) {
                                    Startup::Ready(m) => ctx.model.set(Some(*m)),
                                    Startup::NeedsSetup => {}
                                    Startup::Broken(msg) => ctx.broken.set(Some(msg)),
                                }
                                ctx.active.set(Some(next.id));
                            }
                            if let Some(file) = profiles::vault_file(&data_dir(), &gone) {
                                profiles::delete_vault(&file);
                            }
                            ctx.profiles.set(list);
                        },
                        label { r#for: "del-pw", {tr(lang, Msg::ExportMasterPassword)} }
                        input {
                            id: "del-pw", class: "ltr", dir: "ltr", r#type: "password", autocomplete: "off", value: "{del_pw}",
                            aria_describedby: "delete-warning", oninput: move |e| del_pw.set(e.value()),
                        }
                        if let Some(text) = del_msg() { p { class: "error", role: "alert", "{text}" } }
                        button { class: "danger", r#type: "submit", {tr(lang, Msg::DeleteProfileButton)} }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod shortcut_tests {
    #[test]
    fn keys_sent_by_the_page_script_become_the_same_keys_as_real_events() {
        use dioxus::prelude::{Key, Modifiers};
        let (key, mods) = page_key("2", true, false, false).unwrap();
        assert_eq!(key, Key::Character("2".into()));
        assert!(mods.ctrl() && !mods.alt() && !mods.shift());
        let (key, mods) = page_key("S", true, false, true).unwrap();
        assert_eq!(key, Key::Character("S".into()));
        assert!(mods.ctrl() && mods.shift());
        assert_eq!(
            page_key("Escape", false, false, false).unwrap().0,
            Key::Escape
        );
        assert_eq!(
            Modifiers::empty(),
            page_key("Tab", false, false, false).unwrap().1
        );
        // The shortcut table accepts what the script forwards.
        assert!(
            matches!(Shortcut::from_key("2", false), Some(Shortcut::Trash)),
            "Ctrl+2 opens the trash"
        );
    }

    use super::{page_key, Shortcut};

    #[test]
    fn keys_map_to_shortcuts_and_the_hint_matches() {
        let cases = [
            ("p", false, Shortcut::Pin, "Ctrl+P"),
            ("E", false, Shortcut::Edit, "Ctrl+E"),
            ("d", false, Shortcut::MoveToTrash, "Ctrl+D"),
            ("C", true, Shortcut::CopyPassword, "Ctrl+Shift+C"),
            ("U", true, Shortcut::CopyUsername, "Ctrl+Shift+U"),
            ("W", true, Shortcut::CopyWebsite, "Ctrl+Shift+W"),
            ("T", true, Shortcut::CopyTotp, "Ctrl+Shift+T"),
            ("S", true, Shortcut::ShowPassword, "Ctrl+Shift+S"),
        ];
        for (key, shift, shortcut, hint) in cases {
            assert!(Shortcut::from_key(key, shift) == Some(shortcut), "{key}");
            assert_eq!(shortcut.hint(), hint);
            assert!(shortcut.needs_item());
            assert_eq!(shortcut.aria(), hint.replace("Ctrl", "Control"));
        }
        // The plain letters need Shift for the copy and show shortcuts, so Ctrl+C stays "copy text".
        assert!(Shortcut::from_key("c", false).is_none());
        assert!(Shortcut::from_key("s", false).is_none());
        assert!(!Shortcut::NewItem.needs_item());
    }

    #[test]
    fn fields_have_their_copy_shortcut() {
        assert!(Shortcut::copy_for("password") == Some(Shortcut::CopyPassword));
        assert!(Shortcut::copy_for("totp") == Some(Shortcut::CopyTotp));
        assert!(Shortcut::copy_for("notes").is_none());
    }
}
