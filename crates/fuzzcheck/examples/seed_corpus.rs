//! Writes the starting inputs for each fuzz target into `<dir>/<target>/`, built from the committed test vectors.
//!
//!     cargo run -p rahasyakosh-fuzzcheck --example seed_corpus -- fuzz/corpus
//!
//! A fuzzer finds bugs far faster when it starts from valid inputs and mutates them. Seeds are small and committed;
//! `cargo fuzz` adds to the corpus as it finds inputs that reach new code.
use rahasyakosh_core::item::Item;
use rahasyakosh_transfer::Bundle;
use std::{fs, path::PathBuf};

fn main() {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: seed_corpus <corpus dir>"),
    );
    let vectors = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors");
    let golden = fs::read(vectors.join("golden-v1.rkv")).unwrap();
    let bundle = Bundle::from_slice(&golden).unwrap();
    let items: Vec<Item> =
        serde_json::from_slice(&fs::read(vectors.join("golden-v1.expected.json")).unwrap())
            .unwrap();

    let write = |target: &str, name: &str, bytes: &[u8]| {
        let dir = out.join(target);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(name), bytes).unwrap();
    };
    let json = |v: &dyn erased::Ser| v.to_json();

    // vault_header: the real one, and one with the cheapest key settings so unlocking is actually attempted.
    write("vault_header", "golden", &json(&bundle.header));
    let mut cheap = bundle.header.clone();
    cheap.kdf.m_kib = 19 * 1024;
    cheap.kdf.t = 2;
    write("vault_header", "cheap-kdf", &json(&cheap));
    write("vault_header", "empty-object", b"{}");

    // record: JSON records, and the raw layout (24 nonce, 16 id, 8 rev, ciphertext).
    for (i, r) in bundle.records.iter().take(3).enumerate() {
        write("record", &format!("golden-{i}"), &json(r));
    }
    write("record", "raw", &[7u8; 24 + 16 + 8 + 64]);

    // payload: a correctly padded item (u32 LE length, JSON, zero padding), and broken variants.
    let item_json = serde_json::to_vec(&items[0]).unwrap();
    let mut padded = (item_json.len() as u32).to_le_bytes().to_vec();
    padded.extend_from_slice(&item_json);
    padded.resize(padded.len().div_ceil(64) * 64, 0);
    write("payload", "valid", &padded);
    write(
        "payload",
        "length-too-big",
        &[0xff, 0xff, 0xff, 0xff, b'{', b'}'],
    );
    write("payload", "too-short", &[1, 0]);

    // item_json: each golden item.
    for (i, it) in items.iter().enumerate() {
        write("item_json", &format!("golden-{i}"), &json(it));
    }

    // csv_import
    write(
        "csv_import",
        "chrome",
        b"name,url,username,password,note\nGitHub,https://github.com/login,alice,pw1,work\n",
    );
    write("csv_import", "firefox", b"url,username,password,httpRealm,formActionOrigin,guid\nhttps://www.example.com/a,bob,pw,,https://www.example.com,{x}\n");
    write("csv_import", "generic", b"title,url,username,password,notes,totp,tags\nBank,bank.test,me,pw,\"line1\nline2\",JBSWY3DPEHPK3PXP,a;b\n");
    write(
        "csv_import",
        "bom-ragged",
        b"\xEF\xBB\xBFname,password\nA,p,extra,columns\n,\n\"unterminated,quote\n",
    );

    // totp
    write("totp", "base32", b"JBSWY3DPEHPK3PXP");
    write("totp", "uri", b"otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example&algorithm=SHA256&digits=8&period=60");
    write(
        "totp",
        "bad-uri",
        b"otpauth://totp/?secret=&digits=99&period=0",
    );

    // bundle
    write("bundle", "golden", &golden);
    write("bundle", "truncated", &golden[..golden.len() / 2]);
    write("bundle", "no-records", br#"{"header":null,"records":[]}"#);

    // directory_store: a selector byte (which file to corrupt) followed by that file's content.
    let rec = json(&bundle.records[0]);
    let sel = |s: u8, body: &[u8]| {
        let mut v = vec![s];
        v.extend_from_slice(body);
        v
    };
    write("directory_store", "record", &sel(1, &rec));
    write("directory_store", "tombstone", &sel(2, br#"{"rev":9}"#));
    write(
        "directory_store",
        "both",
        &sel(3, &[rec.clone(), br#"{"rev":9}"#.to_vec()].concat()),
    );
    write("directory_store", "header", &sel(0, &json(&bundle.header)));
    write("directory_store", "garbage", &sel(1, b"{ not json"));

    // sqlite_store: the committed schema-v1 fixture, damaged copies of it, and plain garbage.
    let db = fs::read(vectors.join("schema-v1.sqlite")).unwrap();
    write("sqlite_store", "schema-v1", &db);
    let mut damaged = db.clone();
    damaged[100] ^= 0xff;
    write("sqlite_store", "damaged-header", &damaged);
    write("sqlite_store", "truncated", &db[..db.len() / 2]);
    write("sqlite_store", "garbage", &[0x5a; 512]);

    // strength
    for (i, pw) in [
        "password",
        "correct horse battery staple",
        "Tr0ub4dor&3",
        "aaaaaaaaaaaaaaaa",
        "qwertyuiop",
    ]
    .iter()
    .enumerate()
    {
        write("strength", &format!("pw-{i}"), pw.as_bytes());
    }
}

/// Tiny helper so `json(&x)` accepts any serialisable value without a generic parameter on the closure.
mod erased {
    pub trait Ser {
        fn to_json(&self) -> Vec<u8>;
    }
    impl<T: serde::Serialize> Ser for T {
        fn to_json(&self) -> Vec<u8> {
            serde_json::to_vec(self).unwrap()
        }
    }
}
