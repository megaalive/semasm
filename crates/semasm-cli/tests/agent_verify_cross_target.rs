//! Cross-target agent verify: AArch64 / RV64 execution_denied and verified.

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run_agent_verify(source: &Path, target: &str, allow_execution: bool) -> std::process::Output {
    run_agent_verify_contract(
        source,
        target,
        allow_execution,
        "fixtures/contracts/count_byte.sem.toml",
    )
}

fn run_agent_verify_contract(
    source: &Path,
    target: &str,
    allow_execution: bool,
    contract_rel: &str,
) -> std::process::Output {
    let workspace = workspace_root();
    let contract = workspace.join(contract_rel);
    let binary = env!("CARGO_BIN_EXE_semasm");
    let mut args = vec![
        "agent",
        "verify",
        source.to_str().expect("utf-8 source path"),
        contract.to_str().expect("utf-8 contract path"),
        "--target",
        target,
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
    eprintln!("skipping cross-target agent verify e2e: {stderr}");
    true
}

fn assert_execution_denied(output: &std::process::Output, target: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "expected non-zero exit; stderr={stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|error| {
        panic!("expected VerificationReport JSON ({error}): {stdout}\nstderr={stderr}")
    });
    assert_eq!(value["status"], "execution_denied");
    assert_eq!(value["target"], target);
    assert_eq!(value["semantic"]["abi"], "passed");
    assert_eq!(value["executable"]["status"], "passed");
    assert!(value["behavior"].is_null());
}

fn assert_verified(output: &std::process::Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "expected success; stderr={stderr}; stdout={stdout}"
    );
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|error| {
        panic!("expected VerificationReport JSON ({error}): {stdout}\nstderr={stderr}")
    });
    assert_eq!(value["status"], "verified");
    assert_eq!(value["behavior"]["all_passed"], true);
    assert!(value["behavior"]["cases"].as_array().unwrap().len() >= 6);
}

#[test]
#[ignore = "requires aarch64-linux-gnu-as/ld and qemu-aarch64 on PATH"]
fn agent_verify_aarch64_emits_execution_denied_without_opt_in() {
    let source = workspace_root().join("fixtures/asm/count_byte_aarch64.S");
    let output = run_agent_verify(&source, "aarch64-unknown-linux-gnu", false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_execution_denied(&output, "aarch64-unknown-linux-gnu");
}

#[test]
#[ignore = "requires aarch64-linux-gnu-as/ld and qemu-aarch64 on PATH"]
fn agent_verify_aarch64_allow_execution_is_verified() {
    let source = workspace_root().join("fixtures/asm/count_byte_aarch64.S");
    let output = run_agent_verify(&source, "aarch64-unknown-linux-gnu", true);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_verified(&output);
}

#[test]
#[ignore = "requires riscv64-linux-gnu-as/ld and qemu-riscv64 on PATH"]
fn agent_verify_riscv64_emits_execution_denied_without_opt_in() {
    let source = workspace_root().join("fixtures/asm/count_byte_riscv64.S");
    let output = run_agent_verify(&source, "riscv64gc-unknown-linux-gnu", false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_execution_denied(&output, "riscv64gc-unknown-linux-gnu");
}

#[test]
#[ignore = "requires riscv64-linux-gnu-as/ld and qemu-riscv64 on PATH"]
fn agent_verify_riscv64_allow_execution_is_verified() {
    let source = workspace_root().join("fixtures/asm/count_byte_riscv64.S");
    let output = run_agent_verify(&source, "riscv64gc-unknown-linux-gnu", true);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_verified(&output);
}

#[test]
#[ignore = "requires aarch64-linux-gnu-as/ld and qemu-aarch64 on PATH"]
fn agent_verify_memcmp_aarch64_allow_execution_is_verified() {
    let source = workspace_root().join("fixtures/asm/memcmp_aarch64.S");
    let output = run_agent_verify_contract(
        &source,
        "aarch64-unknown-linux-gnu",
        true,
        "fixtures/contracts/memcmp.sem.toml",
    );
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
    assert_eq!(value["status"], "verified");
    assert_eq!(value["behavior_oracle"]["id"], "builtin.buffer.memcmp_i8");
}

#[test]
#[ignore = "requires riscv64-linux-gnu-as/ld and qemu-riscv64 on PATH"]
fn agent_verify_memcmp_riscv64_allow_execution_is_verified() {
    let source = workspace_root().join("fixtures/asm/memcmp_riscv64.S");
    let output = run_agent_verify_contract(
        &source,
        "riscv64gc-unknown-linux-gnu",
        true,
        "fixtures/contracts/memcmp.sem.toml",
    );
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
    assert_eq!(value["status"], "verified");
    assert_eq!(value["behavior_oracle"]["id"], "builtin.buffer.memcmp_i8");
}
