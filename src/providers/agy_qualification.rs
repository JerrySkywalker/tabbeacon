//! Disposable, content-minimal Agy qualification workspace.
//!
//! This module persists only typed allow-listed facts produced by
//! [`super::agy`] recorders. It never stores raw callback or Hook payloads and
//! every artifact remains explicitly unadmitted.

use std::{
    env, fmt, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use atomic_write_file::AtomicWriteFile;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

use super::agy::{
    AGY_PROVIDER_ID, AGY_SAFE_FALLBACK_TITLE, AgyAttentionObservation, AgyCapability,
    AgyCountObservation, AgyFieldPresence, AgyHookEvent, AgyHookRecord, AgyObservedPhase,
    AgyStateRecord, AgyVersion,
};

/// Stable schema for the durable qualification workspace.
pub const AGY_QUALIFICATION_WORKSPACE_SCHEMA: &str = "tabbeacon-agy-qualification-v1";
/// Stable schema for a candidate that can never satisfy production admission.
pub const AGY_QUALIFICATION_CANDIDATE_SCHEMA: &str = "tabbeacon-agy-qualification-candidate-v1";
/// Maximum durable observation lines per surface.
pub const MAX_AGY_QUALIFICATION_RECORDS: usize = 4_096;
/// Maximum size of one accumulated minimized record file.
pub const MAX_AGY_QUALIFICATION_RECORD_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_AGY_VERSION_OUTPUT_BYTES: u64 = 1_024;
const DIRECT_VERSION_TIMEOUT: Duration = Duration::from_secs(10);
const RUN_FILE: &str = "run.json";
const VERSION_FILE: &str = "version.json";
const TITLE_RECORDS_FILE: &str = "title-observations.jsonl";
const HOOK_RECORDS_FILE: &str = "hook-observations.jsonl";
const CANDIDATE_FILE: &str = "capability-candidate.json";
const REVIEW_FILE: &str = "owner-review.json";
const OWNED_ARTIFACTS: [&str; 6] = [
    RUN_FILE,
    VERSION_FILE,
    TITLE_RECORDS_FILE,
    HOOK_RECORDS_FILE,
    CANDIDATE_FILE,
    REVIEW_FILE,
];

/// Safe workspace failure without filesystem paths or source error strings.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyQualificationWorkspaceError {
    StateRootUnavailable,
    UnsafeLocation,
    NotInitialized,
    AlreadyInitialized,
    InvalidManagedWorkspace,
    RecordLimitReached,
    Io,
    Serialization,
}

impl fmt::Display for AgyQualificationWorkspaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::StateRootUnavailable => "a safe user-local qualification root is unavailable",
            Self::UnsafeLocation => "the qualification workspace location is unsafe",
            Self::NotInitialized => "the Agy qualification workspace is not initialized",
            Self::AlreadyInitialized => "the Agy qualification workspace already exists",
            Self::InvalidManagedWorkspace => {
                "the directory is not a valid managed Agy qualification workspace"
            }
            Self::RecordLimitReached => "the bounded Agy qualification record limit was reached",
            Self::Io => "an Agy qualification workspace operation failed",
            Self::Serialization => "an Agy qualification artifact could not be encoded",
        })
    }
}

impl std::error::Error for AgyQualificationWorkspaceError {}

/// Explicitly bounded executable-resolution fact.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyExecutableResolutionClass {
    LiteralPathSearch,
}

/// Direct version probe result; stderr and executable paths never enter it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgyDirectVersionProbe {
    pub installed: bool,
    pub version: Option<String>,
    pub executable_resolution_class: AgyExecutableResolutionClass,
    pub qualification_admission_state: String,
    pub provider_enabled: bool,
    pub outcome: AgyDirectVersionProbeOutcome,
}

/// Safe process outcome for the direct version probe.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyDirectVersionProbeOutcome {
    Observed,
    NotFound,
    Failed,
    TimedOut,
    Unparseable,
    OversizedOutput,
}

impl AgyDirectVersionProbe {
    fn unavailable(outcome: AgyDirectVersionProbeOutcome) -> Self {
        Self {
            installed: false,
            version: None,
            executable_resolution_class: AgyExecutableResolutionClass::LiteralPathSearch,
            qualification_admission_state: "unadmitted".to_owned(),
            provider_enabled: false,
            outcome,
        }
    }
}

/// Invokes only literal `agy --version`, bounds stdout, drops stderr, and times out.
#[must_use]
pub fn probe_direct_agy_version() -> AgyDirectVersionProbe {
    probe_direct_version_command("agy", DIRECT_VERSION_TIMEOUT)
}

