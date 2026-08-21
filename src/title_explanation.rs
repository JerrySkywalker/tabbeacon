//! Read-only, privacy-safe explanation of the facts that can influence a title.
//!
//! A CLI invocation is not correlated to a native terminal or provider session.
//! This model therefore labels session-derived facts as unavailable or
//! uncorrelated instead of inferring that a lease belongs to the current tab.

use serde::Serialize;

use crate::{
    activity::SessionsOverview, diagnostics::OperationalDiagnostics,
    repo::WorkspaceAliasInspection, settings::PresentationSettings,
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
    /// The only admitted provider in this release train.
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
    /// Provider badge configuration has not been introduced before G62.
    pub provider_badge_policy: &'static str,
    /// Provider badge value has not been introduced before G62.
    pub provider_badge_value: &'static str,
}

impl TitleExplanation {
    /// Builds a read-only explanation from already-collected safe state.
    #[must_use]
    pub fn from_observation(
        diagnostics: &OperationalDiagnostics,
        presentation: Option<PresentationSettings>,
        workspace: Option<&WorkspaceAliasInspection>,
        sessions: &SessionsOverview,
    ) -> Self {
        let workspace = workspace.map(|workspace| TitleWorkspaceExplanation {
            display_hint: workspace.workspace().as_str().to_owned(),
            identity_class: workspace.identity_class().as_str(),
            root_binding_source: "current_cli_workspace",
            root_binding_status: "not_session_correlated",
            workspace_mismatch_observation: "not_session_correlated",
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
        });
        let (title_owner, activity_channel) = presentation.map_or_else(
            || ("unavailable".to_owned(), "unavailable".to_owned()),
            |settings| {
                (
                    settings.title().as_str().to_owned(),
                    settings.activity().as_str().to_owned(),
                )
            },
        );
        Self {
            schema: TITLE_EXPLANATION_SCHEMA,
            provider: "codex",
            semantic_phase: "not_session_correlated",
            attention: "not_session_correlated",
            activity_health: sessions.health.as_str().to_owned(),
            activity_channel,
            session_correlation: if sessions.active_sessions == 0 {
                "unavailable"
            } else {
                "not_session_correlated"
            },
            workspace,
            title_owner,
            codex_writer_state: diagnostics.title.codex_writer_state.clone(),
            title_authority: diagnostics.title.authority.as_str().to_owned(),
            title_conflict: diagnostics.title.conflict_class.as_str().to_owned(),
            provider_badge_policy: "not_applicable",
            provider_badge_value: "not_applicable",
        }
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
            provider_badge_policy: "not_applicable",
            provider_badge_value: "not_applicable",
        }
    }
}
