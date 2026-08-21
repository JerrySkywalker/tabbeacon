//! Agy pre-admission qualification primitives.
//!
//! This module intentionally does **not** provide an Agy runtime adapter,
//! configuration installer, process launcher, or production capability claim.
//! It accepts disposable callback and Hook samples, drops content-capable fields
//! at the boundary, and projects only safe observations required to prepare the
//! later Owner-present `TB-G64` admission spike.

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Stable Agy provider identifier reserved for pre-admission diagnostics.
pub const AGY_PROVIDER_ID: &str = "agy";
/// Schema version for pre-admission Agy qualification records.
pub const AGY_PREADMISSION_SCHEMA_VERSION: u32 = 1;
/// Maximum callback or Hook payload accepted for a disposable observation.
pub const MAX_AGY_QUALIFICATION_INPUT_BYTES: usize = 64 * 1024;
/// Largest count retained from an Agy status payload.
pub const MAX_BACKGROUND_TASK_COUNT: u16 = 1_024;
/// Plain fallback title used only by the disposable title-protocol harness.
pub const AGY_SAFE_FALLBACK_TITLE: &str = "Agy";

/// The only Agy admission state available before the G64 real-environment spike.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyAdmissionState {
    /// No real Owner-present environment has frozen a production profile.
    Unadmitted,
}

impl AgyAdmissionState {
    /// Stable machine spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unadmitted => "unadmitted",
        }
    }
}

/// A bounded semantic version observed without reading Agy configuration.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct AgyVersion {
    major: u16,
    minor: u16,
    patch: u16,
}

impl AgyVersion {
    /// Parses an exact `major.minor.patch` version without retaining source text.
    #[must_use]
    pub fn parse(input: &str) -> Option<Self> {
        let input = input.strip_prefix('v').unwrap_or(input);
        let mut parts = input.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        if parts.next().is_some() || input.len() > 24 {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    /// Stable version spelling constructed from checked numeric parts.
    #[must_use]
    pub fn as_string(self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Relation between a bounded local version and the separately audited docs version.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyVersionDrift {
    /// One or both sources did not expose a parseable bounded version.
    Unknown,
    /// The two version sources agree exactly.
    Match,
    /// The local CLI is newer than the documentation version.
    DocumentationOlder,
    /// The documentation version is newer than the local CLI.
    LocalCliOlder,
}

/// A version comparison that remains explicitly non-admitting.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyVersionDiagnostic {
    pub admission: AgyAdmissionState,
    pub observed_version: Option<AgyVersion>,
    pub documentation_version: Option<AgyVersion>,
    pub drift: AgyVersionDrift,
}

impl AgyVersionDiagnostic {
    /// Creates a safe version-only diagnostic from ephemeral CLI and docs input.
    #[must_use]
    pub fn from_versions(observed: Option<&str>, documentation: Option<&str>) -> Self {
        let observed_version = observed.and_then(AgyVersion::parse);
        let documentation_version = documentation.and_then(AgyVersion::parse);
        let drift = match (observed_version, documentation_version) {
            (Some(observed), Some(documentation)) if observed == documentation => {
                AgyVersionDrift::Match
            }
            (Some(observed), Some(documentation)) if observed > documentation => {
                AgyVersionDrift::DocumentationOlder
            }
            (Some(_), Some(_)) => AgyVersionDrift::LocalCliOlder,
            _ => AgyVersionDrift::Unknown,
        };
        Self {
            admission: AgyAdmissionState::Unadmitted,
            observed_version,
            documentation_version,
            drift,
        }
    }
}

/// Provider-neutral capability categories awaiting a real Agy admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyCapability {
    Phase,
    Attention,
    Approval,
    SessionIdentity,
    WorkspaceRoot,
    BackgroundTasks,
    TitleCallback,
    WindowsTerminalPresentation,
    HookObservation,
    SetupOwnership,
}

impl AgyCapability {
    /// Stable compact capability key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Phase => "phase",
            Self::Attention => "attention",
            Self::Approval => "approval",
            Self::SessionIdentity => "session_identity",
            Self::WorkspaceRoot => "workspace_root",
            Self::BackgroundTasks => "background_tasks",
            Self::TitleCallback => "title_callback",
            Self::WindowsTerminalPresentation => "windows_terminal_presentation",
            Self::HookObservation => "hook_observation",
            Self::SetupOwnership => "setup_ownership",
        }
    }
}

