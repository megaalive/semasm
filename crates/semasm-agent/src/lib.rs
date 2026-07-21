//! Task packet schema, context bundle, and integrity-hashed references for
//! the SemASM agent integration layer.
//!
//! An agent receives a [`TaskPacket`] that bundles everything needed to
//! implement a single function: the contract, target definition, tool-
//! chain snapshot, ABI mapping, test vectors, and acceptance commands.
//! Integrity hashes let the agent and audit tools verify that the
//! contract and target have not been tampered with.

pub mod context;
pub mod harness;
pub mod verify;

use semasm_contract::CheckedContract;
use semasm_core::SEMASM_VERSION;
use semasm_target::TargetIdentity;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// Task packet — root object
// ---------------------------------------------------------------------------

/// A structured task packet delivered to an external coding agent.
///
/// Every packet is self-contained: all contract definitions, target
/// descriptions, tool snapshots, and context are included inline.
/// Integrity hashes let the receiver (and audit tools) verify that
/// the contract and target definitions have not been modified since
/// the packet was created.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TaskPacket {
    /// Packet schema version (semver).
    pub version: String,
    /// ISO 8601 / RFC 3339 timestamp of creation.
    pub created_at: String,
    /// SemASM version that produced this packet.
    pub semasm_version: String,

    /// The function contract, sealed with its SHA-256 hash.
    pub contract: PackedRef<ContractContent>,
    /// The target definition, sealed with its SHA-256 hash.
    pub target: PackedRef<TargetContent>,

    /// Glob patterns for files the agent may create or modify.
    pub allowed_files: Vec<String>,
    /// Shell commands the agent is permitted to run.
    pub allowed_commands: Vec<String>,

    /// Full context the agent needs to complete the task.
    pub context: ContextBundle,
}

// ---------------------------------------------------------------------------
// Packed reference (content + SHA-256 integrity hash)
// ---------------------------------------------------------------------------

/// Bundles arbitrary content with its SHA-256 hex digest.
///
/// The hash is computed over the canonical JSON representation of
/// `content`, so the same logical input always produces the same
/// hash regardless of field ordering or formatting differences.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct PackedRef<T> {
    /// SHA-256 hex digest of the canonical JSON content.
    pub hash: String,
    /// The referenced content.
    pub content: T,
}

impl<T: Serialize> PackedRef<T> {
    /// Build a `PackedRef` by serialising `content` and hashing it.
    #[must_use]
    pub fn new(content: T) -> Self {
        let hash = hash_json(&content);
        Self { hash, content }
    }

    /// Verify that the stored hash still matches the content.
    #[must_use]
    pub fn verify(&self) -> bool
    where
        T: Serialize,
    {
        self.hash == hash_json(&self.content)
    }
}

// ---------------------------------------------------------------------------
// Contract and target content types
// ---------------------------------------------------------------------------

/// The function contract that the agent must implement.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ContractContent {
    /// Raw contract source text (e.g. the `.semasm` file contents).
    pub text: String,
    /// Validated, parsed contract.
    pub contract: CheckedContract,
}

/// The target definition that the agent must target.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TargetContent {
    /// Canonical target identity (triple + structured fields).
    pub identity: TargetIdentity,
    /// Tool binary versions at the time the packet was created.
    pub toolchain: TargetToolchain,
}

/// Snapshot of the host toolchain used for building and verifying.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TargetToolchain {
    /// Assembler binary name (e.g. `nasm`).
    pub assembler: String,
    /// Linker binary name (e.g. `ld.lld`).
    pub linker: String,
    /// Disassembler binary name (e.g. `llvm-objdump`).
    pub disassembler: String,
    /// User-mode runner binary, if available (e.g. `qemu-x86_64`).
    pub runner: Option<String>,
}

// ---------------------------------------------------------------------------
// Context bundle
// ---------------------------------------------------------------------------

