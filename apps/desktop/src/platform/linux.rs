//! Linux: tell the app when a screen reader (Orca) starts or stops.
//!
//! The desktop's accessibility service (`org.a11y.Bus`, on the session bus) has a `Status` object with the
//! properties `IsEnabled` and `ScreenReaderEnabled`; Orca switches both on when it starts and `ScreenReaderEnabled` off when it quits. The service announces a
//! change with the standard `PropertiesChanged` signal, which is what we listen for.
//!
//! This has to run on the main thread: the signal is delivered through that thread's GLib main loop, which the
//! window's event loop runs.
use gio::glib::{self, prelude::*};
use std::cell::RefCell;
use std::collections::HashMap;

/// Run in the page after the window gets keyboard focus back (Alt+Tab) or after in-app navigation while a screen
/// reader runs.
///
/// WebKitGTK reports the whole web view as an unnamed AT-SPI "filler" rather than the focused control inside it.
/// Orca takes the filler as the focused object; because the filler is not a password field, Orca reads out every
/// key typed into the password box. (Recorded with an AT-SPI event listener, 2026-10-24.)
///
/// Two cases:
/// 1. An element already has focus — blur and immediately refocus it. WebKitGTK then reports that specific element
///    to AT-SPI (not the filler). Both steps happen synchronously so no keystroke arrives between them, and focus
///    never visits another control (a button would put Orca in browse mode where letter keys navigate instead of
///    reaching the page).
/// 2. Nothing is focused yet (autofocus pending or new page just mounted) — find the first visible, enabled,
///    non-negative-tabindex element and focus it. This covers initial app launch and in-app page transitions.
///
/// Retries up to ten times at 100 ms intervals to handle slow page loads in the snap environment.
pub const SCRIPT_AFTER_WINDOW_FOCUS: Option<&str> = Some(
    "const refocus=(tries)=>{\
       if(!document.hasFocus()){if(tries>0)setTimeout(()=>refocus(tries-1),100);return}\
       const x=document.activeElement;\
       if(x&&x!==document.body){x.blur();x.focus();return}\
       const f=[...document.querySelectorAll('input:not([disabled]),button:not([disabled]),a[href],[tabindex]')]\
         .filter(e=>e.tabIndex>=0)\
         .find(e=>{const r=e.getBoundingClientRect();return r.width>0&&r.height>0});\
       if(f)f.focus();\
       else if(tries>0)setTimeout(()=>refocus(tries-1),100)\
     };\
     refocus(10)",
);

thread_local! {
    /// Keeps the connection and the subscription alive for as long as the program runs.
    static KEEP: RefCell<Option<(gio::DBusConnection, gio::SignalSubscriptionId)>> = const { RefCell::new(None) };
}

/// Whether the accessibility service says a screen reader is running right now. False if it cannot be asked
/// (no session bus, no accessibility service running: the call does not start one).
pub fn screen_reader_enabled_now() -> bool {
    let Ok(connection) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return false;
    };
    connection
        .call_sync(
            Some("org.a11y.Bus"),
            "/org/a11y/bus",
            "org.freedesktop.DBus.Properties",
            "Get",
            Some(&("org.a11y.Status", "ScreenReaderEnabled").to_variant()),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            500,
            gio::Cancellable::NONE,
        )
        .ok()
        .and_then(|reply| reply.get::<(glib::Variant,)>())
        .and_then(|(value,)| value.get::<bool>())
        .unwrap_or(false)
}

/// Call `on_change(true)` when the accessibility service reports a screen reader started and `on_change(false)` when it
/// reports one stopped.
pub fn watch_screen_reader(on_change: impl Fn(bool) + 'static) {
    // No session bus (a bare session, a container): nothing to watch, and nothing to report.
    let Ok(connection) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return;
    };
    let id = connection.signal_subscribe(
        Some("org.a11y.Bus"),
        Some("org.freedesktop.DBus.Properties"),
        Some("PropertiesChanged"),
        Some("/org/a11y/bus"),
        Some("org.a11y.Status"),
        gio::DBusSignalFlags::NONE,
        move |_, _, _, _, _, parameters| {
            if let Some(enabled) = screen_reader_enabled(parameters) {
                if super::debug() {
                    eprintln!(
                        "Screen reader {}",
                        if enabled { "enabled" } else { "disabled" }
                    );
                }
                on_change(enabled);
            }
        },
    );
    KEEP.with(|k| *k.borrow_mut() = Some((connection, id)));
}

/// The new value of `ScreenReaderEnabled` if a `PropertiesChanged` message `(interface, changed properties,
/// invalidated names)` changes it, else `None`.
fn screen_reader_enabled(parameters: &glib::Variant) -> Option<bool> {
    let (interface, changed, _) =
        parameters.get::<(String, HashMap<String, glib::Variant>, Vec<String>)>()?;
    if interface != "org.a11y.Status" {
        return None;
    }
    changed.get("ScreenReaderEnabled")?.get::<bool>()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `PropertiesChanged` message for `interface` with one changed property.
    fn message(interface: &str, name: &str, value: bool) -> glib::Variant {
        let changed = glib::VariantDict::new(None);
        changed.insert_value(name, &value.to_variant());
        // The real message is `(sa{sv}as)`; building the tuple from parts keeps the dictionary unboxed.
        glib::Variant::tuple_from_iter([
            interface.to_variant(),
            changed.end(),
            Vec::<String>::new().to_variant(),
        ])
    }

    #[test]
    fn the_screen_reader_setting_is_read_in_both_directions() {
        assert_eq!(
            screen_reader_enabled(&message("org.a11y.Status", "ScreenReaderEnabled", true)),
            Some(true)
        );
        assert_eq!(
            screen_reader_enabled(&message("org.a11y.Status", "ScreenReaderEnabled", false)),
            Some(false)
        );
        assert_eq!(
            screen_reader_enabled(&message("org.a11y.Status", "IsEnabled", true)),
            None
        );
        assert_eq!(
            screen_reader_enabled(&message("org.other", "ScreenReaderEnabled", true)),
            None
        );
    }

    #[test]
    fn other_messages_are_ignored() {
        assert_eq!(
            screen_reader_enabled(&message("org.a11y.Status", "Something", true)),
            None
        );
        assert_eq!(screen_reader_enabled(&"not a message".to_variant()), None);
    }
}
