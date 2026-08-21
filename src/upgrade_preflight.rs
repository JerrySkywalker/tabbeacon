//! Read-only Windows package-upgrade diagnosis and an ownership-scoped drain.
//!
//! The package-installed executable remains the public one-shot entrypoint.
//! This module only explains whether a live process can prevent a replacement;
//! it never performs a drain unless the caller supplies the explicit command
//! flag, and it never receives or emits raw command lines.

use std::{
    env,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use serde::Serialize;
use serde_json::Value;

use crate::activity::{
    ActiveWorkerLeaseIdentity, ActiveWorkerLeaseInspection, ActivityLeaseHealth,
    inspect_system_active_worker_identities,
};
use crate::worker_runtime::WorkerRuntimeStore;

/// Stable JSON schema version for upgrade preflight output.
pub const UPGRADE_PREFLIGHT_SCHEMA_VERSION: u32 = 2;

/// Which local executable the preflight inspected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeTargetSource {
    /// The normal per-user Cargo installation was present.
    InstalledCargoBinary,
    /// No installed Cargo binary was present, so the current executable was inspected.
    CurrentExecutable,
    /// The operating system did not disclose a usable executable path.
    Unavailable,
}

impl UpgradeTargetSource {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InstalledCargoBinary => "installed_cargo_binary",
            Self::CurrentExecutable => "current_executable",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Whether a `TabBeacon` process is known to block the target executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeReplaceability {
    /// An explicit drain probe opened the target for replacement without changing it.
    Ready,
    /// No `TabBeacon` process using the target executable remains after observation.
    NoKnownTabBeaconLock,
    /// A matching process or operating-system sharing rule currently blocks replacement.
    Blocked,
    /// Process or filesystem inspection could not establish a safe answer.
    Unavailable,
}

impl UpgradeReplaceability {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::NoKnownTabBeaconLock => "no_known_tabbeacon_lock",
            Self::Blocked => "blocked",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Availability of the bounded operating-system process inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeProcessInspection {
    /// The process list was read through the bounded Windows query.
    Available,
    /// The platform could not safely establish the matching process list.
    Unavailable,
}

impl UpgradeProcessInspection {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Ownership result for one process using the inspected executable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeWorkerOwnership {
    /// The exact internal worker arguments matched one valid active local lease.
    ProvedTabBeaconWorker,
    /// The process must be preserved because ownership was not proven.
    UnownedOrAmbiguous,
}

impl UpgradeWorkerOwnership {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProvedTabBeaconWorker => "proved_tabbeacon_worker",
            Self::UnownedOrAmbiguous => "unowned_or_ambiguous",
        }
    }
}

/// Result for a requested drain, without exposing the process command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeDrainDisposition {
    /// The default read-only preflight did not request a drain.
    NotRequested,
    /// An explicit drain may target this currently proven worker.
    Eligible,
    /// The process was stopped only after a fresh ownership recheck.
    Drained,
    /// A fresh recheck no longer proved ownership, so no signal was sent.
    RefusedAtRecheck,
    /// The operating system declined the requested ownership-scoped stop.
    Failed,
    /// This process was never a permitted drain target.
    PreservedAmbiguous,
}

impl UpgradeDrainDisposition {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRequested => "not_requested",
            Self::Eligible => "eligible",
            Self::Drained => "drained",
            Self::RefusedAtRecheck => "refused_at_recheck",
            Self::Failed => "failed",
            Self::PreservedAmbiguous => "preserved_ambiguous",
        }
    }
}

/// Content-minimal result for one live process using the target executable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradeWorkerDiagnostic {
    /// Windows process identifier. It is display-only and is never persisted.
    pub process_id: u32,
    /// Whether the exact worker identity was proven from a current local lease.
    pub ownership: UpgradeWorkerOwnership,
    /// Explicit-drain decision for this process.
    pub drain: UpgradeDrainDisposition,
}

/// Privacy and mutation boundaries guaranteed by the preflight command.
#[allow(clippy::struct_excessive_bools)] // Each boundary is independently asserted in the stable output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct UpgradePreflightBoundaries {
    /// The normal command path performs no process or configuration mutation.
    pub default_read_only: bool,
    /// Process termination is available only behind the explicit `--drain` flag.
    pub explicit_drain_only: bool,
    /// Arbitrary process command lines are not emitted.
    pub raw_command_lines: bool,
    /// Native provider session identifiers are not emitted.
    pub raw_native_session_ids: bool,
}

