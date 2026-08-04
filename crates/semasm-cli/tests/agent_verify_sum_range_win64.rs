//! End-to-end Win64 agent verify for the unary `sum_range` shape.

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
    eprintln!("skipping win64 sum_range agent verify e2e: {stderr}");
    true
}

fn run_agent_verify(
    source: &Path,
    allow_execution: bool,
    vectors_file: Option<&Path>,
) -> std::process::Output {
    let workspace = workspace_root();
    let contract = workspace.join("fixtures/contracts/sum_range.sem.toml");
    let binary = env!("CARGO_BIN_EXE_semasm");
    let mut args = vec![
        "agent",
        "verify",
        source.to_str().expect("utf-8 source path"),
        contract.to_str().expect("utf-8 contract path"),
        "--target",
        "x86_64-pc-windows-msvc",
        "--format",
        "json",
    ];
    if allow_execution {
        args.push("--allow-execution");
    }
    if let Some(vectors_file) = vectors_file {
        args.push("--vectors-file");
        args.push(vectors_file.to_str().expect("utf-8 vector path"));
    }
    Command::new(binary)
        .args(args)
        .output()
        .expect("run semasm agent verify")
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_sum_range_win64_allow_execution_is_verified() {
    let source = workspace_root().join("fixtures/asm/sum_range_win64.asm");
    let output = run_agent_verify(&source, true, None);
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
    assert_eq!(value["behavior_oracle"]["id"], "builtin.pure_int.unary_i64");
    assert_eq!(value["behavior"]["all_passed"], true);
    assert_eq!(value["behavior"]["cases"].as_array().map(Vec::len), Some(5));
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_sum_range_win64_execution_denied_keeps_oracle() {
    let source = workspace_root().join("fixtures/asm/sum_range_win64.asm");
    let output = run_agent_verify(&source, false, None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|error| {
        panic!("expected VerificationReport JSON ({error}): {stdout}\nstderr={stderr}")
    });
    assert_eq!(value["status"], "execution_denied");
    assert_eq!(value["behavior_oracle"]["id"], "builtin.pure_int.unary_i64");
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_sum_range_win64_wrong_emits_behavior_failed() {
    let source = workspace_root().join("fixtures/asm/sum_range_wrong_win64.asm");
    let output = run_agent_verify(&source, true, None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success(),
        "expected non-zero exit; stderr={stderr}"
    );
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|error| {
        panic!("expected VerificationReport JSON ({error}): {stdout}\nstderr={stderr}")
    });
    assert_eq!(value["status"], "behavior_failed");
    assert_eq!(value["behavior"]["all_passed"], false);
}

#[test]
#[ignore = "requires nasm, lld-link, and native Windows host"]
fn agent_verify_sum_range_win64_adds_oracle_derived_external_vectors() {
    let workspace = workspace_root();
    let source = workspace.join("fixtures/asm/sum_range_win64.asm");
    let vectors = workspace.join("fixtures/vectors/sum_range_win64.json");
    let output = run_agent_verify(&source, true, Some(&vectors));
    let stderr = String::from_utf8_lossy(&output.stderr);
    if skip_if_incomplete(&stderr) {
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stderr={stderr}; stdout={stdout}");
    let value: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(value["status"], "verified");
    assert_eq!(value["vector_set"]["builtin_case_count"], 5);
    assert_eq!(value["vector_set"]["external_case_count"], 2);
    assert!(value["vector_set"]["external_document_digest"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:")));
    assert_eq!(value["behavior"]["cases"][5]["name"], "external:four");
    assert_eq!(value["behavior"]["cases"][5]["expected"], "10");
    assert_eq!(value["behavior"]["cases"][6]["expected"], "66");
}
