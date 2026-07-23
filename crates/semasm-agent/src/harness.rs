//! Behavioral test harness generation for agent-produced routines.
//!
//! Given a validated contract, the harness module:
//!
//! 1. **Synthesises** canonical test vectors from the contract's
//!    requires/effects (buffer length bounds, needle, null-pointer policy).
//! 2. **Generates** a small assembler harness that calls the routine under
//!    test with each vector and records the returned value as 8 raw
//!    little-endian bytes per vector:
//!    - System V AMD64 ELF: NASM `_start` + Linux `write`/`exit` syscalls
//!    - Microsoft x64 PE: NASM `main` + `WriteFile` / `ExitProcess`
//!    - AArch64 Linux ELF: GNU as `_start` + Linux `svc` write/exit
//!    - RISC-V Linux ELF: GNU as `_start` + Linux `ecall` write/exit
//! 3. **Evaluates** observed results against expected values and
//!    produces a per-vector [`HarnessReport`].
//!
//! Supported calling shapes:
//!
//! - **Buffer scan** `(ptr<const u8> buffer, usize length, u8 needle) -> usize`
//!   — `count_byte` (count), `find_first_byte` (first index, or length if absent),
//!   or `find_last_byte` (last index, or length if absent).
//! - **MemCmp** `(ptr<const u8> a, ptr<const u8> b, usize length) -> isize`
//!   - unsigned lexicographic comparison returning only `-1`, `0`, or `1`.
//! - **I64 wrapping sum** `(ptr<const i64> values, usize length) -> i64`
//!   — canonical example `sum_i64`.
//! - **Pure integer** `(usize, usize) -> usize` — canonical examples
//!   `min_usize` / `max_usize` (same calling shape; op from name + ensures).
//!
//! Synthesis tries buffer-scan, MemCmp, i64-sum, then pure-integer.  When the
//! contract matches none of those shapes the synthesizer returns an empty
//! vector set and the caller may fall back to a hand-written set.
//!
//! Synthesis rules for the buffer-scan shape (contract schema 0.1):
//!
//! - **max length** from `requires` of the form `length <= N` / `length < N`
//!   (clamped to [`MAX_FIXTURE_CAP`]); not from `bounded_stack_bytes`.
//! - **needle** from `requires` `needle == K` / `K == needle`, else
//!   [`DEFAULT_BUFFER_SCAN_NEEDLE`].
//! - **null when empty** only when an effect `memory_read` names region
//!   `{ptr}[0..{len}]` for the shape's pointer and length parameters.

use std::fmt::Write;

use semasm_contract::{BinOp, CheckedContract, Expr, SemType};
use semasm_target::Abi;
use serde::{Deserialize, Serialize};

use crate::TestVector;

// ---------------------------------------------------------------------------
// Test-vector synthesis
// ---------------------------------------------------------------------------

/// Maximum length used for the "maximum configured fixture length" case.
///
/// Caps synthesised buffers so the generated harness stays small even when
/// `requires` allow a larger logical bound (for example `length <= 4096`).
const MAX_FIXTURE_CAP: usize = 256;

/// Default needle when the contract does not constrain the u8 parameter.
///
/// This is a synthesizer fixture value (`'A'`), not a claim that the contract
/// requires this byte.
const DEFAULT_BUFFER_SCAN_NEEDLE: u8 = 0x41;

/// Synthesise the canonical test vectors for a contract.
///
/// Returns an empty `Vec` when the contract does not match a supported
/// calling shape (buffer-scan or pure-integer), signalling that no automatic
/// vectors are available.
#[must_use]
pub fn synthesize_vectors(contract: &CheckedContract) -> Vec<TestVector> {
    if let Some(shape) = scan_shape(contract) {
        return match shape.op {
            BufferScanOp::Count => synthesize_buffer_scan_vectors(shape),
            BufferScanOp::FindFirst => synthesize_find_first_vectors(shape),
            BufferScanOp::FindLast => synthesize_find_last_vectors(shape),
        };
    }
    if let Some(shape) = memcmp_shape(contract) {
        return synthesize_memcmp_vectors(shape);
    }
    if let Some(shape) = i64_sum_shape(contract) {
        return synthesize_i64_sum_vectors(shape);
    }
    if let Some(op) = pure_int_shape(contract) {
        return synthesize_pure_int_vectors(op);
    }
    Vec::new()
}

/// Builtin oracle id for buffer-scan count-equal-u8 (`count_byte` shape).
pub const ORACLE_BUFFER_COUNT_EQUAL_U8: &str = "builtin.buffer.count_equal_u8";
/// Profile version for [`ORACLE_BUFFER_COUNT_EQUAL_U8`] (v2: ensures vs claim split).
pub const ORACLE_BUFFER_COUNT_EQUAL_U8_VERSION: u32 = 2;
/// Builtin oracle id for buffer find-first-u8 (`find_first_byte` shape).
pub const ORACLE_BUFFER_FIND_FIRST_U8: &str = "builtin.buffer.find_first_u8";
/// Profile version for [`ORACLE_BUFFER_FIND_FIRST_U8`].
pub const ORACLE_BUFFER_FIND_FIRST_U8_VERSION: u32 = 1;
/// Builtin oracle id for buffer find-last-u8 (`find_last_byte` shape).
pub const ORACLE_BUFFER_FIND_LAST_U8: &str = "builtin.buffer.find_last_u8";
/// Profile version for [`ORACLE_BUFFER_FIND_LAST_U8`].
pub const ORACLE_BUFFER_FIND_LAST_U8_VERSION: u32 = 1;
/// Builtin oracle id for unsigned bytewise lexicographic comparison.
pub const ORACLE_BUFFER_MEMCMP_I8: &str = "builtin.buffer.memcmp_i8";
/// Profile version for [`ORACLE_BUFFER_MEMCMP_I8`].
pub const ORACLE_BUFFER_MEMCMP_I8_VERSION: u32 = 1;
/// Builtin oracle id for i64 wrapping-sum shapes (`sum_i64`).
pub const ORACLE_BUFFER_WRAPPING_SUM_I64: &str = "builtin.buffer.wrapping_sum_i64";
/// Profile version for [`ORACLE_BUFFER_WRAPPING_SUM_I64`] (v2: ensures vs claim split).
pub const ORACLE_BUFFER_WRAPPING_SUM_I64_VERSION: u32 = 2;
/// Builtin oracle id for pure two-`usize` integer shapes (`min_usize` / `max_usize`).
pub const ORACLE_PURE_INT_BINARY_USIZE: &str = "builtin.pure_int.binary_usize";
/// Profile version for [`ORACLE_PURE_INT_BINARY_USIZE`].
pub const ORACLE_PURE_INT_BINARY_USIZE_VERSION: u32 = 2;

/// Recognized binary pure-integer operation for `(usize, usize) -> usize`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PureIntOp {
    Min,
    Max,
}

impl PureIntOp {
    fn claim(self) -> &'static str {
        match self {
            Self::Min => "result equals min(a, b) for the recognized two-usize pure-integer shape",
            Self::Max => "result equals max(a, b) for the recognized two-usize pure-integer shape",
        }
    }

    fn reduce(self, a: u64, b: u64) -> u64 {
        match self {
            Self::Min => a.min(b),
            Self::Max => a.max(b),
        }
    }
}

/// Recognized builtin behavioral oracle for a contract shape, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecognizedOracle {
    /// Stable oracle id.
    pub id: &'static str,
    /// Integer profile version.
    pub version: u32,
    /// Human-readable claim checked by vectors (not a formal ensures AST).
    pub claim: &'static str,
}

/// Detect which named behavioral oracle applies to a contract.
///
/// Contracts may only state weak `ensures` (e.g. `count <= length`). Equality
/// for golden shapes is proven by the returned oracle plus synthesized vectors.
#[must_use]
pub fn recognize_behavior_oracle(contract: &CheckedContract) -> Option<RecognizedOracle> {
    if let Some(shape) = scan_shape(contract) {
        return Some(match shape.op {
            BufferScanOp::Count => RecognizedOracle {
                id: ORACLE_BUFFER_COUNT_EQUAL_U8,
                version: ORACLE_BUFFER_COUNT_EQUAL_U8_VERSION,
                claim: "result equals the number of bytes in buffer[0..length] equal to needle",
            },
            BufferScanOp::FindFirst => RecognizedOracle {
                id: ORACLE_BUFFER_FIND_FIRST_U8,
                version: ORACLE_BUFFER_FIND_FIRST_U8_VERSION,
                claim: "result equals the first index of needle in buffer[0..length], or length when absent",
            },
            BufferScanOp::FindLast => RecognizedOracle {
                id: ORACLE_BUFFER_FIND_LAST_U8,
                version: ORACLE_BUFFER_FIND_LAST_U8_VERSION,
                claim: "result equals the last index of needle in buffer[0..length], or length when absent",
            },
        });
    }
    if memcmp_shape(contract).is_some() {
        return Some(RecognizedOracle {
            id: ORACLE_BUFFER_MEMCMP_I8,
            version: ORACLE_BUFFER_MEMCMP_I8_VERSION,
            claim: "result is -1, 0, or 1 from unsigned lexicographic comparison of a[0..length] and b[0..length]",
        });
    }
    if i64_sum_shape(contract).is_some() {
        return Some(RecognizedOracle {
            id: ORACLE_BUFFER_WRAPPING_SUM_I64,
            version: ORACLE_BUFFER_WRAPPING_SUM_I64_VERSION,
            claim: "result equals the wrapping sum of i64 elements in values[0..length]",
        });
    }
    if let Some(op) = pure_int_shape(contract) {
        return Some(RecognizedOracle {
            id: ORACLE_PURE_INT_BINARY_USIZE,
            version: ORACLE_PURE_INT_BINARY_USIZE_VERSION,
            claim: op.claim(),
        });
    }
    None
}

/// True when the leaf must not store to memory: read-only buffer/sum shapes, or
/// pure-integer shapes that do not declare `memory_write`.
#[must_use]
pub fn is_read_only_buffer_scan(contract: &CheckedContract) -> bool {
    if pure_int_shape(contract).is_some() {
        return !contract
            .effects
            .iter()
            .any(|effect| effect.kind == "memory_write");
    }
    if scan_shape(contract).is_none()
        && memcmp_shape(contract).is_none()
        && i64_sum_shape(contract).is_none()
    {
        return false;
    }
    let has_read = contract
        .effects
        .iter()
        .any(|effect| effect.kind == "memory_read");
    let has_write = contract
        .effects
        .iter()
        .any(|effect| effect.kind == "memory_write");
    has_read && !has_write
}

