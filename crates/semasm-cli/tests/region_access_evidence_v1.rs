//! Region Access Evidence v1 x86 acceptance corpus (ADR 0011 / Sei Ra5).
//!
//! Drives `evaluate_region_access` against the minimum fixture list in
//! `docs/REGION_ACCESS_EVIDENCE_V1_PLAN.md`. Not a full-ISA memory-safety proof.

use semasm_contract::{
    check_file, check_str, evaluate_region_access, AccessAddr, AccessMode, BoundsStatus,
    ObservedMemoryAccess, PermissionStatus, RegionAccessStatus,
};
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

fn load_memory(name: &str) -> semasm_contract::CheckedMemory {
    let path = fixtures().join("contracts").join(name);
    check_file(&path)
        .unwrap_or_else(|e| panic!("read {name}: {e}"))
        .contract
        .unwrap_or_else(|| panic!("{name} must validate"))
        .memory
        .unwrap_or_else(|| panic!("{name} must declare function.memory"))
}

fn access(mode: AccessMode, width: u32, addr: AccessAddr, off: u64) -> ObservedMemoryAccess {
    ObservedMemoryAccess {
        mode,
        width_bytes: width,
        addr,
        mnemonic: "mov".into(),
        instruction_offset: off,
    }
}

fn affine(base: &str, offset: i64) -> AccessAddr {
    AccessAddr::Affine {
        base_param: base.into(),
        offset,
    }
}

#[test]
fn load_inside_read_region() {
    let memory = check_str(
        r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "p"
type = "ptr<const u8>"
[[function.returns]]
name = "status"
type = "usize"
[[function.memory.regions]]
name = "buf"
base = "p"
length = "8"
access = "read"
"#,
    )
    .contract
    .unwrap()
    .memory
    .unwrap();
    let report =
        evaluate_region_access(&memory, &[access(AccessMode::Load, 1, affine("p", 3), 12)]);
    assert_eq!(report.status, RegionAccessStatus::Passed);
    assert_eq!(report.accesses[0].bounds, BoundsStatus::ProvenInside);
    assert_eq!(report.model, "region-access-affine-v1");
}

#[test]
fn store_inside_write_region() {
    let memory = check_str(
        r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "p"
type = "ptr<u8>"
[[function.returns]]
name = "status"
type = "usize"
[[function.memory.regions]]
name = "dst"
base = "p"
length = "8"
access = "write"
"#,
    )
    .contract
    .unwrap()
    .memory
    .unwrap();
    let report =
        evaluate_region_access(&memory, &[access(AccessMode::Store, 1, affine("p", 2), 8)]);
    assert_eq!(report.status, RegionAccessStatus::Passed);
    assert_eq!(report.accesses[0].permission, PermissionStatus::Allowed);
}

#[test]
fn store_to_read_only_region() {
    let memory = check_str(
        r#"
contract_version = "0.1"
[function]
name = "ro"
[[function.parameters]]
name = "p"
type = "ptr<const u8>"
[[function.returns]]
name = "status"
type = "usize"
[[function.memory.regions]]
name = "buf"
base = "p"
length = "8"
access = "read"
"#,
    )
    .contract
    .unwrap()
    .memory
    .unwrap();
    let report =
        evaluate_region_access(&memory, &[access(AccessMode::Store, 1, affine("p", 0), 0)]);
    assert_eq!(report.status, RegionAccessStatus::Failed);
    assert_eq!(report.accesses[0].permission, PermissionStatus::Denied);
}

#[test]
fn load_before_region() {
    let memory = check_str(
        r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "p"
type = "ptr<u8>"
[[function.returns]]
name = "status"
type = "usize"
[[function.memory.regions]]
name = "buf"
base = "p"
offset = "4"
length = "4"
access = "read"
"#,
    )
    .contract
    .unwrap()
    .memory
    .unwrap();
    let report = evaluate_region_access(&memory, &[access(AccessMode::Load, 1, affine("p", 0), 0)]);
    assert_eq!(report.status, RegionAccessStatus::Failed);
    assert_eq!(report.accesses[0].bounds, BoundsStatus::ProvenOutside);
}

