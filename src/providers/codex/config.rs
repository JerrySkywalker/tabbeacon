use std::{
    collections::{BTreeMap, BTreeSet},
    env, fmt,
    fs::{self, File, OpenOptions},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use toml_edit::{Array, DocumentMut, Item, Table, value};

use crate::{
    activity::{
        ACTIVITY_WORKER_PROBE_PROCESS_FILE, ACTIVITY_WORKER_PROBE_RECEIPT_ENV,
        ACTIVITY_WORKER_PROBE_RECEIPT_FILE, ACTIVITY_WORKER_PROBE_STARTED_FILE,
        TARGET_FRAME_INTERVAL_MS,
    },
    hook_inventory::{
        HookCurrentness, HookHandlerKind, HookInventory, HookInventoryEntry, HookOwner,
        HookSourceKind, HookTrustState,
    },
};

use super::{
    CodexCompatibilityRegistry, CodexCompatibilityState, CodexHookProfile, MCP_HOOK_SERVER_NAME,
    MCP_HOOK_TOOL_NAME, hook_input_template,
    runtime::{SESSION_END_PROBE_RECEIPT_ENV, SESSION_END_PROBE_RECEIPT_FILE},
};

const MANIFEST_SCHEMA: &str = "tabbeacon-codex-integration-v1";
const MANIFEST_FILE: &str = "integration-v1.json";
const LOCK_FILE: &str = "integration.lock";
const OWNED_DESCRIPTION: &str = "TabBeacon user-global lifecycle hooks";
const RUNTIME_PROBE_TIMEOUT: Duration = Duration::from_millis(900);
// The MCP probe owns one fresh stdio server plus an immutable worker image,
// then models Codex 0.149 terminating that server before transport close. It
// proves the independent command SessionEnd cleanup in the same isolated state.
// These are diagnostic phase bounds, not Hook-runtime budgets: the release
// performance probe separately enforces the one-second declaration and p99
// contract under representative warm and c8 load.
const MCP_ACTIVITY_RUNTIME_PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const MCP_TERMINATION_RUNTIME_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
const SESSION_END_CLEANUP_RUNTIME_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const RUNTIME_PROBE_EVENT: &str = "UserPromptSubmit";
const HOOK_EVENTS: [&str; 11] = [
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PreCompact",
    "PostCompact",
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "SubagentStart",
    "SubagentStop",
    "Stop",
];
type ProbedCodexProfile = (String, CodexCompatibilityState);

/// Result of a setup invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupOutcome {
    /// The supported Codex user layer was updated and now needs hook review.
    InstalledTrustReviewRequired,
    /// Exact owned declarations were atomically replaced with the current form.
    Upgraded,
    /// The exact owned integration was already present; no file was rewritten.
    AlreadyInstalled,
}

/// Whether the observed Codex version authorizes a configuration mutation.
///
/// This is deliberately independent from [`CodexRuntimeContinuity`]: a known
/// installed integration may continue to decorate a future Codex runtime
/// without granting that future version setup, repair, or reconciliation
/// authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexMutationAuthority {
    /// The detected version has an exact source-audited profile.
    Admitted,
    /// The detected version has no exact source-admitted profile.
    Blocked,
}

impl CodexMutationAuthority {
    /// Stable machine-readable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Blocked => "blocked",
        }
    }
}

/// Whether an already-installed Hook integration can continue at runtime.
///
/// This describes only the installed, manifest-proven command Hook surface.
/// It never upgrades an unadmitted version into a source-admitted profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexRuntimeContinuity {
    /// An admitted version has a fully proven installed integration.
    Admitted,
    /// An unadmitted version retains a fully proven known installed wire shape.
    PreservedUnadmitted,
    /// Required installation, wire-shape, trust, or title proof is absent.
    Unproven,
}

impl CodexRuntimeContinuity {
    /// Stable machine-readable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::PreservedUnadmitted => "preserved_unadmitted",
            Self::Unproven => "unproven",
        }
    }
}

/// Stable disposition of a preview-first owned Hook repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexRepairDisposition {
    /// The preflight proved which exact manifest-owned groups may be restored.
    ReadyToApply,
    /// Exact missing groups were restored without changing Hook trust.
    RepairedTrustReviewRequired,
    /// Every exact manifest-owned declaration is already present.
    AlreadyExact,
}

/// Content-minimal result of an owned Codex Hook repair preflight or apply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CodexRepairReport {
    /// Stable repair result schema.
    pub schema_version: u32,
    /// Whether this was a preview, an apply, or an idempotent exact result.
    pub disposition: CodexRepairDisposition,
    /// Number of exact manifest-owned declarations proven absent.
    pub missing_declarations: usize,
    /// Digest of the exact Hook target observed during this preflight.
    ///
    /// An apply must present this value unchanged, so a preview cannot be
    /// replayed after a concurrent edit to `hooks.json`.
    pub target_digest: String,
    /// Number of non-TabBeacon Hook groups preserved without mutation.
    pub third_party_groups_preserved: usize,
    /// Number of preserved groups that were added after the verified
    /// pre-install Hook backup.
    pub postinstall_third_party_groups_preserved: usize,
    /// Repair never grants Codex Hook trust; the Owner must review `/hooks`.
    pub manual_hook_trust_review_required: bool,
}

/// Result of an uninstall invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallOutcome {
    /// Exact owned declarations were removed and the prior title value restored.
    Removed,
    /// No ownership manifest exists, so no external file was touched.
    NotInstalled,
}

/// Result of reconciling the optional Codex terminal-title ownership layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleOwnershipOutcome {
    /// The owned Codex title setting was changed safely.
    Updated,
    /// The requested ownership was already exact.
    AlreadyConfigured,
    /// No `TabBeacon` `Codex` integration is installed, so user preferences were saved only.
    NotInstalled,
}

/// Severity of one doctor check and of the aggregate report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DoctorStatus {
    /// The condition is proven correct.
    Pass,
    /// The integration is safe but needs an expected user or compatibility action.
    Warning,
    /// The condition is missing, modified, or incompatible.
    Fail,
}

impl fmt::Display for DoctorStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Pass => "PASS",
            Self::Warning => "WARNING",
            Self::Fail => "FAIL",
        })
    }
}

/// One non-sensitive doctor observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorCheck {
    id: &'static str,
    status: DoctorStatus,
    summary: String,
}

impl DoctorCheck {
    /// Stable machine-oriented check identifier.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        self.id
    }

    /// Check disposition.
    #[must_use]
    pub const fn status(&self) -> DoctorStatus {
        self.status
    }

    /// Non-sensitive result summary.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }
}

/// Complete read-only diagnosis of the current Codex integration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexDoctorReport {
    overall: DoctorStatus,
    checks: Vec<DoctorCheck>,
    codex_version: Option<String>,
    compatibility_state: CodexCompatibilityState,
    mutation_authority: CodexMutationAuthority,
    runtime_continuity: CodexRuntimeContinuity,
    hook_profile: Option<CodexHookProfile>,
    owned_hook_count: Option<usize>,
    title_owned: Option<bool>,
}

impl CodexDoctorReport {
    fn from_diagnosis(
        checks: Vec<DoctorCheck>,
        codex_version: Option<String>,
        compatibility_state: CodexCompatibilityState,
        mutation_authority: CodexMutationAuthority,
        runtime_continuity: CodexRuntimeContinuity,
        hook_profile: Option<CodexHookProfile>,
        ownership: (Option<usize>, Option<bool>),
    ) -> Self {
        let overall = checks
            .iter()
            .map(DoctorCheck::status)
            .max()
            .unwrap_or(DoctorStatus::Fail);
        Self {
            overall,
            checks,
            codex_version,
            compatibility_state,
            mutation_authority,
            runtime_continuity,
            hook_profile,
            owned_hook_count: ownership.0,
            title_owned: ownership.1,
        }
    }

    /// Aggregate severity (the strongest individual check disposition).
    #[must_use]
    pub const fn overall(&self) -> DoctorStatus {
        self.overall
    }

    /// Ordered diagnostic checks.
    #[must_use]
    pub fn checks(&self) -> &[DoctorCheck] {
        &self.checks
    }

    /// Detected Codex semantic version, when the executable could be probed.
    #[must_use]
    pub fn codex_version(&self) -> Option<&str> {
        self.codex_version.as_deref()
    }

    /// Exact source-audited Hook profile, when the detected version is supported.
    #[must_use]
    pub const fn hook_profile(&self) -> Option<CodexHookProfile> {
        self.hook_profile
    }

    /// Exact registry classification, including unadmitted and unavailable states.
    #[must_use]
    pub const fn compatibility_state(&self) -> CodexCompatibilityState {
        self.compatibility_state
    }

    /// Whether this observed version authorizes a setup or reconciliation mutation.
    #[must_use]
    pub const fn mutation_authority(&self) -> CodexMutationAuthority {
        self.mutation_authority
    }

    /// Whether the independently proven installed Hook surface may continue at runtime.
    #[must_use]
    pub const fn runtime_continuity(&self) -> CodexRuntimeContinuity {
        self.runtime_continuity
    }

    /// Whether the detected Codex version maps to an admitted Hook profile.
    #[must_use]
    pub const fn profile_supported(&self) -> bool {
        self.compatibility_state.is_supported()
    }

    /// Count of manifest-owned Hook declarations when the manifest is valid.
    #[must_use]
    pub const fn owned_hook_count(&self) -> Option<usize> {
        self.owned_hook_count
    }

    /// Whether the valid ownership manifest records `TabBeacon` title control.
    #[must_use]
    pub const fn title_owned(&self) -> Option<bool> {
        self.title_owned
    }

    /// Looks up one stable non-sensitive doctor check by identifier.
    #[must_use]
    pub fn check(&self, id: &str) -> Option<&DoctorCheck> {
        self.checks.iter().find(|check| check.id() == id)
    }

    /// Disposition of one stable non-sensitive doctor check.
    #[must_use]
    pub fn check_status(&self, id: &str) -> Option<DoctorStatus> {
        self.check(id).map(DoctorCheck::status)
    }

    fn replace_check(&mut self, replacement: DoctorCheck) {
        if let Some(existing) = self
            .checks
            .iter_mut()
            .find(|check| check.id() == replacement.id())
        {
            *existing = replacement;
        } else {
            self.checks.push(replacement);
        }
        self.overall = self
            .checks
            .iter()
            .map(DoctorCheck::status)
            .max()
            .unwrap_or(DoctorStatus::Fail);
    }
}

/// Safe configuration-management error with no config contents.
#[derive(Debug)]
pub enum CodexIntegrationError {
    /// A required per-user path could not be derived.
    StateRootUnavailable,
    /// The detected Codex version has no source-audited Hook profile.
    UnsupportedCodexVersion,
    /// A managed or external file I/O operation failed.
    Io(io::Error),
    /// The existing hooks JSON is not compatible with the current Codex shape.
    HooksShape,
    /// The existing Codex TOML is not compatible with the current Codex shape.
    ConfigShape,
    /// A `TabBeacon`-like Hook group has no exact ownership proof.
    TabBeaconLikeAmbiguityBlocked,
    /// The verified pre-install Hook backup no longer matches its recorded digest.
    BaselineDriftBlocked,
    /// A TabBeacon-owned hook declaration no longer matches its manifest.
    ModifiedOwnedHook,
    /// Manifest-owned declarations are exact but do not match the current admitted source shape.
    StaleOwnedHook,
    /// The terminal-title value owned by setup was modified afterward.
    ModifiedOwnedTitle,
    /// A title configuration not owned by `TabBeacon` conflicts with integration.
    TerminalTitleConflict,
    /// The ownership manifest is absent, corrupt, or belongs to another target.
    OwnershipManifest,
    /// A managed target changed after repair preflight and before its write.
    ConcurrentTargetDrift,
    /// An apply was requested without the target digest returned by preview.
    RepairPreviewDigestRequired,
    /// The supplied preview digest is not a valid SHA-256 target digest.
    RepairPreviewDigestInvalid,
    /// The executable path cannot be represented safely in a Windows command.
    UnsafeExecutablePath,
    /// A target path or ancestor is a symbolic link/reparse point.
    SymbolicLinkTarget,
}

impl fmt::Display for CodexIntegrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::StateRootUnavailable => "a safe per-user integration path is unavailable",
            Self::UnsupportedCodexVersion => {
                "the detected Codex version has no source-audited Hook profile"
            }
            Self::Io(_) => "an integration file operation failed",
            Self::HooksShape => "the Codex hooks file has an unsupported shape",
            Self::ConfigShape => "the Codex config file has an unsupported shape",
            Self::TabBeaconLikeAmbiguityBlocked => {
                "a TabBeacon-like Hook group exists without exact ownership proof"
            }
            Self::BaselineDriftBlocked => {
                "the verified pre-install Hook baseline changed after installation"
            }
            Self::ModifiedOwnedHook => "a TabBeacon-owned hook was modified",
            Self::StaleOwnedHook => {
                "manifest-owned hooks are not current for the admitted Codex profile"
            }
            Self::ModifiedOwnedTitle => "the TabBeacon-owned terminal-title setting was modified",
            Self::TerminalTitleConflict => {
                "Codex terminal-title ownership conflicts with TabBeacon"
            }
            Self::OwnershipManifest => "the Codex integration ownership manifest is invalid",
            Self::ConcurrentTargetDrift => {
                "a Codex integration target changed during repair preflight"
            }
            Self::RepairPreviewDigestRequired => {
                "repair apply requires the target digest returned by a fresh preview"
            }
            Self::RepairPreviewDigestInvalid => {
                "the supplied repair preview target digest is invalid"
            }
            Self::UnsafeExecutablePath => {
                "the TabBeacon executable path is unsafe for a Codex Windows command hook"
            }
            Self::SymbolicLinkTarget => {
                "a Codex integration target is a symbolic link or reparse point"
            }
        })
    }
}

impl CodexIntegrationError {
    /// Stable, content-minimal repair diagnostic classification.
    #[must_use]
    pub const fn repair_failure_class(&self) -> &'static str {
        match self {
            Self::UnsupportedCodexVersion => "UNKNOWN_VERSION_MUTATION_BLOCKED",
            Self::HooksShape => "UNKNOWN_HOOK_WIRE_BLOCKED",
            Self::TabBeaconLikeAmbiguityBlocked => "TABBEACON_LIKE_AMBIGUITY_BLOCKED",
            Self::BaselineDriftBlocked => "BASELINE_DRIFT_BLOCKED",
            Self::ModifiedOwnedHook => "MODIFIED_OWNED_GROUP_BLOCKED",
            Self::StaleOwnedHook => "STALE_OWNED_DECLARATION_BLOCKED",
            Self::ConcurrentTargetDrift => "CONCURRENT_DRIFT_REFUSAL",
            Self::RepairPreviewDigestRequired => "PREVIEW_TARGET_DIGEST_REQUIRED",
            Self::RepairPreviewDigestInvalid => "PREVIEW_TARGET_DIGEST_INVALID",
            Self::OwnershipManifest => "OWNERSHIP_MANIFEST_BLOCKED",
            Self::ModifiedOwnedTitle | Self::TerminalTitleConflict => "TITLE_OWNERSHIP_BLOCKED",
            Self::SymbolicLinkTarget => "UNSAFE_TARGET_PATH_BLOCKED",
            Self::UnsafeExecutablePath => "UNSAFE_EXECUTABLE_PATH_BLOCKED",
            Self::StateRootUnavailable | Self::ConfigShape | Self::Io(_) => "REPAIR_BLOCKED",
        }
    }
}

impl std::error::Error for CodexIntegrationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for CodexIntegrationError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// Ownership-aware manager for the supported user-global Codex layer.
#[derive(Debug, Clone)]
pub struct CodexIntegration {
    codex_home: PathBuf,
    state_root: PathBuf,
    tabbeacon_executable: PathBuf,
    codex_program: Option<PathBuf>,
}

impl CodexIntegration {
    /// Creates an integration manager using explicitly injected paths.
    #[must_use]
    pub fn new(
        codex_home: impl Into<PathBuf>,
        state_root: impl Into<PathBuf>,
        tabbeacon_executable: impl Into<PathBuf>,
    ) -> Self {
        Self {
            codex_home: codex_home.into(),
            state_root: state_root.into(),
            tabbeacon_executable: tabbeacon_executable.into(),
            codex_program: None,
        }
    }

    /// Overrides the Codex probe executable for isolated compatibility tests.
    #[must_use]
    pub fn with_codex_program(mut self, codex_program: impl Into<PathBuf>) -> Self {
        self.codex_program = Some(codex_program.into());
        self
    }

    /// Resolves the current user's supported Codex and `TabBeacon` state roots.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the user profile, local application data, or
    /// current `TabBeacon` executable cannot be resolved safely.
    pub fn from_environment() -> Result<Self, CodexIntegrationError> {
        let codex_home = env::var_os("CODEX_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("USERPROFILE")
                    .filter(|value| !value.is_empty())
                    .map(PathBuf::from)
                    .map(|path| path.join(".codex"))
            })
            .ok_or(CodexIntegrationError::StateRootUnavailable)?;
        let state_root = env::var_os("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .map(|path| path.join("TabBeacon").join("codex-integration"))
            .ok_or(CodexIntegrationError::StateRootUnavailable)?;
        Ok(Self::new(codex_home, state_root, env::current_exe()?))
    }

    /// Installs or verifies the exact owned user-global hook integration.
    ///
    /// # Errors
    ///
    /// Refuses unsupported config shapes, unowned matching hooks, symbolic-link
    /// targets, or drift in an existing owned integration.
    pub fn setup(&self) -> Result<SetupOutcome, CodexIntegrationError> {
        self.setup_with_title_ownership(true)
    }

    /// Installs or upgrades hooks while applying the requested title owner.
    ///
    /// The caller derives this from provider-neutral presentation preferences;
    /// the integration never accepts raw TOML or executable configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if exact hook ownership, the Codex TOML shape, or a
    /// required atomic external-file update cannot be proven safe.
    pub fn setup_with_title_ownership(
        &self,
        tabbeacon_owns_title: bool,
    ) -> Result<SetupOutcome, CodexIntegrationError> {
        // Do not create even TabBeacon's private lock/state root for an
        // unadmitted version. The same admission is repeated under the lock
        // immediately before mutation to prevent a version-swap race.
        self.require_supported_profile()?;
        self.with_lock(|| {
            let profile = self.require_supported_profile()?;
            self.setup_locked(tabbeacon_owns_title, profile)
        })
    }

