//! Ephemeral ownership proof for one package-installed Codex MCP runtime.
//!
//! The MCP server intentionally continues to execute the package-installed
//! `tabbeacon` binary.  Windows therefore keeps that binary mapped for the
//! server lifetime.  A lease lets upgrade preflight distinguish that one
//! internal runtime from a process which merely resembles it.  It never
//! retains a Codex session ID, Hook input, workspace data, or command line.

use std::{
    collections::BTreeSet,
    env,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{providers::codex::CodexIntegration, repo::StableAliasRegistry};

const LEASE_SCHEMA: &str = "tabbeacon-mcp-runtime-lease-v1";
const LEASE_DIRECTORY: &str = "mcp-runtime-v1";
const LEASE_TTL_TICKS: u64 = 12 * 60 * 60 * 10_000_000;
const MAX_LEASE_FILES: usize = 128;
const MAX_LEASE_BYTES: u64 = 16 * 1024;
const WINDOWS_TICKS_AT_UNIX_EPOCH: u64 = 621_355_968_000_000_000;
static LEASE_GENERATION_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Whether the bounded local MCP lease directory was safe to inspect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum McpLeaseHealth {
    /// Every observed lease was current and structurally valid.
    Healthy,
    /// At least one lease was stale, invalid, or could not be bound safely.
    Warning,
    /// The lease state root could not be inspected safely.
    Unavailable,
}

impl McpLeaseHealth {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Warning => "warning",
            Self::Unavailable => "unavailable",
        }
    }
}

/// The non-sensitive identity used to bind an MCP process to its lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct McpLeaseIdentity {
    pub(crate) process_id: u32,
    pub(crate) creation_ticks: u64,
    pub(crate) executable_path_sha256: String,
    pub(crate) executable_sha256: String,
    pub(crate) generation: String,
}

/// Bounded result of inspecting the local MCP ownership state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct McpLeaseInspection {
    pub(crate) health: McpLeaseHealth,
    pub(crate) identities: Vec<McpLeaseIdentity>,
    pub(crate) stale_or_invalid_process_ids: BTreeSet<u32>,
}

/// Content-minimal identity of executable bytes and its canonical path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct McpExecutableIdentity {
    pub(crate) path_sha256: String,
    pub(crate) content_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct McpLeaseRecord {
    schema: String,
    process_id: u32,
    creation_ticks: u64,
    registered_ticks: u64,
    expires_ticks: u64,
    executable_path_sha256: String,
    executable_sha256: String,
    generation: String,
}

/// Removes this exact lease when the actual MCP stdio runtime exits.
///
/// Forced process death intentionally leaves a bounded stale record.  That
/// record is never authority for a later process and naturally becomes a
/// warning rather than a drain target.
pub(crate) struct McpRuntimeLeaseGuard {
    path: PathBuf,
    generation: String,
}

impl Drop for McpRuntimeLeaseGuard {
    fn drop(&mut self) {
        if ensure_safe_path(&self.path).is_err() {
            return;
        }
        let Ok(bytes) = fs::read(&self.path) else {
            return;
        };
        let Ok(record) = serde_json::from_slice::<McpLeaseRecord>(&bytes) else {
            return;
        };
        if record.generation == self.generation && validate_record(&record).is_ok() {
            let _ = fs::remove_file(&self.path);
        }
    }
}

/// Registers a lease only after the internal MCP runtime has validated its
/// exact, manifest-owned server declaration.  Registration failure is
/// fail-open for Codex: the MCP server remains usable but cannot be drained
/// automatically during a later package upgrade.
#[must_use]
pub(crate) fn register_system_mcp_runtime_lease() -> Option<McpRuntimeLeaseGuard> {
    let executable = env::current_exe().ok()?;
    let integration = CodexIntegration::from_environment().ok()?;
    integration.mcp_runtime_lease_authority().ok()?;
    let state_root = StableAliasRegistry::default_state_root().ok()?;
    let process_id = std::process::id();
    let creation_ticks = system_process_creation_ticks(process_id)?;
    let executable_identity = executable_identity(&executable)?;
    McpLeaseStore::new(state_root)
        .register(
            process_id,
            creation_ticks,
            executable_identity,
            current_ticks(),
        )
        .ok()
}

