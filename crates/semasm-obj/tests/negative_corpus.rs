//! Adversarial corpus regression tests for object parsing.

use std::fs;
use std::path::{Path, PathBuf};

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/negative/objects")
}

fn decode_hex(input: &str) -> Vec<u8> {
    let compact: String = input
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    assert_eq!(compact.len() % 2, 0, "hex fixture has an odd length");
    compact
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("ASCII hex pair");
            u8::from_str_radix(text, 16).expect("valid hex fixture")
        })
        .collect()
}

#[test]
fn malformed_object_corpus_is_rejected_without_panic() {
    let entries = fs::read_dir(corpus_dir()).expect("object corpus directory");
    let mut count = 0;
    for entry in entries {
        let path = entry.expect("corpus entry").path();
        let encoded = fs::read_to_string(&path).expect("text-encoded object fixture");
        let bytes = decode_hex(&encoded);
        assert!(
            semasm_obj::parse(&bytes).is_err(),
            "negative fixture was accepted: {}",
            path.display()
        );
        count += 1;
    }
    assert!(count >= 4, "negative object corpus unexpectedly small");
}
