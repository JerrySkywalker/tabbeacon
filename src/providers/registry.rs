//! Provider-neutral, read-only integration management projections.
//!
//! A provider identifier alone never grants support.  This registry contains
//! only adapters registered by the product and projects their admitted
//! observation evidence into a bounded management model. v0.5.1 registers
//! Codex for production. Explicit qualification callers may add an unadmitted
//! Agy candidate without making it part of the ordinary product view.

use std::fmt;

use serde::Serialize;

use crate::{
    diagnostics::OperationalDiagnostics,
    hook_inventory::{HookInventory, HookInventoryAvailability},
    providers::agy::{
        AGY_PROVIDER_ID, AgyCapability, AgyCapabilityAvailability, AgyCapabilityProfile, AgyVersion,
    },
    settings::ProviderBadgePolicy,
};

/// Stable schema version for provider-management projections.
pub const PROVIDER_REGISTRY_SCHEMA_VERSION: u32 = 1;

/// A checked, open provider identifier used by the registry boundary.
///
/// The value is deliberately not an enum: a future admitted provider can use
/// a new identifier without changing provider-neutral management code.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ProviderId(String);

/// Rejection reason for an unsafe provider registration identifier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidProviderId;

impl fmt::Display for InvalidProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("provider ID must be a bounded lowercase identifier")
    }
}

impl std::error::Error for InvalidProviderId {}

impl ProviderId {
    /// Creates one checked open provider identifier.
    ///
    /// IDs begin with an ASCII lowercase letter and contain only lowercase
    /// letters, digits, and hyphens. This keeps provider-facing projections
    /// bounded and free of terminal controls.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidProviderId`] when the value is empty, too long, or
    /// contains characters outside the bounded identifier grammar.
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidProviderId> {
        let value = value.into();
        let valid = (1..=48).contains(&value.len())
            && value
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_lowercase())
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            && !value.ends_with('-');
        valid.then_some(Self(value)).ok_or(InvalidProviderId)
    }

    /// Stable provider identifier spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One bounded, read-only observation gathered by a registered provider adapter.
///
/// Probes carry only safe version/admission/ownership facts. They neither
/// execute setup nor expose provider configuration, Hook commands, paths, or
/// native session identifiers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProbe {
    provider: ProviderId,
    version: Option<String>,
    profile_supported: bool,
    installed: bool,
    hooks_available: bool,
}

impl ProviderProbe {
    /// Builds the bounded Codex probe from an existing diagnostics pass.
    #[must_use]
    pub fn codex(
        version: Option<&str>,
        profile_supported: bool,
        installed: bool,
        hooks_available: bool,
    ) -> Self {
        Self {
            provider: ProviderId("codex".to_owned()),
            version: version.map(ToOwned::to_owned),
            profile_supported,
            installed,
            hooks_available,
        }
    }
}

/// Provider capability categories shown by the Integrations surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapability {
    Phase,
    Attention,
    ApprovalQuestion,
    Health,
    SessionIdentity,
    WorkspaceRootBinding,
    Subagents,
    BackgroundTasks,
    TitleOutput,
    WindowsTerminalPresentation,
    HookInspection,
}

impl ProviderCapability {
    /// Stable compact key for the capability matrix.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Phase => "phase",
            Self::Attention => "attention",
            Self::ApprovalQuestion => "approval_question",
            Self::Health => "health",
            Self::SessionIdentity => "session_identity",
            Self::WorkspaceRootBinding => "workspace_root_binding",
            Self::Subagents => "subagents",
            Self::BackgroundTasks => "background_tasks",
            Self::TitleOutput => "title_output",
            Self::WindowsTerminalPresentation => "windows_terminal_presentation",
            Self::HookInspection => "hook_inspection",
        }
    }
}

/// Evidence status of one actual provider capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAvailability {
    /// The registered backend and its admitted profile prove the capability.
    Proven,
    /// The capability is designed but has no current evidence from this provider.
    Unavailable,
    /// The backend does not claim this capability.
    Unsupported,
}

