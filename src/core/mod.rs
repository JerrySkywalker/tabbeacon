//! Provider-neutral evidence types and deterministic per-session reconciliation.
//!
//! Providers normalize their observations before constructing [`AgentEvidence`].
//! This module deliberately contains no provider transport, repository, process,
//! or terminal concerns.

use std::{collections::BTreeMap, fmt, time::SystemTime};

/// Error returned when a stable core identifier is empty or whitespace only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidIdentifier {
    kind: &'static str,
}

impl InvalidIdentifier {
    /// Returns the rejected identifier category.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        self.kind
    }
}

impl fmt::Display for InvalidIdentifier {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{0} must not be empty", self.kind)
    }
}

impl std::error::Error for InvalidIdentifier {}

/// An open, provider-neutral provider identifier.
///
/// The core intentionally does not enumerate providers. A future adapter can
/// introduce its own stable identifier without changing this module.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AgentProvider(String);

impl AgentProvider {
    /// Creates a non-empty provider identifier.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentifier`] when `value` is empty or whitespace only.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidIdentifier> {
        checked_identifier(value, "provider").map(Self)
    }

    /// Returns the stable provider identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A canonical session key: provider plus its native, non-empty session ID.
///
/// Process IDs, current directories, repositories, terminal tabs, and titles
/// are bindings or metadata owned outside this core contract.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AgentSessionKey {
    provider: AgentProvider,
    native_session_id: String,
}

impl AgentSessionKey {
    /// Creates a session key from a provider and native session identifier.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentifier`] when `native_session_id` is empty or
    /// whitespace only.
    pub fn new(
        provider: AgentProvider,
        native_session_id: impl Into<String>,
    ) -> Result<Self, InvalidIdentifier> {
        Ok(Self {
            provider,
            native_session_id: checked_identifier(native_session_id, "native session ID")?,
        })
    }

    /// Returns the provider component of this canonical key.
    #[must_use]
    pub const fn provider(&self) -> &AgentProvider {
        &self.provider
    }

    /// Returns the provider-native session identifier.
    #[must_use]
    pub fn native_session_id(&self) -> &str {
        &self.native_session_id
    }
}

/// A normalized identity for the backend instance that produced evidence.
///
/// `backend` and `instance` are intentionally opaque to the core. They are
/// stable ordering components, not provider event names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceSource {
    backend: String,
    instance: String,
}

impl EvidenceSource {
    /// Creates a non-empty backend and instance identity.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentifier`] when either identifier is empty or
    /// whitespace only.
    pub fn new(
        backend: impl Into<String>,
        instance: impl Into<String>,
    ) -> Result<Self, InvalidIdentifier> {
        Ok(Self {
            backend: checked_identifier(backend, "evidence backend")?,
            instance: checked_identifier(instance, "evidence source instance")?,
        })
    }

    /// Returns the normalized backend identifier.
    #[must_use]
    pub fn backend(&self) -> &str {
        &self.backend
    }

    /// Returns the stable backend-instance identifier.
    #[must_use]
    pub fn instance(&self) -> &str {
        &self.instance
    }
}

/// A stable, provider-neutral key used to order same-source observations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceTieBreak(String);

impl EvidenceTieBreak {
    /// Creates a non-empty stable tie-break key.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidIdentifier`] when `value` is empty or whitespace only.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidIdentifier> {
        checked_identifier(value, "evidence tie-break key").map(Self)
    }

    /// Returns the stable tie-break key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Semantic strength of a normalized observation.
///
/// Higher variants outrank lower variants only when the candidate is not
/// older than the current winning observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceAuthority {
    /// An inference that must not masquerade as confirmed runtime state.
    Heuristic,
    /// A provider-emitted lifecycle observation.
    Lifecycle,
    /// Direct runtime state from an authoritative source.
    Authoritative,
}

impl EvidenceAuthority {
    const fn capability_bit(self) -> u8 {
        match self {
            Self::Heuristic => 0b001,
            Self::Lifecycle => 0b010,
            Self::Authoritative => 0b100,
        }
    }
}

/// Quality of an observation within the same authority class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceConfidence {
    /// The source supplied a weak but still classified observation.
    Low,
    /// The source supplied its normal expected quality.
    Standard,
    /// The source supplied additional corroboration or precision.
    High,
}

