//! Capability-gated Agy production adapter and ownership-safe setup.
//!
//! The adapter is intentionally narrower than Agy's title/status payload. G64
//! admitted exactly Agy 1.1.19, the user-global structured title callback, a
//! stable conversation identity, an equal current/project workspace root, and
//! the observed `initializing`/`working` -> [`Phase::Working`] plus `idle` ->
//! [`Phase::Ready`] lifecycle subset. Content fields and every unobserved
//! semantic remain outside the production boundary.

use std::{
    fmt, fs,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::{
    activity::{SessionWorkspaceObservability, record_provider_session_observation},
    core::{
        AgentEvidence, AgentProvider, AgentSessionKey, AuthoritySet, BackendCapabilities,
        EvidenceAuthority, EvidenceConfidence, EvidenceSource, EvidenceTieBreak, FieldUpdate,
        Phase, SessionReconciler, StatePatch,
    },
    presentation::{
        PresentationAction, PresentationPolicy, SemanticPresentationInput,
        WindowsTerminalCapabilities, WindowsTerminalRenderer,
    },
    providers::registry::ProviderRegistry,
    repo::{RepositoryAlias, StableAliasRegistry, WorkspaceIdentityResolver},
    settings::{PresentationSettings, PresentationSettingsStore},
};

use super::agy::{
    AGY_PROVIDER_ID, AgyStateRecorder, AgyVersion, MAX_AGY_QUALIFICATION_INPUT_BYTES,
    parse_qualification_object,
};
use super::agy_qualification::probe_direct_agy_version;

/// Daily Agy launch remains the provider's literal native command.
pub const AGY_DAILY_COMMAND: &str = "agy";
/// Exact Agy release admitted by the Owner-present G64 transaction.
pub const AGY_ADMITTED_VERSION: &str = "1.1.19";
/// Stable frozen profile schema accepted by the production capability gate.
pub const AGY_ADMITTED_PROFILE_SCHEMA: &str = "tabbeacon-agy-admitted-profile-v1";
/// Production is enabled only for the exact frozen profile above.
pub const AGY_PROVIDER: bool = true;

/// Stable admitted profile family.
pub const AGY_ADMITTED_PROFILE_FAMILY: &str = "tabbeacon-agy-admitted-profile";

/// Production backend state before real G64 admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyBackendState {
    Unadmitted,
    Admitted,
}

/// Safe backend-source alternatives awaiting selection from real evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyBackendSource {
    Unselected,
    TitleCallback,
    Hooks,
    Hybrid,
}

/// Whether one safe candidate fact class was observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyCandidatePresence {
    Observed,
    NotObserved,
}

/// Qualification-safe observation passed toward the future normalizer.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgySafeRawObservation {
    pub lifecycle: AgyCandidatePresence,
    pub session: AgyCandidatePresence,
    pub workspace: AgyCandidatePresence,
    pub approval: AgyCandidatePresence,
    pub background_count: AgyCandidatePresence,
}

/// A normalized scaffold value that still carries no production authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgyNormalizedObservation {
    pub state: AgyBackendState,
    pub source: AgyBackendSource,
    pub candidate_fact_count: u8,
    pub production_authority: bool,
}

/// Capability-independent normalizer scaffold.
pub struct AgyNormalizer;

impl AgyNormalizer {
    /// Counts only safe candidate fact classes and never assigns semantics.
    #[must_use]
    pub fn normalize(observation: AgySafeRawObservation) -> AgyNormalizedObservation {
        let candidate_fact_count = [
            observation.lifecycle,
            observation.session,
            observation.workspace,
            observation.approval,
            observation.background_count,
        ]
        .into_iter()
        .fold(0_u8, |count, presence| {
            count + u8::from(presence == AgyCandidatePresence::Observed)
        });
        AgyNormalizedObservation {
            state: AgyBackendState::Unadmitted,
            source: AgyBackendSource::Unselected,
            candidate_fact_count,
            production_authority: false,
        }
    }
}

/// Result of attempting to cross the Agy capability gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyEvidenceProjection {
    Unadmitted,
    Evidence,
}

/// Unforgeable admitted profile token produced only by the exact capability gate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgyAdmittedProfile {
    version: AgyVersion,
    source: AgyBackendSource,
}

impl AgyAdmittedProfile {
    /// Returns the exact admitted Agy version.
    #[must_use]
    pub const fn version(self) -> AgyVersion {
        self.version
    }

    /// Returns the sole G64-proven backend.
    #[must_use]
    pub const fn source(self) -> AgyBackendSource {
        self.source
    }
}

/// Safe rejection from the explicit future admitted-profile boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyAdmissionGateError {
    Oversized,
    Malformed,
    NoSupportedAdmittedProfileVersion,
}

impl fmt::Display for AgyAdmissionGateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Oversized => "Agy admitted profile exceeds the bounded input size",
            Self::Malformed => "Agy admitted profile is malformed",
            Self::NoSupportedAdmittedProfileVersion => {
                "no Owner-approved Agy admitted profile version exists"
            }
        })
    }
}

impl std::error::Error for AgyAdmissionGateError {}

/// Single explicit boundary for the versioned Owner-approved profile.
pub struct AgyCapabilityGate;

impl AgyCapabilityGate {
    /// Admits only the exact G64-frozen schema, version, and title backend.
    ///
    /// # Errors
    ///
    /// Returns a bounded rejection; no document can enable Agy today.
    pub fn admit_profile(bytes: &[u8]) -> Result<AgyAdmittedProfile, AgyAdmissionGateError> {
        if bytes.len() > MAX_AGY_QUALIFICATION_INPUT_BYTES {
            return Err(AgyAdmissionGateError::Oversized);
        }
        let object = parse_qualification_object(bytes).ok_or(AgyAdmissionGateError::Malformed)?;
        let exact = object.get("schema").and_then(Value::as_str)
            == Some(AGY_ADMITTED_PROFILE_SCHEMA)
            && object.get("version").and_then(Value::as_str) == Some(AGY_ADMITTED_VERSION)
            && object.get("backend").and_then(Value::as_str) == Some("title_callback")
            && object.len() == 3;
        if !exact {
            return Err(AgyAdmissionGateError::NoSupportedAdmittedProfileVersion);
        }
        Ok(AgyAdmittedProfile {
            version: AgyVersion::from_parts(1, 1, 19),
            source: AgyBackendSource::TitleCallback,
        })
    }

    /// Projects normalized data only when an unforgeable admitted token exists.
    #[must_use]
    pub const fn project(
        profile: &AgyAdmittedProfile,
        observation: AgyNormalizedObservation,
    ) -> AgyEvidenceProjection {
        if matches!(profile.source, AgyBackendSource::TitleCallback)
            && matches!(observation.state, AgyBackendState::Admitted)
            && matches!(observation.source, AgyBackendSource::TitleCallback)
            && observation.production_authority
        {
            AgyEvidenceProjection::Evidence
        } else {
            AgyEvidenceProjection::Unadmitted
        }
    }
}

/// G64-frozen capability declaration for the title callback backend.
#[must_use]
pub const fn agy_backend_capabilities() -> BackendCapabilities {
    BackendCapabilities::new(
        AuthoritySet::LIFECYCLE,
        AuthoritySet::NONE,
        AuthoritySet::NONE,
    )
}

/// A content-minimal admitted title observation.
pub struct AgyTitleObservation {
    evidence: AgentEvidence,
    project_root: PathBuf,
    session_sha256: String,
    event_sequence: u64,
    phase: Phase,
}

impl fmt::Debug for AgyTitleObservation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgyTitleObservation")
            .field("phase", &self.phase)
            .field("root_binding_stable", &true)
            .finish_non_exhaustive()
    }
}

impl AgyTitleObservation {
    /// Returns normalized evidence without exposing the native session value.
    #[must_use]
    pub const fn evidence(&self) -> &AgentEvidence {
        &self.evidence
    }
}

/// Exact title-callback normalizer for the frozen Agy profile.
pub struct AgyTitleNormalizer;

impl AgyTitleNormalizer {
    /// Converts only the G64-proven payload subset into lifecycle evidence.
    ///
    /// Payload content, email, model, transcript, quota, tool names, and prompt
    /// fields are never copied from the parsed object.
    #[must_use]
    pub fn normalize(
        profile: AgyAdmittedProfile,
        raw: &[u8],
        observed_at: SystemTime,
    ) -> Option<AgyTitleObservation> {
        if profile.version.as_string() != AGY_ADMITTED_VERSION
            || profile.source != AgyBackendSource::TitleCallback
        {
            return None;
        }
        if raw.len() > MAX_AGY_QUALIFICATION_INPUT_BYTES {
            return None;
        }
        let object = parse_qualification_object(raw)?;
        if object.get("version").and_then(Value::as_str) != Some(AGY_ADMITTED_VERSION) {
            return None;
        }
        let phase = match object.get("agent_state").and_then(Value::as_str) {
            Some("idle") => Phase::Ready,
            Some("initializing" | "working") => Phase::Working,
            Some(_) | None => return None,
        };
        let native_session = object
            .get("conversation_id")
            .or_else(|| object.get("session_id"))?
            .as_str()?;
        if native_session.is_empty() || native_session.len() > 512 {
            return None;
        }
        let workspace = object.get("workspace")?.as_object()?;
        let current_root = workspace.get("current_dir")?.as_str()?;
        let project_root = workspace.get("project_dir")?.as_str()?;
        if current_root.is_empty() || current_root != project_root || project_root.len() > 32_768 {
            return None;
        }

        let session_sha256 = sha256_hex(native_session.as_bytes());
        let event_sequence = observed_at
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            });
        let provider = AgentProvider::new(AGY_PROVIDER_ID).ok()?;
        let session = AgentSessionKey::new(provider, session_sha256.clone()).ok()?;
        let source = EvidenceSource::new("agy-title-callback", AGY_ADMITTED_VERSION).ok()?;
        let tie_break = EvidenceTieBreak::new(format!("title-{event_sequence:020}")).ok()?;
        let evidence = AgentEvidence::new(
            session,
            source,
            EvidenceAuthority::Lifecycle,
            EvidenceConfidence::Standard,
            observed_at,
            tie_break,
            StatePatch {
                phase: FieldUpdate::Set(phase),
                attention: FieldUpdate::Unchanged,
                health: FieldUpdate::Unchanged,
            },
        );
        Some(AgyTitleObservation {
            evidence,
            project_root: PathBuf::from(project_root),
            session_sha256,
            event_sequence,
            phase,
        })
    }
}

/// Fail-open disposition of one production title callback.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyTitleDispatchOutcome {
    Applied,
    DegradedInput,
    DegradedWorkspaceIdentity,
    DegradedRootWorkspaceAnchor,
    DegradedStateRoot,
}