/// Inspects the current user's MCP lease state without creating, changing, or
/// cleaning anything.  The caller must still bind a lease identity to a fresh
/// operating-system process observation before treating it as authority.
#[must_use]
pub(crate) fn inspect_system_mcp_runtime_leases() -> McpLeaseInspection {
    let Ok(state_root) = StableAliasRegistry::default_state_root() else {
        return McpLeaseInspection {
            health: McpLeaseHealth::Unavailable,
            identities: Vec::new(),
            stale_or_invalid_process_ids: BTreeSet::new(),
        };
    };
    McpLeaseStore::new(state_root).inspect_read_only(current_ticks())
}

/// Settles one lease after an ownership-scoped drain has stopped the exact
/// process bound to it.  This does not widen drain authority: callers already
/// need the independent process/lease/hash/creation-time proof before they
/// may request removal.  A missing or changed record is a safe no-op.
pub(crate) fn remove_system_mcp_runtime_lease(
    process_id: u32,
    creation_ticks: u64,
    executable: &McpExecutableIdentity,
) -> bool {
    let Ok(state_root) = StableAliasRegistry::default_state_root() else {
        return false;
    };
    McpLeaseStore::new(state_root).remove_matching(process_id, creation_ticks, executable)
}

/// Hashes the canonical executable path and its exact bytes.  Nothing is
/// emitted from this identity; it exists solely for the ownership comparison.
#[must_use]
pub(crate) fn executable_identity(path: &Path) -> Option<McpExecutableIdentity> {
    let canonical = fs::canonicalize(path).ok()?;
    let path = canonical
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("//?/")
        .to_lowercase();
    let bytes = fs::read(canonical).ok()?;
    Some(McpExecutableIdentity {
        path_sha256: sha256(path.as_bytes()),
        content_sha256: sha256(&bytes),
    })
}

struct McpLeaseStore {
    directory: PathBuf,
}

impl McpLeaseStore {
    fn new(state_root: impl Into<PathBuf>) -> Self {
        Self {
            directory: state_root.into().join(LEASE_DIRECTORY),
        }
    }

    fn register(
        &self,
        process_id: u32,
        creation_ticks: u64,
        executable: McpExecutableIdentity,
        now_ticks: u64,
    ) -> io::Result<McpRuntimeLeaseGuard> {
        ensure_safe_path(&self.directory)?;
        fs::create_dir_all(&self.directory)?;
        ensure_safe_path(&self.directory)?;
        let generation = lease_generation(process_id, creation_ticks, now_ticks);
        let record = McpLeaseRecord {
            schema: LEASE_SCHEMA.to_owned(),
            process_id,
            creation_ticks,
            registered_ticks: now_ticks,
            expires_ticks: now_ticks.saturating_add(LEASE_TTL_TICKS),
            executable_path_sha256: executable.path_sha256,
            executable_sha256: executable.content_sha256,
            generation: generation.clone(),
        };
        validate_record(&record)?;
        let path = self.lease_path(process_id, &generation)?;
        ensure_safe_path(&path)?;
        let bytes = serde_json::to_vec(&record)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(&bytes)?;
        file.flush()?;
        Ok(McpRuntimeLeaseGuard { path, generation })
    }