/// Read-only runtime-image status used to explain post-G63 package upgrades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpgradeRuntimeImageInspection {
    /// Image directories and all bounded hash-addressed images were verified.
    Available,
    /// Runtime image state could not be safely inspected.
    Unavailable,
}

impl UpgradeRuntimeImageInspection {
    /// Stable machine-oriented spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
        }
    }
}

/// Runtime images related to a package-replacement decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradeRuntimeImageState {
    /// Whether the bounded image inventory was safely inspected.
    pub inspection: UpgradeRuntimeImageInspection,
    /// Content hashes named by active worker leases. These identify only local
    /// executable bytes, not native sessions or Hook content.
    pub active_image_hashes: Vec<String>,
    /// Verified images not named by any active lease, when the inventory is
    /// available. `None` means a safe stale count could not be established.
    pub stale_image_count: Option<usize>,
    /// True only when lease and image inspection proves that opportunistic GC
    /// may delete a hash-verified image not named above.
    pub cleanup_safe: bool,
}

/// Stable read-only report for a local package upgrade decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpgradePreflight {
    /// Stable output schema version.
    pub schema_version: u32,
    /// Compiled product version.
    pub tabbeacon_version: String,
    /// Current executable when the operating system provides it.
    pub current_executable: Option<String>,
    /// The executable against which worker ownership and replaceability were checked.
    pub target_executable: Option<String>,
    /// Why that target was selected.
    pub target_source: UpgradeTargetSource,
    /// Whether the platform process query was available.
    pub process_inspection: UpgradeProcessInspection,
    /// Whether lease state was sufficiently healthy to prove a worker target.
    pub worker_lease_health: String,
    /// Content-minimal runtime-image ownership state.
    pub runtime_images: UpgradeRuntimeImageState,
    /// Current replacement disposition after an optional explicit drain.
    pub replaceability: UpgradeReplaceability,
    /// One non-self matching process per observation, without command content.
    pub workers: Vec<UpgradeWorkerDiagnostic>,
    /// Whether this invocation requested the ownership-scoped drain operation.
    pub drain_requested: bool,
    /// Count of processes stopped after a fresh ownership recheck.
    pub drained_owned_workers: usize,
    /// Command safety and privacy invariants.
    pub boundaries: UpgradePreflightBoundaries,
}

impl UpgradePreflight {
    /// Number of currently listed processes whose ownership was proven.
    #[must_use]
    pub fn proved_owned_worker_count(&self) -> usize {
        self.workers
            .iter()
            .filter(|worker| worker.ownership == UpgradeWorkerOwnership::ProvedTabBeaconWorker)
            .count()
    }

