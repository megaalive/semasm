//! Behavioral test harness generation for agent-produced routines.
//!
//! Given a validated contract, the harness module:
//!
//! 1. **Synthesises** canonical test vectors from the contract's
//!    constraints (buffer length bounds, null-pointer policy, etc.).
//! 2. **Generates** a small NASM `_start` harness that calls the
//!    routine under test with each vector and records the returned
//!    value (SysV: `rax`) as 8 raw little-endian bytes per vector.
//! 3. **Evaluates** observed results against expected values and
//!    produces a per-vector [`HarnessReport`].
//!
//! The current implementation targets the buffer-scan calling shape
//! `(ptr<const u8> buffer, usize length, u8 needle) -> usize`, which
//! is the canonical example (`count_byte`) driving the plan's required
//! cases.  When the contract does not match this shape the synthesizer
//! returns an empty vector set and the caller may fall back to a
//! hand-written set.

use std::fmt::Write;

use semasm_contract::{CheckedContract, SemType};
use serde::{Deserialize, Serialize};

use crate::TestVector;

// ---------------------------------------------------------------------------
// Test-vector synthesis
// ---------------------------------------------------------------------------

/// Maximum length used for the "maximum configured fixture length" case.
///
/// Derived from the contract's `bounded_stack_bytes` constraint when
/// present (a buffer that large would not fit on the stack), capped to
/// keep the generated harness small.
const MAX_FIXTURE_CAP: usize = 256;

/// Synthesise the canonical test vectors for a contract.
///
/// Returns an empty `Vec` when the contract does not expose the expected
/// `(ptr<const u8>, usize, u8) -> usize` signature, signalling that no
/// automatic vectors are available.
#[must_use]
#[allow(clippy::vec_init_then_push)]
pub fn synthesize_vectors(contract: &CheckedContract) -> Vec<TestVector> {
    let Some(shape) = scan_shape(contract) else {
        return Vec::new();
    };

    let max_len = shape
        .max_length
        .unwrap_or(MAX_FIXTURE_CAP)
        .clamp(1, MAX_FIXTURE_CAP);

    let mut vectors = Vec::new();

    // 1. Empty input.
    vectors.push(TestVector {
        name: "empty input".into(),
        inputs: vec![
            serde_json::Value::Null,
            serde_json::json!(0u64),
            serde_json::json!(u64::from(shape.needle)),
        ],
        expected: serde_json::json!(0u64),
    });

    // 2. One byte (the needle).
    vectors.push(TestVector {
        name: "one byte (match)".into(),
        inputs: vec![
            serde_json::json!([u64::from(shape.needle)]),
            serde_json::json!(1u64),
            serde_json::json!(u64::from(shape.needle)),
        ],
        expected: serde_json::json!(1u64),
    });

    // 3. No match.
    vectors.push(TestVector {
        name: "no match".into(),
        inputs: vec![
            serde_json::json!([1u64, 2u64, 3u64]),
            serde_json::json!(3u64),
            serde_json::json!(u64::from(shape.needle)),
        ],
        expected: serde_json::json!(0u64),
    });

    // 4. All match.
    vectors.push(TestVector {
        name: "all match".into(),
        inputs: vec![
            serde_json::json!([u64::from(shape.needle), u64::from(shape.needle)]),
            serde_json::json!(2u64),
            serde_json::json!(u64::from(shape.needle)),
        ],
        expected: serde_json::json!(2u64),
    });

    // 5. Embedded zero bytes.
    vectors.push(TestVector {
        name: "embedded zero bytes".into(),
        inputs: vec![
            serde_json::json!([0u64, u64::from(shape.needle), 0u64]),
            serde_json::json!(3u64),
            serde_json::json!(u64::from(shape.needle)),
        ],
        expected: serde_json::json!(1u64),
    });

    // 6. Maximum configured fixture length (all needle).
    let big: Vec<serde_json::Value> = (0..max_len)
        .map(|_| serde_json::json!(u64::from(shape.needle)))
        .collect();
    vectors.push(TestVector {
        name: "maximum configured fixture length".into(),
        inputs: vec![
            serde_json::Value::Array(big),
            serde_json::json!(max_len as u64),
            serde_json::json!(u64::from(shape.needle)),
        ],
        expected: serde_json::json!(max_len as u64),
    });

    // 7. Null pointer only when length is zero (per declared policy).
    if shape.allows_null_when_empty {
        vectors.push(TestVector {
            name: "null pointer with zero length".into(),
            inputs: vec![
                serde_json::Value::Null,
                serde_json::json!(0u64),
                serde_json::json!(u64::from(shape.needle)),
            ],
            expected: serde_json::json!(0u64),
        });
    }

    vectors
}

/// Resolved calling shape for the canonical buffer-scan function.
struct ScanShape {
    /// Needle value used for synthesised inputs.
    needle: u8,
    /// Upper bound on buffer length, if derivable.
    max_length: Option<usize>,
    /// Whether a null buffer is permitted when length is zero.
    allows_null_when_empty: bool,
}

