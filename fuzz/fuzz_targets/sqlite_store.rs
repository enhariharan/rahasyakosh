#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| rahasyakosh_fuzzcheck::sqlite_store(data));
