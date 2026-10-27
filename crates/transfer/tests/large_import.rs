//! A big CSV imports in time that grows in step with its size, not with its square (audit finding P5-1).
#![cfg(not(target_family = "wasm"))]

use rahasyakosh_transfer::import_csv;
use std::time::Instant;
use uuid::Uuid;

fn ids() -> impl FnMut() -> Uuid {
    let mut n = 0u128;
    move || {
        n += 1;
        Uuid::from_u128(n)
    }
}

fn csv(rows: usize) -> String {
    let mut s = String::from("name,url,username,password\n");
    for i in 0..rows {
        s.push_str(&format!("Site {i},https://s{i}.example,user{i},pw{i}\n"));
    }
    s
}

#[test]
fn forty_thousand_rows_import_quickly() {
    let data = csv(40_000);
    let t = Instant::now();
    let r = import_csv(data.as_bytes(), &[], &[], 0, ids()).unwrap();
    let took = t.elapsed();
    assert_eq!(r.items.len(), 40_000);
    // This took 26 s in a debug build when each row was compared with every earlier one.
    assert!(took.as_secs() < 10, "40 000 rows took {took:?}");
}

#[test]
fn duplicates_and_changed_passwords_are_still_told_apart() {
    let data = "name,url,username,password\n\
                A,https://a.example,me,one\n\
                A,https://a.example/,ME,one\n\
                A,https://a.example,me,two\n\
                B,https://b.example,me,one\n";
    let r = import_csv(data.as_bytes(), &[], &[], 0, ids()).unwrap();
    assert_eq!(r.items.len(), 3, "the second row is the first again");
    assert_eq!(r.skipped_duplicates, 1);
    assert!(
        r.warnings.iter().any(|w| w.starts_with("1 row(s) match")),
        "{:?}",
        r.warnings
    );
}
