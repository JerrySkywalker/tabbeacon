use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tabbeacon::core::{
    AgentEvidence, AgentProvider, AgentSessionKey, Attention, AuthoritySet, BackendCapabilities,
    EvidenceAuthority, EvidenceConfidence, EvidenceSource, EvidenceTieBreak, FieldUpdate, Health,
    Phase, SessionReconciler, StateAxis, StatePatch,
};

fn provider(value: &str) -> AgentProvider {
    AgentProvider::new(value).expect("test provider must be valid")
}

fn session(native_session_id: &str) -> AgentSessionKey {
    AgentSessionKey::new(provider("future-agent"), native_session_id)
        .expect("test session must be valid")
}

fn source(backend: &str, instance: &str) -> EvidenceSource {
    EvidenceSource::new(backend, instance).expect("test source must be valid")
}

fn tie_break(value: &str) -> EvidenceTieBreak {
    EvidenceTieBreak::new(value).expect("test tie-break must be valid")
}

fn observed_at(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(seconds)
}

fn evidence(
    session: AgentSessionKey,
    source: EvidenceSource,
    authority: EvidenceAuthority,
    confidence: EvidenceConfidence,
    seconds: u64,
    tie_break_value: &str,
    patch: StatePatch,
) -> AgentEvidence {
    AgentEvidence::new(
        session,
        source,
        authority,
        confidence,
        observed_at(seconds),
        tie_break(tie_break_value),
        patch,
    )
}

#[test]
fn provider_identity_is_open_ended_and_checked() {
    let future = provider("future-agent-v9");
    assert_eq!(future.as_str(), "future-agent-v9");
    assert!(AgentProvider::new("   ").is_err());
}

#[test]
fn session_key_equality_hash_and_stability_use_only_provider_and_native_id() {
    let first = AgentSessionKey::new(provider("future-agent"), "native-42").unwrap();
    let same = AgentSessionKey::new(provider("future-agent"), "native-42").unwrap();
    let distinct = AgentSessionKey::new(provider("future-agent"), "native-43").unwrap();

    assert_eq!(first, same);
    assert_ne!(first, distinct);
    assert_eq!(hash_of(&first), hash_of(&same));
    assert!(AgentSessionKey::new(provider("future-agent"), "").is_err());
}

#[test]
fn backend_capabilities_declare_axis_specific_authority_sets() {
    let capabilities = BackendCapabilities::new(
        AuthoritySet::LIFECYCLE.union(AuthoritySet::AUTHORITATIVE),
        AuthoritySet::LIFECYCLE,
        AuthoritySet::HEURISTIC,
    );

    assert!(capabilities.supports(StateAxis::Phase, EvidenceAuthority::Lifecycle));
    assert!(capabilities.supports(StateAxis::Phase, EvidenceAuthority::Authoritative));
    assert!(!capabilities.supports(StateAxis::Phase, EvidenceAuthority::Heuristic));
    assert!(capabilities.supports(StateAxis::Attention, EvidenceAuthority::Lifecycle));
    assert!(!capabilities.supports(StateAxis::Attention, EvidenceAuthority::Authoritative));
    assert!(capabilities.supports(StateAxis::Health, EvidenceAuthority::Heuristic));
    assert!(!BackendCapabilities::none().supports(StateAxis::Health, EvidenceAuthority::Heuristic));
}