/// A compact set of authority classes that a backend may assert for an axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AuthoritySet(u8);

impl AuthoritySet {
    /// No assertion authority is supported.
    pub const NONE: Self = Self(0);
    /// Only heuristic observations are supported.
    pub const HEURISTIC: Self = Self(EvidenceAuthority::Heuristic.capability_bit());
    /// Only lifecycle observations are supported.
    pub const LIFECYCLE: Self = Self(EvidenceAuthority::Lifecycle.capability_bit());
    /// Only authoritative runtime observations are supported.
    pub const AUTHORITATIVE: Self = Self(EvidenceAuthority::Authoritative.capability_bit());
    /// Every defined authority class is supported.
    pub const ALL: Self = Self(Self::HEURISTIC.0 | Self::LIFECYCLE.0 | Self::AUTHORITATIVE.0);

    /// Returns the union of two authority sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns whether this set includes `authority`.
    #[must_use]
    pub const fn contains(self, authority: EvidenceAuthority) -> bool {
        self.0 & authority.capability_bit() != 0
    }
}

/// A semantic state axis that a backend can declare support for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StateAxis {
    /// The lifecycle phase axis.
    Phase,
    /// The user-attention axis.
    Attention,
    /// The health axis.
    Health,
}

/// Declared evidence authority support for a provider backend.
///
/// Capability declarations describe a backend's possible claims. They never
/// grant authority to an individual [`AgentEvidence`] record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendCapabilities {
    phase: AuthoritySet,
    attention: AuthoritySet,
    health: AuthoritySet,
}

impl BackendCapabilities {
    /// Creates a capability declaration for every semantic state axis.
    #[must_use]
    pub const fn new(phase: AuthoritySet, attention: AuthoritySet, health: AuthoritySet) -> Self {
        Self {
            phase,
            attention,
            health,
        }
    }

    /// Returns a declaration with no supported assertions.
    #[must_use]
    pub const fn none() -> Self {
        Self::new(AuthoritySet::NONE, AuthoritySet::NONE, AuthoritySet::NONE)
    }

    /// Returns the authority set declared for `axis`.
    #[must_use]
    pub const fn authorities_for(self, axis: StateAxis) -> AuthoritySet {
        match axis {
            StateAxis::Phase => self.phase,
            StateAxis::Attention => self.attention,
            StateAxis::Health => self.health,
        }
    }

    /// Returns whether `axis` may be asserted at `authority`.
    #[must_use]
    pub const fn supports(self, axis: StateAxis, authority: EvidenceAuthority) -> bool {
        self.authorities_for(axis).contains(authority)
    }
}

/// Coarse lifecycle state, independent from attention and health.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Phase {
    /// No active work is known.
    #[default]
    Ready,
    /// Agent work is in progress.
    Working,
    /// The agent is waiting for user input.
    WaitingUser,
    /// The observed session has ended.
    Ended,
}

/// User attention state, independent from lifecycle phase and health.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Attention {
    /// No outstanding user attention is known.
    #[default]
    None,
    /// A result is available for the user.
    ResultReady,
    /// The agent needs an approval decision.
    Approval,
    /// The agent needs an answer to a question.
    Question,
}

/// Observed health, independent from lifecycle phase and attention.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Health {
    /// No adverse health condition is known.
    #[default]
    Normal,
    /// An evidence-backed warning is active.
    Warning,
    /// The session was interrupted.
    Interrupted,
    /// The session failed.
    Failed,
}

/// An explicit update for one state field.
///
/// `Unchanged` means the evidence has no opinion about the field. `Clear` is
/// distinct: it resets the field to its neutral value and becomes that axis's
/// winning provenance. For equal evidence ranks, variants are ordered as
/// `Unchanged < Clear < Set(value)` to provide a final deterministic fallback.
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FieldUpdate<T> {
    /// Do not modify this field.
    #[default]
    Unchanged,
    /// Explicitly clear this field to its neutral value.
    Clear,
    /// Set this field to the supplied value.
    Set(T),
}

impl<T> FieldUpdate<T> {
    /// Creates an unchanged field update.
    #[must_use]
    pub const fn unchanged() -> Self {
        Self::Unchanged
    }

    /// Creates an explicit clear field update.
    #[must_use]
    pub const fn clear() -> Self {
        Self::Clear
    }

