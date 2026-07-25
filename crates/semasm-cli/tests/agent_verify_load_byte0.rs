//! End-to-end agent verify for concrete 1-byte cell load (`load_byte0`).
//!
//! Honesty: literal region length ⇒ region_access `passed` ⇒ overall
//! `verified` (≠ verified_under_preconditions for symbolic-length leaves).

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run_agent_verify(source: &Path, allow_execution: bool) -> std::process::Output {
    let workspace = workspace_root();
    let contract = workspace.join("fixtures/contracts/load_byte0.sem.toml");
    let binary = env!("CARGO_BIN_EXE_semasm");
    let mut args = vec![
        "agent",
        "verify",
        source.to_str().expect("utf-8 source path"),
        contract.to_str().expect("utf-8 contract path"),
        "--format",
        "json",
        "--target",
        "x86_64-pc-windows-msvc",
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
    eprintln!("skipping load_byte0 agent verify e2e: {stderr}");
    true
}

#[test]
#[ignore = "requires nasm, link, and Win64 runner on PATH"]
fn agent_verify_load_byte0_allow_execution_is_verified() {
    let source = workspace_root().join("fixtures/asm/load_byte0_win64.asm");
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
    assert_eq!(
        value["status"], "verified",
        "concrete cell must be unconditional verified: {value}"
    );
    assert_eq!(value["region_access"]["status"], "passed");
    assert_eq!(value["behavior"]["all_passed"], true);
    assert_eq!(value["behavior_oracle"]["id"], "builtin.buffer.load_byte0");
}