#[test]
fn phase_attention_and_health_are_orthogonal() {
    let key = session("orthogonal");
    let state = StatePatch {
        phase: FieldUpdate::set(Phase::Working),
        attention: FieldUpdate::set(Attention::Question),
        health: FieldUpdate::set(Health::Warning),
    };
    let mut reconciler = SessionReconciler::default();
    let snapshot = reconciler.apply(&evidence(
        key,
        source("normalized", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        1,
        "1",
        state,
    ));

    assert_eq!(snapshot.phase(), Phase::Working);
    assert_eq!(snapshot.attention(), Attention::Question);
    assert_eq!(snapshot.health(), Health::Warning);
}

#[test]
fn state_patch_distinguishes_unchanged_set_and_clear() {
    let patch = StatePatch::unchanged();
    assert_eq!(patch.phase, FieldUpdate::Unchanged);
    assert_eq!(patch.attention, FieldUpdate::Unchanged);
    assert_eq!(patch.health, FieldUpdate::Unchanged);
    assert_ne!(FieldUpdate::set(Attention::Approval), FieldUpdate::clear());
}

#[test]
fn explicit_attention_clear_resets_only_attention_and_records_provenance() {
    let key = session("attention-clear");
    let mut reconciler = SessionReconciler::default();
    reconciler.apply(&evidence(
        key.clone(),
        source("normalized", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        1,
        "approval",
        StatePatch {
            phase: FieldUpdate::set(Phase::Working),
            attention: FieldUpdate::set(Attention::Approval),
            health: FieldUpdate::set(Health::Warning),
        },
    ));
    let snapshot = reconciler.apply(&evidence(
        key,
        source("normalized", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        2,
        "clear-attention",
        StatePatch {
            attention: FieldUpdate::clear(),
            ..StatePatch::unchanged()
        },
    ));

    assert_eq!(snapshot.phase(), Phase::Working);
    assert_eq!(snapshot.attention(), Attention::None);
    assert_eq!(snapshot.health(), Health::Warning);
    assert_eq!(
        snapshot
            .attention_provenance()
            .map(tabbeacon::core::EvidenceProvenance::observed_at),
        Some(observed_at(2))
    );
}

#[test]
fn older_evidence_cannot_replace_a_fresher_winner() {
    let key = session("freshness");
    let mut reconciler = SessionReconciler::default();
    reconciler.apply(&evidence(
        key.clone(),
        source("normalized", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        20,
        "newer",
        StatePatch {
            phase: FieldUpdate::set(Phase::Working),
            ..StatePatch::unchanged()
        },
    ));
    let snapshot = reconciler.apply(&evidence(
        key,
        source("normalized", "one"),
        EvidenceAuthority::Authoritative,
        EvidenceConfidence::High,
        10,
        "older",
        StatePatch {
            phase: FieldUpdate::set(Phase::Ready),
            ..StatePatch::unchanged()
        },
    ));

    assert_eq!(snapshot.phase(), Phase::Working);
    assert_eq!(
        snapshot
            .phase_provenance()
            .map(|provenance| provenance.tie_break().as_str()),
        Some("newer")
    );
}

#[test]
fn higher_authority_wins_at_an_equal_timestamp() {
    let key = session("authority-tie");
    let mut reconciler = SessionReconciler::default();
    reconciler.apply(&evidence(
        key.clone(),
        source("z-backend", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::High,
        30,
        "z",
        StatePatch {
            health: FieldUpdate::set(Health::Warning),
            ..StatePatch::unchanged()
        },
    ));
    let snapshot = reconciler.apply(&evidence(
        key,
        source("a-backend", "one"),
        EvidenceAuthority::Authoritative,
        EvidenceConfidence::Low,
        30,
        "a",
        StatePatch {
            health: FieldUpdate::set(Health::Normal),
            ..StatePatch::unchanged()
        },
    ));

    assert_eq!(snapshot.health(), Health::Normal);
    assert_eq!(
        snapshot
            .health_provenance()
            .map(tabbeacon::core::EvidenceProvenance::authority),
        Some(EvidenceAuthority::Authoritative)
    );
}

#[test]
fn higher_confidence_wins_before_source_order_at_an_equal_timestamp() {
    let key = session("confidence-tie");
    let mut reconciler = SessionReconciler::default();
    reconciler.apply(&evidence(
        key.clone(),
        source("z-backend", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Low,
        35,
        "z",
        StatePatch {
            health: FieldUpdate::set(Health::Warning),
            ..StatePatch::unchanged()
        },
    ));
    let snapshot = reconciler.apply(&evidence(
        key,
        source("a-backend", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::High,
        35,
        "a",
        StatePatch {
            health: FieldUpdate::set(Health::Normal),
            ..StatePatch::unchanged()
        },
    ));

    assert_eq!(snapshot.health(), Health::Normal);
    assert_eq!(
        snapshot
            .health_provenance()
            .map(tabbeacon::core::EvidenceProvenance::confidence),
        Some(EvidenceConfidence::High)
    );
}

#[test]
fn source_tie_break_and_patch_order_are_final_deterministic_ties() {
    let key = session("final-tie");
    let low_source = evidence(
        key.clone(),
        source("a-backend", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        40,
        "a",
        StatePatch {
            attention: FieldUpdate::set(Attention::Question),
            ..StatePatch::unchanged()
        },
    );
    let high_source = evidence(
        key.clone(),
        source("z-backend", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        40,
        "a",
        StatePatch {
            attention: FieldUpdate::set(Attention::Approval),
            ..StatePatch::unchanged()
        },
    );

    let mut forward = SessionReconciler::default();
    forward.apply(&low_source);
    let forward_snapshot = forward.apply(&high_source);
    let mut reverse = SessionReconciler::default();
    reverse.apply(&high_source);
    let reverse_snapshot = reverse.apply(&low_source);
    assert_eq!(forward_snapshot, reverse_snapshot);
    assert_eq!(forward_snapshot.attention(), Attention::Approval);

    let higher_tie_break = evidence(
        key.clone(),
        source("z-backend", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        40,
        "z",
        StatePatch {
            attention: FieldUpdate::set(Attention::ResultReady),
            ..StatePatch::unchanged()
        },
    );
    let after_tie_break = forward.apply(&higher_tie_break);
    assert_eq!(after_tie_break.attention(), Attention::ResultReady);

    let clear_collision = evidence(
        key,
        source("z-backend", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        40,
        "z",
        StatePatch {
            attention: FieldUpdate::clear(),
            ..StatePatch::unchanged()
        },
    );
    let after_collision = forward.apply(&clear_collision);
    assert_eq!(after_collision.attention(), Attention::ResultReady);
}

#[test]
fn stale_attention_cannot_revive_after_a_newer_clear() {
    let key = session("stale-attention");
    let mut reconciler = SessionReconciler::default();
    reconciler.apply(&evidence(
        key.clone(),
        source("normalized", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        50,
        "approval",
        StatePatch {
            attention: FieldUpdate::set(Attention::Approval),
            ..StatePatch::unchanged()
        },
    ));
    reconciler.apply(&evidence(
        key.clone(),
        source("normalized", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        60,
        "clear",
        StatePatch {
            attention: FieldUpdate::clear(),
            ..StatePatch::unchanged()
        },
    ));
    let snapshot = reconciler.apply(&evidence(
        key,
        source("normalized", "two"),
        EvidenceAuthority::Authoritative,
        EvidenceConfidence::High,
        55,
        "late-arrival-stale",
        StatePatch {
            attention: FieldUpdate::set(Attention::Approval),
            ..StatePatch::unchanged()
        },
    ));

    assert_eq!(snapshot.attention(), Attention::None);
    assert_eq!(
        snapshot
            .attention_provenance()
            .map(|provenance| provenance.tie_break().as_str()),
        Some("clear")
    );
}

#[test]
fn weaker_later_heuristic_cannot_override_lifecycle_health() {
    let key = session("weaker-authority");
    let mut reconciler = SessionReconciler::default();
    reconciler.apply(&evidence(
        key.clone(),
        source("normalized", "lifecycle"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        70,
        "lifecycle-warning",
        StatePatch {
            health: FieldUpdate::set(Health::Warning),
            ..StatePatch::unchanged()
        },
    ));
    let snapshot = reconciler.apply(&evidence(
        key,
        source("normalized", "heuristic"),
        EvidenceAuthority::Heuristic,
        EvidenceConfidence::High,
        80,
        "heuristic-normal",
        StatePatch {
            health: FieldUpdate::set(Health::Normal),
            ..StatePatch::unchanged()
        },
    ));

    assert_eq!(snapshot.health(), Health::Warning);
}

#[test]
fn independent_fields_support_representative_state_chains() {
    let key = session("state-chains");
    let mut reconciler = SessionReconciler::default();
    reconciler.apply(&evidence(
        key.clone(),
        source("normalized", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        90,
        "working",
        StatePatch {
            phase: FieldUpdate::set(Phase::Working),
            health: FieldUpdate::set(Health::Normal),
            ..StatePatch::unchanged()
        },
    ));
    reconciler.apply(&evidence(
        key.clone(),
        source("normalized", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        91,
        "waiting-approval",
        StatePatch {
            phase: FieldUpdate::set(Phase::WaitingUser),
            attention: FieldUpdate::set(Attention::Approval),
            ..StatePatch::unchanged()
        },
    ));
    let after_clear = reconciler.apply(&evidence(
        key.clone(),
        source("normalized", "one"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        92,
        "working-normal",
        StatePatch {
            phase: FieldUpdate::set(Phase::Working),
            attention: FieldUpdate::clear(),
            health: FieldUpdate::set(Health::Normal),
        },
    ));
    assert_eq!(after_clear.phase(), Phase::Working);
    assert_eq!(after_clear.attention(), Attention::None);
    assert_eq!(after_clear.health(), Health::Normal);

    let warning = reconciler.apply(&evidence(
        key.clone(),
        source("normalized", "two"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        93,
        "working-warning",
        StatePatch {
            health: FieldUpdate::set(Health::Warning),
            ..StatePatch::unchanged()
        },
    ));
    assert_eq!(warning.phase(), Phase::Working);
    assert_eq!(warning.attention(), Attention::None);
    assert_eq!(warning.health(), Health::Warning);

    let result_ready = reconciler.apply(&evidence(
        key,
        source("normalized", "two"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        94,
        "waiting-result",
        StatePatch {
            phase: FieldUpdate::set(Phase::WaitingUser),
            attention: FieldUpdate::set(Attention::ResultReady),
            health: FieldUpdate::set(Health::Normal),
        },
    ));
    assert_eq!(result_ready.phase(), Phase::WaitingUser);
    assert_eq!(result_ready.attention(), Attention::ResultReady);
    assert_eq!(result_ready.health(), Health::Normal);
}

#[test]
fn repeated_identical_evidence_is_idempotent_and_multi_source_updates_are_independent() {
    let key = session("idempotent");
    let phase_evidence = evidence(
        key.clone(),
        source("normalized", "phase"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        100,
        "phase",
        StatePatch {
            phase: FieldUpdate::set(Phase::Working),
            ..StatePatch::unchanged()
        },
    );
    let health_evidence = evidence(
        key.clone(),
        source("normalized", "health"),
        EvidenceAuthority::Lifecycle,
        EvidenceConfidence::Standard,
        101,
        "health",
        StatePatch {
            health: FieldUpdate::set(Health::Warning),
            ..StatePatch::unchanged()
        },
    );
    let mut reconciler = SessionReconciler::default();
    let first = reconciler.apply(&phase_evidence);
    let second = reconciler.apply(&phase_evidence);
    let final_snapshot = reconciler.apply(&health_evidence);

    assert_eq!(first, second);
    assert_eq!(final_snapshot.phase(), Phase::Working);
    assert_eq!(final_snapshot.health(), Health::Warning);
    assert_eq!(reconciler.session_count(), 1);
    assert_eq!(reconciler.snapshot(&key), Some(&final_snapshot));
}

fn hash_of(value: &AgentSessionKey) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}