/// Everything an agent needs to implement and test a single routine.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ContextBundle {
    /// Name of the function to implement.
    pub function_name: String,
    /// How each parameter is passed (register / stack offset).
    pub abi_parameters: Vec<ABIParameter>,
    /// How the return value is passed back.
    pub abi_return: ABIReturn,
    /// Registers the callee must preserve (callee-saved).
    pub preserved_registers: Vec<String>,
    /// Registers the callee may freely clobber (caller-saved).
    pub volatile_registers: Vec<String>,
    /// Allowed instruction mnemonics; empty means all are allowed.
    pub allowed_instructions: Vec<String>,
    /// Existing source lines, if any, to extend or modify.
    pub existing_source: Option<String>,
    /// Test vectors to validate the implementation against.
    pub test_vectors: Vec<TestVector>,
    /// Shell commands that constitute acceptance.
    pub acceptance_commands: Vec<String>,
}

/// A single ABI parameter mapping: name → register.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ABIParameter {
    /// Parameter name as declared in the contract.
    pub name: String,
    /// Register that holds this parameter (e.g. `rdi`, `rsi`).
    pub register: String,
    /// Semantic type name (e.g. `u64`, `ptr u8`).
    pub type_name: String,
}

/// Return value ABI mapping.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ABIReturn {
    /// Register that holds the return value (e.g. `rax`).
    pub register: String,
    /// Semantic type name (e.g. `u64`).
    pub type_name: String,
}

/// A named test case with inputs and expected output.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct TestVector {
    /// Human-readable name for this case (e.g. "empty input").
    pub name: String,
    /// Ordered input values (JSON array).
    pub inputs: Vec<serde_json::Value>,
    /// Expected output value.
    pub expected: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Packet construction helper
// ---------------------------------------------------------------------------

impl TaskPacket {
    /// Build a `TaskPacket` from the required pieces.
    ///
    /// `version` is the packet format version.  The helper sets
    /// `semasm_version` from the crate at compile time, computes
    /// integrity hashes for the contract and target content, and
    /// wires everything into a single packet.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        version: &str,
        created_at: String,
        contract_text: String,
        contract: CheckedContract,
        target_id: TargetIdentity,
        toolchain: TargetToolchain,
        allowed_files: Vec<String>,
        allowed_commands: Vec<String>,
        context: ContextBundle,
    ) -> Self {
        let contract_ref = PackedRef::new(ContractContent {
            text: contract_text,
            contract,
        });
        let target_ref = PackedRef::new(TargetContent {
            identity: target_id,
            toolchain,
        });

        Self {
            version: version.to_string(),
            created_at,
            semasm_version: SEMASM_VERSION.to_string(),
            contract: contract_ref,
            target: target_ref,
            allowed_files,
            allowed_commands,
            context,
        }
    }

    /// Verify both integrity hashes.
    #[must_use]
    pub fn verify_integrity(&self) -> bool {
        self.contract.verify() && self.target.verify()
    }
}

// ---------------------------------------------------------------------------
// Deterministic SHA-256 hashing of serialisable content
// ---------------------------------------------------------------------------

/// Compute the SHA-256 hex digest of the canonical JSON representation.
///
/// Uses `serde_json::to_value` followed by a deterministic
/// representation to ensure that identical data always produces the
/// same hash regardless of struct field order or whitespace.
fn hash_json<T: Serialize>(value: &T) -> String {
    let json_value = serde_json::to_value(value).expect("serialisation to JSON Value cannot fail");
    let canonical = canonical_json(&json_value);
    let mut hasher = Sha256::new();
    hasher.update(&canonical);
    format!("{:x}", hasher.finalize())
}

/// Serialise a `serde_json::Value` to a canonical byte string.
///
/// Keys are sorted lexicographically, no whitespace, and primitives
/// are formatted in a deterministic way (no leading zeros, scientific
/// notation only when required by JSON spec).
fn canonical_json(value: &serde_json::Value) -> Vec<u8> {
    let mut buf = Vec::new();
    write_canonical(&mut buf, value);
    buf
}

