//! Adversarial agent-verify corpus: behavior_failed and semantic_failed paths.

use std::path::{Path, PathBuf};
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
    eprintln!("skipping adversarial agent verify e2e: {stderr}");
    true
}

fn run_agent_verify(
    source: &Path,
    contract: &Path,
    target: Option<&str>,
    allow_execution: bool,
) -> std::process::Output {
    let binary = env!("CARGO_BIN_EXE_semasm");
    let mut args = vec![
        "agent".to_string(),
        "verify".to_string(),
        source.to_str().expect("utf-8").to_string(),
        contract.to_str().expect("utf-8").to_string(),
        "--format".to_string(),
        "json".to_string(),
    ];
    if let Some(target) = target {
        args.push("--target".to_string());
        args.push(target.to_string());
    }
    if allow_execution {
        args.push("--allow-execution".to_string());
    }
    Command::new(binary)
        .args(args)
        .output()
        .expect("run semasm agent verify")
}

fn assert_status(output: &std::process::Output, expected: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "expected failure status={expected}; stderr={stderr}; stdout={stdout}"
    );
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|error| {
        panic!("expected VerificationReport JSON ({error}): {stdout}\nstderr={stderr}")
    });
    assert_eq!(
        value["status"], expected,
        "stdout={stdout}\nstderr={stderr}"
    );
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_wrong_sysv_is_behavior_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_wrong.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, true);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "behavior_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_syscall_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_syscall.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_syscall_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_syscall_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["capability"], "failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_import_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_import.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_noexport_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_noexport.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_import_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_import_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["object_policy"], "failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_noexport_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_noexport_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["object_policy"], "failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_wrong_win64_is_behavior_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_wrong_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), true);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "behavior_failed");
}

#[test]
#[ignore = "requires aarch64-linux-gnu-as/ld and qemu-aarch64 on PATH"]
fn agent_verify_wrong_aarch64_is_behavior_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_wrong_aarch64.S");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("aarch64-unknown-linux-gnu"), true);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "behavior_failed");
}

#[test]
#[ignore = "requires aarch64-linux-gnu-as/ld and qemu-aarch64 on PATH"]
fn agent_verify_svc_aarch64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_svc_aarch64.S");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("aarch64-unknown-linux-gnu"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires riscv64-linux-gnu-as/ld and qemu-riscv64 on PATH"]
fn agent_verify_wrong_riscv64_is_behavior_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_wrong_riscv64.S");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(
        &source,
        &contract,
        Some("riscv64gc-unknown-linux-gnu"),
        true,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "behavior_failed");
}

#[test]
#[ignore = "requires riscv64-linux-gnu-as/ld and qemu-riscv64 on PATH"]
fn agent_verify_ecall_riscv64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_ecall_riscv64.S");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(
        &source,
        &contract,
        Some("riscv64gc-unknown-linux-gnu"),
        false,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_stack_imbalance_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_stack_imbalance.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_stack_imbalance_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_stack_imbalance_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["abi"], "failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_callee_saved_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_callee_saved.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_callee_saved_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_callee_saved_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_min_usize_callee_saved_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/min_usize_callee_saved.asm");
    let contract = workspace_root().join("fixtures/contracts/min_usize.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_min_usize_callee_saved_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/min_usize_callee_saved_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/min_usize.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_max_usize_callee_saved_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/max_usize_callee_saved.asm");
    let contract = workspace_root().join("fixtures/contracts/max_usize.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_max_usize_callee_saved_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/max_usize_callee_saved_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/max_usize.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_sum_i64_callee_saved_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/sum_i64_callee_saved.asm");
    let contract = workspace_root().join("fixtures/contracts/sum_i64.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_sum_i64_callee_saved_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/sum_i64_callee_saved_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/sum_i64.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_find_first_byte_callee_saved_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/find_first_byte_callee_saved.asm");
    let contract = workspace_root().join("fixtures/contracts/find_first_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_find_first_byte_callee_saved_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/find_first_byte_callee_saved_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/find_first_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_find_last_byte_callee_saved_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/find_last_byte_callee_saved.asm");
    let contract = workspace_root().join("fixtures/contracts/find_last_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_find_last_byte_callee_saved_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/find_last_byte_callee_saved_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/find_last_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_red_zone_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_red_zone.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_shadow_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_win64_shadow.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_unknown_insn_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_unknown_insn.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_unknown_insn_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_unknown_insn_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_trailing_bytes_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_trailing_bytes.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_trailing_bytes_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_trailing_bytes_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_wx_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_wx.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_indirect_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_indirect.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["control"], "failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_indirect_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_indirect_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["control"], "failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_write_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_write.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_write_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/count_byte_write_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/count_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_sum_i64_write_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/sum_i64_write.asm");
    let contract = workspace_root().join("fixtures/contracts/sum_i64.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_sum_i64_write_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/sum_i64_write_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/sum_i64.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_min_usize_write_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/min_usize_write.asm");
    let contract = workspace_root().join("fixtures/contracts/min_usize.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_min_usize_write_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/min_usize_write_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/min_usize.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_max_usize_write_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/max_usize_write.asm");
    let contract = workspace_root().join("fixtures/contracts/max_usize.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_max_usize_write_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/max_usize_write_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/max_usize.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_find_first_byte_write_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/find_first_byte_write.asm");
    let contract = workspace_root().join("fixtures/contracts/find_first_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_find_first_byte_write_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/find_first_byte_write_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/find_first_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_find_last_byte_write_sysv_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/find_last_byte_write.asm");
    let contract = workspace_root().join("fixtures/contracts/find_last_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, None, false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_find_last_byte_write_win64_is_semantic_failed() {
    let source = workspace_root().join("fixtures/asm/find_last_byte_write_win64.asm");
    let contract = workspace_root().join("fixtures/contracts/find_last_byte.sem.toml");
    let output = run_agent_verify(&source, &contract, Some("x86_64-pc-windows-msvc"), false);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }
    assert_status(&output, "semantic_failed");
    let value: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
    assert_eq!(value["semantic"]["memory"], "failed");
}