const AGY_ROOT_ANCHOR_SCHEMA: &str = "tabbeacon-agy-root-workspace-anchor-v1";
const AGY_ROOT_ANCHOR_DIRECTORY: &str = "agy-root-workspace-anchor-v1";
const AGY_ROOT_ANCHOR_LOCK: &str = "root-workspace-anchor.lock";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AgyPersistedRootAnchor {
    schema: String,
    provider: String,
    session_sha256: String,
    workspace_identity_sha256: String,
    workspace_location_sha256: String,
    effective_alias: String,
    workspace_mismatch_observed: bool,
    updated_unix_seconds: u64,
}

#[derive(Clone, Debug)]
struct AgyRootSelection {
    effective_alias: RepositoryAlias,
    workspace_mismatch_observed: bool,
}

#[derive(Clone, Debug)]
struct AgyRootAnchorStore {
    directory: PathBuf,
}

impl AgyRootAnchorStore {
    fn new(state_root: &Path) -> Self {
        Self {
            directory: state_root.join(AGY_ROOT_ANCHOR_DIRECTORY),
        }
    }

    fn select_existing(
        &self,
        session_sha256: &str,
        workspace_location_sha256: &str,
        observed_unix_seconds: u64,
    ) -> io::Result<Option<AgyRootSelection>> {
        self.with_anchor(session_sha256, |anchor| {
            let Some(mut anchor) = anchor else {
                return Ok((None, None));
            };
            validate_agy_root_anchor(&anchor, session_sha256)?;
            anchor.workspace_mismatch_observed |=
                anchor.workspace_location_sha256 != workspace_location_sha256;
            anchor.updated_unix_seconds = observed_unix_seconds;
            let selection = agy_root_selection(&anchor)?;
            Ok((Some(anchor), Some(selection)))
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_if_absent(
        &self,
        session_sha256: &str,
        workspace_identity_sha256: &str,
        workspace_location_sha256: &str,
        effective_alias: &RepositoryAlias,
        observed_unix_seconds: u64,
    ) -> io::Result<AgyRootSelection> {
        self.with_anchor(session_sha256, |anchor| {
            if let Some(anchor) = anchor {
                validate_agy_root_anchor(&anchor, session_sha256)?;
                let selection = agy_root_selection(&anchor)?;
                return Ok((Some(anchor), Some(selection)));
            }
            let anchor = AgyPersistedRootAnchor {
                schema: AGY_ROOT_ANCHOR_SCHEMA.to_owned(),
                provider: AGY_PROVIDER_ID.to_owned(),
                session_sha256: session_sha256.to_owned(),
                workspace_identity_sha256: workspace_identity_sha256.to_owned(),
                workspace_location_sha256: workspace_location_sha256.to_owned(),
                effective_alias: effective_alias.as_str().to_owned(),
                workspace_mismatch_observed: false,
                updated_unix_seconds: observed_unix_seconds,
            };
            validate_agy_root_anchor(&anchor, session_sha256)?;
            let selection = agy_root_selection(&anchor)?;
            Ok((Some(anchor), Some(selection)))
        })?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Agy root anchor was not stored"))
    }

    fn with_anchor<T>(
        &self,
        session_sha256: &str,
        operation: impl FnOnce(
            Option<AgyPersistedRootAnchor>,
        ) -> io::Result<(Option<AgyPersistedRootAnchor>, T)>,
    ) -> io::Result<T> {
        if !is_sha256(session_sha256) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid Agy session digest",
            ));
        }
        ensure_safe_directory_path(&self.directory).map_err(io::Error::other)?;
        fs::create_dir_all(&self.directory)?;
        ensure_safe_directory_path(&self.directory).map_err(io::Error::other)?;
        let lock_path = self.directory.join(AGY_ROOT_ANCHOR_LOCK);
        ensure_safe_file_path(&lock_path).map_err(io::Error::other)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)?;
        lock.lock()?;
        let path = self
            .directory
            .join(format!("session-{session_sha256}.json"));
        ensure_safe_file_path(&path).map_err(io::Error::other)?;
        let current = match fs::read(&path) {
            Ok(bytes) if bytes.len() <= 4_096 => Some(
                serde_json::from_slice(&bytes)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
            ),
            Ok(_) => {
                let _ = File::unlock(&lock);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "oversized Agy root anchor",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => {
                let _ = File::unlock(&lock);
                return Err(error);
            }
        };
        let result = operation(current).and_then(|(updated, result)| {
            if let Some(updated) = updated {
                let mut bytes = serde_json::to_vec_pretty(&updated)
                    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
                bytes.push(b'\n');
                atomic_write(&path, &bytes).map_err(io::Error::other)?;
            }
            Ok(result)
        });
        let unlock = File::unlock(&lock);
        match (result, unlock) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        }
    }
}

fn agy_root_selection(anchor: &AgyPersistedRootAnchor) -> io::Result<AgyRootSelection> {
    Ok(AgyRootSelection {
        effective_alias: RepositoryAlias::new(anchor.effective_alias.clone())
            .map_err(io::Error::other)?,
        workspace_mismatch_observed: anchor.workspace_mismatch_observed,
    })
}

fn validate_agy_root_anchor(
    anchor: &AgyPersistedRootAnchor,
    expected_session_sha256: &str,
) -> io::Result<()> {
    if anchor.schema != AGY_ROOT_ANCHOR_SCHEMA
        || anchor.provider != AGY_PROVIDER_ID
        || anchor.session_sha256 != expected_session_sha256
        || !is_sha256(&anchor.session_sha256)
        || !is_sha256(&anchor.workspace_identity_sha256)
        || !is_sha256(&anchor.workspace_location_sha256)
        || RepositoryAlias::new(anchor.effective_alias.clone()).is_err()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid Agy root anchor",
        ));
    }
    Ok(())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Plain title callback result. No terminal protocol bytes are emitted here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgyProductionTitleResponse {
    pub title: String,
    pub outcome: AgyTitleDispatchOutcome,
}

/// One-shot Agy title callback through normalizer, core, root anchor, and policy.
#[derive(Debug, Clone)]
pub struct AgyTitleRuntime {
    profile: AgyAdmittedProfile,
    state_root: PathBuf,
    identity_resolver: WorkspaceIdentityResolver,
    root_anchors: AgyRootAnchorStore,
    renderer: WindowsTerminalRenderer,
}

impl AgyTitleRuntime {
    /// Creates a deterministic runtime for focused tests.
    #[must_use]
    pub fn new(state_root: impl Into<PathBuf>, settings: PresentationSettings) -> Self {
        let state_root = state_root.into();
        Self {
            profile: frozen_profile(),
            state_root: state_root.clone(),
            identity_resolver: WorkspaceIdentityResolver::new(&state_root),
            root_anchors: AgyRootAnchorStore::new(&state_root),
            renderer: WindowsTerminalRenderer::with_settings(
                WindowsTerminalCapabilities::new(false),
                settings,
            ),
        }
    }

    /// Handles a production callback without ever propagating failure to Agy.
    #[must_use]
    pub fn dispatch_system(raw: &[u8]) -> AgyProductionTitleResponse {
        let Ok(state_root) = StableAliasRegistry::default_state_root() else {
            return fallback_title(AgyTitleDispatchOutcome::DegradedStateRoot);
        };
        let settings = PresentationSettingsStore::from_environment().map_or_else(
            |_| PresentationSettings::default(),
            |store| store.load_or_default(),
        );
        let runtime = Self::new(&state_root, settings);
        runtime.dispatch_to(raw, SystemTime::now())
    }

    /// Handles a callback with deterministic runtime dependencies.
    #[must_use]
    pub fn dispatch_to(&self, raw: &[u8], observed_at: SystemTime) -> AgyProductionTitleResponse {
        let Some(normalized) = AgyTitleNormalizer::normalize(self.profile, raw, observed_at) else {
            record_callback_diagnostics(&self.state_root, raw, false, observed_at);
            return fallback_title(AgyTitleDispatchOutcome::DegradedInput);
        };
        let Ok(workspace_location_sha256) =
            WorkspaceIdentityResolver::fast_workspace_location_sha256(&normalized.project_root)
        else {
            record_callback_diagnostics(&self.state_root, raw, false, observed_at);
            return fallback_title(AgyTitleDispatchOutcome::DegradedWorkspaceIdentity);
        };
        let observed_unix_seconds = normalized.event_sequence / 1_000;
        let root_selection = match self.root_anchors.select_existing(
            &normalized.session_sha256,
            &workspace_location_sha256,
            observed_unix_seconds,
        ) {
            Ok(Some(selection)) => selection,
            Ok(None) => {
                let Ok(workspace) = self.identity_resolver.resolve(&normalized.project_root) else {
                    record_callback_diagnostics(&self.state_root, raw, false, observed_at);
                    return fallback_title(AgyTitleDispatchOutcome::DegradedWorkspaceIdentity);
                };
                let workspace_identity_sha256 = sha256_hex(workspace.identity.as_str().as_bytes());
                let Ok(selection) = self.root_anchors.bind_if_absent(
                    &normalized.session_sha256,
                    &workspace_identity_sha256,
                    &workspace_location_sha256,
                    &workspace.effective_alias,
                    observed_unix_seconds,
                ) else {
                    record_callback_diagnostics(&self.state_root, raw, false, observed_at);
                    return fallback_title(AgyTitleDispatchOutcome::DegradedRootWorkspaceAnchor);
                };
                selection
            }
            Err(_) => {
                record_callback_diagnostics(&self.state_root, raw, false, observed_at);
                return fallback_title(AgyTitleDispatchOutcome::DegradedRootWorkspaceAnchor);
            }
        };
        let mut reconciler = SessionReconciler::default();
        let snapshot = reconciler.apply(normalized.evidence());
        let registry = ProviderRegistry::codex_observation(None, true, false, false)
            .with_agy_readiness(AgyReadinessProjection {
                state: AgyIntegrationReadiness::SupportedConfigured,
                version: Some(AGY_ADMITTED_VERSION.to_owned()),
                qualification_available: true,
                qualification_observations_available: true,
                production_enabled: true,
            });
        let provider_badge =
            registry.title_badge_for(AGY_PROVIDER_ID, self.renderer.settings().provider_badge());
        let action = PresentationPolicy::resolve(
            SemanticPresentationInput::from_snapshot_with_provider_badge(
                &snapshot,
                root_selection.effective_alias.as_str(),
                provider_badge.as_deref(),
            ),
        );
        let state = match &action {
            PresentationAction::Apply(state) | PresentationAction::Reset(state) => state,
        };
        let title = self
            .renderer
            .title_for(state)
            .map_or_else(|| "Agy".to_owned(), |title| title.as_str().to_owned());
        let workspace_observability = SessionWorkspaceObservability {
            root_binding_stable: true,
            workspace_mismatch_observed: root_selection.workspace_mismatch_observed,
            active_subagents: 0,
            background_tasks: None,
        };
        let _ = record_provider_session_observation(
            &self.state_root,
            &normalized.session_sha256,
            AGY_PROVIDER_ID,
            root_selection.effective_alias.as_str(),
            match normalized.phase {
                Phase::Ready => "ready",
                Phase::Working => "working",
                _ => "unavailable",
            },
            normalized.event_sequence,
            workspace_observability.clone(),
        );
        record_callback_diagnostics(&self.state_root, raw, true, observed_at);
        AgyProductionTitleResponse {
            title,
            outcome: AgyTitleDispatchOutcome::Applied,
        }
    }
}