fn write_canonical(buf: &mut Vec<u8>, value: &serde_json::Value) {
    match value {
        serde_json::Value::Null => buf.extend_from_slice(b"null"),
        serde_json::Value::Bool(b) => {
            buf.extend_from_slice(if *b { b"true" } else { b"false" });
        }
        serde_json::Value::Number(n) => {
            buf.extend_from_slice(n.to_string().as_bytes());
        }
        serde_json::Value::String(s) => {
            buf.push(b'"');
            for c in s.chars() {
                match c {
                    '"' => buf.extend_from_slice(b"\\\""),
                    '\\' => buf.extend_from_slice(b"\\\\"),
                    '\n' => buf.extend_from_slice(b"\\n"),
                    '\r' => buf.extend_from_slice(b"\\r"),
                    '\t' => buf.extend_from_slice(b"\\t"),
                    c if (c as u32) < 0x20 => {
                        buf.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes());
                    }
                    c => {
                        let mut encoded = [0u8; 4];
                        let s = c.encode_utf8(&mut encoded);
                        buf.extend_from_slice(s.as_bytes());
                    }
                }
            }
            buf.push(b'"');
        }
        serde_json::Value::Array(arr) => {
            buf.push(b'[');
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    buf.push(b',');
                }
                write_canonical(buf, item);
            }
            buf.push(b']');
        }
        serde_json::Value::Object(obj) => {
            buf.push(b'{');
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    buf.push(b',');
                }
                write_canonical(buf, &serde_json::Value::String((*key).clone()));
                buf.push(b':');
                write_canonical(buf, &obj[*key]);
            }
            buf.push(b'}');
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use semasm_contract::check_str;
    use semasm_target::TargetIdentity;

    pub(crate) fn sample_contract_text() -> &'static str {
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

[[function.requires]]
expression = "length <= 4096"
reason = "Arbitrary safety limit for this fixture."

[[function.ensures]]
expression = "count <= length"

[[function.effects]]
kind = "memory_read"
region = "buffer[0..length]"

