//! Allow-listed product facts; density never changes observation or configuration.
use super::{
    ControlCenterApp, HumanMessageKey, ManagementTextKind, ResolvedLocale, Screen, catalog,
    legacy_content_text, management_action_text, management_text, render_human_text, safety_label,
    severity_label, shared_health_label,
};
use terminal_ui_contract::{
    document::{
        EvidenceField, FactFreshness, PermittedText, SafeNextStep, Severity, UiDocument, UiFact,
        Verdict,
    },
    keymap::{Action, ActionEffect, ActionIntent},
};

fn permitted(text: impl AsRef<str>) -> PermittedText {
    // All callers supply normalized product projections, never provider output,
    // configuration bodies, Hook commands or private session payloads.
    PermittedText::from_product_redaction(
        text.as_ref()
            .chars()
            .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
            .collect::<String>(),
    )
}

pub(super) fn presentation_text(
    app: &ControlCenterApp,
    density: terminal_ui_contract::document::ViewDensity,
) -> String {
    use terminal_ui_contract::document::ViewDensity;
    let document = app.ui_document();
    if density == ViewDensity::Raw {
        return document.project(density).lines().join("\n");
    }
    let mut lines = vec![];
    let mut facts = document.project(density).facts().iter().collect::<Vec<_>>();
    facts.sort_by_key(|fact| match fact.verdict {
        Verdict::Fail => 0,
        Verdict::Blocked => 1,
        Verdict::Stale => 2,
        Verdict::Unproven | Verdict::Unknown => 3,
        _ => 4,
    });
    for fact in facts {
        lines.push(format!(
            "[{}] {}: {}",
            fact.verdict.marker(),
            fact.label,
            fact.summary.as_str()
        ));
        lines.push(format!(
            "source={} · scope={} · {:?}",
            fact.source, fact.scope, fact.freshness
        ));
        if fact.id.starts_with("issue.")
            || matches!(
                fact.verdict,
                Verdict::Blocked | Verdict::Stale | Verdict::Fail
            )
        {
            for step in &fact.next_steps {
                lines.push(step.label.clone());
            }
        }
    }
    lines.join("\n")
}
fn fact(
    app: &ControlCenterApp,
    id: String,
    label: String,
    verdict: Verdict,
    summary: String,
    source: &str,
    scope: &str,
) -> UiFact {
    UiFact {
        id,
        label,
        verdict,
        severity: match verdict {
            Verdict::Fail => Severity::Error,
            Verdict::Blocked | Verdict::Stale | Verdict::Unknown | Verdict::Unproven => {
                Severity::Attention
            }
            _ => Severity::Information,
        },
        source: source.into(),
        scope: scope.into(),
        sampled_at: app.sampled_at,
        freshness: if app.snapshot_revision == 0 || app.snapshot_stale {
            FactFreshness::Unknown
        } else {
            FactFreshness::Current
        },
        summary: permitted(summary),
        impact: permitted(if app.locale() == ResolvedLocale::ZhCn {
            "只适用于所示数据源和范围；不证明当前终端投递"
        } else {
            "Applies to this source and scope; current terminal delivery remains independent"
        }),
        evidence: vec![],
        next_steps: vec![SafeNextStep {
            label: if app.locale() == ResolvedLocale::ZhCn {
                "只读刷新；配置与 Hook 信任需明确人工确认"
            } else {
                "Read-only refresh; configuration and Hook trust require explicit review"
            }
            .into(),
            intent: ActionIntent {
                action: Action::Refresh,
                effect: ActionEffect::ReadOnlyQuery,
                count: 1,
            },
        }],
    }
}

