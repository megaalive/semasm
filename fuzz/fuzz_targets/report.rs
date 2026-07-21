#![no_main]

use libfuzzer_sys::fuzz_target;
use semasm_build::report::{
    ArtifactFileInfo, ArtifactReport, ExecutionInfo, FileHash, SourceInfo,
    ARTIFACT_REPORT_SCHEMA_VERSION,
};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data).into_owned();
    let artifact = ArtifactFileInfo {
        path: text.clone(),
        hash: FileHash {
            sha256: format!("{:x}", data.len()),
        },
        size: u64::try_from(data.len()).unwrap_or(u64::MAX),
        sections: Vec::new(),
        symbols: Vec::new(),
        is_dynamic: false,
        raw_sections: text.clone(),
        raw_symbols: String::new(),
        raw_private_headers: String::new(),
    };
    let report = ArtifactReport {
        schema_version: ARTIFACT_REPORT_SCHEMA_VERSION,
        target: "x86_64-unknown-linux-gnu".to_string(),
        source: SourceInfo {
            path: text,
            hash: FileHash {
                sha256: "source".to_string(),
            },
        },
        tool_versions: Vec::new(),
        command_records: Vec::new(),
        object: None,
        executable: artifact,
        execution: ExecutionInfo::NotRequested,
        isolation: semasm_target::ExecutionIsolation::StaticOnly,
    };
    let _ = report.canonical_evidence_json();
    let _ = report.canonical_evidence_hash();
});