[function.constraints]
no_heap = true
no_recursion = true
bounded_stack_bytes = 128
"#
        .trim()
    }

    pub(crate) fn sample_check() -> CheckedContract {
        let result = check_str(sample_contract_text());
        assert!(result.ok(), "sample contract should validate");
        result.contract.unwrap()
    }

    pub(crate) fn sample_target() -> TargetIdentity {
        TargetIdentity::x86_64_linux_gnu()
    }

    pub(crate) fn sample_toolchain() -> TargetToolchain {
        TargetToolchain {
            assembler: "nasm".into(),
            linker: "ld.lld".into(),
            disassembler: "llvm-objdump".into(),
            runner: Some("qemu-x86_64".into()),
        }
    }

    fn sample_context() -> ContextBundle {
        ContextBundle {
            function_name: "count_byte".into(),
            abi_parameters: vec![
                ABIParameter {
                    name: "buffer".into(),
                    register: "rdi".into(),
                    type_name: "ptr u8".into(),
                },
                ABIParameter {
                    name: "length".into(),
                    register: "rsi".into(),
                    type_name: "usize".into(),
                },
                ABIParameter {
                    name: "needle".into(),
                    register: "rdx".into(),
                    type_name: "u8".into(),
                },
            ],
            abi_return: ABIReturn {
                register: "rax".into(),
                type_name: "usize".into(),
            },
            preserved_registers: vec![
                "rbx".into(),
                "rbp".into(),
                "r12".into(),
                "r13".into(),
                "r14".into(),
                "r15".into(),
            ],
            volatile_registers: vec![
                "rax".into(),
                "rcx".into(),
                "rdx".into(),
                "rdi".into(),
                "rsi".into(),
                "r8".into(),
                "r9".into(),
                "r10".into(),
                "r11".into(),
            ],
            allowed_instructions: vec![],
            existing_source: None,
            test_vectors: vec![TestVector {
                name: "empty buffer".into(),
                inputs: vec![
                    serde_json::Value::String("0x0".into()),
                    serde_json::Value::Number(0.into()),
                    serde_json::Value::Number(42.into()),
                ],
                expected: serde_json::Value::Number(0.into()),
            }],
            acceptance_commands: vec!["nasm -f elf64 -o /dev/null src/count_byte.asm".into()],
        }
    }

    #[test]
    fn packet_roundtrip_json() {
        let packet = TaskPacket::new(
            "0.1.0",
            "2026-07-17T12:00:00Z".into(),
            sample_contract_text().into(),
            sample_check(),
            sample_target(),
            sample_toolchain(),
            vec!["src/**/*.asm".into()],
            vec!["nasm".into(), "ld".into()],
            sample_context(),
        );

        let json = serde_json::to_string_pretty(&packet).expect("serialise packet");
        let deserialized: TaskPacket = serde_json::from_str(&json).expect("deserialise packet");

        assert_eq!(packet, deserialized);
        assert!(deserialized.verify_integrity());
    }

    #[test]
    fn integrity_hash_matches() {
        let content = ContractContent {
            text: "dummy".into(),
            contract: sample_check(),
        };
        let first = PackedRef::new(content.clone());
        let second = PackedRef::new(content);
        assert_eq!(first.hash, second.hash, "deterministic hash");
        assert!(first.verify());
    }

    #[test]
    fn integrity_detects_tamper() {
        let content = ContractContent {
            text: "original".into(),
            contract: sample_check(),
        };
        let mut pref = PackedRef::new(content);
        pref.content.text = "tampered".into();
        assert!(!pref.verify(), "should detect content change");
    }

    #[test]
    fn packet_serialises_to_json() {
        let packet = TaskPacket::new(
            "0.1.0",
            "2026-07-17T12:00:00Z".into(),
            sample_contract_text().into(),
            sample_check(),
            sample_target(),
            sample_toolchain(),
            vec![],
            vec![],
            sample_context(),
        );

        let json = serde_json::to_string_pretty(&packet).expect("serialise");
        assert!(json.contains("count_byte"));
        assert!(json.contains("hash"));
        assert!(json.contains("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn packet_verify_integrity() {
        let packet = TaskPacket::new(
            "0.1.0",
            "now".into(),
            sample_contract_text().into(),
            sample_check(),
            sample_target(),
            sample_toolchain(),
            vec![],
            vec![],
            sample_context(),
        );
        assert!(packet.verify_integrity());
    }

    #[test]
    fn canonical_json_is_deterministic() {
        let v1: serde_json::Value =
            serde_json::from_str(r#"{"z":1,"a":{"nested":true,"b":2}}"#).unwrap();
        let v2: serde_json::Value =
            serde_json::from_str(r#"{"a":{"b":2,"nested":true},"z":1}"#).unwrap();

        let c1 = canonical_json(&v1);
        let c2 = canonical_json(&v2);
        assert_eq!(c1, c2, "canonical form must ignore key order");
    }

    #[cfg(feature = "schema")]
    #[test]
    fn generate_json_schema() {
        use std::io::Write;

        let schema = schemars::schema_for!(TaskPacket);
        let json = serde_json::to_string_pretty(&schema).expect("schema serialisation");

        let path = std::env::current_dir()
            .unwrap()
            .join("schemas")
            .join("task-packet.json");

        std::fs::create_dir_all(path.parent().unwrap()).ok();
        let mut file = std::fs::File::create(&path).expect("create schema file");
        file.write_all(json.as_bytes()).expect("write schema");
        file.flush().expect("flush");

        eprintln!("wrote JSON schema to {}", path.display());
    }
}