impl CapabilityAvailability {
    /// Stable compact key for Human and machine projections.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proven => "proven",
            Self::Unavailable => "unavailable",
            Self::Unsupported => "unsupported",
        }
    }
}

/// One provider capability and the authority that establishes it.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderCapabilityStatus {
    pub capability: ProviderCapability,
    pub availability: CapabilityAvailability,
    /// A bounded source class, never a provider event payload or configuration.
    pub authority: &'static str,
}

/// Capability profile obtained from one registered provider probe.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderCapabilityProfile {
    pub capabilities: Vec<ProviderCapabilityStatus>,
}

/// Admission state, deliberately separate from availability and installation.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAdmissionState {
    /// The detected provider maps to a source-audited production profile.
    Admitted,
    /// The provider is registered but its detected version/profile is unproven.
    Unknown,
    /// The product knows this candidate but real-provider admission has not occurred.
    Unadmitted,
}

impl ProviderAdmissionState {
    /// Stable compact key for Human and machine projections.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Unknown => "unknown",
            Self::Unadmitted => "unadmitted",
        }
    }
}

/// Whether a provider has a safely inspectable Hook surface.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderHookState {
    Available,
    Unavailable,
    NotApplicable,
}

impl ProviderHookState {
    /// Stable compact key for Human and machine projections.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::NotApplicable => "not_applicable",
        }
    }
}

/// A bounded user action that is never executed by registry inspection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderManualAction {
    InspectCompatibility,
    ReviewHooks,
    OwnerPresentQualification,
}

impl ProviderManualAction {
    /// Stable compact key for Human and machine projections.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InspectCompatibility => "inspect_compatibility",
            Self::ReviewHooks => "review_hooks",
            Self::OwnerPresentQualification => "owner_present_qualification",
        }
    }
}

/// One registered provider's read-only management projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderReadiness {
    pub qualification_available: bool,
    pub qualification_observations_available: bool,
    pub production_enabled: bool,
}

/// One registered provider's read-only management projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderIntegrationSnapshot {
    /// Checked, open provider identifier from the registered adapter.
    pub id: ProviderId,
    /// Safe product label, never a native provider/session identifier.
    pub label: &'static str,
    /// Whether a bounded local version probe found the registered provider.
    pub available: bool,
    /// Whether an owned integration was observed, where that notion applies.
    pub installed: bool,
    /// Provider version only when the bounded local probe succeeds.
    pub version: Option<String>,
    /// Production admission is never inferred from a newer version.
    pub admission: ProviderAdmissionState,
    /// Registered observation backend, not a raw command/configuration value.
    pub observation_backend: &'static str,
    /// Hook inspection state; future non-Hook providers can be not applicable.
    pub hooks: ProviderHookState,
    /// Capability evidence for this provider only.
    pub capability_profile: ProviderCapabilityProfile,
    /// Whether this provider can participate in `TabBeacon` title output.
    pub title_participation: CapabilityAvailability,
    /// Explicit manual follow-ups, never automatic actions.
    pub manual_actions: Vec<ProviderManualAction>,
    /// Qualification and production readiness, separate from installation.
    pub readiness: ProviderReadiness,
    #[serde(skip)]
    title_badge: &'static str,
}