    /// Creates a field update that sets `value`.
    #[must_use]
    pub const fn set(value: T) -> Self {
        Self::Set(value)
    }
}

/// A provider-neutral, field-independent semantic state update.
#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StatePatch {
    /// Proposed phase update.
    pub phase: FieldUpdate<Phase>,
    /// Proposed attention update.
    pub attention: FieldUpdate<Attention>,
    /// Proposed health update.
    pub health: FieldUpdate<Health>,
}

impl StatePatch {
    /// Creates a patch that leaves every field unchanged.
    #[must_use]
    pub const fn unchanged() -> Self {
        Self {
            phase: FieldUpdate::Unchanged,
            attention: FieldUpdate::Unchanged,
            health: FieldUpdate::Unchanged,
        }
    }
}

/// Normalized evidence from a backend, ready for core reconciliation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentEvidence {
    /// Canonical target session.
    pub session: AgentSessionKey,
    /// Stable identity of the normalized evidence source.
    pub source: EvidenceSource,
    /// Semantic strength of the observation.
    pub authority: EvidenceAuthority,
    /// Quality within the authority class.
    pub confidence: EvidenceConfidence,
    /// Time at which the source observed this state.
    pub observed_at: SystemTime,
    /// Stable same-source ordering key.
    pub tie_break: EvidenceTieBreak,
    /// Independent state-axis updates.
    pub patch: StatePatch,
}

impl AgentEvidence {
    /// Creates normalized evidence for reconciliation.
    #[must_use]
    pub const fn new(
        session: AgentSessionKey,
        source: EvidenceSource,
        authority: EvidenceAuthority,
        confidence: EvidenceConfidence,
        observed_at: SystemTime,
        tie_break: EvidenceTieBreak,
        patch: StatePatch,
    ) -> Self {
        Self {
            session,
            source,
            authority,
            confidence,
            observed_at,
            tie_break,
            patch,
        }
    }
}

/// Provenance retained for the evidence that currently wins one state axis.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EvidenceProvenance {
    source: EvidenceSource,
    authority: EvidenceAuthority,
    confidence: EvidenceConfidence,
    observed_at: SystemTime,
    tie_break: EvidenceTieBreak,
}

impl EvidenceProvenance {
    /// Returns the source of the winning evidence.
    #[must_use]
    pub const fn source(&self) -> &EvidenceSource {
        &self.source
    }

    /// Returns the authority of the winning evidence.
    #[must_use]
    pub const fn authority(&self) -> EvidenceAuthority {
        self.authority
    }

    /// Returns the confidence of the winning evidence.
    #[must_use]
    pub const fn confidence(&self) -> EvidenceConfidence {
        self.confidence
    }

    /// Returns the observation time of the winning evidence.
    #[must_use]
    pub const fn observed_at(&self) -> SystemTime {
        self.observed_at
    }

    /// Returns the stable tie-break key of the winning evidence.
    #[must_use]
    pub const fn tie_break(&self) -> &EvidenceTieBreak {
        &self.tie_break
    }
}

impl From<&AgentEvidence> for EvidenceProvenance {
    fn from(evidence: &AgentEvidence) -> Self {
        Self {
            source: evidence.source.clone(),
            authority: evidence.authority,
            confidence: evidence.confidence,
            observed_at: evidence.observed_at,
            tie_break: evidence.tie_break.clone(),
        }
    }
}

/// Reconciled state and independent winning provenance for one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSnapshot {
    session: AgentSessionKey,
    phase: AxisState<Phase>,
    attention: AxisState<Attention>,
    health: AxisState<Health>,
}

impl SessionSnapshot {
    fn new(session: AgentSessionKey) -> Self {
        Self {
            session,
            phase: AxisState::neutral(Phase::Ready),
            attention: AxisState::neutral(Attention::None),
            health: AxisState::neutral(Health::Normal),
        }
    }

    /// Returns the canonical session key.
    #[must_use]
    pub const fn session(&self) -> &AgentSessionKey {
        &self.session
    }

    /// Returns the reconciled lifecycle phase.
    #[must_use]
    pub const fn phase(&self) -> Phase {
        self.phase.value
    }

    /// Returns the reconciled attention state.
    #[must_use]
    pub const fn attention(&self) -> Attention {
        self.attention.value
    }