    fn inspect_read_only(&self, now_ticks: u64) -> McpLeaseInspection {
        if ensure_safe_path(&self.directory).is_err() {
            return McpLeaseInspection {
                health: McpLeaseHealth::Unavailable,
                identities: Vec::new(),
                stale_or_invalid_process_ids: BTreeSet::new(),
            };
        }

        if !self.directory.exists() {
            return McpLeaseInspection {
                health: McpLeaseHealth::Healthy,
                identities: Vec::new(),
                stale_or_invalid_process_ids: BTreeSet::new(),
            };
        }
        let Ok(entries) = safe_read_directory(&self.directory) else {
            return McpLeaseInspection {
                health: McpLeaseHealth::Unavailable,
                identities: Vec::new(),
                stale_or_invalid_process_ids: BTreeSet::new(),
            };
        };
        let mut health = McpLeaseHealth::Healthy;
        let mut identities = Vec::new();
        let mut stale_or_invalid_process_ids = BTreeSet::new();
        let mut lease_files = 0_usize;
        for entry in entries {
            let Ok(entry) = entry else {
                health = McpLeaseHealth::Warning;
                continue;
            };
            let path = entry.path();
            if !is_lease_name(entry.file_name().to_string_lossy().as_ref()) {
                continue;
            }
            lease_files = lease_files.saturating_add(1);
            if lease_files > MAX_LEASE_FILES {
                return McpLeaseInspection {
                    health: McpLeaseHealth::Warning,
                    identities: Vec::new(),
                    stale_or_invalid_process_ids,
                };
            }
            let record = read_record(&path);
            let Ok(record) = record else {
                health = McpLeaseHealth::Warning;
                continue;
            };
            let filename_matches = self
                .lease_path(record.process_id, &record.generation)
                .is_ok_and(|expected| expected == path);
            if !filename_matches {
                health = McpLeaseHealth::Warning;
                stale_or_invalid_process_ids.insert(record.process_id);
                continue;
            }
            let process_id = record.process_id;
            if validate_record(&record).is_err() || now_ticks > record.expires_ticks {
                health = McpLeaseHealth::Warning;
                stale_or_invalid_process_ids.insert(process_id);
                continue;
            }
            identities.push(McpLeaseIdentity {
                process_id,
                creation_ticks: record.creation_ticks,
                executable_path_sha256: record.executable_path_sha256,
                executable_sha256: record.executable_sha256,
                generation: record.generation,
            });
        }
        identities.sort_by(|left, right| {
            left.process_id
                .cmp(&right.process_id)
                .then_with(|| left.generation.cmp(&right.generation))
        });
        if identities
            .windows(2)
            .any(|pair| pair[0].process_id == pair[1].process_id)
        {
            // Multiple active leases for one PID cannot be safely associated
            // with a single process, even if their other fields appear valid.
            health = McpLeaseHealth::Warning;
            for identity in &identities {
                stale_or_invalid_process_ids.insert(identity.process_id);
            }
            identities.clear();
        }
        McpLeaseInspection {
            health,
            identities,
            stale_or_invalid_process_ids,
        }
    }

    fn lease_path(&self, process_id: u32, generation: &str) -> io::Result<PathBuf> {
        if process_id == 0 || !is_sha256(generation) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid MCP lease identity",
            ));
        }
        Ok(self
            .directory
            .join(format!("lease-{process_id}-{generation}.json")))
    }

    fn remove_matching(
        &self,
        process_id: u32,
        creation_ticks: u64,
        executable: &McpExecutableIdentity,
    ) -> bool {
        let Ok(entries) = safe_read_directory(&self.directory) else {
            return false;
        };
        let mut matches = Vec::new();
        for entry in entries.flatten().take(MAX_LEASE_FILES.saturating_add(1)) {
            let path = entry.path();
            if !is_lease_name(entry.file_name().to_string_lossy().as_ref()) {
                continue;
            }
            let Ok(record) = read_record(&path) else {
                continue;
            };
            if validate_record(&record).is_ok()
                && record.process_id == process_id
                && record.creation_ticks == creation_ticks
                && record.executable_path_sha256 == executable.path_sha256
                && record.executable_sha256 == executable.content_sha256
                && self
                    .lease_path(record.process_id, &record.generation)
                    .is_ok_and(|expected| expected == path)
            {
                matches.push((path, record.generation));
            }
        }
        if matches.len() != 1 {
            return false;
        }
        let (path, generation) = matches.pop().expect("one match is present");
        if ensure_safe_path(&path).is_err() {
            return false;
        }
        let Ok(record) = read_record(&path) else {
            return false;
        };
        if record.generation != generation
            || record.process_id != process_id
            || record.creation_ticks != creation_ticks
            || record.executable_path_sha256 != executable.path_sha256
            || record.executable_sha256 != executable.content_sha256
        {
            return false;
        }
        fs::remove_file(path).is_ok()
    }
}