impl ProviderIntegrationSnapshot {
    fn from_codex_probe(probe: ProviderProbe) -> Self {
        let admission = if probe.profile_supported {
            ProviderAdmissionState::Admitted
        } else {
            ProviderAdmissionState::Unknown
        };
        let lifecycle = if probe.profile_supported {
            CapabilityAvailability::Proven
        } else {
            CapabilityAvailability::Unavailable
        };
        let hooks = if probe.profile_supported && probe.hooks_available {
            ProviderHookState::Available
        } else {
            ProviderHookState::Unavailable
        };
        let hook_capability = if hooks == ProviderHookState::Available {
            CapabilityAvailability::Proven
        } else {
            CapabilityAvailability::Unavailable
        };
        let mut manual_actions = Vec::new();
        if admission == ProviderAdmissionState::Unknown {
            manual_actions.push(ProviderManualAction::InspectCompatibility);
        }
        if probe.installed && hooks != ProviderHookState::Available {
            manual_actions.push(ProviderManualAction::ReviewHooks);
        }
        Self {
            id: probe.provider,
            label: "Codex",
            available: probe.version.is_some(),
            installed: probe.installed,
            version: probe.version,
            admission,
            observation_backend: "codex-hooks",
            hooks,
            capability_profile: ProviderCapabilityProfile {
                capabilities: vec![
                    capability(ProviderCapability::Phase, lifecycle, "lifecycle"),
                    capability(ProviderCapability::Attention, lifecycle, "lifecycle"),
                    capability(ProviderCapability::ApprovalQuestion, lifecycle, "lifecycle"),
                    capability(
                        ProviderCapability::Health,
                        CapabilityAvailability::Unsupported,
                        "not_claimed",
                    ),
                    capability(
                        ProviderCapability::SessionIdentity,
                        lifecycle,
                        "provider_session_key",
                    ),
                    capability(
                        ProviderCapability::WorkspaceRootBinding,
                        lifecycle,
                        "root_workspace_anchor",
                    ),
                    capability(ProviderCapability::Subagents, lifecycle, "lifecycle_count"),
                    capability(
                        ProviderCapability::BackgroundTasks,
                        CapabilityAvailability::Unavailable,
                        "not_observed",
                    ),
                    capability(
                        ProviderCapability::TitleOutput,
                        lifecycle,
                        "tabbeacon_presentation",
                    ),
                    capability(
                        ProviderCapability::WindowsTerminalPresentation,
                        lifecycle,
                        "tabbeacon_presentation",
                    ),
                    capability(
                        ProviderCapability::HookInspection,
                        hook_capability,
                        "redacted_hook_inventory",
                    ),
                ],
            },
            title_participation: lifecycle,
            manual_actions,
            readiness: ProviderReadiness {
                qualification_available: false,
                qualification_observations_available: false,
                production_enabled: admission == ProviderAdmissionState::Admitted,
            },
            title_badge: "C",
        }
    }

    fn from_agy_preadmission(profile: AgyCapabilityProfile) -> Self {
        let version = profile.version.observed_version.map(AgyVersion::as_string);
        let qualification_observations_available = version.is_some();
        let capabilities = profile
            .capabilities
            .into_iter()
            .filter_map(|entry| {
                agy_capability(entry.capability).map(|provider_capability| {
                    capability(
                        provider_capability,
                        match entry.availability {
                            AgyCapabilityAvailability::Unavailable
                            | AgyCapabilityAvailability::Unknown => {
                                CapabilityAvailability::Unavailable
                            }
                        },
                        "unadmitted",
                    )
                })
            })
            .collect();
        Self {
            id: ProviderId(AGY_PROVIDER_ID.to_owned()),
            label: "Agy",
            available: version.is_some(),
            installed: false,
            version,
            admission: ProviderAdmissionState::Unadmitted,
            observation_backend: "unadmitted",
            hooks: ProviderHookState::Unavailable,
            capability_profile: ProviderCapabilityProfile { capabilities },
            title_participation: CapabilityAvailability::Unavailable,
            manual_actions: vec![
                ProviderManualAction::InspectCompatibility,
                ProviderManualAction::OwnerPresentQualification,
            ],
            readiness: ProviderReadiness {
                qualification_available: true,
                qualification_observations_available,
                production_enabled: false,
            },
            title_badge: "A",
        }
    }

    fn title_badge(&self) -> &str {
        self.title_badge
    }
}