/// Build a [`crate::verify::BehaviorOracle`] snapshot for a verification report.
#[must_use]
pub fn build_behavior_oracle(
    recognized: RecognizedOracle,
    contract: &CheckedContract,
    contract_path: &str,
    contract_bytes: &[u8],
    planned_vectors: &[TestVector],
    behavior: Option<&HarnessReport>,
) -> crate::verify::BehaviorOracle {
    use sha2::{Digest, Sha256};

    use crate::verify::ProofBasis;

    let contract_hash = {
        let digest = Sha256::digest(contract_bytes);
        let mut out = String::with_capacity(12);
        for byte in digest.iter().take(6) {
            let _ = write!(out, "{byte:02x}");
        }
        out
    };

    let contract_ensures: Vec<String> = contract
        .ensures
        .iter()
        .map(|condition| condition.source.clone())
        .collect();

    let (vectors_passed, vectors_failed, vectors_total) = match behavior {
        Some(report) => {
            let passed = report.cases.iter().filter(|c| c.passed).count();
            let total = report.cases.len();
            (passed, total.saturating_sub(passed), total)
        }
        None => (0, 0, planned_vectors.len()),
    };

    let mut hasher = Sha256::new();
    hasher.update(recognized.id.as_bytes());
    hasher.update(b"\0");
    hasher.update(recognized.version.to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(ProofBasis::OracleAndVectors.as_str().as_bytes());
    hasher.update(b"\0");
    for ensure in &contract_ensures {
        hasher.update(ensure.as_bytes());
        hasher.update(b"\n");
    }
    if let Some(report) = behavior {
        for case in &report.cases {
            hasher.update(case.name.as_bytes());
            hasher.update(b"\0");
            hasher.update(case.expected.as_bytes());
            hasher.update(b"\0");
            hasher.update(case.observed.as_bytes());
            hasher.update(b"\0");
            hasher.update([u8::from(case.passed)]);
            hasher.update(b"\n");
        }
    } else {
        for vector in planned_vectors {
            hasher.update(vector.name.as_bytes());
            hasher.update(b"\0");
            hasher.update(vector.expected.to_string().as_bytes());
            hasher.update(b"\n");
        }
    }
    let digest = hasher.finalize();
    let mut evidence_hash = String::with_capacity(32);
    for byte in digest.iter().take(16) {
        let _ = write!(evidence_hash, "{byte:02x}");
    }

    crate::verify::BehaviorOracle {
        id: recognized.id.to_string(),
        version: recognized.version,
        contract: contract_path.to_string(),
        contract_hash,
        contract_ensures,
        proof_basis: ProofBasis::OracleAndVectors,
        claim: recognized.claim.to_string(),
        vectors_passed,
        vectors_failed,
        vectors_total,
        evidence_hash,
    }
}

/// Validate that synthesised vectors match the named oracle for `contract`.
///
/// Fail-closed when a recognized oracle's calling shape does not match the
/// vector layout (prevents coincidental harness passes).
pub fn validate_vectors_match_oracle(
    contract: &CheckedContract,
    vectors: &[TestVector],
) -> Result<(), String> {
    let Some(oracle) = recognize_behavior_oracle(contract) else {
        return Err("no recognized behavior oracle for contract shape".into());
    };
    let shape = detect_harness_shape(vectors)?;
    let expected = match oracle.id {
        ORACLE_BUFFER_COUNT_EQUAL_U8 | ORACLE_BUFFER_FIND_FIRST_U8 | ORACLE_BUFFER_FIND_LAST_U8 => {
            HarnessShape::BufferScan
        }
        ORACLE_BUFFER_MEMCMP_I8 => HarnessShape::MemCmp,
        ORACLE_BUFFER_WRAPPING_SUM_I64 => HarnessShape::I64Sum,
        ORACLE_PURE_INT_BINARY_USIZE => HarnessShape::PureInt,
        other => {
            return Err(format!("unrecognized oracle id `{other}`"));
        }
    };
    if shape != expected {
        return Err(format!(
            "oracle `{}` expects {:?} vectors but harness detected {:?}",
            oracle.id, expected, shape
        ));
    }
    Ok(())
}

/// Resolved calling shape for the harness generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HarnessShape {
    BufferScan,
    MemCmp,
    I64Sum,
    PureInt,
}

/// Detect harness shape from the first test vector's input layout.
fn detect_harness_shape(vectors: &[TestVector]) -> Result<HarnessShape, String> {
    let Some(first) = vectors.first() else {
        return Err("no test vectors".into());
    };
    match first.inputs.len() {
        3 if matches!(
            first.inputs.first(),
            Some(serde_json::Value::Null | serde_json::Value::Array(_))
        ) && matches!(
            first.inputs.get(1),
            Some(serde_json::Value::Null | serde_json::Value::Array(_))
        ) && first
            .inputs
            .get(2)
            .is_some_and(serde_json::Value::is_number) =>
        {
            Ok(HarnessShape::MemCmp)
        }
        3 if matches!(
            first.inputs.first(),
            Some(serde_json::Value::Null | serde_json::Value::Array(_))
        ) && first.inputs[1..].iter().all(serde_json::Value::is_number) =>
        {
            Ok(HarnessShape::BufferScan)
        }
        2 if matches!(
            first.inputs.first(),
            Some(serde_json::Value::Null | serde_json::Value::Array(_))
        ) && first
            .inputs
            .get(1)
            .is_some_and(serde_json::Value::is_number) =>
        {
            Ok(HarnessShape::I64Sum)
        }
        2 if first.inputs.iter().all(serde_json::Value::is_number) => Ok(HarnessShape::PureInt),
        n => Err(format!(
            "unsupported test vector shape ({n} inputs); \
             expected buffer-scan (3: array/null + two numbers), memcmp \
             (3: two arrays/null + length), i64-sum (2: array/null + length), \
             or pure-int (2 numeric)"
        )),
    }
}