    /// Previews or applies an ownership-safe restoration of missing exact Hook groups.
    ///
    /// A preview is fully read-only. Apply repeats the complete preflight while
    /// holding the integration lock, writes only the Hook file, and deliberately
    /// leaves Codex trust state, the ownership manifest, and title configuration
    /// unchanged. An unadmitted Codex version cannot use this mutation path.
    ///
    /// # Errors
    ///
    /// Refuses invalid ownership, stale source declarations, symbolic targets,
    /// malformed wire shapes, and every TabBeacon-like unowned or modified group.
    pub fn repair(
        &self,
        apply: bool,
        expected_target_digest: Option<&str>,
    ) -> Result<CodexRepairReport, CodexIntegrationError> {
        if apply {
            // Keep an unadmitted repair fully read-only, including TabBeacon's
            // own state root; repeat the probe under lock before writing.
            self.require_supported_profile()?;
            self.with_lock(|| {
                let profile = self.require_supported_profile()?;
                self.repair_locked(profile, true, expected_target_digest)
            })
        } else {
            let profile = self.require_supported_profile()?;
            self.repair_locked(profile, false, expected_target_digest)
        }
    }

    /// Removes only exact owned declarations and restores the prior title value.
    ///
    /// # Errors
    ///
    /// Performs a full ownership preflight before mutation and refuses modified
    /// owned elements.
    pub fn uninstall(&self) -> Result<UninstallOutcome, CodexIntegrationError> {
        self.with_lock(|| self.uninstall_locked())
    }

    /// Reconciles only the title ownership part of an already installed integration.
    ///
    /// The original pre-install title value remains in the manifest so a later
    /// uninstall restores the A-before-upgrade baseline rather than a transient
    /// user preference.
    ///
    /// # Errors
    ///
    /// Returns an error if the installed integration, owned hooks, or current
    /// title declaration cannot be proven safe to update.
    pub fn reconcile_title_ownership(
        &self,
        tabbeacon_owns_title: bool,
    ) -> Result<TitleOwnershipOutcome, CodexIntegrationError> {
        self.require_supported_profile()?;
        self.with_lock(|| {
            self.require_supported_profile()?;
            self.reconcile_title_ownership_locked(tabbeacon_owns_title)
        })
    }

    /// Audits binary, manifest, hook, trust, and terminal-title state read-only.
    #[must_use]
    #[allow(clippy::too_many_lines)] // Ordered read-only checks are the public doctor contract.
    pub fn doctor(&self) -> CodexDoctorReport {
        let mut checks = Vec::new();
        let version = self.probe_codex_version();
        let codex_version = version.as_ref().map(|(version, _)| version.clone());
        let compatibility_state = compatibility_state(version.as_ref());
        let hook_profile = compatibility_state.supported_profile();
        let mutation_authority = if compatibility_state.is_supported() {
            CodexMutationAuthority::Admitted
        } else {
            CodexMutationAuthority::Blocked
        };
        checks.push(codex_version_check(version.as_ref()));
        checks.push(codex_profile_check(version.as_ref()));
        let executable_present = self.tabbeacon_executable.is_file();
        checks.push(if executable_present {
            pass("tabbeacon.executable", "managed hook executable exists")
        } else {
            fail("tabbeacon.executable", "managed hook executable is missing")
        });

        let manifest = self
            .load_manifest()
            .ok()
            .flatten()
            .filter(|manifest| self.validate_manifest_scope(manifest).is_ok());
        let manifest_has_known_owned_declarations = manifest
            .as_ref()
            .is_some_and(Self::manifest_has_known_owned_declarations);
        let owned_hook_count = manifest.as_ref().map(|manifest| manifest.hooks.len());
        let title_owned = manifest.as_ref().map(|manifest| manifest.title_owned);
        checks.push(if manifest.is_some() {
            pass(
                "ownership.manifest",
                "ownership manifest is present and parseable",
            )
        } else {
            fail(
                "ownership.manifest",
                "ownership manifest is missing or invalid",
            )
        });

        let hooks = read_hooks_document(&self.hooks_path());
        let config = read_config_document(&self.config_path());
        let known_wire_shape = hooks
            .as_ref()
            .is_ok_and(|hooks| validate_known_hook_wire_shape(hooks).is_ok());
        let mcp_server_exact = match (&manifest, &config) {
            (Some(manifest), Ok(config)) => {
                Self::validate_mcp_server_ownership(manifest, config).is_ok()
            }
            _ => false,
        };
        checks.push(match (&manifest, &config) {
            (
                Some(IntegrationManifest {
                    mcp_server: Some(_),
                    ..
                }),
                Ok(_),
            ) if mcp_server_exact => pass(
                "mcp.server",
                "MCP_SERVER_EXACT: owned session-scoped TabBeacon MCP server is exact",
            ),
            (
                Some(IntegrationManifest {
                    mcp_server: Some(_),
                    ..
                }),
                Ok(_),
            ) => fail(
                "mcp.server",
                "MCP_SERVER_MODIFIED_OR_MISSING: owned TabBeacon MCP server is not exact",
            ),
            (Some(_), Ok(_)) => pass(
                "mcp.server",
                "MCP_SERVER_NA: this exact legacy command transport owns no MCP server",
            ),
            _ => fail(
                "mcp.server",
                "MCP_SERVER_UNPROVEN: Codex config is unavailable",
            ),
        });
        let mcp_terminal_binding_exact = match &manifest {
            Some(IntegrationManifest {
                mcp_server: Some(server),
                ..
            }) => {
                mcp_server_exact && server.env_vars.len() == 1 && server.env_vars[0] == "WT_SESSION"
            }
            // The admitted 0.147 command transport owns no MCP child, so the
            // forwarding contract is outside that profile's risk surface.
            Some(IntegrationManifest {
                mcp_server: None, ..
            }) => true,
            None => false,
        };
        checks.push(match (&manifest, &config) {
            (
                Some(IntegrationManifest {
                    mcp_server: Some(_),
                    ..
                }),
                Ok(_),
            ) if mcp_terminal_binding_exact => pass(
                "mcp.terminal-binding",
                "MCP_WT_SESSION_FORWARDING_EXACT: owned MCP child forwards only WT_SESSION for terminal activity binding",
            ),
            (
                Some(IntegrationManifest {
                    mcp_server: Some(_),
                    ..
                }),
                Ok(_),
            ) => fail(
                "mcp.terminal-binding",
                "MCP_WT_SESSION_FORWARDING_MISSING_OR_MODIFIED: owned MCP terminal binding must declare exactly WT_SESSION",
            ),
            (Some(_), Ok(_)) => pass(
                "mcp.terminal-binding",
                "MCP_WT_SESSION_FORWARDING_NA: this exact legacy command transport owns no MCP child",
            ),
            _ => fail(
                "mcp.terminal-binding",
                "MCP_WT_SESSION_FORWARDING_UNPROVEN: owned MCP terminal binding cannot be verified",
            ),
        });
        let declarations_exact = match (&manifest, &hooks) {
            (Some(manifest), Ok(hooks))
                if manifest_has_known_owned_declarations
                    && known_wire_shape
                    && mcp_server_exact =>
            {
                locate_owned_hooks(hooks, &manifest.hooks)
                    .is_ok_and(|locations| locations.len() == manifest.hooks.len())
            }
            _ => false,
        };
        checks.push(if declarations_exact {
            pass(
                "hooks.declarations",
                "DECLARATION_EXACT: all owned hook declarations are exact",
            )
        } else {
            fail(
                "hooks.declarations",
                "DECLARATION_MODIFIED: owned hooks are missing, modified, or use an incompatible wire shape",
            )
        });
        checks.push(match (&manifest, hook_profile) {
            (Some(manifest), Some(profile)) => match (
                desired_hooks(&self.tabbeacon_executable, profile),
                desired_mcp_server(&self.tabbeacon_executable, profile),
            ) {
                (Ok(desired), Ok(desired_mcp))
                    if desired == manifest.hooks
                        && desired_mcp == manifest.mcp_server
                        && mcp_server_exact => pass(
                    "hooks.currentness",
                    "CURRENTNESS_CURRENT: owned hook declarations match the current TabBeacon integration",
                ),
                (Ok(_), Ok(_)) => fail(
                    "hooks.currentness",
                    "CURRENTNESS_STALE: owned hook declarations require a TabBeacon upgrade",
                ),
                _ => fail(
                    "hooks.currentness",
                    "CURRENTNESS_UNPROVEN: current TabBeacon hook declarations cannot be generated safely",
                ),
            },
            (Some(_), None) if declarations_exact && known_wire_shape => warning(
                "hooks.currentness",
                "CURRENTNESS_MUTATION_BLOCKED: an unadmitted Codex version cannot rewrite the installed declarations",
            ),
            (Some(_) | None, None) => fail(
                "hooks.currentness",
                "CURRENTNESS_UNPROVEN: Codex hook profile is not source-audited",
            ),
            (None, Some(_)) => fail(
                "hooks.currentness",
                "CURRENTNESS_UNPROVEN: ownership manifest is missing or hooks are incompatible",
            ),
        });
        let trust_check = match (&manifest, &hooks, &config) {
            (Some(manifest), Ok(hooks), Ok(config)) if known_wire_shape && declarations_exact => {
                hook_trust_check(config, &self.hooks_path(), hooks, &manifest.hooks)
            }
            _ => fail(
                "hooks.trust",
                "hook trust cannot be proven for this Codex/config shape",
            ),
        };
        let trust_exact = trust_check.status() == DoctorStatus::Pass;
        checks.push(trust_check);
        let title_check = match (&manifest, &config) {
            (Some(manifest), Ok(config))
                if manifest.title_owned && terminal_title_is_disabled(config).unwrap_or(false) =>
            {
                pass("terminal.title", "TabBeacon owns the Codex terminal title")
            }
            (Some(manifest), Ok(config))
                if !manifest.title_owned
                    && !terminal_title_is_disabled(config).unwrap_or(false) =>
            {
                pass(
                    "terminal.title",
                    "Codex native terminal-title ownership is restored",
                )
            }
            (Some(_), Ok(_)) => fail(
                "terminal.title",
                "Codex terminal-title ownership conflicts with the TabBeacon preference",
            ),
            (None, _) => fail(
                "terminal.title",
                "TabBeacon title ownership is not installed",
            ),
            (_, Err(_)) => fail("terminal.title", "Codex config is incompatible"),
        };
        let title_exact = title_check.status() == DoctorStatus::Pass;
        checks.push(title_check);

        checks.push(match mutation_authority {
            CodexMutationAuthority::Admitted => pass(
                "codex.mutation-authority",
                "MUTATION_ADMITTED: exact source-audited Codex profile permits setup and repair preflight",
            ),
            CodexMutationAuthority::Blocked => fail(
                "codex.mutation-authority",
                "MUTATION_BLOCKED: setup, rewrite, repair, and title reconciliation require an exact source admission",
            ),
        });
        let runtime_proven = version.is_some()
            && executable_present
            && manifest.is_some()
            && manifest_has_known_owned_declarations
            && known_wire_shape
            && declarations_exact
            && mcp_server_exact
            && mcp_terminal_binding_exact
            && trust_exact
            && title_exact;
        let runtime_continuity = match (runtime_proven, mutation_authority) {
            (true, CodexMutationAuthority::Admitted) => CodexRuntimeContinuity::Admitted,
            (true, CodexMutationAuthority::Blocked) => CodexRuntimeContinuity::PreservedUnadmitted,
            (false, _) => CodexRuntimeContinuity::Unproven,
        };
        checks.push(match runtime_continuity {
            CodexRuntimeContinuity::Admitted => pass(
                "codex.runtime-continuity",
                "RUNTIME_CONTINUITY_ADMITTED: exact installed integration is active on a source-audited Codex profile",
            ),
            CodexRuntimeContinuity::PreservedUnadmitted => warning(
                "codex.runtime-continuity",
                "RUNTIME_CONTINUITY_PRESERVED: exact installed Hook declarations remain usable; mutation stays blocked pending source admission",
            ),
            CodexRuntimeContinuity::Unproven
                if mutation_authority == CodexMutationAuthority::Admitted => warning(
                    "codex.runtime-continuity",
                    "RUNTIME_CONTINUITY_PENDING: installed Hook declarations, trust, title ownership, or known wire shape is not exact",
                ),
            CodexRuntimeContinuity::Unproven => fail(
                "codex.runtime-continuity",
                "RUNTIME_CONTINUITY_UNPROVEN: installed Hook declarations, trust, title ownership, or known wire shape is not exact",
            ),
        });
        if runtime_proven {
            checks.push(warning(
                "hooks.runtime-probe",
                "RUNTIME_PROBE_REQUIRED: static declaration health is not execution proof; run `tabbeacon doctor --probe-hook-runtime`",
            ));
        }

        CodexDoctorReport::from_diagnosis(
            checks,
            codex_version,
            compatibility_state,
            mutation_authority,
            runtime_continuity,
            hook_profile,
            (owned_hook_count, title_owned),
        )
    }

    /// Audits the installed integration, then executes one exact owned Hook
    /// declaration in isolated temporary state. This never mutates Codex
    /// configuration or Hook trust.
    #[must_use]
    pub fn doctor_with_runtime_probe(&self) -> CodexDoctorReport {
        let mut report = self.doctor();
        let hybrid_transport = report
            .hook_profile()
            .is_some_and(CodexHookProfile::uses_mcp_hook_transport);
        let runtime_checks = self.runtime_execution_probe(&report);
        let hybrid_claims_present = [
            "hooks.mcp-event-transport",
            "hooks.codex-terminate-before-eof",
            "hooks.session-end-cleanup",
        ]
        .into_iter()
        .all(|id| runtime_checks.iter().any(|check| check.id() == id));
        for check in runtime_checks {
            report.replace_check(check);
        }
        if hybrid_transport && !hybrid_claims_present {
            report.replace_check(fail(
                "hooks.mcp-event-transport",
                "MCP_EVENT_TRANSPORT_UNPROVEN: runtime probe preflight did not reach the independent MCP event check",
            ));
            report.replace_check(fail(
                "hooks.codex-terminate-before-eof",
                "CODEX_0149_TERMINATE_BEFORE_EOF_UNPROVEN: runtime probe preflight did not reach the independent MCP process-termination check",
            ));
            report.replace_check(fail(
                "hooks.session-end-cleanup",
                "REAL_SESSION_END_CLEANUP_UNPROVEN: runtime probe preflight did not reach the independent SessionEnd check; EOF remains fallback only",
            ));
        }
        report
    }

    #[allow(clippy::too_many_lines)] // Ordered preflight and exact transport proof remain one diagnostic contract.
    fn runtime_execution_probe(&self, report: &CodexDoctorReport) -> Vec<DoctorCheck> {
        if report.check_status("codex.runtime-continuity") != Some(DoctorStatus::Pass) {
            return vec![fail(
                "hooks.runtime-probe",
                "RUNTIME_PROBE_BLOCKED: exact admitted declarations, trust, and title ownership are required before execution",
            )];
        }

        let Ok(profile) = self.require_supported_profile() else {
            return vec![fail(
                "hooks.runtime-probe",
                "RUNTIME_PROBE_BLOCKED: the Codex Hook profile is no longer admitted",
            )];
        };
        let Ok(desired) = desired_hooks(&self.tabbeacon_executable, profile) else {
            return vec![fail(
                "hooks.runtime-probe",
                "RUNTIME_PROBE_BLOCKED: the current owned declaration cannot be generated safely",
            )];
        };
        let Some(manifest) = self.load_manifest().ok().flatten() else {
            return vec![fail(
                "hooks.runtime-probe",
                "RUNTIME_PROBE_BLOCKED: the ownership manifest is unavailable",
            )];
        };
        let Ok(hooks) = read_hooks_document(&self.hooks_path()) else {
            return vec![fail(
                "hooks.runtime-probe",
                "RUNTIME_PROBE_BLOCKED: the Hook declaration document is unavailable",
            )];
        };
        if manifest.hooks != desired
            || !locate_owned_hooks(&hooks, &manifest.hooks)
                .is_ok_and(|locations| locations.len() == manifest.hooks.len())
        {
            return vec![fail(
                "hooks.runtime-probe",
                "RUNTIME_PROBE_BLOCKED: the owned declaration changed during probe preflight",
            )];
        }
        let outcome = if profile.uses_mcp_hook_transport() {
            let Some(command) = desired
                .iter()
                .find(|hook| hook.event == "SessionEnd")
                .and_then(|hook| hook.group.pointer("/hooks/0/commandWindows"))
                .and_then(Value::as_str)
            else {
                return vec![fail(
                    "hooks.runtime-probe",
                    "RUNTIME_PROBE_BLOCKED: the hybrid SessionEnd command declaration is unavailable",
                )];
            };
            run_windows_mcp_hook_runtime_probe(&self.tabbeacon_executable, command)
        } else {
            let Some(command) = desired
                .iter()
                .find(|hook| hook.event == RUNTIME_PROBE_EVENT)
                .and_then(|hook| hook.group.pointer("/hooks/0/commandWindows"))
                .and_then(Value::as_str)
            else {
                return vec![fail(
                    "hooks.runtime-probe",
                    "RUNTIME_PROBE_BLOCKED: the representative owned declaration is unavailable",
                )];
            };
            run_windows_hook_runtime_probe(command)
        };

        match outcome {
            RuntimeProbeOutcome::Pass => vec![pass(
                "hooks.runtime-probe",
                "RUNTIME_PROBE_PASS: representative owned Hook executed through the bounded COMSPEC fallback",
            )],
            RuntimeProbeOutcome::McpHybrid {
                mcp_event,
                termination,
                session_end,
                frame_interval_ms,
            } => {
                hybrid_runtime_probe_checks(mcp_event, termination, session_end, frame_interval_ms)
            }
            RuntimeProbeOutcome::TimedOut => vec![fail(
                "hooks.runtime-probe",
                if profile.uses_mcp_hook_transport() {
                    "MCP_RUNTIME_PROBE_TIMEOUT: terminal-bound activity proof exceeded the 10 s diagnostic bound"
                } else {
                    "RUNTIME_PROBE_TIMEOUT: representative owned Hook exceeded the 900 ms bound"
                },
            )],
            RuntimeProbeOutcome::NonZero => vec![fail(
                "hooks.runtime-probe",
                "RUNTIME_PROBE_FAILED: representative owned Hook exited nonzero",
            )],
            RuntimeProbeOutcome::MissingMarker => vec![fail(
                "hooks.runtime-probe",
                "RUNTIME_PROBE_FAILED: representative owned Hook did not publish its isolated timing marker",
            )],
            RuntimeProbeOutcome::Unavailable => vec![fail(
                "hooks.runtime-probe",
                "RUNTIME_PROBE_UNAVAILABLE: the bounded Windows Hook probe could not start",
            )],
        }
    }