fn read_record(path: &Path) -> io::Result<McpLeaseRecord> {
    ensure_safe_path(path)?;
    let metadata = fs::metadata(path)?;
    if metadata.len() > MAX_LEASE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "MCP lease exceeds bounded size",
        ));
    }
    let bytes = fs::read(path)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn safe_read_directory(path: &Path) -> io::Result<fs::ReadDir> {
    ensure_safe_path(path)?;
    fs::read_dir(path)
}

fn validate_record(record: &McpLeaseRecord) -> io::Result<()> {
    let duration = record.expires_ticks.saturating_sub(record.registered_ticks);
    if record.schema != LEASE_SCHEMA
        || record.process_id == 0
        || record.creation_ticks < WINDOWS_TICKS_AT_UNIX_EPOCH
        || record.registered_ticks < WINDOWS_TICKS_AT_UNIX_EPOCH
        || duration == 0
        || duration > LEASE_TTL_TICKS
        || !is_sha256(&record.executable_path_sha256)
        || !is_sha256(&record.executable_sha256)
        || !is_sha256(&record.generation)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid MCP runtime lease",
        ));
    }
    Ok(())
}

fn lease_generation(process_id: u32, creation_ticks: u64, now_ticks: u64) -> String {
    let counter = LEASE_GENERATION_COUNTER.fetch_add(1, Ordering::Relaxed);
    sha256(format!("{process_id}:{creation_ticks}:{now_ticks}:{counter}").as_bytes())
}

fn is_lease_name(value: &str) -> bool {
    let Some(value) = value
        .strip_prefix("lease-")
        .and_then(|value| value.strip_suffix(".json"))
    else {
        return false;
    };
    let Some((process_id, generation)) = value.rsplit_once('-') else {
        return false;
    };
    process_id
        .parse::<u32>()
        .is_ok_and(|process_id| process_id != 0)
        && is_sha256(generation)
}

fn current_ticks() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(WINDOWS_TICKS_AT_UNIX_EPOCH, |duration| {
            WINDOWS_TICKS_AT_UNIX_EPOCH
                .saturating_add(u64::try_from(duration.as_nanos() / 100).unwrap_or(u64::MAX))
        })
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ensure_safe_path(path: &Path) -> io::Result<()> {
    let mut cursor = Some(path);
    while let Some(candidate) = cursor {
        match fs::symlink_metadata(candidate) {
            Ok(metadata)
                if metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata) =>
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "MCP lease state cannot use a symbolic link or reparse point",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        cursor = candidate.parent();
    }
    Ok(())
}

#[cfg(windows)]
fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_attributes() & 0x0400 != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse_point(_metadata: &fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
fn system_process_creation_ticks(process_id: u32) -> Option<u64> {
    let powershell = system_powershell_path()?;
    let output = Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$p = Get-Process -Id ([int]$env:TABBEACON_MCP_LEASE_PID) -ErrorAction Stop; [Console]::Out.Write($p.StartTime.ToUniversalTime().Ticks)",
        ])
        .env("TABBEACON_MCP_LEASE_PID", process_id.to_string())
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;
    output.status.success().then(|| {
        String::from_utf8(output.stdout)
            .ok()?
            .trim()
            .parse::<u64>()
            .ok()
    })?
}

#[cfg(not(windows))]
fn system_process_creation_ticks(_process_id: u32) -> Option<u64> {
    None
}