fn probe_direct_version_command(command: &str, timeout: Duration) -> AgyDirectVersionProbe {
    let Ok(mut child) = Command::new(command)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return AgyDirectVersionProbe::unavailable(AgyDirectVersionProbeOutcome::NotFound);
    };
    let stdout = child.stdout.take();
    let reader = stdout.map(|stdout| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout
                .take(MAX_AGY_VERSION_OUTPUT_BYTES.saturating_add(1))
                .read_to_end(&mut bytes);
            bytes
        })
    });
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let bytes = reader
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    let Some(status) = status else {
        return AgyDirectVersionProbe {
            installed: true,
            ..AgyDirectVersionProbe::unavailable(AgyDirectVersionProbeOutcome::TimedOut)
        };
    };
    if !status.success() {
        return AgyDirectVersionProbe {
            installed: true,
            ..AgyDirectVersionProbe::unavailable(AgyDirectVersionProbeOutcome::Failed)
        };
    }
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_AGY_VERSION_OUTPUT_BYTES {
        return AgyDirectVersionProbe {
            installed: true,
            ..AgyDirectVersionProbe::unavailable(AgyDirectVersionProbeOutcome::OversizedOutput)
        };
    }
    let version = std::str::from_utf8(&bytes)
        .ok()
        .and_then(parse_version_output);
    AgyDirectVersionProbe {
        installed: true,
        outcome: if version.is_some() {
            AgyDirectVersionProbeOutcome::Observed
        } else {
            AgyDirectVersionProbeOutcome::Unparseable
        },
        version,
        executable_resolution_class: AgyExecutableResolutionClass::LiteralPathSearch,
        qualification_admission_state: "unadmitted".to_owned(),
        provider_enabled: false,
    }
}

fn parse_version_output(output: &str) -> Option<String> {
    output
        .split_ascii_whitespace()
        .map(|token| {
            token.trim_matches(|character: char| {
                !character.is_ascii_alphanumeric() && character != '.'
            })
        })
        .find_map(|token| AgyVersion::parse(token).map(AgyVersion::as_string))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct AgyQualificationRunMetadata {
    schema: String,
    provider: String,
    run_identity: String,
    created_unix_ms: u64,
    admission: String,
    provider_enabled: bool,
    daily_command: String,
    raw_content_persisted: bool,
    owner_config_mutated: bool,
}

/// Durable content-minimal title observation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyObservationPresence {
    Observed,
    NotObserved,
}

impl AgyObservationPresence {
    fn from_bool(observed: bool) -> Self {
        if observed {
            Self::Observed
        } else {
            Self::NotObserved
        }
    }

    const fn is_observed(self) -> bool {
        matches!(self, Self::Observed)
    }
}

/// Durable content-minimal title observation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgyStoredTitleObservation {
    pub disposition: String,
    pub phase: Option<String>,
    pub session_identity: AgyObservationPresence,
    pub current_workspace: AgyObservationPresence,
    pub project_workspace: AgyObservationPresence,
    pub current_matches_project: Option<bool>,
    pub background_count: Option<u16>,
    pub approval_evidence: AgyObservationPresence,
}

impl From<&AgyStateRecord> for AgyStoredTitleObservation {
    fn from(record: &AgyStateRecord) -> Self {
        let observation = record.observation.as_ref();
        Self {
            disposition: record.disposition.as_str().to_owned(),
            phase: observation.map(|observation| phase_name(observation.phase).to_owned()),
            session_identity: AgyObservationPresence::from_bool(observation.is_some_and(
                |observation| observation.conversation_identity == AgyFieldPresence::Present,
            )),
            current_workspace: AgyObservationPresence::from_bool(
                observation
                    .is_some_and(|observation| observation.workspace.current_workspace_present),
            ),
            project_workspace: AgyObservationPresence::from_bool(
                observation
                    .is_some_and(|observation| observation.workspace.project_workspace_present),
            ),
            current_matches_project: observation
                .and_then(|observation| observation.workspace.current_matches_project),
            background_count: observation.and_then(|observation| {
                match observation.background_tasks {
                    AgyCountObservation::Observed(count) => Some(count),
                    AgyCountObservation::Unavailable => None,
                }
            }),
            approval_evidence: AgyObservationPresence::from_bool(observation.is_some_and(
                |observation| observation.attention == AgyAttentionObservation::ApprovalPending,
            )),
        }
    }
}

/// Durable content-minimal Hook observation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgyStoredHookObservation {
    pub disposition: String,
    pub event_class: Option<String>,
    pub session_identity_observed: bool,
    pub workspace_path_count: Option<u8>,
    pub content_fields_dropped: bool,
}

impl From<&AgyHookRecord> for AgyStoredHookObservation {
    fn from(record: &AgyHookRecord) -> Self {
        let observation = record.observation.as_ref();
        Self {
            disposition: record.disposition.as_str().to_owned(),
            event_class: observation
                .map(|observation| hook_event_name(observation.event).to_owned()),
            session_identity_observed: observation.is_some_and(|observation| {
                observation.conversation_identity == AgyFieldPresence::Present
            }),
            workspace_path_count: observation.map(|observation| observation.workspace_path_count),
            content_fields_dropped: observation
                .is_some_and(|observation| observation.content_fields_dropped),
        }
    }
}

/// Stable evidence states used only by an unreviewed candidate.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyCandidateEvidenceState {
    ObservedSupportedCandidate,
    NotObserved,
    Unavailable,
    Ambiguous,
    RequiresOwnerReview,
}

/// One candidate capability claim that cannot become production authority.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgyQualificationCandidateCapability {
    pub capability: String,
    pub evidence: AgyCandidateEvidenceState,
    pub authority: String,
}