    /// Produces a provider-neutral, command-redacted Hook inventory without
    /// mutating provider configuration, trust, or ownership state.
    #[must_use]
    #[allow(clippy::too_many_lines)] // The read-only parser keeps ownership and redaction decisions adjacent.
    pub fn hook_inventory(&self) -> HookInventory {
        if [self.hooks_path(), self.config_path(), self.manifest_path()]
            .iter()
            .any(|path| reject_symbolic_link(path).is_err())
        {
            return HookInventory::unavailable();
        }
        let Ok(hooks) = read_hooks_document(&self.hooks_path()) else {
            return HookInventory::unavailable();
        };
        let Ok(config) = read_config_document(&self.config_path()) else {
            return HookInventory::unavailable();
        };
        let manifest = self
            .load_manifest()
            .ok()
            .flatten()
            .filter(|manifest| self.validate_manifest_scope(manifest).is_ok());
        let known_wire_shape = validate_known_hook_wire_shape(&hooks).is_ok();
        if !known_wire_shape {
            return HookInventory::unavailable();
        }
        let runtime_continuity = self.doctor().runtime_continuity();
        let profile_is_supported = self
            .probe_codex_version()
            .is_some_and(|(_, state)| state.is_supported());
        let desired = self
            .probe_codex_version()
            .and_then(|(_, state)| state.supported_profile())
            .and_then(|profile| desired_hooks(&self.tabbeacon_executable, profile).ok());
        let Ok(events) = hooks_events(&hooks) else {
            return HookInventory::unavailable();
        };

        let mut exact_owned_events = BTreeSet::new();
        let mut entries = Vec::new();
        for (event, groups) in events {
            let Some(groups) = groups.as_array() else {
                return HookInventory::unavailable();
            };
            for (group_index, group) in groups.iter().enumerate() {
                let Some(handlers) = group.get("hooks").and_then(Value::as_array) else {
                    return HookInventory::unavailable();
                };
                let matching_declaration = manifest.as_ref().and_then(|manifest| {
                    manifest.hooks.iter().find(|declaration| {
                        declaration.event == *event && declaration.group == *group
                    })
                });
                if matching_declaration.is_some() {
                    exact_owned_events.insert(event.clone());
                }
                for (handler_index, handler) in handlers.iter().enumerate() {
                    let (owner, trust_state, currentness, source_kind, fingerprint) =
                        if let Some(declaration) = matching_declaration {
                            let state_key = inventory_state_key(
                                &self.hooks_path(),
                                event,
                                group_index,
                                handler_index,
                            );
                            let enabled = hook_is_enabled(&config, &state_key);
                            let trust_state = inventory_trust_state(
                                known_wire_shape,
                                enabled,
                                trusted_hash(&config, &state_key),
                                declaration,
                            );
                            let currentness = inventory_currentness(
                                profile_is_supported,
                                desired.as_deref(),
                                declaration,
                                runtime_continuity,
                            );
                            entries.push(HookInventoryEntry::new(
                                "codex",
                                inventory_event_id(event),
                                HookOwner::TabBeacon,
                                enabled,
                                trust_state,
                                currentness,
                                HookSourceKind::ProviderUserGlobal,
                                inventory_handler_kind(handler),
                                inventory_timeout(handler),
                                normalized_hook_hash(declaration),
                            ));
                            continue;
                        } else if contains_tabbeacon_like_group(group) {
                            (
                                HookOwner::UnownedOrAmbiguous,
                                HookTrustState::UnownedOrAmbiguous,
                                HookCurrentness::UnownedOrAmbiguous,
                                HookSourceKind::ProviderUserGlobal,
                                inventory_fingerprint(handler),
                            )
                        } else {
                            (
                                HookOwner::ThirdParty,
                                HookTrustState::UnownedOrAmbiguous,
                                HookCurrentness::UnownedOrAmbiguous,
                                HookSourceKind::ProviderUserGlobal,
                                inventory_fingerprint(handler),
                            )
                        };
                    let enabled = inventory_event_id(event) != "unsupported"
                        && hook_is_enabled(
                            &config,
                            &inventory_state_key(
                                &self.hooks_path(),
                                event,
                                group_index,
                                handler_index,
                            ),
                        );
                    entries.push(HookInventoryEntry::new(
                        "codex",
                        inventory_event_id(event),
                        owner,
                        enabled,
                        trust_state,
                        currentness,
                        source_kind,
                        inventory_handler_kind(handler),
                        inventory_timeout(handler),
                        fingerprint,
                    ));
                }
            }
        }

        if let Some(manifest) = manifest.as_ref() {
            for declaration in &manifest.hooks {
                if exact_owned_events.contains(&declaration.event) {
                    continue;
                }
                entries.push(HookInventoryEntry::new(
                    "codex",
                    inventory_event_id(&declaration.event),
                    HookOwner::UnownedOrAmbiguous,
                    false,
                    HookTrustState::UnownedOrAmbiguous,
                    HookCurrentness::DeclarationModifiedOrMissing,
                    HookSourceKind::OwnedManifestExpectation,
                    HookHandlerKind::Command,
                    inventory_timeout(&declaration.group["hooks"][0]),
                    normalized_hook_hash(declaration),
                ));
            }
        }
        HookInventory::available(entries)
    }

    fn setup_locked(
        &self,
        tabbeacon_owns_title: bool,
        profile: CodexHookProfile,
    ) -> Result<SetupOutcome, CodexIntegrationError> {
        fs::create_dir_all(&self.codex_home)?;
        reject_symbolic_link(&self.hooks_path())?;
        reject_symbolic_link(&self.config_path())?;
        let desired_hooks = desired_hooks(&self.tabbeacon_executable, profile)?;
        let desired_mcp_server = desired_mcp_server(&self.tabbeacon_executable, profile)?;
        if let Some(mut manifest) = self.load_manifest()? {
            self.validate_manifest_scope(&manifest)?;
            let mut hooks = read_hooks_document(&self.hooks_path())?;
            let mut config = read_config_document(&self.config_path())?;
            locate_owned_hooks(&hooks, &manifest.hooks)
                .map_err(|_| CodexIntegrationError::ModifiedOwnedHook)?;
            Self::validate_title_ownership(&manifest, &config)?;
            Self::validate_mcp_server_ownership(&manifest, &config)?;
            let mut config_changed =
                Self::apply_title_ownership(&mut manifest, &mut config, tabbeacon_owns_title)?;
            config_changed |= Self::reconcile_mcp_server_ownership(
                &mut manifest,
                &mut config,
                desired_mcp_server,
            )?;
            let mut changed = config_changed;
            if manifest.hooks != desired_hooks {
                if is_legacy_mcp_session_end_upgrade(&manifest, &desired_hooks) {
                    // Codex 0.149's first MCP transport deliberately omitted
                    // SessionEnd because that event is not admitted as an
                    // mcp_tool Hook. Keep those ten exact declarations (and
                    // therefore their existing Codex trust hashes) in place;
                    // this upgrade adds only the independently reviewable
                    // command cleanup boundary.
                    let session_end = desired_hooks
                        .iter()
                        .filter(|declaration| declaration.event == "SessionEnd")
                        .cloned()
                        .collect::<Vec<_>>();
                    if session_end.len() != 1 {
                        return Err(CodexIntegrationError::OwnershipManifest);
                    }
                    append_owned_hooks(&mut hooks, &session_end)?;
                } else {
                    remove_owned_hooks(&mut hooks, &manifest.hooks)?;
                    append_owned_hooks(&mut hooks, &desired_hooks)?;
                }
                manifest.hooks = desired_hooks;
                manifest.executable.clone_from(&self.tabbeacon_executable);
                changed = true;
            }
            if changed {
                atomic_write(&self.hooks_path(), &serialize_hooks(&hooks)?)?;
                if config_changed {
                    self.write_owned_config(&manifest, &config)?;
                }
                self.write_manifest(&manifest)?;
                return Ok(SetupOutcome::Upgraded);
            }
            return Ok(SetupOutcome::AlreadyInstalled);
        }

        let original_hooks = read_optional_bytes(&self.hooks_path())?;
        let original_config = read_optional_bytes(&self.config_path())?;
        let mut hooks = parse_hooks_bytes_for_setup(original_hooks.as_deref())?;
        let mut config = parse_config_bytes(original_config.as_deref())?;
        if contains_tabbeacon_like_hook(&hooks) {
            return Err(CodexIntegrationError::TabBeaconLikeAmbiguityBlocked);
        }
        append_owned_hooks(&mut hooks, &desired_hooks)?;
        let prior_title = terminal_title_item(&config)?.map(ToString::to_string);
        let title_owned = tabbeacon_owns_title && !terminal_title_is_disabled(&config)?;
        if title_owned {
            disable_terminal_title(&mut config)?;
        }
        let mcp_server = desired_mcp_server;
        if let Some(server) = &mcp_server {
            install_owned_mcp_server(&mut config, server)?;
        }

        let hooks_backup = self.backup("hooks", original_hooks.as_deref())?;
        let config_backup = self.backup("config", original_config.as_deref())?;
        let mut manifest = IntegrationManifest {
            schema: MANIFEST_SCHEMA.to_owned(),
            phase: ManifestPhase::Installing,
            codex_home: self.codex_home.clone(),
            hooks_path: self.hooks_path(),
            config_path: self.config_path(),
            executable: self.tabbeacon_executable.clone(),
            created_hooks_file: original_hooks.is_none(),
            hooks_backup,
            config_backup,
            title_owned,
            prior_title,
            mcp_server,
            hooks: desired_hooks,
        };
        self.write_manifest(&manifest)?;
        atomic_write(&self.hooks_path(), &serialize_hooks(&hooks)?)?;
        if title_owned || manifest.mcp_server.is_some() {
            self.write_owned_config(&manifest, &config)?;
        }
        manifest.phase = ManifestPhase::Active;
        self.write_manifest(&manifest)?;
        Ok(SetupOutcome::InstalledTrustReviewRequired)
    }

    fn repair_locked(
        &self,
        profile: CodexHookProfile,
        apply: bool,
        expected_target_digest: Option<&str>,
    ) -> Result<CodexRepairReport, CodexIntegrationError> {
        self.reject_repair_target_paths()?;
        let manifest = self
            .load_manifest()?
            .ok_or(CodexIntegrationError::OwnershipManifest)?;
        self.validate_manifest_scope(&manifest)?;
        let desired = desired_hooks(&self.tabbeacon_executable, profile)?;
        let desired_mcp_server = desired_mcp_server(&self.tabbeacon_executable, profile)?;
        if manifest.hooks != desired
            || manifest.mcp_server != desired_mcp_server
            || !Self::manifest_has_known_owned_declarations(&manifest)
        {
            return Err(CodexIntegrationError::StaleOwnedHook);
        }

        let expected_target_digest = if apply {
            let digest =
                expected_target_digest.ok_or(CodexIntegrationError::RepairPreviewDigestRequired)?;
            if !is_sha256_digest(digest) {
                return Err(CodexIntegrationError::RepairPreviewDigestInvalid);
            }
            Some(digest)
        } else {
            None
        };
        let original_hooks = read_required_safe_bytes(&self.hooks_path()).map_err(|error| {
            if expected_target_digest.is_some() {
                CodexIntegrationError::ConcurrentTargetDrift
            } else {
                error
            }
        })?;
        let mut hooks = parse_existing_hooks_bytes(&original_hooks)?;
        validate_known_hook_wire_shape(&hooks)?;
        let config = read_config_document(&self.config_path())?;
        Self::validate_title_ownership(&manifest, &config)?;
        Self::validate_mcp_server_ownership(&manifest, &config)?;
        let repairable = self.missing_repairable_owned_hooks(&hooks, &manifest)?;
        let target_digest = sha256_digest(&original_hooks);
        if let Some(expected_target_digest) = expected_target_digest
            && expected_target_digest != target_digest
        {
            return Err(CodexIntegrationError::ConcurrentTargetDrift);
        }
        if repairable.missing.is_empty() {
            return Ok(CodexRepairReport {
                schema_version: 2,
                disposition: CodexRepairDisposition::AlreadyExact,
                missing_declarations: 0,
                target_digest,
                third_party_groups_preserved: repairable.third_party_groups,
                postinstall_third_party_groups_preserved: repairable.postinstall_third_party_groups,
                manual_hook_trust_review_required: true,
            });
        }
        if !apply {
            return Ok(CodexRepairReport {
                schema_version: 2,
                disposition: CodexRepairDisposition::ReadyToApply,
                missing_declarations: repairable.missing.len(),
                target_digest,
                third_party_groups_preserved: repairable.third_party_groups,
                postinstall_third_party_groups_preserved: repairable.postinstall_third_party_groups,
                manual_hook_trust_review_required: true,
            });
        }

        append_owned_hooks(&mut hooks, &repairable.missing)?;
        let repaired_hooks = append_owned_hooks_preserving_external_bytes(
            &original_hooks,
            &hooks,
            &repairable.missing,
        )?;
        write_if_unchanged(&self.hooks_path(), &original_hooks, &repaired_hooks)?;
        Ok(CodexRepairReport {
            schema_version: 2,
            disposition: CodexRepairDisposition::RepairedTrustReviewRequired,
            missing_declarations: repairable.missing.len(),
            target_digest,
            third_party_groups_preserved: repairable.third_party_groups,
            postinstall_third_party_groups_preserved: repairable.postinstall_third_party_groups,
            manual_hook_trust_review_required: true,
        })
    }

    fn require_supported_profile(&self) -> Result<CodexHookProfile, CodexIntegrationError> {
        self.probe_codex_version()
            .and_then(|(_, state)| state.supported_profile())
            .ok_or(CodexIntegrationError::UnsupportedCodexVersion)
    }

    fn reconcile_title_ownership_locked(
        &self,
        tabbeacon_owns_title: bool,
    ) -> Result<TitleOwnershipOutcome, CodexIntegrationError> {
        let Some(mut manifest) = self.load_manifest()? else {
            return Ok(TitleOwnershipOutcome::NotInstalled);
        };
        self.validate_manifest_scope(&manifest)?;
        let hooks = read_hooks_document(&self.hooks_path())?;
        locate_owned_hooks(&hooks, &manifest.hooks)
            .map_err(|_| CodexIntegrationError::ModifiedOwnedHook)?;
        let mut config = read_config_document(&self.config_path())?;
        Self::validate_title_ownership(&manifest, &config)?;
        Self::validate_mcp_server_ownership(&manifest, &config)?;
        if !Self::apply_title_ownership(&mut manifest, &mut config, tabbeacon_owns_title)? {
            return Ok(TitleOwnershipOutcome::AlreadyConfigured);
        }
        self.write_owned_config(&manifest, &config)?;
        self.write_manifest(&manifest)?;
        Ok(TitleOwnershipOutcome::Updated)
    }

    fn validate_title_ownership(
        manifest: &IntegrationManifest,
        config: &DocumentMut,
    ) -> Result<(), CodexIntegrationError> {
        let disabled = terminal_title_is_disabled(config)?;
        if manifest.title_owned && !disabled {
            return Err(CodexIntegrationError::ModifiedOwnedTitle);
        }
        if !manifest.title_owned && disabled {
            return Err(CodexIntegrationError::TerminalTitleConflict);
        }
        Ok(())
    }

    fn apply_title_ownership(
        manifest: &mut IntegrationManifest,
        config: &mut DocumentMut,
        tabbeacon_owns_title: bool,
    ) -> Result<bool, CodexIntegrationError> {
        if manifest.title_owned == tabbeacon_owns_title {
            return Ok(false);
        }
        if tabbeacon_owns_title {
            disable_terminal_title(config)?;
            manifest.title_owned = true;
            return Ok(true);
        }
        restore_terminal_title(config, manifest.prior_title.as_deref())?;
        manifest.title_owned = false;
        Ok(true)
    }

    fn uninstall_locked(&self) -> Result<UninstallOutcome, CodexIntegrationError> {
        let Some(manifest) = self.load_manifest()? else {
            return Ok(UninstallOutcome::NotInstalled);
        };
        self.validate_manifest_scope(&manifest)?;
        let mut hooks = read_hooks_document(&self.hooks_path())?;
        let mut config = read_config_document(&self.config_path())?;
        locate_owned_hooks(&hooks, &manifest.hooks)
            .map_err(|_| CodexIntegrationError::ModifiedOwnedHook)?;
        Self::validate_title_ownership(&manifest, &config)?;
        Self::validate_mcp_server_ownership(&manifest, &config)?;

        remove_owned_hooks(&mut hooks, &manifest.hooks)?;
        if manifest.created_hooks_file && hooks_is_only_owned_scaffold(&hooks) {
            fs::remove_file(self.hooks_path())?;
        } else {
            atomic_write(&self.hooks_path(), &serialize_hooks(&hooks)?)?;
        }
        let config_changed = manifest.title_owned || manifest.mcp_server.is_some();
        if let Some(server) = &manifest.mcp_server {
            remove_owned_mcp_server(&mut config, server)?;
        }
        if manifest.title_owned {
            restore_terminal_title(&mut config, manifest.prior_title.as_deref())?;
        }
        if config_changed {
            self.write_owned_config(&manifest, &config)?;
        }
        fs::remove_file(self.manifest_path())?;
        Ok(UninstallOutcome::Removed)
    }

