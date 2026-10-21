//! Runs every robustness check on stable Rust, in ordinary `cargo test`:
//!  1. over the committed seed corpus (`fuzz/corpus/<check>/`), and
//!  2. over every input that ever crashed a fuzzer (`fuzz/regressions/<check>/`), so a fixed bug stays fixed, and
//!  3. over random bytes.
//!
//! Finding new bugs is the fuzzer's job (`cargo fuzz run <check>`, see the book); this keeps the old ones from returning.
use proptest::prelude::*;
use rahasyakosh_fuzzcheck::CHECKS;
use std::path::{Path, PathBuf};

fn fuzz_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz")
}

fn files_in(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

#[test]
fn every_check_has_a_seed_corpus() {
    for (name, _) in CHECKS {
        assert!(
            !files_in(&fuzz_dir().join("corpus").join(name)).is_empty(),
            "no seeds for `{name}`; run: cargo run -p rahasyakosh-fuzzcheck --example seed_corpus -- fuzz/corpus"
        );
    }
}

#[test]
fn corpus_and_past_crashes_pass_every_check() {
    for (name, check) in CHECKS {
        for kind in ["corpus", "regressions"] {
            for file in files_in(&fuzz_dir().join(kind).join(name)) {
                let data = std::fs::read(&file).unwrap();
                // Name the file if the check panics, so the failing input is easy to find.
                let result = std::panic::catch_unwind(|| check(&data));
                assert!(
                    result.is_ok(),
                    "check `{name}` failed on {}",
                    file.display()
                );
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn random_bytes_pass_every_check(data in prop::collection::vec(any::<u8>(), 0..600)) {
        for (_, check) in CHECKS {
            check(&data);
        }
    }
}
