#![no_main]

use libfuzzer_sys::fuzz_target;
use redoc_formula::{parse_formula, tokenize};

fuzz_target!(|input: &[u8]| {
    if let Ok(formula) = std::str::from_utf8(input) {
        let _ = tokenize(formula);
        let _ = parse_formula(formula);
    }
});