fn fallback_title(outcome: AgyTitleDispatchOutcome) -> AgyProductionTitleResponse {
    AgyProductionTitleResponse {
        title: "Agy".to_owned(),
        outcome,
    }
}

const fn frozen_profile() -> AgyAdmittedProfile {
    AgyAdmittedProfile {
        version: AgyVersion::from_parts(1, 1, 19),
        source: AgyBackendSource::TitleCallback,
    }
}

fn record_callback_diagnostics(
    state_root: &Path,
    raw: &[u8],
    production_projection: bool,
    observed_at: SystemTime,
) {
    let observed_unix_ms = observed_at
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| {
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
        });
    let document = json!({
        "schema": "tabbeacon-agy-callback-diagnostics-v1",
        "production_projection": production_projection,
        "observed_unix_ms": observed_unix_ms,
        "content_minimized": true,
        "record": AgyStateRecorder::record(raw),
    });
    let _ = atomic_write_json(
        &state_root
            .join("agy-callback-v1")
            .join("last-observation.json"),
        &document,
    );
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = std::fmt::Write::write_fmt(&mut output, format_args!("{byte:02x}"));
    }
    output
}

/// Production adapter object bound to the exact frozen profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgyBackend {
    pub state: AgyBackendState,
    pub source: AgyBackendSource,
}

impl Default for AgyBackend {
    fn default() -> Self {
        Self {
            state: AgyBackendState::Admitted,
            source: AgyBackendSource::TitleCallback,
        }
    }
}

impl AgyBackend {
    /// Agy participates only through the exact admitted profile.
    #[must_use]
    pub const fn provider_enabled(self) -> bool {
        AGY_PROVIDER
    }

    /// The runtime remains fail open because unsupported observations project no evidence.
    #[must_use]
    pub const fn observe_fail_open(
        self,
        observation: AgyNormalizedObservation,
    ) -> AgyEvidenceProjection {
        if matches!(observation.state, AgyBackendState::Admitted)
            && matches!(observation.source, AgyBackendSource::TitleCallback)
            && observation.production_authority
        {
            AgyEvidenceProjection::Evidence
        } else {
            AgyEvidenceProjection::Unadmitted
        }
    }
}

/// Root Workspace Anchor boundary for pre-admission and admitted observations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyRootAnchorBoundary {
    Unadmitted,
    StableProjectRoot,
}

/// Typed Agy management states.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyIntegrationReadiness {
    KnownUnadmitted,
    SupportedConfigured,
    SupportedNotConfigured,
    UnsupportedVersion,
    ConfigurationDrift,
}

/// Current truthful Agy Doctor/Integrations projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyReadinessProjection {
    pub state: AgyIntegrationReadiness,
    pub version: Option<String>,
    pub qualification_available: bool,
    pub qualification_observations_available: bool,
    pub production_enabled: bool,
}

impl AgyReadinessProjection {
    /// Builds today's unadmitted readiness state.
    #[must_use]
    pub fn unadmitted(qualification_observations_available: bool) -> Self {
        Self {
            state: AgyIntegrationReadiness::KnownUnadmitted,
            version: None,
            qualification_available: true,
            qualification_observations_available,
            production_enabled: false,
        }
    }
}

/// Ownership-safe production setup failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyProductionSetupError {
    EnvironmentUnavailable,
    ProbeUnavailable,
    UnsupportedVersion,
    MalformedConfiguration,
    ForeignTitleOwner,
    ConfigurationDrift,
    OwnershipStateInvalid,
    UnrepresentableCallbackCommand,
    Io,
}

impl fmt::Display for AgyProductionSetupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EnvironmentUnavailable => "the user-global Agy environment is unavailable",
            Self::ProbeUnavailable => "literal agy --version could not be verified",
            Self::UnsupportedVersion => "the installed Agy version is not admitted",
            Self::MalformedConfiguration => "the user-global Agy configuration is malformed",
            Self::ForeignTitleOwner => "the Agy title callback is owned by another configuration",
            Self::ConfigurationDrift => "Agy configuration drift prevents an ownership-safe change",
            Self::OwnershipStateInvalid => "TabBeacon Agy ownership state is invalid",
            Self::UnrepresentableCallbackCommand => {
                "the TabBeacon executable path cannot be represented safely in Agy's title command"
            }
            Self::Io => "an Agy setup filesystem operation failed",
        })
    }
}

impl std::error::Error for AgyProductionSetupError {}

/// Result of an ownership-safe production setup operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyProductionSetupOutcome {
    Installed,
    AlreadyConfigured,
    Removed,
    NotInstalled,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AgySetupManifest {
    schema: String,
    admitted_version: String,
    original_present: bool,
    original_sha256: String,
    applied_sha256: String,
    executable_sha256: String,
    callback_sha256: String,
}

const AGY_SETUP_MANIFEST_SCHEMA_V1: &str = "tabbeacon-agy-setup-v1";
const AGY_SETUP_MANIFEST_SCHEMA: &str = "tabbeacon-agy-setup-v2";

/// Production user-global setup with injectable paths for focused tests.
#[derive(Clone, Debug)]
pub struct AgyProductionSetup {
    config_path: PathBuf,
    state_root: PathBuf,
    executable: PathBuf,
    agy_program: PathBuf,
    #[cfg(test)]
    version_override: Option<AgyVersion>,
}

