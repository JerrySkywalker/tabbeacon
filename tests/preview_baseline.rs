//! Real transactions on explicitly injected stores. No environment-derived stores.
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Barrier},
    thread,
};
use tabbeacon::{
    repo::{WorkspaceAliasError, WorkspaceIdentityResolver},
    windows_terminal_policy::{
        SettingsCandidate, TitleRemediationState, WindowsTerminalInstallation,
        WindowsTerminalPolicyStore,
    },
};

fn owned_root() -> tempfile::TempDir {
    let mut builder = tempfile::Builder::new();
    builder.prefix("preview-baseline-");
    if let Some(goal) = std::env::var_os("TB_ENGINEERING_ARTIFACT_ROOT") {
        let path = PathBuf::from(goal).join("preview-stores");
        fs::create_dir_all(&path).unwrap();
        builder.tempdir_in(path).unwrap()
    } else {
        builder.tempdir().unwrap()
    }
}
const PROFILE: &str = "{11111111-1111-1111-1111-111111111111}";
const DOCUMENT: &str = r#"{
  // unrelated bytes must survive
  "profiles": { "list": [{ "guid": "{11111111-1111-1111-1111-111111111111}", "suppressApplicationTitle": true }] },
  "unrelated": { "value": 42 }
}"#;
fn repair_store(root: &std::path::Path) -> WindowsTerminalPolicyStore {
    WindowsTerminalPolicyStore::new_for_testing(
        vec![SettingsCandidate::new(
            WindowsTerminalInstallation::Stable,
            root.join("settings.json"),
        )],
        root.join("receipt"),
        true,
        Some(PROFILE.to_owned()),
    )
}

#[test]
fn repair_original_preview_normal_confirmation_and_ownership_restore() {
    let root = owned_root();
    fs::write(root.path().join("settings.json"), DOCUMENT).unwrap();
    let store = repair_store(root.path());
    let expected = store
        .preview_repair("terminal.title_policy_repair")
        .unwrap();
    assert!(
        !root.path().join("receipt").exists(),
        "preview is read-only"
    );
    assert_eq!(
        store.repair_if_unchanged(&expected).unwrap().state,
        TitleRemediationState::Available
    );
    assert_eq!(
        fs::read_to_string(root.path().join("settings.json")).unwrap(),
        DOCUMENT.replace(": true", ": false")
    );
    assert_eq!(
        store.inspect().remediation,
        TitleRemediationState::AlreadyOwned
    );
    assert_eq!(
        store.repair_if_unchanged(&expected).unwrap().state,
        TitleRemediationState::BlockedDrift
    );
    store.restore().unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("settings.json")).unwrap(),
        DOCUMENT
    );
}

#[test]
fn repair_preview_drift_identity_action_and_receipt_are_bound() {
    let root = owned_root();
    let path = root.path().join("settings.json");
    fs::write(&path, DOCUMENT).unwrap();
    let store = repair_store(root.path());
    let expected = store
        .preview_repair("terminal.title_policy_repair")
        .unwrap();
    assert!(store.preview_repair("hooks.trust").is_err());
    fs::write(&path, format!("{DOCUMENT}\n")).unwrap();
    assert_eq!(
        store.repair_if_unchanged(&expected).unwrap().state,
        TitleRemediationState::BlockedDrift
    );
    assert!(!root.path().join("receipt").exists());
    fs::write(&path, DOCUMENT).unwrap();
    let other = owned_root();
    fs::write(other.path().join("settings.json"), DOCUMENT).unwrap();
    assert_eq!(
        repair_store(other.path())
            .repair_if_unchanged(&expected)
            .unwrap()
            .state,
        TitleRemediationState::BlockedDrift
    );
    let wrong_profile = WindowsTerminalPolicyStore::new_for_testing(
        vec![SettingsCandidate::new(
            WindowsTerminalInstallation::Stable,
            &path,
        )],
        root.path().join("receipt"),
        true,
        Some("{22222222-2222-2222-2222-222222222222}".into()),
    );
    assert!(
        !wrong_profile
            .repair_if_unchanged(&expected)
            .unwrap()
            .document_modified
    );
    fs::create_dir(root.path().join("receipt")).unwrap();
    fs::write(
        root.path()
            .join("receipt/windows-terminal-title-policy-v1.json"),
        b"{}",
    )
    .unwrap();
    assert_eq!(
        store.repair_if_unchanged(&expected).unwrap().state,
        TitleRemediationState::BlockedDrift
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), DOCUMENT);
}

#[test]
fn two_cooperating_repair_writers_have_one_conditional_commit() {
    let root = owned_root();
    fs::write(root.path().join("settings.json"), DOCUMENT).unwrap();
    let store = repair_store(root.path());
    let expected = store
        .preview_repair("terminal.title_policy_repair")
        .unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let writers: Vec<_> = (0..2)
        .map(|_| {
            let store = store.clone();
            let expected = expected.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                store.repair_if_unchanged(&expected).unwrap().state
            })
        })
        .collect();
    barrier.wait();
    let states: Vec<_> = writers
        .into_iter()
        .map(|writer| writer.join().unwrap())
        .collect();
    assert_eq!(
        states
            .iter()
            .filter(|state| **state == TitleRemediationState::Available)
            .count(),
        1
    );
    assert_eq!(
        states
            .iter()
            .filter(|state| **state == TitleRemediationState::BlockedDrift)
            .count(),
        1
    );
    assert_eq!(
        store.inspect().remediation,
        TitleRemediationState::AlreadyOwned
    );
}