/// Stable unreviewed profile artifact, deliberately distinct from `AgyCapabilityProfile`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgyQualificationCandidate {
    pub schema: String,
    pub provider: String,
    pub admission: String,
    pub provider_enabled: bool,
    pub production_supported: bool,
    pub owner_review_required: bool,
    pub observed_version: Option<String>,
    pub capabilities: Vec<AgyQualificationCandidateCapability>,
}

/// Accumulated Human/JSON inspection model.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgyQualificationInspection {
    pub schema: String,
    pub initialized: bool,
    pub observed_version: Option<String>,
    pub title_samples: usize,
    pub usable_title_samples: usize,
    pub hook_samples: usize,
    pub usable_hook_samples: usize,
    pub session_identity: AgyCandidateEvidenceState,
    pub workspace_root: AgyCandidateEvidenceState,
    pub lifecycle_ready: AgyCandidateEvidenceState,
    pub lifecycle_working: AgyCandidateEvidenceState,
    pub lifecycle_result_ready: AgyCandidateEvidenceState,
    pub approval: AgyCandidateEvidenceState,
    pub background_tasks: AgyCandidateEvidenceState,
    pub title_callback: AgyCandidateEvidenceState,
    pub hooks: AgyCandidateEvidenceState,
    pub production_admission: String,
    pub provider_enabled: bool,
}

/// Compact workspace status.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyQualificationArtifactState {
    Available,
    Unavailable,
}

impl AgyQualificationArtifactState {
    fn from_bool(available: bool) -> Self {
        if available {
            Self::Available
        } else {
            Self::Unavailable
        }
    }
}

/// Compact workspace status.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgyQualificationWorkspaceStatus {
    pub schema: String,
    pub initialized: bool,
    pub version_observation: AgyQualificationArtifactState,
    pub title_samples: usize,
    pub hook_samples: usize,
    pub candidate: AgyQualificationArtifactState,
    pub owner_review_packet: AgyQualificationArtifactState,
    pub production_enabled: bool,
    pub provider_enabled: bool,
}

/// Owner review packet that remains pending by construction.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgyOwnerReviewBoundaries {
    pub raw_content_persisted: bool,
    pub owner_config_mutated: bool,
}

/// Owner review packet that remains pending by construction.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgyOwnerReviewPacket {
    pub schema: String,
    pub candidate: AgyQualificationCandidate,
    pub decision: String,
    pub real_g64_required: bool,
    pub provider_enabled: bool,
    pub boundaries: AgyOwnerReviewBoundaries,
}

/// Explicit disposable qualification workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgyQualificationWorkspace {
    root: PathBuf,
}

impl AgyQualificationWorkspace {
    /// Resolves the normal user-local qualification root without creating it.
    ///
    /// # Errors
    ///
    /// Returns a safe error if no supported user-local state base is available.
    pub fn user_local() -> Result<Self, AgyQualificationWorkspaceError> {
        #[cfg(windows)]
        let base = env::var_os("LOCALAPPDATA").map(PathBuf::from);
        #[cfg(not(windows))]
        let base = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state")));
        base.map(|base| Self::new(base.join("TabBeacon/qualification/agy")))
            .ok_or(AgyQualificationWorkspaceError::StateRootUnavailable)
    }

    /// Builds an explicit workspace handle without touching the filesystem.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Initializes a new managed disposable workspace.
    ///
    /// # Errors
    ///
    /// Refuses links, every pre-existing destination, and failed atomic writes.
    pub fn initialize(
        &self,
    ) -> Result<AgyQualificationWorkspaceStatus, AgyQualificationWorkspaceError> {
        create_new_safe_qualification_directory(&self.root)?;
        let now = unix_millis();
        let identity = opaque_run_identity(now);
        let metadata = AgyQualificationRunMetadata {
            schema: AGY_QUALIFICATION_WORKSPACE_SCHEMA.to_owned(),
            provider: AGY_PROVIDER_ID.to_owned(),
            run_identity: identity,
            created_unix_ms: now,
            admission: "unadmitted".to_owned(),
            provider_enabled: false,
            daily_command: "agy".to_owned(),
            raw_content_persisted: false,
            owner_config_mutated: false,
        };
        if let Err(error) = self.write_json(RUN_FILE, &metadata) {
            // The leaf was created by this call and is still expected to be empty.
            // Never recurse here: an unexpected entry must make cleanup fail closed.
            let _ = fs::remove_dir(&self.root);
            return Err(error);
        }
        self.status()
    }

