use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use tabbeacon::repo::{
    AbbreviationPolicy, CanonicalRepositoryIdentity, RepositoryDiscovery, RepositoryDisplayName,
    RepositoryIdentityResolver, StableAliasRegistry, canonicalize_repository, normalize_remote_url,
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TestRoot {
    path: PathBuf,
}

impl TestRoot {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos();
        let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "tabbeacon-g04-{label}-{}-{nonce}-{counter}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("isolated test root is created");
        Self { path }
    }

    fn child(&self, name: &str) -> PathBuf {
        self.path.join(name)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn git(cwd: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout).expect("Git test output is UTF-8")
}

fn init_repo(path: &Path, with_commit: bool) {
    fs::create_dir_all(path).expect("repository directory is created");
    git(path, &["init", "--quiet"]);
    if with_commit {
        fs::write(path.join("README.md"), "offline test repository\n")
            .expect("test file is written");
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
                "test root",
            ],
        );
    }
}

fn add_remote(repo: &Path, name: &str, url: &str) {
    git(repo, &["remote", "add", name, url]);
}

fn compile_offline_git_probe(root: &TestRoot) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("offline_git_probe.rs");
    let executable = root.child(if cfg!(windows) {
        "offline-git-probe.exe"
    } else {
        "offline-git-probe"
    });
    let compiler = env::var_os("RUSTC").map_or_else(|| "rustc".into(), PathBuf::from);
    let output = Command::new(compiler)
        .args(["--edition=2024"])
        .arg(source)
        .arg("-o")
        .arg(&executable)
        .output()
        .expect("offline Git probe compiler starts");
    assert!(
        output.status.success(),
        "offline Git probe failed to compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    executable
}

#[test]
fn discovery_executes_only_the_admitted_local_git_command_set() {
    let root = TestRoot::new("git-command-audit");
    let repo = root.child("repo");
    fs::create_dir_all(&repo).expect("probe repository directory is created");
    let probe = compile_offline_git_probe(&root);
    let discovered = RepositoryDiscovery::with_git_executable(probe)
        .discover(&repo)
        .expect("probe discovery succeeds");
    assert_eq!(discovered.worktree_root, repo);
    let audit = fs::read_to_string(root.child("git-command-audit.log"))
        .expect("probe command audit exists");
    assert_eq!(
        audit.lines().collect::<Vec<_>>(),
        [
            "rev-parse\t--path-format=absolute\t--show-toplevel\t--absolute-git-dir\t--git-common-dir",
            "config\t--local\t--null\t--get-regexp\t^remote\\..*\\.url$",
            "rev-list\t--max-parents=0\t--all",
        ]
    );
    assert!(
        audit
            .to_ascii_lowercase()
            .split_whitespace()
            .all(|word| !matches!(word, "fetch" | "pull" | "push" | "ls-remote" | "clone"))
    );
}

#[test]
fn ordinary_repository_resolves_from_nested_cwd_without_project_writes() {
    let root = TestRoot::new("ordinary");
    let repo = root.child("tabbeacon");
    let nested = repo.join("nested").join("deeper");
    init_repo(&repo, true);
    fs::create_dir_all(&nested).expect("nested cwd is created");
    add_remote(
        &repo,
        "origin",
        "https://github.com/JerrySkywalker/tabbeacon.git",
    );
    let identity_resolver = RepositoryIdentityResolver::new(root.child("state"));
    let resolved = identity_resolver
        .resolve(&nested)
        .expect("offline resolution succeeds");
    assert_eq!(
        resolved.identity.as_str(),
        "remote:github.com/JerrySkywalker/tabbeacon"
    );
    assert_eq!(resolved.display_name.as_str(), "tabbeacon");
    assert_eq!(resolved.alias.as_str(), "T");
    assert_eq!(resolved.worktree_root, repo);
    assert!(git(&resolved.worktree_root, &["status", "--porcelain"]).is_empty());
}