/// Truthful capability status before a real provider profile exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyCapabilityAvailability {
    /// The capability may be qualified but has no real admitted evidence.
    Unavailable,
    /// A disposable input did not contain enough safe information to classify it.
    Unknown,
}

/// One unadmitted capability projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyCapabilityStatus {
    pub capability: AgyCapability,
    pub availability: AgyCapabilityAvailability,
    pub authority: &'static str,
}

/// Frozen shape for a future profile; all values remain unadmitted today.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyCapabilityProfile {
    pub schema_version: u32,
    pub admission: AgyAdmissionState,
    pub provider_enabled: bool,
    pub version: AgyVersionDiagnostic,
    pub capabilities: Vec<AgyCapabilityStatus>,
}

impl AgyCapabilityProfile {
    /// Creates the only safe profile before G64: known candidate, no capabilities enabled.
    #[must_use]
    pub fn unadmitted(version: AgyVersionDiagnostic) -> Self {
        let capabilities = [
            AgyCapability::Phase,
            AgyCapability::Attention,
            AgyCapability::Approval,
            AgyCapability::SessionIdentity,
            AgyCapability::WorkspaceRoot,
            AgyCapability::BackgroundTasks,
            AgyCapability::TitleCallback,
            AgyCapability::WindowsTerminalPresentation,
            AgyCapability::HookObservation,
            AgyCapability::SetupOwnership,
        ]
        .into_iter()
        .map(|capability| AgyCapabilityStatus {
            capability,
            availability: AgyCapabilityAvailability::Unavailable,
            authority: "unadmitted",
        })
        .collect();
        Self {
            schema_version: AGY_PREADMISSION_SCHEMA_VERSION,
            admission: AgyAdmissionState::Unadmitted,
            provider_enabled: false,
            version,
            capabilities,
        }
    }
}

/// A direct-command boundary that rules out every launch-interception mechanism.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyDirectCommandBoundary {
    DirectOnly,
}

/// A direct command qualification plan. It never launches or configures Agy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyDirectCommandQualification {
    pub executable: &'static str,
    pub arguments: Vec<&'static str>,
    pub launch_boundary: AgyDirectCommandBoundary,
    pub status: AgyQualificationStatus,
}

impl AgyDirectCommandQualification {
    /// Plans the narrow read-only version observation used by the G64 runbook.
    #[must_use]
    pub fn version_probe() -> Self {
        Self {
            executable: "agy",
            arguments: vec!["--version"],
            launch_boundary: AgyDirectCommandBoundary::DirectOnly,
            status: AgyQualificationStatus::NotRun,
        }
    }
}

/// Whether a future Owner-present qualification has been performed.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyQualificationStatus {
    NotRun,
    Pass,
    Fail,
    Unavailable,
}

/// The real-environment prerequisites that only the Owner-present G64 spike can satisfy.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyOwnerGate {
    OwnerPresentAuthenticatedEnvironmentRequired,
}

/// Production provider state held fixed during the pre-admission train.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyProviderEnablement {
    Disabled,
}

/// Configuration mutation state held fixed during the pre-admission train.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyConfigurationMutation {
    None,
}

/// Preadmission plan with every production-changing action intentionally excluded.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyQualificationPlan {
    pub schema_version: u32,
    pub admission: AgyAdmissionState,
    pub owner_gate: AgyOwnerGate,
    pub provider_enablement: AgyProviderEnablement,
    pub direct_command: AgyDirectCommandQualification,
    pub title_callback: AgyQualificationStatus,
    pub hooks: AgyQualificationStatus,
    pub windows_terminal: AgyQualificationStatus,
    pub configuration_mutation: AgyConfigurationMutation,
}

impl Default for AgyQualificationPlan {
    fn default() -> Self {
        Self {
            schema_version: AGY_PREADMISSION_SCHEMA_VERSION,
            admission: AgyAdmissionState::Unadmitted,
            owner_gate: AgyOwnerGate::OwnerPresentAuthenticatedEnvironmentRequired,
            provider_enablement: AgyProviderEnablement::Disabled,
            direct_command: AgyDirectCommandQualification::version_probe(),
            title_callback: AgyQualificationStatus::NotRun,
            hooks: AgyQualificationStatus::NotRun,
            windows_terminal: AgyQualificationStatus::NotRun,
            configuration_mutation: AgyConfigurationMutation::None,
        }
    }
}