impl AgyProductionSetup {
    /// Discovers only the official user-global Agy settings surface.
    ///
    /// # Errors
    ///
    /// Returns a safe error when the per-user roots or current executable are unavailable.
    pub fn from_environment() -> Result<Self, AgyProductionSetupError> {
        let home = platform_home().ok_or(AgyProductionSetupError::EnvironmentUnavailable)?;
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| platform_home().map(|path| path.join(".local").join("state")))
            .ok_or(AgyProductionSetupError::EnvironmentUnavailable)?;
        let executable = std::env::current_exe().map_err(|_| AgyProductionSetupError::Io)?;
        Ok(Self::new(
            home.join(".gemini")
                .join("antigravity-cli")
                .join("settings.json"),
            local_app_data.join("TabBeacon").join("agy"),
            executable,
            PathBuf::from(AGY_DAILY_COMMAND),
        ))
    }

    /// Creates a setup facade with explicit test/embedding dependencies.
    #[must_use]
    pub fn new(
        config_path: impl Into<PathBuf>,
        state_root: impl Into<PathBuf>,
        executable: impl Into<PathBuf>,
        agy_program: impl Into<PathBuf>,
    ) -> Self {
        Self {
            config_path: config_path.into(),
            state_root: state_root.into(),
            executable: executable.into(),
            agy_program: agy_program.into(),
            #[cfg(test)]
            version_override: None,
        }
    }

    #[cfg(test)]
    fn with_admitted_version(mut self) -> Self {
        self.version_override = AgyVersion::parse(AGY_ADMITTED_VERSION);
        self
    }

    /// Read-only current compatibility and ownership projection.
    #[must_use]
    pub fn inspect(&self) -> AgyReadinessProjection {
        if self.validate_paths().is_err() {
            return readiness(AgyIntegrationReadiness::ConfigurationDrift, None, false);
        }
        let Ok(version) = self.probe_version() else {
            return readiness(AgyIntegrationReadiness::KnownUnadmitted, None, false);
        };
        let version = version.as_string();
        if version != AGY_ADMITTED_VERSION {
            return readiness(
                AgyIntegrationReadiness::UnsupportedVersion,
                Some(version),
                false,
            );
        }
        let manifest = self.read_manifest();
        match manifest {
            Ok(None) => readiness(
                AgyIntegrationReadiness::SupportedNotConfigured,
                Some(version),
                false,
            ),
            Ok(Some(manifest)) => match self.current_matches_owned(&manifest) {
                Ok(true) if manifest.schema == AGY_SETUP_MANIFEST_SCHEMA => readiness(
                    AgyIntegrationReadiness::SupportedConfigured,
                    Some(version),
                    true,
                ),
                Ok(true | false) | Err(_) => readiness(
                    AgyIntegrationReadiness::ConfigurationDrift,
                    Some(version),
                    false,
                ),
            },
            Err(_) => readiness(
                AgyIntegrationReadiness::ConfigurationDrift,
                Some(version),
                false,
            ),
        }
    }

    /// Installs the smallest supported user-global title callback mutation.
    ///
    /// # Errors
    ///
    /// Refuses unsupported versions, foreign title owners, malformed input,
    /// ownership-state drift, and any pre-write byte drift.
    pub fn setup(&self) -> Result<AgyProductionSetupOutcome, AgyProductionSetupError> {
        self.require_admitted_version()?;
        self.validate_paths()?;
        let _lock = SetupLock::acquire(&self.state_root)?;
        self.validate_paths()?;
        if let Some(manifest) = self.read_manifest()? {
            let current = fs::read(&self.config_path)
                .map_err(|_| AgyProductionSetupError::ConfigurationDrift)?;
            if self
                .owned_current_without_title(&manifest, &current, false)?
                .is_none()
            {
                return Err(AgyProductionSetupError::ConfigurationDrift);
            }
            return if manifest.schema == AGY_SETUP_MANIFEST_SCHEMA_V1 {
                self.upgrade_owned_manifest(&manifest)
            } else if self.executable_matches_manifest(&manifest)? {
                Ok(AgyProductionSetupOutcome::AlreadyConfigured)
            } else {
                self.refresh_owned_executable(&manifest)
            };
        }

        let before = read_optional_bytes(&self.config_path)?;
        let mut object = match before.as_deref() {
            Some(bytes) => parse_qualification_object(bytes)
                .ok_or(AgyProductionSetupError::MalformedConfiguration)?,
            None => Map::new(),
        };
        if object.contains_key("title") {
            return Err(AgyProductionSetupError::ForeignTitleOwner);
        }
        let callback = self.callback_value()?;
        object.insert("title".to_owned(), callback.clone());
        let mut candidate = serde_json::to_vec_pretty(&Value::Object(object))
            .map_err(|_| AgyProductionSetupError::MalformedConfiguration)?;
        candidate.push(b'\n');

        let backup = before.clone().unwrap_or_default();
        let manifest = AgySetupManifest {
            schema: AGY_SETUP_MANIFEST_SCHEMA.to_owned(),
            admitted_version: AGY_ADMITTED_VERSION.to_owned(),
            original_present: before.is_some(),
            original_sha256: sha256_hex(&backup),
            applied_sha256: sha256_hex(&candidate),
            executable_sha256: sha256_hex(
                &fs::read(&self.executable).map_err(|_| AgyProductionSetupError::Io)?,
            ),
            callback_sha256: sha256_hex(
                &serde_json::to_vec(&callback)
                    .map_err(|_| AgyProductionSetupError::MalformedConfiguration)?,
            ),
        };

        fs::create_dir_all(&self.state_root).map_err(|_| AgyProductionSetupError::Io)?;
        atomic_write(&self.backup_path(), &backup)?;
        atomic_write_json(&self.manifest_path(), &manifest)?;
        write_optional_if_unchanged(&self.config_path, before.as_deref(), &candidate)?;
        if fs::read(&self.config_path).map_err(|_| AgyProductionSetupError::Io)? != candidate {
            return Err(AgyProductionSetupError::ConfigurationDrift);
        }
        Ok(AgyProductionSetupOutcome::Installed)
    }

    /// Removes exactly the owned title declaration.
    ///
    /// The original bytes are restored when unrelated settings are unchanged.
    /// Otherwise the current unrelated settings are preserved semantically and
    /// only the owned title member is removed.
    ///
    /// # Errors
    ///
    /// Refuses when the callback or ownership state drifted.
    pub fn uninstall(&self) -> Result<AgyProductionSetupOutcome, AgyProductionSetupError> {
        self.validate_paths()?;
        let _lock = SetupLock::acquire(&self.state_root)?;
        self.validate_paths()?;
        let Some(manifest) = self.read_manifest()? else {
            return Ok(AgyProductionSetupOutcome::NotInstalled);
        };
        let current =
            fs::read(&self.config_path).map_err(|_| AgyProductionSetupError::ConfigurationDrift)?;
        let current_without_title = self
            .owned_current_without_title(&manifest, &current, true)?
            .ok_or(AgyProductionSetupError::ConfigurationDrift)?;
        let backup = fs::read(self.backup_path())
            .map_err(|_| AgyProductionSetupError::OwnershipStateInvalid)?;
        if sha256_hex(&backup) != manifest.original_sha256 {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        let original_without_title = if manifest.original_present {
            parse_qualification_object(&backup)
                .ok_or(AgyProductionSetupError::OwnershipStateInvalid)?
        } else {
            Map::new()
        };
        let desired = if current_without_title == original_without_title {
            manifest.original_present.then_some(backup)
        } else {
            let mut bytes = serde_json::to_vec_pretty(&Value::Object(current_without_title))
                .map_err(|_| AgyProductionSetupError::MalformedConfiguration)?;
            bytes.push(b'\n');
            Some(bytes)
        };
        if let Some(bytes) = desired.as_deref() {
            write_if_unchanged(&self.config_path, &current, bytes)?;
        } else {
            remove_if_unchanged(&self.config_path, &current)?;
        }
        if read_optional_bytes(&self.config_path)? != desired {
            return Err(AgyProductionSetupError::ConfigurationDrift);
        }
        fs::remove_file(self.manifest_path()).map_err(|_| AgyProductionSetupError::Io)?;
        fs::remove_file(self.backup_path()).map_err(|_| AgyProductionSetupError::Io)?;
        Ok(AgyProductionSetupOutcome::Removed)
    }

    fn require_admitted_version(&self) -> Result<(), AgyProductionSetupError> {
        let version = self.probe_version()?;
        if version.as_string() == AGY_ADMITTED_VERSION {
            Ok(())
        } else {
            Err(AgyProductionSetupError::UnsupportedVersion)
        }
    }

    fn probe_version(&self) -> Result<AgyVersion, AgyProductionSetupError> {
        #[cfg(test)]
        if let Some(version) = self.version_override {
            return Ok(version);
        }
        if self.agy_program == Path::new(AGY_DAILY_COMMAND) {
            return probe_direct_agy_version()
                .version
                .as_deref()
                .and_then(AgyVersion::parse)
                .ok_or(AgyProductionSetupError::ProbeUnavailable);
        }
        let output = Command::new(&self.agy_program)
            .arg("--version")
            .output()
            .map_err(|_| AgyProductionSetupError::ProbeUnavailable)?;
        if !output.status.success() || output.stdout.len() > 128 {
            return Err(AgyProductionSetupError::ProbeUnavailable);
        }
        let value = std::str::from_utf8(&output.stdout)
            .map_err(|_| AgyProductionSetupError::ProbeUnavailable)?
            .trim();
        AgyVersion::parse(value).ok_or(AgyProductionSetupError::ProbeUnavailable)
    }

    fn callback_value(&self) -> Result<Value, AgyProductionSetupError> {
        self.callback_value_for_schema(AGY_SETUP_MANIFEST_SCHEMA)
    }

    fn callback_value_for_schema(&self, schema: &str) -> Result<Value, AgyProductionSetupError> {
        let executable = self
            .executable
            .to_str()
            .ok_or(AgyProductionSetupError::EnvironmentUnavailable)?;
        if schema == AGY_SETUP_MANIFEST_SCHEMA_V1 {
            return Ok(json!({
                "type": "command",
                "command": format!("\"{executable}\" agy __title-callback-v1")
            }));
        }
        if schema != AGY_SETUP_MANIFEST_SCHEMA {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        if executable.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(
                    character,
                    '"' | '&' | '|' | '<' | '>' | '^' | '%' | '!' | '(' | ')'
                )
        }) {
            return Err(AgyProductionSetupError::UnrepresentableCallbackCommand);
        }
        Ok(json!({
            "type": "command",
            "command": format!("{executable} agy __title-callback-v1"),
            "enabled": true
        }))
    }

    fn upgrade_owned_manifest(
        &self,
        manifest: &AgySetupManifest,
    ) -> Result<AgyProductionSetupOutcome, AgyProductionSetupError> {
        if manifest.schema != AGY_SETUP_MANIFEST_SCHEMA_V1 {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        let current =
            fs::read(&self.config_path).map_err(|_| AgyProductionSetupError::ConfigurationDrift)?;
        let mut object = parse_qualification_object(&current)
            .ok_or(AgyProductionSetupError::MalformedConfiguration)?;
        if object.remove("title") != Some(self.callback_value_for_schema(&manifest.schema)?) {
            return Err(AgyProductionSetupError::ConfigurationDrift);
        }
        let callback = self.callback_value()?;
        object.insert("title".to_owned(), callback.clone());
        let mut candidate = serde_json::to_vec_pretty(&Value::Object(object))
            .map_err(|_| AgyProductionSetupError::MalformedConfiguration)?;
        candidate.push(b'\n');

        let backup = fs::read(self.backup_path())
            .map_err(|_| AgyProductionSetupError::OwnershipStateInvalid)?;
        let upgraded = AgySetupManifest {
            schema: AGY_SETUP_MANIFEST_SCHEMA.to_owned(),
            admitted_version: AGY_ADMITTED_VERSION.to_owned(),
            original_present: manifest.original_present,
            original_sha256: manifest.original_sha256.clone(),
            applied_sha256: sha256_hex(&candidate),
            executable_sha256: sha256_hex(
                &fs::read(&self.executable).map_err(|_| AgyProductionSetupError::Io)?,
            ),
            callback_sha256: sha256_hex(
                &serde_json::to_vec(&callback)
                    .map_err(|_| AgyProductionSetupError::MalformedConfiguration)?,
            ),
        };
        if sha256_hex(&backup) != upgraded.original_sha256 {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        write_if_unchanged(&self.config_path, &current, &candidate)?;
        atomic_write_json(&self.manifest_path(), &upgraded)?;
        if !self.current_matches_owned(&upgraded)? {
            return Err(AgyProductionSetupError::ConfigurationDrift);
        }
        Ok(AgyProductionSetupOutcome::Installed)
    }

    fn current_matches_owned(
        &self,
        manifest: &AgySetupManifest,
    ) -> Result<bool, AgyProductionSetupError> {
        let current =
            fs::read(&self.config_path).map_err(|_| AgyProductionSetupError::ConfigurationDrift)?;
        self.bytes_match_owned(manifest, &current)
    }

    fn bytes_match_owned(
        &self,
        manifest: &AgySetupManifest,
        current: &[u8],
    ) -> Result<bool, AgyProductionSetupError> {
        Ok(self
            .owned_current_without_title(manifest, current, true)?
            .is_some())
    }

    fn owned_current_without_title(
        &self,
        manifest: &AgySetupManifest,
        current: &[u8],
        require_current_executable: bool,
    ) -> Result<Option<Map<String, Value>>, AgyProductionSetupError> {
        if (manifest.schema != AGY_SETUP_MANIFEST_SCHEMA
            && manifest.schema != AGY_SETUP_MANIFEST_SCHEMA_V1)
            || manifest.admitted_version != AGY_ADMITTED_VERSION
        {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        let backup = fs::read(self.backup_path())
            .map_err(|_| AgyProductionSetupError::OwnershipStateInvalid)?;
        if sha256_hex(&backup) != manifest.original_sha256 {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        if manifest.executable_sha256.len() != 64
            || !manifest
                .executable_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        if require_current_executable
            && manifest.schema == AGY_SETUP_MANIFEST_SCHEMA
            && !self.executable_matches_manifest(manifest)?
        {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        let mut current_object = parse_qualification_object(current)
            .ok_or(AgyProductionSetupError::MalformedConfiguration)?;
        let expected_callback = self.callback_value_for_schema(&manifest.schema)?;
        if current_object.remove("title") != Some(expected_callback.clone()) {
            return Ok(None);
        }
        if sha256_hex(
            &serde_json::to_vec(&expected_callback)
                .map_err(|_| AgyProductionSetupError::MalformedConfiguration)?,
        ) != manifest.callback_sha256
        {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        let mut applied_object = if manifest.original_present {
            parse_qualification_object(&backup)
                .ok_or(AgyProductionSetupError::OwnershipStateInvalid)?
        } else {
            Map::new()
        };
        if applied_object.contains_key("title") {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        applied_object.insert("title".to_owned(), expected_callback);
        let mut applied_bytes = serde_json::to_vec_pretty(&Value::Object(applied_object))
            .map_err(|_| AgyProductionSetupError::OwnershipStateInvalid)?;
        applied_bytes.push(b'\n');
        if sha256_hex(&applied_bytes) != manifest.applied_sha256 {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        Ok(Some(current_object))
    }

    fn executable_matches_manifest(
        &self,
        manifest: &AgySetupManifest,
    ) -> Result<bool, AgyProductionSetupError> {
        Ok(sha256_hex(
            &fs::read(&self.executable)
                .map_err(|_| AgyProductionSetupError::OwnershipStateInvalid)?,
        ) == manifest.executable_sha256)
    }

    fn refresh_owned_executable(
        &self,
        manifest: &AgySetupManifest,
    ) -> Result<AgyProductionSetupOutcome, AgyProductionSetupError> {
        if manifest.schema != AGY_SETUP_MANIFEST_SCHEMA {
            return Err(AgyProductionSetupError::OwnershipStateInvalid);
        }
        let mut refreshed = manifest.clone();
        refreshed.executable_sha256 =
            sha256_hex(&fs::read(&self.executable).map_err(|_| AgyProductionSetupError::Io)?);
        atomic_write_json(&self.manifest_path(), &refreshed)?;
        if !self.current_matches_owned(&refreshed)? {
            return Err(AgyProductionSetupError::ConfigurationDrift);
        }
        Ok(AgyProductionSetupOutcome::Installed)
    }

    fn validate_paths(&self) -> Result<(), AgyProductionSetupError> {
        ensure_safe_file_path(&self.config_path)?;
        ensure_safe_directory_path(&self.state_root)?;
        ensure_safe_file_path(&self.manifest_path())?;
        ensure_safe_file_path(&self.backup_path())?;
        ensure_safe_file_path(&self.executable)
    }

    fn read_manifest(&self) -> Result<Option<AgySetupManifest>, AgyProductionSetupError> {
        let Some(bytes) = read_optional_bytes(&self.manifest_path())? else {
            return Ok(None);
        };
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|_| AgyProductionSetupError::OwnershipStateInvalid)
    }

    fn manifest_path(&self) -> PathBuf {
        self.state_root.join("setup.json")
    }

    fn backup_path(&self) -> PathBuf {
        self.state_root.join("original-settings.bin")
    }
}

fn readiness(
    state: AgyIntegrationReadiness,
    version: Option<String>,
    production_enabled: bool,
) -> AgyReadinessProjection {
    AgyReadinessProjection {
        state,
        version,
        qualification_available: true,
        qualification_observations_available: true,
        production_enabled,
    }
}

fn platform_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .filter(|value| !value.is_empty())
        .or_else(|| std::env::var_os("HOME").filter(|value| !value.is_empty()))
        .map(PathBuf::from)
}

struct SetupLock {
    path: PathBuf,
}

impl SetupLock {
    fn acquire(root: &Path) -> Result<Self, AgyProductionSetupError> {
        ensure_safe_directory_path(root)?;
        fs::create_dir_all(root).map_err(|_| AgyProductionSetupError::Io)?;
        ensure_safe_directory_path(root)?;
        let path = root.join("transaction.lock");
        ensure_safe_file_path(&path)?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| AgyProductionSetupError::ConfigurationDrift)?;
        Ok(Self { path })
    }
}

impl Drop for SetupLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn read_optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, AgyProductionSetupError> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(AgyProductionSetupError::Io),
    }
}

fn ensure_safe_file_path(path: &Path) -> Result<(), AgyProductionSetupError> {
    let parent = path
        .parent()
        .ok_or(AgyProductionSetupError::ConfigurationDrift)?;
    ensure_safe_ancestors(parent)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_file() || metadata_is_link_or_reparse(&metadata) => {
            Err(AgyProductionSetupError::ConfigurationDrift)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(AgyProductionSetupError::Io),
    }
}

fn ensure_safe_directory_path(path: &Path) -> Result<(), AgyProductionSetupError> {
    ensure_safe_ancestors(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.is_dir() || metadata_is_link_or_reparse(&metadata) => {
            Err(AgyProductionSetupError::ConfigurationDrift)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(AgyProductionSetupError::Io),
    }
}

fn ensure_safe_ancestors(path: &Path) -> Result<(), AgyProductionSetupError> {
    if !path.is_absolute() {
        return Err(AgyProductionSetupError::ConfigurationDrift);
    }
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if !metadata.is_dir() || metadata_is_link_or_reparse(&metadata) => {
                return Err(AgyProductionSetupError::ConfigurationDrift);
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(AgyProductionSetupError::Io),
        }
    }
    Ok(())
}

fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;

        metadata.file_type().is_symlink() || metadata.file_attributes() & 0x0400 != 0
    }
    #[cfg(not(windows))]
    metadata.file_type().is_symlink()
}

