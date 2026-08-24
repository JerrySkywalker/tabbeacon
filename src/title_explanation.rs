//! Read-only, privacy-safe explanation of the facts that can influence a title.
//!
//! A CLI invocation is not correlated to a native terminal or provider session.
//! This model therefore labels session-derived facts as unavailable or
//! uncorrelated instead of inferring that a lease belongs to the current tab.

use serde::Serialize;

use crate::{
    activity::SessionsOverview, diagnostics::OperationalDiagnostics,
    providers::registry::ProviderRegistry, repo::WorkspaceAliasInspection,
    settings::PresentationSettings,
};

/// Stable schema for the read-only title explanation transport.
pub const TITLE_EXPLANATION_SCHEMA: &str = "tabbeacon-title-explanation-v1";

/// Safe workspace provenance relevant to an explained title.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TitleWorkspaceExplanation {
    /// Safe display hint, never a path or canonical identity.
    pub display_hint: String,
    /// Coarse identity evidence class without opaque identity material.
    pub identity_class: &'static str,
    /// This invocation's read-only workspace source.
    pub root_binding_source: &'static str,
    /// Whether the CLI has proof this workspace belongs to a live title.
    pub root_binding_status: &'static str,
    /// Session mismatch cannot be attributed to this CLI invocation.
    pub workspace_mismatch_observation: &'static str,
    /// Generated Adaptive Naming alias.
    pub automatic_alias: String,
    /// Optional device-local alias override.
    pub override_alias: Option<String>,
    /// Alias which would be presented for this workspace.
    pub effective_alias: String,
    /// Whether an override or generated alias is effective.
    pub alias_source: &'static str,
    /// Fixed deterministic naming policy marker.
    pub naming_policy: String,
}

/// One bounded, read-only answer to `Why this title?`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TitleExplanation {
    /// Versioned schema label.
    pub schema: &'static str,
    /// Provider observed for this workspace, or a bounded ambiguity token.
    pub provider: &'static str,
    /// No native session is correlated from this read-only surface.
    pub semantic_phase: &'static str,
    /// No native attention state is correlated from this read-only surface.
    pub attention: &'static str,
    /// Aggregate lease health, never process or session identity.
    pub activity_health: String,
    /// Selected activity channel from read-only presentation settings.
    pub activity_channel: String,
    /// Explicit boundary on the relationship between this CLI and lease rows.
    pub session_correlation: &'static str,
    /// Current workspace facts when they can be read safely.
    pub workspace: Option<TitleWorkspaceExplanation>,
    /// Configured title writer, or `unavailable` when settings could not be read.
    pub title_owner: String,
    /// Existing bounded Codex title-writer diagnosis.
    pub codex_writer_state: String,
    /// Existing bounded visible-title authority classification.
    pub title_authority: String,
    /// Existing bounded title-conflict classification.
    pub title_conflict: String,
    /// Persisted provider-badge policy, or `unavailable` when unreadable.
    pub provider_badge_policy: String,
    /// Safe deterministic badge outcome for the current Codex-only registry.
    pub provider_badge_value: String,
}

impl TitleExplanation {
    /// Builds a read-only explanation from already-collected safe state.
    #[must_use]
    pub fn from_observation(
        diagnostics: &OperationalDiagnostics,
        presentation: Option<PresentationSettings>,
        workspace: Option<&WorkspaceAliasInspection>,
        sessions: &SessionsOverview,
        integrations: &ProviderRegistry,
    ) -> Self {
        let effective_alias = workspace.map(|value| value.effective_alias().as_str());
        let matching_sessions = sessions
            .sessions
            .iter()
            .filter(|session| effective_alias.is_some_and(|alias| session.workspace_alias == alias))
            .collect::<Vec<_>>();
        let provider = observed_provider(&matching_sessions);
        let semantic_phase = observed_phase(&matching_sessions);
        let workspace = workspace.map(|workspace| explain_workspace(workspace, &matching_sessions));
        let (title_owner, activity_channel, provider_badge_policy, provider_badge_value) =
            presentation.map_or_else(
                || {
                    (
                        "unavailable".to_owned(),
                        "unavailable".to_owned(),
                        "unavailable".to_owned(),
                        "unavailable".to_owned(),
                    )
                },
                |settings| {
                    (
                        settings.title().as_str().to_owned(),
                        settings.activity().as_str().to_owned(),
                        settings.provider_badge().as_str().to_owned(),
                        integrations
                            .title_badge_for(provider, settings.provider_badge())
                            .unwrap_or_else(|| match settings.provider_badge() {
                                crate::settings::ProviderBadgePolicy::Off => {
                                    "not_emitted".to_owned()
                                }
                                crate::settings::ProviderBadgePolicy::Auto => {
                                    if provider == "multiple" {
                                        "emitted_per_provider".to_owned()
                                    } else {
                                        "not_emitted_single_provider".to_owned()
                                    }
                                }
                                crate::settings::ProviderBadgePolicy::Always => {
                                    if provider == "multiple" {
                                        "emitted_per_provider".to_owned()
                                    } else {
                                        "unavailable_unadmitted_provider".to_owned()
                                    }
                                }
                            }),
                    )
                },
            );
        Self {
            schema: TITLE_EXPLANATION_SCHEMA,
            provider,
            semantic_phase,
            attention: "unavailable",
            activity_health: sessions.health.as_str().to_owned(),
            activity_channel,
            session_correlation: if matching_sessions.is_empty() {
                "unavailable"
            } else if provider == "multiple" {
                "multiple_workspace_observations"
            } else {
                "workspace_observation_only"
            },
            workspace,
            title_owner,
            codex_writer_state: if provider == "agy" {
                "not_applicable".to_owned()
            } else {
                diagnostics.title.codex_writer_state.clone()
            },
            title_authority: if provider == "agy" {
                "structured_title_callback".to_owned()
            } else if provider == "multiple" {
                "provider_specific".to_owned()
            } else {
                diagnostics.title.authority.as_str().to_owned()
            },
            title_conflict: if provider == "agy" {
                "not_applicable".to_owned()
            } else {
                diagnostics.title.conflict_class.as_str().to_owned()
            },
            provider_badge_policy,
            provider_badge_value,
        }
    }
}