#[test]
fn equivalent_remote_forms_and_reclones_share_identity_and_alias() {
    let root = TestRoot::new("reclone");
    let first = root.child("first");
    let second = root.child("second");
    init_repo(&first, true);
    init_repo(&second, true);
    add_remote(
        &first,
        "origin",
        "https://github.com/JerrySkywalker/opencode-workspace-hub.git",
    );
    add_remote(
        &second,
        "origin",
        "git@github.com:JerrySkywalker/opencode-workspace-hub.git",
    );
    let resolver = RepositoryIdentityResolver::new(root.child("state"));
    let first_identity = resolver.resolve(&first).expect("first clone resolves");
    let second_identity = resolver.resolve(&second).expect("second clone resolves");
    assert_eq!(first_identity.identity, second_identity.identity);
    assert_eq!(first_identity.alias, second_identity.alias);
    assert_eq!(first_identity.alias.as_str(), "OWH");
}

#[test]
fn linked_worktrees_share_the_common_repository_identity() {
    let root = TestRoot::new("worktrees");
    let repo = root.child("ordinary");
    let linked = root.child("linked");
    init_repo(&repo, true);
    add_remote(&repo, "origin", "ssh://git@example.com/team/project.git");
    git(
        &repo,
        &["worktree", "add", "--quiet", "--detach", path_text(&linked)],
    );
    let discovery = RepositoryDiscovery::default();
    let ordinary = discovery.discover(&repo).expect("ordinary repo discovered");
    let worktree = discovery
        .discover(&linked)
        .expect("linked worktree discovered");
    assert_ne!(ordinary.git_dir, worktree.git_dir);
    assert_eq!(ordinary.git_common_dir, worktree.git_common_dir);
    let ordinary_identity = canonicalize_repository(&ordinary).expect("ordinary canonicalized");
    let worktree_identity = canonicalize_repository(&worktree).expect("worktree canonicalized");
    assert_eq!(ordinary_identity, worktree_identity);
}

#[test]
fn origin_absent_chooses_the_first_usable_remote_deterministically() {
    let root = TestRoot::new("remotes");
    let repo = root.child("repo");
    init_repo(&repo, true);
    add_remote(&repo, "zeta", "https://example.com/zeta/project.git");
    add_remote(&repo, "alpha", "ssh://git@example.com/alpha/project.git");
    let discovered = RepositoryDiscovery::default()
        .discover(&repo)
        .expect("repository discovered");
    let canonical = canonicalize_repository(&discovered).expect("repository canonicalized");
    assert_eq!(
        canonical.identity.as_str(),
        "remote:example.com/alpha/project"
    );
}

#[test]
fn originless_committed_repository_keeps_identity_after_move() {
    let root = TestRoot::new("move");
    let before = root.child("before");
    let after = root.child("after");
    init_repo(&before, true);
    let resolver = RepositoryIdentityResolver::new(root.child("state"));
    let first = resolver.resolve(&before).expect("originless repo resolves");
    assert!(first.identity.as_str().starts_with("local-roots:"));
    fs::rename(&before, &after).expect("repository directory moves locally");
    let moved = resolver.resolve(&after).expect("moved repository resolves");
    assert_eq!(first.identity, moved.identity);
    assert_eq!(first.alias, moved.alias);
}

#[test]
fn unborn_repository_fallback_is_repeatable_and_worktree_local() {
    let root = TestRoot::new("unborn");
    let repo = root.child("new-repo");
    init_repo(&repo, false);
    let resolver = RepositoryIdentityResolver::new(root.child("state"));
    let first = resolver.resolve(&repo).expect("unborn repository resolves");
    let repeated = resolver.resolve(&repo).expect("repeat resolution succeeds");
    assert!(first.identity.as_str().starts_with("local-unborn:"));
    assert_eq!(first.identity, repeated.identity);
    assert_eq!(first.alias, repeated.alias);
}

#[test]
fn unicode_names_are_safe_and_hostile_or_overlong_urls_are_rejected() {
    let root = TestRoot::new("unicode");
    let repo = root.child("工程_工作区");
    init_repo(&repo, true);
    add_remote(&repo, "origin", "https://例子.invalid/团队/工程_工作区.git");
    let identity_resolver = RepositoryIdentityResolver::new(root.child("state"));
    let resolved = identity_resolver
        .resolve(&repo)
        .expect("Unicode repository resolves");
    assert_eq!(resolved.display_name.as_str(), "工程_工作区");
    assert!(
        resolved
            .alias
            .as_str()
            .chars()
            .all(|character| character.is_alphanumeric() || character == '-')
    );
    assert!(normalize_remote_url("https://example.invalid/repo\nname", &repo).is_err());
    let overlong = format!("https://example.invalid/{}", "a".repeat(5000));
    assert!(normalize_remote_url(&overlong, &repo).is_err());
}