    /// Reads current bounded artifact presence and counts.
    ///
    /// # Errors
    ///
    /// Returns a safe error for invalid or unreadable managed state.
    pub fn status(
        &self,
    ) -> Result<AgyQualificationWorkspaceStatus, AgyQualificationWorkspaceError> {
        if !self.root.exists() {
            return Ok(AgyQualificationWorkspaceStatus {
                schema: AGY_QUALIFICATION_WORKSPACE_SCHEMA.to_owned(),
                initialized: false,
                version_observation: AgyQualificationArtifactState::Unavailable,
                title_samples: 0,
                hook_samples: 0,
                candidate: AgyQualificationArtifactState::Unavailable,
                owner_review_packet: AgyQualificationArtifactState::Unavailable,
                production_enabled: false,
                provider_enabled: false,
            });
        }
        self.validate_managed()?;
        Ok(AgyQualificationWorkspaceStatus {
            schema: AGY_QUALIFICATION_WORKSPACE_SCHEMA.to_owned(),
            initialized: true,
            version_observation: AgyQualificationArtifactState::from_bool(
                self.root.join(VERSION_FILE).is_file(),
            ),
            title_samples: self
                .read_lines::<AgyStoredTitleObservation>(TITLE_RECORDS_FILE)?
                .len(),
            hook_samples: self
                .read_lines::<AgyStoredHookObservation>(HOOK_RECORDS_FILE)?
                .len(),
            candidate: AgyQualificationArtifactState::from_bool(
                self.root.join(CANDIDATE_FILE).is_file(),
            ),
            owner_review_packet: AgyQualificationArtifactState::from_bool(
                self.root.join(REVIEW_FILE).is_file(),
            ),
            production_enabled: false,
            provider_enabled: false,
        })
    }

    /// Persists a safe direct probe result.
    ///
    /// # Errors
    ///
    /// Returns a safe workspace error on failed validation or atomic write.
    pub fn record_version_probe(
        &self,
        probe: &AgyDirectVersionProbe,
    ) -> Result<(), AgyQualificationWorkspaceError> {
        self.validate_managed()?;
        self.write_json(VERSION_FILE, probe)
    }

    /// Appends one already-minimized title record.
    ///
    /// # Errors
    ///
    /// Refuses unsafe files and bounded record/file limits.
    pub fn record_title(
        &self,
        record: &AgyStateRecord,
    ) -> Result<AgyStoredTitleObservation, AgyQualificationWorkspaceError> {
        self.validate_managed()?;
        let stored = AgyStoredTitleObservation::from(record);
        self.append_json_line(TITLE_RECORDS_FILE, &stored)?;
        Ok(stored)
    }

    /// Appends one already-minimized Hook record.
    ///
    /// # Errors
    ///
    /// Refuses unsafe files and bounded record/file limits.
    pub fn record_hook(
        &self,
        record: &AgyHookRecord,
    ) -> Result<AgyStoredHookObservation, AgyQualificationWorkspaceError> {
        self.validate_managed()?;
        let stored = AgyStoredHookObservation::from(record);
        self.append_json_line(HOOK_RECORDS_FILE, &stored)?;
        Ok(stored)
    }

    /// Inspects accumulated minimized facts without returning raw documents.
    ///
    /// # Errors
    ///
    /// Returns a safe error for malformed managed artifacts.
    pub fn inspect(&self) -> Result<AgyQualificationInspection, AgyQualificationWorkspaceError> {
        self.validate_managed()?;
        let version = self.read_optional_json::<AgyDirectVersionProbe>(VERSION_FILE)?;
        let title = self.read_lines::<AgyStoredTitleObservation>(TITLE_RECORDS_FILE)?;
        let hooks = self.read_lines::<AgyStoredHookObservation>(HOOK_RECORDS_FILE)?;
        let observed_titles = title
            .iter()
            .filter(|record| record.disposition == "observed")
            .collect::<Vec<_>>();
        let observed_hooks = hooks
            .iter()
            .filter(|record| record.disposition == "observed")
            .collect::<Vec<_>>();
        let workspace_matches = observed_titles
            .iter()
            .filter_map(|record| record.current_matches_project)
            .collect::<Vec<_>>();
        let workspace_root = if workspace_matches.iter().any(|matches| !matches) {
            AgyCandidateEvidenceState::Ambiguous
        } else if observed_titles.iter().any(|record| {
            record.project_workspace.is_observed() || record.current_workspace.is_observed()
        }) {
            AgyCandidateEvidenceState::ObservedSupportedCandidate
        } else {
            AgyCandidateEvidenceState::NotObserved
        };
        Ok(AgyQualificationInspection {
            schema: AGY_QUALIFICATION_WORKSPACE_SCHEMA.to_owned(),
            initialized: true,
            observed_version: version.and_then(|probe| probe.version),
            title_samples: title.len(),
            usable_title_samples: observed_titles.len(),
            hook_samples: hooks.len(),
            usable_hook_samples: observed_hooks.len(),
            session_identity: observed_candidate(
                observed_titles
                    .iter()
                    .any(|record| record.session_identity.is_observed())
                    || observed_hooks
                        .iter()
                        .any(|record| record.session_identity_observed),
            ),
            workspace_root,
            lifecycle_ready: observed_candidate(
                observed_titles
                    .iter()
                    .any(|record| record.phase.as_deref() == Some("idle")),
            ),
            lifecycle_working: observed_candidate(observed_titles.iter().any(|record| {
                matches!(
                    record.phase.as_deref(),
                    Some("thinking" | "working" | "tool_use" | "initializing")
                )
            })),
            lifecycle_result_ready: AgyCandidateEvidenceState::NotObserved,
            approval: observed_candidate(
                observed_titles
                    .iter()
                    .any(|record| record.approval_evidence.is_observed()),
            ),
            background_tasks: observed_candidate(
                observed_titles
                    .iter()
                    .any(|record| record.background_count.is_some()),
            ),
            title_callback: observed_candidate(!observed_titles.is_empty()),
            hooks: observed_candidate(!observed_hooks.is_empty()),
            production_admission: "blocked_owner_g64_approval_required".to_owned(),
            provider_enabled: false,
        })
    }

