//! Linux: tell the app when a screen reader (Orca) starts.
//!
//! The desktop's accessibility service (`org.a11y.Bus`, on the session bus) has a `Status` object with the
//! properties `IsEnabled` and `ScreenReaderEnabled`; Orca switches both on when it starts. The service announces a
//! change with the standard `PropertiesChanged` signal, which is what we listen for.
//!
//! This has to run on the main thread: the signal is delivered through that thread's GLib main loop, which the
//! window's event loop runs.
use gio::glib;
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    /// Keeps the connection and the subscription alive for as long as the program runs.
    static KEEP: RefCell<Option<(gio::DBusConnection, gio::SignalSubscriptionId)>> = const { RefCell::new(None) };
}

/// Call `on_start` whenever the accessibility service reports a screen reader (or accessibility in general) switched on.
pub fn watch_screen_reader(on_start: impl Fn() + 'static) {
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
            if super::debug() {
                match screen_reader_enabled(parameters) {
                    Some(true) => eprintln!("Screen reader enabled"),
                    Some(false) => eprintln!("Screen reader disabled"),
                    None => {}
                }
            }
            if switched_on(parameters) {
                on_start();
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

/// Whether a `PropertiesChanged` message `(interface, changed properties, invalidated names)` says that
/// `ScreenReaderEnabled` or `IsEnabled` became true.
fn switched_on(parameters: &glib::Variant) -> bool {
    let Some((interface, changed, _)) =
        parameters.get::<(String, HashMap<String, glib::Variant>, Vec<String>)>()
    else {
        return false;
    };
    interface == "org.a11y.Status"
        && ["ScreenReaderEnabled", "IsEnabled"]
            .iter()
            .any(|name| changed.get(*name).and_then(|v| v.get::<bool>()) == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gio::glib::prelude::*;

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
    fn a_screen_reader_switching_on_is_noticed() {
        assert!(switched_on(&message(
            "org.a11y.Status",
            "ScreenReaderEnabled",
            true
        )));
        assert!(switched_on(&message("org.a11y.Status", "IsEnabled", true)));
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
    fn switching_off_or_other_messages_are_ignored() {
        assert!(!switched_on(&message(
            "org.a11y.Status",
            "ScreenReaderEnabled",
            false
        )));
        assert!(!switched_on(&message("org.a11y.Status", "Something", true)));
        assert!(!switched_on(&message(
            "org.other",
            "ScreenReaderEnabled",
            true
        )));
        assert!(!switched_on(&"not a message".to_variant()));
    }
}
