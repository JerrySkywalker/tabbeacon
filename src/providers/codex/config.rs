use std::{
    collections::BTreeMap,
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

use super::CodexHookProfile;

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
type ProbedCodexProfile = (String, Option<CodexHookProfile>);

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
    hook_profile: Option<CodexHookProfile>,
}

impl CodexDoctorReport {
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

    /// Whether the detected Codex version maps to an admitted Hook profile.
    #[must_use]
    pub const fn profile_supported(&self) -> bool {
        self.hook_profile.is_some()
    }

    /// Looks up one stable non-sensitive doctor check by identifier.
    #[must_use]
    pub fn check(&self, id: &str) -> Option<&DoctorCheck> {
        self.checks.iter().find(|check| check.id() == id)
    }
}

/// Safe configuration-management error with no config contents.
#[derive(Debug)]
pub enum CodexIntegrationError {
    /// A required per-user path could not be derived.
    StateRootUnavailable,
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
    /// The terminal-title value owned by setup was modified afterward.
    ModifiedOwnedTitle,
    /// A title configuration not owned by `TabBeacon` conflicts with integration.
    TerminalTitleConflict,
    /// The ownership manifest is absent, corrupt, or belongs to another target.
    OwnershipManifest,
    /// The executable path cannot be represented safely in a Windows command.
    UnsafeExecutablePath,
    /// A target file is a symbolic link and is not replaced implicitly.
    SymbolicLinkTarget,
}

impl fmt::Display for CodexIntegrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::StateRootUnavailable => "a safe per-user integration path is unavailable",
            Self::Io(_) => "an integration file operation failed",
            Self::HooksShape => "the Codex hooks file has an unsupported shape",
            Self::ConfigShape => "the Codex config file has an unsupported shape",
            Self::UnownedHookConflict => {
                "a matching TabBeacon-like hook exists without ownership proof"
            }
            Self::ModifiedOwnedHook => "a TabBeacon-owned hook was modified",
            Self::ModifiedOwnedTitle => "the TabBeacon-owned terminal-title setting was modified",
            Self::TerminalTitleConflict => {
                "Codex terminal-title ownership conflicts with TabBeacon"
            }
            Self::OwnershipManifest => "the Codex integration ownership manifest is invalid",
            Self::UnsafeExecutablePath => {
                "the TabBeacon executable path is unsafe for a Codex Windows command hook"
            }
            Self::SymbolicLinkTarget => "a Codex integration target is a symbolic link",
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
        self.with_lock(|| self.setup_locked(tabbeacon_owns_title))
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
        self.with_lock(|| self.reconcile_title_ownership_locked(tabbeacon_owns_title))
    }

    /// Audits binary, manifest, hook, trust, and terminal-title state read-only.
    #[must_use]
    pub fn doctor(&self) -> CodexDoctorReport {
        let mut checks = Vec::new();
        let version = self.probe_codex_version();
        let codex_version = version.as_ref().map(|(version, _)| version.clone());
        let hook_profile = version.as_ref().and_then(|(_, profile)| *profile);
        checks.push(codex_version_check(version.as_ref()));
        checks.push(codex_profile_check(version.as_ref()));
        checks.push(if self.tabbeacon_executable.is_file() {
            pass("tabbeacon.executable", "managed hook executable exists")
        } else {
            fail("tabbeacon.executable", "managed hook executable is missing")
        });

        let manifest = self.load_manifest().ok().flatten();
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
        if let (Some(manifest), Ok(hooks)) = (&manifest, &hooks) {
            checks.push(match locate_owned_hooks(hooks, &manifest.hooks) {
                Ok(locations) if locations.len() == manifest.hooks.len() => pass(
                    "hooks.declarations",
                    "all owned hook declarations are exact",
                ),
                _ => fail("hooks.declarations", "owned hooks are missing or modified"),
            });
            checks.push(match desired_hooks(&self.tabbeacon_executable) {
                Ok(desired) if desired == manifest.hooks => pass(
                    "hooks.currentness",
                    "owned hook declarations match the current TabBeacon integration",
                ),
                Ok(_) => fail(
                    "hooks.currentness",
                    "owned hook declarations require a TabBeacon upgrade",
                ),
                Err(_) => fail(
                    "hooks.currentness",
                    "current TabBeacon hook declarations cannot be generated safely",
                ),
            });
            checks.push(match (&version, &config) {
                (Some((_, Some(_))), Ok(config)) => {
                    hook_trust_check(config, &self.hooks_path(), hooks, &manifest.hooks)
                }
                _ => fail(
                    "hooks.trust",
                    "hook trust cannot be proven for this Codex/config shape",
                ),
            });
        } else {
            checks.push(fail(
                "hooks.declarations",
                "hooks file is missing or incompatible",
            ));
            checks.push(fail("hooks.trust", "hook trust is not proven"));
        }
        checks.push(match (&manifest, config) {
            (Some(manifest), Ok(config))
                if manifest.title_owned && terminal_title_is_disabled(&config).unwrap_or(false) =>
            {
                pass("terminal.title", "TabBeacon owns the Codex terminal title")
            }
            (Some(manifest), Ok(config))
                if !manifest.title_owned
                    && !terminal_title_is_disabled(&config).unwrap_or(false) =>
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
        });

        let overall = checks
            .iter()
            .map(DoctorCheck::status)
            .max()
            .unwrap_or(DoctorStatus::Fail);
        CodexDoctorReport {
            overall,
            checks,
            codex_version,
            hook_profile,
        }
    }

    fn setup_locked(
        &self,
        tabbeacon_owns_title: bool,
    ) -> Result<SetupOutcome, CodexIntegrationError> {
        fs::create_dir_all(&self.codex_home)?;
        reject_symbolic_link(&self.hooks_path())?;
        reject_symbolic_link(&self.config_path())?;
        let desired_hooks = desired_hooks(&self.tabbeacon_executable)?;
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
        fs::create_dir_all(&self.state_root)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.state_root.join(LOCK_FILE))?;
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
        desired_hooks(&manifest.executable)
            .map_err(|_| CodexIntegrationError::OwnershipManifest)?;
        Ok(())
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
        let profile = CodexHookProfile::for_version(version);
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

