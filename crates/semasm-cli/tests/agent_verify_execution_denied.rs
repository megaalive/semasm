//! End-to-end agent verify: structured ExecutionDenied without opt-in.

use std::path::PathBuf;
use std::process::Command;

#[test]
#[ignore = "requires nasm, ld, objdump, and qemu-user on PATH"]
fn agent_verify_emits_execution_denied_json_without_opt_in() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = workspace.join("fixtures/asm/count_byte.asm");
    let contract = workspace.join("fixtures/contracts/count_byte.sem.toml");
    let binary = env!("CARGO_BIN_EXE_semasm");
    let output = Command::new(binary)
        .args([
            "agent",
            "verify",
            source.to_str().expect("utf-8 source path"),
            contract.to_str().expect("utf-8 contract path"),
            "--format",
            "json",
        ])
        .output()
        .expect("run semasm agent verify");

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("toolchain incomplete") {
        // Local Windows hosts often lack the Linux verification toolchain.
        // GitHub Actions sets CI=true and the decode job installs the tools,
        // so incomplete toolchain there must fail the test.
        assert!(
            std::env::var_os("CI").is_none(),
            "CI must provide the verification toolchain: {stderr}"
        );
        eprintln!("skipping execution_denied e2e: {stderr}");
        return;
    }

    assert!(
        !output.status.success(),
        "expected non-zero exit; stderr={stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|error| {
        panic!("expected VerificationReport JSON on stdout ({error}): {stdout}\nstderr={stderr}")
    });
    assert_eq!(value["schema_version"], "0.1");
    assert_eq!(value["status"], "execution_denied");
    assert_eq!(value["semantic"]["abi"], "passed");
    assert_eq!(value["semantic"]["capability"], "passed");
    assert_eq!(value["executable"]["status"], "passed");
    assert!(value["behavior"].is_null());
}
