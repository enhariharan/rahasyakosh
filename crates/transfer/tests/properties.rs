//! Property tests for the CSV importer and exporter. Native only: `proptest` does not build for WASI.
#![cfg(not(target_family = "wasm"))]

use proptest::prelude::*;
use rahasyakosh_core::item::Item;
use rahasyakosh_transfer::{export_csv, export_json, import_csv};
use uuid::Uuid;

fn ids() -> impl FnMut() -> Uuid {
    let mut n = 0u128;
    move || {
        n += 1;
        Uuid::from_u128(n)
    }
}

/// Anything: quotes, commas, newlines, control characters, spaces at the edges, non-BMP characters.
fn text() -> impl Strategy<Value = String> {
    "(?s).{0,30}"
}

/// Text the importer is allowed to normalise: no edge whitespace, never empty.
fn label() -> impl Strategy<Value = String> {
    "[^\\s;,][^;,]{0,20}"
        .prop_map(|s| s.trim().to_string())
        .prop_filter("non-empty", |s| !s.is_empty())
}

#[derive(Debug, Clone)]
struct Row {
    title: String,
    url: Option<String>,
    username: String,
    password: String,
    notes: String,
    tags: Vec<String>,
}

fn row() -> impl Strategy<Value = Row> {
    (
        label(),
        prop::option::of(label()),
        text(),
        text(),
        text(),
        prop::collection::vec(label(), 0..3),
    )
        .prop_map(|(title, url, username, password, notes, tags)| Row {
            title,
            url,
            username,
            password,
            notes,
            tags,
        })
}

proptest! {
    #[test]
    fn importing_arbitrary_bytes_never_panics(data in prop::collection::vec(any::<u8>(), 0..2048)) {
        let _ = import_csv(&data, &[], &[], 0, ids());
    }

    #[test]
    fn importing_arbitrary_text_with_csv_headers_never_panics(body in "(?s).{0,400}") {
        let data = format!("name,url,username,password,note,totp,tags\n{body}");
        let _ = import_csv(data.as_bytes(), &[], &[], 0, ids());
    }

    #[test]
    fn csv_export_then_import_preserves_every_value(rows in prop::collection::vec(row(), 0..8)) {
        let items: Vec<Item> = rows.iter().enumerate().map(|(i, r)| {
            // A unique prefix keeps the duplicate detector from merging generated rows.
            let mut item = Item::login(&format!("{i}-{}", r.title), &r.username, &r.password, 1, Uuid::from_u128(i as u128 + 1));
            item.urls.extend(r.url.clone());
            item.notes = r.notes.clone();
            item.tags = r.tags.clone();
            item
        }).collect();

        let csv = export_csv(&items).unwrap();
        let report = import_csv(csv.as_bytes(), &[], &[], 1, ids()).unwrap();
        prop_assert_eq!(report.skipped_duplicates, 0);
        prop_assert_eq!(report.skipped_empty, 0);
        prop_assert_eq!(report.items.len(), items.len());
        for (a, b) in items.iter().zip(&report.items) {
            prop_assert_eq!(&a.title, &b.title);
            prop_assert_eq!(&a.urls, &b.urls);
            prop_assert_eq!(&a.notes, &b.notes);
            prop_assert_eq!(&a.tags, &b.tags);
            // Secrets and free text must come back exactly, not trimmed or altered.
            prop_assert_eq!(&a.field("username").unwrap().value, &b.field("username").unwrap().value);
            prop_assert_eq!(&a.field("password").unwrap().value, &b.field("password").unwrap().value);
        }
    }

    #[test]
    fn importing_the_same_export_twice_adds_nothing_the_second_time(rows in prop::collection::vec(row(), 1..6)) {
        let items: Vec<Item> = rows.iter().enumerate().map(|(i, r)| {
            Item::login(&format!("{i}-{}", r.title), &r.username, &r.password, 1, Uuid::from_u128(i as u128 + 1))
        }).collect();
        let csv = export_csv(&items).unwrap();
        let first = import_csv(csv.as_bytes(), &[], &[], 1, ids()).unwrap();
        let second = import_csv(csv.as_bytes(), &[], &first.items, 2, ids()).unwrap();
        prop_assert_eq!(second.items.len(), 0);
        prop_assert_eq!(second.skipped_duplicates, items.len());
    }

    #[test]
    fn exports_leave_out_trashed_items(rows in prop::collection::vec(row(), 1..6), trashed in any::<prop::sample::Index>()) {
        let mut items: Vec<Item> = rows.iter().enumerate().map(|(i, r)| {
            Item::login(&format!("{i}-{}", r.title), &r.username, &r.password, 1, Uuid::from_u128(i as u128 + 1))
        }).collect();
        let gone = trashed.index(items.len());
        items[gone].deleted_at = Some(5);
        let report = import_csv(export_csv(&items).unwrap().as_bytes(), &[], &[], 1, ids()).unwrap();
        prop_assert_eq!(report.items.len(), items.len() - 1);
        let json: serde_json::Value = serde_json::from_str(&export_json(&items).unwrap()).unwrap();
        prop_assert_eq!(json["items"].as_array().unwrap().len(), items.len() - 1);
    }
}