    /// Returns the reconciled health state.
    #[must_use]
    pub const fn health(&self) -> Health {
        self.health.value
    }

    /// Returns provenance for the current phase winner, if evidence set it.
    #[must_use]
    pub const fn phase_provenance(&self) -> Option<&EvidenceProvenance> {
        self.phase.provenance.as_ref()
    }

    /// Returns provenance for the current attention winner, if evidence set it.
    #[must_use]
    pub const fn attention_provenance(&self) -> Option<&EvidenceProvenance> {
        self.attention.provenance.as_ref()
    }

    /// Returns provenance for the current health winner, if evidence set it.
    #[must_use]
    pub const fn health_provenance(&self) -> Option<&EvidenceProvenance> {
        self.health.provenance.as_ref()
    }
}

/// Deterministic per-session state reconciler.
///
/// Internally this uses an ordered map so session storage order cannot affect a
/// result. Within a session each axis is reconciled independently.
#[derive(Debug, Default)]
pub struct SessionReconciler {
    sessions: BTreeMap<AgentSessionKey, SessionSnapshot>,
}

impl SessionReconciler {
    /// Reconciles one normalized evidence record and returns its snapshot.
    pub fn apply(&mut self, evidence: &AgentEvidence) -> SessionSnapshot {
        let provenance = EvidenceProvenance::from(evidence);
        let snapshot = self
            .sessions
            .entry(evidence.session.clone())
            .or_insert_with(|| SessionSnapshot::new(evidence.session.clone()));

        apply_axis(
            &mut snapshot.phase,
            &evidence.patch.phase,
            Phase::Ready,
            &provenance,
        );
        apply_axis(
            &mut snapshot.attention,
            &evidence.patch.attention,
            Attention::None,
            &provenance,
        );
        apply_axis(
            &mut snapshot.health,
            &evidence.patch.health,
            Health::Normal,
            &provenance,
        );

        snapshot.clone()
    }

    /// Returns a previously reconciled session snapshot.
    #[must_use]
    pub fn snapshot(&self, session: &AgentSessionKey) -> Option<&SessionSnapshot> {
        self.sessions.get(session)
    }

    /// Returns the number of sessions with at least one applied evidence record.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AxisState<T> {
    value: T,
    provenance: Option<EvidenceProvenance>,
    winning_update: Option<FieldUpdate<T>>,
}

impl<T> AxisState<T> {
    const fn neutral(value: T) -> Self {
        Self {
            value,
            provenance: None,
            winning_update: None,
        }
    }
}

fn apply_axis<T>(
    axis: &mut AxisState<T>,
    update: &FieldUpdate<T>,
    neutral: T,
    candidate: &EvidenceProvenance,
) where
    T: Clone + Ord,
{
    if matches!(update, FieldUpdate::Unchanged) {
        return;
    }

    let replaces_winner = match (&axis.provenance, &axis.winning_update) {
        (Some(winner), Some(winning_update)) => {
            candidate_replaces(candidate, update, winner, winning_update)
        }
        _ => true,
    };

    if replaces_winner {
        axis.value = match update {
            FieldUpdate::Unchanged => return,
            FieldUpdate::Clear => neutral,
            FieldUpdate::Set(value) => value.clone(),
        };
        axis.provenance = Some(candidate.clone());
        axis.winning_update = Some(update.clone());
    }
}

fn candidate_replaces<T>(
    candidate: &EvidenceProvenance,
    candidate_update: &FieldUpdate<T>,
    winner: &EvidenceProvenance,
    winning_update: &FieldUpdate<T>,
) -> bool
where
    T: Ord,
{
    if candidate.observed_at < winner.observed_at || candidate.authority < winner.authority {
        return false;
    }

    candidate
        .observed_at
        .cmp(&winner.observed_at)
        .then(candidate.authority.cmp(&winner.authority))
        .then(candidate.confidence.cmp(&winner.confidence))
        .then(candidate.source.cmp(&winner.source))
        .then(candidate.tie_break.cmp(&winner.tie_break))
        .then(candidate_update.cmp(winning_update))
        .is_gt()
}

fn checked_identifier(
    value: impl Into<String>,
    kind: &'static str,
) -> Result<String, InvalidIdentifier> {
    let value = value.into();
    if value.trim().is_empty() {
        Err(InvalidIdentifier { kind })
    } else {
        Ok(value)
    }
}
