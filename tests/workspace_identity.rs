use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use tabbeacon::repo::{
    RepositoryAlias, RepositoryIdentityResolver, WorkspaceAliasError, WorkspaceIdentityResolver,
    WorkspaceKind,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "tabbeacon-g10a-{label}-{}-{nonce}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("isolated test root is created");
        Self(path)
    }

    fn child(&self, name: impl AsRef<Path>) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(cwd: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .output()
        .expect("local Git executable starts");
    assert!(
        output.status.success(),
        "local Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_repo(path: &Path, remote: Option<&str>) {
    fs::create_dir_all(path).expect("repository directory is created");
    git(path, &["init", "--quiet"]);
    fs::write(path.join("README.md"), "workspace identity test\n").expect("fixture writes");
    git(path, &["add", "README.md"]);
    git(
        path,
        &[
            "-c",
            "user.name=TabBeacon Test",
            "-c",
            "user.email=tabbeacon@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        ],
    );
    if let Some(remote) = remote {
        git(path, &["remote", "add", "origin", remote]);
    }
}

#[test]
fn git_identity_alias_linked_worktree_and_originless_behavior_are_unchanged() {
    let root = TestRoot::new("git-compatibility");
    let state = root.child("state");
    let repo = root.child("repo");
    let linked = root.child("linked");
    init_repo(
        &repo,
        Some("https://github.com/JerrySkywalker/opencode-workspace-hub.git"),
    );
    git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            linked.to_str().expect("linked path is Unicode"),
        ],
    );
    let legacy = RepositoryIdentityResolver::new(&state)
        .resolve(&repo)
        .expect("legacy Git identity resolves");
    let resolver = WorkspaceIdentityResolver::with_home_directory(&state, None);
    let workspace = resolver.resolve(&repo).expect("Git workspace resolves");
    let linked_workspace = resolver
        .resolve(&linked)
        .expect("linked workspace resolves");
    assert_eq!(workspace.kind, WorkspaceKind::Git);
    assert_eq!(workspace.identity, legacy.identity);
    assert_eq!(workspace.display_name, legacy.display_name);
    assert_eq!(workspace.alias, legacy.alias);
    assert_eq!(linked_workspace.identity, workspace.identity);
    assert_eq!(linked_workspace.alias, workspace.alias);

    let originless = root.child("originless");
    init_repo(&originless, None);
    let old_originless = RepositoryIdentityResolver::new(&state)
        .resolve(&originless)
        .expect("legacy originless identity resolves");
    let new_originless = resolver
        .resolve(&originless)
        .expect("originless workspace resolves");
    assert_eq!(new_originless.identity, old_originless.identity);
    assert_eq!(new_originless.alias, old_originless.alias);
}

#[test]
fn workspace_identity_fingerprint_observes_an_alternate_cwd_without_registry_writes() {
    let root = TestRoot::new("anchor-fingerprint");
    let state = root.child("state");
    let alternate = root.child("temporary-worktree");
    init_repo(
        &alternate,
        Some("https://example.invalid/team/temporary-worktree.git"),
    );
    let resolver = WorkspaceIdentityResolver::with_home_directory(&state, None);

    let fingerprint = resolver
        .workspace_identity_sha256(&alternate)
        .expect("alternate workspace identity is observable without assignment");
    assert_eq!(fingerprint.len(), 64);
    assert!(
        !state.exists(),
        "a non-authoritative cwd observation must not allocate local alias state"
    );
}

#[test]
fn ordinary_directory_is_stable_opaque_and_creates_no_local_marker() {
    let root = TestRoot::new("ordinary");
    let state = root.child("state");
    let workspace = root.child("ordinary-workspace");
    fs::create_dir_all(&workspace).expect("ordinary workspace is created");
    let resolver = WorkspaceIdentityResolver::with_home_directory(&state, None);
    let first = resolver.resolve(&workspace).expect("directory resolves");
    let repeated = resolver.resolve(&workspace).expect("directory repeats");
    assert_eq!(first.kind, WorkspaceKind::Directory);
    assert_eq!(first.identity, repeated.identity);
    assert_eq!(first.alias, repeated.alias);
    assert!(first.identity.as_str().starts_with("dir-v1:"));
    assert_eq!(first.identity.as_str().len(), "dir-v1:".len() + 64);
    assert!(
        !first
            .identity
            .as_str()
            .contains(&workspace.to_string_lossy().to_string())
    );
    assert!(
        fs::read_dir(&workspace)
            .expect("workspace reads")
            .next()
            .is_none()
    );

    let raw = workspace.to_string_lossy().replace('\\', "/");
    for entry in fs::read_dir(&state).expect("shared registry reads") {
        let entry = entry.expect("registry entry reads");
        if entry.file_type().expect("entry type reads").is_file() {
            let bytes = fs::read(entry.path()).expect("registry file reads");
            assert!(
                !String::from_utf8_lossy(&bytes)
                    .replace('\\', "/")
                    .contains(&raw)
            );
        }
    }
}

