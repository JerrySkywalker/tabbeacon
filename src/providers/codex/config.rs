use std::{
    collections::{BTreeMap, BTreeSet},
    env, fmt,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use toml_edit::{Array, DocumentMut, Item, Table, value};

use crate::hook_inventory::{
    HookCurrentness, HookHandlerKind, HookInventory, HookInventoryEntry, HookOwner, HookSourceKind,
    HookTrustState,
};

use super::{CodexCompatibilityRegistry, CodexCompatibilityState, CodexHookProfile};

const MANIFEST_SCHEMA: &str = "tabbeacon-codex-integration-v1";
const MANIFEST_FILE: &str = "integration-v1.json";
const LOCK_FILE: &str = "integration.lock";
const OWNED_DESCRIPTION: &str = "TabBeacon user-global lifecycle hooks";
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CodexRepairReport {
    /// Stable repair result schema.
    pub schema_version: u32,
    /// Whether this was a preview, an apply, or an idempotent exact result.
    pub disposition: CodexRepairDisposition,
    /// Number of exact manifest-owned declarations proven absent.
    pub missing_declarations: usize,
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
    /// A pre-existing `TabBeacon`-like hook has no `TabBeacon` ownership proof.
    UnownedHookConflict,
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
            Self::UnownedHookConflict => {
                "a matching TabBeacon-like hook exists without ownership proof"
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
            Self::UnsafeExecutablePath => {
                "the TabBeacon executable path is unsafe for a Codex Windows command hook"
            }
            Self::SymbolicLinkTarget => {
                "a Codex integration target is a symbolic link or reparse point"
            }
        })
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
    pub fn repair(&self, apply: bool) -> Result<CodexRepairReport, CodexIntegrationError> {
        if apply {
            // Keep an unadmitted repair fully read-only, including TabBeacon's
            // own state root; repeat the probe under lock before writing.
            self.require_supported_profile()?;
            self.with_lock(|| {
                let profile = self.require_supported_profile()?;
                self.repair_locked(profile, true)
            })
        } else {
            let profile = self.require_supported_profile()?;
            self.repair_locked(profile, false)
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
        let declarations_exact = match (&manifest, &hooks) {
            (Some(manifest), Ok(hooks))
                if manifest_has_known_owned_declarations && known_wire_shape =>
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
            (Some(manifest), Some(profile)) => match desired_hooks(&self.tabbeacon_executable, profile) {
                Ok(desired) if desired == manifest.hooks => pass(
                    "hooks.currentness",
                    "CURRENTNESS_CURRENT: owned hook declarations match the current TabBeacon integration",
                ),
                Ok(_) => fail(
                    "hooks.currentness",
                    "CURRENTNESS_STALE: owned hook declarations require a TabBeacon upgrade",
                ),
                Err(_) => fail(
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
        if let Some(mut manifest) = self.load_manifest()? {
            self.validate_manifest_scope(&manifest)?;
            let mut hooks = read_hooks_document(&self.hooks_path())?;
            let mut config = read_config_document(&self.config_path())?;
            locate_owned_hooks(&hooks, &manifest.hooks)
                .map_err(|_| CodexIntegrationError::ModifiedOwnedHook)?;
            Self::validate_title_ownership(&manifest, &config)?;
            let mut changed =
                self.apply_title_ownership(&mut manifest, &mut config, tabbeacon_owns_title)?;
            if manifest.hooks != desired_hooks {
                remove_owned_hooks(&mut hooks, &manifest.hooks)?;
                append_owned_hooks(&mut hooks, &desired_hooks)?;
                atomic_write(&self.hooks_path(), &serialize_hooks(&hooks)?)?;
                manifest.hooks = desired_hooks;
                manifest.executable.clone_from(&self.tabbeacon_executable);
                changed = true;
            }
            if changed {
                self.write_manifest(&manifest)?;
                return Ok(SetupOutcome::Upgraded);
            }
            return Ok(SetupOutcome::AlreadyInstalled);
        }

        let original_hooks = read_optional_bytes(&self.hooks_path())?;
        let original_config = read_optional_bytes(&self.config_path())?;
        let mut hooks = parse_hooks_bytes(original_hooks.as_deref())?;
        let mut config = parse_config_bytes(original_config.as_deref())?;
        if contains_tabbeacon_like_hook(&hooks) {
            return Err(CodexIntegrationError::UnownedHookConflict);
        }
        append_owned_hooks(&mut hooks, &desired_hooks)?;
        let prior_title = terminal_title_item(&config)?.map(ToString::to_string);
        let title_owned = tabbeacon_owns_title && !terminal_title_is_disabled(&config)?;
        if title_owned {
            disable_terminal_title(&mut config)?;
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
            hooks: desired_hooks,
        };
        self.write_manifest(&manifest)?;
        atomic_write(&self.hooks_path(), &serialize_hooks(&hooks)?)?;
        if title_owned {
            atomic_write(&self.config_path(), config.to_string().as_bytes())?;
        }
        manifest.phase = ManifestPhase::Active;
        self.write_manifest(&manifest)?;
        Ok(SetupOutcome::InstalledTrustReviewRequired)
    }

    fn repair_locked(
        &self,
        profile: CodexHookProfile,
        apply: bool,
    ) -> Result<CodexRepairReport, CodexIntegrationError> {
        self.reject_repair_target_paths()?;
        let manifest = self
            .load_manifest()?
            .ok_or(CodexIntegrationError::OwnershipManifest)?;
        self.validate_manifest_scope(&manifest)?;
        let desired = desired_hooks(&self.tabbeacon_executable, profile)?;
        if manifest.hooks != desired || !Self::manifest_has_known_owned_declarations(&manifest) {
            return Err(CodexIntegrationError::StaleOwnedHook);
        }

        let original_hooks = read_required_safe_bytes(&self.hooks_path())?;
        let mut hooks = parse_hooks_bytes(Some(&original_hooks))?;
        validate_known_hook_wire_shape(&hooks)?;
        let config = read_config_document(&self.config_path())?;
        Self::validate_title_ownership(&manifest, &config)?;
        let missing = self.missing_repairable_owned_hooks(&hooks, &manifest)?;
        if missing.is_empty() {
            return Ok(CodexRepairReport {
                schema_version: 1,
                disposition: CodexRepairDisposition::AlreadyExact,
                missing_declarations: 0,
                manual_hook_trust_review_required: true,
            });
        }
        if !apply {
            return Ok(CodexRepairReport {
                schema_version: 1,
                disposition: CodexRepairDisposition::ReadyToApply,
                missing_declarations: missing.len(),
                manual_hook_trust_review_required: true,
            });
        }

        append_owned_hooks(&mut hooks, &missing)?;
        let repaired_hooks = serialize_hooks(&hooks)?;
        write_if_unchanged(&self.hooks_path(), &original_hooks, &repaired_hooks)?;
        Ok(CodexRepairReport {
            schema_version: 1,
            disposition: CodexRepairDisposition::RepairedTrustReviewRequired,
            missing_declarations: missing.len(),
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
        if !self.apply_title_ownership(&mut manifest, &mut config, tabbeacon_owns_title)? {
            return Ok(TitleOwnershipOutcome::AlreadyConfigured);
        }
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
        &self,
        manifest: &mut IntegrationManifest,
        config: &mut DocumentMut,
        tabbeacon_owns_title: bool,
    ) -> Result<bool, CodexIntegrationError> {
        if manifest.title_owned == tabbeacon_owns_title {
            return Ok(false);
        }
        if tabbeacon_owns_title {
            disable_terminal_title(config)?;
            atomic_write(&self.config_path(), config.to_string().as_bytes())?;
            manifest.title_owned = true;
            return Ok(true);
        }
        restore_terminal_title(config, manifest.prior_title.as_deref())?;
        let restored = config.to_string();
        if !manifest.config_backup.existed && restored.trim().is_empty() {
            fs::remove_file(self.config_path())?;
        } else {
            atomic_write(&self.config_path(), restored.as_bytes())?;
        }
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
        if manifest.title_owned && !terminal_title_is_disabled(&config)? {
            return Err(CodexIntegrationError::ModifiedOwnedTitle);
        }

        remove_owned_hooks(&mut hooks, &manifest.hooks)?;
        if manifest.created_hooks_file && hooks_is_only_owned_scaffold(&hooks) {
            fs::remove_file(self.hooks_path())?;
        } else {
            atomic_write(&self.hooks_path(), &serialize_hooks(&hooks)?)?;
        }
        if manifest.title_owned {
            restore_terminal_title(&mut config, manifest.prior_title.as_deref())?;
            let restored = config.to_string();
            if !manifest.config_backup.existed && restored.trim().is_empty() {
                fs::remove_file(self.config_path())?;
            } else {
                atomic_write(&self.config_path(), restored.as_bytes())?;
            }
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
        owned_command_hooks(&manifest.executable, 1, false)
            .map_err(|_| CodexIntegrationError::OwnershipManifest)?;
        self.validate_backup_record("hooks", &manifest.hooks_backup)?;
        self.validate_backup_record("config", &manifest.config_backup)?;
        Ok(())
    }

    /// The manifest is not merely structurally valid: runtime continuity and
    /// repair both need proof that its owned declarations are the known command
    /// Hook contract generated by the installing executable.
    fn manifest_has_known_owned_declarations(manifest: &IntegrationManifest) -> bool {
        owned_command_hooks(&manifest.executable, 1, false)
            .is_ok_and(|expected| expected == manifest.hooks)
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
    /// digest, and JSON shape have all been re-proven. A retained group that is
    /// not in this baseline could be a replacement for an owned group, so
    /// repair must leave it untouched and fail closed.
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
        let backup_bytes = read_required_safe_bytes(backup_path)?;
        if manifest.hooks_backup.digest.as_deref() != Some(&hex_sha256(&backup_bytes)) {
            return Err(CodexIntegrationError::OwnershipManifest);
        }
        let backup_hooks = parse_hooks_bytes(Some(&backup_bytes))?;
        let events = hooks_events(&backup_hooks)?;
        let mut original = BTreeMap::new();
        for (event, groups) in events {
            let groups = groups.as_array().ok_or(CodexIntegrationError::HooksShape)?;
            original.insert(event.clone(), groups.clone());
        }
        Ok(original)
    }

    /// Finds only declarations absent from a current, target-bound manifest.
    /// Every retained non-owned group must match the exact original baseline;
    /// this is what distinguishes a provably unrelated Hook from an arbitrary
    /// replacement that must remain fail-closed.
    fn missing_repairable_owned_hooks(
        &self,
        hooks: &Value,
        manifest: &IntegrationManifest,
    ) -> Result<Vec<OwnedHook>, CodexIntegrationError> {
        let events = hooks_events(hooks)?;
        let original = self.original_hook_groups(manifest)?;
        let mut missing = Vec::new();
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
                if group_looks_like_tabbeacon_hook(group, Some(&manifest.executable))
                    || !baseline
                        .iter()
                        .any(|original_group| original_group == group)
                {
                    return Err(CodexIntegrationError::UnownedHookConflict);
                }
            }
        }
        Ok(missing)
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
    hooks: Vec<OwnedHook>,
}

fn desired_hooks(
    executable: &Path,
    profile: CodexHookProfile,
) -> Result<Vec<OwnedHook>, CodexIntegrationError> {
    owned_command_hooks(
        executable,
        profile.timeout().declaration_timeout_seconds(),
        !profile.timeout().synchronous_required(),
    )
}

fn owned_command_hooks(
    executable: &Path,
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
    let windows_command = shell_independent_windows_hook_command(executable);
    Ok(HOOK_EVENTS
        .into_iter()
        .map(|event| OwnedHook {
            event: event.to_owned(),
            group: json!({
                "hooks": [{
                    "type": "command",
                    "command": windows_command.clone(),
                    "commandWindows": windows_command,
                    "timeout": timeout_seconds,
                    "async": asynchronous
                }]
            }),
        })
        .collect())
}

fn shell_independent_windows_hook_command(executable: &str) -> String {
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
    parse_hooks_bytes(Some(&bytes))
}

fn read_config_document(path: &Path) -> Result<DocumentMut, CodexIntegrationError> {
    reject_symbolic_link(path)?;
    let bytes = read_optional_bytes(path)?;
    parse_config_bytes(bytes.as_deref())
}

fn parse_hooks_bytes(bytes: Option<&[u8]>) -> Result<Value, CodexIntegrationError> {
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
            if handlers.iter().any(|handler| {
                !handler.is_object()
                    || handler
                        .get("type")
                        .and_then(Value::as_str)
                        .is_none_or(|handler_type| handler_type.trim().is_empty())
            }) {
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
    group
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

fn command_looks_like_tabbeacon_hook(command: &str, executable: Option<&Path>) -> bool {
    let direct = command.to_ascii_lowercase();
    if direct.contains("tabbeacon") && direct.contains("hook codex") {
        return true;
    }
    let Some(encoded) = command
        .split_ascii_whitespace()
        .collect::<Vec<_>>()
        .windows(2)
        .find_map(|parts| {
            parts[0]
                .eq_ignore_ascii_case("-encodedcommand")
                .then_some(parts[1])
        })
    else {
        return false;
    };
    let Some(bytes) = decode_base64(encoded) else {
        return false;
    };
    let units = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    let Ok(script) = String::from_utf16(&units) else {
        return false;
    };
    let script = script.to_ascii_lowercase();
    if !script.contains("hook codex") {
        return false;
    }
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
    if handler.get("type").and_then(Value::as_str) == Some("command") {
        HookHandlerKind::Command
    } else {
        HookHandlerKind::Unsupported
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
                "{}: wire={}; events={}; turn-aware={}; agent-aware={}; compact-aware={}; synchronous={}; timeout={}s; title={}; unknown=ignore-fail-open; reconcile={}",
                profile.id(),
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
    let normalized = json!({
        "event_name": event_key_label(&declaration.event),
        "hooks": [{
            "type": "command",
            "command": handler["commandWindows"],
            "timeout": handler["timeout"],
            "async": handler["async"]
        }]
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
/// parsed during repair preflight. This detects an independent Codex or
/// third-party edit before `TabBeacon` commits a repair and leaves the target
/// untouched when the ownership proof has gone stale.
fn write_if_unchanged(
    path: &Path,
    expected_before: &[u8],
    replacement: &[u8],
) -> Result<(), CodexIntegrationError> {
    reject_symbolic_link(path)?;
    let actual_before = fs::read(path)?;
    if actual_before != expected_before {
        return Err(CodexIntegrationError::ConcurrentTargetDrift);
    }
    atomic_write(path, replacement)?;
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
        CodexIntegrationError, OwnedHook, ensure_not_symbolic_link, normalized_hook_hash,
        write_if_unchanged,
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
}