    /// Number of listed processes that are intentionally preserved.
    #[must_use]
    pub fn ambiguous_process_count(&self) -> usize {
        self.workers
            .iter()
            .filter(|worker| worker.ownership == UpgradeWorkerOwnership::UnownedOrAmbiguous)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedProcess {
    process_id: u32,
    command_line: String,
}

trait UpgradeProcessInspector {
    fn matching_processes(&mut self, target: &Path) -> Result<Vec<ObservedProcess>, ()>;
    fn probe_replaceability(&mut self, target: &Path) -> UpgradeReplaceability;
    fn terminate_proved_worker(&mut self, process_id: u32) -> Result<(), ()>;
}

/// Inspects the installed Cargo executable when available, otherwise the current
/// executable. Without `--drain`, this function performs no process mutation.
#[must_use]
pub fn inspect_system_upgrade_preflight(drain_requested: bool) -> UpgradePreflight {
    let current_executable = env::current_exe().ok();
    let (target, target_source) = select_upgrade_target(current_executable.as_deref());
    let leases = inspect_system_active_worker_identities();
    let state_root = crate::repo::StableAliasRegistry::default_state_root().ok();
    let runtime_images = runtime_image_state(state_root.as_deref(), &leases);
    let mut inspector = SystemUpgradeProcessInspector;
    inspect_with_inspector(
        &mut inspector,
        current_executable.as_deref(),
        target.as_deref(),
        target_source,
        &leases,
        runtime_images,
        drain_requested,
    )
}

#[allow(clippy::too_many_lines)] // Ownership rechecks and final replacement state stay in one auditable flow.
fn inspect_with_inspector(
    inspector: &mut impl UpgradeProcessInspector,
    current_executable: Option<&Path>,
    target: Option<&Path>,
    target_source: UpgradeTargetSource,
    leases: &ActiveWorkerLeaseInspection,
    runtime_images: UpgradeRuntimeImageState,
    drain_requested: bool,
) -> UpgradePreflight {
    let boundaries = UpgradePreflightBoundaries {
        default_read_only: !drain_requested,
        explicit_drain_only: true,
        raw_command_lines: false,
        raw_native_session_ids: false,
    };
    let Some(target) = target else {
        return UpgradePreflight {
            schema_version: UPGRADE_PREFLIGHT_SCHEMA_VERSION,
            tabbeacon_version: env!("CARGO_PKG_VERSION").to_owned(),
            current_executable: current_executable.map(display_path),
            target_executable: None,
            target_source,
            process_inspection: UpgradeProcessInspection::Unavailable,
            worker_lease_health: leases.health.as_str().to_owned(),
            runtime_images,
            replaceability: UpgradeReplaceability::Unavailable,
            workers: Vec::new(),
            drain_requested,
            drained_owned_workers: 0,
            boundaries,
        };
    };

    let current_process_id = std::process::id();
    let initial_processes = inspector.matching_processes(target);
    let Ok(initial_processes) = initial_processes else {
        return UpgradePreflight {
            schema_version: UPGRADE_PREFLIGHT_SCHEMA_VERSION,
            tabbeacon_version: env!("CARGO_PKG_VERSION").to_owned(),
            current_executable: current_executable.map(display_path),
            target_executable: Some(display_path(target)),
            target_source,
            process_inspection: UpgradeProcessInspection::Unavailable,
            worker_lease_health: leases.health.as_str().to_owned(),
            runtime_images,
            replaceability: UpgradeReplaceability::Unavailable,
            workers: Vec::new(),
            drain_requested,
            drained_owned_workers: 0,
            boundaries,
        };
    };

    let mut workers = classify_processes(
        &initial_processes,
        current_process_id,
        leases,
        drain_requested,
    );
    let mut drained_owned_workers = 0_usize;
    if drain_requested {
        for worker in &mut workers {
            if worker.ownership != UpgradeWorkerOwnership::ProvedTabBeaconWorker {
                worker.drain = UpgradeDrainDisposition::PreservedAmbiguous;
                continue;
            }
            let rechecked = inspector
                .matching_processes(target)
                .ok()
                .map(|processes| classify_processes(&processes, current_process_id, leases, true));
            let still_proved = rechecked.as_ref().is_some_and(|processes| {
                processes.iter().any(|candidate| {
                    candidate.process_id == worker.process_id
                        && candidate.ownership == UpgradeWorkerOwnership::ProvedTabBeaconWorker
                })
            });
            if !still_proved {
                worker.drain = UpgradeDrainDisposition::RefusedAtRecheck;
            } else if inspector.terminate_proved_worker(worker.process_id).is_ok() {
                worker.drain = UpgradeDrainDisposition::Drained;
                drained_owned_workers = drained_owned_workers.saturating_add(1);
            } else {
                worker.drain = UpgradeDrainDisposition::Failed;
            }
        }
    }

    let final_processes = if drain_requested {
        inspector.matching_processes(target).ok()
    } else {
        Some(initial_processes)
    };
    let (process_inspection, final_workers, replaceability) = match final_processes {
        Some(processes) => {
            let mut final_workers = if drain_requested {
                classify_processes(&processes, current_process_id, leases, true)
            } else {
                workers.clone()
            };
            if drain_requested {
                for worker in &mut final_workers {
                    if let Some(initial) = workers
                        .iter()
                        .find(|initial| initial.process_id == worker.process_id)
                        && initial.drain != UpgradeDrainDisposition::Eligible
                    {
                        worker.drain = initial.drain;
                    }
                }
            }
            let replaceability = if final_workers.is_empty() {
                if drain_requested {
                    inspector.probe_replaceability(target)
                } else {
                    UpgradeReplaceability::NoKnownTabBeaconLock
                }
            } else {
                UpgradeReplaceability::Blocked
            };
            (
                UpgradeProcessInspection::Available,
                final_workers,
                replaceability,
            )
        }
        None => (
            UpgradeProcessInspection::Unavailable,
            workers,
            UpgradeReplaceability::Unavailable,
        ),
    };

    UpgradePreflight {
        schema_version: UPGRADE_PREFLIGHT_SCHEMA_VERSION,
        tabbeacon_version: env!("CARGO_PKG_VERSION").to_owned(),
        current_executable: current_executable.map(display_path),
        target_executable: Some(display_path(target)),
        target_source,
        process_inspection,
        worker_lease_health: leases.health.as_str().to_owned(),
        runtime_images,
        replaceability,
        workers: final_workers,
        drain_requested,
        drained_owned_workers,
        boundaries,
    }
}

fn runtime_image_state(
    state_root: Option<&Path>,
    leases: &ActiveWorkerLeaseInspection,
) -> UpgradeRuntimeImageState {
    let active_image_hashes = leases
        .runtime_image_hashes
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let Some(state_root) = state_root else {
        return UpgradeRuntimeImageState {
            inspection: UpgradeRuntimeImageInspection::Unavailable,
            active_image_hashes,
            stale_image_count: None,
            cleanup_safe: false,
        };
    };
    let inventory = WorkerRuntimeStore::new(state_root).inspect_read_only();
    let inspection = if inventory.healthy {
        UpgradeRuntimeImageInspection::Available
    } else {
        UpgradeRuntimeImageInspection::Unavailable
    };
    let cleanup_safe = inventory.healthy
        && leases.health == ActivityLeaseHealth::Healthy
        && leases.active_legacy_lease_count == 0
        && leases
            .runtime_image_hashes
            .iter()
            .all(|hash| inventory.image_hashes.contains(hash));
    let stale_image_count = inventory.healthy.then(|| {
        inventory
            .image_hashes
            .difference(&leases.runtime_image_hashes)
            .count()
    });
    UpgradeRuntimeImageState {
        inspection,
        active_image_hashes,
        stale_image_count,
        cleanup_safe,
    }
}

fn classify_processes(
    processes: &[ObservedProcess],
    current_process_id: u32,
    leases: &ActiveWorkerLeaseInspection,
    drain_requested: bool,
) -> Vec<UpgradeWorkerDiagnostic> {
    let lease_state_is_healthy = leases.health == ActivityLeaseHealth::Healthy;
    let mut result = processes
        .iter()
        .filter(|process| process.process_id != 0 && process.process_id != current_process_id)
        .map(|process| {
            let proved = lease_state_is_healthy
                && leases
                    .identities
                    .iter()
                    .any(|identity| worker_command_matches(&process.command_line, identity));
            let ownership = if proved {
                UpgradeWorkerOwnership::ProvedTabBeaconWorker
            } else {
                UpgradeWorkerOwnership::UnownedOrAmbiguous
            };
            let drain = match (drain_requested, ownership) {
                (false, _) => UpgradeDrainDisposition::NotRequested,
                (true, UpgradeWorkerOwnership::ProvedTabBeaconWorker) => {
                    UpgradeDrainDisposition::Eligible
                }
                (true, UpgradeWorkerOwnership::UnownedOrAmbiguous) => {
                    UpgradeDrainDisposition::PreservedAmbiguous
                }
            };
            UpgradeWorkerDiagnostic {
                process_id: process.process_id,
                ownership,
                drain,
            }
        })
        .collect::<Vec<_>>();
    result.sort_by_key(|worker| worker.process_id);
    result
}

fn worker_command_matches(command_line: &str, identity: &ActiveWorkerLeaseIdentity) -> bool {
    let arguments = command_line
        .split_ascii_whitespace()
        .map(|argument| argument.trim_matches('"'))
        .collect::<Vec<_>>();
    arguments.windows(4).any(|window| {
        window[0] == "__activity-worker-v1"
            && window[1] == identity.key_sha256
            && window[2] == identity.generation.to_string()
            && window[3] == identity.revision.to_string()
    })
}

fn select_upgrade_target(
    current_executable: Option<&Path>,
) -> (Option<PathBuf>, UpgradeTargetSource) {
    let installed = cargo_bin_executable();
    if installed.as_ref().is_some_and(|path| path.is_file()) {
        return (installed, UpgradeTargetSource::InstalledCargoBinary);
    }
    match current_executable {
        Some(path) => (
            Some(path.to_owned()),
            UpgradeTargetSource::CurrentExecutable,
        ),
        None => (None, UpgradeTargetSource::Unavailable),
    }
}

fn cargo_bin_executable() -> Option<PathBuf> {
    let cargo_home = env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("USERPROFILE").map(|home| PathBuf::from(home).join(".cargo")))
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))?;
    Some(cargo_home.join("bin").join(if cfg!(windows) {
        "tabbeacon.exe"
    } else {
        "tabbeacon"
    }))
}