#[test]
fn home_and_filesystem_root_receive_safe_local_hints() {
    let root = TestRoot::new("special-roots");
    let home = root.child("owner-home");
    fs::create_dir_all(&home).expect("home fixture is created");
    let resolver =
        WorkspaceIdentityResolver::with_home_directory(root.child("state"), Some(home.clone()));
    let resolved_home = resolver.resolve(&home).expect("home resolves");
    assert_eq!(resolved_home.display_name.as_str(), "HOME");
    assert!(resolved_home.alias.as_str().starts_with('H'));

    let filesystem_root = root
        .0
        .ancestors()
        .last()
        .expect("temporary path has a filesystem root");
    let resolved_root = resolver
        .resolve(filesystem_root)
        .expect("filesystem root resolves");
    assert_eq!(resolved_root.kind, WorkspaceKind::Directory);
    assert!(resolved_root.display_name.as_str().ends_with("ROOT"));
    assert!(
        resolved_root
            .alias
            .as_str()
            .chars()
            .all(|character| character.is_alphanumeric() || character == '-')
    );
}

#[test]
fn directory_and_git_collisions_share_one_alias_namespace() {
    let root = TestRoot::new("shared-collisions");
    let state = root.child("state");
    let git_workspace = root.child("git-workspace");
    init_repo(
        &git_workspace,
        Some("https://example.invalid/team/shared-name.git"),
    );
    let resolver = WorkspaceIdentityResolver::with_home_directory(&state, None);
    let git_identity = resolver
        .resolve(&git_workspace)
        .expect("Git workspace resolves");

    let first = root.child("first/shared-name");
    let second = root.child("second/shared-name");
    fs::create_dir_all(&first).expect("first directory is created");
    fs::create_dir_all(&second).expect("second directory is created");
    let first_identity = resolver.resolve(&first).expect("first directory resolves");
    let second_identity = resolver
        .resolve(&second)
        .expect("second directory resolves");
    assert_ne!(first_identity.identity, second_identity.identity);
    assert_ne!(first_identity.alias, second_identity.alias);
    assert_ne!(git_identity.alias, first_identity.alias);
    assert_ne!(git_identity.alias, second_identity.alias);
    assert_eq!(
        resolver
            .resolve(&git_workspace)
            .expect("existing Git alias remains stable")
            .alias,
        git_identity.alias
    );
}

#[test]
fn unicode_long_and_hostile_directory_hints_remain_presentation_safe() {
    let root = TestRoot::new("hostile-hints");
    let state = root.child("state");
    let unicode = root.child("工程 工作区");
    let hostile = root.child(format!("{} ]0; beacon ()[]", "LongWorkspace".repeat(7)));
    fs::create_dir_all(&unicode).expect("Unicode directory is created");
    fs::create_dir_all(&hostile).expect("hostile directory is created");
    let resolver = WorkspaceIdentityResolver::with_home_directory(&state, None);
    for resolved in [
        resolver
            .resolve(&unicode)
            .expect("Unicode directory resolves"),
        resolver
            .resolve(&hostile)
            .expect("hostile directory resolves"),
    ] {
        assert!(resolved.identity.as_str().starts_with("dir-v1:"));
        assert!(resolved.alias.as_str().chars().count() <= 20);
        assert!(
            resolved
                .alias
                .as_str()
                .chars()
                .all(|character| character.is_alphanumeric() || character == '-')
        );
        assert!(!resolved.alias.as_str().contains(']'));
        assert!(!resolved.alias.as_str().contains(';'));
    }
}