/// Generic input result deliberately free of parser error strings and source content.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyInputDisposition {
    Observed,
    Malformed,
    Oversized,
    UnknownEvent,
}

impl AgyInputDisposition {
    /// Stable compact machine spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observed => "observed",
            Self::Malformed => "malformed",
            Self::Oversized => "oversized",
            Self::UnknownEvent => "unknown_event",
        }
    }
}

/// A safe presence bit; raw IDs and values do not cross the recorder boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyFieldPresence {
    Present,
    Absent,
}

/// A lifecycle spelling observed in a disposable state payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyObservedPhase {
    Idle,
    Thinking,
    Working,
    ToolUse,
    Initializing,
    Unknown,
}

impl AgyObservedPhase {
    fn from_input(value: Option<&str>) -> Self {
        match value {
            Some("idle") => Self::Idle,
            Some("thinking") => Self::Thinking,
            Some("working") => Self::Working,
            Some("tool_use") => Self::ToolUse,
            Some("initializing") => Self::Initializing,
            Some(_) | None => Self::Unknown,
        }
    }
}

/// A count reported by a sample without treating absence as zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state", content = "value")]
pub enum AgyCountObservation {
    Observed(u16),
    Unavailable,
}

/// A single attention observation waiting for G64 authority qualification.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyAttentionObservation {
    ApprovalPending,
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContentFingerprint([u8; 32]);

impl ContentFingerprint {
    fn from_value(value: &str) -> Self {
        Self(Sha256::digest(value.as_bytes()).into())
    }
}

/// Workspace facts stripped of paths before any qualification record is returned.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyWorkspaceObservation {
    pub current_workspace_present: bool,
    pub project_workspace_present: bool,
    pub current_matches_project: Option<bool>,
    #[serde(skip)]
    current_fingerprint: Option<ContentFingerprint>,
    #[serde(skip)]
    project_fingerprint: Option<ContentFingerprint>,
}

impl AgyWorkspaceObservation {
    fn from_paths(current: Option<&str>, project: Option<&str>) -> Self {
        let current = current.filter(|value| !value.is_empty());
        let project = project.filter(|value| !value.is_empty());
        let current_fingerprint = current.map(ContentFingerprint::from_value);
        let project_fingerprint = project.map(ContentFingerprint::from_value);
        let current_matches_project = match (&current_fingerprint, &project_fingerprint) {
            (Some(current), Some(project)) => Some(current == project),
            _ => None,
        };
        Self {
            current_workspace_present: current_fingerprint.is_some(),
            project_workspace_present: project_fingerprint.is_some(),
            current_matches_project,
            current_fingerprint,
            project_fingerprint,
        }
    }

    fn root_candidate(&self) -> Option<&ContentFingerprint> {
        self.project_fingerprint
            .as_ref()
            .or(self.current_fingerprint.as_ref())
    }
}

/// Content-minimal observation from one Agy title/status-state payload.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyStateObservation {
    pub admission: AgyAdmissionState,
    pub version: Option<AgyVersion>,
    pub phase: AgyObservedPhase,
    pub conversation_identity: AgyFieldPresence,
    pub workspace: AgyWorkspaceObservation,
    pub background_tasks: AgyCountObservation,
    pub attention: AgyAttentionObservation,
}

/// One state recorder result; it never contains source payload text or paths.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyStateRecord {
    pub disposition: AgyInputDisposition,
    pub observation: Option<AgyStateObservation>,
}

/// Strict, non-persisting recorder for the Agy title/status state payload.
pub struct AgyStateRecorder;

