#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| rahasyakosh_fuzzcheck::csv_import(data));