    fn with_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, CodexIntegrationError>,
    ) -> Result<T, CodexIntegrationError> {
        // The lock itself is a write target. Prove its full ancestry before
        // creating or opening it; otherwise a redirected state root could
        // escape the owned integration boundary before repair preflight runs.
        reject_symbolic_link(&self.state_root)?;
        fs::create_dir_all(&self.state_root)?;
        reject_symbolic_link(&self.state_root)?;
        let lock_path = self.state_root.join(LOCK_FILE);
        reject_symbolic_link(&lock_path)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)?;
        reject_symbolic_link(&lock_path)?;
        lock.lock()?;
        let result = operation();
        File::unlock(&lock)?;
        result
    }

    fn hooks_path(&self) -> PathBuf {
        self.codex_home.join("hooks.json")
    }

    fn config_path(&self) -> PathBuf {
        self.codex_home.join("config.toml")
    }

    fn manifest_path(&self) -> PathBuf {
        self.state_root.join(MANIFEST_FILE)
    }

    fn load_manifest(&self) -> Result<Option<IntegrationManifest>, CodexIntegrationError> {
        reject_symbolic_link(&self.manifest_path())?;
        let Some(bytes) = read_optional_bytes(&self.manifest_path())? else {
            return Ok(None);
        };
        let manifest: IntegrationManifest =
            serde_json::from_slice(&bytes).map_err(|_| CodexIntegrationError::OwnershipManifest)?;
        if manifest.schema != MANIFEST_SCHEMA || manifest.phase != ManifestPhase::Active {
            return Err(CodexIntegrationError::OwnershipManifest);
        }
        Ok(Some(manifest))
    }

    fn validate_manifest_scope(
        &self,
        manifest: &IntegrationManifest,
    ) -> Result<(), CodexIntegrationError> {
        if manifest.codex_home != self.codex_home
            || manifest.hooks_path != self.hooks_path()
            || manifest.config_path != self.config_path()
        {
            return Err(CodexIntegrationError::OwnershipManifest);
        }
        // A manifest records the executable that installed the exact owned
        // declarations. It must be shell-safe, but it is intentionally not
        // required to equal this process: setup is the ownership-proven path
        // that migrates hooks during a same-user binary relocation.
        if let Some(mcp_server) = &manifest.mcp_server {
            if mcp_server.name != MCP_HOOK_SERVER_NAME
                || mcp_server.args != ["__mcp-hook-stdio-v1"]
                || !mcp_env_vars_are_current_or_prerelease(&mcp_server.env_vars)
                || !mcp_server.command.is_absolute()
                || mcp_server
                    .command
                    .to_str()
                    .is_none_or(|path| path.trim().is_empty() || path.contains('\0'))
            {
                return Err(CodexIntegrationError::OwnershipManifest);
            }
        } else {
            owned_command_hooks(&manifest.executable, 1, false)
                .map_err(|_| CodexIntegrationError::OwnershipManifest)?;
        }
        self.validate_backup_record("hooks", &manifest.hooks_backup)?;
        self.validate_backup_record("config", &manifest.config_backup)?;
        Ok(())
    }

    /// The manifest is not merely structurally valid: runtime continuity and
    /// repair both need proof that its owned declarations are the known command
    /// Hook contract generated by the installing executable.
    fn manifest_has_known_owned_declarations(manifest: &IntegrationManifest) -> bool {
        match &manifest.mcp_server {
            Some(mcp_server) => {
                owned_mcp_hook_sets_for_manifest(&manifest.executable).is_ok_and(|expected| {
                    expected.into_iter().any(|hooks| hooks == manifest.hooks)
                        && desired_mcp_server_for_manifest(&manifest.executable).is_ok_and(
                            |expected| {
                                expected.as_ref().is_some_and(|expected| {
                                    mcp_server.name == expected.name
                                        && mcp_server.command == expected.command
                                        && mcp_server.args == expected.args
                                        // Version 0.5.2's pre-release MCP
                                        // declaration did not persist the
                                        // WT_SESSION forwarding allow-list.
                                        // Keep that exact predecessor
                                        // repairable, then reconcile it to
                                        // the current minimum declaration.
                                        && (mcp_server.env_vars.is_empty()
                                            || mcp_server.env_vars == expected.env_vars)
                                        // Version 0.5.2's pre-release MCP declaration did
                                        // not yet persist this visibility field. Keep that
                                        // precise predecessor repairable, then reconcile it
                                        // to the non-model-callable declaration.
                                        && (mcp_server.omit_tools_from.is_empty()
                                            || mcp_server.omit_tools_from
                                                == expected.omit_tools_from)
                                })
                            },
                        )
                })
            }
            None => owned_command_hooks(&manifest.executable, 1, false)
                .is_ok_and(|expected| expected == manifest.hooks),
        }
    }

    fn validate_mcp_server_ownership(
        manifest: &IntegrationManifest,
        config: &DocumentMut,
    ) -> Result<(), CodexIntegrationError> {
        if let Some(server) = &manifest.mcp_server
            && !owned_mcp_server_is_exact(config, server)?
        {
            return Err(CodexIntegrationError::ModifiedOwnedHook);
        }
        Ok(())
    }

    fn reconcile_mcp_server_ownership(
        manifest: &mut IntegrationManifest,
        config: &mut DocumentMut,
        desired: Option<OwnedMcpServer>,
    ) -> Result<bool, CodexIntegrationError> {
        if manifest.mcp_server == desired {
            return Ok(false);
        }
        if let Some(previous) = &manifest.mcp_server {
            remove_owned_mcp_server(config, previous)?;
        }
        if let Some(next) = &desired {
            install_owned_mcp_server(config, next)?;
        }
        manifest.mcp_server = desired;
        Ok(true)
    }

    fn write_owned_config(
        &self,
        manifest: &IntegrationManifest,
        config: &DocumentMut,
    ) -> Result<(), CodexIntegrationError> {
        let rendered = config.to_string();
        if !manifest.config_backup.existed && rendered.trim().is_empty() {
            if self.config_path().exists() {
                fs::remove_file(self.config_path())?;
            }
        } else {
            atomic_write(&self.config_path(), rendered.as_bytes())?;
        }
        Ok(())
    }

    /// Refuse repair when a target or any existing parent redirects elsewhere.
    /// A leaf-only symlink check is insufficient on Windows because a junction
    /// in the `.codex` or state-root ancestry can redirect the eventual write.
    fn reject_repair_target_paths(&self) -> Result<(), CodexIntegrationError> {
        let hooks_path = self.hooks_path();
        let config_path = self.config_path();
        let manifest_path = self.manifest_path();
        for path in [
            self.codex_home.as_path(),
            self.state_root.as_path(),
            hooks_path.as_path(),
            config_path.as_path(),
            manifest_path.as_path(),
        ] {
            reject_symbolic_link(path)?;
        }
        Ok(())
    }

    fn validate_backup_record(
        &self,
        kind: &str,
        backup: &BackupRecord,
    ) -> Result<(), CodexIntegrationError> {
        match (
            backup.existed,
            backup.digest.as_deref(),
            backup.path.as_deref(),
        ) {
            (false, None, None) => Ok(()),
            (true, Some(digest), Some(path))
                if is_sha256_hex(digest)
                    && path == self.state_root.join(format!("before-{kind}-{digest}")) =>
            {
                reject_symbolic_link(path)
            }
            _ => Err(CodexIntegrationError::OwnershipManifest),
        }
    }

    /// Returns the original pre-install Hook groups only after the backup path,
    /// digest, and JSON shape have all been re-proven. The backup distinguishes
    /// pre-install groups from later third-party groups for diagnostics; both
    /// are preserved when the current known envelope proves they are non-TabBeacon.
    fn original_hook_groups(
        &self,
        manifest: &IntegrationManifest,
    ) -> Result<BTreeMap<String, Vec<Value>>, CodexIntegrationError> {
        self.validate_backup_record("hooks", &manifest.hooks_backup)?;
        if !manifest.hooks_backup.existed {
            return Ok(BTreeMap::new());
        }
        let backup_path = manifest
            .hooks_backup
            .path
            .as_deref()
            .ok_or(CodexIntegrationError::OwnershipManifest)?;
        let backup_bytes = read_required_safe_bytes(backup_path)
            .map_err(|_| CodexIntegrationError::BaselineDriftBlocked)?;
        if manifest.hooks_backup.digest.as_deref() != Some(&hex_sha256(&backup_bytes)) {
            return Err(CodexIntegrationError::BaselineDriftBlocked);
        }
        let backup_hooks = parse_existing_hooks_bytes(&backup_bytes)
            .map_err(|_| CodexIntegrationError::BaselineDriftBlocked)?;
        let events = hooks_events(&backup_hooks)?;
        let mut original = BTreeMap::new();
        for (event, groups) in events {
            let groups = groups.as_array().ok_or(CodexIntegrationError::HooksShape)?;
            original.insert(event.clone(), groups.clone());
        }
        Ok(original)
    }

    /// Finds only declarations absent from a current, target-bound manifest.
    /// Each retained non-owned group must have the known Hook envelope and be
    /// provably non-TabBeacon. Verified baseline and later third-party groups
    /// are both preserved verbatim at the semantic group level.
    fn missing_repairable_owned_hooks(
        &self,
        hooks: &Value,
        manifest: &IntegrationManifest,
    ) -> Result<RepairableOwnedHooks, CodexIntegrationError> {
        let events = hooks_events(hooks)?;
        let original = self.original_hook_groups(manifest)?;
        let mut missing = Vec::new();
        let mut third_party_groups = 0;
        let mut postinstall_third_party_groups = 0;
        for declaration in &manifest.hooks {
            let matches = events
                .get(&declaration.event)
                .and_then(Value::as_array)
                .map_or(0, |groups| {
                    groups
                        .iter()
                        .filter(|group| *group == &declaration.group)
                        .count()
                });
            match matches {
                0 => missing.push(declaration.clone()),
                1 => {}
                _ => return Err(CodexIntegrationError::ModifiedOwnedHook),
            }
        }

        for (event, groups) in events {
            let groups = groups.as_array().ok_or(CodexIntegrationError::HooksShape)?;
            let baseline = original.get(event).map(Vec::as_slice).unwrap_or_default();
            for group in groups {
                let is_exact_manifest_group = manifest
                    .hooks
                    .iter()
                    .any(|declaration| declaration.event == *event && declaration.group == *group);
                if is_exact_manifest_group {
                    continue;
                }
                if group_is_partial_manifest_owned(group, event, &manifest.hooks) {
                    return Err(CodexIntegrationError::ModifiedOwnedHook);
                }
                if group_looks_like_tabbeacon_hook(group, Some(&manifest.executable)) {
                    return Err(CodexIntegrationError::TabBeaconLikeAmbiguityBlocked);
                }
                if !baseline
                    .iter()
                    .any(|original_group| original_group == group)
                {
                    if !has_external_hook_provenance(group) {
                        return Err(CodexIntegrationError::BaselineDriftBlocked);
                    }
                    postinstall_third_party_groups += 1;
                }
                third_party_groups += 1;
            }
        }
        Ok(RepairableOwnedHooks {
            missing,
            third_party_groups,
            postinstall_third_party_groups,
        })
    }

    fn write_manifest(&self, manifest: &IntegrationManifest) -> Result<(), CodexIntegrationError> {
        let mut bytes = serde_json::to_vec_pretty(manifest)
            .map_err(|_| CodexIntegrationError::OwnershipManifest)?;
        bytes.push(b'\n');
        atomic_write(&self.manifest_path(), &bytes)?;
        Ok(())
    }

    fn backup(
        &self,
        kind: &str,
        contents: Option<&[u8]>,
    ) -> Result<BackupRecord, CodexIntegrationError> {
        let Some(contents) = contents else {
            return Ok(BackupRecord {
                existed: false,
                digest: None,
                path: None,
            });
        };
        let digest = hex_sha256(contents);
        let path = self.state_root.join(format!("before-{kind}-{digest}"));
        if path.exists() {
            if fs::read(&path)? != contents {
                return Err(CodexIntegrationError::OwnershipManifest);
            }
        } else {
            atomic_write(&path, contents)?;
        }
        Ok(BackupRecord {
            existed: true,
            digest: Some(digest),
            path: Some(path),
        })
    }

    fn probe_codex_version(&self) -> Option<ProbedCodexProfile> {
        let output = if let Some(program) = &self.codex_program {
            Command::new(program).arg("--version").output().ok()?
        } else {
            default_codex_version_command().output().ok()?
        };
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8(output.stdout).ok()?;
        let version = stdout.split_whitespace().find_map(parse_semver)?;
        let profile = CodexCompatibilityRegistry::classify(Some(version));
        Some((
            format!("{}.{}.{}", version.0, version.1, version.2),
            profile,
        ))
    }
}

#[cfg(windows)]
fn default_codex_version_command() -> Command {
    let shell = env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
    let mut command = Command::new(shell);
    command.args(["/D", "/S", "/C", "codex --version"]);
    command
}

#[cfg(not(windows))]
fn default_codex_version_command() -> Command {
    let mut command = Command::new("codex");
    command.arg("--version");
    command
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ManifestPhase {
    Installing,
    Active,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BackupRecord {
    existed: bool,
    digest: Option<String>,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OwnedHook {
    event: String,
    group: Value,
}

/// Exact, process-scoped MCP server declaration that `TabBeacon` may own.
///
/// The server is never a global daemon: Codex starts it from this stdio
/// declaration for one Codex runtime and EOF closes that ownership boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct OwnedMcpServer {
    name: String,
    command: PathBuf,
    args: Vec<String>,
    /// Explicit parent-environment allow-list for the session-scoped child.
    /// Older v0.5.2 pre-release manifests omitted it, so deserialize that
    /// precise predecessor as an upgradeable empty declaration.
    #[serde(default)]
    env_vars: Vec<String>,
    #[serde(default)]
    omit_tools_from: Vec<String>,
}

/// The sole compatibility exception is the pre-release declaration which
/// lacked the field altogether. Any non-empty declaration other than the
/// current one is ambiguous ownership rather than a migration candidate.
fn mcp_env_vars_are_current_or_prerelease(env_vars: &[String]) -> bool {
    match env_vars {
        [] => true,
        [value] => value == "WT_SESSION",
        _ => false,
    }
}

/// The exact owned declarations eligible for restoration plus a non-sensitive
/// accounting of third-party groups retained by the repair.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RepairableOwnedHooks {
    missing: Vec<OwnedHook>,
    third_party_groups: usize,
    postinstall_third_party_groups: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct IntegrationManifest {
    schema: String,
    phase: ManifestPhase,
    codex_home: PathBuf,
    hooks_path: PathBuf,
    config_path: PathBuf,
    executable: PathBuf,
    created_hooks_file: bool,
    hooks_backup: BackupRecord,
    config_backup: BackupRecord,
    title_owned: bool,
    prior_title: Option<String>,
    #[serde(default)]
    mcp_server: Option<OwnedMcpServer>,
    hooks: Vec<OwnedHook>,
}

fn desired_hooks(
    executable: &Path,
    profile: CodexHookProfile,
) -> Result<Vec<OwnedHook>, CodexIntegrationError> {
    if profile.uses_mcp_hook_transport() {
        let mut hooks = owned_mcp_tool_hooks(executable, profile)?;
        hooks.extend(owned_command_hooks_for_events(
            executable,
            &["SessionEnd"],
            profile.timeout().declaration_timeout_seconds(),
            false,
        )?);
        Ok(hooks)
    } else {
        owned_command_hooks(
            executable,
            profile.timeout().declaration_timeout_seconds(),
            !profile.timeout().synchronous_required(),
        )
    }
}

fn desired_mcp_server(
    executable: &Path,
    profile: CodexHookProfile,
) -> Result<Option<OwnedMcpServer>, CodexIntegrationError> {
    if !profile.uses_mcp_hook_transport() {
        return Ok(None);
    }
    if !executable.is_absolute()
        || executable
            .to_str()
            .is_none_or(|path| path.trim().is_empty() || path.contains('\0'))
    {
        return Err(CodexIntegrationError::UnsafeExecutablePath);
    }
    Ok(Some(OwnedMcpServer {
        name: MCP_HOOK_SERVER_NAME.to_owned(),
        command: executable.to_path_buf(),
        args: vec!["__mcp-hook-stdio-v1".to_owned()],
        // Codex 0.149 clears its MCP child environment. Activity workers bind
        // to WT_SESSION, so forward that exact terminal identity and nothing
        // else from the parent environment.
        env_vars: vec!["WT_SESSION".to_owned()],
        // Codex 0.149 supports omitting one server's tools from each
        // model-facing exposure surface. Hook delivery calls this tool through
        // the internal MCP runtime; it must never become model-callable.
        omit_tools_from: vec![
            "code_mode".to_owned(),
            "deferred".to_owned(),
            "direct".to_owned(),
        ],
    }))
}

fn mcp_transport_profile() -> Result<CodexHookProfile, CodexIntegrationError> {
    CodexHookProfile::for_version((0, 149, 0)).ok_or(CodexIntegrationError::OwnershipManifest)
}

fn desired_mcp_server_for_manifest(
    executable: &Path,
) -> Result<Option<OwnedMcpServer>, CodexIntegrationError> {
    desired_mcp_server(executable, mcp_transport_profile()?)
}

fn owned_mcp_tool_hooks(
    executable: &Path,
    profile: CodexHookProfile,
) -> Result<Vec<OwnedHook>, CodexIntegrationError> {
    let server = desired_mcp_server(executable, profile)?
        .ok_or(CodexIntegrationError::UnsupportedCodexVersion)?;
    Ok(profile
        .lifecycle_events()
        .iter()
        .filter_map(|event| hook_input_template(*event).map(|input| (*event, input)))
        .map(|(event, input)| OwnedHook {
            event: event.as_str().to_owned(),
            group: json!({
                "hooks": [{
                    "type": "mcp_tool",
                    "server": server.name,
                    "tool": MCP_HOOK_TOOL_NAME,
                    "input": input,
                    "timeout": profile.timeout().declaration_timeout_seconds()
                }]
            }),
        })
        .collect())
}

fn owned_mcp_hook_sets_for_manifest(
    executable: &Path,
) -> Result<Vec<Vec<OwnedHook>>, CodexIntegrationError> {
    let profile = mcp_transport_profile()?;
    let legacy = owned_mcp_tool_hooks(executable, profile)?;
    let hybrid = desired_hooks(executable, profile)?;
    Ok(vec![legacy, hybrid])
}

/// Recognizes the one admitted migration from the first 0.149 transport to the
/// hybrid transport. The previous ten MCP declarations are complete and exact;
/// only the command `SessionEnd` boundary is newly introduced and therefore
/// must become the sole new Codex trust-review item.
fn is_legacy_mcp_session_end_upgrade(
    manifest: &IntegrationManifest,
    desired: &[OwnedHook],
) -> bool {
    if manifest.mcp_server.is_none() {
        return false;
    }
    let Ok(sets) = owned_mcp_hook_sets_for_manifest(&manifest.executable) else {
        return false;
    };
    let Some((legacy, hybrid)) = sets
        .split_first()
        .and_then(|(legacy, rest)| rest.first().map(|hybrid| (legacy, hybrid)))
    else {
        return false;
    };
    manifest.hooks == *legacy && desired == hybrid
}

fn owned_command_hooks(
    executable: &Path,
    timeout_seconds: u8,
    asynchronous: bool,
) -> Result<Vec<OwnedHook>, CodexIntegrationError> {
    owned_command_hooks_for_events(executable, &HOOK_EVENTS, timeout_seconds, asynchronous)
}

fn owned_command_hooks_for_events(
    executable: &Path,
    events: &[&str],
    timeout_seconds: u8,
    asynchronous: bool,
) -> Result<Vec<OwnedHook>, CodexIntegrationError> {
    if !executable.is_absolute() {
        return Err(CodexIntegrationError::UnsafeExecutablePath);
    }
    let executable = executable
        .to_str()
        .filter(|path| {
            !path.is_empty()
                && !path
                    .chars()
                    .any(|character| matches!(character, '"' | '%' | '\r' | '\n'))
        })
        .ok_or(CodexIntegrationError::UnsafeExecutablePath)?;
    // Keep the generic command on the proven cross-shell PowerShell envelope.
    // The Windows-specific field is selected by Codex's current Windows runner
    // and may use the faster direct shape for a shell-neutral executable path.
    let command = powershell_encoded_windows_hook_command(executable);
    let windows_command = windows_hook_command_for_default_comspec(executable);
    Ok(events
        .iter()
        .copied()
        .map(|event| OwnedHook {
            event: event.to_owned(),
            group: json!({
                "hooks": [{
                    "type": "command",
                    "command": command.clone(),
                    "commandWindows": windows_command,
                    "timeout": timeout_seconds,
                    "async": asynchronous
                }]
            }),
        })
        .collect())
}

fn windows_hook_command_for_default_comspec(executable: &str) -> String {
    // Codex 0.149 passes `commandWindows` to a non-empty TurnEnvironment shell
    // when one is configured; COMSPEC is only the empty-shell fallback. A
    // quoted executable plus `|| exit /b 0` is therefore cmd syntax, not a
    // Windows declaration. For a shell-safe, whitespace-free native `.exe`
    // path use one direct native invocation. A Codex raw-quoted cmd /c command
    // accepts only its first line, so a shell-neutral trailing exit helper is
    // not representable. The Hook ingress itself is silent and fail-open for
    // malformed input and runtime errors; non-fast paths retain the encoded
    // PowerShell envelope for their compatibility exit handling.
    //
    // Paths outside that narrow grammar retain the encoded PowerShell envelope
    // in both fields. It quotes hostile paths safely and ends in `exit 0`.
    if requires_powershell_command_envelope(executable) {
        powershell_encoded_windows_hook_command(executable)
    } else {
        format!("{executable} hook codex")
    }
}

fn requires_powershell_command_envelope(executable: &str) -> bool {
    !executable.to_ascii_lowercase().ends_with(".exe")
        || !executable.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, ':' | '\\' | '/' | '.' | '_' | '-')
        })
}