#[test]
fn alias_inspection_is_passive_and_set_reset_preserve_generated_history() {
    let root = TestRoot::new("alias-preferences");
    let state = root.child("state");
    let workspace = root.child("ordinary-workspace");
    fs::create_dir_all(&workspace).expect("ordinary workspace is created");
    let resolver = WorkspaceIdentityResolver::with_home_directory(&state, None);

    let preview = resolver
        .inspect_alias(&workspace)
        .expect("passive alias inspection succeeds");
    assert!(!preview.is_assigned());
    assert!(
        !state.exists(),
        "inspection must not create a registry root"
    );
    assert!(
        !root.child("workspace-preferences").exists(),
        "inspection must not create a preference root or lock"
    );

    let saved = resolver
        .set_alias_override(&workspace, "CUSTOM")
        .expect("explicit alias saves");
    assert!(saved.is_assigned());
    assert_eq!(
        saved.custom_alias().map(RepositoryAlias::as_str),
        Some("CUSTOM")
    );
    assert_eq!(saved.effective_alias().as_str(), "CUSTOM");
    let generated = saved.automatic_alias().clone();
    let runtime_identity = resolver
        .resolve(&workspace)
        .expect("runtime resolve succeeds");
    assert_eq!(
        runtime_identity.alias, generated,
        "compatibility alias remains generated"
    );
    assert_eq!(runtime_identity.effective_alias.as_str(), "CUSTOM");

    let reset = resolver
        .reset_alias_override(&workspace)
        .expect("override reset succeeds");
    assert_eq!(reset.custom_alias(), None);
    assert_eq!(reset.automatic_alias(), &generated);
    assert_eq!(reset.effective_alias(), &generated);
    assert_eq!(
        resolver
            .resolve(&workspace)
            .expect("runtime resolve after reset")
            .effective_alias,
        generated
    );
}

#[test]
fn alias_override_collision_is_generic_and_preserves_both_workspaces() {
    let root = TestRoot::new("alias-collision");
    let state = root.child("state");
    let first = root.child("first");
    let second = root.child("second");
    fs::create_dir_all(&first).expect("first workspace is created");
    fs::create_dir_all(&second).expect("second workspace is created");
    let resolver = WorkspaceIdentityResolver::with_home_directory(&state, None);

    resolver
        .set_alias_override(&first, "CUSTOM")
        .expect("first alias saves");
    assert_eq!(
        resolver.set_alias_override(&second, "CUSTOM"),
        Err(WorkspaceAliasError::Collision)
    );
    let first_view = resolver.inspect_alias(&first).expect("first inspection");
    let second_view = resolver.inspect_alias(&second).expect("second inspection");
    assert_eq!(
        first_view.custom_alias().map(RepositoryAlias::as_str),
        Some("CUSTOM")
    );
    assert_eq!(second_view.custom_alias(), None);
    assert_ne!(first_view.effective_alias(), second_view.effective_alias());
}

#[test]
fn automatic_registry_allocation_reserves_existing_local_overrides() {
    let root = TestRoot::new("override-reservation");
    let state = root.child("state");
    let directory = root.child("directory");
    let repository = root.child("repository");
    fs::create_dir_all(&directory).expect("directory workspace is created");
    init_repo(&repository, Some("https://example.invalid/team/custom.git"));
    let workspace_resolver = WorkspaceIdentityResolver::with_home_directory(&state, None);
    workspace_resolver
        .set_alias_override(&directory, "CUSTOM")
        .expect("directory override saves");

    let generated = RepositoryIdentityResolver::new(&state)
        .resolve(&repository)
        .expect("automatic repository alias resolves");
    assert_ne!(
        generated.alias.as_str(),
        "CUSTOM",
        "automatic allocation must reserve every explicit local override"
    );
}

#[test]
fn linked_worktrees_share_one_device_local_alias_override() {
    let root = TestRoot::new("linked-override");
    let state = root.child("state");
    let repo = root.child("repo");
    let linked = root.child("linked");
    init_repo(&repo, Some("https://example.invalid/team/linked.git"));
    git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            linked.to_str().expect("linked path is Unicode"),
        ],
    );
    let resolver = WorkspaceIdentityResolver::with_home_directory(&state, None);

    resolver
        .set_alias_override(&repo, "LINKED")
        .expect("main worktree alias saves");
    let linked_view = resolver.inspect_alias(&linked).expect("linked inspection");
    assert_eq!(
        linked_view.custom_alias().map(RepositoryAlias::as_str),
        Some("LINKED")
    );
    assert_eq!(linked_view.effective_alias().as_str(), "LINKED");
}

#[test]
fn custom_alias_validation_uses_nfc_and_display_width_bounds() {
    let root = TestRoot::new("alias-validation");
    let workspace = root.child("workspace");
    fs::create_dir_all(&workspace).expect("workspace is created");
    let resolver = WorkspaceIdentityResolver::with_home_directory(root.child("state"), None);

    let nfc = resolver
        .set_alias_override(&workspace, "e\u{301}")
        .expect("NFC-composable alias is accepted");
    assert_eq!(nfc.effective_alias().as_str(), "é");
    assert_eq!(
        resolver.set_alias_override(&workspace, "中".repeat(11)),
        Err(WorkspaceAliasError::InvalidAlias)
    );
    assert_eq!(
        resolver.set_alias_override(&workspace, "unsafe!"),
        Err(WorkspaceAliasError::InvalidAlias)
    );
}
