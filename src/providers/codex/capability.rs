//! Local, bounded Codex capability discovery.
//!
//! Codex release numbers are useful diagnostics, but they are deliberately not
//! an input to compatibility or mutation authority. The probe uses only local
//! noninteractive commands and stores no configuration, Hook payload, prompt,
//! or credential data.

use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{CodexCompatibilityState, CodexHookProfile};

const CACHE_SCHEMA: &str = "tabbeacon-codex-capability-v3";
const CACHE_FILE: &str = "capability-v1.json";
const RUNTIME_FEATURE_DEADLINE: Duration = Duration::from_millis(400);
const RUNTIME_IDENTITY_DEADLINE: Duration = Duration::from_millis(250);

/// Runtime Hook admission uses the capability proven during owned setup,
/// bound to the current executable bytes. Only the mutable feature flag is
/// rechecked, with a short deadline; schema generation never runs in a Hook.
pub(crate) fn interrupt_runtime_capable(codex_program: Option<&Path>, state_root: &Path) -> bool {
    let program = codex_program.map(Path::to_path_buf);
    let cache_path = state_root.join(CACHE_FILE);
    let (sender, receiver) = mpsc::sync_channel(1);
    // The identity check may encounter a slow filesystem. It runs in a
    // bounded-lived Hook process, and the main Hook never waits beyond this
    // budget before conservatively declining Interrupt authority.
    if thread::Builder::new()
        .name("tabbeacon-codex-identity".to_owned())
        .spawn(move || {
            let admitted = read_cache(&cache_path).is_some_and(|record| {
                record.schema == CACHE_SCHEMA
                    && matches!(
                        record.state,
                        CachedCapabilityState::FullInterrupt
                            | CachedCapabilityState::DegradedInterrupt
                    )
                    && executable_identity(program.as_deref()).as_deref()
                        == Some(&record.executable_identity)
            });
            let _ = sender.send(admitted);
        })
        .is_err()
    {
        return false;
    }
    if !matches!(receiver.recv_timeout(RUNTIME_IDENTITY_DEADLINE), Ok(true)) {
        return false;
    }
    probe_hook_feature_bounded(codex_program) == HookFeature::Enabled
}

/// Content-minimal result of a local Codex capability probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodexCapabilityProbe {
    version: Option<String>,
    state: CodexCompatibilityState,
    cache_hit: bool,
    schema_fingerprint: Option<String>,
}

impl CodexCapabilityProbe {
    /// Version text is diagnostic-only and never grants compatibility.
    #[must_use]
    pub(crate) fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Capability-derived compatibility result.
    #[must_use]
    pub(crate) const fn state(&self) -> CodexCompatibilityState {
        self.state
    }

    /// Whether a cache record bound to the same executable identity was reused.
    #[must_use]
    pub(crate) const fn cache_hit(&self) -> bool {
        self.cache_hit
    }

