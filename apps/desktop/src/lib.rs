//! Rahasyakosh desktop app (library, so examples can launch the same screens). This file only draws screens and forwards clicks; the behaviour lives in `model.rs`
//! (tested without a window) and in the session of `rahasyakosh-app`.
pub mod model;
#[cfg(test)]
mod model_tests;
mod style;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use model::{Model, NoticeKind, SystemClipboard, View};
use rahasyakosh_app::{Session, Settings};
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
        Ok(store) => Startup::Ready(Box::new(make_model(Session::new(store, load_settings())))),
        Err(e) => Startup::Broken(format!("Cannot open the vault: {e}")),
    }
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
    let window = WindowBuilder::new()
        .with_title("Rahasyakosh")
        .with_window_icon(window_icon())
        .with_inner_size(LogicalSize::new(1080.0, 720.0))
        .with_min_inner_size(LogicalSize::new(760.0, 520.0));
    dioxus::LaunchBuilder::desktop()
        .with_cfg(Config::new().with_window(window).with_menu(None))
        .launch(app);
}

/// The taskbar / title-bar icon: the chest, pre-rendered to raw 64x64 RGBA (see `packaging/`).
fn window_icon() -> Option<dioxus::desktop::tao::window::Icon> {
    dioxus::desktop::tao::window::Icon::from_rgba(include_bytes!("../packaging/window-icon-64.rgba").to_vec(), 64, 64).ok()
}

