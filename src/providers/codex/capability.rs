//! Local, bounded Codex capability discovery.
//!
//! Codex release numbers are useful diagnostics, but they are deliberately not
//! an input to compatibility or mutation authority. The probe uses only local
//! noninteractive commands and stores no configuration, Hook payload, prompt,
//! or credential data.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{CodexCompatibilityState, CodexHookProfile};

const CACHE_SCHEMA: &str = "tabbeacon-codex-capability-v1";
const CACHE_FILE: &str = "capability-v1.json";

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
    profile: Option<CachedProfile>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CachedCapabilityState {
    Full,
    Degraded,
    Incompatible,
    Unproven,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CachedProfile {
    CommandV1,
    McpHybridV1,
}

impl CachedProfile {
    const fn from_profile(profile: CodexHookProfile) -> Self {
        if profile.uses_mcp_hook_transport() {
            Self::McpHybridV1
        } else {
            Self::CommandV1
        }
    }

    const fn into_profile(self) -> CodexHookProfile {
        match self {
            Self::CommandV1 => CodexHookProfile::command_v1(),
            Self::McpHybridV1 => CodexHookProfile::mcp_hybrid_v1(),
        }
    }
}

impl CachedCapabilityState {
    const fn from_state(state: CodexCompatibilityState) -> Self {
        match state {
            CodexCompatibilityState::Full(_) => Self::Full,
            CodexCompatibilityState::Degraded(_) => Self::Degraded,
            CodexCompatibilityState::Incompatible => Self::Incompatible,
            CodexCompatibilityState::Unproven => Self::Unproven,
        }
    }

    fn into_state(self, profile: Option<CachedProfile>) -> CodexCompatibilityState {
        let profile = profile.unwrap_or(CachedProfile::CommandV1).into_profile();
        match self {
            Self::Full => CodexCompatibilityState::Full(profile),
            Self::Degraded => CodexCompatibilityState::Degraded(profile),
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
    let executable_identity = executable_identity(codex_program);
    let cache_path = state_root.join(CACHE_FILE);
    if let Some(identity) = executable_identity.as_deref()
        && let Some(record) = read_cache(&cache_path)
        && record.schema == CACHE_SCHEMA
        && record.executable_identity == identity
    {
        return CodexCapabilityProbe {
            version,
            state: record.state.into_state(record.profile),
            cache_hit: true,
            schema_fingerprint: record
                .capability_fingerprint
                .strip_prefix("schema:")
                .map(str::to_owned),
        };
    }

    let hook_feature = probe_hook_feature(codex_program);
    let (state, schema_fingerprint) = match hook_feature {
        HookFeature::Enabled => match probe_schema(codex_program) {
            Some(schema) => (
                CodexCompatibilityState::Full(if schema.mcp_hook_transport {
                    CodexHookProfile::mcp_hybrid_v1()
                } else {
                    CodexHookProfile::command_v1()
                }),
                Some(schema.fingerprint),
            ),
            None => (
                CodexCompatibilityState::Degraded(CodexHookProfile::command_v1()),
                None,
            ),
        },
        HookFeature::Disabled => (CodexCompatibilityState::Incompatible, None),
        HookFeature::Unproven => (CodexCompatibilityState::Unproven, None),
    };

    if persist_cache && let Some(identity) = executable_identity {
        if state_root.is_dir() || fs::create_dir_all(state_root).is_ok() {
            let record = CapabilityCacheRecord {
                schema: CACHE_SCHEMA.to_owned(),
                executable_identity: identity,
                capability_fingerprint: schema_fingerprint.as_deref().map_or_else(
                    || "schema:unavailable".to_owned(),
                    |value| format!("schema:{value}"),
                ),
                state: CachedCapabilityState::from_state(state),
                profile: state.supported_profile().map(CachedProfile::from_profile),
            };
            let _ = write_cache(&cache_path, &record);
        }
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
    let Ok(stdout) = String::from_utf8(output.stdout) else {
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
    HookFeature::Disabled
}

struct SchemaEvidence {
    fingerprint: String,
    mcp_hook_transport: bool,
}

fn probe_schema(codex_program: Option<&Path>) -> Option<SchemaEvidence> {
    let root = temporary_schema_root()?;
    let status = command(codex_program)
        .args(["app-server", "generate-json-schema", "--out"])
        .arg(&root)
        .status()
        .ok();
    let fingerprint = status
        .filter(|status| status.success())
        .and_then(|_| directory_fingerprint(&root));
    let mcp_hook_transport = fingerprint
        .as_ref()
        .is_some_and(|_| directory_contains(&root, b"mcp_tool"));
    let _ = fs::remove_dir_all(&root);
    fingerprint.map(|fingerprint| SchemaEvidence {
        fingerprint,
        mcp_hook_transport,
    })
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

fn directory_contains(root: &Path, needle: &[u8]) -> bool {
    let mut files = Vec::new();
    collect_files(root, root, &mut files).is_some_and(|()| {
        files.into_iter().any(|file| {
            fs::read(root.join(file))
                .ok()
                .is_some_and(|bytes| bytes.windows(needle.len()).any(|window| window == needle))
        })
    })
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