fn agy_capability(capability: AgyCapability) -> Option<ProviderCapability> {
    Some(match capability {
        AgyCapability::Phase => ProviderCapability::Phase,
        AgyCapability::Attention => ProviderCapability::Attention,
        AgyCapability::Approval => ProviderCapability::ApprovalQuestion,
        AgyCapability::Health => ProviderCapability::Health,
        AgyCapability::SessionIdentity => ProviderCapability::SessionIdentity,
        AgyCapability::WorkspaceRoot => ProviderCapability::WorkspaceRootBinding,
        AgyCapability::BackgroundTasks => ProviderCapability::BackgroundTasks,
        AgyCapability::TitleCallback => ProviderCapability::TitleOutput,
        AgyCapability::WindowsTerminalPresentation => {
            ProviderCapability::WindowsTerminalPresentation
        }
        AgyCapability::HookObservation => ProviderCapability::HookInspection,
        AgyCapability::SetupOwnership => return None,
    })
}

fn capability(
    capability: ProviderCapability,
    availability: CapabilityAvailability,
    authority: &'static str,
) -> ProviderCapabilityStatus {
    ProviderCapabilityStatus {
        capability,
        availability,
        authority,
    }
}

/// Registered, provider-neutral management projection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ProviderRegistry {
    pub schema_version: u32,
    /// Registry inspection is observational and has no mutation authority.
    pub read_only: bool,
    pub providers: Vec<ProviderIntegrationSnapshot>,
}

impl ProviderRegistry {
    /// Builds the registered-provider view from an existing diagnostic pass.
    #[must_use]
    pub fn from_diagnostics(report: &OperationalDiagnostics, hooks: &HookInventory) -> Self {
        Self::codex_observation(
            report.codex.version.as_deref(),
            report.codex.profile_supported,
            report.integration.installed,
            hooks.availability == HookInventoryAvailability::Available,
        )
    }

    /// Builds the only v0.5.1 production registration from bounded probe facts.
    ///
    /// The constructor is intentionally useful to offline guided-setup and
    /// focused tests. It has no environment or provider side effects.
    #[must_use]
    pub fn codex_observation(
        version: Option<&str>,
        profile_supported: bool,
        installed: bool,
        hooks_available: bool,
    ) -> Self {
        Self {
            schema_version: PROVIDER_REGISTRY_SCHEMA_VERSION,
            read_only: true,
            providers: vec![
                ProviderIntegrationSnapshot::from_codex_probe(ProviderProbe::codex(
                    version,
                    profile_supported,
                    installed,
                    hooks_available,
                )),
                ProviderIntegrationSnapshot::from_agy_preadmission(
                    AgyCapabilityProfile::unadmitted(
                        crate::providers::agy::AgyVersionDiagnostic::from_versions(None, None),
                    ),
                ),
            ],
        }
    }

    /// Replaces the unadmitted Agy catalog row with a supplied qualification observation.
    ///
    /// The profile type cannot represent an admitted state, so this helper has
    /// no path to enable an Agy provider or title badge.
    #[must_use]
    pub fn with_agy_preadmission(mut self, profile: AgyCapabilityProfile) -> Self {
        let snapshot = ProviderIntegrationSnapshot::from_agy_preadmission(profile);
        if let Some(existing) = self
            .providers
            .iter_mut()
            .find(|provider| provider.id.as_str() == AGY_PROVIDER_ID)
        {
            *existing = snapshot;
        } else {
            self.providers.push(snapshot);
        }
        self
    }

    /// Registered production IDs in deterministic order, excluding unadmitted candidates.
    #[must_use]
    pub fn registered_ids(&self) -> Vec<&str> {
        self.providers
            .iter()
            .filter(|provider| provider.admission == ProviderAdmissionState::Admitted)
            .map(|provider| provider.id.as_str())
            .collect()
    }

