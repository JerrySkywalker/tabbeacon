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
    assert!(classify("candidate-safe").contains("CLASSIFICATION=SAFE_COMPATIBLE"));
    assert!(classify("candidate-review").contains("CLASSIFICATION=REQUIRES_REVIEW"));
    assert!(classify("candidate-breaking").contains("CLASSIFICATION=BREAKING_OR_UNPROVEN"));
}