fn write_optional_if_unchanged(
    path: &Path,
    expected_before: Option<&[u8]>,
    replacement: &[u8],
) -> Result<(), AgyProductionSetupError> {
    if let Some(expected) = expected_before {
        write_if_unchanged(path, expected, replacement)
    } else {
        let parent = path
            .parent()
            .ok_or(AgyProductionSetupError::ConfigurationDrift)?;
        fs::create_dir_all(parent).map_err(|_| AgyProductionSetupError::Io)?;
        ensure_safe_ancestors(parent)?;
        ensure_safe_file_path(path)?;
        let mut target = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| AgyProductionSetupError::ConfigurationDrift)?;
        target
            .write_all(replacement)
            .and_then(|()| target.flush())
            .map_err(|_| AgyProductionSetupError::Io)
    }
}

fn write_if_unchanged(
    path: &Path,
    expected_before: &[u8],
    replacement: &[u8],
) -> Result<(), AgyProductionSetupError> {
    ensure_safe_file_path(path)?;
    let mut target = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_| AgyProductionSetupError::ConfigurationDrift)?;
    target
        .lock()
        .map_err(|_| AgyProductionSetupError::ConfigurationDrift)?;
    let result = (|| {
        target
            .seek(SeekFrom::Start(0))
            .map_err(|_| AgyProductionSetupError::Io)?;
        let mut actual_before = Vec::new();
        target
            .read_to_end(&mut actual_before)
            .map_err(|_| AgyProductionSetupError::Io)?;
        if actual_before != expected_before {
            return Err(AgyProductionSetupError::ConfigurationDrift);
        }
        atomic_write(path, replacement)
    })();
    let unlock = File::unlock(&target).map_err(|_| AgyProductionSetupError::Io);
    result?;
    unlock
}

fn remove_if_unchanged(path: &Path, expected_before: &[u8]) -> Result<(), AgyProductionSetupError> {
    ensure_safe_file_path(path)?;
    let mut target = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_| AgyProductionSetupError::ConfigurationDrift)?;
    target
        .lock()
        .map_err(|_| AgyProductionSetupError::ConfigurationDrift)?;
    target
        .seek(SeekFrom::Start(0))
        .map_err(|_| AgyProductionSetupError::Io)?;
    let mut actual_before = Vec::new();
    target
        .read_to_end(&mut actual_before)
        .map_err(|_| AgyProductionSetupError::Io)?;
    if actual_before != expected_before {
        let _ = File::unlock(&target);
        return Err(AgyProductionSetupError::ConfigurationDrift);
    }
    let remove = fs::remove_file(path).map_err(|_| AgyProductionSetupError::Io);
    let unlock = File::unlock(&target).map_err(|_| AgyProductionSetupError::Io);
    remove?;
    unlock
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), AgyProductionSetupError> {
    ensure_safe_file_path(path)?;
    let parent = path.parent().ok_or(AgyProductionSetupError::Io)?;
    fs::create_dir_all(parent).map_err(|_| AgyProductionSetupError::Io)?;
    let mut file = AtomicWriteFile::options()
        .open(path)
        .map_err(|_| AgyProductionSetupError::Io)?;
    file.write_all(bytes)
        .map_err(|_| AgyProductionSetupError::Io)?;
    file.flush().map_err(|_| AgyProductionSetupError::Io)?;
    file.commit().map_err(|_| AgyProductionSetupError::Io)
}

fn atomic_write_json(path: &Path, value: &impl Serialize) -> Result<(), AgyProductionSetupError> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|_| AgyProductionSetupError::Io)?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}

/// Generic fixture patch used to prove ownership algorithms without guessing Agy schema.
#[derive(Clone, Eq, PartialEq)]
pub struct AgyOwnedConfigPatch {
    path: Vec<String>,
    expected: Option<Value>,
    replacement: Value,
}

impl fmt::Debug for AgyOwnedConfigPatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgyOwnedConfigPatch")
            .field("path_depth", &self.path.len())
            .field("expected_present", &self.expected.is_some())
            .finish_non_exhaustive()
    }
}

impl AgyOwnedConfigPatch {
    /// Creates a bounded generic patch for disposable fixtures.
    #[must_use]
    pub fn new(path: &[&str], expected: Option<Value>, replacement: Value) -> Option<Self> {
        let valid = !path.is_empty()
            && path.len() <= 16
            && path.iter().all(|segment| {
                !segment.is_empty()
                    && segment.len() <= 64
                    && segment.chars().all(|character| {
                        character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
                    })
            });
        valid.then(|| Self {
            path: path.iter().map(|segment| (*segment).to_owned()).collect(),
            expected,
            replacement,
        })
    }
}

/// Ownership-safe generic transaction result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgySetupTransactionError {
    Oversized,
    MalformedOrAmbiguous,
    ConcurrentDrift,
    WorkspaceLocalForbidden,
}

impl fmt::Display for AgySetupTransactionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Oversized => "Agy fixture configuration exceeds the bounded size",
            Self::MalformedOrAmbiguous => "Agy fixture configuration shape is ambiguous",
            Self::ConcurrentDrift => "Agy fixture configuration changed after preview",
            Self::WorkspaceLocalForbidden => {
                "workspace-local Agy configuration is forbidden before explicit admission"
            }
        })
    }
}

impl std::error::Error for AgySetupTransactionError {}

/// Opaque byte-exact snapshot for a generic disposable setup transaction.
pub struct AgyConfigSnapshot {
    original: Vec<u8>,
    object: Map<String, Value>,
    digest: [u8; 32],
}

impl fmt::Debug for AgyConfigSnapshot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgyConfigSnapshot")
            .field("byte_len", &self.original.len())
            .finish_non_exhaustive()
    }
}

/// Opaque receipt binding an exact before/candidate pair.
pub struct AgyConfigTransactionReceipt {
    original: Vec<u8>,
    applied: Vec<u8>,
    applied_digest: [u8; 32],
}

impl fmt::Debug for AgyConfigTransactionReceipt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgyConfigTransactionReceipt")
            .field("original_len", &self.original.len())
            .field("applied_len", &self.applied.len())
            .finish_non_exhaustive()
    }
}

impl AgyConfigTransactionReceipt {
    /// Candidate bytes are exposed only to the caller-owned disposable fixture layer.
    #[must_use]
    pub fn candidate_bytes(&self) -> &[u8] {
        &self.applied
    }
}

/// Generic, no-filesystem setup transaction scaffold.
pub struct AgySetupTransaction;