    /// Compiles and atomically writes a candidate-only capability artifact.
    ///
    /// # Errors
    ///
    /// Returns a safe error for invalid workspace artifacts or atomic write failure.
    pub fn compile_candidate(
        &self,
    ) -> Result<AgyQualificationCandidate, AgyQualificationWorkspaceError> {
        let inspection = self.inspect()?;
        let capabilities = [
            (
                AgyCapability::Phase,
                merge_candidate(inspection.lifecycle_ready, inspection.lifecycle_working),
            ),
            (AgyCapability::Attention, inspection.approval),
            (AgyCapability::Approval, inspection.approval),
            (
                AgyCapability::Health,
                AgyCandidateEvidenceState::Unavailable,
            ),
            (AgyCapability::SessionIdentity, inspection.session_identity),
            (AgyCapability::WorkspaceRoot, inspection.workspace_root),
            (AgyCapability::BackgroundTasks, inspection.background_tasks),
            (AgyCapability::TitleCallback, inspection.title_callback),
            (
                AgyCapability::WindowsTerminalPresentation,
                AgyCandidateEvidenceState::Unavailable,
            ),
            (AgyCapability::HookObservation, inspection.hooks),
            (
                AgyCapability::SetupOwnership,
                AgyCandidateEvidenceState::RequiresOwnerReview,
            ),
        ]
        .into_iter()
        .map(
            |(capability, evidence)| AgyQualificationCandidateCapability {
                capability: capability.as_str().to_owned(),
                evidence,
                authority: if matches!(
                    evidence,
                    AgyCandidateEvidenceState::ObservedSupportedCandidate
                ) {
                    "qualification_observation"
                } else {
                    "owner_review_required"
                }
                .to_owned(),
            },
        )
        .collect();
        let candidate = AgyQualificationCandidate {
            schema: AGY_QUALIFICATION_CANDIDATE_SCHEMA.to_owned(),
            provider: AGY_PROVIDER_ID.to_owned(),
            admission: "unadmitted".to_owned(),
            provider_enabled: false,
            production_supported: false,
            owner_review_required: true,
            observed_version: inspection.observed_version,
            capabilities,
        };
        self.write_json(CANDIDATE_FILE, &candidate)?;
        Ok(candidate)
    }

    /// Produces a pending Owner review packet from the current candidate.
    ///
    /// # Errors
    ///
    /// Returns a safe error on invalid artifacts or atomic write failure.
    pub fn owner_review_packet(
        &self,
    ) -> Result<AgyOwnerReviewPacket, AgyQualificationWorkspaceError> {
        let candidate = self.compile_candidate()?;
        let packet = AgyOwnerReviewPacket {
            schema: "tabbeacon-agy-owner-review-v1".to_owned(),
            candidate,
            decision: "pending_owner_g64_review".to_owned(),
            real_g64_required: true,
            provider_enabled: false,
            boundaries: AgyOwnerReviewBoundaries {
                raw_content_persisted: false,
                owner_config_mutated: false,
            },
        };
        self.write_json(REVIEW_FILE, &packet)?;
        Ok(packet)
    }