fn powershell_encoded_windows_hook_command(executable: &str) -> String {
    let executable = executable.replace('\'', "''");
    let script = format!(
        "$ErrorActionPreference = 'SilentlyContinue'; & '{executable}' hook codex 1>$null 2>$null; exit 0"
    );
    let mut utf16 = Vec::with_capacity(script.len() * 2);
    for unit in script.encode_utf16() {
        utf16.extend_from_slice(&unit.to_le_bytes());
    }
    format!(
        "powershell.exe -NoProfile -NonInteractive -EncodedCommand {}",
        base64_encode(&utf16)
    )
}

#[derive(Clone, Copy)]
enum RuntimeProbeOutcome {
    Pass,
    McpHybrid {
        mcp_event: ProbeClaim,
        termination: ProbeClaim,
        session_end: ProbeClaim,
        frame_interval_ms: Option<u64>,
    },
    TimedOut,
    NonZero,
    MissingMarker,
    Unavailable,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProbeClaim {
    Proven,
    Failed,
    TimedOut,
}

impl ProbeClaim {
    const fn label(self) -> &'static str {
        match self {
            Self::Proven => "PROVEN",
            Self::Failed => "UNPROVEN",
            Self::TimedOut => "TIMEOUT",
        }
    }
}

/// Converts the bounded hybrid probe's independent claims into four doctor
/// observations. The aggregate remains strict, while a proof for one transport
/// is never erased merely because the other transport failed.
fn hybrid_runtime_probe_checks(
    mcp_event: ProbeClaim,
    termination: ProbeClaim,
    session_end: ProbeClaim,
    frame_interval_ms: Option<u64>,
) -> Vec<DoctorCheck> {
    let mcp_check = match mcp_event {
        ProbeClaim::Proven => pass(
            "hooks.mcp-event-transport",
            format!(
                "MCP_EVENT_TRANSPORT_PROVEN: direct stdio delivered WT_SESSION-bound Working/Stop/result-ready transitions and terminal activity with two spinner frames at {} ms without a command shell",
                frame_interval_ms.unwrap_or_default()
            ),
        ),
        ProbeClaim::Failed => fail(
            "hooks.mcp-event-transport",
            "MCP_EVENT_TRANSPORT_UNPROVEN: the normal MCP conversation did not prove all required lifecycle/activity observations",
        ),
        ProbeClaim::TimedOut => fail(
            "hooks.mcp-event-transport",
            "MCP_EVENT_TRANSPORT_TIMEOUT: the normal MCP conversation exceeded its bounded diagnostic window",
        ),
    };
    let termination_check = match termination {
        ProbeClaim::Proven => pass(
            "hooks.codex-terminate-before-eof",
            "CODEX_0149_TERMINATE_BEFORE_EOF_REPRODUCED: Codex's owned MCP process exited while its stdio input remained open",
        ),
        ProbeClaim::Failed => fail(
            "hooks.codex-terminate-before-eof",
            "CODEX_0149_TERMINATE_BEFORE_EOF_UNPROVEN: the owned MCP process did not reproduce the required terminate-before-EOF ordering",
        ),
        ProbeClaim::TimedOut => fail(
            "hooks.codex-terminate-before-eof",
            "CODEX_0149_TERMINATE_BEFORE_EOF_TIMEOUT: the owned MCP process did not exit within its bounded termination phase",
        ),
    };
    let session_end_check = match session_end {
        ProbeClaim::Proven => pass(
            "hooks.session-end-cleanup",
            "REAL_SESSION_END_CLEANUP_PROVEN: the independent synchronous SessionEnd command ran after terminate-before-EOF and reset terminal cleanup state",
        ),
        ProbeClaim::Failed => fail(
            "hooks.session-end-cleanup",
            "REAL_SESSION_END_CLEANUP_UNPROVEN: the independent SessionEnd command did not prove all cleanup facts; EOF remains fallback only",
        ),
        ProbeClaim::TimedOut => fail(
            "hooks.session-end-cleanup",
            "REAL_SESSION_END_CLEANUP_TIMEOUT: the independent SessionEnd command did not settle within the bounded cleanup diagnostic; timeout and p99 remain separately enforced by the exact transport measurement",
        ),
    };
    let aggregate = if mcp_event == ProbeClaim::Proven
        && termination == ProbeClaim::Proven
        && session_end == ProbeClaim::Proven
    {
        pass(
            "hooks.runtime-probe",
            "RUNTIME_PROBE_PASS: MCP_EVENT_TRANSPORT_PROVEN, CODEX_0149_TERMINATE_BEFORE_EOF_REPRODUCED, and REAL_SESSION_END_CLEANUP_PROVEN",
        )
    } else {
        fail(
            "hooks.runtime-probe",
            format!(
                "RUNTIME_PROBE_PARTIAL_OR_FAILED: MCP_EVENT_TRANSPORT={}; CODEX_0149_TERMINATE_BEFORE_EOF={}; REAL_SESSION_END_CLEANUP={}",
                mcp_event.label(),
                termination.label(),
                session_end.label()
            ),
        )
    };
    vec![aggregate, mcp_check, termination_check, session_end_check]
}

#[cfg(windows)]
fn run_windows_hook_runtime_probe(command_line: &str) -> RuntimeProbeOutcome {
    let Ok(probe_root) = create_runtime_probe_root() else {
        return RuntimeProbeOutcome::Unavailable;
    };
    let marker = probe_root.join("hook-timing.txt");
    let payload = json!({
        "hook_event_name": RUNTIME_PROBE_EVENT,
        "session_id": "00000000-0000-0000-0000-000000000052",
        "cwd": probe_root,
    })
    .to_string();

    let comspec = env::var_os("COMSPEC")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "cmd.exe".into());
    let mut process = Command::new(comspec);
    process.arg("/C");
    process.raw_arg(format!(r#""{command_line}""#));
    process
        .current_dir(&probe_root)
        .env("LOCALAPPDATA", &probe_root)
        .env("TABBEACON_HOOK_TIMING_FILE", &marker)
        .env_remove("WT_SESSION")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let Ok(mut child) = process.spawn() else {
        let _ = fs::remove_dir_all(&probe_root);
        return RuntimeProbeOutcome::Unavailable;
    };
    let write_succeeded = child
        .stdin
        .take()
        .is_some_and(|mut stdin| stdin.write_all(payload.as_bytes()).is_ok());
    let deadline = Instant::now() + RUNTIME_PROBE_TIMEOUT;
    let exited_successfully = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status.success()),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
            Ok(None) => {
                if !terminate_runtime_probe_tree(child.id()) {
                    // Do not wait here: the probe's deadline must remain a real
                    // bound even if Windows cannot start the tree terminator.
                    // This only addresses the directly-owned command root; the
                    // normal path uses taskkill /T to include its descendants.
                    let _ = child.kill();
                }
                break None;
            }
            Err(_) => break Some(false),
        }
    };
    let marker_present = fs::read_to_string(&marker)
        .is_ok_and(|contents| contents.starts_with("TABBEACON_HOOK_TIMING_V1 "));
    let _ = fs::remove_dir_all(&probe_root);

    match (write_succeeded, exited_successfully, marker_present) {
        (_, None, _) => RuntimeProbeOutcome::TimedOut,
        (true, Some(true), true) => RuntimeProbeOutcome::Pass,
        (false, _, _) | (_, Some(_), false) => RuntimeProbeOutcome::MissingMarker,
        _ => RuntimeProbeOutcome::NonZero,
    }
}

#[cfg(windows)]
fn terminate_runtime_probe_tree(process_id: u32) -> bool {
    let taskkill = env::var_os("SystemRoot").map_or_else(
        || PathBuf::from("taskkill.exe"),
        |root| PathBuf::from(root).join("System32").join("taskkill.exe"),
    );
    let Ok(mut terminator) = Command::new(taskkill)
        .args(["/PID", &process_id.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };

    let deadline = Instant::now() + Duration::from_millis(100);
    loop {
        match terminator.try_wait() {
            Ok(Some(status)) => return status.success(),
            Err(_) => {
                let _ = terminator.kill();
                return false;
            }
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(5)),
            Ok(None) => {
                // A hung taskkill process must not extend the probe beyond its
                // published bound. Dropping the owned handles never waits.
                let _ = terminator.kill();
                return false;
            }
        }
    }
}

#[cfg(windows)]
fn create_runtime_probe_root() -> io::Result<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    for attempt in 0..8_u8 {
        let path = env::temp_dir().join(format!(
            "tabbeacon-hook-runtime-probe-{}-{nonce}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create a unique runtime-probe directory",
    ))
}

#[cfg(not(windows))]
fn run_windows_hook_runtime_probe(_command_line: &str) -> RuntimeProbeOutcome {
    RuntimeProbeOutcome::Unavailable
}

/// Executes one real MCP Hook transport conversation without invoking a
/// command shell. The temporary state root is isolated from Owner state and
/// the Codex configuration/trust files are only read by the caller's
/// preflight. It models Codex process termination before transport close and
/// proves cleanup through the independent `SessionEnd` command declaration.
#[cfg(windows)]
#[allow(clippy::too_many_lines)] // One bounded probe owns its child lifecycle, protocol exchange, and receipt check.
fn run_windows_mcp_hook_runtime_probe(
    executable: &Path,
    session_end_command: &str,
) -> RuntimeProbeOutcome {
    let Ok(probe_root) = create_runtime_probe_root() else {
        return RuntimeProbeOutcome::Unavailable;
    };
    let activity_receipt = probe_root.join(ACTIVITY_WORKER_PROBE_RECEIPT_FILE);
    let activity_process = probe_root.join(ACTIVITY_WORKER_PROBE_PROCESS_FILE);
    let activity_started = probe_root.join(ACTIVITY_WORKER_PROBE_STARTED_FILE);
    // SessionStart is allowed to perform local Git discovery before it creates
    // its fast anchor. Keep that setup confined to the probe root; ordinary
    // events after the anchor remain zero-Git.
    let repository_ready = Command::new("git")
        .args(["init", "--quiet"])
        .arg(&probe_root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !repository_ready {
        let _ = fs::remove_dir_all(&probe_root);
        return RuntimeProbeOutcome::Unavailable;
    }
    let terminal_binding = "00000000-0000-0000-0000-000000000052";
    let mut process = Command::new(executable);
    // Mirror Codex 0.149's LocalStdioServerLauncher: construct the child from
    // a clean environment, retain only fixed launcher prerequisites, then add
    // the one value in the owned MCP `env_vars` declaration. This test must
    // never pass through the complete parent environment.
    process.env_clear();
    process
        .arg("__mcp-hook-stdio-v1")
        .current_dir(&probe_root)
        .env("LOCALAPPDATA", &probe_root)
        .env(ACTIVITY_WORKER_PROBE_RECEIPT_ENV, &activity_receipt)
        .env("WT_SESSION", terminal_binding)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(path) = env::var_os("PATH") {
        process.env("PATH", path);
    }
    if let Some(system_root) = env::var_os("SystemRoot") {
        process.env("SystemRoot", system_root);
    }
    let Ok(mut child) = process.spawn() else {
        let _ = fs::remove_dir_all(&probe_root);
        return RuntimeProbeOutcome::Unavailable;
    };
    let Some(mut stdin) = child.stdin.take() else {
        let _ = terminate_runtime_probe_tree(child.id());
        let _ = fs::remove_dir_all(&probe_root);
        return RuntimeProbeOutcome::Unavailable;
    };
    let Some(stdout) = child.stdout.take() else {
        let _ = terminate_runtime_probe_tree(child.id());
        let _ = fs::remove_dir_all(&probe_root);
        return RuntimeProbeOutcome::Unavailable;
    };
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        while reader.read_line(&mut line).is_ok_and(|count| count > 0) {
            if sender.send(line.clone()).is_err() {
                break;
            }
            line.clear();
        }
    });

    let initialize = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "protocolVersion": "2025-06-18", "capabilities": {} }
    });
    let start = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": MCP_HOOK_TOOL_NAME,
            "arguments": {
                "hook_event_name": "SessionStart",
                "session_id": "00000000-0000-0000-0000-000000000052",
                "cwd": probe_root,
                "source": "startup",
            }
        }
    });
    let call = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": MCP_HOOK_TOOL_NAME,
            "arguments": {
                "hook_event_name": RUNTIME_PROBE_EVENT,
                "session_id": "00000000-0000-0000-0000-000000000052",
                "turn_id": "00000000-0000-0000-0000-000000000052",
                "cwd": probe_root,
            }
        }
    });
    let stop = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": MCP_HOOK_TOOL_NAME,
            "arguments": {
                "hook_event_name": "Stop",
                "session_id": "00000000-0000-0000-0000-000000000052",
                "turn_id": "00000000-0000-0000-0000-000000000052",
                "cwd": probe_root,
            }
        }
    });
    let renewed_call = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": MCP_HOOK_TOOL_NAME,
            "arguments": {
                "hook_event_name": RUNTIME_PROBE_EVENT,
                "session_id": "00000000-0000-0000-0000-000000000052",
                "turn_id": "00000000-0000-0000-0000-000000000053",
                "cwd": probe_root,
            }
        }
    });
    let writes_succeeded = [initialize, start, call].iter().all(|request| {
        serde_json::to_writer(&mut stdin, request)
            .and_then(|()| stdin.write_all(b"\n").map_err(serde_json::Error::io))
            .is_ok()
    }) && stdin.flush().is_ok();

    let deadline = Instant::now() + MCP_ACTIVITY_RUNTIME_PROBE_TIMEOUT;
    let mut completed_calls = BTreeSet::new();
    while Instant::now() < deadline && completed_calls.len() < 2 {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Ok(line) = receiver.recv_timeout(remaining) else {
            break;
        };
        if let Ok(response) = serde_json::from_str::<Value>(&line)
            && response.pointer("/result/isError") == Some(&Value::Bool(false))
            && let Some(id) = response.get("id").and_then(Value::as_i64)
            && matches!(id, 2 | 3)
        {
            completed_calls.insert(id);
        }
    }
    // Keep a deterministic closeout budget. A missing worker receipt must not
    // consume the entire probe window and masquerade as an MCP event failure.
    let animation_deadline = deadline
        .checked_sub(Duration::from_secs(1))
        .unwrap_or_else(Instant::now);
    let animation_receipt = loop {
        let receipt = fs::read(&activity_receipt)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        if receipt.is_some() || Instant::now() >= animation_deadline {
            break receipt;
        }
        thread::sleep(Duration::from_millis(5));
    };
    // Prove the normal MCP Stop/result-ready path independently, then open a
    // new active generation so the command SessionEnd must revoke a live
    // activity-worker lease rather than only clear an already-stopped state.
    let stop_and_renew_succeeded = [stop, renewed_call].iter().all(|request| {
        serde_json::to_writer(&mut stdin, request)
            .and_then(|()| stdin.write_all(b"\n").map_err(serde_json::Error::io))
            .is_ok()
    }) && stdin.flush().is_ok();
    while Instant::now() < deadline && !completed_calls.is_superset(&BTreeSet::from([4, 5])) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let Ok(line) = receiver.recv_timeout(remaining) else {
            break;
        };
        if let Ok(response) = serde_json::from_str::<Value>(&line)
            && response.pointer("/result/isError") == Some(&Value::Bool(false))
            && let Some(id) = response.get("id").and_then(Value::as_i64)
            && matches!(id, 4 | 5)
        {
            completed_calls.insert(id);
        }
    }
    // Model Codex 0.149's LocalStdioServerTransport::close ordering exactly:
    // the server process is terminated before its transport is closed. Keep
    // our stdin handle open during termination so this cannot be mistaken for
    // a successful EOF cleanup. EOF remains a best-effort fallback, not the
    // authoritative SessionEnd boundary.
    // `LocalStdioServerTransport::close()` terminates its owned MCP server
    // process; it does not terminate TabBeacon's independently spawned
    // activity worker. Killing the whole tree here would both model the wrong
    // Codex boundary and race a dying worker's file lock against the real
    // SessionEnd command. Retaining that worker until SessionEnd revokes its
    // lease is the property this probe is intended to prove.
    let mcp_terminated_before_eof = child.kill().is_ok();
    drop(stdin);
    let termination_deadline = Instant::now() + MCP_TERMINATION_RUNTIME_PROBE_TIMEOUT;
    let mcp_terminated = loop {
        match child.try_wait() {
            Ok(Some(_)) => break true,
            Ok(None) if Instant::now() < termination_deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            Ok(None) => {
                let _ = child.kill();
                break false;
            }
            Err(_) => break false,
        }
    };
    let animation_frame_interval_ms = animation_receipt
        .as_ref()
        .and_then(|receipt| receipt.get("frame_interval_ms"))
        .and_then(Value::as_u64);
    let animation_proves_runtime = animation_receipt.is_some_and(|receipt| {
        let frame_interval_ms = animation_frame_interval_ms.unwrap_or(u64::MAX);
        receipt.get("schema") == Some(&json!("tabbeacon-activity-worker-probe-v1"))
            && receipt.get("worker_started") == Some(&json!(true))
            && receipt.get("distinct_spinner_frames") == Some(&json!(2))
            && (TARGET_FRAME_INTERVAL_MS / 2..=TARGET_FRAME_INTERVAL_MS * 2 + 50)
                .contains(&frame_interval_ms)
    });
    let activity_worker_started = fs::read(&activity_started)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some_and(|receipt| {
            receipt.get("schema") == Some(&json!("tabbeacon-activity-worker-probe-v1"))
                && receipt.get("worker_entered") == Some(&json!(true))
        });
    let activity_worker_process_started = fs::read(&activity_process)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some_and(|receipt| {
            receipt.get("schema") == Some(&json!("tabbeacon-activity-worker-probe-v1"))
                && receipt.get("worker_process_entered") == Some(&json!(true))
        });
    let session_end_receipt = probe_root.join(SESSION_END_PROBE_RECEIPT_FILE);
    let session_end_payload = json!({
        "hook_event_name": "SessionEnd",
        "session_id": "00000000-0000-0000-0000-000000000052",
        "cwd": probe_root,
    })
    .to_string();
    let comspec = env::var_os("COMSPEC")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "cmd.exe".into());
    let mut session_end = Command::new(comspec);
    session_end.arg("/C");
    session_end.raw_arg(format!(r#""{session_end_command}""#));
    session_end
        .current_dir(&probe_root)
        .env("LOCALAPPDATA", &probe_root)
        .env("WT_SESSION", terminal_binding)
        .env(SESSION_END_PROBE_RECEIPT_ENV, &session_end_receipt)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut session_end = session_end.spawn().ok();
    let session_end_write_succeeded = session_end.as_mut().is_some_and(|child| {
        child.stdin.take().is_some_and(|mut stdin| {
            stdin.write_all(session_end_payload.as_bytes()).is_ok() && stdin.flush().is_ok()
        })
    });
    let session_end_exited = session_end.as_mut().and_then(|child| {
        let deadline = Instant::now() + SESSION_END_CLEANUP_RUNTIME_PROBE_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status.success()),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                Ok(None) => {
                    if !terminate_runtime_probe_tree(child.id()) {
                        let _ = child.kill();
                    }
                    break None;
                }
                Err(_) => break Some(false),
            }
        }
    });
    let session_end_cleanup_proven = fs::read(&session_end_receipt)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some_and(|receipt| {
            receipt.get("schema") == Some(&json!("tabbeacon-session-end-probe-v1"))
                && receipt.get("generation_retired") == Some(&json!(true))
                && receipt.get("root_anchor_retired") == Some(&json!(true))
                && receipt.get("activity_lease_revoked") == Some(&json!(true))
                && receipt.get("progress_reset") == Some(&json!(true))
                && receipt.get("frame_color_reset") == Some(&json!(true))
                && receipt.get("windows_terminal_indexed_reset") == Some(&json!(true))
        });
    let _ = fs::remove_dir_all(&probe_root);
    let mcp_event = if writes_succeeded
        && stop_and_renew_succeeded
        && completed_calls == BTreeSet::from([2, 3, 4, 5])
        && activity_worker_process_started
        && activity_worker_started
        && animation_proves_runtime
    {
        ProbeClaim::Proven
    } else {
        ProbeClaim::Failed
    };
    let termination = if mcp_terminated_before_eof && mcp_terminated {
        ProbeClaim::Proven
    } else if !mcp_terminated {
        ProbeClaim::TimedOut
    } else {
        ProbeClaim::Failed
    };
    let session_end = if session_end_exited.is_none() {
        ProbeClaim::TimedOut
    } else if session_end_write_succeeded
        && session_end_exited == Some(true)
        && session_end_cleanup_proven
    {
        ProbeClaim::Proven
    } else {
        ProbeClaim::Failed
    };
    RuntimeProbeOutcome::McpHybrid {
        mcp_event,
        termination,
        session_end,
        frame_interval_ms: animation_frame_interval_ms,
    }
}

