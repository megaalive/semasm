//! Evidence card + candidate compare smoke tests for agent verify.

use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn skip_if_incomplete(stderr: &str) -> bool {
    if !stderr.contains("toolchain incomplete") {
        return false;
    }
    assert!(
        std::env::var_os("SEMASM_REQUIRE_TOOLCHAIN").is_none(),
        "toolchain incomplete in owner CI job: {stderr}"
    );
    eprintln!("skipping evidence/compare e2e: {stderr}");
    true
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_sysv_writes_evidence_card() {
    let workspace = workspace_root();
    let source = workspace.join("fixtures/asm/count_byte.asm");
    let contract = workspace.join("fixtures/contracts/count_byte.sem.toml");
    let card = std::env::temp_dir().join(format!(
        "semasm-card-{}-{}.md",
        std::process::id(),
        "sysv"
    ));
    let _ = std::fs::remove_file(&card);

    let output = Command::new(env!("CARGO_BIN_EXE_semasm"))
        .args([
            "agent",
            "verify",
            source.to_str().unwrap(),
            contract.to_str().unwrap(),
            "--format",
            "json",
            "--card",
            card.to_str().unwrap(),
        ])
        .output()
        .expect("run verify");
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert!(
        card.is_file(),
        "expected evidence card at {}; stderr={stderr}",
        card.display()
    );
    let body = std::fs::read_to_string(&card).expect("read card");
    assert!(body.contains("SemASM Evidence Card"));
    assert!(body.contains("execution_denied") || body.contains("verified"));
    assert!(body.contains("Control"));
    let _ = std::fs::remove_file(&card);
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_compare_sysv_diffs_correct_vs_wrong() {
    let workspace = workspace_root();
    let a = workspace.join("fixtures/asm/count_byte.asm");
    let b = workspace.join("fixtures/asm/count_byte_wrong.asm");
    let contract = workspace.join("fixtures/contracts/count_byte.sem.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_semasm"))
        .args([
            "agent",
            "compare",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            contract.to_str().unwrap(),
            "--format",
            "json",
            "--allow-execution",
        ])
        .output()
        .expect("run compare");
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|error| panic!("json ({error}): {stdout}"));
    assert_eq!(value["status_a"], "verified");
    assert_eq!(value["status_b"], "behavior_failed");
    assert_eq!(value["preferred"], "count_byte.asm");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_compare_sysv_static_flags_indirect() {
    let workspace = workspace_root();
    let a = workspace.join("fixtures/asm/count_byte.asm");
    let b = workspace.join("fixtures/asm/count_byte_indirect.asm");
    let contract = workspace.join("fixtures/contracts/count_byte.sem.toml");

    let output = Command::new(env!("CARGO_BIN_EXE_semasm"))
        .args([
            "agent",
            "compare",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
            contract.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("run compare");
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value =
        serde_json::from_str(&stdout).unwrap_or_else(|error| panic!("json ({error}): {stdout}"));
    assert_eq!(value["status_a"], "execution_denied");
    assert_eq!(value["status_b"], "semantic_failed");
    let diffs = value["gate_diffs"].as_array().expect("gate_diffs");
    assert!(
        diffs
            .iter()
            .any(|d| d.as_str().is_some_and(|s| s.contains("control"))),
        "expected control gate diff: {diffs:?}"
    );
}