/// The app icon (same drawing as `packaging/rahasyakosh-chest.svg`), shown above the title on the first-run and unlock screens.
#[component]
fn Logo() -> Element {
    rsx! {
        svg { class: "logo", view_box: "0 0 128 128", width: "72", height: "72", role: "img", "aria-label": "Rahasyakosh",
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

fn app() -> Element {
    let mut model: Signal<Option<AppModel>> = use_signal(|| None);
    let mut broken: Signal<Option<String>> = use_signal(|| None);
    let clock = use_signal(now);
    let last_input = use_hook(|| Rc::new(Cell::new(now())));
    use_hook(|| {
        if let Some(m) = INITIAL.with(|c| c.borrow_mut().take()) {
            model.set(Some(m));
            return;
        }
        match startup(&vault_path()) {
            Startup::Ready(m) => model.set(Some(*m)),
            Startup::NeedsSetup => {}
            Startup::Broken(msg) => broken.set(Some(msg)),
        };
    });
    let ctx = use_context_provider(|| Ctx {
        model,
        clock,
        last_input: last_input.clone(),
        broken,
    });

    // Once a second: pass on user input, apply the inactivity lock, clear the clipboard when due, and redraw.
    {
        let last_input = last_input.clone();
        let mut clock = clock;
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
                }
            }
        });
    }

    // Lock when the window loses focus (if enabled), and wipe the clipboard secret when it is closed.
    dioxus::desktop::use_wry_event_handler(move |event, _| {
        use dioxus::desktop::tao::event::{Event, WindowEvent};
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::Focused(false) => {
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
    rsx! {
        style { "{style::CSS}" }
        div {
            class: "app",
            onpointermove: move |_| input.set(now()),
            onpointerdown: move |_| input_down.set(now()),
            onkeydown: move |e| {
                input_key.set(now());
                if e.modifiers().ctrl() && e.key() == Key::Character("k".into()) {
                    e.prevent_default();
                    spawn(async move {
                        let _ = dioxus::document::eval("document.getElementById('search')?.focus()").await;
                    });
                }
            },
            { screen(ctx.clone()) }
        }
    }
}

fn screen(ctx: Ctx) -> Element {
    let _ = (ctx.clock)(); // redraw every second
    if let Some(msg) = (ctx.broken)() {
        return rsx! { main { class: "centre", h1 { "Rahasyakosh" } p { class: "error", role: "alert", "{msg}" } } };
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
    let mut passphrase = use_signal(new_passphrase);
    let mut started = use_signal(|| false);
    let mut own = use_signal(|| false);
    let mut pw = use_signal(String::new);
    let mut pw2 = use_signal(String::new);
    let mut saved = use_signal(|| false);
    let mut copied = use_signal(|| false);
    let mut error = use_signal(|| Option::<String>::None);
    let strength = rahasyakosh_strength::estimate(&pw.read(), &[]);

    let create = move |_| {
        let password = match model::new_master_password(
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
        let settings = load_settings();
        let path = vault_path();
        match SqliteStore::open(&path)
            .map_err(|e| e.to_string())
            .and_then(|store| {
                Session::create(store, &password, settings, now(), &mut rand::rng())
                    .map_err(|e| e.to_string())
            }) {
            Ok(session) => ctx.model.set(Some(make_model(session))),
            Err(e) => error.set(Some(format!("Could not create the vault: {e}"))),
        }
    };

    if !started() {
        return rsx! {
            main { class: "centre landing",
                Logo {}
                h1 { "Welcome to Rahasyakosh" }
                p { class: "tagline spaced",
                    "A secure store for your passwords." br {}
                    "Stored in an encrypted file on this computer." br {}
                    "Protected by a master passphrase."
                }
                button { class: "primary", onclick: move |_| started.set(true), "Create master passphrase" }
            }
        };
    }

    // Copy the generated passphrase, and take it off the clipboard again after the usual delay if it is still there.
    let copy_passphrase = move |_| {
        let text = passphrase.read().clone();
        if model::Clipboard::copy(&mut SystemClipboard, &text).is_err() {
            error.set(Some("Could not copy to the clipboard.".into()));
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
            h1 { "Create a master passphrase" }
            p { class: "warn", "This master passphrase is shown " span { class: "emph", "only once" } ". Keep it safe." }
            if !own() {
                section { aria_label: "Generated passphrase",
                    p { class: "label", "Your new master passphrase" }
                    div { class: "passbox",
                        p { class: "passphrase", "{passphrase}" }
                        button {
                            class: "icon", r#type: "button",
                            aria_label: if copied() { "Passphrase copied" } else { "Copy passphrase" },
                            title: if copied() { "Copied" } else { "Copy to clipboard" },
                            onclick: copy_passphrase,
                            if copied() { CheckIcon {} } else { CopyIcon {} }
                        }
                    }
                    div { class: "row",
                        button { onclick: move |_| { passphrase.set(new_passphrase()); saved.set(false); }, "Generate another" }
                        button { class: "link", onclick: move |_| own.set(true), "Choose my own instead" }
                    }
                    label { class: "check",
                        input { r#type: "checkbox", checked: saved(), onchange: move |e| saved.set(e.checked()) }
                        " I have written it down"
                    }
                }
            } else {
                section { aria_label: "Choose a master password",
                    label { r#for: "pw", "Master password" }
                    input { id: "pw", r#type: "password", value: "{pw}", oninput: move |e| pw.set(e.value()) }
                    label { r#for: "pw2", "Repeat it" }
                    input { id: "pw2", r#type: "password", value: "{pw2}", oninput: move |e| pw2.set(e.value()) }
                    if !pw.read().is_empty() {
                        p { class: "meter", "Strength: {strength.label()} (offline cracking time: {strength.crack_time})" }
                        if let Some(w) = strength.warning.clone() { p { class: "hint", "{w}" } }
                    }
                    button { class: "link", onclick: move |_| own.set(false), "Use a generated passphrase instead" }
                }
            }
            if let Some(e) = error() { p { class: "error", role: "alert", "{e}" } }
            button { class: "primary", onclick: create, disabled: !own() && !saved(), "Create vault" }
            p { class: "small", "Vault location: {vault_path().display()}" }
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
    let mut submit = move || {
        let t = now();
        if let Some(m) = ctx.model.write().as_mut() {
            m.unlock(&pw.read(), t);
        }
        pw.set(String::new());
    };
    rsx! {
        main { class: "centre landing",
            Logo {}
            h1 { "Rahasyakosh" }
            p { class: "tagline spaced", "Vault locked." }
            form {
                onsubmit: move |e| { e.prevent_default(); submit(); },
                label { class: "sr-only", r#for: "master", "Master password" }
                input {
                    id: "master", r#type: "password", autofocus: true, autocomplete: "off",
                    placeholder: "Enter master password",
                    value: "{pw}", oninput: move |e| pw.set(e.value()),
                }
                if let Some(e) = err { p { class: "error", role: "alert", "{e}" } }
                if wait > 0 { p { class: "muted", "Try again in {wait} s." } }
                button { class: "primary", r#type: "submit", disabled: wait > 0, "Unlock" }
            }
        }
    }
}

// ---- the main window ----------------------------------------------------------------------------------------

#[component]
fn Main() -> Element {
    let mut ctx = use_context::<Ctx>();
    let t = (ctx.clock)();
    let (view, query, left, notice, trash_count) = {
        let m = ctx.model.read();
        let m = m.as_ref().expect("unlocked");
        (
            m.view.clone(),
            m.query.clone(),
            m.session.seconds_until_lock(t),
            m.notice.clone(),
            m.trash_count(),
        )
    };
    let lock = move |_| {
        if let Some(m) = ctx.model.write().as_mut() {
            m.lock();
        }
    };
    let lock_text = match left {
        Some(s) if s < 60 => format!("Locks in {s} s"),
        Some(s) => format!("Locks in {} min", (s + 59) / 60),
        None => "Auto-lock off".to_string(),
    };
    rsx! {
        header { class: "bar",
            h1 { "Rahasyakosh" }
            input {
                id: "search", r#type: "search", placeholder: "Search  (Ctrl+K)", aria_label: "Search",
                value: "{query}",
                oninput: move |e| {
                    if let Some(m) = ctx.model.write().as_mut() {
                        m.query = e.value();
                        if m.view != View::Items { m.show(View::Items, now()); m.query = e.value(); }
                    }
                },
            }
            nav { aria_label: "Main",
                button {
                    aria_current: if view == View::Items || view == View::Form { "page" } else { "false" },
                    class: if view == View::Items || view == View::Form { "tab on" } else { "tab" },
                    onclick: move |_| { if let Some(m) = ctx.model.write().as_mut() { m.show(View::Items, now()); } },
                    "Items"
                }
                button {
                    aria_current: if view == View::Trash { "page" } else { "false" },
                    class: if view == View::Trash { "tab on" } else { "tab" },
                    onclick: move |_| { if let Some(m) = ctx.model.write().as_mut() { m.show(View::Trash, now()); } },
                    "Trash ({trash_count})"
                }
                button {
                    aria_current: if view == View::Settings { "page" } else { "false" },
                    class: if view == View::Settings { "tab on" } else { "tab" },
                    onclick: move |_| { if let Some(m) = ctx.model.write().as_mut() { m.show(View::Settings, now()); } },
                    "Settings"
                }
            }
            span { class: "muted small", "{lock_text}" }
            button { class: "danger", onclick: lock, "Lock" }
        }
        if let Some(n) = notice {
            div {
                class: if n.kind == NoticeKind::Error { "toast error" } else { "toast" },
                role: if n.kind == NoticeKind::Error { "alert" } else { "status" },
                "{n.text}"
            }
        }
        match view {
            View::Settings => rsx! { SettingsView {} },
            View::Form => rsx! { FormView {} },
            _ => rsx! { Browser {} },
        }
    }
}

#[component]
fn Browser() -> Element {
    let ctx = use_context::<Ctx>();
    let mut model = ctx.model;
    let _ = (ctx.clock)();
    let (rows, selected, view) = {
        let m = ctx.model.read();
        let m = m.as_ref().expect("unlocked");
        (m.rows(), m.selected, m.view.clone())
    };
    let trash = view == View::Trash;
    rsx! {
        div { class: "split",
            aside { aria_label: if trash { "Deleted items" } else { "Items" },
                if !trash {
                    button {
                        class: "primary wide",
                        onclick: move |_| { if let Some(m) = model.write().as_mut() { m.start_new(now()); } },
                        "+ New item"
                    }
                } else {
                    p { class: "muted small pad", "Items are deleted for good after 30 days." }
                }
                if rows.is_empty() {
                    p { class: "muted pad", if trash { "The trash is empty." } else { "Nothing here yet." } }
                }
                ul { class: "list", aria_label: "Item list",
                    for r in rows {
                        li { key: "{r.id}",
                            button {
                                class: if selected == Some(r.id) { "row on" } else { "row" },
                                aria_current: if selected == Some(r.id) { "true" } else { "false" },
                                onclick: move |_| select(model, r.id),
                                span { class: "title", if r.favourite { span { class: "sr-only", "Pinned: " } span { aria_hidden: "true", "★ " } } "{r.title}" }
                                span { class: "sub", "{r.subtitle}" }
                            }
                        }
                    }
                }
            }
            main { class: "detail", aria_label: "Item details",
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
    let detail = ctx.model.read().as_ref().and_then(|m| m.detail(t));
    let Some(d) = detail else {
        return rsx! { p { class: "muted pad", "Select an item, or press Ctrl+K to search." } };
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
    rsx! {
        div { class: "pad",
            div { class: "head",
                h2 { if d.favourite { "★ " } "{d.title}" }
                div { class: "row",
                    if d.trashed {
                        button { class: "primary", onclick: act!(m, now => m.restore_selected(now)), "Restore" }
                        button { class: "danger", onclick: act!(m, now => m.purge_selected(now)), "Delete for good" }
                    } else {
                        button { onclick: act!(m, now => m.toggle_favourite(now)), if d.favourite { "Unpin" } else { "Pin" } }
                        button { onclick: act!(m, now => m.start_edit(now)), "Edit" }
                        button { class: "danger", onclick: act!(m, now => m.trash_selected(now)), "Move to trash" }
                    }
                }
            }
            dl {
                for f in d.fields.clone() {
                    {
                        let name = f.name.clone();
                        let (n1, n2, n3) = (name.clone(), name.clone(), name.clone());
                        rsx! {
                            div { class: "field", key: "{f.name}",
                                dt { "{f.label}" }
                                dd {
                                    if f.secret && f.value.is_none() { span { class: "mask", role: "img", aria_label: "{f.label} is hidden", "••••••••••" } }
                                    else if f.name == "notes" { span { class: "notes", "{f.value.clone().unwrap_or_default()}" } }
                                    else { span { class: if f.secret { "mono" } else { "" }, "{f.value.clone().unwrap_or_default()}" } }
                                    span { class: "row tight",
                                        if f.secret {
                                            if f.value.is_none() {
                                                button { aria_label: "Show {f.label}", onclick: act!(m, now => m.reveal(&n1, now)), "Show" }
                                            } else {
                                                button { aria_label: "Hide {f.label}", onclick: act!(m, now => m.hide(&n2, now)), "Hide" }
                                            }
                                        }
                                        button { aria_label: "Copy {f.label}", onclick: act!(m, now => m.copy(&n3, now)), "Copy" }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Some(t) = d.totp.clone() {
                    div { class: "field",
                        dt { "Two-factor code" }
                        dd {
                            span { class: "mono big", "{t.code}" }
                            span { class: "muted small", " {t.seconds_left} s" }
                            span { class: "row tight", button { aria_label: "Copy two-factor code", onclick: act!(m, now => m.copy("totp", now)), "Copy" } }
                        }
                    }
                }
                for u in d.urls.clone() {
                    div { class: "field", key: "{u}",
                        dt { "Website" }
                        dd { span { "{u}" } span { class: "row tight", button { aria_label: "Copy website {u}", onclick: act!(m, now => m.copy("url", now)), "Copy" } } }
                    }
                }
                if !d.tags.is_empty() {
                    div { class: "field",
                        dt { "Tags" }
                        dd { for tag in d.tags.clone() { span { class: "tag", key: "{tag}", "{tag}" } } }
                    }
                }
            }
            if !d.history.is_empty() {
                h3 { "Earlier versions" }
                ul { class: "history",
                    for h in d.history.clone() {
                        li { key: "{h.index}",
                            span { "{h.when}: changed {h.changed}" }
                            if !d.trashed {
                                button { aria_label: "Go back to the version from {h.when}", onclick: act!(m, now => m.revert_selected(h.index, now)), "Go back to this" }
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
    let form = ctx.model.read().as_ref().and_then(|m| m.form.clone());
    let Some(f) = form else { return rsx! {} };
    let editing = f.id.is_some();
    let strength = rahasyakosh_strength::estimate(&f.password, &[&f.title, &f.username]);
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
            h2 { if editing { "Edit item" } else { "New item" } }
            form {
                onsubmit: move |e| { e.prevent_default(); if let Some(m) = ctx.model.write().as_mut() { m.save_form(now()); } },
                label { r#for: "f-title", "Title" }
                input { id: "f-title", autofocus: true, value: "{f.title}", oninput: field!(title) }
                label { r#for: "f-user", "Username or email" }
                input { id: "f-user", autocomplete: "off", value: "{f.username}", oninput: field!(username) }
                label { r#for: "f-pw", "Password" }
                div { class: "row",
                    input { id: "f-pw", class: "grow mono", autocomplete: "off", value: "{f.password}", oninput: field!(password) }
                    button {
                        r#type: "button",
                        onclick: move |_| { if let Some(m) = ctx.model.write().as_mut() { m.generate_password(20); } },
                        aria_label: "Generate a random password",
                        "Generate"
                    }
                }
                if !f.password.is_empty() {
                    p { class: "meter", "Strength: {strength.label()} (offline cracking time: {strength.crack_time})" }
                }
                label { r#for: "f-url", "Website" }
                input { id: "f-url", value: "{f.url}", oninput: field!(url) }
                label { r#for: "f-totp", "Two-factor secret (base32 or otpauth:// link)" }
                input { id: "f-totp", autocomplete: "off", value: "{f.totp}", oninput: field!(totp) }
                label { r#for: "f-tags", "Tags (separated by commas)" }
                input { id: "f-tags", value: "{f.tags}", oninput: field!(tags) }
                label { r#for: "f-notes", "Notes" }
                textarea { id: "f-notes", rows: "4", value: "{f.notes}", oninput: field!(notes) }
                div { class: "row",
                    button { class: "primary", r#type: "submit", "Save" }
                    button {
                        r#type: "button",
                        onclick: move |_| { if let Some(m) = ctx.model.write().as_mut() { m.cancel_form(now()); } },
                        "Cancel"
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
            h2 { "Settings" }
            form {
                onsubmit: move |e| { e.prevent_default(); let new = s.read().clone(); if let Some(m) = ctx.model.write().as_mut() { m.apply_settings(new, now()); } },
                label { r#for: "s-idle", "Lock after this many minutes of inactivity (0 = never)" }
                input { id: "s-idle", r#type: "number", min: "0", value: "{s.read().idle_lock_secs / 60}",
                    oninput: move |e| { if let Ok(v) = e.value().trim().parse::<u32>() { s.write().idle_lock_secs = v.saturating_mul(60); } } }
                label { class: "check",
                    input { r#type: "checkbox", checked: s.read().lock_on_blur, onchange: move |e| s.write().lock_on_blur = e.checked() }
                    " Lock as soon as this window loses focus"
                }
                label { r#for: "s-clip", "Clear a copied password from the clipboard after this many seconds (0 = never)" }
                input { id: "s-clip", r#type: "number", min: "0", value: "{s.read().clipboard_clear_secs}", oninput: num!(clipboard_clear_secs, u32) }
                label { r#for: "s-reveal", "Hide a shown password again after this many seconds (0 = when I hide it)" }
                input { id: "s-reveal", r#type: "number", min: "0", value: "{s.read().reveal_secs}", oninput: num!(reveal_secs, u32) }
                label { r#for: "s-trash", "Keep deleted items for this many days" }
                input { id: "s-trash", r#type: "number", min: "0", value: "{s.read().trash_retention_days}", oninput: num!(trash_retention_days, u32) }
                label { r#for: "s-hist", "Earlier versions kept per item" }
                input { id: "s-hist", r#type: "number", min: "0", value: "{s.read().history_limit}", oninput: num!(history_limit, usize) }
                h3 { "Protection against password guessing" }
                p { class: "muted small",
                    "How costly it is to guess your master password from a stolen copy of the vault. Higher is safer but makes unlocking slower. "
                    "This applies when a vault is created or its master password is changed."
                }
                label { r#for: "s-mem", "Memory used, in MiB" }
                input { id: "s-mem", r#type: "number", min: "19", value: "{s.read().kdf_memory_mib}", oninput: num!(kdf_memory_mib, u32) }
                label { r#for: "s-pass", "Passes" }
                input { id: "s-pass", r#type: "number", min: "2", value: "{s.read().kdf_passes}", oninput: num!(kdf_passes, u32) }
                button { class: "primary", r#type: "submit", "Save settings" }
            }
            h3 { "Change the master password" }
            form {
                onsubmit: move |e| {
                    e.prevent_default();
                    let (a, b) = (new_pw.read().clone(), new_pw2.read().clone());
                    let r = ctx.model.write().as_mut().map(|m| m.change_master_password(&a, &b, now()));
                    match r {
                        Some(Ok(())) => { new_pw.set(String::new()); new_pw2.set(String::new()); pw_msg.set(Some((true, "Master password changed.".into()))); }
                        Some(Err(e)) => pw_msg.set(Some((false, e))),
                        None => {}
                    }
                },
                label { r#for: "p1", "New master password" }
                input { id: "p1", r#type: "password", autocomplete: "off", value: "{new_pw}", oninput: move |e| new_pw.set(e.value()) }
                label { r#for: "p2", "Repeat it" }
                input { id: "p2", r#type: "password", autocomplete: "off", value: "{new_pw2}", oninput: move |e| new_pw2.set(e.value()) }
                if let Some((ok, text)) = pw_msg() { p { class: if ok { "ok" } else { "error" }, role: "status", "{text}" } }
                button { r#type: "submit", "Change master password" }
            }
        }
    }
}