impl AgyStateRecorder {
    /// Parses one disposable callback sample and retains only approved typed fields.
    #[must_use]
    pub fn record(payload: &[u8]) -> AgyStateRecord {
        if payload.len() > MAX_AGY_QUALIFICATION_INPUT_BYTES {
            return AgyStateRecord {
                disposition: AgyInputDisposition::Oversized,
                observation: None,
            };
        }
        let Ok(Value::Object(root)) = serde_json::from_slice::<Value>(payload) else {
            return AgyStateRecord {
                disposition: AgyInputDisposition::Malformed,
                observation: None,
            };
        };
        let workspace = root.get("workspace").and_then(Value::as_object);
        let current = workspace
            .and_then(|workspace| string_at(workspace, "current_dir"))
            .or_else(|| string_at(&root, "cwd"));
        let project = workspace.and_then(|workspace| string_at(workspace, "project_dir"));
        let conversation_identity = if ["conversation_id", "session_id"]
            .into_iter()
            .any(|key| string_at(&root, key).is_some())
        {
            AgyFieldPresence::Present
        } else {
            AgyFieldPresence::Absent
        };
        let background_tasks = root
            .get("task_count")
            .and_then(Value::as_u64)
            .and_then(|value| u16::try_from(value).ok())
            .filter(|value| *value <= MAX_BACKGROUND_TASK_COUNT)
            .map_or(
                AgyCountObservation::Unavailable,
                AgyCountObservation::Observed,
            );
        let attention = root
            .get("tool_confirmation_pending")
            .and_then(Value::as_bool)
            .filter(|pending| *pending)
            .map_or(AgyAttentionObservation::Unavailable, |_| {
                AgyAttentionObservation::ApprovalPending
            });
        AgyStateRecord {
            disposition: AgyInputDisposition::Observed,
            observation: Some(AgyStateObservation {
                admission: AgyAdmissionState::Unadmitted,
                version: root
                    .get("version")
                    .and_then(Value::as_str)
                    .and_then(AgyVersion::parse),
                phase: AgyObservedPhase::from_input(
                    root.get("agent_state").and_then(Value::as_str),
                ),
                conversation_identity,
                workspace: AgyWorkspaceObservation::from_paths(current, project),
                background_tasks,
                attention,
            }),
        }
    }
}

fn string_at<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

/// A generic phase candidate suitable for fixture-only normalization tests.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyNormalizedPhase {
    Ready,
    Working,
    Unknown,
}

/// Provider-neutral candidate output that cannot enter core reconciliation yet.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyNormalizationCandidate {
    pub admission: AgyAdmissionState,
    pub phase: AgyNormalizedPhase,
    pub attention: AgyAttentionObservation,
    pub session_identity: AgyFieldPresence,
    pub workspace_root_evidence: AgyFieldPresence,
    pub background_tasks: AgyCountObservation,
}

/// Normalizes only a disposable observation and never constructs `AgentEvidence`.
pub struct AgyPreAdmissionNormalizer;

impl AgyPreAdmissionNormalizer {
    /// Builds a provider-neutral candidate while preserving the unadmitted boundary.
    #[must_use]
    pub fn normalize(record: &AgyStateRecord) -> Option<AgyNormalizationCandidate> {
        let observation = record.observation.as_ref()?;
        let phase = match observation.phase {
            AgyObservedPhase::Idle => AgyNormalizedPhase::Ready,
            AgyObservedPhase::Thinking
            | AgyObservedPhase::Working
            | AgyObservedPhase::ToolUse
            | AgyObservedPhase::Initializing => AgyNormalizedPhase::Working,
            AgyObservedPhase::Unknown => AgyNormalizedPhase::Unknown,
        };
        let workspace_root_evidence = if observation.workspace.root_candidate().is_some() {
            AgyFieldPresence::Present
        } else {
            AgyFieldPresence::Absent
        };
        Some(AgyNormalizationCandidate {
            admission: AgyAdmissionState::Unadmitted,
            phase,
            attention: observation.attention,
            session_identity: observation.conversation_identity,
            workspace_root_evidence,
            background_tasks: observation.background_tasks,
        })
    }
}

/// Qualification-only Root Workspace Anchor fixture state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyRootAnchorQualification {
    pub admission: AgyAdmissionState,
    pub root_candidate_observed: bool,
    pub root_candidate_stable: bool,
    pub workspace_mismatch_observed: bool,
    pub observation_count: u16,
    #[serde(skip)]
    first_root_candidate: Option<ContentFingerprint>,
}

impl Default for AgyRootAnchorQualification {
    fn default() -> Self {
        Self {
            admission: AgyAdmissionState::Unadmitted,
            root_candidate_observed: false,
            root_candidate_stable: false,
            workspace_mismatch_observed: false,
            observation_count: 0,
            first_root_candidate: None,
        }
    }
}