fn desired_hooks(executable: &Path) -> Result<Vec<OwnedHook>, CodexIntegrationError> {
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
                    "timeout": 1,
                    "async": false
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
    let bytes = fs::read(path)?;
    parse_hooks_bytes(Some(&bytes))
}

fn read_config_document(path: &Path) -> Result<DocumentMut, CodexIntegrationError> {
    let bytes = fs::read(path)?;
    parse_config_bytes(Some(&bytes))
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

fn contains_tabbeacon_like_hook(hooks: &Value) -> bool {
    hooks_events(hooks).is_ok_and(|events| {
        events.values().any(|groups| {
            groups.as_array().is_some_and(|groups| {
                groups.iter().any(|group| {
                    group
                        .get("hooks")
                        .and_then(Value::as_array)
                        .is_some_and(|handlers| {
                            handlers.iter().any(|handler| {
                                ["command", "commandWindows"]
                                    .into_iter()
                                    .filter_map(|key| handler.get(key).and_then(Value::as_str))
                                    .any(|command| command.contains("tabbeacon hook codex"))
                            })
                        })
                })
            })
        })
    })
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
        fail(
            "hooks.trust",
            format!(
                "{modified} owned hook definitions are modified/inactive; {disabled} owned hooks are disabled"
            ),
        )
    } else if untrusted > 0 {
        warning(
            "hooks.trust",
            format!("{untrusted} owned hooks require review in Codex /hooks"),
        )
    } else {
        pass("hooks.trust", "all owned hooks are trusted and active")
    }
}

fn codex_version_check(version: Option<&ProbedCodexProfile>) -> DoctorCheck {
    match version {
        Some((version, Some(_))) => pass(
            "codex.version",
            format!("Codex {version} is source-audited"),
        ),
        Some((version, None)) => fail(
            "codex.version",
            format!("Codex {version} is outside the admitted hook contract"),
        ),
        None => fail("codex.version", "Codex executable/version is unavailable"),
    }
}

fn codex_profile_check(version: Option<&ProbedCodexProfile>) -> DoctorCheck {
    match version {
        Some((_, Some(profile))) => pass(
            "codex.hook-profile",
            format!(
                "{}: events={}; turn-aware={}; agent-aware={}; compact-aware={}; unknown=ignore-fail-open",
                profile.id(),
                profile.lifecycle_events().len(),
                profile.turn_aware(),
                profile.agent_aware(),
                profile.compact_aware()
            ),
        ),
        Some((version, None)) => fail(
            "codex.hook-profile",
            format!("no source-audited Hook profile matches Codex {version}"),
        ),
        None => fail(
            "codex.hook-profile",
            "Hook profile cannot be classified without a Codex version",
        ),
    }
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
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(CodexIntegrationError::SymbolicLinkTarget)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, io::Error> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
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
    use super::{OwnedHook, normalized_hook_hash};
    use serde_json::json;

    #[test]
    fn normalized_hash_matches_codex_0_147_0_hooks_list() {
        let command = r#""V:\src\tabbeacon\target\debug\tabbeacon.exe" hook codex || exit /b 0"#;
        let expected = [
            (
                "PreCompact",
                "sha256:2a11700970f86f25c3a894ef6f13ccc653c0dbbb786e9a439a9808fdfc1febfa",
            ),
            (
                "PostCompact",
                "sha256:bd9dba3835dcfc8b51ea3dbc075ce23e28f09c3d9190d9ff3af7451d492e69da",
            ),
            (
                "SessionStart",
                "sha256:db9a01c26f509ce8f06a93e13a2fc19aad3658e1e8a950e16072fbd423565b61",
            ),
            (
                "UserPromptSubmit",
                "sha256:3f5a99613c9710c539eea51071addd0bbf7c003dec4941c65409a70abfeec295",
            ),
            (
                "PreToolUse",
                "sha256:deb7ed5d0ad26b0c89257fba71c876db1ce0f1af1aaa24fb1cfaa44855645f8c",
            ),
            (
                "PermissionRequest",
                "sha256:2b0781987fdad32fb28faa76e3e26822c9c94d7f416a2e37bf416f1dbc72e152",
            ),
            (
                "PostToolUse",
                "sha256:e009a62adb6baf29335b2751ed38d8311d2f05c39038707d97af0e78a0757ef9",
            ),
            (
                "Stop",
                "sha256:74332d15fd960bc8eb04f4a792327e320007ca3ebe93ed9863122bbb97ab38ef",
            ),
            (
                "SessionEnd",
                "sha256:364d9e07702ed4381fe806a253b5b565388d3c857cfff51ea583fde02e325101",
            ),
            (
                "SubagentStart",
                "sha256:4338387a6874fe79a5f7a14a09a635ca885a91006f9667f1b48cc23a2ff09206",
            ),
            (
                "SubagentStop",
                "sha256:8c02618ab03d57f168092e445e7b5bc0a11684b52001d017e0f80be5743b4aa8",
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
}