    /// Returns a safe product label for a checked provider ID.
    #[must_use]
    pub fn label_for(&self, provider_id: &str) -> &'static str {
        self.providers
            .iter()
            .find(|provider| provider.id.as_str() == provider_id)
            .map_or("Unknown provider", |provider| provider.label)
    }

    /// Selects a bounded provider-title suffix without exposing native IDs.
    #[must_use]
    pub fn title_badge_for(
        &self,
        provider_id: &str,
        policy: ProviderBadgePolicy,
    ) -> Option<String> {
        if policy == ProviderBadgePolicy::Off {
            return None;
        }
        let provider = self.providers.iter().find(|provider| {
            provider.id.as_str() == provider_id
                && provider.admission == ProviderAdmissionState::Admitted
        })?;
        let admitted_count = self
            .providers
            .iter()
            .filter(|provider| provider.admission == ProviderAdmissionState::Admitted)
            .count();
        if policy == ProviderBadgePolicy::Auto && admitted_count <= 1 {
            return None;
        }
        let badge = provider.title_badge();
        (badge.len() == 1 && badge.bytes().all(|byte| byte.is_ascii_uppercase()))
            .then(|| badge.to_owned())
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::codex_observation(None, false, false, false)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CapabilityAvailability, ProviderAdmissionState, ProviderCapability, ProviderId,
        ProviderRegistry,
    };
    use crate::providers::agy::{AgyCapabilityProfile, AgyVersionDiagnostic};
    use crate::settings::ProviderBadgePolicy;

    #[test]
    fn production_registry_is_codex_only_but_exposes_unadmitted_agy_readiness() {
        let registry = ProviderRegistry::codex_observation(Some("0.149.0"), true, true, true);

        assert_eq!(registry.registered_ids(), vec!["codex"]);
        assert_eq!(registry.providers.len(), 2);
        assert_eq!(registry.providers[0].label, "Codex");
        assert_eq!(registry.providers[1].label, "Agy");
        assert!(registry.providers[1].readiness.qualification_available);
        assert!(!registry.providers[1].readiness.production_enabled);
        assert_eq!(
            registry.providers[0].admission,
            ProviderAdmissionState::Admitted
        );
        assert_eq!(
            registry.title_badge_for("codex", ProviderBadgePolicy::Auto),
            None,
            "single-provider auto preserves the existing compact title"
        );
        assert_eq!(
            registry.title_badge_for("codex", ProviderBadgePolicy::Always),
            Some("C".to_owned())
        );
        assert_eq!(
            registry.title_badge_for("codex", ProviderBadgePolicy::Off),
            None
        );
    }

    #[test]
    fn unknown_profile_does_not_inherit_provider_support() {
        let registry = ProviderRegistry::codex_observation(Some("9.9.9"), false, true, false);
        let codex = &registry.providers[0];

        assert_eq!(codex.admission, ProviderAdmissionState::Unknown);
        assert!(
            codex
                .capability_profile
                .capabilities
                .iter()
                .any(|capability| {
                    capability.capability == ProviderCapability::Phase
                        && capability.availability == CapabilityAvailability::Unavailable
                })
        );
        assert_eq!(
            registry.title_badge_for("codex", ProviderBadgePolicy::Always),
            None,
            "unadmitted providers cannot affect ordinary terminal titles"
        );
    }

    #[test]
    fn agy_catalog_entry_stays_unadmitted_and_cannot_affect_titles() {
        let registry =
            ProviderRegistry::default().with_agy_preadmission(AgyCapabilityProfile::unadmitted(
                AgyVersionDiagnostic::from_versions(Some("1.1.17"), Some("1.1.14")),
            ));
        let agy = registry
            .providers
            .iter()
            .find(|provider| provider.id.as_str() == "agy")
            .expect("Agy catalog row exists");

        assert!(agy.available);
        assert_eq!(agy.admission, ProviderAdmissionState::Unadmitted);
        assert!(
            agy.capability_profile
                .capabilities
                .iter()
                .all(|capability| capability.availability == CapabilityAvailability::Unavailable)
        );
        assert_eq!(
            registry.title_badge_for("agy", ProviderBadgePolicy::Always),
            None
        );
        assert_eq!(registry.registered_ids(), Vec::<&str>::new());
    }

    #[test]
    fn provider_ids_are_open_but_checked() {
        assert_eq!(
            ProviderId::new("future-agent-v9").unwrap().as_str(),
            "future-agent-v9"
        );
        assert!(ProviderId::new("Future").is_err());
        assert!(ProviderId::new("future agent").is_err());
        assert!(ProviderId::new("future-").is_err());
    }
}
