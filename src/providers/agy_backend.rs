//! Capability-gated Agy production-adapter foundation.
//!
//! Every public constructor in this module keeps Agy unadmitted. The future
//! G64 conversion point is deliberately explicit and currently rejects every
//! document because no real Owner-reviewed admitted profile version exists.

use std::fmt;

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::agy::{MAX_AGY_QUALIFICATION_INPUT_BYTES, parse_qualification_object};

/// Daily Agy launch remains the provider's literal native command.
pub const AGY_DAILY_COMMAND: &str = "agy";
/// Absolute production hard gate before real Owner G64 admission.
pub const AGY_PROVIDER: bool = false;

/// Stable future profile family. No version in this family is admitted today.
pub const AGY_ADMITTED_PROFILE_FAMILY: &str = "tabbeacon-agy-admitted-profile";

/// Production backend state before real G64 admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyBackendState {
    Unadmitted,
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
}

/// Unforgeable admitted profile token. No public constructor exists.
pub struct AgyAdmittedProfile {
    _private: (),
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

/// Single explicit boundary for a future versioned Owner-approved profile.
pub struct AgyCapabilityGate;

impl AgyCapabilityGate {
    /// Rejects every candidate today after bounded structural validation.
    ///
    /// A later G64 change must add one exact schema-version branch here and
    /// construct the otherwise unforgeable [`AgyAdmittedProfile`] token.
    ///
    /// # Errors
    ///
    /// Returns a bounded rejection; no document can enable Agy today.
    pub fn admit_profile(bytes: &[u8]) -> Result<AgyAdmittedProfile, AgyAdmissionGateError> {
        if bytes.len() > MAX_AGY_QUALIFICATION_INPUT_BYTES {
            return Err(AgyAdmissionGateError::Oversized);
        }
        if parse_qualification_object(bytes).is_none() {
            return Err(AgyAdmissionGateError::Malformed);
        }
        Err(AgyAdmissionGateError::NoSupportedAdmittedProfileVersion)
    }

    /// Projects normalized data only when an unforgeable admitted token exists.
    #[must_use]
    pub const fn project(
        _profile: &AgyAdmittedProfile,
        _observation: AgyNormalizedObservation,
    ) -> AgyEvidenceProjection {
        // The only branch remains unavailable until G64 defines mappings.
        AgyEvidenceProjection::Unadmitted
    }
}

/// Production adapter object whose only constructible state is unadmitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgyBackend {
    pub state: AgyBackendState,
    pub source: AgyBackendSource,
}

impl Default for AgyBackend {
    fn default() -> Self {
        Self {
            state: AgyBackendState::Unadmitted,
            source: AgyBackendSource::Unselected,
        }
    }
}

impl AgyBackend {
    /// Agy cannot participate in production until a later admitted token path exists.
    #[must_use]
    pub const fn provider_enabled(self) -> bool {
        AGY_PROVIDER
    }

    /// The runtime remains fail open because no unadmitted observation is dispatched.
    #[must_use]
    pub const fn observe_fail_open(
        self,
        _observation: AgyNormalizedObservation,
    ) -> AgyEvidenceProjection {
        AgyEvidenceProjection::Unadmitted
    }
}

/// Root Workspace Anchor handoff remains blocked before admission.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyRootAnchorBoundary {
    Unadmitted,
}

/// Typed management states prepared for later G65 activation.
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
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgyReadinessProjection {
    pub state: AgyIntegrationReadiness,
    pub qualification_available: bool,
    pub qualification_observations_available: bool,
    pub production_enabled: bool,
}

impl AgyReadinessProjection {
    /// Builds today's unadmitted readiness state.
    #[must_use]
    pub const fn unadmitted(qualification_observations_available: bool) -> Self {
        Self {
            state: AgyIntegrationReadiness::KnownUnadmitted,
            qualification_available: true,
            qualification_observations_available,
            production_enabled: false,
        }
    }
}

/// Production setup is unavailable without a real admitted setup profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgyProductionSetupError {
    NoAdmittedAgySetupProfile,
}

impl fmt::Display for AgyProductionSetupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("no admitted Agy setup profile")
    }
}

impl std::error::Error for AgyProductionSetupError {}

/// Production setup facade that refuses rather than guessing a config location.
pub struct AgyProductionSetup;

impl AgyProductionSetup {
    /// # Errors
    ///
    /// Always returns [`AgyProductionSetupError::NoAdmittedAgySetupProfile`] today.
    pub const fn preview() -> Result<(), AgyProductionSetupError> {
        Err(AgyProductionSetupError::NoAdmittedAgySetupProfile)
    }
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
    use serde_json::{Value, json};

    use super::{
        AGY_DAILY_COMMAND, AgyAdmissionGateError, AgyBackend, AgyCandidatePresence,
        AgyCapabilityGate, AgyEvidenceProjection, AgyNormalizer, AgyOwnedConfigPatch,
        AgyProductionSetup, AgyProductionSetupError, AgyReadinessProjection, AgySafeRawObservation,
        AgySetupScope, AgySetupTransaction, AgySetupTransactionError,
    };

    #[test]
    fn every_forged_or_candidate_profile_remains_unadmitted() {
        for document in [
            br"{}".as_slice(),
            br#"{"schema":"tabbeacon-agy-admitted-profile","version":1,"provider_enabled":true}"#,
            br#"{"admission":"admitted","capabilities":["all"]}"#,
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
        assert!(!AgyBackend::default().provider_enabled());
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
    fn production_setup_refuses_without_an_admitted_profile() {
        assert_eq!(
            AgyProductionSetup::preview(),
            Err(AgyProductionSetupError::NoAdmittedAgySetupProfile)
        );
        let readiness = AgyReadinessProjection::unadmitted(true);
        assert!(readiness.qualification_available);
        assert!(readiness.qualification_observations_available);
        assert!(!readiness.production_enabled);
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
}
