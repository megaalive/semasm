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
//!   — canonical example `count_byte`.
//! - **Pure integer** `(usize, usize) -> usize` — canonical example `min_usize`.
//!
//! Synthesis tries buffer-scan first, then pure-integer.  When the contract
//! matches neither shape the synthesizer returns an empty vector set and the
//! caller may fall back to a hand-written set.
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
        return synthesize_buffer_scan_vectors(shape);
    }
    if pure_int_shape(contract).is_some() {
        return synthesize_pure_int_vectors();
    }
    Vec::new()
}

/// Resolved calling shape for the harness generator.
enum HarnessShape {
    BufferScan,
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
        ) =>
        {
            Ok(HarnessShape::BufferScan)
        }
        2 if first.inputs.iter().all(serde_json::Value::is_number) => Ok(HarnessShape::PureInt),
        n => Err(format!(
            "unsupported test vector shape ({n} inputs); \
             expected buffer-scan (3) or pure-int (2 numeric)"
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

/// Synthesise canonical pure-integer test vectors for `(usize, usize) -> usize`.
#[allow(clippy::vec_init_then_push)]
fn synthesize_pure_int_vectors() -> Vec<TestVector> {
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
            expected: serde_json::json!(a.min(b)),
        })
        .collect()
}

/// Detect the `(usize, usize) -> usize` pure-integer shape.
fn pure_int_shape(contract: &CheckedContract) -> Option<()> {
    let usize_count = contract
        .parameters
        .iter()
        .filter(|p| matches!(p.ty, SemType::Usize))
        .count();
    if usize_count != 2 {
        return None;
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
        return None;
    }

    let returns_usize = contract
        .returns
        .iter()
        .any(|r| matches!(r.ty, SemType::Usize));
    if returns_usize {
        Some(())
    } else {
        None
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

/// Resolved calling shape for the canonical buffer-scan function.
#[derive(Clone, Copy)]
struct ScanShape {
    /// Needle value used for synthesised inputs.
    needle: u8,
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

    Some(ScanShape {
        needle: needle_from_requires(contract, &needle_param.name)
            .unwrap_or(DEFAULT_BUFFER_SCAN_NEEDLE),
        max_length: length_bound_from_requires(contract, &len_param.name),
        allows_null_when_empty: allows_null_when_empty(contract, &ptr_param.name, &len_param.name),
    })
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
        (Abi::SysVAmd64, HarnessShape::PureInt) => {
            Ok(generate_sysv_pure_int_harness(routine_symbol, vectors))
        }
        (Abi::WindowsX64, HarnessShape::BufferScan) => {
            Ok(generate_win64_buffer_harness(routine_symbol, vectors))
        }
        (Abi::WindowsX64, HarnessShape::PureInt) => {
            Ok(generate_win64_pure_int_harness(routine_symbol, vectors))
        }
        (Abi::Aapcs64, HarnessShape::BufferScan) => {
            Ok(generate_aapcs64_buffer_harness(routine_symbol, vectors))
        }
        (Abi::Aapcs64, HarnessShape::PureInt) => {
            Ok(generate_aapcs64_pure_int_harness(routine_symbol, vectors))
        }
        (Abi::Riscv, HarnessShape::BufferScan) => {
            Ok(generate_riscv_buffer_harness(routine_symbol, vectors))
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