/// Configuration scope for the generic setup scaffold.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgySetupScope {
    UserGlobal,
    WorkspaceLocal,
}

/// Content-free preview of a generic fixture-backed owned change.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgySetupPreview {
    pub scope: AgySetupScope,
    pub unrelated_values_preserved: bool,
    pub production_authority: bool,
}

impl AgySetupTransaction {
    /// Takes an exact bounded snapshot while rejecting duplicate/deep JSON.
    ///
    /// # Errors
    ///
    /// Rejects oversized or structurally ambiguous fixture documents.
    pub fn snapshot(bytes: &[u8]) -> Result<AgyConfigSnapshot, AgySetupTransactionError> {
        if bytes.len() > MAX_AGY_QUALIFICATION_INPUT_BYTES {
            return Err(AgySetupTransactionError::Oversized);
        }
        let object = parse_qualification_object(bytes)
            .ok_or(AgySetupTransactionError::MalformedOrAmbiguous)?;
        Ok(AgyConfigSnapshot {
            original: bytes.to_vec(),
            object,
            digest: Sha256::digest(bytes).into(),
        })
    }

    /// Previews exact-field applicability without returning configuration content.
    ///
    /// # Errors
    ///
    /// Refuses ambiguous shapes and workspace-local scope.
    pub fn preview(
        scope: AgySetupScope,
        snapshot: &AgyConfigSnapshot,
        patch: &AgyOwnedConfigPatch,
    ) -> Result<AgySetupPreview, AgySetupTransactionError> {
        if scope == AgySetupScope::WorkspaceLocal {
            return Err(AgySetupTransactionError::WorkspaceLocalForbidden);
        }
        let mut root = Value::Object(snapshot.object.clone());
        replace_exact_path(&mut root, patch)?;
        Ok(AgySetupPreview {
            scope,
            unrelated_values_preserved: true,
            production_authority: false,
        })
    }

    /// Applies an exact owned field only while the byte-exact snapshot is current.
    ///
    /// # Errors
    ///
    /// Refuses concurrent drift and ambiguous/mismatched target shapes.
    pub fn apply_if_unchanged(
        snapshot: &AgyConfigSnapshot,
        current: &[u8],
        patch: &AgyOwnedConfigPatch,
    ) -> Result<AgyConfigTransactionReceipt, AgySetupTransactionError> {
        let current_digest: [u8; 32] = Sha256::digest(current).into();
        if snapshot.original != current || snapshot.digest != current_digest {
            return Err(AgySetupTransactionError::ConcurrentDrift);
        }
        let mut root = Value::Object(snapshot.object.clone());
        replace_exact_path(&mut root, patch)?;
        let applied = serde_json::to_vec_pretty(&root)
            .map_err(|_| AgySetupTransactionError::MalformedOrAmbiguous)?;
        Ok(AgyConfigTransactionReceipt {
            original: snapshot.original.clone(),
            applied_digest: Sha256::digest(&applied).into(),
            applied,
        })
    }

    /// Applies only to the future user-global supported surface.
    ///
    /// # Errors
    ///
    /// Refuses workspace-local configuration before an explicit later admission.
    pub fn apply_for_scope(
        scope: AgySetupScope,
        snapshot: &AgyConfigSnapshot,
        current: &[u8],
        patch: &AgyOwnedConfigPatch,
    ) -> Result<AgyConfigTransactionReceipt, AgySetupTransactionError> {
        if scope == AgySetupScope::WorkspaceLocal {
            return Err(AgySetupTransactionError::WorkspaceLocalForbidden);
        }
        Self::apply_if_unchanged(snapshot, current, patch)
    }

    /// Restores the byte-exact original only while the applied candidate is current.
    ///
    /// # Errors
    ///
    /// Refuses to overwrite a concurrent change.
    pub fn restore_if_unchanged(
        receipt: &AgyConfigTransactionReceipt,
        current: &[u8],
    ) -> Result<Vec<u8>, AgySetupTransactionError> {
        let current_digest: [u8; 32] = Sha256::digest(current).into();
        if receipt.applied != current || receipt.applied_digest != current_digest {
            return Err(AgySetupTransactionError::ConcurrentDrift);
        }
        Ok(receipt.original.clone())
    }

    /// Uninstall is the same exact-owned restoration operation.
    ///
    /// # Errors
    ///
    /// Refuses to overwrite a concurrent change.
    pub fn uninstall_if_owned(
        receipt: &AgyConfigTransactionReceipt,
        current: &[u8],
    ) -> Result<Vec<u8>, AgySetupTransactionError> {
        Self::restore_if_unchanged(receipt, current)
    }
}

