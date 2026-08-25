#![cfg(windows)]

//! Real Windows cutover regression for package-installed MCP children.
//!
//! Every binary, Cargo home, Codex configuration, lease root, and child in
//! this file lives below one disposable test directory.  It never reads or
//! writes the Owner's Cargo/Codex state.

use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::{Value, json};
use tabbeacon::providers::codex::{CodexIntegration, SetupOutcome};
use toml_edit::{Array, DocumentMut, Item, Table, value};

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock after epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "tabbeacon-upgrade-safe-mcp-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("isolated fixture root creates");
        Self(path)
    }

    fn cargo_home(&self) -> PathBuf {
        self.0.join("cargo-home")
    }

    fn installed_binary(&self) -> PathBuf {
        self.cargo_home().join("bin").join("tabbeacon.exe")
    }

    fn local_appdata(&self) -> PathBuf {
        self.0.join("local-appdata")
    }

    fn codex_home(&self) -> PathBuf {
        self.0.join("codex-home")
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct OwnedChildren(Vec<Child>);

impl OwnedChildren {
    fn push(&mut self, child: Child) {
        self.0.push(child);
    }

    fn wait_for_exit(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if self.0.iter_mut().all(|child| {
                child
                    .try_wait()
                    .expect("fixture child state reads")
                    .is_some()
            }) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "an exact MCP child did not exit within the bounded drain window"
            );
            thread::sleep(Duration::from_millis(25));
        }
    }
}