#[cfg(not(windows))]
fn run_windows_mcp_hook_runtime_probe(
    _executable: &Path,
    _session_end_command: &str,
) -> RuntimeProbeOutcome {
    RuntimeProbeOutcome::Unavailable
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = *chunk.get(1).unwrap_or(&0);
        let third = *chunk.get(2).unwrap_or(&0);
        encoded.push(char::from(ALPHABET[usize::from(first >> 2)]));
        encoded.push(char::from(
            ALPHABET[usize::from(((first & 0b0000_0011) << 4) | (second >> 4))],
        ));
        encoded.push(if chunk.len() > 1 {
            char::from(ALPHABET[usize::from(((second & 0b0000_1111) << 2) | (third >> 6))])
        } else {
            '='
        });
        encoded.push(if chunk.len() > 2 {
            char::from(ALPHABET[usize::from(third & 0b0011_1111)])
        } else {
            '='
        });
    }
    encoded
}

fn read_hooks_document(path: &Path) -> Result<Value, CodexIntegrationError> {
    let bytes = read_required_safe_bytes(path)?;
    parse_existing_hooks_bytes(&bytes)
}

fn read_config_document(path: &Path) -> Result<DocumentMut, CodexIntegrationError> {
    reject_symbolic_link(path)?;
    let bytes = read_optional_bytes(path)?;
    parse_config_bytes(bytes.as_deref())
}

/// Parses setup input. Only setup may synthesize the empty owned scaffold when
/// `hooks.json` does not exist yet; repair never normalizes a pre-existing file.
fn parse_hooks_bytes_for_setup(bytes: Option<&[u8]>) -> Result<Value, CodexIntegrationError> {
    let mut value = match bytes {
        Some(bytes) => {
            serde_json::from_slice(bytes).map_err(|_| CodexIntegrationError::HooksShape)?
        }
        None => json!({"description": OWNED_DESCRIPTION, "hooks": {}}),
    };
    let object = value
        .as_object_mut()
        .ok_or(CodexIntegrationError::HooksShape)?;
    match object.get("hooks") {
        Some(Value::Object(_)) => {}
        None => {
            object.insert("hooks".to_owned(), Value::Object(Map::new()));
        }
        Some(_) => return Err(CodexIntegrationError::HooksShape),
    }
    Ok(value)
}

/// Parses an existing `hooks.json` without synthesizing missing structural
/// fields. This preserves the repair boundary: an unknown or partial document
/// cannot be upgraded into a known writable shape by a repair attempt.
fn parse_existing_hooks_bytes(bytes: &[u8]) -> Result<Value, CodexIntegrationError> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|_| CodexIntegrationError::HooksShape)?;
    validate_existing_hooks_wire_bytes(bytes)?;
    value
        .as_object()
        .and_then(|root| root.get("hooks"))
        .and_then(Value::as_object)
        .ok_or(CodexIntegrationError::HooksShape)?;
    Ok(value)
}

fn parse_config_bytes(bytes: Option<&[u8]>) -> Result<DocumentMut, CodexIntegrationError> {
    let text = match bytes {
        Some(bytes) => {
            std::str::from_utf8(bytes).map_err(|_| CodexIntegrationError::ConfigShape)?
        }
        None => "",
    };
    text.parse::<DocumentMut>()
        .map_err(|_| CodexIntegrationError::ConfigShape)
}

fn append_owned_hooks(hooks: &mut Value, owned: &[OwnedHook]) -> Result<(), CodexIntegrationError> {
    let events = hooks_events_mut(hooks)?;
    for declaration in owned {
        let groups = events
            .entry(declaration.event.clone())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or(CodexIntegrationError::HooksShape)?;
        groups.push(declaration.group.clone());
    }
    Ok(())
}

fn remove_owned_hooks(hooks: &mut Value, owned: &[OwnedHook]) -> Result<(), CodexIntegrationError> {
    let events = hooks_events_mut(hooks)?;
    for declaration in owned {
        let groups = events
            .get_mut(&declaration.event)
            .and_then(Value::as_array_mut)
            .ok_or(CodexIntegrationError::ModifiedOwnedHook)?;
        let matches = groups
            .iter()
            .enumerate()
            .filter_map(|(index, group)| (group == &declaration.group).then_some(index))
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(CodexIntegrationError::ModifiedOwnedHook);
        }
        groups.remove(matches[0]);
        if groups.is_empty() {
            events.remove(&declaration.event);
        }
    }
    Ok(())
}

fn locate_owned_hooks(
    hooks: &Value,
    owned: &[OwnedHook],
) -> Result<BTreeMap<String, usize>, CodexIntegrationError> {
    let events = hooks_events(hooks)?;
    let mut locations = BTreeMap::new();
    for declaration in owned {
        let groups = events
            .get(&declaration.event)
            .and_then(Value::as_array)
            .ok_or(CodexIntegrationError::ModifiedOwnedHook)?;
        let matches = groups
            .iter()
            .enumerate()
            .filter_map(|(index, group)| (group == &declaration.group).then_some(index))
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(CodexIntegrationError::ModifiedOwnedHook);
        }
        locations.insert(declaration.event.clone(), matches[0]);
    }
    Ok(locations)
}

/// Checks the bounded outer wire shape shared by the admitted command-Hook
/// profiles. External Hook handler kinds (including MCP) remain opaque, but
/// they must still use the known group/handler envelope so that an unknown
/// future schema cannot be mistaken for an installable runtime.
fn validate_known_hook_wire_shape(hooks: &Value) -> Result<(), CodexIntegrationError> {
    for groups in hooks_events(hooks)?.values() {
        let groups = groups.as_array().ok_or(CodexIntegrationError::HooksShape)?;
        for group in groups {
            let handlers = group
                .get("hooks")
                .and_then(Value::as_array)
                .ok_or(CodexIntegrationError::HooksShape)?;
            if handlers.is_empty()
                || handlers.iter().any(|handler| {
                    !handler.is_object()
                        || handler
                            .get("type")
                            .and_then(Value::as_str)
                            .is_none_or(|handler_type| handler_type.trim().is_empty())
                })
            {
                return Err(CodexIntegrationError::HooksShape);
            }
        }
    }
    Ok(())
}

fn contains_tabbeacon_like_hook(hooks: &Value) -> bool {
    hooks_events(hooks).is_ok_and(|events| {
        events.values().any(|groups| {
            groups
                .as_array()
                .is_some_and(|groups| groups.iter().any(contains_tabbeacon_like_group))
        })
    })
}

fn contains_tabbeacon_like_group(group: &Value) -> bool {
    group_looks_like_tabbeacon_hook(group, None)
}

fn group_looks_like_tabbeacon_hook(group: &Value, executable: Option<&Path>) -> bool {
    value_contains_tabbeacon_marker(group)
        || group
            .get("hooks")
            .and_then(Value::as_array)
            .is_some_and(|handlers| {
                handlers.iter().any(|handler| {
                    ["command", "commandWindows"]
                        .into_iter()
                        .filter_map(|key| handler.get(key).and_then(Value::as_str))
                        .any(|command| command_looks_like_tabbeacon_hook(command, executable))
                })
            })
}

/// A current group that shares a manifest-owned command is a modified owned
/// declaration, even when a cosmetic or timeout field has changed. It must not
/// be treated as an unrelated third-party Hook merely because its full JSON
/// value is no longer exact.
fn group_is_partial_manifest_owned(group: &Value, event: &str, owned: &[OwnedHook]) -> bool {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|handlers| {
            owned
                .iter()
                .filter(|declaration| declaration.event == event)
                .filter_map(|declaration| declaration.group.get("hooks").and_then(Value::as_array))
                .any(|owned_handlers| {
                    handlers.iter().any(|handler| {
                        owned_handlers.iter().any(|owned_handler| {
                            (["command", "commandWindows"].into_iter().any(|key| {
                                handler.get(key).and_then(Value::as_str)
                                    == owned_handler.get(key).and_then(Value::as_str)
                                    && handler.get(key).and_then(Value::as_str).is_some()
                            })) || (handler.get("type").and_then(Value::as_str) == Some("mcp_tool")
                                && owned_handler.get("type").and_then(Value::as_str)
                                    == Some("mcp_tool")
                                && handler.get("server").and_then(Value::as_str)
                                    == owned_handler.get("server").and_then(Value::as_str)
                                && handler.get("server").and_then(Value::as_str).is_some())
                        })
                    })
                })
        })
}

/// A group added after the saved baseline needs an affirmative external source
/// marker. The marker is intentionally narrow: either a non-TabBeacon plugin
/// identifier, or an MCP server/tool pair. Arbitrary command text is never an
/// ownership proof and therefore remains a baseline-drift hard stop.
fn has_external_hook_provenance(group: &Value) -> bool {
    let plugin_provenance = group
        .get("plugin")
        .and_then(Value::as_str)
        .is_some_and(|plugin| {
            let plugin = plugin.trim();
            !plugin.is_empty() && !plugin.to_ascii_lowercase().contains("tabbeacon")
        });
    let mcp_provenance = group
        .get("hooks")
        .and_then(Value::as_array)
        .is_some_and(|handlers| {
            handlers.iter().any(|handler| {
                handler
                    .get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| kind.eq_ignore_ascii_case("mcp_tool"))
                    && handler
                        .get("server")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.trim().is_empty())
                    && handler
                        .get("tool")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.trim().is_empty())
            })
        });
    plugin_provenance || mcp_provenance
}

fn value_contains_tabbeacon_marker(value: &Value) -> bool {
    match value {
        Value::String(text) => text.to_ascii_lowercase().contains("tabbeacon"),
        Value::Array(values) => values.iter().any(value_contains_tabbeacon_marker),
        Value::Object(values) => values.iter().any(|(key, value)| {
            key.to_ascii_lowercase().contains("tabbeacon") || value_contains_tabbeacon_marker(value)
        }),
        Value::Null | Value::Bool(_) | Value::Number(_) => false,
    }
}

fn command_looks_like_tabbeacon_hook(command: &str, executable: Option<&Path>) -> bool {
    let direct = command.to_ascii_lowercase();
    if direct.contains("tabbeacon") {
        return true;
    }
    let parts = command.split_ascii_whitespace().collect::<Vec<_>>();
    let Some(encoded) = parts.windows(2).find_map(|parts| {
        matches!(
            parts[0].trim_matches(['\'', '"']),
            value if value.eq_ignore_ascii_case("-encodedcommand")
                || value.eq_ignore_ascii_case("-enc")
        )
        .then_some(parts[1].trim_matches(['\'', '"']))
    }) else {
        return false;
    };
    let Some(bytes) = decode_base64(encoded) else {
        // A malformed encoded PowerShell command cannot prove that the group
        // is external. Fail closed as TabBeacon-like when it advertises the
        // same invocation channel.
        return direct.contains("powershell");
    };
    if !bytes.len().is_multiple_of(2) {
        return true;
    }
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let Ok(script) = String::from_utf16(&units) else {
        return true;
    };
    let script = script.to_ascii_lowercase();
    script.contains("tabbeacon")
        || executable.is_some_and(|path| {
            path.to_str()
                .is_some_and(|path| script.contains(&path.to_ascii_lowercase()))
        })
}

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(4) {
        return None;
    }
    let mut bytes = Vec::with_capacity(value.len() / 4 * 3);
    for chunk in value.as_bytes().chunks_exact(4) {
        let first = base64_value(chunk[0])?;
        let second = base64_value(chunk[1])?;
        let third = if chunk[2] == b'=' {
            None
        } else {
            Some(base64_value(chunk[2])?)
        };
        let fourth = if chunk[3] == b'=' {
            None
        } else {
            Some(base64_value(chunk[3])?)
        };
        if third.is_none() && fourth.is_some() {
            return None;
        }
        bytes.push((first << 2) | (second >> 4));
        if let Some(third) = third {
            bytes.push(((second & 0b0000_1111) << 4) | (third >> 2));
            if let Some(fourth) = fourth {
                bytes.push(((third & 0b0000_0011) << 6) | fourth);
            }
        }
    }
    Some(bytes)
}

fn base64_value(byte: u8) -> Option<u8> {
    Some(match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => return None,
    })
}

fn inventory_event_id(event: &str) -> &'static str {
    match event_key_label(event) {
        "unsupported" => "unsupported",
        value => value,
    }
}

fn inventory_state_key(
    hooks_path: &Path,
    event: &str,
    group_index: usize,
    handler_index: usize,
) -> String {
    format!(
        "{}:{}:{group_index}:{handler_index}",
        hooks_path.display(),
        event_key_label(event)
    )
}