#[test]
fn abbreviation_collision_expands_only_the_newcomer_and_has_hash_fallback() {
    let root = TestRoot::new("collisions");
    let registry = StableAliasRegistry::new(root.child("state"));
    let first = CanonicalRepositoryIdentity::new("remote:example/jerry-proxy-control")
        .expect("valid identity");
    let second = CanonicalRepositoryIdentity::new("remote:example/java-platform-core")
        .expect("valid identity");
    let first_name = RepositoryDisplayName::new("jerry-proxy-control").expect("valid name");
    let second_name = RepositoryDisplayName::new("java-platform-core").expect("valid name");
    let old = registry
        .resolve(&first, &first_name)
        .expect("first assignment");
    let newcomer = registry
        .resolve(&second, &second_name)
        .expect("new collision assignment");
    assert_eq!(old.as_str(), "JPC");
    assert_ne!(newcomer, old);
    assert_eq!(registry.lookup(&first).expect("lookup succeeds"), Some(old));

    let hash_identity =
        CanonicalRepositoryIdentity::new("remote:example/hash-fallback").expect("valid identity");
    let candidates = AbbreviationPolicy::candidates(&first_name, &hash_identity);
    let hash = candidates
        .iter()
        .find(|candidate| candidate.as_str().contains('-'))
        .expect("stable hash fallback exists");
    assert_eq!(
        hash,
        AbbreviationPolicy::candidates(&first_name, &hash_identity)
            .iter()
            .find(|candidate| candidate.as_str().contains('-'))
            .expect("hash fallback repeats")
    );

    let single_token = RepositoryDisplayName::new("x").expect("valid single-token name");
    let single_first = CanonicalRepositoryIdentity::new("remote:example/single-first")
        .expect("valid single-token identity");
    let single_second = CanonicalRepositoryIdentity::new("remote:example/single-second")
        .expect("valid colliding identity");
    assert_eq!(
        registry
            .resolve(&single_first, &single_token)
            .expect("base single-token alias")
            .as_str(),
        "X"
    );
    let assigned_hash = registry
        .resolve(&single_second, &single_token)
        .expect("hash fallback is assigned after readable exhaustion");
    assert!(assigned_hash.as_str().starts_with("X-"));
}

#[test]
fn corrupt_latest_snapshot_recovers_from_prior_valid_generation() {
    let root = TestRoot::new("recovery");
    let state = root.child("state");
    let registry = StableAliasRegistry::new(&state);
    let first = CanonicalRepositoryIdentity::new("remote:example/first").expect("valid identity");
    let first_name = RepositoryDisplayName::new("first").expect("valid name");
    let original = registry
        .resolve(&first, &first_name)
        .expect("valid generation");
    let corrupt_name = format!("registry-v1-{:020}-{}.json", 2, "a".repeat(64));
    fs::write(state.join(corrupt_name), b"partial-corrupt-snapshot")
        .expect("corrupt later snapshot is created");
    assert_eq!(
        registry.lookup(&first).expect("older generation recovers"),
        Some(original)
    );
    let second =
        CanonicalRepositoryIdentity::new("remote:example/second").expect("valid second identity");
    let second_name = RepositoryDisplayName::new("second").expect("valid second name");
    registry
        .resolve(&second, &second_name)
        .expect("new atomic generation skips the occupied corrupt generation");
    assert!(
        fs::read_dir(&state)
            .expect("state directory reads")
            .filter_map(Result::ok)
            .all(|entry| !entry.file_name().to_string_lossy().ends_with(".tmp"))
    );
}

#[test]
fn only_corrupt_published_state_fails_closed_without_overwrite() {
    let root = TestRoot::new("corrupt-only");
    let state = root.child("state");
    fs::create_dir_all(&state).expect("state root is created");
    let corrupt = state.join(format!("registry-v1-{:020}-{}.json", 1, "b".repeat(64)));
    fs::write(&corrupt, b"not a valid registry").expect("corrupt state is written");
    let registry = StableAliasRegistry::new(&state);
    let identity = CanonicalRepositoryIdentity::new("remote:example/repo").expect("valid key");
    let name = RepositoryDisplayName::new("repo").expect("valid name");
    assert!(registry.resolve(&identity, &name).is_err());
    assert_eq!(
        fs::read(&corrupt).expect("corrupt state remains untouched"),
        b"not a valid registry"
    );
}