impl Drop for OwnedChildren {
    fn drop(&mut self) {
        for child in &mut self.0 {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn compile_codex_0149_probe(root: &TestRoot) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("codex_version_probe_0149.rs");
    let output = root.0.join("codex-version-probe.exe");
    let compiler = env::var_os("RUSTC").map_or_else(|| "rustc".into(), PathBuf::from);
    let result = Command::new(compiler)
        .args(["--edition=2024"])
        .arg(source)
        .arg("-o")
        .arg(&output)
        .output()
        .expect("fixture compiler starts");
    assert!(
        result.status.success(),
        "fixture compiler failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    output
}

fn install_exact_mcp_fixture(root: &TestRoot) -> PathBuf {
    let installed = root.installed_binary();
    fs::create_dir_all(installed.parent().expect("fixture Cargo bin parent"))
        .expect("fixture Cargo bin creates");
    fs::copy(PathBuf::from(env!("CARGO_BIN_EXE_tabbeacon")), &installed)
        .expect("candidate package binary copies");
    let integration = CodexIntegration::new(
        root.codex_home(),
        root.local_appdata()
            .join("TabBeacon")
            .join("codex-integration"),
        &installed,
    )
    .with_codex_program(compile_codex_0149_probe(root));
    assert_eq!(
        integration
            .setup()
            .expect("isolated compatible fresh setup succeeds"),
        SetupOutcome::InstalledTrustReviewRequired
    );
    install_exact_existing_hybrid(root, &integration);
    assert!(
        integration.mcp_runtime_lease_authority().is_ok(),
        "fixture owns an exact MCP declaration without modifying Hook trust"
    );
    installed
}

/// Builds only a fixture for a predecessor-owned hybrid declaration. Fresh
/// compatible setup remains command-only; the fixture is needed here because
/// the upgrade drain is intentionally applicable only to an already exact MCP
/// transport.
fn install_exact_existing_hybrid(root: &TestRoot, integration: &CodexIntegration) {
    let hooks_path = root.codex_home().join("hooks.json");
    let state_root = root
        .local_appdata()
        .join("TabBeacon")
        .join("codex-integration");
    let manifest_path = state_root.join("integration-v1.json");
    let config_path = root.codex_home().join("config.toml");
    let mut hooks: Value =
        serde_json::from_slice(&fs::read(&hooks_path).expect("fresh hooks read"))
            .expect("fresh hooks parse");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("fresh manifest read"))
            .expect("fresh manifest parse");
    let session_end = manifest["hooks"]
        .as_array()
        .expect("fresh manifest hooks are an array")
        .iter()
        .find(|declaration| declaration["event"] == "SessionEnd")
        .cloned()
        .expect("fresh command profile has SessionEnd");
    for declaration in manifest["hooks"]
        .as_array()
        .expect("fresh manifest hooks are an array")
    {
        let event = declaration["event"]
            .as_str()
            .expect("fresh declaration event is a string");
        let group = &declaration["group"];
        hooks["hooks"][event]
            .as_array_mut()
            .expect("fresh Hook event is an array")
            .retain(|candidate| candidate != group);
    }

    let hybrid = exact_hybrid_declarations(session_end);
    for declaration in &hybrid {
        let event = declaration["event"]
            .as_str()
            .expect("hybrid event is a string");
        hooks["hooks"]
            .as_object_mut()
            .expect("Hook document root is an object")
            .entry(event.to_owned())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .expect("hybrid Hook event is an array")
            .push(declaration["group"].clone());
    }
    let executable = manifest["executable"].clone();
    manifest["hooks"] = Value::Array(hybrid);
    manifest["mcp_server"] = json!({
        "name": "tabbeacon-hook",
        "command": executable,
        "args": ["__mcp-hook-stdio-v1"],
        "env_vars": ["WT_SESSION"],
        "omit_tools_from": ["code_mode", "deferred", "direct"]
    });

    let config = exact_hybrid_config(
        &config_path,
        manifest["mcp_server"]["command"]
            .as_str()
            .expect("hybrid command is a string"),
    );

    fs::write(
        &hooks_path,
        serde_json::to_vec_pretty(&hooks).expect("hybrid hooks serialize"),
    )
    .expect("hybrid hooks write");
    fs::write(&config_path, config).expect("hybrid config write");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("hybrid manifest serialize"),
    )
    .expect("hybrid manifest write");
    assert!(
        integration.mcp_runtime_lease_authority().is_ok(),
        "the fixture must model only an exact manifest-owned MCP transport"
    );
}

fn exact_hybrid_config(config_path: &Path, command: &str) -> String {
    let mut config: DocumentMut = fs::read_to_string(config_path)
        .expect("fresh config read")
        .parse()
        .expect("fresh config parses");
    if config.get("mcp_servers").is_none() {
        config["mcp_servers"] = Item::Table(Table::new());
    }
    let mut server = Table::new();
    server.insert("command", value(command));
    let mut args = Array::new();
    args.push("__mcp-hook-stdio-v1");
    server.insert("args", value(args));
    let mut env_vars = Array::new();
    env_vars.push("WT_SESSION");
    server.insert("env_vars", value(env_vars));
    let mut omitted = Array::new();
    omitted.push("code_mode");
    omitted.push("deferred");
    omitted.push("direct");
    server.insert("omit_tools_from", value(omitted));
    config["mcp_servers"]
        .as_table_like_mut()
        .expect("MCP server table is writable")
        .insert("tabbeacon-hook", Item::Table(server));
    config.to_string()
}

fn exact_hybrid_declarations(session_end: Value) -> Vec<Value> {
    let mut declarations = [
        ("PreToolUse", json!({"cwd":"${cwd}","turn_id":"${turn_id}"})),
        ("PermissionRequest", json!({"turn_id":"${turn_id}"})),
        ("PostToolUse", json!({"cwd":"${cwd}","turn_id":"${turn_id}"})),
        ("PreCompact", json!({"cwd":"${cwd}","turn_id":"${turn_id}"})),
        ("PostCompact", json!({"cwd":"${cwd}","turn_id":"${turn_id}"})),
        ("SessionStart", json!({"cwd":"${cwd}","source":"${source}"})),
        ("UserPromptSubmit", json!({"cwd":"${cwd}","turn_id":"${turn_id}"})),
        (
            "SubagentStart",
            json!({"cwd":"${cwd}","turn_id":"${turn_id}","agent_id":"${agent_id}","agent_type":"${agent_type}"}),
        ),
        (
            "SubagentStop",
            json!({"cwd":"${cwd}","turn_id":"${turn_id}","agent_id":"${agent_id}","agent_type":"${agent_type}"}),
        ),
        ("Stop", json!({"cwd":"${cwd}","turn_id":"${turn_id}"})),
    ]
    .into_iter()
    .map(|(event, mut input)| {
        input["hook_event_name"] = Value::String(event.to_owned());
        input["session_id"] = Value::String("${session_id}".to_owned());
        json!({
            "event": event,
            "group": {
                "hooks": [{
                    "type": "mcp_tool",
                    "server": "tabbeacon-hook",
                    "tool": "tabbeacon_hook_event",
                    "input": input,
                    "timeout": 1
                }]
            }
        })
    })
    .collect::<Vec<_>>();
    declarations.push(session_end);
    declarations
}

fn spawn_mcp(installed: &Path, root: &TestRoot, codex_home: &Path) -> Child {
    Command::new(installed)
        .arg("__mcp-hook-stdio-v1")
        .env("CARGO_HOME", root.cargo_home())
        .env("CODEX_HOME", codex_home)
        .env("LOCALAPPDATA", root.local_appdata())
        .env("USERPROFILE", root.0.join("fixture-user"))
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("fixture MCP child starts")
}

fn preflight(installed: &Path, root: &TestRoot, drain: bool) -> Value {
    let mut command = Command::new(installed);
    command.args(["upgrade-preflight", "--json"]);
    if drain {
        command.arg("--drain");
    }
    let output = command
        .env("CARGO_HOME", root.cargo_home())
        .env("CODEX_HOME", root.codex_home())
        .env("LOCALAPPDATA", root.local_appdata())
        .env("USERPROFILE", root.0.join("fixture-user"))
        .output()
        .expect("fixture preflight starts");
    assert!(
        output.status.success(),
        "fixture preflight failed: status={:?}; stdout={}; stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("fixture preflight emits JSON")
}

fn wait_for_matching_processes(installed: &Path, root: &TestRoot, expected_count: usize) -> Value {
    let deadline = Instant::now() + Duration::from_secs(45);
    let last_observation = loop {
        let report = preflight(installed, root, false);
        let observed_count = report["workers"].as_array().map_or(0, Vec::len);
        if observed_count == expected_count {
            return report;
        }
        let observation = (
            observed_count,
            report["process_inspection"]
                .as_str()
                .unwrap_or("missing")
                .to_owned(),
            report["replaceability"]
                .as_str()
                .unwrap_or("missing")
                .to_owned(),
        );
        if Instant::now() >= deadline {
            break observation;
        }
        thread::sleep(Duration::from_millis(25));
    };
    panic!(
        "fixture MCP process observation did not settle: expected={expected_count}; last={last_observation:?}"
    );
}

fn wait_for_lease_count(root: &TestRoot, expected_count: usize) {
    let directory = root
        .local_appdata()
        .join("TabBeacon")
        .join("repository-identity")
        .join("mcp-runtime-v1");
    let deadline = Instant::now() + Duration::from_secs(45);
    loop {
        let count = fs::read_dir(&directory)
            .ok()
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|entry| entry.file_name().to_string_lossy().starts_with("lease-"))
                    .count()
            })
            .unwrap_or_default();
        if count == expected_count {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "fixture MCP lease registration did not settle"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

fn replacement(root: &TestRoot, cycle: usize) -> PathBuf {
    let path = root.0.join(format!("replacement-{cycle}.exe"));
    fs::copy(PathBuf::from(env!("CARGO_BIN_EXE_tabbeacon")), &path)
        .expect("next package candidate copies");
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("next package candidate opens")
        .write_all(b"tabbeacon-mcp-upgrade-fixture")
        .expect("next package candidate gets inert PE overlay");
    path
}

#[test]
fn package_mcp_cutover_drains_only_exact_children_across_1_4_8_sessions_and_two_cycles() {
    let root = TestRoot::new();
    let installed = install_exact_mcp_fixture(&root);

    // A server with no manifest authority has the same package image and MCP
    // entrypoint but must remain ambiguous and survive --drain.
    let mut ambiguous = spawn_mcp(&installed, &root, &root.0.join("unowned-codex-home"));
    let ambiguous_report = wait_for_matching_processes(&installed, &root, 1);
    assert_eq!(
        ambiguous_report["workers"][0]["ownership"],
        Value::String("unowned_or_ambiguous".to_owned())
    );
    let drained_ambiguous = preflight(&installed, &root, true);
    assert_eq!(
        drained_ambiguous["drained_owned_mcp_children"],
        Value::from(0)
    );
    assert!(
        ambiguous
            .try_wait()
            .expect("ambiguous child state reads")
            .is_none(),
        "an ambiguous package MCP process must not be terminated"
    );
    ambiguous
        .kill()
        .expect("fixture-owned ambiguous child stops");
    let _ = ambiguous.wait();
    // The first three rows cover the requested 1/4/8 concurrency states. The
    // fourth is a second consecutive one-session cutover after the package
    // path has already changed three times.
    for (cycle, session_count) in [(0_usize, 1_usize), (1, 4), (2, 8), (3, 1)] {
        let mut children = OwnedChildren(Vec::new());
        for _ in 0..session_count {
            children.push(spawn_mcp(&installed, &root, &root.codex_home()));
        }
        wait_for_lease_count(&root, session_count);
        let observed = wait_for_matching_processes(&installed, &root, session_count);
        assert_eq!(
            observed["replaceability"],
            Value::String("blocked_by_owned_tabbeacon_mcp".to_owned())
        );
        let process_classes = observed["workers"]
            .as_array()
            .expect("process diagnostics are an array")
            .iter()
            .map(|worker| {
                (
                    worker["kind"].as_str().unwrap_or("missing"),
                    worker["ownership"].as_str().unwrap_or("missing"),
                )
            })
            .collect::<Vec<_>>();
        assert!(
            process_classes.iter().all(|(kind, ownership)| {
                *kind == "mcp_stdio_server" && *ownership == "proved_tabbeacon_mcp"
            }),
            "unexpected content-minimal process classes: {process_classes:?}"
        );

        let next = replacement(&root, cycle * 10 + session_count);
        assert!(
            fs::rename(&next, &installed).is_err(),
            "Windows must keep a mapped package binary non-replaceable while exact MCP children live"
        );
        let drained = preflight(&installed, &root, true);
        assert_eq!(
            drained["drained_owned_mcp_children"],
            Value::from(session_count)
        );
        let post_drain_classes = drained["workers"]
            .as_array()
            .expect("post-drain diagnostics are an array")
            .iter()
            .map(|worker| {
                (
                    worker["kind"].as_str().unwrap_or("missing"),
                    worker["ownership"].as_str().unwrap_or("missing"),
                    worker["drain"].as_str().unwrap_or("missing"),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            drained["replaceability"],
            Value::String("ready".to_owned()),
            "post-drain classes: {post_drain_classes:?}"
        );
        children.wait_for_exit();
        fs::rename(&next, &installed)
            .expect("Windows replaces the package binary after exact MCP drain");
        wait_for_lease_count(&root, 0);
    }
}