impl AgyRootAnchorQualification {
    /// Records a disposable sample without binding an actual `TabBeacon` root anchor.
    pub fn observe(&mut self, record: &AgyStateRecord) {
        let Some(observation) = record.observation.as_ref() else {
            return;
        };
        self.observation_count = self.observation_count.saturating_add(1);
        if observation.workspace.current_matches_project == Some(false) {
            self.workspace_mismatch_observed = true;
        }
        let Some(candidate) = observation.workspace.root_candidate().cloned() else {
            return;
        };
        self.root_candidate_observed = true;
        if let Some(first) = &self.first_root_candidate {
            if first == &candidate {
                self.root_candidate_stable = true;
            } else {
                self.root_candidate_stable = false;
                self.workspace_mismatch_observed = true;
            }
        } else {
            self.first_root_candidate = Some(candidate);
        }
    }
}

/// Known Agy Hook event categories documented for a future qualification only.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyHookEvent {
    PreToolUse,
    PostToolUse,
    PreInvocation,
    PostInvocation,
    Stop,
    Unknown,
}

impl AgyHookEvent {
    /// Parses a Hook event name without storing unknown source strings.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        match value {
            "PreToolUse" => Self::PreToolUse,
            "PostToolUse" => Self::PostToolUse,
            "PreInvocation" => Self::PreInvocation,
            "PostInvocation" => Self::PostInvocation,
            "Stop" => Self::Stop,
            _ => Self::Unknown,
        }
    }
}

/// Content-minimal Hook observation; tool arguments, transcript paths, and errors are dropped.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyHookObservation {
    pub admission: AgyAdmissionState,
    pub event: AgyHookEvent,
    pub conversation_identity: AgyFieldPresence,
    pub workspace_path_count: u8,
    pub content_fields_dropped: bool,
}

/// Result from a Hook recorder that never returns raw callback input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyHookRecord {
    pub disposition: AgyInputDisposition,
    pub observation: Option<AgyHookObservation>,
}

/// Strict, non-persisting recorder for the Agy Hook JSON contract.
pub struct AgyHookRecorder;

impl AgyHookRecorder {
    /// Parses one disposable Hook sample and removes all content-capable fields.
    #[must_use]
    pub fn record(event_name: &str, payload: &[u8]) -> AgyHookRecord {
        let event = AgyHookEvent::parse(event_name);
        if event == AgyHookEvent::Unknown {
            return AgyHookRecord {
                disposition: AgyInputDisposition::UnknownEvent,
                observation: None,
            };
        }
        if payload.len() > MAX_AGY_QUALIFICATION_INPUT_BYTES {
            return AgyHookRecord {
                disposition: AgyInputDisposition::Oversized,
                observation: None,
            };
        }
        let Ok(Value::Object(root)) = serde_json::from_slice::<Value>(payload) else {
            return AgyHookRecord {
                disposition: AgyInputDisposition::Malformed,
                observation: None,
            };
        };
        let workspace_path_count = root
            .get("workspacePaths")
            .and_then(Value::as_array)
            .map_or(0, |paths| u8::try_from(paths.len()).unwrap_or(u8::MAX));
        let conversation_identity = if string_at(&root, "conversationId").is_some() {
            AgyFieldPresence::Present
        } else {
            AgyFieldPresence::Absent
        };
        AgyHookRecord {
            disposition: AgyInputDisposition::Observed,
            observation: Some(AgyHookObservation {
                admission: AgyAdmissionState::Unadmitted,
                event,
                conversation_identity,
                workspace_path_count,
                content_fields_dropped: true,
            }),
        }
    }
}

/// Result of testing the title callback protocol without retaining title text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyTitleProtocolSafety {
    NotRun,
    SafePlainTitle,
    UnsafeOutput,
}

/// Preadmission title/Windows Terminal feasibility harness state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyTitleWindowsTerminalQualification {
    pub admission: AgyAdmissionState,
    pub title_callback: AgyQualificationStatus,
    pub windows_terminal: AgyQualificationStatus,
    pub animation_worker: AgyQualificationStatus,
    pub protocol_safety: AgyTitleProtocolSafety,
}

impl Default for AgyTitleWindowsTerminalQualification {
    fn default() -> Self {
        Self {
            admission: AgyAdmissionState::Unadmitted,
            title_callback: AgyQualificationStatus::NotRun,
            windows_terminal: AgyQualificationStatus::NotRun,
            animation_worker: AgyQualificationStatus::NotRun,
            protocol_safety: AgyTitleProtocolSafety::NotRun,
        }
    }
}