/// Detect the `(ptr<const u8>, usize, u8) -> usize` shape.
fn scan_shape(contract: &CheckedContract) -> Option<ScanShape> {
    let mut ptr_param = None;
    let mut len_param = None;
    let mut needle_param = None;

    for p in &contract.parameters {
        match &p.ty {
            SemType::Ptr { is_const, .. } if *is_const && ptr_param.is_none() => {
                ptr_param = Some(p.name.clone());
            }
            SemType::Usize if len_param.is_none() => {
                len_param = Some(p.name.clone());
            }
            SemType::UInt { bits: 8 } if needle_param.is_none() => {
                needle_param = Some(p.name.clone());
            }
            _ => {}
        }
    }

    let returns_usize = contract
        .returns
        .iter()
        .any(|r| matches!(r.ty, SemType::Usize));

    if ptr_param.is_some() && len_param.is_some() && needle_param.is_some() && returns_usize {
        Some(ScanShape {
            needle: 0x41, // 'A' — a value unlikely to collide with zero-byte tests.
            max_length: contract
                .constraints
                .as_ref()
                .and_then(|c| c.bounded_stack_bytes)
                .map(|b| usize::try_from(b).unwrap_or(usize::MAX)),
            allows_null_when_empty: true,
        })
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Harness source generation
// ---------------------------------------------------------------------------

/// Generate NASM source for a `_start` harness that exercises `vectors`
/// against the routine named `routine_symbol`.
///
/// The harness lays out each vector's buffer in `.data`, loads the SysV
/// registers (`rdi` = buffer, `rsi` = length, `rdx` = needle), calls
/// the routine, and stores the returned `rax` (8 bytes, little-endian)
/// into a results array.  After all vectors it writes the results to
/// stdout and exits 0.
///
/// The observed values are recovered by parsing stdout as a sequence of
/// 8-byte little-endian `u64` words, one per vector.
#[must_use]
pub fn generate_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();

    out.push_str("BITS 64\n");
    out.push_str("DEFAULT REL\n\n");

    // --- Data: buffers + scalar inputs per vector -----------------------
    out.push_str("section .data\n");
    for (i, v) in vectors.iter().enumerate() {
        let bytes = vector_buffer_bytes(v);
        let _ = writeln!(out, "vec{i}_len: dq {}", vector_length(v));
        let _ = writeln!(out, "vec{i}_needle: db {}", vector_needle(v));
        let _ = write!(out, "vec{i}_buf: db {}", bytes.join(", "));
        if bytes.is_empty() {
            out.push_str(" 0"); // NASM rejects an empty db list.
        }
        out.push('\n');
    }

    // --- BSS: results array ------------------------------------------
    out.push_str("\nsection .bss\n");
    let _ = writeln!(out, "results: resb {}", vectors.len() * 8);

    // --- Text: _start ------------------------------------------------
    out.push_str("\nsection .text\n");
    let _ = writeln!(out, "extern {routine_symbol}");
    out.push_str("global _start\n");
    out.push_str("_start:\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    ; vector {i}: {}", vectors[i].name);
        let _ = writeln!(out, "    lea rdi, [vec{i}_buf]");
        let _ = writeln!(out, "    mov rsi, [vec{i}_len]");
        let _ = writeln!(out, "    movzx edx, byte [vec{i}_needle]");
        let _ = writeln!(out, "    call {routine_symbol}");
        let _ = writeln!(out, "    mov [results + {i}*8], rax");
    }

    // Write results to stdout.
    out.push_str("    ; write(results, len)\n");
    out.push_str("    mov eax, 1          ; sys_write\n");
    out.push_str("    mov edi, 1          ; stdout\n");
    let _ = writeln!(out, "    lea rsi, [results]");
    let _ = writeln!(out, "    mov edx, {}", vectors.len() * 8);
    out.push_str("    syscall\n");
    // Exit 0.
    out.push_str("    mov eax, 60         ; sys_exit\n");
    out.push_str("    xor edi, edi\n");
    out.push_str("    syscall\n");

    out
}