    /// Deletes only allow-listed files from a positively identified workspace.
    ///
    /// # Errors
    ///
    /// Refuses unmanaged, linked, unreadable, or unexpectedly populated directories.
    pub fn clean(self) -> Result<(), AgyQualificationWorkspaceError> {
        self.validate_managed()?;
        let mut owned_files = Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|_| AgyQualificationWorkspaceError::Io)? {
            let entry = entry.map_err(|_| AgyQualificationWorkspaceError::Io)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| AgyQualificationWorkspaceError::InvalidManagedWorkspace)?;
            if !OWNED_ARTIFACTS.contains(&name.as_str()) {
                return Err(AgyQualificationWorkspaceError::InvalidManagedWorkspace);
            }
            let metadata = fs::symlink_metadata(entry.path())
                .map_err(|_| AgyQualificationWorkspaceError::Io)?;
            if !metadata.is_file() || metadata_is_link_or_reparse(&metadata) {
                return Err(AgyQualificationWorkspaceError::InvalidManagedWorkspace);
            }
            owned_files.push(entry.path());
        }
        for path in owned_files {
            fs::remove_file(path).map_err(|_| AgyQualificationWorkspaceError::Io)?;
        }
        fs::remove_dir(&self.root).map_err(|_| AgyQualificationWorkspaceError::Io)
    }

    fn validate_managed(
        &self,
    ) -> Result<AgyQualificationRunMetadata, AgyQualificationWorkspaceError> {
        validate_dedicated_qualification_leaf(&self.root)?;
        ensure_no_reparse_ancestors(&self.root)?;
        ensure_safe_existing_directory(&self.root)?;
        let metadata: AgyQualificationRunMetadata = self
            .read_optional_json(RUN_FILE)?
            .ok_or(AgyQualificationWorkspaceError::NotInitialized)?;
        if metadata.schema != AGY_QUALIFICATION_WORKSPACE_SCHEMA
            || metadata.provider != AGY_PROVIDER_ID
            || metadata.admission != "unadmitted"
            || metadata.provider_enabled
            || metadata.raw_content_persisted
            || metadata.owner_config_mutated
            || metadata.daily_command != "agy"
            || metadata.created_unix_ms == 0
            || metadata.run_identity.len() != 32
            || !metadata
                .run_identity
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(AgyQualificationWorkspaceError::InvalidManagedWorkspace);
        }
        Ok(metadata)
    }

    fn write_json(
        &self,
        name: &str,
        value: &impl Serialize,
    ) -> Result<(), AgyQualificationWorkspaceError> {
        let mut bytes = serde_json::to_vec_pretty(value)
            .map_err(|_| AgyQualificationWorkspaceError::Serialization)?;
        bytes.push(b'\n');
        atomic_write_safe(&self.root.join(name), &bytes)
    }

    fn read_optional_json<T: for<'de> Deserialize<'de>>(
        &self,
        name: &str,
    ) -> Result<Option<T>, AgyQualificationWorkspaceError> {
        let path = self.root.join(name);
        reject_link_if_exists(&path)?;
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|_| AgyQualificationWorkspaceError::InvalidManagedWorkspace),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(AgyQualificationWorkspaceError::Io),
        }
    }

    fn append_json_line(
        &self,
        name: &str,
        value: &impl Serialize,
    ) -> Result<(), AgyQualificationWorkspaceError> {
        let path = self.root.join(name);
        reject_link_if_exists(&path)?;
        let existing_len = match fs::metadata(&path) {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
            Err(_) => return Err(AgyQualificationWorkspaceError::Io),
        };
        if existing_len >= MAX_AGY_QUALIFICATION_RECORD_FILE_BYTES
            || self.read_lines::<serde_json::Value>(name)?.len() >= MAX_AGY_QUALIFICATION_RECORDS
        {
            return Err(AgyQualificationWorkspaceError::RecordLimitReached);
        }
        let mut bytes =
            serde_json::to_vec(value).map_err(|_| AgyQualificationWorkspaceError::Serialization)?;
        bytes.push(b'\n');
        if existing_len.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX))
            > MAX_AGY_QUALIFICATION_RECORD_FILE_BYTES
        {
            return Err(AgyQualificationWorkspaceError::RecordLimitReached);
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|_| AgyQualificationWorkspaceError::Io)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_data())
            .map_err(|_| AgyQualificationWorkspaceError::Io)
    }

    fn read_lines<T: for<'de> Deserialize<'de>>(
        &self,
        name: &str,
    ) -> Result<Vec<T>, AgyQualificationWorkspaceError> {
        let path = self.root.join(name);
        reject_link_if_exists(&path)?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(_) => return Err(AgyQualificationWorkspaceError::Io),
        };
        if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_AGY_QUALIFICATION_RECORD_FILE_BYTES
        {
            return Err(AgyQualificationWorkspaceError::RecordLimitReached);
        }
        let mut records = Vec::new();
        for line in bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            if records.len() >= MAX_AGY_QUALIFICATION_RECORDS {
                return Err(AgyQualificationWorkspaceError::RecordLimitReached);
            }
            records.push(
                serde_json::from_slice(line)
                    .map_err(|_| AgyQualificationWorkspaceError::InvalidManagedWorkspace)?,
            );
        }
        Ok(records)
    }
}

fn observed_candidate(observed: bool) -> AgyCandidateEvidenceState {
    if observed {
        AgyCandidateEvidenceState::ObservedSupportedCandidate
    } else {
        AgyCandidateEvidenceState::NotObserved
    }
}

fn merge_candidate(
    left: AgyCandidateEvidenceState,
    right: AgyCandidateEvidenceState,
) -> AgyCandidateEvidenceState {
    if matches!(left, AgyCandidateEvidenceState::ObservedSupportedCandidate)
        || matches!(right, AgyCandidateEvidenceState::ObservedSupportedCandidate)
    {
        AgyCandidateEvidenceState::ObservedSupportedCandidate
    } else {
        AgyCandidateEvidenceState::NotObserved
    }
}

fn phase_name(phase: AgyObservedPhase) -> &'static str {
    match phase {
        AgyObservedPhase::Idle => "idle",
        AgyObservedPhase::Thinking => "thinking",
        AgyObservedPhase::Working => "working",
        AgyObservedPhase::ToolUse => "tool_use",
        AgyObservedPhase::Initializing => "initializing",
        AgyObservedPhase::Unknown => "unknown",
    }
}

fn hook_event_name(event: AgyHookEvent) -> &'static str {
    match event {
        AgyHookEvent::PreToolUse => "pre_tool_use",
        AgyHookEvent::PostToolUse => "post_tool_use",
        AgyHookEvent::PreInvocation => "pre_invocation",
        AgyHookEvent::PostInvocation => "post_invocation",
        AgyHookEvent::Stop => "stop",
        AgyHookEvent::Unknown => "unknown",
    }
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