impl AgyTitleWindowsTerminalQualification {
    /// Classifies candidate callback stdout without storing its title text.
    pub fn observe_callback_stdout(&mut self, stdout: &str) {
        self.protocol_safety = if is_safe_plain_title(stdout) {
            AgyTitleProtocolSafety::SafePlainTitle
        } else {
            AgyTitleProtocolSafety::UnsafeOutput
        };
    }
}

fn is_safe_plain_title(value: &str) -> bool {
    !value.is_empty() && value.len() <= 240 && !value.chars().any(char::is_control)
}

/// A title-callback response that is deliberately plain and fail-open.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyTitleCallbackResponse {
    pub input: AgyInputDisposition,
    pub admission: AgyAdmissionState,
    pub fallback_title: &'static str,
    pub title_state_retained: bool,
}

/// Evaluates title-state input and always provides a plain fallback response.
pub struct AgyTitleCallbackHarness;

impl AgyTitleCallbackHarness {
    /// Produces a static safe title even when the input is malformed or unknown.
    #[must_use]
    pub fn respond(payload: &[u8]) -> AgyTitleCallbackResponse {
        let record = AgyStateRecorder::record(payload);
        AgyTitleCallbackResponse {
            input: record.disposition,
            admission: AgyAdmissionState::Unadmitted,
            fallback_title: AGY_SAFE_FALLBACK_TITLE,
            title_state_retained: false,
        }
    }
}

/// Ownership-safe setup policy that can be reviewed without reading Agy configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyMutationDisposition {
    RefusedUnadmitted,
}

/// Backup requirement for any future Owner-approved configuration transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyBackupRequirement {
    FreshBeforeEveryApply,
}

/// Restore requirement for any future Owner-approved configuration transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyRestoreRequirement {
    ExactOwnedFingerprintOnly,
}

/// Configuration location boundary before a real provider profile is accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyProjectConfigurationBoundary {
    ProjectLocalForbidden,
}

/// Preservation boundary for user content not proven to be `TabBeacon` owned.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyUnrelatedContentBoundary {
    Preserve,
}

/// Scope boundary for planner tests before G64 transaction authority exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyFixtureScope {
    DisposableOnly,
}

/// Raw source content retention policy at the Agy qualification boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyContentRetention {
    None,
}

/// Fresh backup and exact-drift conditions required for any later Owner-approved apply.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgyOwnershipPlan {
    pub admission: AgyAdmissionState,
    pub mutation: AgyMutationDisposition,
    pub backup_requirement: AgyBackupRequirement,
    pub restore_requirement: AgyRestoreRequirement,
    pub project_configuration: AgyProjectConfigurationBoundary,
    pub unrelated_content: AgyUnrelatedContentBoundary,
    pub fixture_scope: AgyFixtureScope,
    pub content_retention: AgyContentRetention,
}

impl Default for AgyOwnershipPlan {
    fn default() -> Self {
        Self {
            admission: AgyAdmissionState::Unadmitted,
            mutation: AgyMutationDisposition::RefusedUnadmitted,
            backup_requirement: AgyBackupRequirement::FreshBeforeEveryApply,
            restore_requirement: AgyRestoreRequirement::ExactOwnedFingerprintOnly,
            project_configuration: AgyProjectConfigurationBoundary::ProjectLocalForbidden,
            unrelated_content: AgyUnrelatedContentBoundary::Preserve,
            fixture_scope: AgyFixtureScope::DisposableOnly,
            content_retention: AgyContentRetention::None,
        }
    }
}

/// In-memory disposable document fixture; no filesystem path or document text is retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgyDisposableSetupFixture {
    original_fingerprint: ContentFingerprint,
    plan: AgyOwnershipPlan,
}

impl AgyDisposableSetupFixture {
    /// Builds test-only ownership machinery from caller-owned disposable bytes.
    #[must_use]
    pub fn new(document: &[u8]) -> Self {
        Self {
            original_fingerprint: ContentFingerprint(Sha256::digest(document).into()),
            plan: AgyOwnershipPlan::default(),
        }
    }

    /// Returns the no-mutation plan without exposing fixture content.
    #[must_use]
    pub const fn plan(&self) -> &AgyOwnershipPlan {
        &self.plan
    }

