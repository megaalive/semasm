//! Adversarial corpus regression tests for contract and expression parsing.

use std::fs;
use std::path::{Path, PathBuf};

use semasm_contract::{check_str, Expr};

fn corpus_dir(kind: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/negative")
        .join(kind)
}

#[test]
fn malformed_contract_corpus_is_rejected() {
    let entries = fs::read_dir(corpus_dir("contracts")).expect("contract corpus directory");
    let mut count = 0;
    for entry in entries {
        let path = entry.expect("corpus entry").path();
        let input = fs::read_to_string(&path).expect("UTF-8 contract fixture");
        let result = check_str(&input);
        assert!(
            !result.ok(),
            "negative fixture was accepted: {}",
            path.display()
        );
        count += 1;
    }
    assert!(count >= 3, "negative contract corpus unexpectedly small");
}

#[test]
fn malformed_expression_corpus_is_rejected_without_panic() {
    let entries = fs::read_dir(corpus_dir("expressions")).expect("expression corpus directory");
    let mut count = 0;
    for entry in entries {
        let path = entry.expect("corpus entry").path();
        let input = fs::read_to_string(&path).expect("UTF-8 expression fixture");
        let parsed = Expr::parse(input.trim());
        if path.file_name().and_then(|name| name.to_str()) == Some("pathological-depth.txt") {
            assert!(
                parsed.is_ok(),
                "deep but valid expression should parse safely"
            );
        } else {
            assert!(
                parsed.is_err(),
                "negative fixture was accepted: {}",
                path.display()
            );
        }
        count += 1;
    }
    assert!(count >= 3, "negative expression corpus unexpectedly small");
}