fn opaque_run_identity(now: u64) -> String {
    let digest =
        Sha256::digest(format!("agy-qualification-{now}-{}", std::process::id()).as_bytes());
    digest[..16]
        .iter()
        .fold(String::with_capacity(32), |mut identity, byte| {
            let _ = fmt::Write::write_fmt(&mut identity, format_args!("{byte:02x}"));
            identity
        })
}

fn validate_dedicated_qualification_leaf(
    path: &Path,
) -> Result<(), AgyQualificationWorkspaceError> {
    if !path.is_absolute() {
        return Err(AgyQualificationWorkspaceError::UnsafeLocation);
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return Err(AgyQualificationWorkspaceError::UnsafeLocation);
    };
    if name != "agy"
        && name
            .strip_prefix("tabbeacon-agy-qualification-")
            .is_none_or(str::is_empty)
    {
        return Err(AgyQualificationWorkspaceError::UnsafeLocation);
    }
    Ok(())
}

fn create_new_safe_qualification_directory(
    path: &Path,
) -> Result<(), AgyQualificationWorkspaceError> {
    validate_dedicated_qualification_leaf(path)?;
    ensure_no_reparse_ancestors(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_dir() || metadata_is_link_or_reparse(&metadata) {
                return Err(AgyQualificationWorkspaceError::UnsafeLocation);
            }
            if path.join(RUN_FILE).exists() {
                return Err(AgyQualificationWorkspaceError::AlreadyInitialized);
            }
            return Err(AgyQualificationWorkspaceError::UnsafeLocation);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(AgyQualificationWorkspaceError::Io),
    }
    let parent = path
        .parent()
        .ok_or(AgyQualificationWorkspaceError::UnsafeLocation)?;
    fs::create_dir_all(parent).map_err(|_| AgyQualificationWorkspaceError::Io)?;
    ensure_no_reparse_ancestors(parent)?;
    fs::create_dir(path).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            AgyQualificationWorkspaceError::UnsafeLocation
        } else {
            AgyQualificationWorkspaceError::Io
        }
    })?;
    ensure_no_reparse_ancestors(path)?;
    ensure_safe_existing_directory(path)
}

fn ensure_no_reparse_ancestors(path: &Path) -> Result<(), AgyQualificationWorkspaceError> {
    for ancestor in path.ancestors() {
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) => {
                if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
                    return Err(AgyQualificationWorkspaceError::UnsafeLocation);
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return Err(AgyQualificationWorkspaceError::Io),
        }
    }
    Ok(())
}

fn ensure_safe_existing_directory(path: &Path) -> Result<(), AgyQualificationWorkspaceError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            AgyQualificationWorkspaceError::NotInitialized
        } else {
            AgyQualificationWorkspaceError::Io
        }
    })?;
    if !metadata.is_dir() || metadata_is_link_or_reparse(&metadata) {
        return Err(AgyQualificationWorkspaceError::UnsafeLocation);
    }
    Ok(())
}

fn reject_link_if_exists(path: &Path) -> Result<(), AgyQualificationWorkspaceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata_is_link_or_reparse(&metadata) || metadata.is_dir() => {
            Err(AgyQualificationWorkspaceError::UnsafeLocation)
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(AgyQualificationWorkspaceError::Io),
    }
}

fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    metadata.file_type().is_symlink()
}

fn atomic_write_safe(path: &Path, bytes: &[u8]) -> Result<(), AgyQualificationWorkspaceError> {
    reject_link_if_exists(path)?;
    let parent = path
        .parent()
        .ok_or(AgyQualificationWorkspaceError::UnsafeLocation)?;
    ensure_safe_existing_directory(parent)?;
    let mut file = AtomicWriteFile::options()
        .open(path)
        .map_err(|_| AgyQualificationWorkspaceError::Io)?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.commit())
        .map_err(|_| AgyQualificationWorkspaceError::Io)
}

/// Protocol-safe fallback returned by the durable callback recorder.
#[must_use]
pub const fn qualification_callback_fallback() -> &'static str {
    AGY_SAFE_FALLBACK_TITLE
}

#[cfg(test)]
mod tests {
    use std::{env, fs, path::PathBuf};