    /// Detects drift against a second disposable document without preserving either document.
    #[must_use]
    pub fn has_drifted(&self, candidate: &[u8]) -> bool {
        self.original_fingerprint != ContentFingerprint(Sha256::digest(candidate).into())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AGY_SAFE_FALLBACK_TITLE, AgyAdmissionState, AgyAttentionObservation,
        AgyCapabilityAvailability, AgyCapabilityProfile, AgyCountObservation,
        AgyDirectCommandBoundary, AgyDirectCommandQualification, AgyFieldPresence, AgyHookEvent,
        AgyHookRecorder, AgyInputDisposition, AgyNormalizedPhase, AgyPreAdmissionNormalizer,
        AgyRootAnchorQualification, AgyStateRecorder, AgyTitleCallbackHarness,
        AgyTitleProtocolSafety, AgyTitleWindowsTerminalQualification, AgyVersionDiagnostic,
        AgyVersionDrift, MAX_AGY_QUALIFICATION_INPUT_BYTES,
    };

    fn state_payload() -> Vec<u8> {
        br#"{
          "conversation_id":"private-conversation-id",
          "version":"1.1.17",
          "agent_state":"working",
          "workspace":{"current_dir":"C:/private/worktree","project_dir":"C:/private/project"},
          "task_count":2,
          "tool_confirmation_pending":true,
          "transcript_path":"C:/private/transcript.jsonl",
          "email":"owner@example.test",
          "model":{"id":"private-model"},
          "quota":{"private":"content"}
        }"#
        .to_vec()
    }

    #[test]
    fn profile_is_explicitly_unadmitted_and_has_no_enabled_capability() {
        let profile = AgyCapabilityProfile::unadmitted(AgyVersionDiagnostic::from_versions(
            Some("1.1.17"),
            Some("1.1.14"),
        ));

        assert_eq!(profile.admission, AgyAdmissionState::Unadmitted);
        assert!(!profile.provider_enabled);
        assert_eq!(profile.version.drift, AgyVersionDrift::DocumentationOlder);
        assert!(profile.capabilities.iter().all(|capability| {
            capability.availability == AgyCapabilityAvailability::Unavailable
        }));
    }

    #[test]
    fn direct_version_probe_is_literal_agy_without_interception() {
        let plan = AgyDirectCommandQualification::version_probe();
        assert_eq!(plan.executable, "agy");
        assert_eq!(plan.arguments, vec!["--version"]);
        assert_eq!(plan.launch_boundary, AgyDirectCommandBoundary::DirectOnly);
    }

    #[test]
    fn state_recorder_projects_only_minimized_safe_fields() {
        let record = AgyStateRecorder::record(&state_payload());
        let observation = record.observation.as_ref().expect("state is observed");
        let json = serde_json::to_string(&record).expect("record serializes");

        assert_eq!(record.disposition, AgyInputDisposition::Observed);
        assert_eq!(observation.conversation_identity, AgyFieldPresence::Present);
        assert_eq!(
            observation.background_tasks,
            AgyCountObservation::Observed(2)
        );
        assert_eq!(
            observation.attention,
            AgyAttentionObservation::ApprovalPending
        );
        for forbidden in [
            "private-conversation-id",
            "C:/private",
            "transcript_path",
            "owner@example.test",
            "private-model",
            "quota",
        ] {
            assert!(!json.contains(forbidden), "record leaked {forbidden}");
        }
    }

    #[test]
    fn malformed_and_oversized_state_are_ignored_without_error_content() {
        assert_eq!(
            AgyStateRecorder::record(br#"{"agent_state": "working""#).disposition,
            AgyInputDisposition::Malformed
        );
        let oversized = vec![b'x'; MAX_AGY_QUALIFICATION_INPUT_BYTES + 1];
        assert_eq!(
            AgyStateRecorder::record(&oversized).disposition,
            AgyInputDisposition::Oversized
        );
    }

    #[test]
    fn unknown_state_never_becomes_a_core_ready_or_working_claim() {
        let record = AgyStateRecorder::record(
            br#"{"agent_state":"future-private-state","conversation_id":"id"}"#,
        );
        let normalized = AgyPreAdmissionNormalizer::normalize(&record).expect("candidate exists");

        assert_eq!(normalized.admission, AgyAdmissionState::Unadmitted);
        assert_eq!(normalized.phase, AgyNormalizedPhase::Unknown);
    }

    #[test]
    fn hook_recorder_drops_tool_args_errors_and_transcript_locations() {
        let record = AgyHookRecorder::record(
            "PostToolUse",
            br#"{
              "conversationId":"private-id",
              "workspacePaths":["C:/secret/a","C:/secret/b"],
              "transcriptPath":"C:/secret/transcript.jsonl",
              "artifactDirectoryPath":"C:/secret/artifacts",
              "toolCall":{"name":"run_command","args":{"CommandLine":"secret command"}},
              "error":"secret failure"
            }"#,
        );
        let json = serde_json::to_string(&record).expect("Hook record serializes");
        let observation = record.observation.expect("known Hook is observed");

        assert_eq!(observation.event, AgyHookEvent::PostToolUse);
        assert_eq!(observation.workspace_path_count, 2);
        assert!(observation.content_fields_dropped);
        for forbidden in [
            "private-id",
            "C:/secret",
            "secret command",
            "secret failure",
        ] {
            assert!(!json.contains(forbidden), "Hook record leaked {forbidden}");
        }
    }

    #[test]
    fn unknown_hook_event_is_fail_open_and_does_not_parse_payload() {
        let record = AgyHookRecorder::record("FuturePrivateEvent", br#"{"content":"secret"}"#);
        assert_eq!(record.disposition, AgyInputDisposition::UnknownEvent);
        assert!(record.observation.is_none());
    }

    #[test]
    fn root_anchor_fixture_detects_stability_and_dynamic_workspace_mismatch() {
        let root = AgyStateRecorder::record(
            br#"{"workspace":{"current_dir":"C:/root","project_dir":"C:/root"}}"#,
        );
        let dynamic = AgyStateRecorder::record(
            br#"{"workspace":{"current_dir":"C:/worktree","project_dir":"C:/root"}}"#,
        );
        let mut qualification = AgyRootAnchorQualification::default();
        qualification.observe(&root);
        qualification.observe(&dynamic);
        let json = serde_json::to_string(&qualification).expect("qualification serializes");

        assert!(qualification.root_candidate_observed);
        assert!(qualification.root_candidate_stable);
        assert!(qualification.workspace_mismatch_observed);
        assert!(!json.contains("C:/root"));
        assert!(!json.contains("C:/worktree"));
    }

    #[test]
    fn absent_task_count_stays_unavailable_not_zero() {
        let record = AgyStateRecorder::record(br#"{"agent_state":"idle"}"#);
        let normalized = AgyPreAdmissionNormalizer::normalize(&record).expect("candidate exists");

        assert_eq!(
            normalized.background_tasks,
            AgyCountObservation::Unavailable
        );
        assert_eq!(normalized.phase, AgyNormalizedPhase::Ready);
    }

    #[test]
    fn callback_harness_uses_static_plain_fallback_for_bad_input() {
        let response = AgyTitleCallbackHarness::respond(br#"{"agent_state":"broken""#);
        assert_eq!(response.input, AgyInputDisposition::Malformed);
        assert_eq!(response.fallback_title, AGY_SAFE_FALLBACK_TITLE);
        assert!(!response.title_state_retained);

        let mut qualification = AgyTitleWindowsTerminalQualification::default();
        qualification.observe_callback_stdout("Agy");
        assert_eq!(
            qualification.protocol_safety,
            AgyTitleProtocolSafety::SafePlainTitle
        );
        qualification.observe_callback_stdout("Agy\u{1b}[31m");
        assert_eq!(
            qualification.protocol_safety,
            AgyTitleProtocolSafety::UnsafeOutput
        );
    }

    #[test]
    fn ownership_fixture_refuses_mutation_and_detects_disposable_drift() {
        let fixture =
            super::AgyDisposableSetupFixture::new(b"{\"unrelated\":\"foreign-value-379\"}");
        let plan = fixture.plan();
        let json = serde_json::to_string(plan).expect("plan serializes");

        assert_eq!(plan.admission, AgyAdmissionState::Unadmitted);
        assert_eq!(
            plan.backup_requirement,
            super::AgyBackupRequirement::FreshBeforeEveryApply
        );
        assert_eq!(
            plan.restore_requirement,
            super::AgyRestoreRequirement::ExactOwnedFingerprintOnly
        );
        assert_eq!(
            plan.project_configuration,
            super::AgyProjectConfigurationBoundary::ProjectLocalForbidden
        );
        assert!(fixture.has_drifted(b"{\"unrelated\":\"changed\"}"));
        assert!(!json.contains("foreign-value-379"));
    }
}
