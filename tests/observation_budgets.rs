use sha2::{Digest, Sha256};
use std::fs;
use tabbeacon::{
    interface_preferences::InterfacePreferencesStore,
    provider_observation::{hash_observation_file, read_observation_file},
    repo::{AliasRegistryHealth, StableAliasRegistry, WorkspacePreferenceStore},
    settings::PresentationSettingsStore,
};

#[test]
fn oversized_read_only_documents_fail_without_changing_transactional_reads_or_bytes() {
    let root = tempfile::tempdir().unwrap();
    let toml = format!("#{}\n", "x".repeat(300 * 1024));
    let json = format!(
        "{}{{\"schema\":\"tabbeacon-workspace-preferences-v1\",\"overrides\":{{}}}}",
        " ".repeat(300 * 1024)
    );
    let settings = PresentationSettingsStore::new(root.path().join("settings.toml"));
    let interface = InterfacePreferencesStore::new(root.path().join("interface.toml"));
    let workspace = WorkspacePreferenceStore::new(root.path().join("workspace.json"));
    fs::write(settings.path(), &toml).unwrap();
    fs::write(interface.path(), &toml).unwrap();
    fs::write(workspace.path(), &json).unwrap();
    assert!(settings.snapshot_read_only().is_ok());
    assert!(interface.snapshot_read_only().is_ok());
    assert!(workspace.snapshot_read_only().is_ok());
    assert!(settings.snapshot_read_only_controlled(None).is_err());
    assert!(interface.snapshot_read_only_controlled(None).is_err());
    assert!(workspace.snapshot_read_only_controlled(None).is_err());
    assert_eq!(fs::read(settings.path()).unwrap(), toml.as_bytes());
    assert_eq!(fs::read(interface.path()).unwrap(), toml.as_bytes());
    assert_eq!(fs::read(workspace.path()).unwrap(), json.as_bytes());
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 3);
}

#[test]
fn registry_entry_budget_is_unavailable_and_does_not_publish_or_drop_history() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..4097 {
        fs::write(root.path().join(format!("foreign-{index}")), b"").unwrap();
    }
    let registry = StableAliasRegistry::new(root.path());
    assert_eq!(
        registry.inspect_read_only().health(),
        AliasRegistryHealth::Healthy
    );
    let bounded = registry.inspect_read_only_controlled(None);
    assert_eq!(bounded.health(), AliasRegistryHealth::Unavailable);
    assert_eq!(bounded.assignment_count(), None);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 4097);
}

#[test]
fn chunked_hash_preserves_identity_and_byte_budget_rejects_growth() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("owned-fixture");
    let bytes = vec![0x42; 300 * 1024];
    fs::write(&path, &bytes).unwrap();
    assert_eq!(
        hash_observation_file(&path, None).unwrap(),
        format!("{:x}", Sha256::digest(&bytes))
    );
    assert!(read_observation_file(&path, 256 * 1024, None).is_err());
    assert_eq!(
        read_observation_file(&path, bytes.len(), None)
            .unwrap()
            .unwrap(),
        bytes
    );
}

#[test]
fn bounded_registry_preserves_normal_assignment_candidates_and_generation_bytes() {
    use std::collections::BTreeSet;
    use tabbeacon::repo::{CanonicalRepositoryIdentity, RepositoryDisplayName};
    let root = tempfile::tempdir().unwrap();
    let registry = StableAliasRegistry::new(root.path());
    let identity = CanonicalRepositoryIdentity::new("fixture:bounded-registry").unwrap();
    let display = RepositoryDisplayName::new("Bounded Registry").unwrap();
    registry.resolve(&identity, &display).unwrap();
    let before = fs::read_dir(root.path())
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            (path.clone(), fs::read(path).unwrap())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        registry.lookup_assignment_read_only(&identity).unwrap(),
        registry
            .lookup_assignment_read_only_controlled(&identity, None)
            .unwrap()
    );
    assert_eq!(
        registry
            .preview_candidates_read_only(&identity, &display, &BTreeSet::new())
            .unwrap(),
        registry
            .preview_candidates_read_only_controlled(&identity, &display, &BTreeSet::new(), None)
            .unwrap()
    );
    assert_eq!(
        registry
            .inspect_read_only_controlled(None)
            .assignment_count(),
        Some(1)
    );
    for (path, bytes) in before {
        assert_eq!(fs::read(path).unwrap(), bytes);
    }
}