/// Synthesise canonical buffer-scan test vectors.
#[allow(clippy::vec_init_then_push)]
fn synthesize_buffer_scan_vectors(shape: ScanShape) -> Vec<TestVector> {
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

    // 3. No match (buffer bytes deliberately avoid the needle).
    let no_match = non_needle_bytes(shape.needle);
    vectors.push(TestVector {
        name: "no match".into(),
        inputs: vec![
            serde_json::json!([
                u64::from(no_match[0]),
                u64::from(no_match[1]),
                u64::from(no_match[2])
            ]),
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

    // 7. Null pointer only when length is zero (per derived policy).
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

/// Synthesise canonical find-first test vectors for `(ptr<const u8>, usize, u8) -> usize`.
///
/// Absent needle yields `length` (not a sentinel outside the buffer).
#[allow(clippy::vec_init_then_push)]
fn synthesize_find_first_vectors(shape: ScanShape) -> Vec<TestVector> {
    let max_len = shape
        .max_length
        .unwrap_or(MAX_FIXTURE_CAP)
        .clamp(1, MAX_FIXTURE_CAP);
    let needle = u64::from(shape.needle);
    let no_match = non_needle_bytes(shape.needle);

    let mut vectors = Vec::new();

    vectors.push(TestVector {
        name: "empty input".into(),
        inputs: vec![
            serde_json::Value::Null,
            serde_json::json!(0u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(0u64),
    });

    vectors.push(TestVector {
        name: "one byte (match)".into(),
        inputs: vec![
            serde_json::json!([needle]),
            serde_json::json!(1u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(0u64),
    });

    vectors.push(TestVector {
        name: "no match".into(),
        inputs: vec![
            serde_json::json!([
                u64::from(no_match[0]),
                u64::from(no_match[1]),
                u64::from(no_match[2])
            ]),
            serde_json::json!(3u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(3u64),
    });

    vectors.push(TestVector {
        name: "all match".into(),
        inputs: vec![
            serde_json::json!([needle, needle]),
            serde_json::json!(2u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(0u64),
    });

    vectors.push(TestVector {
        name: "match after zeros".into(),
        inputs: vec![
            serde_json::json!([0u64, needle, 0u64]),
            serde_json::json!(3u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(1u64),
    });

    vectors.push(TestVector {
        name: "match at end".into(),
        inputs: vec![
            serde_json::json!([u64::from(no_match[0]), u64::from(no_match[1]), needle]),
            serde_json::json!(3u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(2u64),
    });

    let big: Vec<serde_json::Value> = (0..max_len).map(|_| serde_json::json!(needle)).collect();
    vectors.push(TestVector {
        name: "maximum configured fixture length".into(),
        inputs: vec![
            serde_json::Value::Array(big),
            serde_json::json!(max_len as u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(0u64),
    });

    if shape.allows_null_when_empty {
        vectors.push(TestVector {
            name: "null pointer with zero length".into(),
            inputs: vec![
                serde_json::Value::Null,
                serde_json::json!(0u64),
                serde_json::json!(needle),
            ],
            expected: serde_json::json!(0u64),
        });
    }

    vectors
}

/// Synthesise canonical find-last test vectors for `(ptr<const u8>, usize, u8) -> usize`.
///
/// Absent needle yields `length` (same fail-closed contract as find-first).
#[allow(clippy::vec_init_then_push)]
fn synthesize_find_last_vectors(shape: ScanShape) -> Vec<TestVector> {
    let max_len = shape
        .max_length
        .unwrap_or(MAX_FIXTURE_CAP)
        .clamp(1, MAX_FIXTURE_CAP);
    let needle = u64::from(shape.needle);
    let no_match = non_needle_bytes(shape.needle);

    let mut vectors = Vec::new();

    vectors.push(TestVector {
        name: "empty input".into(),
        inputs: vec![
            serde_json::Value::Null,
            serde_json::json!(0u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(0u64),
    });

    vectors.push(TestVector {
        name: "one byte (match)".into(),
        inputs: vec![
            serde_json::json!([needle]),
            serde_json::json!(1u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(0u64),
    });

    vectors.push(TestVector {
        name: "no match".into(),
        inputs: vec![
            serde_json::json!([
                u64::from(no_match[0]),
                u64::from(no_match[1]),
                u64::from(no_match[2])
            ]),
            serde_json::json!(3u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(3u64),
    });

    vectors.push(TestVector {
        name: "all match".into(),
        inputs: vec![
            serde_json::json!([needle, needle]),
            serde_json::json!(2u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(1u64),
    });

    vectors.push(TestVector {
        name: "match after zeros".into(),
        inputs: vec![
            serde_json::json!([0u64, needle, 0u64]),
            serde_json::json!(3u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(1u64),
    });

    vectors.push(TestVector {
        name: "match at end".into(),
        inputs: vec![
            serde_json::json!([u64::from(no_match[0]), u64::from(no_match[1]), needle]),
            serde_json::json!(3u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(2u64),
    });

    vectors.push(TestVector {
        name: "last of two matches".into(),
        inputs: vec![
            serde_json::json!([needle, u64::from(no_match[0]), needle]),
            serde_json::json!(3u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!(2u64),
    });

    let big: Vec<serde_json::Value> = (0..max_len).map(|_| serde_json::json!(needle)).collect();
    vectors.push(TestVector {
        name: "maximum configured fixture length".into(),
        inputs: vec![
            serde_json::Value::Array(big),
            serde_json::json!(max_len as u64),
            serde_json::json!(needle),
        ],
        expected: serde_json::json!((max_len as u64) - 1),
    });

    if shape.allows_null_when_empty {
        vectors.push(TestVector {
            name: "null pointer with zero length".into(),
            inputs: vec![
                serde_json::Value::Null,
                serde_json::json!(0u64),
                serde_json::json!(needle),
            ],
            expected: serde_json::json!(0u64),
        });
    }

    vectors
}

/// Synthesise canonical unsigned lexicographic comparison vectors.
#[allow(clippy::vec_init_then_push)]
fn synthesize_memcmp_vectors(shape: MemCmpShape) -> Vec<TestVector> {
    let max_len = shape
        .max_length
        .unwrap_or(MAX_FIXTURE_CAP)
        .clamp(1, MAX_FIXTURE_CAP);
    let mut vectors = Vec::new();

    vectors.push(TestVector {
        name: "empty buffers".into(),
        inputs: vec![
            serde_json::json!([]),
            serde_json::json!([]),
            serde_json::json!(0u64),
        ],
        expected: serde_json::json!(0i64),
    });
    vectors.push(TestVector {
        name: "equal buffers".into(),
        inputs: vec![
            serde_json::json!([0u64, 127, 255]),
            serde_json::json!([0u64, 127, 255]),
            serde_json::json!(3u64),
        ],
        expected: serde_json::json!(0i64),
    });
    vectors.push(TestVector {
        name: "a less than b".into(),
        inputs: vec![
            serde_json::json!([0u64]),
            serde_json::json!([255u64]),
            serde_json::json!(1u64),
        ],
        expected: serde_json::json!(-1i64),
    });
    vectors.push(TestVector {
        name: "a greater than b".into(),
        inputs: vec![
            serde_json::json!([255u64]),
            serde_json::json!([0u64]),
            serde_json::json!(1u64),
        ],
        expected: serde_json::json!(1i64),
    });
    vectors.push(TestVector {
        name: "equal prefix then difference".into(),
        inputs: vec![
            serde_json::json!([1u64, 2, 3, 4]),
            serde_json::json!([1u64, 2, 3, 5]),
            serde_json::json!(4u64),
        ],
        expected: serde_json::json!(-1i64),
    });

    let equal: Vec<serde_json::Value> = (0..max_len)
        .map(|i| serde_json::json!((i & 0xff) as u64))
        .collect();
    vectors.push(TestVector {
        name: "maximum configured fixture length".into(),
        inputs: vec![
            serde_json::Value::Array(equal.clone()),
            serde_json::Value::Array(equal),
            serde_json::json!(max_len as u64),
        ],
        expected: serde_json::json!(0i64),
    });

    if shape.allows_null {
        vectors.push(TestVector {
            name: "null buffers with zero length".into(),
            inputs: vec![
                serde_json::Value::Null,
                serde_json::Value::Null,
                serde_json::json!(0u64),
            ],
            expected: serde_json::json!(0i64),
        });
    }

    vectors
}

/// Synthesise canonical pure-integer test vectors for `(usize, usize) -> usize`.
#[allow(clippy::vec_init_then_push)]
fn synthesize_pure_int_vectors(op: PureIntOp) -> Vec<TestVector> {
    let cases: [(&str, u64, u64); 6] = [
        ("both zero", 0, 0),
        ("a smaller", 1, 2),
        ("b smaller", 5, 3),
        ("equal", 7, 7),
        ("zero and large", 0, 1_000_000),
        ("wide spread", 100, 50),
    ];

    cases
        .into_iter()
        .map(|(name, a, b)| TestVector {
            name: name.into(),
            inputs: vec![serde_json::json!(a), serde_json::json!(b)],
            expected: serde_json::json!(op.reduce(a, b)),
        })
        .collect()
}

/// Synthesise canonical wrapping-sum vectors for `(ptr<const i64>, usize) -> i64`.
#[allow(clippy::vec_init_then_push)]
fn synthesize_i64_sum_vectors(shape: I64SumShape) -> Vec<TestVector> {
    let max_len = shape
        .max_length
        .unwrap_or(MAX_FIXTURE_CAP)
        .clamp(1, MAX_FIXTURE_CAP);

    let mut vectors = Vec::new();

    vectors.push(TestVector {
        name: "empty".into(),
        inputs: vec![serde_json::Value::Null, serde_json::json!(0u64)],
        expected: serde_json::json!(0i64),
    });

    vectors.push(TestVector {
        name: "positive".into(),
        inputs: vec![serde_json::json!([1i64, 2, 3, 4]), serde_json::json!(4u64)],
        expected: serde_json::json!(10i64),
    });

    vectors.push(TestVector {
        name: "mixed".into(),
        inputs: vec![serde_json::json!([-5i64, 2, 10]), serde_json::json!(3u64)],
        expected: serde_json::json!(7i64),
    });

    vectors.push(TestVector {
        name: "single negative".into(),
        inputs: vec![serde_json::json!([-1i64]), serde_json::json!(1u64)],
        expected: serde_json::json!(-1i64),
    });

    vectors.push(TestVector {
        name: "wrapping overflow".into(),
        inputs: vec![serde_json::json!([i64::MAX, 1i64]), serde_json::json!(2u64)],
        expected: serde_json::json!(i64::MIN),
    });

    let ones: Vec<serde_json::Value> = (0..max_len).map(|_| serde_json::json!(1i64)).collect();
    vectors.push(TestVector {
        name: "maximum configured fixture length".into(),
        inputs: vec![
            serde_json::Value::Array(ones),
            serde_json::json!(max_len as u64),
        ],
        expected: serde_json::json!(i64::try_from(max_len).unwrap_or(i64::MAX)),
    });

    if shape.allows_null_when_empty {
        vectors.push(TestVector {
            name: "null pointer with zero length".into(),
            inputs: vec![serde_json::Value::Null, serde_json::json!(0u64)],
            expected: serde_json::json!(0i64),
        });
    }

    vectors
}

/// Detect the `(usize, usize) -> usize` pure-integer shape and which binary op.
///
/// Discriminator (fail-closed when ambiguous or conflicting):
/// - function name containing `min` xor `max` (case-insensitive), and/or
/// - weak ensures `result <= a` + `result <= b` (min) vs `result >= a` +
///   `result >= b` (max).
fn pure_int_shape(contract: &CheckedContract) -> Option<PureIntOp> {
    if !pure_int_types(contract) {
        return None;
    }

    let by_name = pure_int_op_from_name(&contract.name);
    let by_ensures = pure_int_op_from_ensures(contract);

    match (by_name, by_ensures) {
        (Some(a), Some(b)) if a == b => Some(a),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (Some(_), Some(_)) | (None, None) => None,
    }
}

fn pure_int_types(contract: &CheckedContract) -> bool {
    let usize_count = contract
        .parameters
        .iter()
        .filter(|p| matches!(p.ty, SemType::Usize))
        .count();
    if usize_count != 2 {
        return false;
    }

    let has_ptr = contract
        .parameters
        .iter()
        .any(|p| matches!(p.ty, SemType::Ptr { .. }));
    let has_u8 = contract
        .parameters
        .iter()
        .any(|p| matches!(p.ty, SemType::UInt { bits: 8 }));
    if has_ptr || has_u8 {
        return false;
    }

    contract
        .returns
        .iter()
        .any(|r| matches!(r.ty, SemType::Usize))
}

fn pure_int_op_from_name(name: &str) -> Option<PureIntOp> {
    let lower = name.to_ascii_lowercase();
    let has_min = lower.contains("min");
    let has_max = lower.contains("max");
    match (has_min, has_max) {
        (true, false) => Some(PureIntOp::Min),
        (false, true) => Some(PureIntOp::Max),
        _ => None,
    }
}

fn pure_int_op_from_ensures(contract: &CheckedContract) -> Option<PureIntOp> {
    let sources: Vec<String> = contract
        .ensures
        .iter()
        .map(|c| {
            c.source
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .collect();

    let le_pair =
        sources.iter().any(|s| s == "result<=a") && sources.iter().any(|s| s == "result<=b");
    let ge_pair =
        sources.iter().any(|s| s == "result>=a") && sources.iter().any(|s| s == "result>=b");

    match (le_pair, ge_pair) {
        (true, false) => Some(PureIntOp::Min),
        (false, true) => Some(PureIntOp::Max),
        _ => None,
    }
}

/// Three distinct bytes that are not equal to `needle`.
fn non_needle_bytes(needle: u8) -> [u8; 3] {
    let mut out = [0u8; 3];
    let mut value = 0u8;
    let mut filled = 0usize;
    while filled < 3 {
        if value != needle {
            out[filled] = value;
            filled += 1;
        }
        value = value.wrapping_add(1);
    }
    out
}

/// Which buffer-scan semantic the same calling shape implements.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BufferScanOp {
    Count,
    FindFirst,
    FindLast,
}

/// Resolved calling shape for the canonical buffer-scan function.
#[derive(Clone, Copy)]
struct ScanShape {
    /// Count vs find-first discriminator (from function name).
    op: BufferScanOp,
    /// Needle value used for synthesised inputs.
    needle: u8,
    /// Upper bound on buffer length, if derivable from requires.
    max_length: Option<usize>,
    /// Whether a null buffer is permitted when length is zero.
    allows_null_when_empty: bool,
}

/// Resolved calling shape for unsigned lexicographic byte comparison.
#[derive(Clone, Copy)]
struct MemCmpShape {
    /// Upper bound on buffer length, if derivable from requires.
    max_length: Option<usize>,
    /// Whether null buffers are permitted when length is zero.
    allows_null: bool,
}

/// Detect `(ptr<const u8>, ptr<const u8>, usize) -> isize` memcmp shapes.
fn memcmp_shape(contract: &CheckedContract) -> Option<MemCmpShape> {
    let lower_name = contract.name.to_ascii_lowercase();
    if !lower_name.contains("memcmp") && !lower_name.contains("bcmp") {
        return None;
    }
    if contract.parameters.len() != 3 || contract.returns.len() != 1 {
        return None;
    }

    let pointers: Vec<_> = contract
        .parameters
        .iter()
        .filter(|p| {
            matches!(
                &p.ty,
                SemType::Ptr {
                    is_const: true,
                    inner
                } if matches!(inner.as_ref(), SemType::UInt { bits: 8 })
            )
        })
        .collect();
    let lengths: Vec<_> = contract
        .parameters
        .iter()
        .filter(|p| matches!(p.ty, SemType::Usize))
        .collect();
    if pointers.len() != 2
        || pointers[0].name == pointers[1].name
        || lengths.len() != 1
        || !matches!(contract.returns[0].ty, SemType::Isize)
    {
        return None;
    }

    let length = lengths[0];
    let allows_null = pointers
        .iter()
        .all(|p| allows_null_when_empty(contract, &p.name, &length.name));
    Some(MemCmpShape {
        max_length: length_bound_from_requires(contract, &length.name),
        allows_null,
    })
}

/// Detect the `(ptr<const i64>, usize) -> i64` wrapping-sum shape.
fn i64_sum_shape(contract: &CheckedContract) -> Option<I64SumShape> {
    let mut ptr_param = None;
    let mut len_param = None;

    for p in &contract.parameters {
        match &p.ty {
            SemType::Ptr {
                is_const: true,
                inner,
            } if ptr_param.is_none() && matches!(inner.as_ref(), SemType::Int { bits: 64 }) => {
                ptr_param = Some(p);
            }
            SemType::Usize if len_param.is_none() => {
                len_param = Some(p);
            }
            _ => {}
        }
    }

    let returns_i64 = contract
        .returns
        .iter()
        .any(|r| matches!(r.ty, SemType::Int { bits: 64 }));

    // Reject buffer-scan / other shapes that also carry a pointer + length.
    let extra_params = contract.parameters.len() != 2;

    let (ptr_param, len_param) = match (ptr_param, len_param) {
        (Some(p), Some(l)) if returns_i64 && !extra_params => (p, l),
        _ => return None,
    };

    Some(I64SumShape {
        max_length: length_bound_from_requires(contract, &len_param.name),
        allows_null_when_empty: allows_null_when_empty(contract, &ptr_param.name, &len_param.name),
    })
}

/// Resolved calling shape for the canonical i64 wrapping-sum function.
#[derive(Clone, Copy)]
struct I64SumShape {
    /// Upper bound on buffer length, if derivable from requires.
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
                ptr_param = Some(p);
            }
            SemType::Usize if len_param.is_none() => {
                len_param = Some(p);
            }
            SemType::UInt { bits: 8 } if needle_param.is_none() => {
                needle_param = Some(p);
            }
            _ => {}
        }
    }

    let returns_usize = contract
        .returns
        .iter()
        .any(|r| matches!(r.ty, SemType::Usize));

    let (ptr_param, len_param, needle_param) = match (ptr_param, len_param, needle_param) {
        (Some(p), Some(l), Some(n)) if returns_usize => (p, l, n),
        _ => return None,
    };

    let op = buffer_scan_op_from_name(&contract.name)?;

    Some(ScanShape {
        op,
        needle: needle_from_requires(contract, &needle_param.name)
            .unwrap_or(DEFAULT_BUFFER_SCAN_NEEDLE),
        max_length: length_bound_from_requires(contract, &len_param.name),
        allows_null_when_empty: allows_null_when_empty(contract, &ptr_param.name, &len_param.name),
    })
}

fn buffer_scan_op_from_name(name: &str) -> Option<BufferScanOp> {
    let lower = name.to_ascii_lowercase();
    let has_count = lower.contains("count");
    let has_last = lower.contains("last") || lower.contains("rfind");
    let has_find = lower.contains("find") || lower.contains("index");
    match (has_count, has_last, has_find) {
        (true, false, false) => Some(BufferScanOp::Count),
        (false, true, _) => Some(BufferScanOp::FindLast),
        (false, false, true) => Some(BufferScanOp::FindFirst),
        _ => None,
    }
}

fn length_bound_from_requires(contract: &CheckedContract, length_name: &str) -> Option<usize> {
    let mut best: Option<usize> = None;
    for condition in &contract.requires {
        if let Some(bound) = length_bound_from_expr(&condition.expr, length_name) {
            best = Some(match best {
                Some(existing) => existing.min(bound),
                None => bound,
            });
        }
    }
    best
}

fn length_bound_from_expr(expr: &Expr, length_name: &str) -> Option<usize> {
    let Expr::Binary {
        op, left, right, ..
    } = expr
    else {
        return None;
    };

    let ident_left = ident_name(left);
    let ident_right = ident_name(right);
    let int_left = int_value(left);
    let int_right = int_value(right);

    match (*op, ident_left, ident_right, int_left, int_right) {
        (BinOp::Le, Some(name), None, None, Some(n)) if name == length_name => {
            usize::try_from(n).ok()
        }
        (BinOp::Lt, Some(name), None, None, Some(n)) if name == length_name => {
            usize::try_from(n).ok().map(|n| n.saturating_sub(1))
        }
        (BinOp::Ge, None, Some(name), Some(n), None) if name == length_name => {
            usize::try_from(n).ok()
        }
        (BinOp::Gt, None, Some(name), Some(n), None) if name == length_name => {
            usize::try_from(n).ok().map(|n| n.saturating_sub(1))
        }
        _ => None,
    }
}

fn needle_from_requires(contract: &CheckedContract, needle_name: &str) -> Option<u8> {
    for condition in &contract.requires {
        if let Some(needle) = needle_from_expr(&condition.expr, needle_name) {
            return Some(needle);
        }
    }
    None
}

fn needle_from_expr(expr: &Expr, needle_name: &str) -> Option<u8> {
    let Expr::Binary {
        op: BinOp::Eq,
        left,
        right,
        ..
    } = expr
    else {
        return None;
    };

    match (
        ident_name(left),
        ident_name(right),
        int_value(left),
        int_value(right),
    ) {
        (Some(name), None, None, Some(n)) | (None, Some(name), Some(n), None)
            if name == needle_name =>
        {
            u8::try_from(n).ok()
        }
        _ => None,
    }
}

fn allows_null_when_empty(contract: &CheckedContract, ptr_name: &str, length_name: &str) -> bool {
    let expected = format!("{ptr_name}[0..{length_name}]");
    contract.effects.iter().any(|effect| {
        effect.kind == "memory_read"
            && effect
                .region
                .as_deref()
                .is_some_and(|region| region.trim() == expected)
    })
}

fn ident_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

fn int_value(expr: &Expr) -> Option<i64> {
    match expr {
        Expr::Int { value, .. } => Some(*value),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Harness source generation
// ---------------------------------------------------------------------------

/// Generate assembler source for a harness that exercises `vectors`
/// against the routine named `routine_symbol`.
///
/// The calling shape is inferred from `vectors` (buffer-scan vs pure-int).
///
/// Supported ABIs:
/// - [`Abi::SysVAmd64`]: NASM `_start`, args in `rdi`/`rsi`/`rdx`, Linux syscalls
/// - [`Abi::WindowsX64`]: NASM `main`, args in `rcx`/`rdx`/`r8`, Win32 I/O
/// - [`Abi::Aapcs64`]: GNU as `_start`, args in `x0`/`x1`/`x2`, Linux `svc`
/// - [`Abi::Riscv`]: GNU as `_start`, args in `a0`/`a1`/`a2`, Linux `ecall`
pub fn generate_harness(
    routine_symbol: &str,
    vectors: &[TestVector],
    abi: Abi,
) -> Result<String, String> {
    let shape = detect_harness_shape(vectors)?;
    match (abi, shape) {
        (Abi::SysVAmd64, HarnessShape::BufferScan) => {
            Ok(generate_sysv_buffer_harness(routine_symbol, vectors))
        }
        (Abi::SysVAmd64, HarnessShape::MemCmp) => {
            Ok(generate_sysv_memcmp_harness(routine_symbol, vectors))
        }
        (Abi::SysVAmd64, HarnessShape::I64Sum) => {
            Ok(generate_sysv_i64_sum_harness(routine_symbol, vectors))
        }
        (Abi::SysVAmd64, HarnessShape::PureInt) => {
            Ok(generate_sysv_pure_int_harness(routine_symbol, vectors))
        }
        (Abi::WindowsX64, HarnessShape::BufferScan) => {
            Ok(generate_win64_buffer_harness(routine_symbol, vectors))
        }
        (Abi::WindowsX64, HarnessShape::MemCmp) => {
            Ok(generate_win64_memcmp_harness(routine_symbol, vectors))
        }
        (Abi::WindowsX64, HarnessShape::I64Sum) => {
            Ok(generate_win64_i64_sum_harness(routine_symbol, vectors))
        }
        (Abi::WindowsX64, HarnessShape::PureInt) => {
            Ok(generate_win64_pure_int_harness(routine_symbol, vectors))
        }
        (Abi::Aapcs64, HarnessShape::BufferScan) => {
            Ok(generate_aapcs64_buffer_harness(routine_symbol, vectors))
        }
        (Abi::Aapcs64 | Abi::Riscv, HarnessShape::MemCmp) => {
            Err("memcmp harness not yet supported on this ABI".into())
        }
        (Abi::Aapcs64, HarnessShape::I64Sum) => {
            Ok(generate_aapcs64_i64_sum_harness(routine_symbol, vectors))
        }
        (Abi::Aapcs64, HarnessShape::PureInt) => {
            Ok(generate_aapcs64_pure_int_harness(routine_symbol, vectors))
        }
        (Abi::Riscv, HarnessShape::BufferScan) => {
            Ok(generate_riscv_buffer_harness(routine_symbol, vectors))
        }
        (Abi::Riscv, HarnessShape::I64Sum) => {
            Ok(generate_riscv_i64_sum_harness(routine_symbol, vectors))
        }
        (Abi::Riscv, HarnessShape::PureInt) => {
            Ok(generate_riscv_pure_int_harness(routine_symbol, vectors))
        }
    }
}

fn emit_vector_data(out: &mut String, vectors: &[TestVector]) {
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
}

fn emit_gas_vector_data(out: &mut String, vectors: &[TestVector]) {
    out.push_str(".section .data\n");
    for (i, v) in vectors.iter().enumerate() {
        let bytes = vector_buffer_bytes(v);
        // Keep each len quad 8-byte aligned — RISC-V `ld` faults on misalignment.
        out.push_str(".align 3\n");
        let _ = writeln!(out, "vec{i}_len:\n    .quad {}", vector_length(v));
        let _ = writeln!(out, "vec{i}_needle:\n    .byte {}", vector_needle(v));
        out.push_str(".align 3\n");
        let _ = write!(out, "vec{i}_buf:\n    .byte ");
        if bytes.is_empty() {
            out.push_str("0\n");
        } else {
            out.push_str(&bytes.join(", "));
            out.push('\n');
        }
    }
}

fn emit_pure_int_vector_data(out: &mut String, vectors: &[TestVector]) {
    out.push_str("section .data\n");
    for (i, v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "vec{i}_a: dq {}", vector_int_input(v, 0));
        let _ = writeln!(out, "vec{i}_b: dq {}", vector_int_input(v, 1));
    }
}

fn emit_i64_sum_vector_data(out: &mut String, vectors: &[TestVector]) {
    out.push_str("section .data\n");
    for (i, v) in vectors.iter().enumerate() {
        let words = vector_i64_words(v);
        let _ = writeln!(out, "vec{i}_len: dq {}", vector_length(v));
        let _ = write!(out, "vec{i}_buf: dq {}", words.join(", "));
        if words.is_empty() {
            out.push_str(" 0");
        }
        out.push('\n');
    }
}

fn emit_memcmp_vector_data(out: &mut String, vectors: &[TestVector]) {
    out.push_str("section .data\n");
    for (i, v) in vectors.iter().enumerate() {
        let a = vector_memcmp_a_bytes(v);
        let b = vector_memcmp_b_bytes(v);
        let _ = writeln!(out, "vec{i}_len: dq {}", vector_memcmp_length(v));
        let _ = write!(out, "vec{i}_a: db {}", a.join(", "));
        if a.is_empty() {
            out.push_str(" 0");
        }
        out.push('\n');
        let _ = write!(out, "vec{i}_b: db {}", b.join(", "));
        if b.is_empty() {
            out.push_str(" 0");
        }
        out.push('\n');
    }
}

fn emit_gas_i64_sum_vector_data(out: &mut String, vectors: &[TestVector]) {
    out.push_str(".section .data\n");
    for (i, v) in vectors.iter().enumerate() {
        let words = vector_i64_words(v);
        out.push_str(".align 3\n");
        let _ = writeln!(out, "vec{i}_len:\n    .quad {}", vector_length(v));
        out.push_str(".align 3\n");
        let _ = write!(out, "vec{i}_buf:\n    .quad ");
        if words.is_empty() {
            out.push_str("0\n");
        } else {
            out.push_str(&words.join(", "));
            out.push('\n');
        }
    }
}

fn emit_gas_pure_int_vector_data(out: &mut String, vectors: &[TestVector]) {
    out.push_str(".section .data\n");
    for (i, v) in vectors.iter().enumerate() {
        out.push_str(".align 3\n");
        let _ = writeln!(out, "vec{i}_a:\n    .quad {}", vector_int_input(v, 0));
        out.push_str(".align 3\n");
        let _ = writeln!(out, "vec{i}_b:\n    .quad {}", vector_int_input(v, 1));
    }
}

/// Generate NASM source for a `_start` harness (System V + Linux syscalls).
fn generate_sysv_buffer_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();

    out.push_str("BITS 64\n");
    out.push_str("DEFAULT REL\n\n");

    emit_vector_data(&mut out, vectors);

    out.push_str("\nsection .bss\n");
    let _ = writeln!(out, "results: resb {}", vectors.len() * 8);

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

    out.push_str("    ; write(results, len)\n");
    out.push_str("    mov eax, 1          ; sys_write\n");
    out.push_str("    mov edi, 1          ; stdout\n");
    let _ = writeln!(out, "    lea rsi, [results]");
    let _ = writeln!(out, "    mov edx, {}", vectors.len() * 8);
    out.push_str("    syscall\n");
    out.push_str("    mov eax, 60         ; sys_exit\n");
    out.push_str("    xor edi, edi\n");
    out.push_str("    syscall\n");

    out
}

/// Generate NASM source for a SysV dual-buffer memcmp `_start` harness.
fn generate_sysv_memcmp_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();

    out.push_str("BITS 64\n");
    out.push_str("DEFAULT REL\n\n");
    emit_memcmp_vector_data(&mut out, vectors);

    out.push_str("\nsection .bss\n");
    let _ = writeln!(out, "results: resb {}", vectors.len() * 8);
    out.push_str("\nsection .text\n");
    let _ = writeln!(out, "extern {routine_symbol}");
    out.push_str("global _start\n");
    out.push_str("_start:\n");

    for (i, v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    ; vector {i}: {}", v.name);
        if v.inputs.first().is_some_and(serde_json::Value::is_null) {
            out.push_str("    xor edi, edi\n");
        } else {
            let _ = writeln!(out, "    lea rdi, [vec{i}_a]");
        }
        if v.inputs.get(1).is_some_and(serde_json::Value::is_null) {
            out.push_str("    xor esi, esi\n");
        } else {
            let _ = writeln!(out, "    lea rsi, [vec{i}_b]");
        }
        let _ = writeln!(out, "    mov rdx, [vec{i}_len]");
        let _ = writeln!(out, "    call {routine_symbol}");
        let _ = writeln!(out, "    mov [results + {i}*8], rax");
    }

    out.push_str("    ; write(results, len)\n");
    out.push_str("    mov eax, 1          ; sys_write\n");
    out.push_str("    mov edi, 1          ; stdout\n");
    out.push_str("    lea rsi, [results]\n");
    let _ = writeln!(out, "    mov edx, {}", vectors.len() * 8);
    out.push_str("    syscall\n");
    out.push_str("    mov eax, 60         ; sys_exit\n");
    out.push_str("    xor edi, edi\n");
    out.push_str("    syscall\n");

    out
}

/// Generate NASM source for a SysV pure-integer `_start` harness.
fn generate_sysv_pure_int_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();

    out.push_str("BITS 64\n");
    out.push_str("DEFAULT REL\n\n");

    emit_pure_int_vector_data(&mut out, vectors);

    out.push_str("\nsection .bss\n");
    let _ = writeln!(out, "results: resb {}", vectors.len() * 8);

    out.push_str("\nsection .text\n");
    let _ = writeln!(out, "extern {routine_symbol}");
    out.push_str("global _start\n");
    out.push_str("_start:\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    ; vector {i}: {}", vectors[i].name);
        let _ = writeln!(out, "    mov rdi, [vec{i}_a]");
        let _ = writeln!(out, "    mov rsi, [vec{i}_b]");
        let _ = writeln!(out, "    call {routine_symbol}");
        let _ = writeln!(out, "    mov [results + {i}*8], rax");
    }

    out.push_str("    ; write(results, len)\n");
    out.push_str("    mov eax, 1          ; sys_write\n");
    out.push_str("    mov edi, 1          ; stdout\n");
    let _ = writeln!(out, "    lea rsi, [results]");
    let _ = writeln!(out, "    mov edx, {}", vectors.len() * 8);
    out.push_str("    syscall\n");
    out.push_str("    mov eax, 60         ; sys_exit\n");
    out.push_str("    xor edi, edi\n");
    out.push_str("    syscall\n");

    out
}

/// Generate NASM source for a SysV i64 wrapping-sum `_start` harness.
fn generate_sysv_i64_sum_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();

    out.push_str("BITS 64\n");
    out.push_str("DEFAULT REL\n\n");

    emit_i64_sum_vector_data(&mut out, vectors);

    out.push_str("\nsection .bss\n");
    let _ = writeln!(out, "results: resb {}", vectors.len() * 8);

    out.push_str("\nsection .text\n");
    let _ = writeln!(out, "extern {routine_symbol}");
    out.push_str("global _start\n");
    out.push_str("_start:\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    ; vector {i}: {}", vectors[i].name);
        let _ = writeln!(out, "    lea rdi, [vec{i}_buf]");
        let _ = writeln!(out, "    mov rsi, [vec{i}_len]");
        let _ = writeln!(out, "    call {routine_symbol}");
        let _ = writeln!(out, "    mov [results + {i}*8], rax");
    }

    out.push_str("    ; write(results, len)\n");
    out.push_str("    mov eax, 1          ; sys_write\n");
    out.push_str("    mov edi, 1          ; stdout\n");
    let _ = writeln!(out, "    lea rsi, [results]");
    let _ = writeln!(out, "    mov edx, {}", vectors.len() * 8);
    out.push_str("    syscall\n");
    out.push_str("    mov eax, 60         ; sys_exit\n");
    out.push_str("    xor edi, edi\n");
    out.push_str("    syscall\n");

    out
}

/// Generate NASM source for a Win64 `main` harness (kernel32 WriteFile/ExitProcess).
fn generate_win64_buffer_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();

    out.push_str("BITS 64\n");
    out.push_str("DEFAULT REL\n\n");
    out.push_str("EXTERN GetStdHandle\n");
    out.push_str("EXTERN WriteFile\n");
    out.push_str("EXTERN ExitProcess\n\n");

    emit_vector_data(&mut out, vectors);

    out.push_str("\nsection .bss\n");
    let _ = writeln!(out, "results: resb {}", vectors.len() * 8);
    out.push_str("written: resq 1\n");

    out.push_str("\nsection .text\n");
    let _ = writeln!(out, "extern {routine_symbol}");
    out.push_str("global main\n");
    out.push_str("main:\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    ; vector {i}: {}", vectors[i].name);
        out.push_str("    sub rsp, 40\n");
        let _ = writeln!(out, "    lea rcx, [vec{i}_buf]");
        let _ = writeln!(out, "    mov rdx, [vec{i}_len]");
        let _ = writeln!(out, "    movzx r8d, byte [vec{i}_needle]");
        let _ = writeln!(out, "    call {routine_symbol}");
        out.push_str("    add rsp, 40\n");
        let _ = writeln!(out, "    mov [results + {i}*8], rax");
    }

    out.push_str("    ; WriteFile(stdout, results, len, &written, NULL)\n");
    out.push_str("    sub rsp, 40\n");
    out.push_str("    mov ecx, -11        ; STD_OUTPUT_HANDLE\n");
    out.push_str("    call GetStdHandle\n");
    out.push_str("    mov rcx, rax\n");
    out.push_str("    lea rdx, [results]\n");
    let _ = writeln!(out, "    mov r8d, {}", vectors.len() * 8);
    out.push_str("    lea r9, [written]\n");
    out.push_str("    mov qword [rsp + 32], 0\n");
    out.push_str("    call WriteFile\n");
    out.push_str("    add rsp, 40\n");

    out.push_str("    sub rsp, 40\n");
    out.push_str("    xor ecx, ecx\n");
    out.push_str("    call ExitProcess\n");

    out
}

/// Generate NASM source for a Win64 dual-buffer memcmp `main` harness.
fn generate_win64_memcmp_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();

    out.push_str("BITS 64\n");
    out.push_str("DEFAULT REL\n\n");
    out.push_str("EXTERN GetStdHandle\n");
    out.push_str("EXTERN WriteFile\n");
    out.push_str("EXTERN ExitProcess\n\n");
    emit_memcmp_vector_data(&mut out, vectors);

    out.push_str("\nsection .bss\n");
    let _ = writeln!(out, "results: resb {}", vectors.len() * 8);
    out.push_str("written: resq 1\n");
    out.push_str("\nsection .text\n");
    let _ = writeln!(out, "extern {routine_symbol}");
    out.push_str("global main\n");
    out.push_str("main:\n");

    for (i, v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    ; vector {i}: {}", v.name);
        out.push_str("    sub rsp, 40\n");
        if v.inputs.first().is_some_and(serde_json::Value::is_null) {
            out.push_str("    xor ecx, ecx\n");
        } else {
            let _ = writeln!(out, "    lea rcx, [vec{i}_a]");
        }
        if v.inputs.get(1).is_some_and(serde_json::Value::is_null) {
            out.push_str("    xor edx, edx\n");
        } else {
            let _ = writeln!(out, "    lea rdx, [vec{i}_b]");
        }
        let _ = writeln!(out, "    mov r8, [vec{i}_len]");
        let _ = writeln!(out, "    call {routine_symbol}");
        out.push_str("    add rsp, 40\n");
        let _ = writeln!(out, "    mov [results + {i}*8], rax");
    }

    out.push_str("    ; WriteFile(stdout, results, len, &written, NULL)\n");
    out.push_str("    sub rsp, 40\n");
    out.push_str("    mov ecx, -11        ; STD_OUTPUT_HANDLE\n");
    out.push_str("    call GetStdHandle\n");
    out.push_str("    mov rcx, rax\n");
    out.push_str("    lea rdx, [results]\n");
    let _ = writeln!(out, "    mov r8d, {}", vectors.len() * 8);
    out.push_str("    lea r9, [written]\n");
    out.push_str("    mov qword [rsp + 32], 0\n");
    out.push_str("    call WriteFile\n");
    out.push_str("    add rsp, 40\n");
    out.push_str("    sub rsp, 40\n");
    out.push_str("    xor ecx, ecx\n");
    out.push_str("    call ExitProcess\n");

    out
}

/// Generate NASM source for a Win64 pure-integer `main` harness.
fn generate_win64_pure_int_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();

    out.push_str("BITS 64\n");
    out.push_str("DEFAULT REL\n\n");
    out.push_str("EXTERN GetStdHandle\n");
    out.push_str("EXTERN WriteFile\n");
    out.push_str("EXTERN ExitProcess\n\n");

    emit_pure_int_vector_data(&mut out, vectors);

    out.push_str("\nsection .bss\n");
    let _ = writeln!(out, "results: resb {}", vectors.len() * 8);
    out.push_str("written: resq 1\n");

    out.push_str("\nsection .text\n");
    let _ = writeln!(out, "extern {routine_symbol}");
    out.push_str("global main\n");
    out.push_str("main:\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    ; vector {i}: {}", vectors[i].name);
        out.push_str("    sub rsp, 40\n");
        let _ = writeln!(out, "    mov rcx, [vec{i}_a]");
        let _ = writeln!(out, "    mov rdx, [vec{i}_b]");
        let _ = writeln!(out, "    call {routine_symbol}");
        out.push_str("    add rsp, 40\n");
        let _ = writeln!(out, "    mov [results + {i}*8], rax");
    }

    out.push_str("    ; WriteFile(stdout, results, len, &written, NULL)\n");
    out.push_str("    sub rsp, 40\n");
    out.push_str("    mov ecx, -11        ; STD_OUTPUT_HANDLE\n");
    out.push_str("    call GetStdHandle\n");
    out.push_str("    mov rcx, rax\n");
    out.push_str("    lea rdx, [results]\n");
    let _ = writeln!(out, "    mov r8d, {}", vectors.len() * 8);
    out.push_str("    lea r9, [written]\n");
    out.push_str("    mov qword [rsp + 32], 0\n");
    out.push_str("    call WriteFile\n");
    out.push_str("    add rsp, 40\n");

    out.push_str("    sub rsp, 40\n");
    out.push_str("    xor ecx, ecx\n");
    out.push_str("    call ExitProcess\n");

    out
}

/// Generate NASM source for a Win64 i64 wrapping-sum `main` harness.
fn generate_win64_i64_sum_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();

    out.push_str("BITS 64\n");
    out.push_str("DEFAULT REL\n\n");
    out.push_str("EXTERN GetStdHandle\n");
    out.push_str("EXTERN WriteFile\n");
    out.push_str("EXTERN ExitProcess\n\n");

    emit_i64_sum_vector_data(&mut out, vectors);

    out.push_str("\nsection .bss\n");
    let _ = writeln!(out, "results: resb {}", vectors.len() * 8);
    out.push_str("written: resq 1\n");

    out.push_str("\nsection .text\n");
    let _ = writeln!(out, "extern {routine_symbol}");
    out.push_str("global main\n");
    out.push_str("main:\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    ; vector {i}: {}", vectors[i].name);
        out.push_str("    sub rsp, 40\n");
        let _ = writeln!(out, "    lea rcx, [vec{i}_buf]");
        let _ = writeln!(out, "    mov rdx, [vec{i}_len]");
        let _ = writeln!(out, "    call {routine_symbol}");
        out.push_str("    add rsp, 40\n");
        let _ = writeln!(out, "    mov [results + {i}*8], rax");
    }

    out.push_str("    ; WriteFile(stdout, results, len, &written, NULL)\n");
    out.push_str("    sub rsp, 40\n");
    out.push_str("    mov ecx, -11        ; STD_OUTPUT_HANDLE\n");
    out.push_str("    call GetStdHandle\n");
    out.push_str("    mov rcx, rax\n");
    out.push_str("    lea rdx, [results]\n");
    let _ = writeln!(out, "    mov r8d, {}", vectors.len() * 8);
    out.push_str("    lea r9, [written]\n");
    out.push_str("    mov qword [rsp + 32], 0\n");
    out.push_str("    call WriteFile\n");
    out.push_str("    add rsp, 40\n");

    out.push_str("    sub rsp, 40\n");
    out.push_str("    xor ecx, ecx\n");
    out.push_str("    call ExitProcess\n");

    out
}

/// Generate GNU as source for an AArch64 `_start` harness (Linux syscalls).
fn generate_aapcs64_buffer_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();
    let results_len = vectors.len() * 8;

    emit_gas_vector_data(&mut out, vectors);

    out.push_str("\n.section .bss\n");
    out.push_str(".align 3\n");
    out.push_str("results:\n");
    let _ = writeln!(out, "    .space {results_len}");

    out.push_str("\n.section .text\n");
    out.push_str(".global _start\n");
    out.push_str("_start:\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    // vector {i}: {}", vectors[i].name);
        let _ = writeln!(out, "    ldr x0, =vec{i}_buf");
        let _ = writeln!(out, "    ldr x1, =vec{i}_len");
        out.push_str("    ldr x1, [x1]\n");
        let _ = writeln!(out, "    ldr x2, =vec{i}_needle");
        out.push_str("    ldrb w2, [x2]\n");
        let _ = writeln!(out, "    bl {routine_symbol}");
        out.push_str("    ldr x3, =results\n");
        let _ = writeln!(out, "    str x0, [x3, #{offset}]", offset = i * 8);
    }

    out.push_str("    // write(1, results, len)\n");
    out.push_str("    mov x0, #1\n");
    out.push_str("    ldr x1, =results\n");
    let _ = writeln!(out, "    mov x2, #{results_len}");
    out.push_str("    mov x8, #64\n");
    out.push_str("    svc #0\n");
    out.push_str("    // exit(0)\n");
    out.push_str("    mov x0, #0\n");
    out.push_str("    mov x8, #93\n");
    out.push_str("    svc #0\n");

    out
}

/// Generate GNU as source for an AArch64 pure-integer `_start` harness.
fn generate_aapcs64_pure_int_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();
    let results_len = vectors.len() * 8;

    emit_gas_pure_int_vector_data(&mut out, vectors);

    out.push_str("\n.section .bss\n");
    out.push_str(".align 3\n");
    out.push_str("results:\n");
    let _ = writeln!(out, "    .space {results_len}");

    out.push_str("\n.section .text\n");
    out.push_str(".global _start\n");
    out.push_str("_start:\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    // vector {i}: {}", vectors[i].name);
        let _ = writeln!(out, "    ldr x0, =vec{i}_a");
        out.push_str("    ldr x0, [x0]\n");
        let _ = writeln!(out, "    ldr x1, =vec{i}_b");
        out.push_str("    ldr x1, [x1]\n");
        let _ = writeln!(out, "    bl {routine_symbol}");
        out.push_str("    ldr x2, =results\n");
        let _ = writeln!(out, "    str x0, [x2, #{offset}]", offset = i * 8);
    }

    out.push_str("    // write(1, results, len)\n");
    out.push_str("    mov x0, #1\n");
    out.push_str("    ldr x1, =results\n");
    let _ = writeln!(out, "    mov x2, #{results_len}");
    out.push_str("    mov x8, #64\n");
    out.push_str("    svc #0\n");
    out.push_str("    // exit(0)\n");
    out.push_str("    mov x0, #0\n");
    out.push_str("    mov x8, #93\n");
    out.push_str("    svc #0\n");

    out
}

/// Generate GNU as source for an AArch64 i64 wrapping-sum `_start` harness.
fn generate_aapcs64_i64_sum_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();
    let results_len = vectors.len() * 8;

    emit_gas_i64_sum_vector_data(&mut out, vectors);

    out.push_str("\n.section .bss\n");
    out.push_str(".align 3\n");
    out.push_str("results:\n");
    let _ = writeln!(out, "    .space {results_len}");

    out.push_str("\n.section .text\n");
    out.push_str(".global _start\n");
    out.push_str("_start:\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    // vector {i}: {}", vectors[i].name);
        let _ = writeln!(out, "    ldr x0, =vec{i}_buf");
        let _ = writeln!(out, "    ldr x1, =vec{i}_len");
        out.push_str("    ldr x1, [x1]\n");
        let _ = writeln!(out, "    bl {routine_symbol}");
        out.push_str("    ldr x2, =results\n");
        let _ = writeln!(out, "    str x0, [x2, #{offset}]", offset = i * 8);
    }

    out.push_str("    // write(1, results, len)\n");
    out.push_str("    mov x0, #1\n");
    out.push_str("    ldr x1, =results\n");
    let _ = writeln!(out, "    mov x2, #{results_len}");
    out.push_str("    mov x8, #64\n");
    out.push_str("    svc #0\n");
    out.push_str("    // exit(0)\n");
    out.push_str("    mov x0, #0\n");
    out.push_str("    mov x8, #93\n");
    out.push_str("    svc #0\n");

    out
}

/// Generate GNU as source for a RISC-V `_start` harness (Linux syscalls).
fn generate_riscv_buffer_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();
    let results_len = vectors.len() * 8;

    out.push_str(".option norelax\n");
    emit_gas_vector_data(&mut out, vectors);

    out.push_str("\n.section .bss\n");
    out.push_str(".align 4\n");
    out.push_str("results:\n");
    let _ = writeln!(out, "    .space {results_len}");
    out.push_str(".align 4\n");
    out.push_str("    .space 16384\n");
    out.push_str("__stack_top:\n");

    out.push_str("\n.section .text\n");
    out.push_str(".global _start\n");
    out.push_str("_start:\n");
    out.push_str("    la sp, __stack_top\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    # vector {i}: {}", vectors[i].name);
        let _ = writeln!(out, "    la a0, vec{i}_buf");
        let _ = writeln!(out, "    la t0, vec{i}_len");
        out.push_str("    ld a1, 0(t0)\n");
        let _ = writeln!(out, "    la t0, vec{i}_needle");
        out.push_str("    lbu a2, 0(t0)\n");
        let _ = writeln!(out, "    jal {routine_symbol}");
        out.push_str("    la t0, results\n");
        let _ = writeln!(out, "    sd a0, {offset}(t0)", offset = i * 8);
    }

    out.push_str("    # write(1, results, len)\n");
    out.push_str("    li a0, 1\n");
    out.push_str("    la a1, results\n");
    let _ = writeln!(out, "    li a2, {results_len}");
    out.push_str("    li a7, 64\n");
    out.push_str("    ecall\n");
    out.push_str("    # exit(0)\n");
    out.push_str("    li a0, 0\n");
    out.push_str("    li a7, 93\n");
    out.push_str("    ecall\n");

    out
}

/// Generate GNU as source for a RISC-V pure-integer `_start` harness.
fn generate_riscv_pure_int_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();
    let results_len = vectors.len() * 8;

    out.push_str(".option norelax\n");
    emit_gas_pure_int_vector_data(&mut out, vectors);

    out.push_str("\n.section .bss\n");
    out.push_str(".align 4\n");
    out.push_str("results:\n");
    let _ = writeln!(out, "    .space {results_len}");
    out.push_str(".align 4\n");
    out.push_str("    .space 16384\n");
    out.push_str("__stack_top:\n");

    out.push_str("\n.section .text\n");
    out.push_str(".global _start\n");
    out.push_str("_start:\n");
    out.push_str("    la sp, __stack_top\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    # vector {i}: {}", vectors[i].name);
        let _ = writeln!(out, "    la a0, vec{i}_a");
        out.push_str("    ld a0, 0(a0)\n");
        let _ = writeln!(out, "    la a1, vec{i}_b");
        out.push_str("    ld a1, 0(a1)\n");
        let _ = writeln!(out, "    jal {routine_symbol}");
        out.push_str("    la t0, results\n");
        let _ = writeln!(out, "    sd a0, {offset}(t0)", offset = i * 8);
    }

    out.push_str("    # write(1, results, len)\n");
    out.push_str("    li a0, 1\n");
    out.push_str("    la a1, results\n");
    let _ = writeln!(out, "    li a2, {results_len}");
    out.push_str("    li a7, 64\n");
    out.push_str("    ecall\n");
    out.push_str("    # exit(0)\n");
    out.push_str("    li a0, 0\n");
    out.push_str("    li a7, 93\n");
    out.push_str("    ecall\n");

    out
}

/// Generate GNU as source for a RISC-V i64 wrapping-sum `_start` harness.
fn generate_riscv_i64_sum_harness(routine_symbol: &str, vectors: &[TestVector]) -> String {
    let mut out = String::new();
    let results_len = vectors.len() * 8;

    out.push_str(".option norelax\n");
    emit_gas_i64_sum_vector_data(&mut out, vectors);

    out.push_str("\n.section .bss\n");
    out.push_str(".align 4\n");
    out.push_str("results:\n");
    let _ = writeln!(out, "    .space {results_len}");
    out.push_str(".align 4\n");
    out.push_str("    .space 16384\n");
    out.push_str("__stack_top:\n");

    out.push_str("\n.section .text\n");
    out.push_str(".global _start\n");
    out.push_str("_start:\n");
    out.push_str("    la sp, __stack_top\n");

    for (i, _v) in vectors.iter().enumerate() {
        let _ = writeln!(out, "    # vector {i}: {}", vectors[i].name);
        let _ = writeln!(out, "    la a0, vec{i}_buf");
        let _ = writeln!(out, "    la t0, vec{i}_len");
        out.push_str("    ld a1, 0(t0)\n");
        let _ = writeln!(out, "    jal {routine_symbol}");
        out.push_str("    la t0, results\n");
        let _ = writeln!(out, "    sd a0, {offset}(t0)", offset = i * 8);
    }

    out.push_str("    # write(1, results, len)\n");
    out.push_str("    li a0, 1\n");
    out.push_str("    la a1, results\n");
    let _ = writeln!(out, "    li a2, {results_len}");
    out.push_str("    li a7, 64\n");
    out.push_str("    ecall\n");
    out.push_str("    # exit(0)\n");
    out.push_str("    li a0, 0\n");
    out.push_str("    li a7, 93\n");
    out.push_str("    ecall\n");

    out
}

/// Extract signed/unsigned JSON numbers as decimal strings for `dq` / `.quad`.
fn vector_i64_words(v: &TestVector) -> Vec<String> {
    match v.inputs.first() {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(|x| {
                if let Some(i) = x.as_i64() {
                    i.to_string()
                } else if let Some(u) = x.as_u64() {
                    u.to_string()
                } else {
                    "0".into()
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Map a JSON expected value to the raw 8-byte LE bit pattern returned by harnesses.
#[allow(clippy::cast_sign_loss)] // intentional two's-complement bit pattern
fn expected_bits(value: &serde_json::Value) -> u64 {
    if let Some(u) = value.as_u64() {
        return u;
    }
    if let Some(i) = value.as_i64() {
        return i as u64;
    }
    u64::MAX
}

/// Format a harness result word for reports, preferring signed display when expected was signed.
#[allow(clippy::cast_possible_wrap)] // reinterpret harness u64 bits as i64 when expected was signed
fn format_observed(bits: u64, expected: &serde_json::Value) -> String {
    if expected.as_i64().is_some() && expected.as_u64().is_none() {
        return (bits as i64).to_string();
    }
    bits.to_string()
}

fn format_expected(expected: &serde_json::Value) -> String {
    if let Some(i) = expected.as_i64() {
        if expected.as_u64().is_none() {
            return i.to_string();
        }
    }
    if let Some(u) = expected.as_u64() {
        return u.to_string();
    }
    expected.to_string()
}

/// Extract a numeric input for a pure-integer vector.
fn vector_int_input(v: &TestVector, index: usize) -> u64 {
    v.inputs
        .get(index)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
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

fn vector_memcmp_buffer_bytes(v: &TestVector, index: usize) -> Vec<String> {
    match v.inputs.get(index) {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(|x| x.as_u64().unwrap_or(0).min(255).to_string())
            .collect(),
        _ => Vec::new(),
    }
}

/// Extract memcmp buffer `a` (first input).
fn vector_memcmp_a_bytes(v: &TestVector) -> Vec<String> {
    vector_memcmp_buffer_bytes(v, 0)
}

/// Extract memcmp buffer `b` (second input).
fn vector_memcmp_b_bytes(v: &TestVector) -> Vec<String> {
    vector_memcmp_buffer_bytes(v, 1)
}

/// Extract memcmp length (third input).
fn vector_memcmp_length(v: &TestVector) -> u64 {
    v.inputs
        .get(2)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0)
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
    let mut observed: Vec<Option<u64>> = Vec::with_capacity(vectors.len());
    for i in 0..vectors.len() {
        let base = i * 8;
        let word = if stdout.len() >= base + 8 {
            let mut b = [0u8; 8];
            b.copy_from_slice(&stdout[base..base + 8]);
            Some(u64::from_le_bytes(b))
        } else {
            None
        };
        observed.push(word);
    }

    let cases: Vec<VectorResult> = vectors
        .iter()
        .zip(observed)
        .map(|(v, got)| {
            let expected = expected_bits(&v.expected);
            let (passed, observed) = match got {
                Some(bits) => (bits == expected, format_observed(bits, &v.expected)),
                None => (false, "<no output>".into()),
            };
            VectorResult {
                name: v.name.clone(),
                passed,
                expected: format_expected(&v.expected),
                observed,
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
    use semasm_contract::check_str;

    fn count_byte_shape() -> CheckedContract {
        sample_check()
    }

    fn check_contract(toml: &str) -> CheckedContract {
        let result = check_str(toml);
        assert!(
            result.ok(),
            "contract should validate: {:?}",
            result.diagnostics
        );
        result.contract.expect("ok implies Some")
    }

    fn buffer_scan_toml(extra_requires: &str, effects: &str, constraints: &str) -> String {
        format!(
            r#"
contract_version = "0.1"

[function]
name = "count_byte"
summary = "Count occurrences of a byte in a buffer"

[[function.parameters]]
name = "buffer"
type = "ptr<const u8>"
role = "input"

[[function.parameters]]
name = "length"
type = "usize"
role = "input"

[[function.parameters]]
name = "needle"
type = "u8"
role = "input"

[[function.returns]]
name = "count"
type = "usize"

{extra_requires}

[[function.ensures]]
expression = "count <= length"

{effects}

{constraints}
"#
        )
    }

    fn vector_needle(v: &TestVector) -> u64 {
        v.inputs[2].as_u64().expect("needle input")
    }

    fn min_usize_contract() -> CheckedContract {
        let toml = include_str!("../../../fixtures/contracts/min_usize.sem.toml");
        check_contract(toml)
    }

    fn max_usize_contract() -> CheckedContract {
        let toml = include_str!("../../../fixtures/contracts/max_usize.sem.toml");
        check_contract(toml)
    }

    fn sum_i64_contract() -> CheckedContract {
        let toml = include_str!("../../../fixtures/contracts/sum_i64.sem.toml");
        check_contract(toml)
    }

    fn memcmp_contract() -> CheckedContract {
        let toml = include_str!("../../../fixtures/contracts/memcmp.sem.toml");
        check_contract(toml)
    }

    #[test]
    fn memcmp_recognizes_oracle() {
        let c = memcmp_contract();
        let oracle = recognize_behavior_oracle(&c).expect("memcmp shape");
        assert_eq!(oracle.id, ORACLE_BUFFER_MEMCMP_I8);
        assert_eq!(oracle.version, ORACLE_BUFFER_MEMCMP_I8_VERSION);
        assert_eq!(oracle.version, 1);
        assert!(oracle.claim.contains("-1, 0, or 1"));
        assert!(oracle.claim.contains("unsigned lexicographic"));
        assert!(is_read_only_buffer_scan(&c));
    }

    #[test]
    fn memcmp_synthesizes_fail_closed_vectors() {
        let vectors = synthesize_vectors(&memcmp_contract());
        assert_eq!(vectors.len(), 7);
        let expected: Vec<i64> = vectors
            .iter()
            .map(|v| v.expected.as_i64().expect("signed memcmp result"))
            .collect();
        assert!(expected.contains(&-1));
        assert!(expected.contains(&0));
        assert!(expected.contains(&1));
        assert!(expected.iter().all(|value| (-1..=1).contains(value)));
        assert_eq!(vectors[0].name, "empty buffers");
        assert_eq!(vectors[0].expected, serde_json::json!(0i64));
        assert!(vectors
            .iter()
            .any(|v| v.name == "equal prefix then difference"));
        assert!(vectors
            .iter()
            .any(|v| v.name == "maximum configured fixture length"));
        assert!(vectors
            .iter()
            .any(|v| v.name == "null buffers with zero length"));
    }

    #[test]
    fn memcmp_name_discriminator_is_fail_closed() {
        let source = include_str!("../../../fixtures/contracts/memcmp.sem.toml");
        let unnamed = check_contract(&source.replace("name = \"memcmp\"", "name = \"compare\""));
        assert!(memcmp_shape(&unnamed).is_none());
        assert!(recognize_behavior_oracle(&unnamed).is_none());
        assert!(synthesize_vectors(&unnamed).is_empty());

        let bcmp = check_contract(&source.replace("name = \"memcmp\"", "name = \"secure_bcmp\""));
        assert!(memcmp_shape(&bcmp).is_some());
    }

    #[test]
    fn memcmp_vectors_validate_and_detect_distinct_shape() {
        let c = memcmp_contract();
        let vectors = synthesize_vectors(&c);
        assert_eq!(
            detect_harness_shape(&vectors).expect("shape"),
            HarnessShape::MemCmp
        );
        validate_vectors_match_oracle(&c, &vectors).expect("memcmp vectors match oracle");
    }

    #[test]
    fn memcmp_sysv_harness_loads_both_buffers() {
        let vectors = synthesize_vectors(&memcmp_contract());
        let src = generate_harness("memcmp", &vectors, Abi::SysVAmd64).expect("sysv harness");
        assert!(src.contains("lea rdi, [vec0_a]"));
        assert!(src.contains("lea rsi, [vec0_b]"));
        assert!(src.contains("mov rdx, [vec0_len]"));
        assert!(src.contains("vec0_a: db"));
        assert!(src.contains("vec0_b: db"));
    }

    #[test]
    fn synthesizes_seven_i64_sum_vectors() {
        let c = sum_i64_contract();
        let v = synthesize_vectors(&c);
        assert_eq!(v.len(), 7);
        assert_eq!(v[0].name, "empty");
        assert_eq!(v[0].expected, serde_json::json!(0i64));
        assert_eq!(v[1].name, "positive");
        assert_eq!(v[1].expected, serde_json::json!(10i64));
        assert_eq!(v[2].name, "mixed");
        assert_eq!(v[2].expected, serde_json::json!(7i64));
        assert_eq!(v[4].expected, serde_json::json!(i64::MIN));
        let oracle = recognize_behavior_oracle(&c).expect("sum_i64 shape");
        assert_eq!(oracle.id, ORACLE_BUFFER_WRAPPING_SUM_I64);
        assert_eq!(oracle.version, ORACLE_BUFFER_WRAPPING_SUM_I64_VERSION);
        assert_eq!(ORACLE_BUFFER_WRAPPING_SUM_I64_VERSION, 2);
        assert!(oracle.claim.contains("wrapping sum"));
        assert!(is_read_only_buffer_scan(&c));
        let built = build_behavior_oracle(oracle, &c, "sum_i64.sem.toml", b"contract", &v, None);
        assert!(built.contract_ensures.iter().any(|e| e == "true"));
        assert_eq!(
            built.proof_basis,
            crate::verify::ProofBasis::OracleAndVectors
        );
        let src = generate_harness("sum_i64", &v, Abi::SysVAmd64).expect("sysv harness");
        assert!(src.contains("lea rdi, [vec0_buf]"));
        assert!(src.contains("mov rsi, [vec0_len]"));
        assert!(!src.contains("needle"));
    }

    #[test]
    fn validate_vectors_match_oracle_accepts_sum_i64_shape() {
        let c = sum_i64_contract();
        let v = synthesize_vectors(&c);
        validate_vectors_match_oracle(&c, &v).expect("matching shape");
    }

    #[test]
    fn validate_vectors_match_oracle_rejects_mismatched_shape() {
        let c = sum_i64_contract();
        let foreign = synthesize_vectors(&count_byte_shape());
        let err = validate_vectors_match_oracle(&c, &foreign).expect_err("mismatch");
        assert!(err.contains("expects"));
        assert!(err.contains("I64Sum") || err.contains("wrapping_sum"));
    }

    #[test]
    fn pure_int_oracle_claim_names_min() {
        let c = min_usize_contract();
        let oracle = recognize_behavior_oracle(&c).expect("pure-int shape");
        assert_eq!(oracle.id, ORACLE_PURE_INT_BINARY_USIZE);
        assert!(
            oracle.claim.contains("min(a, b)"),
            "claim must name the operation, got {:?}",
            oracle.claim
        );
        assert!(
            !oracle.claim.contains("max(a, b)"),
            "min claim must not name max, got {:?}",
            oracle.claim
        );
        assert!(
            is_read_only_buffer_scan(&c),
            "pure-int without memory_write must reject stores"
        );
    }

    #[test]
    fn pure_int_oracle_claim_names_max() {
        let c = max_usize_contract();
        let oracle = recognize_behavior_oracle(&c).expect("pure-int max shape");
        assert_eq!(oracle.id, ORACLE_PURE_INT_BINARY_USIZE);
        assert!(
            oracle.claim.contains("max(a, b)"),
            "claim must name the operation, got {:?}",
            oracle.claim
        );
        assert!(
            !oracle.claim.contains("min(a, b)"),
            "max claim must not name min, got {:?}",
            oracle.claim
        );
        assert!(
            is_read_only_buffer_scan(&c),
            "pure-int without memory_write must reject stores"
        );
    }

    #[test]
    fn synthesizes_six_pure_int_vectors() {
        let c = min_usize_contract();
        let v = synthesize_vectors(&c);
        assert_eq!(v.len(), 6, "expected 6 pure-int cases, got {}", v.len());
        assert!(v.iter().all(|case| case.inputs.len() == 2));
        assert!(v.iter().all(|case| {
            let a = case.inputs[0].as_u64().unwrap();
            let b = case.inputs[1].as_u64().unwrap();
            case.expected.as_u64() == Some(a.min(b))
        }));
        let names: Vec<&str> = v.iter().map(|x| x.name.as_str()).collect();
        assert!(names.contains(&"both zero"));
        assert!(names.contains(&"a smaller"));
        assert!(names.contains(&"b smaller"));
        assert!(names.contains(&"equal"));
        assert!(names.contains(&"zero and large"));
        assert!(names.contains(&"wide spread"));
    }

    #[test]
    fn synthesizes_six_pure_int_max_vectors() {
        let c = max_usize_contract();
        let v = synthesize_vectors(&c);
        assert_eq!(v.len(), 6, "expected 6 pure-int max cases, got {}", v.len());
        assert!(v.iter().all(|case| {
            let a = case.inputs[0].as_u64().unwrap();
            let b = case.inputs[1].as_u64().unwrap();
            case.expected.as_u64() == Some(a.max(b))
        }));
        assert_ne!(
            v.iter()
                .find(|case| case.name == "a smaller")
                .unwrap()
                .expected,
            synthesize_vectors(&min_usize_contract())
                .iter()
                .find(|case| case.name == "a smaller")
                .unwrap()
                .expected
        );
    }

    #[test]
    fn pure_int_ambiguous_name_is_fail_closed() {
        let toml = r#"
contract_version = "0.1"

[function]
name = "binary_usize"
summary = "Ambiguous pure-int without min/max discriminator"

[[function.parameters]]
name = "a"
type = "usize"
role = "input"

[[function.parameters]]
name = "b"
type = "usize"
role = "input"

[[function.returns]]
name = "result"
type = "usize"

[[function.ensures]]
expression = "true"

[function.constraints]
no_heap = true
no_recursion = true
bounded_stack_bytes = 64
"#;
        let c = check_contract(toml);
        assert!(
            recognize_behavior_oracle(&c).is_none(),
            "ambiguous pure-int must not claim a min/max oracle"
        );
        assert!(synthesize_vectors(&c).is_empty());
    }

    #[test]
    fn pure_int_sysv_harness_loads_abi_registers() {
        let c = min_usize_contract();
        let v = synthesize_vectors(&c);
        let src = generate_harness("min_usize", &v, Abi::SysVAmd64).unwrap();
        assert!(src.contains("extern min_usize"));
        assert!(src.contains("global _start"));
        assert!(src.contains("call min_usize"));
        assert!(src.contains("vec0_a"));
        assert!(src.contains("vec0_b"));
        assert!(src.contains("mov rdi, [vec0_a]"));
        assert!(src.contains("mov rsi, [vec0_b]"));
        assert!(!src.contains("vec0_buf"));
    }

    #[test]
    fn pure_int_win64_harness_loads_abi_registers() {
        let c = min_usize_contract();
        let v = synthesize_vectors(&c);
        let src = generate_harness("min_usize", &v, Abi::WindowsX64).unwrap();
        assert!(src.contains("global main"));
        assert!(src.contains("mov rcx, [vec0_a]"));
        assert!(src.contains("mov rdx, [vec0_b]"));
        assert!(src.contains("WriteFile"));
        assert!(!src.contains("vec0_buf"));
    }

    #[test]
    fn pure_int_aarch64_harness_loads_abi_registers() {
        let c = min_usize_contract();
        let v = synthesize_vectors(&c);
        let src = generate_harness("min_usize", &v, Abi::Aapcs64).unwrap();
        assert!(src.contains("bl min_usize"));
        assert!(src.contains("vec0_a"));
        assert!(src.contains("vec0_b"));
        assert!(!src.contains("vec0_buf"));
    }

    #[test]
    fn pure_int_riscv_harness_loads_abi_registers() {
        let c = min_usize_contract();
        let v = synthesize_vectors(&c);
        let src = generate_harness("min_usize", &v, Abi::Riscv).unwrap();
        assert!(src.contains("jal min_usize"));
        assert!(src.contains("vec0_a"));
        assert!(src.contains("vec0_b"));
        assert!(!src.contains("vec0_buf"));
    }

    #[test]
    fn generate_harness_rejects_unsupported_vector_shape() {
        let mixed = vec![
            TestVector {
                name: "bad".into(),
                inputs: vec![serde_json::json!(1u64)],
                expected: serde_json::json!(1u64),
            },
            TestVector {
                name: "also bad".into(),
                inputs: vec![serde_json::json!(2u64), serde_json::json!(3u64)],
                expected: serde_json::json!(2u64),
            },
        ];
        let err = generate_harness("min_usize", &mixed, Abi::SysVAmd64).unwrap_err();
        assert!(err.contains("unsupported test vector shape"));
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
        assert!(
            v.iter()
                .all(|case| vector_needle(case) == u64::from(DEFAULT_BUFFER_SCAN_NEEDLE)),
            "default needle should be 0x41 when requires omit needle =="
        );
        let max_case = v
            .iter()
            .find(|case| case.name == "maximum configured fixture length")
            .expect("max length case");
        assert_eq!(
            max_case.inputs[1].as_u64(),
            Some(MAX_FIXTURE_CAP as u64),
            "length <= 4096 must clamp to MAX_FIXTURE_CAP, not bounded_stack_bytes"
        );
    }

    #[test]
    fn recognizes_named_count_equal_oracle() {
        let c = count_byte_shape();
        let oracle = recognize_behavior_oracle(&c).expect("count_byte shape");
        assert_eq!(oracle.id, ORACLE_BUFFER_COUNT_EQUAL_U8);
        assert_eq!(oracle.version, ORACLE_BUFFER_COUNT_EQUAL_U8_VERSION);
        assert!(oracle.claim.contains("equal to needle"));
        let vectors = synthesize_vectors(&c);
        let built = build_behavior_oracle(
            oracle,
            &c,
            "count_byte.sem.toml",
            b"contract",
            &vectors,
            None,
        );
        assert_eq!(built.vectors_total, vectors.len());
        assert_eq!(built.vectors_passed, 0);
        assert!(!built.evidence_hash.is_empty());
        assert!(built
            .contract_ensures
            .iter()
            .any(|e| e.contains("count <= length")));
        assert_eq!(
            built.proof_basis,
            crate::verify::ProofBasis::OracleAndVectors
        );
        assert!(is_read_only_buffer_scan(&c));
    }

    fn find_first_byte_contract() -> CheckedContract {
        let toml = include_str!("../../../fixtures/contracts/find_first_byte.sem.toml");
        check_contract(toml)
    }

    #[test]
    fn recognizes_named_find_first_oracle() {
        let c = find_first_byte_contract();
        let oracle = recognize_behavior_oracle(&c).expect("find_first shape");
        assert_eq!(oracle.id, ORACLE_BUFFER_FIND_FIRST_U8);
        assert_eq!(oracle.version, ORACLE_BUFFER_FIND_FIRST_U8_VERSION);
        assert!(oracle.claim.contains("first index"));
        assert!(oracle.claim.contains("length when absent"));
        assert!(is_read_only_buffer_scan(&c));
    }

    #[test]
    fn synthesizes_find_first_vectors_with_absent_as_length() {
        let c = find_first_byte_contract();
        let v = synthesize_vectors(&c);
        assert!(v.len() >= 7);
        let no_match = v.iter().find(|case| case.name == "no match").unwrap();
        assert_eq!(no_match.expected.as_u64(), Some(3));
        let one = v
            .iter()
            .find(|case| case.name == "one byte (match)")
            .unwrap();
        assert_eq!(one.expected.as_u64(), Some(0));
        let at_end = v.iter().find(|case| case.name == "match at end").unwrap();
        assert_eq!(at_end.expected.as_u64(), Some(2));
        let after = v
            .iter()
            .find(|case| case.name == "match after zeros")
            .unwrap();
        assert_eq!(after.expected.as_u64(), Some(1));
        validate_vectors_match_oracle(&c, &v).expect("find_first vectors match oracle");
    }

    fn find_last_byte_contract() -> CheckedContract {
        let toml = include_str!("../../../fixtures/contracts/find_last_byte.sem.toml");
        check_contract(toml)
    }

    #[test]
    fn recognizes_named_find_last_oracle() {
        let c = find_last_byte_contract();
        let oracle = recognize_behavior_oracle(&c).expect("find_last shape");
        assert_eq!(oracle.id, ORACLE_BUFFER_FIND_LAST_U8);
        assert_eq!(oracle.version, ORACLE_BUFFER_FIND_LAST_U8_VERSION);
        assert!(oracle.claim.contains("last index"));
        assert!(oracle.claim.contains("length when absent"));
        assert!(is_read_only_buffer_scan(&c));
    }

    #[test]
    fn synthesizes_find_last_vectors_with_absent_as_length() {
        let c = find_last_byte_contract();
        let v = synthesize_vectors(&c);
        assert!(v.len() >= 7);
        let no_match = v.iter().find(|case| case.name == "no match").unwrap();
        assert_eq!(no_match.expected.as_u64(), Some(3));
        let all = v.iter().find(|case| case.name == "all match").unwrap();
        assert_eq!(all.expected.as_u64(), Some(1));
        let last_two = v
            .iter()
            .find(|case| case.name == "last of two matches")
            .unwrap();
        assert_eq!(last_two.expected.as_u64(), Some(2));
        validate_vectors_match_oracle(&c, &v).expect("find_last vectors match oracle");
    }

    #[test]
    fn find_last_name_discriminator_fail_closed_on_ambiguous() {
        assert_eq!(
            buffer_scan_op_from_name("find_last_byte"),
            Some(BufferScanOp::FindLast)
        );
        assert_eq!(
            buffer_scan_op_from_name("find_first_byte"),
            Some(BufferScanOp::FindFirst)
        );
        assert_eq!(
            buffer_scan_op_from_name("count_byte"),
            Some(BufferScanOp::Count)
        );
        assert!(buffer_scan_op_from_name("count_find_byte").is_none());
    }

    #[test]
    fn ambiguous_buffer_scan_name_is_fail_closed() {
        let toml = buffer_scan_toml("", "", "").replace("count_byte", "scan_buffer");
        let c = check_contract(&toml);
        assert!(
            recognize_behavior_oracle(&c).is_none(),
            "ambiguous buffer-scan name must not claim count or find"
        );
        assert!(synthesize_vectors(&c).is_empty());
    }

    #[test]
    fn needle_from_requires_equality() {
        let toml = buffer_scan_toml(
            r#"
[[function.requires]]
expression = "length <= 64"

[[function.requires]]
expression = "needle == 7"
"#,
            r#"
[[function.effects]]
kind = "memory_read"
region = "buffer[0..length]"
"#,
            r"
[function.constraints]
no_heap = true
",
        );
        let v = synthesize_vectors(&check_contract(&toml));
        assert_eq!(v.len(), 7);
        assert!(v.iter().all(|case| vector_needle(case) == 7));
        let no_match = v.iter().find(|case| case.name == "no match").unwrap();
        let buffer = no_match.inputs[0].as_array().expect("buffer array");
        assert!(
            buffer.iter().all(|byte| byte.as_u64() != Some(7)),
            "no-match buffer must not contain the needle"
        );
    }

    #[test]
    fn length_bound_from_requires_not_stack_bytes() {
        let toml = buffer_scan_toml(
            r#"
[[function.requires]]
expression = "length <= 32"
"#,
            r#"
[[function.effects]]
kind = "memory_read"
region = "buffer[0..length]"
"#,
            r"
[function.constraints]
no_heap = true
bounded_stack_bytes = 128
",
        );
        let v = synthesize_vectors(&check_contract(&toml));
        let max_case = v
            .iter()
            .find(|case| case.name == "maximum configured fixture length")
            .expect("max length case");
        assert_eq!(max_case.inputs[1].as_u64(), Some(32));
        assert_eq!(max_case.expected.as_u64(), Some(32));
    }

    #[test]
    fn omits_null_vector_without_matching_memory_read_region() {
        let toml = buffer_scan_toml(
            r#"
[[function.requires]]
expression = "length <= 64"
"#,
            r#"
[[function.effects]]
kind = "no_memory"
"#,
            r"
[function.constraints]
no_heap = true
",
        );
        let v = synthesize_vectors(&check_contract(&toml));
        assert_eq!(v.len(), 6);
        assert!(!v
            .iter()
            .any(|case| case.name == "null pointer with zero length"));
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
        let src = generate_harness("count_byte", &v, Abi::SysVAmd64).unwrap();
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
    fn win64_harness_uses_microsoft_registers_and_win32_io() {
        let c = count_byte_shape();
        let v = synthesize_vectors(&c);
        let src = generate_harness("count_byte", &v, Abi::WindowsX64).unwrap();
        assert!(src.contains("global main"));
        assert!(src.contains("lea rcx,"));
        assert!(src.contains("mov rdx,"));
        assert!(src.contains("movzx r8d,"));
        assert!(src.contains("WriteFile"));
        assert!(src.contains("ExitProcess"));
        assert!(!src.contains("syscall"));
    }

    #[test]
    fn aarch64_harness_uses_aapcs64_registers_and_linux_syscalls() {
        let c = count_byte_shape();
        let v = synthesize_vectors(&c);
        let src = generate_harness("count_byte", &v, Abi::Aapcs64).unwrap();
        assert!(src.contains(".global _start"));
        assert!(src.contains("bl count_byte"));
        assert!(src.contains("ldr x0, =vec0_buf"));
        assert!(src.contains("mov x8, #64"));
        assert!(src.contains("mov x8, #93"));
        assert!(src.contains("svc #0"));
        assert!(!src.contains("BITS 64"));
    }

    #[test]
    fn riscv_harness_uses_lp64_registers_and_linux_syscalls() {
        let c = count_byte_shape();
        let v = synthesize_vectors(&c);
        let src = generate_harness("count_byte", &v, Abi::Riscv).unwrap();
        assert!(src.contains(".global _start"));
        assert!(src.contains("jal count_byte"));
        assert!(src.contains("la a0, vec0_buf"));
        assert!(src.contains("li a7, 64"));
        assert!(src.contains("li a7, 93"));
        assert!(src.contains("ecall"));
        assert!(!src.contains("BITS 64"));
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