#[test]
fn receipt_write_failure_and_receipt_config_inconsistency_are_controlled() {
    let root = owned_root();
    let path = root.path().join("settings.json");
    fs::write(&path, DOCUMENT).unwrap();
    let store = repair_store(root.path());
    let expected = store
        .preview_repair("terminal.title_policy_repair")
        .unwrap();
    fs::write(root.path().join("receipt"), b"parent obstruction").unwrap();
    assert!(store.repair_if_unchanged(&expected).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), DOCUMENT);
    fs::remove_file(root.path().join("receipt")).unwrap();
    assert!(
        store
            .repair_if_unchanged(&expected)
            .unwrap()
            .document_modified
    );
    fs::write(
        &path,
        DOCUMENT.replace("42", "43").replace(": true", ": false"),
    )
    .unwrap();
    // Unrelated change outside the owned profile is preserved by restore.
    store.restore().unwrap();
    assert!(fs::read_to_string(&path).unwrap().contains("43"));
    let expected = store
        .preview_repair("terminal.title_policy_repair")
        .unwrap();
    store.repair_if_unchanged(&expected).unwrap();
    fs::write(
        &path,
        DOCUMENT
            .replace(": true", ": false")
            .replace(PROFILE, "{22222222-2222-2222-2222-222222222222}"),
    )
    .unwrap();
    assert!(
        !store.restore().unwrap().document_modified,
        "receipt mismatch must not authorize restore"
    );
}

#[test]
fn workspace_snapshot_absence_drift_wrong_path_and_reset() {
    let root = owned_root();
    let cwd = root.path().join("workspace");
    fs::create_dir(&cwd).unwrap();
    let resolver = WorkspaceIdentityResolver::with_preference_path(
        root.path().join("registry"),
        root.path().join("preferences.json"),
        None,
    );
    let preview = resolver.inspect_alias(&cwd).unwrap();
    let expected = preview.expected_baseline().unwrap();
    assert!(!root.path().join("preferences.json").exists());
    resolver
        .set_alias_override_if_unchanged(&cwd, "ALPHA", expected)
        .unwrap();
    assert_eq!(
        resolver
            .set_alias_override_if_unchanged(&cwd, "BETA", expected)
            .unwrap_err(),
        WorkspaceAliasError::Conflict
    );
    let current = resolver.inspect_alias(&cwd).unwrap();
    let expected = current.expected_baseline().unwrap();
    let other = root.path().join("other");
    fs::create_dir(&other).unwrap();
    assert_eq!(
        resolver
            .reset_alias_override_if_unchanged(&other, expected)
            .unwrap_err(),
        WorkspaceAliasError::Conflict
    );
    let wrong_store = WorkspaceIdentityResolver::with_preference_path(
        root.path().join("registry"),
        root.path().join("other-preferences.json"),
        None,
    );
    assert_eq!(
        wrong_store
            .set_alias_override_if_unchanged(&cwd, "BETA", expected)
            .unwrap_err(),
        WorkspaceAliasError::Conflict
    );
    fs::remove_file(root.path().join("preferences.json")).unwrap();
    assert_eq!(
        resolver
            .reset_alias_override_if_unchanged(&cwd, expected)
            .unwrap_err(),
        WorkspaceAliasError::Conflict
    );
    let preview = resolver.inspect_alias(&cwd).unwrap();
    resolver
        .set_alias_override_if_unchanged(&cwd, "GAMMA", preview.expected_baseline().unwrap())
        .unwrap();
    let preview = resolver.inspect_alias(&cwd).unwrap();
    resolver
        .reset_alias_override_if_unchanged(&cwd, preview.expected_baseline().unwrap())
        .unwrap();
    assert!(
        resolver
            .inspect_alias(&cwd)
            .unwrap()
            .custom_alias()
            .is_none()
    );
}

#[test]
fn two_cooperating_workspace_writers_have_one_conditional_commit() {
    let root = owned_root();
    let cwd = root.path().join("workspace");
    fs::create_dir(&cwd).unwrap();
    let resolver = WorkspaceIdentityResolver::with_preference_path(
        root.path().join("registry"),
        root.path().join("preferences.json"),
        None,
    );
    let expected = resolver
        .inspect_alias(&cwd)
        .unwrap()
        .expected_baseline()
        .unwrap()
        .clone();
    let barrier = Arc::new(Barrier::new(3));
    let writers: Vec<_> = ["ALPHA", "BETA"]
        .into_iter()
        .map(|alias| {
            let resolver = resolver.clone();
            let cwd = cwd.clone();
            let expected = expected.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                resolver.set_alias_override_if_unchanged(&cwd, alias, &expected)
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = writers
        .into_iter()
        .map(|writer| writer.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(WorkspaceAliasError::Conflict)))
            .count(),
        1
    );
}