#[cfg(windows)]
fn system_powershell_path() -> Option<PathBuf> {
    let path = PathBuf::from(env::var_os("SystemRoot")?)
        .join("System32")
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    path.is_file().then_some(path)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::{
        LEASE_SCHEMA, McpExecutableIdentity, McpLeaseHealth, McpLeaseRecord, McpLeaseStore,
        WINDOWS_TICKS_AT_UNIX_EPOCH, sha256,
    };

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "tabbeacon-{name}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("test clock is after epoch")
                    .as_nanos()
            ));
            fs::create_dir_all(&path).expect("isolated test root creates");
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn executable() -> McpExecutableIdentity {
        McpExecutableIdentity {
            path_sha256: sha256(b"canonical-path"),
            content_sha256: sha256(b"executable-bytes"),
        }
    }

    #[test]
    fn live_lease_is_content_minimal_and_drop_removes_only_its_generation() {
        let root = TestRoot::new("mcp-lease-drop");
        let store = McpLeaseStore::new(&root.0);
        let now = WINDOWS_TICKS_AT_UNIX_EPOCH + 10_000;
        let guard = store
            .register(77, now - 100, executable(), now)
            .expect("lease registers");
        let inspection = store.inspect_read_only(now + 1);
        assert_eq!(inspection.health, McpLeaseHealth::Healthy);
        assert_eq!(inspection.identities.len(), 1);
        assert_eq!(inspection.identities[0].process_id, 77);
        assert!(inspection.stale_or_invalid_process_ids.is_empty());
        drop(guard);
        assert!(store.inspect_read_only(now + 1).identities.is_empty());
    }

    #[test]
    fn stale_or_invalid_leases_are_never_authority_for_a_later_process() {
        let root = TestRoot::new("mcp-lease-stale");
        let store = McpLeaseStore::new(&root.0);
        let now = WINDOWS_TICKS_AT_UNIX_EPOCH + 10_000;
        fs::create_dir_all(&store.directory).expect("lease directory creates");
        let record = McpLeaseRecord {
            schema: LEASE_SCHEMA.to_owned(),
            process_id: 88,
            creation_ticks: now - 100,
            registered_ticks: now - 50,
            expires_ticks: now - 1,
            executable_path_sha256: executable().path_sha256,
            executable_sha256: executable().content_sha256,
            generation: sha256(b"stale-generation"),
        };
        let bytes = serde_json::to_vec(&record).expect("lease serializes");
        fs::write(
            store
                .lease_path(record.process_id, &record.generation)
                .expect("lease path"),
            bytes,
        )
        .expect("stale lease writes");
        let inspection = store.inspect_read_only(now);
        assert_eq!(inspection.health, McpLeaseHealth::Warning);
        assert!(inspection.identities.is_empty());
        assert!(inspection.stale_or_invalid_process_ids.contains(&88));
    }

    #[test]
    fn duplicate_active_pid_leases_fail_closed() {
        let root = TestRoot::new("mcp-lease-duplicate");
        let store = McpLeaseStore::new(&root.0);
        let now = WINDOWS_TICKS_AT_UNIX_EPOCH + 10_000;
        let _first = store
            .register(99, now - 100, executable(), now)
            .expect("first lease registers");
        let _second = store
            .register(99, now - 100, executable(), now + 1)
            .expect("second lease registers");
        let inspection = store.inspect_read_only(now + 2);
        assert_eq!(inspection.health, McpLeaseHealth::Warning);
        assert!(inspection.identities.is_empty());
        assert!(inspection.stale_or_invalid_process_ids.contains(&99));
    }

    #[test]
    fn explicit_drain_settlement_removes_only_the_exact_proven_lease() {
        let root = TestRoot::new("mcp-lease-drain-settle");
        let store = McpLeaseStore::new(&root.0);
        let now = WINDOWS_TICKS_AT_UNIX_EPOCH + 10_000;
        let identity = executable();
        let guard = store
            .register(111, now - 100, identity.clone(), now)
            .expect("lease registers");
        std::mem::forget(guard);
        assert!(store.remove_matching(111, now - 100, &identity));
        assert!(store.inspect_read_only(now + 1).identities.is_empty());
        assert!(
            !store.remove_matching(111, now - 99, &identity),
            "a creation-time mismatch must not settle the lease"
        );
    }
}