impl ControlCenterApp {
    /// One immutable semantic document for the selected screen and accepted revision.
    ///
    /// # Panics
    /// Panics if internal product facts violate unique-ID or typed serialization invariants.
    #[must_use]
    #[allow(clippy::too_many_lines)] // Nine product screens project through a single allow-listed document boundary.
    pub fn ui_document(&self) -> UiDocument {
        use crate::{
            doctor_matrix::EvidenceVerdict,
            hook_inventory::{HookCurrentness, HookTrustState},
        };
        let legacy = legacy_content_text(self);
        let mut facts = vec![];
        let summary = match self.screen {
            Screen::Overview => format!(
                "{}: {} · TabBeacon {} · Codex {} · Hooks {} · trust {}",
                catalog(self.locale(), HumanMessageKey::OverallHealth),
                shared_health_label(self.locale(), self.snapshot.health),
                self.overview.tabbeacon_version,
                self.overview.codex_version,
                self.overview.hooks,
                self.overview.hook_trust
            ),
            Screen::Integration => format!(
                "{} · preference {:?} · {} registered providers",
                self.selected_provider.as_str(),
                self.selected_provider_mode,
                self.integrations.providers.len()
            ),
            Screen::Hooks => format!(
                "{} / name · {} · inventory={} · {} entries · commands redacted",
                catalog(self.locale(), HumanMessageKey::Provider),
                catalog(self.locale(), HumanMessageKey::Trust),
                self.hooks.availability.as_str(),
                self.hooks.entries.len()
            ),
            Screen::Diagnostics => format!(
                "provider={} · queried={} · observed mode remains evidence-scoped",
                self.diagnostic_provider.as_str(),
                self.diagnostic_mode.as_str()
            ),
            _ => legacy.clone(),
        };
        let verdict = if self.snapshot_revision == 0 {
            Verdict::Unknown
        } else {
            match self.snapshot.health {
                crate::management::ManagementHealth::Healthy => Verdict::StaticPass,
                crate::management::ManagementHealth::Warning => Verdict::Unproven,
                crate::management::ManagementHealth::Error => Verdict::Fail,
            }
        };
        let mut primary = fact(
            self,
            "screen.summary".into(),
            self.screen.localized_title(self.locale()).into(),
            verdict,
            summary,
            "product management snapshot",
            "read-only normalized product facts",
        );
        primary.evidence.push(EvidenceField {
            label: "normalized details".into(),
            value: permitted(legacy),
        });
        facts.push(primary);
        if self.snapshot_revision > 0 && self.snapshot_stale {
            facts.push(fact(
                self,
                "snapshot.stale".into(),
                if self.locale() == ResolvedLocale::ZhCn {
                    "数据已过期"
                } else {
                    "Snapshot stale"
                }
                .into(),
                Verdict::Stale,
                if self.locale() == ResolvedLocale::ZhCn {
                    "保留上一轮已知事实；只读刷新后确认新鲜度"
                } else {
                    "Last-good facts retained; read-only refresh required for current observation"
                }
                .into(),
                "shared refresh controller",
                "snapshot freshness; historical verdicts unchanged",
            ));
        }
        match self.screen {
            Screen::Overview | Screen::Diagnostics => {
                for issue in &self.snapshot.issues {
                    let verdict = match issue.severity {
                        crate::management::HealthSeverity::Error => Verdict::Fail,
                        crate::management::HealthSeverity::Warning => Verdict::Unproven,
                    };
                    let mut item = fact(
                        self,
                        format!("issue.{}", issue.id),
                        format!(
                            "{} {}",
                            severity_label(self.locale(), issue.severity),
                            render_human_text(
                                self.locale(),
                                &management_text(
                                    ManagementTextKind::IssueTitle,
                                    &issue.id,
                                    &issue.title
                                )
                            )
                        ),
                        verdict,
                        render_human_text(
                            self.locale(),
                            &management_text(
                                ManagementTextKind::IssueExplanation,
                                &issue.id,
                                &issue.explanation,
                            ),
                        ),
                        "doctor diagnostic projection",
                        "static local health",
                    );
                    if let Some(action) = &issue.remediation {
                        item.next_steps[0].label = format!(
                            "{} [{}]",
                            render_human_text(
                                self.locale(),
                                &management_action_text(&issue.id, &action.id, &action.instruction)
                            ),
                            safety_label(self.locale(), action.safety)
                        );
                    }
                    facts.push(item);
                }
            }
            Screen::Integration => {
                for provider in &self.integrations.providers {
                    let mut item = fact(
                        self,
                        format!("provider.{}", provider.id),
                        provider.label.into(),
                        Verdict::Unproven,
                        format!(
                            "{} · admission={} · installed={} · current delivery UNPROVEN",
                            provider.configuration_state,
                            provider.admission.as_str(),
                            provider.installed
                        ),
                        provider.observation_backend,
                        "local provider configuration/capability observation",
                    );
                    item.evidence.push(EvidenceField {
                        label: "normalized provider".into(),
                        value: permitted(
                            serde_json::to_string(provider)
                                .expect("safe typed provider serializes"),
                        ),
                    });
                    facts.push(item);
                }
            }
            _ => {}
        }
        if self.screen == Screen::Diagnostics {
            let matrix = self.doctor_matrix.filtered(
                Some(self.diagnostic_provider.as_str()),
                Some(self.diagnostic_mode),
            );
            for row in &matrix.rows {
                for evidence in &row.evidence {
                    let verdict = match evidence.verdict {
                        EvidenceVerdict::Proven => {
                            if evidence.scope == "static_inspection" {
                                Verdict::StaticPass
                            } else {
                                Verdict::Pass
                            }
                        }
                        EvidenceVerdict::ObservedFailed => Verdict::Fail,
                        EvidenceVerdict::BlockedUpstream => Verdict::Blocked,
                        EvidenceVerdict::Unproven => Verdict::Unproven,
                        EvidenceVerdict::Unsupported | EvidenceVerdict::NotApplicable => {
                            Verdict::Unknown
                        }
                    };
                    let mut item = fact(
                        self,
                        format!(
                            "matrix.{}.{}.{}",
                            row.provider,
                            row.execution_mode.as_str(),
                            evidence.capability
                        ),
                        evidence.capability.into(),
                        verdict,
                        format!("{}: {}", evidence.verdict.as_str(), evidence.reason),
                        evidence.source,
                        evidence.scope,
                    );
                    item.next_steps[0].label = row.safe_action.into();
                    item.evidence.push(EvidenceField {
                        label: "queried / observed mode".into(),
                        value: permitted(format!(
                            "{} / {}",
                            row.execution_mode.as_str(),
                            row.observed_execution_mode.as_str()
                        )),
                    });
                    facts.push(item);
                }
            }
            for (index, history) in matrix.historical_evidence.iter().enumerate() {
                let mut item = fact(
                    self,
                    format!("history.{index}"),
                    "Historical evidence / 历史证据".into(),
                    Verdict::Unproven,
                    format!(
                        "{} {} · {} · current delivery UNPROVEN",
                        history.provider,
                        history.provider_version,
                        history.execution_mode.as_str()
                    ),
                    history.source,
                    history.scope,
                );
                item.freshness = FactFreshness::Historical;
                item.evidence.push(EvidenceField {
                    label: "normalized historical receipt".into(),
                    value: permitted(serde_json::to_string(history).unwrap()),
                });
                facts.push(item);
            }
        }
        if self.screen == Screen::Hooks {
            for (index, hook) in self.hooks.entries.iter().enumerate() {
                let currentness = match hook.currentness {
                    HookCurrentness::Current => Verdict::StaticPass,
                    HookCurrentness::Stale => Verdict::Stale,
                    HookCurrentness::DeclarationModifiedOrMissing => Verdict::Fail,
                    _ => Verdict::Unproven,
                };
                let mut item = fact(
                    self,
                    format!("hook.{index}.currentness"),
                    hook.readable_name.clone().unwrap_or_else(|| {
                        format!(
                            "{} {}",
                            hook.provider,
                            crate::hook_inventory::event_label(&hook.event)
                        )
                    }),
                    currentness,
                    format!(
                        "currentness={} · owner={} · enabled={}",
                        hook.currentness.as_str(),
                        hook.owner.as_str(),
                        hook.enabled
                    ),
                    hook.source_kind.as_str(),
                    "static declaration currentness; not delivery",
                );
                item.evidence.push(EvidenceField {
                    label: "normalized command-redacted Hook".into(),
                    value: permitted(serde_json::to_string(hook).unwrap()),
                });
                facts.push(item);
                facts.push(fact(
                    self,
                    format!("hook.{index}.trust"),
                    format!("{} trust", hook.event),
                    if hook.trust_state == HookTrustState::Trusted {
                        Verdict::StaticPass
                    } else {
                        Verdict::Unproven
                    },
                    format!(
                        "{} {}",
                        catalog(self.locale(), HumanMessageKey::Trust),
                        if hook.trust_state == HookTrustState::Trusted
                            && self.locale() == ResolvedLocale::ZhCn
                        {
                            "可信"
                        } else {
                            hook.trust_state.as_str()
                        }
                    ),
                    hook.source_kind.as_str(),
                    "native manual trust observation; not current terminal proof",
                ));
            }
        }
        UiDocument::new(self.snapshot_revision, self.sampled_at, facts)
            .expect("product fact IDs are unique and sourced")
    }
}
