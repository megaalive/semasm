//! End-to-end agent verify for the buffer `find_last_byte` harness shape.

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run_agent_verify(source: &Path, allow_execution: bool) -> std::process::Output {
    let workspace = workspace_root();
    let contract = workspace.join("fixtures/contracts/find_last_byte.sem.toml");
    let binary = env!("CARGO_BIN_EXE_semasm");
    let mut args = vec![
        "agent",
        "verify",
        source.to_str().expect("utf-8 source path"),
        contract.to_str().expect("utf-8 contract path"),
        "--format",
        "json",
    ];
    if allow_execution {
        args.push("--allow-execution");
    }
    Command::new(binary)
        .args(args)
        .output()
        .expect("run semasm agent verify")
}

fn skip_if_incomplete(stderr: &str) -> bool {
    if !stderr.contains("toolchain incomplete") {
        return false;
    }
    assert!(
        std::env::var_os("SEMASM_REQUIRE_TOOLCHAIN").is_none(),
        "toolchain incomplete in owner CI job: {stderr}"
    );
    eprintln!("skipping find_last_byte agent verify e2e: {stderr}");
    true
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_find_last_byte_allow_execution_is_verified() {
    let source = workspace_root().join("fixtures/asm/find_last_byte.asm");
    let output = run_agent_verify(&source, true);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected success; stderr={stderr}; stdout={stdout}"
    );
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|error| {
        panic!("expected VerificationReport JSON ({error}): {stdout}\nstderr={stderr}")
    });
    let status = value["status"].as_str().unwrap_or("");
    assert!(
        status == "verified" || status == "verified_under_preconditions",
        "expected verified or verified_under_preconditions, got {status}: {value}"
    );
    assert_eq!(value["behavior"]["all_passed"], true);
    assert!(value["behavior"]["cases"].as_array().unwrap().len() >= 7);
    assert_eq!(
        value["behavior_oracle"]["id"],
        "builtin.buffer.find_last_u8"
    );
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_find_last_byte_execution_denied_keeps_oracle() {
    let source = workspace_root().join("fixtures/asm/find_last_byte.asm");
    let output = run_agent_verify(&source, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|error| {
        panic!("expected VerificationReport JSON ({error}): {stdout}\nstderr={stderr}")
    });
    assert_eq!(value["status"], "execution_denied");
    assert_eq!(
        value["behavior_oracle"]["id"],
        "builtin.buffer.find_last_u8"
    );
    assert!(value["behavior_oracle"]["claim"]
        .as_str()
        .unwrap_or("")
        .contains("last index"));
    assert!(value["behavior"].is_null());
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_find_last_byte_wrong_emits_behavior_failed() {
    let source = workspace_root().join("fixtures/asm/find_last_byte_wrong.asm");
    let output = run_agent_verify(&source, true);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }

    assert!(
        !output.status.success(),
        "expected non-zero exit; stderr={stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|error| {
        panic!("expected VerificationReport JSON ({error}): {stdout}\nstderr={stderr}")
    });
    assert_eq!(value["status"], "behavior_failed");
    assert_eq!(value["behavior"]["all_passed"], false);
}