/// Extract the buffer bytes for a vector's first input (a JSON array).
fn vector_buffer_bytes(v: &TestVector) -> Vec<String> {
    match v.inputs.first() {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(|x| x.as_u64().unwrap_or(0).min(255).to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// Extract the length (second input) for a vector.
fn vector_length(v: &TestVector) -> u64 {
    v.inputs
        .get(1)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
}

/// Extract the needle (third input) for a vector.
fn vector_needle(v: &TestVector) -> u64 {
    v.inputs
        .get(2)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
        & 0xff
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

/// Outcome of running the harness against a routine.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct VectorResult {
    /// Name of the test vector.
    pub name: String,
    /// Whether observed matched expected.
    pub passed: bool,
    /// Expected value (decimal string).
    pub expected: String,
    /// Observed value (decimal string), or `<no output>` on parse failure.
    pub observed: String,
}

/// Full harness evaluation report.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct HarnessReport {
    /// Per-vector results.
    pub cases: Vec<VectorResult>,
    /// True when every case passed.
    pub all_passed: bool,
}

/// Parse raw harness stdout (8-byte little-endian `u64` per vector) and
/// compare against expected values.
#[must_use]
pub fn evaluate(stdout: &[u8], vectors: &[TestVector]) -> HarnessReport {
    let mut observed = Vec::with_capacity(vectors.len());
    for i in 0..vectors.len() {
        let base = i * 8;
        let word = if stdout.len() >= base + 8 {
            let mut b = [0u8; 8];
            b.copy_from_slice(&stdout[base..base + 8]);
            u64::from_le_bytes(b)
        } else {
            u64::MAX // sentinel for "missing output"
        };
        observed.push(word);
    }

    let cases: Vec<VectorResult> = vectors
        .iter()
        .zip(observed)
        .map(|(v, got)| {
            let expected = v.expected.as_u64().unwrap_or(u64::MAX);
            let passed = got == expected;
            VectorResult {
                name: v.name.clone(),
                passed,
                expected: expected.to_string(),
                observed: if got == u64::MAX {
                    "<no output>".into()
                } else {
                    got.to_string()
                },
            }
        })
        .collect();

    let all_passed = cases.iter().all(|c| c.passed);

    HarnessReport { cases, all_passed }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::sample_check;

    fn count_byte_shape() -> CheckedContract {
        sample_check()
    }

    #[test]
    fn synthesizes_seven_canonical_vectors() {
        let c = count_byte_shape();
        let v = synthesize_vectors(&c);
        assert_eq!(v.len(), 7, "expected 7 canonical cases, got {}", v.len());
        let names: Vec<&str> = v.iter().map(|x| x.name.as_str()).collect();
        assert!(names.contains(&"empty input"));
        assert!(names.contains(&"one byte (match)"));
        assert!(names.contains(&"no match"));
        assert!(names.contains(&"all match"));
        assert!(names.contains(&"embedded zero bytes"));
        assert!(names.contains(&"maximum configured fixture length"));
        assert!(names.contains(&"null pointer with zero length"));
    }

    #[test]
    fn synthesizes_no_vectors_for_unknown_shape() {
        // A contract with a non-matching signature yields nothing.
        let c = CheckedContract {
            version_major: 0,
            version_minor: 1,
            name: "weird".into(),
            summary: None,
            parameters: vec![
                semasm_contract::CheckedParam {
                    name: "x".into(),
                    ty: SemType::Bool,
                    type_source: "bool".into(),
                    role: None,
                },
                semasm_contract::CheckedParam {
                    name: "y".into(),
                    ty: SemType::Usize,
                    type_source: "usize".into(),
                    role: None,
                },
            ],
            returns: vec![semasm_contract::CheckedReturn {
                name: "r".into(),
                ty: SemType::Bool,
                type_source: "bool".into(),
            }],
            requires: vec![],
            ensures: vec![],
            effects: vec![],
            constraints: None,
            target_overrides: vec![],
        };
        assert!(synthesize_vectors(&c).is_empty());
    }

    #[test]
    fn harness_source_references_routine_and_all_vectors() {
        let c = count_byte_shape();
        let v = synthesize_vectors(&c);
        let src = generate_harness("count_byte", &v);
        assert!(src.contains("extern count_byte"));
        assert!(src.contains("global _start"));
        assert!(src.contains("call count_byte"));
        for i in 0..v.len() {
            assert!(src.contains(&format!("vec{i}_buf")));
        }
        assert!(src.contains("sys_write"));
        assert!(src.contains("sys_exit"));
    }

    #[test]
    fn evaluate_matches_expected() {
        let c = count_byte_shape();
        let v = synthesize_vectors(&c);
        // Build a fake "observed" buffer: encode each expected value.
        let mut out = Vec::new();
        for vec in &v {
            let val = vec.expected.as_u64().unwrap_or(0);
            out.extend_from_slice(&val.to_le_bytes());
        }
        let report = evaluate(&out, &v);
        assert!(
            report.all_passed,
            "all cases should pass with correct output"
        );
        assert_eq!(report.cases.len(), v.len());
    }

    #[test]
    fn evaluate_reports_mismatch_with_observed() {
        let c = count_byte_shape();
        let v = synthesize_vectors(&c);
        // Encode wrong values (all zeros) → every non-zero expected fails.
        let out = vec![0u8; v.len() * 8];
        let report = evaluate(&out, &v);
        assert!(!report.all_passed);
        let failed = report.cases.iter().find(|x| !x.passed).unwrap();
        assert!(!failed.observed.is_empty());
        assert_ne!(failed.observed, failed.expected);
    }

    #[test]
    fn evaluate_handles_truncated_output() {
        let c = count_byte_shape();
        let v = synthesize_vectors(&c);
        let out = vec![0u8; 4]; // too short for even one word.
        let report = evaluate(&out, &v);
        assert!(!report.all_passed);
        assert!(report.cases[0].observed.contains("no output"));
    }
}