#[test]
fn store_after_region() {
    let memory = check_str(
        r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "p"
type = "ptr<u8>"
[[function.returns]]
name = "status"
type = "usize"
[[function.memory.regions]]
name = "dst"
base = "p"
length = "4"
access = "write"
"#,
    )
    .contract
    .unwrap()
    .memory
    .unwrap();
    let report =
        evaluate_region_access(&memory, &[access(AccessMode::Store, 1, affine("p", 4), 0)]);
    assert_eq!(report.status, RegionAccessStatus::Failed);
    assert_eq!(report.accesses[0].bounds, BoundsStatus::ProvenOutside);
}

#[test]
fn unknown_base_register() {
    let memory = load_memory("memcpy.sem.toml");
    let report = evaluate_region_access(
        &memory,
        &[access(AccessMode::Load, 1, AccessAddr::Unknown, 0)],
    );
    assert_eq!(report.status, RegionAccessStatus::Incomplete);
    assert!(report.accesses_unknown >= 1);
}

#[test]
fn known_base_symbolic_length_under_preconditions() {
    let memory = load_memory("memcpy.sem.toml");
    let report =
        evaluate_region_access(&memory, &[access(AccessMode::Load, 1, affine("src", 0), 0)]);
    assert_eq!(
        report.status,
        RegionAccessStatus::PassedUnderPreconditions
    );
    assert_eq!(report.accesses[0].bounds, BoundsStatus::MayEscape);
    assert_eq!(report.accesses[0].permission, PermissionStatus::Allowed);
    assert_eq!(report.accesses_unknown, 0);
}

#[test]
fn multi_byte_access_crosses_end() {
    let memory = check_str(
        r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "p"
type = "ptr<u8>"
[[function.returns]]
name = "status"
type = "usize"
[[function.memory.regions]]
name = "buf"
base = "p"
length = "4"
access = "read"
"#,
    )
    .contract
    .unwrap()
    .memory
    .unwrap();
    let report = evaluate_region_access(&memory, &[access(AccessMode::Load, 4, affine("p", 2), 0)]);
    assert_eq!(report.status, RegionAccessStatus::Incomplete);
    assert_eq!(report.accesses[0].bounds, BoundsStatus::MayEscape);
}

#[test]
fn same_region_read_write() {
    let memory = check_str(
        r#"
contract_version = "0.1"
[function]
name = "f"
[[function.parameters]]
name = "p"
type = "ptr<u8>"
[[function.returns]]
name = "status"
type = "usize"
[[function.memory.regions]]
name = "buf"
base = "p"
length = "8"
access = "read_write"
"#,
    )
    .contract
    .unwrap()
    .memory
    .unwrap();
    let report = evaluate_region_access(
        &memory,
        &[
            access(AccessMode::Load, 1, affine("p", 0), 0),
            access(AccessMode::Store, 1, affine("p", 1), 4),
        ],
    );
    assert_eq!(report.status, RegionAccessStatus::Passed);
    assert_eq!(report.accesses_proven_inside, 2);
}

#[test]
fn memcpy_disjoint_regions() {
    let memory = check_str(
        r#"
contract_version = "0.1"
[function]
name = "memcpy"
[[function.parameters]]
name = "dst"
type = "ptr<u8>"
[[function.parameters]]
name = "src"
type = "ptr<const u8>"
[[function.returns]]
name = "status"
type = "usize"
[[function.memory.regions]]
name = "src"
base = "src"
length = "8"
access = "read"
[[function.memory.regions]]
name = "dst"
base = "dst"
length = "8"
access = "write"
[[function.memory.relations]]
left = "src"
right = "dst"
require = "disjoint"
basis = "precondition"
"#,
    )
    .contract
    .unwrap()
    .memory
    .unwrap();
    let report = evaluate_region_access(
        &memory,
        &[
            access(AccessMode::Load, 1, affine("src", 0), 0),
            access(AccessMode::Store, 1, affine("dst", 0), 4),
        ],
    );
    assert_eq!(report.status, RegionAccessStatus::Passed);
    assert_eq!(report.accesses_proven_inside, 2);
}

#[test]
fn memcpy_possible_overlap() {
    let memory = load_memory("memcpy_partial_overlap.sem.toml");
    let report = evaluate_region_access(
        &memory,
        &[access(AccessMode::Store, 1, affine("buf", 4), 0)],
    );
    assert_eq!(report.status, RegionAccessStatus::Passed);
    assert_eq!(report.accesses[0].bounds, BoundsStatus::ProvenInside);
    assert_eq!(report.accesses[0].permission, PermissionStatus::Allowed);
    assert_eq!(report.accesses[0].region.as_deref(), Some("tail"));
}