fn inventory_handler_kind(handler: &Value) -> HookHandlerKind {
    match handler.get("type").and_then(Value::as_str) {
        Some("command") => HookHandlerKind::Command,
        Some("mcp_tool") => HookHandlerKind::McpTool,
        Some(_) | None => HookHandlerKind::Unsupported,
    }
}

fn inventory_timeout(handler: &Value) -> Option<u64> {
    handler.get("timeout").and_then(Value::as_u64)
}

fn inventory_fingerprint(value: &Value) -> String {
    let bytes = serde_json::to_vec(&canonical_json(value)).expect("JSON values always serialize");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn inventory_trust_state(
    known_wire_shape: bool,
    enabled: bool,
    trusted: Option<&str>,
    declaration: &OwnedHook,
) -> HookTrustState {
    if !known_wire_shape {
        HookTrustState::UnsupportedOrUnavailable
    } else if !enabled {
        HookTrustState::Disabled
    } else if trusted.is_none() {
        HookTrustState::ReviewRequired
    } else if trusted == Some(normalized_hook_hash(declaration).as_str()) {
        HookTrustState::Trusted
    } else {
        HookTrustState::HashStaleOrChanged
    }
}

fn inventory_currentness(
    profile_is_supported: bool,
    desired: Option<&[OwnedHook]>,
    declaration: &OwnedHook,
    runtime_continuity: CodexRuntimeContinuity,
) -> HookCurrentness {
    if profile_is_supported
        && desired.is_some_and(|desired| desired.iter().any(|candidate| candidate == declaration))
    {
        HookCurrentness::Current
    } else if !profile_is_supported
        && runtime_continuity == CodexRuntimeContinuity::PreservedUnadmitted
    {
        HookCurrentness::InstalledExactUnadmitted
    } else if !profile_is_supported {
        HookCurrentness::UnsupportedOrUnavailable
    } else {
        HookCurrentness::Stale
    }
}

fn hooks_events(hooks: &Value) -> Result<&Map<String, Value>, CodexIntegrationError> {
    hooks
        .as_object()
        .and_then(|root| root.get("hooks"))
        .and_then(Value::as_object)
        .ok_or(CodexIntegrationError::HooksShape)
}

fn hooks_events_mut(hooks: &mut Value) -> Result<&mut Map<String, Value>, CodexIntegrationError> {
    hooks
        .as_object_mut()
        .and_then(|root| root.get_mut("hooks"))
        .and_then(Value::as_object_mut)
        .ok_or(CodexIntegrationError::HooksShape)
}

fn hooks_is_only_owned_scaffold(hooks: &Value) -> bool {
    let Some(root) = hooks.as_object() else {
        return false;
    };
    root.len() == 2
        && root.get("description").and_then(Value::as_str) == Some(OWNED_DESCRIPTION)
        && root
            .get("hooks")
            .and_then(Value::as_object)
            .is_some_and(Map::is_empty)
}

fn serialize_hooks(hooks: &Value) -> Result<Vec<u8>, CodexIntegrationError> {
    let mut bytes =
        serde_json::to_vec_pretty(hooks).map_err(|_| CodexIntegrationError::HooksShape)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct JsonByteRange {
    start: usize,
    end: usize,
}

/// Adds missing owned groups by inserting only compact new JSON fragments. The
/// byte ranges representing existing third-party groups are never parsed and
/// re-emitted, so their whitespace, key order, and handler representation stay
/// exactly as the user or external tool wrote them.
fn append_owned_hooks_preserving_external_bytes(
    original: &[u8],
    repaired: &Value,
    missing: &[OwnedHook],
) -> Result<Vec<u8>, CodexIntegrationError> {
    let original_value = parse_existing_hooks_bytes(original)?;
    let original_events = hooks_events(&original_value)?;
    let repaired_events = hooks_events(repaired)?;
    let document = json_document_range(original).ok_or(CodexIntegrationError::HooksShape)?;
    let root = json_object_members(original, document).ok_or(CodexIntegrationError::HooksShape)?;
    let hooks_range = root
        .iter()
        .find_map(|(key, range)| (key == "hooks").then_some(*range))
        .filter(|range| original.get(range.start) == Some(&b'{'))
        .ok_or(CodexIntegrationError::HooksShape)?;
    let hook_members =
        json_object_members(original, hooks_range).ok_or(CodexIntegrationError::HooksShape)?;
    let mut raw_events = BTreeMap::new();
    for (event, range) in &hook_members {
        if raw_events.insert(event.as_str(), *range).is_some() {
            return Err(CodexIntegrationError::HooksShape);
        }
    }

    let mut insertions = Vec::new();
    let mut new_event_members = Vec::new();
    for declaration in missing {
        let group = serde_json::to_vec(&declaration.group)
            .map_err(|_| CodexIntegrationError::HooksShape)?;
        if let Some(range) = raw_events.get(declaration.event.as_str()) {
            if original.get(range.start) != Some(&b'[') || range.end <= range.start + 1 {
                return Err(CodexIntegrationError::HooksShape);
            }
            let was_empty = original_events
                .get(&declaration.event)
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty);
            let mut fragment = Vec::with_capacity(group.len() + usize::from(!was_empty));
            if !was_empty {
                fragment.push(b',');
            }
            fragment.extend(group);
            insertions.push((range.end - 1, fragment));
        } else {
            let repaired_groups = repaired_events
                .get(&declaration.event)
                .and_then(Value::as_array)
                .ok_or(CodexIntegrationError::HooksShape)?;
            if repaired_groups.len() != 1 || repaired_groups[0] != declaration.group {
                return Err(CodexIntegrationError::HooksShape);
            }
            let event = serde_json::to_string(&declaration.event)
                .map_err(|_| CodexIntegrationError::HooksShape)?;
            let mut member = event.into_bytes();
            member.push(b':');
            member.push(b'[');
            member.extend(group);
            member.push(b']');
            new_event_members.push(member);
        }
    }
    if !new_event_members.is_empty() {
        let mut fragment = Vec::new();
        if !hook_members.is_empty() {
            fragment.push(b',');
        }
        for (index, member) in new_event_members.into_iter().enumerate() {
            if index > 0 {
                fragment.push(b',');
            }
            fragment.extend(member);
        }
        insertions.push((hooks_range.end - 1, fragment));
    }
    insertions.sort_by_key(|(offset, _)| *offset);
    if insertions.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(CodexIntegrationError::HooksShape);
    }
    let mut output = original.to_vec();
    for (offset, fragment) in insertions.into_iter().rev() {
        output.splice(offset..offset, fragment);
    }
    let output_value = parse_existing_hooks_bytes(&output)?;
    if output_value != *repaired {
        return Err(CodexIntegrationError::HooksShape);
    }
    Ok(output)
}

fn validate_existing_hooks_wire_bytes(bytes: &[u8]) -> Result<(), CodexIntegrationError> {
    let document = json_document_range(bytes).ok_or(CodexIntegrationError::HooksShape)?;
    if bytes.get(document.start) != Some(&b'{') {
        return Err(CodexIntegrationError::HooksShape);
    }
    validate_json_value_keys(bytes, document)?;
    let root = json_object_members(bytes, document).ok_or(CodexIntegrationError::HooksShape)?;
    let hooks = root
        .iter()
        .find_map(|(key, range)| (key == "hooks").then_some(*range))
        .filter(|range| bytes.get(range.start) == Some(&b'{'))
        .ok_or(CodexIntegrationError::HooksShape)?;
    let _ = json_object_members(bytes, hooks).ok_or(CodexIntegrationError::HooksShape)?;
    Ok(())
}

fn validate_json_value_keys(
    bytes: &[u8],
    range: JsonByteRange,
) -> Result<(), CodexIntegrationError> {
    match bytes.get(range.start) {
        Some(b'{') => {
            let members =
                json_object_members(bytes, range).ok_or(CodexIntegrationError::HooksShape)?;
            let mut keys = BTreeSet::new();
            for (key, value) in members {
                if !keys.insert(key) {
                    return Err(CodexIntegrationError::HooksShape);
                }
                validate_json_value_keys(bytes, value)?;
            }
        }
        Some(b'[') => {
            for value in json_array_values(bytes, range).ok_or(CodexIntegrationError::HooksShape)? {
                validate_json_value_keys(bytes, value)?;
            }
        }
        Some(_) => {}
        None => return Err(CodexIntegrationError::HooksShape),
    }
    Ok(())
}

fn json_document_range(bytes: &[u8]) -> Option<JsonByteRange> {
    let start = skip_json_whitespace(bytes, 0);
    let end = json_value_end(bytes, start)?;
    (skip_json_whitespace(bytes, end) == bytes.len()).then_some(JsonByteRange { start, end })
}

fn json_object_members(bytes: &[u8], range: JsonByteRange) -> Option<Vec<(String, JsonByteRange)>> {
    (bytes.get(range.start) == Some(&b'{') && bytes.get(range.end.checked_sub(1)?) == Some(&b'}'))
        .then_some(())?;
    let mut cursor = skip_json_whitespace(bytes, range.start + 1);
    if cursor == range.end - 1 {
        return Some(Vec::new());
    }
    let mut members = Vec::new();
    loop {
        let key_start = cursor;
        let key_end = json_string_end(bytes, key_start)?;
        let key = serde_json::from_slice::<String>(&bytes[key_start..key_end]).ok()?;
        cursor = skip_json_whitespace(bytes, key_end);
        (bytes.get(cursor) == Some(&b':')).then_some(())?;
        cursor = skip_json_whitespace(bytes, cursor + 1);
        let value_start = cursor;
        let value_end = json_value_end(bytes, value_start)?;
        members.push((
            key,
            JsonByteRange {
                start: value_start,
                end: value_end,
            },
        ));
        cursor = skip_json_whitespace(bytes, value_end);
        match bytes.get(cursor) {
            Some(b',') => cursor = skip_json_whitespace(bytes, cursor + 1),
            Some(b'}') if cursor == range.end - 1 => return Some(members),
            _ => return None,
        }
    }
}

fn json_array_values(bytes: &[u8], range: JsonByteRange) -> Option<Vec<JsonByteRange>> {
    (bytes.get(range.start) == Some(&b'[') && bytes.get(range.end.checked_sub(1)?) == Some(&b']'))
        .then_some(())?;
    let mut cursor = skip_json_whitespace(bytes, range.start + 1);
    if cursor == range.end - 1 {
        return Some(Vec::new());
    }
    let mut values = Vec::new();
    loop {
        let start = cursor;
        let end = json_value_end(bytes, start)?;
        values.push(JsonByteRange { start, end });
        cursor = skip_json_whitespace(bytes, end);
        match bytes.get(cursor) {
            Some(b',') => cursor = skip_json_whitespace(bytes, cursor + 1),
            Some(b']') if cursor == range.end - 1 => return Some(values),
            _ => return None,
        }
    }
}

fn json_value_end(bytes: &[u8], start: usize) -> Option<usize> {
    match bytes.get(start) {
        Some(b'"') => json_string_end(bytes, start),
        Some(b'{') => json_object_end(bytes, start),
        Some(b'[') => json_array_end(bytes, start),
        Some(_) => {
            let end = bytes[start..]
                .iter()
                .position(|byte| byte.is_ascii_whitespace() || matches!(*byte, b',' | b']' | b'}'))
                .map_or(bytes.len(), |offset| start + offset);
            (end > start).then_some(end)
        }
        None => None,
    }
}

fn json_object_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut cursor = skip_json_whitespace(bytes, start + 1);
    if bytes.get(cursor) == Some(&b'}') {
        return Some(cursor + 1);
    }
    loop {
        cursor = json_string_end(bytes, cursor)?;
        cursor = skip_json_whitespace(bytes, cursor);
        (bytes.get(cursor) == Some(&b':')).then_some(())?;
        cursor = skip_json_whitespace(bytes, cursor + 1);
        cursor = json_value_end(bytes, cursor)?;
        cursor = skip_json_whitespace(bytes, cursor);
        match bytes.get(cursor) {
            Some(b',') => cursor = skip_json_whitespace(bytes, cursor + 1),
            Some(b'}') => return Some(cursor + 1),
            _ => return None,
        }
    }
}

fn json_array_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut cursor = skip_json_whitespace(bytes, start + 1);
    if bytes.get(cursor) == Some(&b']') {
        return Some(cursor + 1);
    }
    loop {
        cursor = json_value_end(bytes, cursor)?;
        cursor = skip_json_whitespace(bytes, cursor);
        match bytes.get(cursor) {
            Some(b',') => cursor = skip_json_whitespace(bytes, cursor + 1),
            Some(b']') => return Some(cursor + 1),
            _ => return None,
        }
    }
}

fn json_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    (bytes.get(start) == Some(&b'"')).then_some(())?;
    let mut cursor = start + 1;
    while let Some(byte) = bytes.get(cursor) {
        match byte {
            b'"' => return Some(cursor + 1),
            b'\\' => cursor += 2,
            _ => cursor += 1,
        }
    }
    None
}

fn skip_json_whitespace(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    cursor
}

fn terminal_title_item(config: &DocumentMut) -> Result<Option<&Item>, CodexIntegrationError> {
    let Some(tui) = config.as_table().get("tui") else {
        return Ok(None);
    };
    let table = tui
        .as_table_like()
        .ok_or(CodexIntegrationError::ConfigShape)?;
    Ok(table.get("terminal_title"))
}

fn terminal_title_is_disabled(config: &DocumentMut) -> Result<bool, CodexIntegrationError> {
    Ok(terminal_title_item(config)?
        .and_then(Item::as_array)
        .is_some_and(Array::is_empty))
}

fn disable_terminal_title(config: &mut DocumentMut) -> Result<(), CodexIntegrationError> {
    if !config.as_table().contains_key("tui") {
        config["tui"] = Item::Table(Table::new());
    }
    let tui = config["tui"]
        .as_table_like_mut()
        .ok_or(CodexIntegrationError::ConfigShape)?;
    tui.insert("terminal_title", value(Array::new()));
    Ok(())
}

fn restore_terminal_title(
    config: &mut DocumentMut,
    prior: Option<&str>,
) -> Result<(), CodexIntegrationError> {
    if let Some(prior) = prior {
        // `toml_edit::Item::to_string` retains the item-leading whitespace.
        // Keeping that spacing rather than adding another separator restores
        // the user's original title declaration byte-for-byte in the ordinary
        // supported shape.
        let restored = format!("terminal_title ={prior}")
            .parse::<DocumentMut>()
            .map_err(|_| CodexIntegrationError::OwnershipManifest)?;
        let item = restored
            .as_table()
            .get("terminal_title")
            .cloned()
            .ok_or(CodexIntegrationError::OwnershipManifest)?;
        let tui = config["tui"]
            .as_table_like_mut()
            .ok_or(CodexIntegrationError::ConfigShape)?;
        tui.insert("terminal_title", item);
    } else {
        let remove_tui = {
            let tui = config["tui"]
                .as_table_like_mut()
                .ok_or(CodexIntegrationError::ConfigShape)?;
            tui.remove("terminal_title");
            tui.is_empty()
        };
        if remove_tui {
            config.as_table_mut().remove("tui");
        }
    }
    Ok(())
}

fn owned_mcp_server_is_exact(
    config: &DocumentMut,
    declaration: &OwnedMcpServer,
) -> Result<bool, CodexIntegrationError> {
    let Some(servers) = config.as_table().get("mcp_servers") else {
        return Ok(false);
    };
    let servers = servers
        .as_table_like()
        .ok_or(CodexIntegrationError::ConfigShape)?;
    let Some(server) = servers.get(&declaration.name) else {
        return Ok(false);
    };
    let server = server
        .as_table_like()
        .ok_or(CodexIntegrationError::ConfigShape)?;
    let command = server
        .get("command")
        .and_then(Item::as_str)
        .map(PathBuf::from);
    let args = server
        .get("args")
        .and_then(Item::as_array)
        .and_then(|args| {
            args.iter()
                .map(|item| item.as_str().map(ToOwned::to_owned))
                .collect::<Option<Vec<_>>>()
        });
    // Preserve the distinction between an absent legacy field and a present
    // malformed one. The former is the exact v0.5.2 migration predecessor;
    // the latter is an external modification that setup must never overwrite.
    let env_vars = server.get("env_vars").map(|variables| {
        variables.as_array().and_then(|variables| {
            variables
                .iter()
                .map(|item| item.as_str().map(ToOwned::to_owned))
                .collect::<Option<Vec<_>>>()
        })
    });
    let omit_tools_from = server
        .get("omit_tools_from")
        .and_then(Item::as_array)
        .and_then(|surfaces| {
            surfaces
                .iter()
                .map(|item| item.as_str().map(ToOwned::to_owned))
                .collect::<Option<Vec<_>>>()
        });
    let no_unowned_fields = server
        .iter()
        .all(|(key, _)| matches!(key, "command" | "args" | "env_vars" | "omit_tools_from"));
    let env_vars_are_exact = if declaration.env_vars.is_empty() {
        env_vars.is_none()
    } else {
        env_vars.as_ref().and_then(|variables| variables.as_deref())
            == Some(declaration.env_vars.as_slice())
    };
    // A short-lived pre-release manifest can legitimately lack the new
    // visibility declaration. Recognize that exact older form so setup can
    // replace it transactionally; never treat an arbitrary extra field as
    // owned.
    let omit_tools_from_is_exact = if declaration.omit_tools_from.is_empty() {
        omit_tools_from.is_none()
    } else {
        omit_tools_from.as_deref() == Some(declaration.omit_tools_from.as_slice())
    };
    Ok(command.as_ref() == Some(&declaration.command)
        && args.as_deref() == Some(declaration.args.as_slice())
        && env_vars_are_exact
        && omit_tools_from_is_exact
        && no_unowned_fields)
}