fn normalized_path(path: &Path) -> Option<String> {
    fs::canonicalize(path).ok().map(|path| {
        path.to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches("//?/")
            .to_lowercase()
    })
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

struct SystemUpgradeProcessInspector;

impl UpgradeProcessInspector for SystemUpgradeProcessInspector {
    fn matching_processes(&mut self, target: &Path) -> Result<Vec<ObservedProcess>, ()> {
        system_matching_processes(target)
    }

    fn probe_replaceability(&mut self, target: &Path) -> UpgradeReplaceability {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(target)
            .map_or(UpgradeReplaceability::Blocked, |_| {
                UpgradeReplaceability::Ready
            })
    }

    fn terminate_proved_worker(&mut self, process_id: u32) -> Result<(), ()> {
        #[cfg(windows)]
        {
            let Some(taskkill) = system_path("taskkill.exe") else {
                return Err(());
            };
            let mut command = Command::new(taskkill);
            command
                .args(["/PID", &process_id.to_string(), "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(0x0800_0000);
            command
                .status()
                .map_err(|_| ())
                .and_then(|status| if status.success() { Ok(()) } else { Err(()) })
        }
        #[cfg(not(windows))]
        {
            let _ = process_id;
            Err(())
        }
    }
}

#[cfg(windows)]
fn system_matching_processes(target: &Path) -> Result<Vec<ObservedProcess>, ()> {
    let target = normalized_path(target).ok_or(())?;
    let powershell = system_path("WindowsPowerShell\\v1.0\\powershell.exe").ok_or(())?;
    let script = r"
$target = $env:TABBEACON_UPGRADE_TARGET
Get-CimInstance -ClassName Win32_Process -ErrorAction Stop |
  Where-Object {
    -not [string]::IsNullOrWhiteSpace($_.ExecutablePath) -and
    (([IO.Path]::GetFullPath($_.ExecutablePath).Replace('\','/').ToLowerInvariant()).Replace('//?/','') -eq $target)
  } |
  Select-Object -First 64 |
  ForEach-Object {
    $line = [string]$_.CommandLine
    if ($line.Length -gt 4096) { $line = $line.Substring(0, 4096) }
    [pscustomobject]@{ process_id = [uint32]$_.ProcessId; command_line = $line }
  } |
  ConvertTo-Json -Compress
";
    let output = Command::new(powershell)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            script,
        ])
        .env("TABBEACON_UPGRADE_TARGET", target)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(0x0800_0000)
        .output()
        .map_err(|_| ())?;
    if !output.status.success() {
        return Err(());
    }
    let output = String::from_utf8(output.stdout).map_err(|_| ())?;
    parse_processes(&output)
}

#[cfg(not(windows))]
fn system_matching_processes(_target: &Path) -> Result<Vec<ObservedProcess>, ()> {
    Err(())
}

#[cfg(windows)]
fn system_path(suffix: &str) -> Option<PathBuf> {
    let path = PathBuf::from(env::var_os("SystemRoot")?)
        .join("System32")
        .join(suffix);
    path.is_file().then_some(path)
}

fn parse_processes(output: &str) -> Result<Vec<ObservedProcess>, ()> {
    let text = output.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let value = serde_json::from_str::<Value>(text).map_err(|_| ())?;
    let values = match value {
        Value::Array(values) => values,
        Value::Object(_) => vec![value],
        _ => return Err(()),
    };
    if values.len() > 64 {
        return Err(());
    }
    let mut processes = Vec::with_capacity(values.len());
    for value in values {
        let process_id = value
            .get("process_id")
            .and_then(Value::as_u64)
            .and_then(|process_id| u32::try_from(process_id).ok())
            .filter(|process_id| *process_id != 0)
            .ok_or(())?;
        let command_line = value
            .get("command_line")
            .and_then(Value::as_str)
            .filter(|command_line| command_line.len() <= 4096)
            .ok_or(())?
            .to_owned();
        processes.push(ObservedProcess {
            process_id,
            command_line,
        });
    }
    processes.sort_by_key(|process| process.process_id);
    if processes
        .windows(2)
        .any(|pair| pair[0].process_id == pair[1].process_id)
    {
        return Err(());
    }
    Ok(processes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeInspector {
        processes: Vec<ObservedProcess>,
        stopped: Vec<u32>,
    }

    impl UpgradeProcessInspector for FakeInspector {
        fn matching_processes(&mut self, _target: &Path) -> Result<Vec<ObservedProcess>, ()> {
            Ok(self.processes.clone())
        }

        fn probe_replaceability(&mut self, _target: &Path) -> UpgradeReplaceability {
            UpgradeReplaceability::Ready
        }

        fn terminate_proved_worker(&mut self, process_id: u32) -> Result<(), ()> {
            self.stopped.push(process_id);
            self.processes
                .retain(|process| process.process_id != process_id);
            Ok(())
        }
    }

    fn identities() -> ActiveWorkerLeaseInspection {
        ActiveWorkerLeaseInspection {
            health: ActivityLeaseHealth::Healthy,
            identities: vec![ActiveWorkerLeaseIdentity {
                key_sha256: "a".repeat(64),
                generation: 7,
                revision: 3,
            }],
            runtime_image_hashes: std::collections::BTreeSet::new(),
            active_legacy_lease_count: 0,
        }
    }

    fn runtime_images() -> UpgradeRuntimeImageState {
        UpgradeRuntimeImageState {
            inspection: UpgradeRuntimeImageInspection::Available,
            active_image_hashes: Vec::new(),
            stale_image_count: Some(0),
            cleanup_safe: true,
        }
    }

    #[test]
    fn default_preflight_is_read_only_and_preserves_ambiguous_processes() {
        let target = PathBuf::from("C:/Users/test/.cargo/bin/tabbeacon.exe");
        let mut inspector = FakeInspector {
            processes: vec![
                ObservedProcess {
                    process_id: 41,
                    command_line: format!(
                        "tabbeacon.exe __activity-worker-v1 {} 7 3",
                        "a".repeat(64)
                    ),
                },
                ObservedProcess {
                    process_id: 42,
                    command_line: "tabbeacon.exe status".to_owned(),
                },
            ],
            stopped: Vec::new(),
        };

        let report = inspect_with_inspector(
            &mut inspector,
            None,
            Some(&target),
            UpgradeTargetSource::InstalledCargoBinary,
            &identities(),
            runtime_images(),
            false,
        );

        assert!(report.boundaries.default_read_only);
        assert_eq!(report.proved_owned_worker_count(), 1);
        assert_eq!(report.ambiguous_process_count(), 1);
        assert_eq!(report.replaceability, UpgradeReplaceability::Blocked);
        assert!(inspector.stopped.is_empty());
        assert!(
            report
                .workers
                .iter()
                .all(|worker| { worker.drain == UpgradeDrainDisposition::NotRequested })
        );
    }

    #[test]
    fn explicit_drain_rechecks_and_stops_only_the_proved_worker() {
        let target = PathBuf::from("C:/Users/test/.cargo/bin/tabbeacon.exe");
        let mut inspector = FakeInspector {
            processes: vec![
                ObservedProcess {
                    process_id: 51,
                    command_line: format!(
                        "tabbeacon.exe __activity-worker-v1 {} 7 3",
                        "a".repeat(64)
                    ),
                },
                ObservedProcess {
                    process_id: 52,
                    command_line: "tabbeacon.exe __activity-cleanup-observer-v1 51".to_owned(),
                },
            ],
            stopped: Vec::new(),
        };

        let report = inspect_with_inspector(
            &mut inspector,
            None,
            Some(&target),
            UpgradeTargetSource::InstalledCargoBinary,
            &identities(),
            runtime_images(),
            true,
        );

        assert_eq!(inspector.stopped, vec![51]);
        assert_eq!(report.drained_owned_workers, 1);
        assert_eq!(report.ambiguous_process_count(), 1);
        assert_eq!(report.replaceability, UpgradeReplaceability::Blocked);
        assert!(report.boundaries.explicit_drain_only);
    }

    #[test]
    fn preflight_after_an_owned_drain_reports_replaceable_target() {
        let target = PathBuf::from("C:/Users/test/.cargo/bin/tabbeacon.exe");
        let mut inspector = FakeInspector {
            processes: vec![ObservedProcess {
                process_id: 61,
                command_line: format!("tabbeacon.exe __activity-worker-v1 {} 7 3", "a".repeat(64)),
            }],
            stopped: Vec::new(),
        };

        let report = inspect_with_inspector(
            &mut inspector,
            None,
            Some(&target),
            UpgradeTargetSource::InstalledCargoBinary,
            &identities(),
            runtime_images(),
            true,
        );

        assert_eq!(inspector.stopped, vec![61]);
        assert!(report.workers.is_empty());
        assert_eq!(report.replaceability, UpgradeReplaceability::Ready);
    }

    #[test]
    fn preflight_without_a_matching_process_reports_no_known_tabbeacon_lock() {
        let target = PathBuf::from("C:/Users/test/.cargo/bin/tabbeacon.exe");
        let mut inspector = FakeInspector::default();

        let report = inspect_with_inspector(
            &mut inspector,
            None,
            Some(&target),
            UpgradeTargetSource::InstalledCargoBinary,
            &identities(),
            runtime_images(),
            false,
        );

        assert!(report.workers.is_empty());
        assert_eq!(
            report.replaceability,
            UpgradeReplaceability::NoKnownTabBeaconLock
        );
        assert!(inspector.stopped.is_empty());
    }

    #[test]
    fn malformed_process_probe_is_not_treated_as_an_empty_process_list() {
        assert!(parse_processes("not-json").is_err());
        assert!(parse_processes(r#"[{"process_id":7}]"#).is_err());
        assert_eq!(
            parse_processes(" ").expect("empty output is valid"),
            Vec::new()
        );
    }

    #[test]
    fn runtime_image_state_reports_cleanup_only_after_lease_and_image_proof() {
        let root = std::env::temp_dir().join(format!(
            "tabbeacon-upgrade-preflight-runtime-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("test clock is after epoch")
                .as_nanos()
        ));
        let source = root.join("installed.exe");
        std::fs::create_dir_all(&root).expect("isolated state root creates");
        std::fs::write(&source, b"runtime-preflight-test-image").expect("source writes");
        let store = WorkerRuntimeStore::new(root.join("state"));
        let image = store.publish(&source).expect("runtime image publishes");
        let leases = ActiveWorkerLeaseInspection {
            health: ActivityLeaseHealth::Healthy,
            identities: Vec::new(),
            runtime_image_hashes: std::collections::BTreeSet::from([image.content_sha256.clone()]),
            active_legacy_lease_count: 0,
        };
        let report = runtime_image_state(Some(&root.join("state")), &leases);
        assert_eq!(report.inspection, UpgradeRuntimeImageInspection::Available);
        assert_eq!(report.active_image_hashes, vec![image.content_sha256]);
        assert_eq!(report.stale_image_count, Some(0));
        assert!(report.cleanup_safe);

        let legacy = ActiveWorkerLeaseInspection {
            active_legacy_lease_count: 1,
            ..leases
        };
        assert!(!runtime_image_state(Some(&root.join("state")), &legacy).cleanup_safe);
        let _ = std::fs::remove_dir_all(root);
    }
}
