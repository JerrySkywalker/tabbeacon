#![cfg(windows)]

use std::{path::PathBuf, process::Command};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/codex-compatibility-source-diff")
        .join(name)
}

fn classify(candidate: &str) -> String {
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/compare-codex-compatibility.ps1");
    let output = Command::new("pwsh.exe")
        .args(["-NoProfile", "-File"])
        .arg(script)
        .arg("-AdmittedSource")
        .arg(fixture("admitted"))
        .arg("-CandidateSource")
        .arg(fixture(candidate))
        .output()
        .expect("compatibility source diff starts");
    assert!(
        output.status.success(),
        "source diff failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("source diff output is UTF-8")
}

#[test]
fn source_diff_classifies_safe_review_and_breaking_fixtures() {
    let safe = classify("candidate-safe");
    assert!(safe.contains("DELTA_AUDIT_SCHEMA=tabbeacon-codex-hook-delta-v1"));
    assert!(safe.contains("CLASSIFICATION=SAFE_COMPATIBLE"));
    assert!(safe.contains("PROTOCOL_DELTA=NONE_RELEVANT"));
    assert!(safe.contains("EXACT_PRODUCTION_ADMISSION=NOT_GRANTED"));

    let review = classify("candidate-review");
    assert!(review.contains("CLASSIFICATION=REQUIRES_REVIEW"));
    assert!(review.contains("PROTOCOL_DELTA=REQUIRES_SOURCE_REVIEW"));

    let breaking = classify("candidate-breaking");
    assert!(breaking.contains("CLASSIFICATION=BREAKING_OR_UNPROVEN"));
    assert!(breaking.contains("PROTOCOL_DELTA=BREAKING_OR_UNPROVEN"));

    let schema = classify("candidate-schema");
    assert!(schema.contains("CLASSIFICATION=BREAKING_OR_UNPROVEN"));
    assert!(schema.contains("PROTOCOL_DELTA=BREAKING_OR_UNPROVEN"));
}