fn explain_workspace(
    workspace: &WorkspaceAliasInspection,
    matching_sessions: &[&crate::activity::SessionOverview],
) -> TitleWorkspaceExplanation {
    let stable_root_observed = !matching_sessions.is_empty()
        && matching_sessions
            .iter()
            .all(|session| session.workspace_observability.root_binding_stable);
    let workspace_mismatch_observation = if matching_sessions
        .iter()
        .any(|session| session.workspace_observability.workspace_mismatch_observed)
    {
        "observed"
    } else if stable_root_observed {
        "not_observed"
    } else {
        "not_session_correlated"
    };
    TitleWorkspaceExplanation {
        display_hint: workspace.workspace().as_str().to_owned(),
        identity_class: workspace.identity_class().as_str(),
        root_binding_source: if stable_root_observed {
            "provider_session_observation"
        } else {
            "current_cli_workspace"
        },
        root_binding_status: if stable_root_observed {
            "stable_workspace_observation"
        } else {
            "not_session_correlated"
        },
        workspace_mismatch_observation,
        automatic_alias: workspace.automatic_alias().as_str().to_owned(),
        override_alias: workspace
            .custom_alias()
            .map(|alias| alias.as_str().to_owned()),
        effective_alias: workspace.effective_alias().as_str().to_owned(),
        alias_source: if workspace.custom_alias().is_some() {
            "override"
        } else {
            "automatic"
        },
        naming_policy: workspace.policy_version().to_owned(),
    }
}

fn observed_provider(sessions: &[&crate::activity::SessionOverview]) -> &'static str {
    let mut observed = sessions.iter().map(|session| session.provider.as_str());
    let Some(first) = observed.next() else {
        return "not_session_correlated";
    };
    if observed.any(|provider| provider != first) {
        return "multiple";
    }
    match first {
        "agy" => "agy",
        "codex" => "codex",
        _ => "unknown",
    }
}

fn observed_phase(sessions: &[&crate::activity::SessionOverview]) -> &'static str {
    let mut observed = sessions
        .iter()
        .map(|session| session.semantic_state.as_str());
    let Some(first) = observed.next() else {
        return "not_session_correlated";
    };
    if observed.any(|phase| phase != first) {
        return "multiple";
    }
    match first {
        "ready" => "ready",
        "working" => "working",
        "result-ready" => "result_ready",
        "approval" => "approval",
        _ => "unknown",
    }
}

impl Default for TitleExplanation {
    fn default() -> Self {
        Self {
            schema: TITLE_EXPLANATION_SCHEMA,
            provider: "codex",
            semantic_phase: "unavailable",
            attention: "unavailable",
            activity_health: "unavailable".to_owned(),
            activity_channel: "unavailable".to_owned(),
            session_correlation: "unavailable",
            workspace: None,
            title_owner: "unavailable".to_owned(),
            codex_writer_state: "unavailable".to_owned(),
            title_authority: "unavailable".to_owned(),
            title_conflict: "unavailable".to_owned(),
            provider_badge_policy: "unavailable".to_owned(),
            provider_badge_value: "unavailable".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{observed_phase, observed_provider};
    use crate::activity::{
        SessionOverview, SessionRecency, SessionWorkerHealth, SessionWorkspaceObservability,
    };

    fn session(provider: &str, phase: &str) -> SessionOverview {
        SessionOverview {
            workspace_alias: "TB".to_owned(),
            provider: provider.to_owned(),
            semantic_state: phase.to_owned(),
            age_seconds: 0,
            recency: SessionRecency::JustNow,
            worker_health: SessionWorkerHealth::RecentlyAuthorized,
            workspace_observability: SessionWorkspaceObservability {
                root_binding_stable: true,
                workspace_mismatch_observed: false,
                active_subagents: 0,
                background_tasks: None,
            },
        }
    }

    #[test]
    fn explanation_context_projects_agy_without_native_identity_and_marks_ambiguity() {
        let agy = session("agy", "ready");
        assert_eq!(observed_provider(&[&agy]), "agy");
        assert_eq!(observed_phase(&[&agy]), "ready");

        let codex = session("codex", "working");
        assert_eq!(observed_provider(&[&agy, &codex]), "multiple");
        assert_eq!(observed_phase(&[&agy, &codex]), "multiple");
    }
}