#[test]
fn concurrent_first_registration_is_process_safe_and_stable() {
    let root = TestRoot::new("process-race");
    let state = root.child("state");
    let barrier = root.child("go");
    let mut children = Vec::new();
    let process_count = 10_usize;
    for index in 0..process_count {
        let output = root.child(&format!("alias-{index}.txt"));
        let identity = if index < 3 {
            "remote:example/shared".to_owned()
        } else {
            format!("remote:example/collision-{index}")
        };
        let mut child = Command::new(env::current_exe().expect("test executable is known"));
        child
            .args([
                "--ignored",
                "--exact",
                "registry_process_helper",
                "--nocapture",
                "--test-threads=1",
            ])
            .env("TABBEACON_G04_HELPER", "1")
            .env("TABBEACON_G04_STATE", &state)
            .env("TABBEACON_G04_BARRIER", &barrier)
            .env("TABBEACON_G04_IDENTITY", identity)
            .env("TABBEACON_G04_DISPLAY", "jerry-proxy-control")
            .env("TABBEACON_G04_OUTPUT", output)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        children.push(child.spawn().expect("registry helper starts"));
    }
    fs::write(&barrier, b"go").expect("concurrency barrier is released");
    for child in children {
        let output = child
            .wait_with_output()
            .expect("registry helper completion is observed");
        assert!(
            output.status.success(),
            "registry helper failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let aliases = (0..process_count)
        .map(|index| {
            fs::read_to_string(root.child(&format!("alias-{index}.txt")))
                .expect("helper alias output exists")
        })
        .collect::<Vec<_>>();
    assert!(aliases[..3].windows(2).all(|pair| pair[0] == pair[1]));
    let distinct = aliases[3..].iter().collect::<BTreeSet<_>>();
    assert_eq!(distinct.len(), process_count - 3);
    let all_aliases = aliases.iter().collect::<BTreeSet<_>>();
    assert_eq!(all_aliases.len(), process_count - 2);

    let registry = StableAliasRegistry::new(&state);
    let shared = CanonicalRepositoryIdentity::new("remote:example/shared")
        .expect("shared identity is valid");
    assert_eq!(
        registry.lookup(&shared).expect("post-race lookup succeeds"),
        Some(
            tabbeacon::repo::RepositoryAlias::new(aliases[0].clone())
                .expect("helper alias is valid")
        )
    );
}

#[test]
#[ignore = "spawned only by the process-concurrency acceptance test"]
fn registry_process_helper() {
    if env::var_os("TABBEACON_G04_HELPER").is_none() {
        return;
    }
    let state = required_env_path("TABBEACON_G04_STATE");
    let barrier = required_env_path("TABBEACON_G04_BARRIER");
    let output = required_env_path("TABBEACON_G04_OUTPUT");
    let identity = env::var("TABBEACON_G04_IDENTITY").expect("helper identity is present");
    let display = env::var("TABBEACON_G04_DISPLAY").expect("helper display is present");
    let deadline = Instant::now() + Duration::from_secs(15);
    while !barrier.is_file() {
        assert!(Instant::now() < deadline, "helper barrier timed out");
        thread::sleep(Duration::from_millis(5));
    }
    let registry = StableAliasRegistry::new(state);
    let identity = CanonicalRepositoryIdentity::new(identity).expect("helper identity is valid");
    let display = RepositoryDisplayName::new(display).expect("helper display is valid");
    let alias = registry
        .resolve(&identity, &display)
        .expect("helper assignment succeeds");
    fs::write(output, alias.as_str()).expect("helper output is durable");
}

fn required_env_path(name: &str) -> PathBuf {
    env::var_os(name).map_or_else(
        || panic!("required helper environment variable is missing: {name}"),
        PathBuf::from,
    )
}

fn path_text(path: &Path) -> &str {
    path.to_str().expect("test path is Unicode")
}
