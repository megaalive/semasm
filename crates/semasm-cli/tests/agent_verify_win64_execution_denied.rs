//! End-to-end Win64 agent verify: structured ExecutionDenied without opt-in.

use std::path::PathBuf;
use std::process::Command;

fn skip_if_incomplete(stderr: &str) -> bool {
    if !stderr.contains("toolchain incomplete") {
        return false;
    }
    assert!(
        std::env::var_os("SEMASM_REQUIRE_TOOLCHAIN").is_none(),
        "toolchain incomplete in owner CI job: {stderr}"
    );
    eprintln!("skipping win64 execution_denied e2e: {stderr}");
    true
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_win64_emits_execution_denied_json_without_opt_in() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = workspace.join("fixtures/asm/count_byte_win64.asm");
    let contract = workspace.join("fixtures/contracts/count_byte.sem.toml");
    let binary = env!("CARGO_BIN_EXE_semasm");
    let output = Command::new(binary)
        .args([
            "agent",
            "verify",
            source.to_str().expect("utf-8 source path"),
            contract.to_str().expect("utf-8 contract path"),
            "--target",
            "x86_64-pc-windows-msvc",
            "--format",
            "json",
        ])
        .output()
        .expect("run semasm agent verify");

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
        panic!("expected VerificationReport JSON on stdout ({error}): {stdout}\nstderr={stderr}")
    });
    assert_eq!(value["schema_version"], "0.4");
    assert!(value["tool_version"]
        .as_str()
        .is_some_and(|v| v.starts_with("semasm ")));
    assert!(value["contract_digest"]
        .as_str()
        .is_some_and(|v| v.starts_with("sha256:") && v.len() == 7 + 64));
    assert!(value["source_digest"]
        .as_str()
        .is_some_and(|v| v.starts_with("sha256:") && v.len() == 7 + 64));
    assert_eq!(value["status"], "execution_denied");
    assert_eq!(value["target"], "x86_64-pc-windows-msvc");
    assert_eq!(value["semantic"]["abi"], "passed");
    assert_eq!(value["semantic"]["capability"], "passed");
    assert_eq!(value["executable"]["status"], "passed");
    assert!(value["behavior"].is_null());
    assert_eq!(
        value["behavior_oracle"]["id"],
        "builtin.buffer.count_equal_u8"
    );
    assert_eq!(value["behavior_oracle"]["version"], 2);
    assert_eq!(
        value["behavior_oracle"]["proof_basis"],
        "oracle_and_vectors"
    );
}