fn replace_exact_path(
    root: &mut Value,
    patch: &AgyOwnedConfigPatch,
) -> Result<(), AgySetupTransactionError> {
    let (leaf, parents) = patch
        .path
        .split_last()
        .ok_or(AgySetupTransactionError::MalformedOrAmbiguous)?;
    let mut current = root;
    for parent in parents {
        current = current
            .as_object_mut()
            .and_then(|object| object.get_mut(parent))
            .ok_or(AgySetupTransactionError::MalformedOrAmbiguous)?;
    }
    let object = current
        .as_object_mut()
        .ok_or(AgySetupTransactionError::MalformedOrAmbiguous)?;
    match (&patch.expected, object.get(leaf)) {
        (Some(expected), Some(actual)) if expected == actual => {}
        (None, None) => {}
        _ => return Err(AgySetupTransactionError::MalformedOrAmbiguous),
    }
    object.insert(leaf.clone(), patch.replacement.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{Duration, SystemTime},
    };

    use serde_json::{Value, json};

    use super::{
        AGY_ADMITTED_PROFILE_SCHEMA, AGY_ADMITTED_VERSION, AGY_DAILY_COMMAND,
        AgyAdmissionGateError, AgyBackend, AgyBackendSource, AgyBackendState, AgyCandidatePresence,
        AgyCapabilityGate, AgyEvidenceProjection, AgyIntegrationReadiness,
        AgyNormalizedObservation, AgyNormalizer, AgyOwnedConfigPatch, AgyProductionSetup,
        AgyProductionSetupError, AgyProductionSetupOutcome, AgySafeRawObservation, AgySetupScope,
        AgySetupTransaction, AgySetupTransactionError, AgyTitleDispatchOutcome, AgyTitleNormalizer,
        AgyTitleRuntime,
    };
    use crate::{
        core::{FieldUpdate, Phase},
        settings::{
            ActivityMode, PresentationSettings, PresentationTheme, ProviderBadgePolicy,
            SpinnerPreset, TabColorMode, TitleMode,
        },
    };

    #[test]
    fn capability_gate_admits_only_the_exact_frozen_profile() {
        for document in [
            br"{}".as_slice(),
            br#"{"schema":"tabbeacon-agy-admitted-profile","version":1,"provider_enabled":true}"#,
            br#"{"admission":"admitted","capabilities":["all"]}"#,
            br#"{"schema":"tabbeacon-agy-admitted-profile-v1","version":"1.1.20","backend":"title_callback"}"#,
        ] {
            assert_eq!(
                AgyCapabilityGate::admit_profile(document).err(),
                Some(AgyAdmissionGateError::NoSupportedAdmittedProfileVersion)
            );
        }
        assert_eq!(
            AgyCapabilityGate::admit_profile(br#"{"x":1,"x":2}"#).err(),
            Some(AgyAdmissionGateError::Malformed)
        );
        let document = json!({
            "schema": AGY_ADMITTED_PROFILE_SCHEMA,
            "version": AGY_ADMITTED_VERSION,
            "backend": "title_callback"
        });
        let profile = AgyCapabilityGate::admit_profile(
            &serde_json::to_vec(&document).expect("frozen profile"),
        )
        .expect("exact profile admitted");
        assert_eq!(profile.version().as_string(), AGY_ADMITTED_VERSION);
        assert!(AgyBackend::default().provider_enabled());
    }

    #[test]
    fn observations_normalize_without_semantic_or_production_authority() {
        let normalized = AgyNormalizer::normalize(AgySafeRawObservation {
            lifecycle: AgyCandidatePresence::Observed,
            session: AgyCandidatePresence::Observed,
            workspace: AgyCandidatePresence::NotObserved,
            approval: AgyCandidatePresence::NotObserved,
            background_count: AgyCandidatePresence::NotObserved,
        });
        assert_eq!(normalized.candidate_fact_count, 2);
        assert!(!normalized.production_authority);
        assert_eq!(
            AgyBackend::default().observe_fail_open(normalized),
            AgyEvidenceProjection::Unadmitted
        );
        assert_eq!(AGY_DAILY_COMMAND, "agy");
    }

    #[test]
    fn fail_open_backend_rejects_non_title_sources_even_if_marked_authoritative() {
        let admitted = AgyNormalizedObservation {
            state: AgyBackendState::Admitted,
            source: AgyBackendSource::TitleCallback,
            candidate_fact_count: 1,
            production_authority: true,
        };
        assert_eq!(
            AgyBackend::default().observe_fail_open(admitted),
            AgyEvidenceProjection::Evidence
        );
        assert_eq!(
            AgyBackend::default().observe_fail_open(AgyNormalizedObservation {
                source: AgyBackendSource::Hooks,
                ..admitted
            }),
            AgyEvidenceProjection::Unadmitted
        );
    }

    #[test]
    fn title_normalizer_maps_only_the_real_observed_lifecycle_subset_without_content() {
        let root = temp_root("normalizer");
        fs::create_dir_all(&root).expect("workspace");
        let payload = serde_json::to_vec(&json!({
            "version": "1.1.19",
            "agent_state": "initializing",
            "conversation_id": "native-secret-session",
            "workspace": {"current_dir": root, "project_dir": root},
            "transcript": "private prompt and tool content",
            "model": "private model",
            "email": "private@example.invalid"
        }))
        .expect("payload");
        let normalized = AgyTitleNormalizer::normalize(
            super::frozen_profile(),
            &payload,
            SystemTime::UNIX_EPOCH,
        )
        .expect("exact admitted payload");
        assert_eq!(
            normalized.evidence.patch.phase,
            FieldUpdate::Set(Phase::Working)
        );
        assert_eq!(normalized.evidence.patch.attention, FieldUpdate::Unchanged);
        assert_eq!(normalized.evidence.patch.health, FieldUpdate::Unchanged);
        let debug = format!("{normalized:?}");
        assert!(!debug.contains("native-secret-session"));
        assert!(!debug.contains("private prompt"));
        assert!(!debug.contains("private@example.invalid"));

        for (state, phase) in [("idle", Phase::Ready), ("working", Phase::Working)] {
            let accepted = serde_json::to_vec(&json!({
                "version": "1.1.19",
                "agent_state": state,
                "conversation_id": "session",
                "workspace": {"current_dir": root, "project_dir": root}
            }))
            .expect("payload");
            assert_eq!(
                AgyTitleNormalizer::normalize(
                    super::frozen_profile(),
                    &accepted,
                    SystemTime::now(),
                )
                .expect("observed state is admitted")
                .evidence
                .patch
                .phase,
                FieldUpdate::Set(phase)
            );
        }

        for state in ["thinking", "tool_use", "error"] {
            let rejected = serde_json::to_vec(&json!({
                "version": "1.1.19",
                "agent_state": state,
                "conversation_id": "session",
                "workspace": {"current_dir": root, "project_dir": root}
            }))
            .expect("payload");
            assert!(
                AgyTitleNormalizer::normalize(
                    super::frozen_profile(),
                    &rejected,
                    SystemTime::now(),
                )
                .is_none()
            );
        }
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn title_runtime_is_plain_and_fail_open() {
        let root = temp_root("runtime");
        let workspace = root.join("workspace");
        fs::create_dir_all(&workspace).expect("workspace");
        let settings = PresentationSettings::new(
            TitleMode::TabBeacon,
            TabColorMode::TabBeacon,
            ActivityMode::TitleIndicator,
            SpinnerPreset::Codex,
            PresentationTheme::Classic,
        );
        let runtime = AgyTitleRuntime::new(root.join("state"), settings);
        let payload = serde_json::to_vec(&json!({
            "version": "1.1.19",
            "agent_state": "initializing",
            "conversation_id": "private-native-session",
            "workspace": {"current_dir": workspace, "project_dir": workspace},
            "transcript": "private prompt and tool output",
            "email": "private@example.invalid"
        }))
        .expect("payload");
        let response = runtime.dispatch_to(&payload, SystemTime::UNIX_EPOCH);
        assert_eq!(response.outcome, AgyTitleDispatchOutcome::Applied);
        assert_ne!(response.title, "Agy");
        assert!(response.title.contains("·A"));
        assert!(!response.title.contains('\u{1b}'));
        let off_runtime = AgyTitleRuntime::new(
            root.join("off-state"),
            settings.with_provider_badge(ProviderBadgePolicy::Off),
        );
        let off = off_runtime.dispatch_to(&payload, SystemTime::UNIX_EPOCH);
        assert!(!off.title.contains("·A"));
        let always_runtime = AgyTitleRuntime::new(
            root.join("always-state"),
            settings.with_provider_badge(ProviderBadgePolicy::Always),
        );
        let always = always_runtime.dispatch_to(&payload, SystemTime::UNIX_EPOCH);
        assert!(always.title.contains("·A"));
        let diagnostics =
            fs::read_to_string(root.join("state/agy-callback-v1/last-observation.json"))
                .expect("minimized diagnostics");
        let provider_files = fs::read_dir(root.join("state/provider-session-v1"))
            .expect("provider observation directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|value| value == "json")
            })
            .map(|entry| fs::read_to_string(entry.path()).expect("provider observation"))
            .collect::<String>();
        for forbidden in [
            "private-native-session",
            "private prompt",
            "private@example.invalid",
            workspace.to_str().expect("workspace path"),
        ] {
            assert!(!diagnostics.contains(forbidden));
            assert!(!provider_files.contains(forbidden));
        }
        let degraded = runtime.dispatch_to(b"not json", SystemTime::UNIX_EPOCH);
        assert_eq!(degraded.title, "Agy");
        assert_eq!(degraded.outcome, AgyTitleDispatchOutcome::DegradedInput);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn agy_root_anchor_is_provider_namespaced_and_resists_later_workspace_drift() {
        let root = temp_root("root-anchor-isolation");
        let first_workspace = root.join("first-workspace");
        let second_workspace = root.join("second-workspace");
        let state = root.join("state");
        fs::create_dir_all(&first_workspace).expect("first workspace");
        fs::create_dir_all(&second_workspace).expect("second workspace");
        let codex_anchor = state
            .join("codex-root-workspace-anchor-v1")
            .join("foreign-sentinel.bin");
        fs::create_dir_all(codex_anchor.parent().expect("codex anchor parent"))
            .expect("codex anchor directory");
        fs::write(&codex_anchor, b"codex-owned-sentinel").expect("Codex sentinel");
        let runtime = AgyTitleRuntime::new(&state, PresentationSettings::default());
        let payload = |workspace: &std::path::Path| {
            serde_json::to_vec(&json!({
                "version": "1.1.19",
                "agent_state": "working",
                "conversation_id": "same-private-native-session",
                "workspace": {"current_dir": workspace, "project_dir": workspace}
            }))
            .expect("payload")
        };

        let first = runtime.dispatch_to(
            &payload(&first_workspace),
            SystemTime::UNIX_EPOCH + Duration::from_secs(1_000),
        );
        let second = runtime.dispatch_to(
            &payload(&second_workspace),
            SystemTime::UNIX_EPOCH + Duration::from_secs(2_000),
        );
        assert_eq!(first.outcome, AgyTitleDispatchOutcome::Applied);
        assert_eq!(second.outcome, AgyTitleDispatchOutcome::Applied);
        assert_eq!(
            first.title, second.title,
            "later workspace input cannot rebind the root alias"
        );
        let shared_alias = crate::repo::WorkspaceIdentityResolver::new(&state)
            .resolve(&first_workspace)
            .expect("shared workspace resolver")
            .effective_alias;
        assert!(first.title.contains(shared_alias.as_str()));
        assert_eq!(
            fs::read(&codex_anchor).expect("Codex sentinel remains"),
            b"codex-owned-sentinel"
        );

        let session_sha256 = super::sha256_hex(b"same-private-native-session");
        let provider_row: Value = serde_json::from_slice(
            &fs::read(
                state
                    .join("provider-session-v1")
                    .join(format!("session-{session_sha256}.json")),
            )
            .expect("Agy provider row"),
        )
        .expect("provider row JSON");
        assert_eq!(provider_row["provider"], "agy");
        assert_eq!(
            provider_row["workspace_observability"]["workspace_mismatch_observed"],
            true
        );
        let anchor_text = fs::read_to_string(
            state
                .join("agy-root-workspace-anchor-v1")
                .join(format!("session-{session_sha256}.json")),
        )
        .expect("Agy anchor");
        assert!(!anchor_text.contains(first_workspace.to_string_lossy().as_ref()));
        assert!(!anchor_text.contains(second_workspace.to_string_lossy().as_ref()));
        assert!(!anchor_text.contains("same-private-native-session"));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn agy_plain_callback_never_creates_codex_generation_or_activity_worker_state() {
        let root = temp_root("no-cross-provider-worker");
        let workspace = root.join("workspace");
        let state = root.join("state");
        fs::create_dir_all(&workspace).expect("workspace");
        let runtime = AgyTitleRuntime::new(&state, PresentationSettings::default());
        let payload = serde_json::to_vec(&json!({
            "version": "1.1.19",
            "agent_state": "working",
            "conversation_id": "private-agy-session",
            "workspace": {"current_dir": workspace, "project_dir": workspace}
        }))
        .expect("payload");
        let response = runtime.dispatch_to(&payload, SystemTime::UNIX_EPOCH);
        assert_eq!(response.outcome, AgyTitleDispatchOutcome::Applied);
        assert!(!response.title.contains('\u{1b}'));
        assert!(!state.join("activity-worker-v1").exists());
        assert!(!state.join("codex-generation-v1").exists());
        assert!(!state.join("codex-root-workspace-anchor-v1").exists());
        assert!(state.join("agy-root-workspace-anchor-v1").is_dir());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn production_setup_preserves_unrelated_values_and_restores_exact_bytes() {
        let root = temp_root("setup");
        let config = root.join("home/.gemini/antigravity-cli/settings.json");
        let state = root.join("state");
        let executable = root.join("tabbeacon.exe");
        fs::create_dir_all(config.parent().expect("parent")).expect("config parent");
        fs::write(&executable, b"fixture executable").expect("executable");
        let original = br#"{
  "foreign": {"keep": true},
  "hooks": {"codex-owned-sentinel": true},
  "theme": "owner"
}
"#;
        fs::write(&config, original).expect("original");
        let setup =
            AgyProductionSetup::new(&config, &state, &executable, "agy").with_admitted_version();
        assert_eq!(setup.setup(), Ok(AgyProductionSetupOutcome::Installed));
        assert_eq!(
            setup.inspect().state,
            AgyIntegrationReadiness::SupportedConfigured
        );
        let applied: Value =
            serde_json::from_slice(&fs::read(&config).expect("applied")).expect("applied json");
        assert_eq!(applied["foreign"]["keep"], true);
        assert_eq!(applied["hooks"]["codex-owned-sentinel"], true);
        assert_eq!(applied["theme"], "owner");
        assert_eq!(applied["title"]["type"], "command");
        assert_eq!(applied["title"]["enabled"], true);
        assert_eq!(
            applied["title"]["command"],
            format!("{} agy __title-callback-v1", executable.to_string_lossy())
        );
        assert_eq!(
            setup.setup(),
            Ok(AgyProductionSetupOutcome::AlreadyConfigured)
        );

        // Agy may rewrite harmless formatting; semantic ownership still permits
        // the exact original restore observed during G64.
        fs::write(&config, serde_json::to_vec(&applied).expect("compact"))
            .expect("Agy-style rewrite");
        assert_eq!(setup.uninstall(), Ok(AgyProductionSetupOutcome::Removed));
        assert_eq!(fs::read(&config).expect("restored"), original);
        assert_eq!(
            setup.uninstall(),
            Ok(AgyProductionSetupOutcome::NotInstalled)
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn production_setup_upgrades_owned_quoted_v1_command_and_enables_title() {
        let root = temp_root("setup-v1-upgrade");
        let config = root.join("home/.gemini/antigravity-cli/settings.json");
        let state = root.join("state");
        let executable = root.join("tabbeacon.exe");
        fs::create_dir_all(config.parent().expect("parent")).expect("config parent");
        fs::create_dir_all(&state).expect("state");
        fs::write(&executable, b"fixture executable").expect("executable");
        let original = br#"{"foreign":1}"#;
        fs::write(state.join("original-settings.bin"), original).expect("backup");
        let setup =
            AgyProductionSetup::new(&config, &state, &executable, "agy").with_admitted_version();
        let legacy_callback = setup
            .callback_value_for_schema(super::AGY_SETUP_MANIFEST_SCHEMA_V1)
            .expect("legacy callback");
        let mut legacy_document = serde_json::Map::new();
        legacy_document.insert("foreign".to_owned(), json!(1));
        legacy_document.insert("title".to_owned(), legacy_callback.clone());
        let mut legacy_bytes =
            serde_json::to_vec_pretty(&Value::Object(legacy_document)).expect("legacy document");
        legacy_bytes.push(b'\n');
        fs::write(&config, &legacy_bytes).expect("legacy config");
        let legacy_manifest = super::AgySetupManifest {
            schema: super::AGY_SETUP_MANIFEST_SCHEMA_V1.to_owned(),
            admitted_version: AGY_ADMITTED_VERSION.to_owned(),
            original_present: true,
            original_sha256: super::sha256_hex(original),
            applied_sha256: super::sha256_hex(&legacy_bytes),
            executable_sha256: super::sha256_hex(b"fixture executable"),
            callback_sha256: super::sha256_hex(
                &serde_json::to_vec(&legacy_callback).expect("legacy callback bytes"),
            ),
        };
        fs::write(
            state.join("setup.json"),
            serde_json::to_vec_pretty(&legacy_manifest).expect("legacy manifest"),
        )
        .expect("write legacy manifest");

        assert_eq!(
            setup.inspect().state,
            AgyIntegrationReadiness::ConfigurationDrift
        );
        assert_eq!(setup.setup(), Ok(AgyProductionSetupOutcome::Installed));
        assert_eq!(
            setup.inspect().state,
            AgyIntegrationReadiness::SupportedConfigured
        );
        let upgraded: Value =
            serde_json::from_slice(&fs::read(&config).expect("upgraded")).expect("upgraded json");
        assert_eq!(upgraded["foreign"], 1);
        assert_eq!(upgraded["title"]["enabled"], true);
        assert_eq!(
            upgraded["title"]["command"],
            format!("{} agy __title-callback-v1", executable.to_string_lossy())
        );
        assert_eq!(setup.uninstall(), Ok(AgyProductionSetupOutcome::Removed));
        assert_eq!(fs::read(&config).expect("restored"), original);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn production_setup_refuses_callback_paths_requiring_shell_quoting() {
        let root = temp_root("setup-unsafe-command");
        let config = root.join("settings.json");
        let state = root.join("state");
        let executable = root.join("path with space").join("tabbeacon.exe");
        fs::create_dir_all(executable.parent().expect("parent")).expect("executable parent");
        fs::write(&executable, b"fixture executable").expect("executable");
        fs::write(&config, br#"{"foreign":1}"#).expect("config");
        let setup =
            AgyProductionSetup::new(&config, &state, &executable, "agy").with_admitted_version();

        assert_eq!(
            setup.setup(),
            Err(AgyProductionSetupError::UnrepresentableCallbackCommand)
        );
        assert_eq!(fs::read(&config).expect("unchanged"), br#"{"foreign":1}"#);
        assert!(!state.join("setup.json").exists());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn production_setup_restores_an_originally_absent_config() {
        let root = temp_root("absent");
        let config = root.join("home/.gemini/antigravity-cli/settings.json");
        let state = root.join("state");
        let executable = root.join("tabbeacon.exe");
        fs::create_dir_all(config.parent().expect("parent")).expect("config parent");
        fs::write(&executable, b"fixture executable").expect("executable");
        let setup =
            AgyProductionSetup::new(&config, &state, &executable, "agy").with_admitted_version();

        assert_eq!(setup.setup(), Ok(AgyProductionSetupOutcome::Installed));
        assert!(config.is_file());
        assert_eq!(setup.uninstall(), Ok(AgyProductionSetupOutcome::Removed));
        assert!(!config.exists());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn production_setup_refuses_foreign_title_and_preserves_unrelated_changes() {
        let root = temp_root("drift");
        let config = root.join("settings.json");
        let state = root.join("state");
        let executable = root.join("tabbeacon.exe");
        fs::create_dir_all(&root).expect("root");
        fs::write(&executable, b"fixture executable").expect("executable");
        fs::write(
            &config,
            br#"{"title":{"type":"command","command":"foreign"}}"#,
        )
        .expect("foreign title");
        let setup =
            AgyProductionSetup::new(&config, &state, &executable, "agy").with_admitted_version();
        assert_eq!(
            setup.setup(),
            Err(AgyProductionSetupError::ForeignTitleOwner)
        );

        fs::write(&config, br#"{"foreign":1}"#).expect("clean config");
        assert_eq!(setup.setup(), Ok(AgyProductionSetupOutcome::Installed));
        let mut applied: Value =
            serde_json::from_slice(&fs::read(&config).expect("applied")).expect("applied json");
        applied["foreign"] = json!(2);
        fs::write(&config, serde_json::to_vec(&applied).expect("drift bytes")).expect("drift");
        assert_eq!(
            setup.inspect().state,
            AgyIntegrationReadiness::SupportedConfigured
        );
        assert_eq!(setup.uninstall(), Ok(AgyProductionSetupOutcome::Removed));
        let uninstalled: Value = serde_json::from_slice(&fs::read(&config).expect("uninstalled"))
            .expect("uninstalled json");
        assert_eq!(uninstalled["foreign"], 2);
        assert!(uninstalled.get("title").is_none());

        fs::write(&config, br#"{"foreign":1}"#).expect("second clean config");
        assert_eq!(setup.setup(), Ok(AgyProductionSetupOutcome::Installed));
        let mut foreign_title: Value =
            serde_json::from_slice(&fs::read(&config).expect("second applied"))
                .expect("second applied json");
        foreign_title["title"]["command"] = json!("foreign");
        fs::write(
            &config,
            serde_json::to_vec(&foreign_title).expect("foreign title bytes"),
        )
        .expect("foreign title");
        assert_eq!(
            setup.uninstall(),
            Err(AgyProductionSetupError::ConfigurationDrift)
        );
        assert_eq!(
            setup.inspect().state,
            AgyIntegrationReadiness::ConfigurationDrift
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn production_setup_refuses_manifest_digest_and_shape_drift() {
        let root = temp_root("manifest-drift");
        let config = root.join("settings.json");
        let state = root.join("state");
        let executable = root.join("tabbeacon.exe");
        fs::create_dir_all(&root).expect("root");
        fs::write(&executable, b"fixture executable").expect("executable");
        fs::write(&config, br#"{"foreign":1}"#).expect("config");
        let setup =
            AgyProductionSetup::new(&config, &state, &executable, "agy").with_admitted_version();
        assert_eq!(setup.setup(), Ok(AgyProductionSetupOutcome::Installed));

        let manifest_path = state.join("setup.json");
        let exact_manifest = fs::read(&manifest_path).expect("manifest");
        for mutation in ["applied_sha256", "executable_sha256", "unknown"] {
            let mut document: Value =
                serde_json::from_slice(&exact_manifest).expect("manifest json");
            if mutation == "unknown" {
                document["foreign"] = json!(true);
            } else {
                document[mutation] = json!("0".repeat(64));
            }
            fs::write(
                &manifest_path,
                serde_json::to_vec_pretty(&document).expect("drift manifest"),
            )
            .expect("write drift");
            assert_eq!(
                setup.inspect().state,
                AgyIntegrationReadiness::ConfigurationDrift
            );
            if mutation == "executable_sha256" {
                assert_eq!(setup.setup(), Ok(AgyProductionSetupOutcome::Installed));
                assert_eq!(
                    setup.inspect().state,
                    AgyIntegrationReadiness::SupportedConfigured
                );
            } else {
                assert!(matches!(
                    setup.setup(),
                    Err(AgyProductionSetupError::OwnershipStateInvalid)
                ));
            }
            fs::write(&manifest_path, &exact_manifest).expect("restore manifest");
        }

        assert_eq!(setup.uninstall(), Ok(AgyProductionSetupOutcome::Removed));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn generic_setup_transaction_preserves_unrelated_values_and_restores_exact_bytes() {
        let original = br#"{
  "hooks": {"foreign": ["keep"]},
  "title": "owner-title",
  "status": {"foreign": true},
  "integration": {"callback": "old-owned"}
}"#;
        let snapshot = AgySetupTransaction::snapshot(original).expect("snapshot");
        let patch = AgyOwnedConfigPatch::new(
            &["integration", "callback"],
            Some(json!("old-owned")),
            json!("new-owned"),
        )
        .expect("bounded patch");
        let receipt = AgySetupTransaction::apply_if_unchanged(&snapshot, original, &patch)
            .expect("conditional apply");
        let preview = AgySetupTransaction::preview(AgySetupScope::UserGlobal, &snapshot, &patch)
            .expect("preview");
        assert!(preview.unrelated_values_preserved);
        assert!(!preview.production_authority);
        let candidate: Value =
            serde_json::from_slice(receipt.candidate_bytes()).expect("candidate JSON");
        assert_eq!(candidate["hooks"]["foreign"], json!(["keep"]));
        assert_eq!(candidate["title"], "owner-title");
        assert_eq!(candidate["status"]["foreign"], true);
        assert_eq!(candidate["integration"]["callback"], "new-owned");

        let restored =
            AgySetupTransaction::restore_if_unchanged(&receipt, receipt.candidate_bytes())
                .expect("exact restore");
        assert_eq!(restored, original);
        assert_eq!(
            AgySetupTransaction::uninstall_if_owned(&receipt, receipt.candidate_bytes())
                .expect("exact uninstall"),
            original
        );
    }

    #[test]
    fn setup_transaction_refuses_drift_ambiguity_and_duplicate_fields() {
        let original = br#"{"integration":{"callback":"owned"},"foreign":1}"#;
        let snapshot = AgySetupTransaction::snapshot(original).expect("snapshot");
        let patch = AgyOwnedConfigPatch::new(
            &["integration", "callback"],
            Some(json!("different")),
            json!("replacement"),
        )
        .expect("patch");
        assert_eq!(
            AgySetupTransaction::apply_if_unchanged(&snapshot, original, &patch).err(),
            Some(AgySetupTransactionError::MalformedOrAmbiguous)
        );
        let valid_patch = AgyOwnedConfigPatch::new(
            &["integration", "callback"],
            Some(json!("owned")),
            json!("replacement"),
        )
        .expect("patch");
        assert_eq!(
            AgySetupTransaction::apply_if_unchanged(
                &snapshot,
                br#"{"integration":{"callback":"drift"},"foreign":1}"#,
                &valid_patch,
            )
            .err(),
            Some(AgySetupTransactionError::ConcurrentDrift)
        );
        assert_eq!(
            AgySetupTransaction::snapshot(br#"{"x":1,"x":2}"#).err(),
            Some(AgySetupTransactionError::MalformedOrAmbiguous)
        );
        assert_eq!(
            AgySetupTransaction::apply_for_scope(
                AgySetupScope::WorkspaceLocal,
                &snapshot,
                original,
                &valid_patch,
            )
            .err(),
            Some(AgySetupTransactionError::WorkspaceLocalForbidden)
        );
    }

    fn temp_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "tabbeacon-agy-{label}-{}-{nonce}",
            std::process::id()
        ))
    }
}
