//! End-to-end Win64 agent verify with `--allow-execution`.

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
    eprintln!("skipping win64 verified e2e: {stderr}");
    true
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_win64_allow_execution_is_verified() {
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
            "--allow-execution",
            "--format",
            "json",
        ])
        .output()
        .expect("run semasm agent verify");

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
    assert!(value["behavior"]["cases"].as_array().unwrap().len() >= 6);
}