    use super::{
        AgyCandidateEvidenceState, AgyDirectVersionProbe, AgyDirectVersionProbeOutcome,
        AgyExecutableResolutionClass, AgyQualificationWorkspace, AgyQualificationWorkspaceError,
        parse_version_output,
    };
    use crate::providers::agy::{AgyHookRecorder, AgyStateRecorder};
    use crate::providers::agy_backend::{AgyAdmissionGateError, AgyCapabilityGate};

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new(name: &str) -> Self {
            let path = env::temp_dir().join(format!(
                "tabbeacon-agy-qualification-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn probe() -> AgyDirectVersionProbe {
        AgyDirectVersionProbe {
            installed: true,
            version: Some("1.2.3".to_owned()),
            executable_resolution_class: AgyExecutableResolutionClass::LiteralPathSearch,
            qualification_admission_state: "unadmitted".to_owned(),
            provider_enabled: false,
            outcome: AgyDirectVersionProbeOutcome::Observed,
        }
    }

    #[test]
    fn version_output_parser_retains_only_a_bounded_semver() {
        assert_eq!(
            parse_version_output("agy 1.2.3\n"),
            Some("1.2.3".to_owned())
        );
        assert_eq!(parse_version_output("private path and account"), None);
    }

    #[test]
    fn workspace_accumulates_only_minimized_facts_and_builds_pending_candidate() {
        let root = TestRoot::new("workflow");
        let workspace = AgyQualificationWorkspace::new(root.0.clone());
        let initialized = workspace.initialize().expect("workspace initializes");
        assert!(initialized.initialized);
        assert!(!initialized.production_enabled);
        workspace
            .record_version_probe(&probe())
            .expect("probe records");

        let title = AgyStateRecorder::record(
            br#"{
          "conversation_id":"private-id",
          "agent_state":"working",
          "workspace":{"current_dir":"C:/private/root","project_dir":"C:/private/root"},
          "task_count":2,
          "tool_confirmation_pending":true,
          "prompt":"private prompt",
          "transcript_path":"C:/private/transcript"
        }"#,
        );
        workspace.record_title(&title).expect("title records");
        let hook = AgyHookRecorder::record(
            "PostToolUse",
            br#"{"conversationId":"private-hook","workspacePaths":["C:/private/root"],"toolCall":{"args":{"prompt":"private"}}}"#,
        );
        workspace.record_hook(&hook).expect("Hook records");

        let inspection = workspace.inspect().expect("inspection");
        assert_eq!(inspection.observed_version.as_deref(), Some("1.2.3"));
        assert_eq!(inspection.usable_title_samples, 1);
        assert_eq!(inspection.usable_hook_samples, 1);
        assert_eq!(
            inspection.session_identity,
            AgyCandidateEvidenceState::ObservedSupportedCandidate
        );
        assert!(!inspection.provider_enabled);

        let candidate = workspace.compile_candidate().expect("candidate compiles");
        assert!(candidate.owner_review_required);
        assert!(!candidate.production_supported);
        assert!(!candidate.provider_enabled);
        let candidate_bytes = serde_json::to_vec(&candidate).expect("candidate serializes");
        assert_eq!(
            AgyCapabilityGate::admit_profile(&candidate_bytes).err(),
            Some(AgyAdmissionGateError::NoSupportedAdmittedProfileVersion)
        );
        let packet = workspace.owner_review_packet().expect("packet builds");
        assert_eq!(packet.decision, "pending_owner_g64_review");
        assert!(!packet.provider_enabled);

        let all = fs::read_dir(&root.0)
            .expect("workspace reads")
            .flat_map(|entry| fs::read(entry.expect("entry").path()).expect("artifact reads"))
            .collect::<Vec<_>>();
        let serialized = String::from_utf8_lossy(&all);
        for forbidden in [
            "private-id",
            "private-hook",
            "C:/private",
            "private prompt",
            "transcript_path",
            "toolCall",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "artifact leaked {forbidden}"
            );
        }
        workspace.clean().expect("managed cleanup");
        assert!(!root.0.exists());
    }

    #[test]
    fn cleanup_refuses_an_unmanaged_directory() {
        let root = TestRoot::new("unmanaged");
        fs::create_dir_all(&root.0).expect("unmanaged directory creates");
        let workspace = AgyQualificationWorkspace::new(root.0.clone());
        assert_eq!(
            workspace.initialize(),
            Err(AgyQualificationWorkspaceError::UnsafeLocation)
        );
        assert_eq!(
            workspace.clean(),
            Err(AgyQualificationWorkspaceError::NotInitialized)
        );
        assert!(root.0.exists());
    }

    #[test]
    fn cleanup_refuses_foreign_content_without_deleting_any_artifact() {
        let root = TestRoot::new("foreign-content");
        let workspace = AgyQualificationWorkspace::new(root.0.clone());
        workspace.initialize().expect("workspace initializes");
        workspace
            .record_version_probe(&probe())
            .expect("probe records");
        let marker_before = fs::read(root.0.join("run.json")).expect("marker reads");
        fs::write(root.0.join("foreign.txt"), b"must survive").expect("foreign file writes");

        assert_eq!(
            workspace.clone().clean(),
            Err(AgyQualificationWorkspaceError::InvalidManagedWorkspace)
        );
        assert_eq!(
            fs::read(root.0.join("foreign.txt")).expect("foreign file survives"),
            b"must survive"
        );
        assert_eq!(
            fs::read(root.0.join("run.json")).expect("marker survives"),
            marker_before
        );
        assert!(root.0.join("version.json").exists());

        fs::remove_file(root.0.join("foreign.txt")).expect("foreign fixture removes");
        workspace.clean().expect("allow-listed cleanup succeeds");
        assert!(!root.0.exists());
    }

    #[test]
    fn initialization_requires_an_absolute_dedicated_leaf() {
        let relative = AgyQualificationWorkspace::new(PathBuf::from("qualification"));
        assert_eq!(
            relative.initialize(),
            Err(AgyQualificationWorkspaceError::UnsafeLocation)
        );

        let root = TestRoot::new("parent");
        let ordinary = root.0.join("ordinary");
        let workspace = AgyQualificationWorkspace::new(ordinary.clone());
        assert_eq!(
            workspace.initialize(),
            Err(AgyQualificationWorkspaceError::UnsafeLocation)
        );
        assert!(!ordinary.exists());
    }
}