fn install_owned_mcp_server(
    config: &mut DocumentMut,
    declaration: &OwnedMcpServer,
) -> Result<bool, CodexIntegrationError> {
    let root = config.as_table_mut();
    if !root.contains_key("mcp_servers") {
        root.insert("mcp_servers", Item::Table(Table::new()));
    }
    let servers = root
        .get_mut("mcp_servers")
        .and_then(Item::as_table_like_mut)
        .ok_or(CodexIntegrationError::ConfigShape)?;
    if servers.contains_key(&declaration.name) {
        return Err(CodexIntegrationError::TabBeaconLikeAmbiguityBlocked);
    }
    let command = declaration
        .command
        .to_str()
        .ok_or(CodexIntegrationError::UnsafeExecutablePath)?;
    let mut args = Array::new();
    for arg in &declaration.args {
        args.push(arg.as_str());
    }
    let mut env_vars = Array::new();
    for variable in &declaration.env_vars {
        env_vars.push(variable.as_str());
    }
    let mut omitted_surfaces = Array::new();
    for surface in &declaration.omit_tools_from {
        omitted_surfaces.push(surface.as_str());
    }
    let mut server = Table::new();
    server.insert("command", value(command));
    server.insert("args", value(args));
    server.insert("env_vars", value(env_vars));
    server.insert("omit_tools_from", value(omitted_surfaces));
    servers.insert(&declaration.name, Item::Table(server));
    Ok(true)
}

fn remove_owned_mcp_server(
    config: &mut DocumentMut,
    declaration: &OwnedMcpServer,
) -> Result<(), CodexIntegrationError> {
    if !owned_mcp_server_is_exact(config, declaration)? {
        return Err(CodexIntegrationError::ModifiedOwnedHook);
    }
    let remove_servers_table = {
        let root = config.as_table_mut();
        let servers = root
            .get_mut("mcp_servers")
            .and_then(Item::as_table_like_mut)
            .ok_or(CodexIntegrationError::ConfigShape)?;
        servers.remove(&declaration.name);
        servers.is_empty()
    };
    if remove_servers_table {
        config.as_table_mut().remove("mcp_servers");
    }
    Ok(())
}

fn hook_trust_check(
    config: &DocumentMut,
    hooks_path: &Path,
    hooks: &Value,
    owned: &[OwnedHook],
) -> DoctorCheck {
    let Ok(locations) = locate_owned_hooks(hooks, owned) else {
        return fail("hooks.trust", "owned hook positions cannot be resolved");
    };
    let mut untrusted = 0_usize;
    let mut modified = 0_usize;
    let mut disabled = 0_usize;
    for declaration in owned {
        let Some(group_index) = locations.get(&declaration.event).copied() else {
            return fail("hooks.trust", "owned hook positions cannot be resolved");
        };
        let key = format!(
            "{}:{}:{group_index}:0",
            hooks_path.display(),
            event_key_label(&declaration.event)
        );
        let expected = normalized_hook_hash(declaration);
        if !hook_is_enabled(config, &key) {
            disabled += 1;
            continue;
        }
        match trusted_hash(config, &key) {
            Some(actual) if actual == expected => {}
            Some(_) => modified += 1,
            None => untrusted += 1,
        }
    }
    if modified > 0 || disabled > 0 {
        let summary = if disabled > 0 {
            format!(
                "HOOK_DISABLED: {disabled} owned hooks are disabled; TRUST_HASH_STALE_OR_CHANGED: {modified} trusted hashes differ while declarations remain exact"
            )
        } else {
            format!(
                "TRUST_HASH_STALE_OR_CHANGED: {modified} trusted hashes differ while declarations remain exact"
            )
        };
        fail("hooks.trust", summary)
    } else if untrusted > 0 {
        warning(
            "hooks.trust",
            format!(
                "TRUST_REVIEW_REQUIRED: {untrusted} owned hooks require review in Codex /hooks"
            ),
        )
    } else {
        pass(
            "hooks.trust",
            "TRUST_HASH_CURRENT_AND_ACTIVE: all owned hooks are trusted and active",
        )
    }
}

fn codex_version_check(version: Option<&ProbedCodexProfile>) -> DoctorCheck {
    match version {
        Some((_, CodexCompatibilityState::Supported(_))) => {
            pass("codex.version", "Codex version is source-audited")
        }
        Some((version, CodexCompatibilityState::Experimental(_))) => fail(
            "codex.version",
            format!("Codex {version} is tracked but hook-profile review is experimental"),
        ),
        Some((version, CodexCompatibilityState::Unknown)) => {
            fail("codex.version", unknown_profile_summary(version))
        }
        Some((version, CodexCompatibilityState::Unsupported(_))) => fail(
            "codex.version",
            format!("Codex {version} is source-audited as unsupported"),
        ),
        None => fail("codex.version", "Codex executable/version is unavailable"),
    }
}

fn compatibility_state(version: Option<&ProbedCodexProfile>) -> CodexCompatibilityState {
    version.map_or(CodexCompatibilityState::Unknown, |(_, state)| *state)
}

fn codex_profile_check(version: Option<&ProbedCodexProfile>) -> DoctorCheck {
    match version {
        Some((_, CodexCompatibilityState::Supported(profile))) => pass(
            "codex.hook-profile",
            format!(
                "{}: transport={}; wire={}; events={}; turn-aware={}; agent-aware={}; compact-aware={}; synchronous={}; timeout={}s; title={}; unknown=ignore-fail-open; reconcile={}",
                profile.id(),
                if profile.uses_mcp_hook_transport() {
                    "mcp_tool"
                } else {
                    "command"
                },
                profile.wire_shape().id(),
                profile.lifecycle_events().len(),
                profile.turn_aware(),
                profile.agent_aware(),
                profile.compact_aware(),
                profile.timeout().synchronous_required(),
                profile.timeout().declaration_timeout_seconds(),
                profile
                    .terminal_title_ownership()
                    .tabbeacon_delegation_key(),
                profile.reconciliation_note()
            ),
        ),
        Some((version, CodexCompatibilityState::Experimental(_))) => fail(
            "codex.hook-profile",
            format!("Codex {version} has an experimental Hook profile"),
        ),
        Some((version, CodexCompatibilityState::Unknown)) => {
            fail("codex.hook-profile", unknown_profile_summary(version))
        }
        Some((version, CodexCompatibilityState::Unsupported(_))) => fail(
            "codex.hook-profile",
            format!("Codex {version} is source-audited as unsupported"),
        ),
        None => fail(
            "codex.hook-profile",
            "Hook profile cannot be classified without a Codex version",
        ),
    }
}

fn unknown_profile_summary(version: &str) -> String {
    format!(
        "Detected: Codex {version}; Registry: unknown; Hook profile: unclassified; Risk: manual review required"
    )
}

fn hook_is_enabled(config: &DocumentMut, key: &str) -> bool {
    config
        .as_table()
        .get("hooks")
        .and_then(Item::as_table_like)
        .and_then(|hooks| hooks.get("state"))
        .and_then(Item::as_table_like)
        .and_then(|state| state.get(key))
        .and_then(Item::as_table_like)
        .and_then(|entry| entry.get("enabled"))
        .is_none_or(|enabled| enabled.as_bool().unwrap_or(false))
}

fn trusted_hash<'a>(config: &'a DocumentMut, key: &str) -> Option<&'a str> {
    config
        .as_table()
        .get("hooks")?
        .as_table_like()?
        .get("state")?
        .as_table_like()?
        .get(key)?
        .as_table_like()?
        .get("trusted_hash")?
        .as_str()
}

fn normalized_hook_hash(declaration: &OwnedHook) -> String {
    let handler = &declaration.group["hooks"][0];
    let normalized_handler = match handler.get("type").and_then(Value::as_str) {
        Some("command") => json!({
            "type": "command",
            "command": handler["commandWindows"],
            "timeout": handler["timeout"],
            "async": handler["async"]
        }),
        Some("mcp_tool") => json!({
            "type": "mcp_tool",
            "server": handler["server"],
            "tool": handler["tool"],
            "input": handler["input"],
            "timeout": handler["timeout"]
        }),
        Some(_) | None => Value::Null,
    };
    let normalized = json!({
        "event_name": event_key_label(&declaration.event),
        "hooks": [normalized_handler]
    });
    let canonical = canonical_json(&normalized);
    let bytes = serde_json::to_vec(&canonical).expect("JSON values always serialize");
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonical_json(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut entries = object.iter().collect::<Vec<_>>();
            entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
            Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key.clone(), canonical_json(value)))
                    .collect(),
            )
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical_json).collect()),
        other => other.clone(),
    }
}

fn event_key_label(event: &str) -> &'static str {
    match event {
        "PreToolUse" => "pre_tool_use",
        "PermissionRequest" => "permission_request",
        "PostToolUse" => "post_tool_use",
        "PreCompact" => "pre_compact",
        "PostCompact" => "post_compact",
        "SessionStart" => "session_start",
        "SessionEnd" => "session_end",
        "UserPromptSubmit" => "user_prompt_submit",
        "SubagentStart" => "subagent_start",
        "SubagentStop" => "subagent_stop",
        "Stop" => "stop",
        _ => "unsupported",
    }
}

fn parse_semver(value: &str) -> Option<(u64, u64, u64)> {
    let value = value.trim_start_matches('v');
    let mut parts = value.split(|character: char| !character.is_ascii_digit());
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

fn reject_symbolic_link(path: &Path) -> Result<(), CodexIntegrationError> {
    let mut cursor = Some(path);
    while let Some(candidate) = cursor {
        match fs::symlink_metadata(candidate) {
            Ok(metadata) => {
                ensure_not_symbolic_link(
                    metadata.file_type().is_symlink() || metadata_is_reparse_point(&metadata),
                )?;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        cursor = candidate.parent();
    }
    Ok(())
}

fn ensure_not_symbolic_link(is_symbolic_link: bool) -> Result<(), CodexIntegrationError> {
    if is_symbolic_link {
        Err(CodexIntegrationError::SymbolicLinkTarget)
    } else {
        Ok(())
    }
}

fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, io::Error> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn read_required_safe_bytes(path: &Path) -> Result<Vec<u8>, CodexIntegrationError> {
    reject_symbolic_link(path)?;
    Ok(fs::read(path)?)
}

/// Writes only when the on-disk target is byte-for-byte the version that was
/// parsed during repair preflight. A target-file lock spans the second read and
/// atomic commit, narrowing the compare/commit window for cooperating Codex or
/// third-party writers; a stale preview digest independently refuses any drift
/// observed before this bounded commit.
fn write_if_unchanged(
    path: &Path,
    expected_before: &[u8],
    replacement: &[u8],
) -> Result<(), CodexIntegrationError> {
    reject_symbolic_link(path)?;
    let mut target = OpenOptions::new().read(true).write(true).open(path)?;
    target.lock()?;
    let write_result = (|| {
        target.seek(SeekFrom::Start(0))?;
        let mut actual_before = Vec::new();
        target.read_to_end(&mut actual_before)?;
        if actual_before != expected_before {
            return Err(CodexIntegrationError::ConcurrentTargetDrift);
        }
        atomic_write(path, replacement)?;
        Ok(())
    })();
    let unlock_result = File::unlock(&target);
    write_result?;
    unlock_result?;
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

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "atomic target has no parent",
        ));
    };
    fs::create_dir_all(parent)?;
    let mut file = AtomicWriteFile::options().open(path)?;
    file.write_all(bytes)?;
    file.flush()?;
    file.commit()
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex_sha256(bytes))
}

fn is_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(is_sha256_hex)
}

fn pass(id: &'static str, summary: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        id,
        status: DoctorStatus::Pass,
        summary: summary.into(),
    }
}

fn warning(id: &'static str, summary: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        id,
        status: DoctorStatus::Warning,
        summary: summary.into(),
    }
}

fn fail(id: &'static str, summary: impl Into<String>) -> DoctorCheck {
    DoctorCheck {
        id,
        status: DoctorStatus::Fail,
        summary: summary.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        CodexIntegrationError, DoctorStatus, OwnedHook, ProbeClaim, ensure_not_symbolic_link,
        hybrid_runtime_probe_checks, normalized_hook_hash,
        windows_hook_command_for_default_comspec, write_if_unchanged,
    };
    use serde_json::json;

    #[test]
    fn normalized_hash_matches_codex_0_147_0_hooks_list() {
        let command =
            r#""C:\tabbeacon-fixture\target\debug\tabbeacon.exe" hook codex || exit /b 0"#;
        let expected = [
            (
                "PreCompact",
                "sha256:937a9e3ef2059da0b9292da7cb12f704fc94a246cb6a19a154c800654efff69e",
            ),
            (
                "PostCompact",
                "sha256:c2b55408c6a221fbdb073e30f2f3faf7caf3acfb88d9dee41e6f473ac983b873",
            ),
            (
                "SessionStart",
                "sha256:9da2b9767770763172e8d8397a3cdd721eb48a06dd39167e031429b513095752",
            ),
            (
                "UserPromptSubmit",
                "sha256:5153af8574637eca0401fd4f4a1bfe9955f810f87df37c20fbcbbc33cf9abebd",
            ),
            (
                "PreToolUse",
                "sha256:0ac98ca4ef877b0e1cd1200ba20ca91ea0d67e222b9abce123c4875b27d65a1d",
            ),
            (
                "PermissionRequest",
                "sha256:33f626e4c168e6781d4e7a058f41b0f37ee5bafa2d590e52c0926ce644df9f1e",
            ),
            (
                "PostToolUse",
                "sha256:3bc9cf13b69738ec697bacca756091910f55179a3b94ba7aedf992a1bbfa34e5",
            ),
            (
                "Stop",
                "sha256:0a6b5ac721be3f635a3c95a607e802f4469a0af2b36490d18aa168b0524698e6",
            ),
            (
                "SessionEnd",
                "sha256:d05bb545d5ac6bdc29f43fb8f5f74bd5592da686f9382ffaf34ba7222b573b28",
            ),
            (
                "SubagentStart",
                "sha256:226161dfef45cbc6eea02cf7fb2d739d2bcc7715d6ad8015743a14c1b5a3b28e",
            ),
            (
                "SubagentStop",
                "sha256:b9a14b7e612bc2a4aea0762c08cf7dabecf082a1892211b98943063b86843a50",
            ),
        ];
        for (event, hash) in expected {
            let declaration = OwnedHook {
                event: event.to_owned(),
                group: json!({
                    "hooks": [{
                        "type": "command",
                        "command": "tabbeacon hook codex",
                        "commandWindows": command,
                        "timeout": 1,
                        "async": false
                    }]
                }),
            };
            assert_eq!(normalized_hook_hash(&declaration), hash, "event={event}");
        }
    }

    #[test]
    fn windows_hook_envelope_uses_shell_neutral_fast_path_and_preserves_hostile_path_safety() {
        let fast = windows_hook_command_for_default_comspec(r"C:\TabBeacon\tabbeacon.exe");
        assert_eq!(fast, "C:\\TabBeacon\\tabbeacon.exe hook codex");

        let whitespace =
            windows_hook_command_for_default_comspec(r"C:\Program Files\TabBeacon\tabbeacon.exe");
        assert!(
            whitespace.starts_with("powershell.exe -NoProfile -NonInteractive -EncodedCommand ")
        );

        let hostile =
            windows_hook_command_for_default_comspec(r"C:\real binary & quote'\tabbeacon.exe");
        assert!(hostile.starts_with("powershell.exe -NoProfile -NonInteractive -EncodedCommand "));
        assert!(
            !hostile.contains(r"C:\real binary & quote'\tabbeacon.exe"),
            "hostile paths stay inside the encoded PowerShell payload"
        );
        assert!(
            windows_hook_command_for_default_comspec(r"C:\release!candidate\tabbeacon.exe")
                .starts_with("powershell.exe -NoProfile -NonInteractive -EncodedCommand ")
        );
        assert!(
            windows_hook_command_for_default_comspec(r"C:\TabBeacon\tabbeacon.cmd")
                .starts_with("powershell.exe -NoProfile -NonInteractive -EncodedCommand ")
        );
    }

    #[test]
    fn symbolic_link_policy_refuses_link_targets() {
        assert!(ensure_not_symbolic_link(false).is_ok());
        assert!(matches!(
            ensure_not_symbolic_link(true),
            Err(CodexIntegrationError::SymbolicLinkTarget)
        ));
    }

    #[test]
    fn repair_write_refuses_a_target_that_drifted_after_preflight() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("tabbeacon-repair-drift-{nonce}"));
        fs::create_dir_all(&root).expect("isolated repair test root");
        let path = root.join("hooks.json");
        fs::write(&path, b"preflight snapshot").expect("preflight snapshot writes");
        fs::write(&path, b"external replacement").expect("external replacement writes");

        assert!(matches!(
            write_if_unchanged(&path, b"preflight snapshot", b"repair output"),
            Err(CodexIntegrationError::ConcurrentTargetDrift)
        ));
        assert_eq!(
            fs::read(&path).expect("drifted target reads"),
            b"external replacement"
        );
        fs::remove_dir_all(&root).expect("isolated repair test cleanup");
    }

    #[test]
    fn hybrid_runtime_probe_reports_each_claim_independently() {
        for (
            mcp_event,
            termination,
            session_end,
            expected_mcp,
            expected_termination,
            expected_session_end,
        ) in [
            (
                ProbeClaim::Proven,
                ProbeClaim::Failed,
                ProbeClaim::Proven,
                DoctorStatus::Pass,
                DoctorStatus::Fail,
                DoctorStatus::Pass,
            ),
            (
                ProbeClaim::Proven,
                ProbeClaim::Proven,
                ProbeClaim::Failed,
                DoctorStatus::Pass,
                DoctorStatus::Pass,
                DoctorStatus::Fail,
            ),
            (
                ProbeClaim::Failed,
                ProbeClaim::Proven,
                ProbeClaim::Proven,
                DoctorStatus::Fail,
                DoctorStatus::Pass,
                DoctorStatus::Pass,
            ),
        ] {
            let checks =
                hybrid_runtime_probe_checks(mcp_event, termination, session_end, Some(100));
            assert_eq!(
                checks
                    .iter()
                    .find(|check| check.id() == "hooks.runtime-probe")
                    .map(super::DoctorCheck::status),
                Some(DoctorStatus::Fail)
            );
            assert_eq!(
                checks
                    .iter()
                    .find(|check| check.id() == "hooks.mcp-event-transport")
                    .map(super::DoctorCheck::status),
                Some(expected_mcp)
            );
            assert_eq!(
                checks
                    .iter()
                    .find(|check| check.id() == "hooks.codex-terminate-before-eof")
                    .map(super::DoctorCheck::status),
                Some(expected_termination)
            );
            assert_eq!(
                checks
                    .iter()
                    .find(|check| check.id() == "hooks.session-end-cleanup")
                    .map(super::DoctorCheck::status),
                Some(expected_session_end)
            );
        }
    }
}