    /// Hash of generated local schema metadata, when that optional surface was
    /// available. It contains no schema body or provider content.
    #[must_use]
    pub(crate) fn schema_fingerprint(&self) -> Option<&str> {
        self.schema_fingerprint.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CapabilityCacheRecord {
    schema: String,
    executable_identity: String,
    capability_fingerprint: String,
    state: CachedCapabilityState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CachedCapabilityState {
    Full,
    FullInterrupt,
    Degraded,
    DegradedInterrupt,
    Incompatible,
    Unproven,
}

impl CachedCapabilityState {
    fn from_state(state: CodexCompatibilityState) -> Self {
        match state {
            CodexCompatibilityState::Full(profile) => {
                if profile
                    .lifecycle_events()
                    .contains(&super::CodexHookEvent::Interrupt)
                {
                    Self::FullInterrupt
                } else {
                    Self::Full
                }
            }
            CodexCompatibilityState::Degraded(profile) => {
                if profile
                    .lifecycle_events()
                    .contains(&super::CodexHookEvent::Interrupt)
                {
                    Self::DegradedInterrupt
                } else {
                    Self::Degraded
                }
            }
            CodexCompatibilityState::Incompatible => Self::Incompatible,
            CodexCompatibilityState::Unproven => Self::Unproven,
        }
    }

    fn into_state(self) -> CodexCompatibilityState {
        match self {
            // Capability discovery authorizes only the conservative command
            // transport. An existing exact hybrid transport is selected from
            // the separately validated ownership manifest, never a cache.
            Self::Full => CodexCompatibilityState::Full(CodexHookProfile::command_v1()),
            Self::FullInterrupt => {
                CodexCompatibilityState::Full(CodexHookProfile::command_interrupt_v1())
            }
            Self::Degraded => CodexCompatibilityState::Degraded(CodexHookProfile::command_v1()),
            Self::DegradedInterrupt => {
                CodexCompatibilityState::Degraded(CodexHookProfile::command_interrupt_v1())
            }
            Self::Incompatible => CodexCompatibilityState::Incompatible,
            Self::Unproven => CodexCompatibilityState::Unproven,
        }
    }
}

/// Runs local capability discovery. `persist_cache` is true only from an
/// ownership-authorized mutation path; read-only doctor calls may reuse but
/// never create cache state.
pub(crate) fn probe(
    codex_program: Option<&Path>,
    state_root: &Path,
    persist_cache: bool,
) -> CodexCapabilityProbe {
    let version = probe_version(codex_program);
    // The feature flag can change in user configuration while the executable
    // remains byte-identical. A cache entry never overrides current evidence
    // that Hooks are disabled or unavailable.
    let hook_feature = probe_hook_feature(codex_program);
    let executable_identity = executable_identity(codex_program);
    let cache_path = state_root.join(CACHE_FILE);
    if let Some(identity) = executable_identity.as_deref()
        && let Some(record) = read_cache(&cache_path)
        && record.schema == CACHE_SCHEMA
        && record.executable_identity == identity
        && hook_feature == HookFeature::Enabled
        && (version.as_deref() == Some("0.156.1"))
            == matches!(
                record.state,
                CachedCapabilityState::FullInterrupt | CachedCapabilityState::DegradedInterrupt
            )
    {
        return CodexCapabilityProbe {
            version,
            state: record.state.into_state(),
            cache_hit: true,
            schema_fingerprint: record
                .capability_fingerprint
                .strip_prefix("schema:")
                .map(str::to_owned),
        };
    }

    // This exact installed release was audited against its matching upstream
    // source tag. Other release numbers retain command-v1; ordering grants no
    // event authority.
    let command_profile = if version.as_deref() == Some("0.156.1") {
        CodexHookProfile::command_interrupt_v1()
    } else {
        CodexHookProfile::command_v1()
    };
    let (state, schema_fingerprint) = match hook_feature {
        HookFeature::Enabled => match probe_schema(codex_program) {
            Some(schema) => (
                // A generated schema is diagnostic-only. In particular, an
                // unrelated `mcp_tool` string does not positively establish
                // the actual Hook MCP declaration contract.
                CodexCompatibilityState::Full(command_profile),
                Some(schema.fingerprint),
            ),
            None => (CodexCompatibilityState::Degraded(command_profile), None),
        },
        HookFeature::Disabled => (CodexCompatibilityState::Incompatible, None),
        HookFeature::Unproven => (CodexCompatibilityState::Unproven, None),
    };

    if persist_cache
        && let Some(identity) = executable_identity
        && (state_root.is_dir() || fs::create_dir_all(state_root).is_ok())
    {
        let record = CapabilityCacheRecord {
            schema: CACHE_SCHEMA.to_owned(),
            executable_identity: identity,
            capability_fingerprint: schema_fingerprint.as_deref().map_or_else(
                || "schema:unavailable".to_owned(),
                |value| format!("schema:{value}"),
            ),
            state: CachedCapabilityState::from_state(state),
        };
        let _ = write_cache(&cache_path, &record);
    }

    CodexCapabilityProbe {
        version,
        state,
        cache_hit: false,
        schema_fingerprint,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookFeature {
    Enabled,
    Disabled,
    Unproven,
}

fn probe_hook_feature(codex_program: Option<&Path>) -> HookFeature {
    let Ok(output) = command(codex_program).args(["features", "list"]).output() else {
        return HookFeature::Unproven;
    };
    if !output.status.success() {
        return HookFeature::Unproven;
    }
    parse_hook_feature(&output.stdout)
}

fn probe_hook_feature_bounded(codex_program: Option<&Path>) -> HookFeature {
    let deadline = Instant::now() + RUNTIME_FEATURE_DEADLINE;
    // A file avoids a pipe held open by a descendant after the owned child
    // exits. The response is capped and discarded on every path.
    let Ok(mut output_file) = tempfile::tempfile() else {
        return HookFeature::Unproven;
    };
    let Ok(child_stdout) = output_file.try_clone() else {
        return HookFeature::Unproven;
    };
    let Ok(mut child) = command(codex_program)
        .args(["features", "list"])
        .stdin(Stdio::null())
        .stdout(Stdio::from(child_stdout))
        .stderr(Stdio::null())
        .spawn()
    else {
        return HookFeature::Unproven;
    };
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return HookFeature::Unproven;
                }
                if Instant::now() >= deadline || output_file.seek(SeekFrom::Start(0)).is_err() {
                    return HookFeature::Unproven;
                }
                let mut bytes = Vec::new();
                return if output_file.take(4097).read_to_end(&mut bytes).is_ok()
                    && bytes.len() <= 4096
                    && Instant::now() < deadline
                {
                    parse_hook_feature(&bytes)
                } else {
                    HookFeature::Unproven
                };
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            _ => {
                // This is the child that this Hook started, not an ambient
                // provider or another user's process.
                if child.kill().is_ok() {
                    let _ = child.try_wait();
                }
                return HookFeature::Unproven;
            }
        }
    }
}

fn parse_hook_feature(bytes: &[u8]) -> HookFeature {
    let Ok(stdout) = std::str::from_utf8(bytes) else {
        return HookFeature::Unproven;
    };
    for line in stdout.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.first() != Some(&"hooks") {
            continue;
        }
        return match fields.last().copied() {
            Some("true") => HookFeature::Enabled,
            Some("false") => HookFeature::Disabled,
            _ => HookFeature::Unproven,
        };
    }
    // A missing row says nothing about whether Hooks have graduated from a
    // feature flag. Only an explicit `hooks … false` is negative evidence.
    HookFeature::Unproven
}

struct SchemaEvidence {
    fingerprint: String,
}

fn probe_schema(codex_program: Option<&Path>) -> Option<SchemaEvidence> {
    let root = temporary_schema_root()?;
    let status = command(codex_program)
        .args(["app-server", "generate-json-schema", "--out"])
        .arg(&root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok();
    let fingerprint = status
        .filter(std::process::ExitStatus::success)
        .and_then(|_| directory_fingerprint(&root));
    let _ = fs::remove_dir_all(&root);
    fingerprint.map(|fingerprint| SchemaEvidence { fingerprint })
}

fn temporary_schema_root() -> Option<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "tabbeacon-codex-capability-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&root).ok()?;
    Some(root)
}

fn directory_fingerprint(root: &Path) -> Option<String> {
    let mut files = Vec::new();
    collect_files(root, root, &mut files)?;
    files.sort();
    if files.is_empty() {
        return None;
    }
    let mut digest = Sha256::new();
    for file in files {
        digest.update(file.to_string_lossy().as_bytes());
        digest.update(fs::read(root.join(file)).ok()?);
    }
    Some(format!("sha256:{:x}", digest.finalize()))
}

fn collect_files(root: &Path, current: &Path, files: &mut Vec<PathBuf>) -> Option<()> {
    for entry in fs::read_dir(current).ok()?.filter_map(Result::ok) {
        let kind = entry.file_type().ok()?;
        if kind.is_file() {
            files.push(entry.path().strip_prefix(root).ok()?.to_path_buf());
        } else if kind.is_dir() {
            collect_files(root, &entry.path(), files)?;
        }
    }
    Some(())
}

fn probe_version(codex_program: Option<&Path>) -> Option<String> {
    let output = command(codex_program).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout
        .split_whitespace()
        .find(|value| {
            value
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
        })
        .map(str::to_owned)
}

fn command(codex_program: Option<&Path>) -> Command {
    Command::new(
        codex_program
            .map(Path::to_path_buf)
            .or_else(resolve_default_program)
            .unwrap_or_else(|| PathBuf::from("codex")),
    )
}

fn executable_identity(codex_program: Option<&Path>) -> Option<String> {
    let path = codex_program
        .map(Path::to_path_buf)
        .or_else(resolve_default_program)?
        .canonicalize()
        .ok()?;
    let bytes = fs::read(path).ok()?;
    Some(format!("sha256:{:x}", Sha256::digest(bytes)))
}

/// Resolves only the executable that normal `Command::new("codex")` would use
/// from the current process PATH. It reads no profile or credential state.
fn resolve_default_program() -> Option<PathBuf> {
    let extensions = if cfg!(windows) {
        ["codex.exe", "codex.cmd", "codex.bat", "codex"]
    } else {
        ["codex", "codex", "codex", "codex"]
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .flat_map(|directory| extensions.iter().map(move |name| directory.join(name)))
        .find(|candidate| candidate.is_file())
}

fn read_cache(path: &Path) -> Option<CapabilityCacheRecord> {
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn write_cache(path: &Path, record: &CapabilityCacheRecord) -> Result<(), ()> {
    let bytes = serde_json::to_vec(record).map_err(|_| ())?;
    fs::write(path, bytes).map_err(|_| ())
}
