use std::{
    collections::BTreeMap,
    fs,
    io::{self, IsTerminal, Read, Write},
    process::ExitCode,
    thread,
    time::Duration,
};

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use dialoguer::{Confirm, Select};
use tabbeacon::cli::{
    AliasCommand, Cli, Command, ConfigCommand, ConvergenceCommand, DoctorArgs, ExplainCommand,
    HumanOutputArgs, InterfaceCommand, InterfacePreferenceKey, OutputMode, PreviewArgs, Provider,
    SetupCommand, TitlePolicyCommand, UpgradePreflightArgs,
};
use tabbeacon::diagnostics::{
    collect_operational_diagnostics, collect_operational_diagnostics_with_title_probe,
    human_doctor_lines, human_status_lines,
};
use tabbeacon::guided_setup::{GuidedInput, choose_interface_preferences, choose_presentation};
use tabbeacon::human_diagnostics::{render_human_doctor, render_human_status, terminal_width};
use tabbeacon::human_output::{HumanTone, style};
use tabbeacon::human_presentation::{
    HumanAction, HumanDocument, HumanField, HumanLine, HumanMessage, HumanMessageKey,
    HumanRenderer, HumanSection, HumanText, ResolvedLocale, color_enabled, render_human_text,
    resolve_runtime_locale,
};
use tabbeacon::management::ManagementSnapshot;
use tabbeacon::providers::codex::{
    CodexHookRuntime, CodexIntegration, SetupOutcome, TitleOwnershipOutcome, UninstallOutcome,
};
use tabbeacon::setup::{
    GuidedSetupApplyResult, GuidedSetupPlan, SetupDecision, SetupDiscovery, WindowsTerminalState,
    detect_windows_terminal,
};
use tabbeacon::{
    activity::{
        inspect_system_sessions, run_activity_cleanup_observer_system, run_activity_worker_system,
    },
    core::{Attention, Health, Phase},
    interface_preferences::{
        HumanColor, InterfaceLanguage, InterfacePreferences,
        InterfacePreferencesConditionalOutcome, InterfacePreferencesSnapshotSaveOutcome,
        InterfacePreferencesStore,
    },
    presentation::{
        PresentationPolicy, SemanticPresentationInput, WindowsTerminalCapabilities,
        WindowsTerminalRenderer,
    },
    repo::{
        AliasCandidate, RepositoryAlias, StableAliasRegistry, WorkspaceAliasError,
        WorkspaceAliasInspection, WorkspaceIdentityResolver, WorkspacePreferenceStore,
    },
    settings::{
        ActivityMode, ConditionalSaveOutcome, PresentationSettings, PresentationSettingsSnapshot,
        PresentationSettingsStore, PresentationTheme, SnapshotSaveOutcome, SpinnerPreset,
        TabColorMode, TitleMode,
    },
    settings_transfer::{
        ImportApplyOutcome, ImportPlan, MAX_EXPORT_BYTES, SettingsExportV1, apply_import_plan,
        write_export_file,
    },
    title_explanation::TitleExplanation,
    upgrade_preflight::{
        UpgradePreflight, UpgradeProcessInspection, UpgradeReplaceability,
        inspect_system_upgrade_preflight,
    },
    windows_terminal_policy::{TitleRemediationState, WindowsTerminalPolicyStore},
};

const MAX_HOOK_INPUT_BYTES: u64 = 1024 * 1024;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = u8::try_from(error.exit_code()).unwrap_or(1);
            let _ = error.print();
            return ExitCode::from(exit_code);
        }
    };
    dispatch(cli)
}

#[allow(clippy::too_many_lines)]
fn dispatch(cli: Cli) -> ExitCode {
    match cli.command {
        None | Some(Command::Ui) => ui(),
        Some(Command::ActivityWorker {
            key_digest,
            generation,
            revision,
        }) => {
            if let (Ok(generation), Ok(revision)) =
                (generation.parse::<u64>(), revision.parse::<u64>())
            {
                run_activity_worker_system(&key_digest, generation, revision);
            }
            ExitCode::SUCCESS
        }
        Some(Command::TitleProbeFixture {
            run_id,
            hold_millis,
        }) => match hold_millis.parse::<u64>() {
            Ok(hold_millis) => {
                match tabbeacon::visual::emit_title_authority_fixture(&run_id, hold_millis) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(_) => ExitCode::FAILURE,
                }
            }
            Err(_) => ExitCode::FAILURE,
        },
        Some(Command::ActivityCleanupObserver {
            worker_pid,
            key_digest,
            generation,
            revision,
            owner_sha256,
            expected_executable,
        }) => {
            if let (Ok(worker_pid), Ok(generation), Ok(revision)) = (
                worker_pid.parse::<u32>(),
                generation.parse::<u64>(),
                revision.parse::<u64>(),
            ) {
                run_activity_cleanup_observer_system(
                    worker_pid,
                    &key_digest,
                    generation,
                    revision,
                    &owner_sha256,
                    &expected_executable,
                );
            }
            ExitCode::SUCCESS
        }
        Some(Command::Setup {
            command: None,
            quick,
            full,
            output,
        }) => guided_setup(quick, full, output),
        Some(Command::Setup {
            command: Some(SetupCommand::Codex),
            output,
            ..
        }) => setup_codex(output.mode(), output.language.preference()),
        Some(Command::Doctor(DoctorArgs {
            output,
            probe_title,
        })) => doctor(output.mode(), probe_title, output.language.preference()),
        Some(Command::Status(output)) => status(output.mode(), output.language.preference()),
        Some(Command::Sessions(output)) => sessions(output.mode(), output.language.preference()),
        Some(Command::Hooks(output)) => hooks(output.mode(), output.language.preference()),
        Some(Command::UpgradePreflight(arguments)) => upgrade_preflight(arguments),
        Some(Command::TitlePolicy { command }) => match command {
            TitlePolicyCommand::Inspect(output) => title_policy_inspect(output.json),
            TitlePolicyCommand::Repair(output) => title_policy_repair(output.json),
            TitlePolicyCommand::Restore(output) => title_policy_restore(output.json),
        },
        Some(Command::Convergence { command }) => match command {
            ConvergenceCommand::Matrix(output) => convergence_matrix(output.json),
            ConvergenceCommand::Verify {
                matrix,
                expected_head,
            } => convergence_verify(&matrix, &expected_head),
        },
        Some(Command::Uninstall {
            provider: Provider::Codex,
            output,
        }) => uninstall_codex(output.mode()),
        Some(Command::Hook {
            provider: Provider::Codex,
        }) => run_codex_hook(),
        Some(Command::Config { command, output }) => {
            config_command(command, output.mode(), output.language.preference())
        }
        Some(Command::Interface { command, output }) => {
            interface_command(command, output.mode(), output.language.preference())
        }
        Some(Command::Alias { command, output }) => {
            alias_command(command, output.mode(), output.language.preference())
        }
        Some(Command::Explain {
            command: ExplainCommand::Title,
            output,
        }) => explain_title(output.mode(), output.language.preference()),
        Some(Command::Export {
            destination,
            force,
            output,
        }) => export_settings(destination.as_deref(), force, output),
        Some(Command::Import {
            path,
            apply,
            output,
        }) => import_settings(&path, apply, output),
        Some(Command::Preview(arguments)) => preview(arguments),
        Some(Command::Completions { shell }) => completions(shell),
    }
}

fn config_command(
    command: ConfigCommand,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    match command {
        ConfigCommand::Show => config_show(output_mode, language),
        ConfigCommand::Set { key, value } => config_set(&key, &value, output_mode, language),
        ConfigCommand::Preset { name } => config_preset(&name, output_mode, language),
        ConfigCommand::Reset => config_reset(output_mode, language),
        ConfigCommand::Wizard => config_wizard(output_mode, language),
    }
}

fn interface_command(
    command: InterfaceCommand,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    match command {
        InterfaceCommand::Show => interface_show(output_mode, language),
        InterfaceCommand::Set { key, value } => interface_set(key, &value, output_mode, language),
    }
}

fn alias_command(
    command: Option<AliasCommand>,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    let operation = match command.as_ref() {
        None | Some(AliasCommand::Show) => "show",
        Some(AliasCommand::Preview) => "preview",
        Some(AliasCommand::Explain) => "explain",
        Some(AliasCommand::Set { .. }) => "set",
        Some(AliasCommand::Reset) => "reset",
    };
    let Ok(resolver) = WorkspaceIdentityResolver::with_default_state_root() else {
        return alias_failure(
            operation,
            WorkspaceAliasError::Unavailable,
            output_mode,
            language,
        );
    };
    let Ok(cwd) = std::env::current_dir() else {
        return alias_failure(
            operation,
            WorkspaceAliasError::Unavailable,
            output_mode,
            language,
        );
    };
    let result = match command.unwrap_or(AliasCommand::Show) {
        AliasCommand::Show | AliasCommand::Preview | AliasCommand::Explain => {
            resolver.inspect_alias(&cwd)
        }
        AliasCommand::Set { alias } => resolver.set_alias_override(&cwd, alias),
        AliasCommand::Reset => resolver.reset_alias_override(&cwd),
    };
    match result {
        Ok(inspection) => {
            print_alias_output(operation, &inspection, output_mode, language);
            ExitCode::SUCCESS
        }
        Err(error) => alias_failure(operation, error, output_mode, language),
    }
}

fn print_alias_output(
    operation: &str,
    inspection: &WorkspaceAliasInspection,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) {
    match output_mode {
        OutputMode::Human => print_human_document(&alias_document(operation, inspection), language),
        OutputMode::Plain => print_alias_plain(operation, inspection),
        OutputMode::Json => match serde_json::to_string(&alias_json(operation, inspection)) {
            Ok(value) => println!("{value}"),
            Err(_) => {
                // The DTO has only strings, booleans, and integers. This is a
                // defensive safe fallback rather than a raw serialization error.
                println!(
                    "{{\"schema\":\"tabbeacon-alias-v1\",\"operation\":\"{operation}\",\"result\":\"failure\",\"reason\":\"unavailable\"}}"
                );
            }
        },
    }
}

fn alias_failure(
    operation: &str,
    error: WorkspaceAliasError,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    let reason = match error {
        WorkspaceAliasError::InvalidAlias => "invalid_alias",
        WorkspaceAliasError::Collision => "alias_conflict",
        WorkspaceAliasError::Conflict => "preference_conflict",
        WorkspaceAliasError::Unavailable => "unavailable",
    };
    match output_mode {
        OutputMode::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "schema": "tabbeacon-alias-v1",
                    "operation": operation,
                    "result": "failure",
                    "reason": reason,
                    "unchanged": true,
                })
            );
        }
        OutputMode::Plain => {
            eprintln!("ALIAS_SCHEMA_VERSION=1");
            eprintln!("ALIAS_OPERATION={operation}");
            eprintln!("ALIAS=FAIL");
            eprintln!("REASON={reason}");
            eprintln!("ALIAS_UNCHANGED=true");
        }
        OutputMode::Human => {
            eprint_human_text(
                HumanTone::Attention,
                &HumanText::message(alias_error_message_key(error)),
                language,
            );
        }
    }
    ExitCode::from(2)
}

fn alias_document(operation: &str, inspection: &WorkspaceAliasInspection) -> HumanDocument {
    let title = match operation {
        "preview" => HumanMessageKey::AliasPreview,
        "explain" => HumanMessageKey::AliasExplanation,
        _ => HumanMessageKey::WorkspaceAlias,
    };
    let status = match operation {
        "set" => Some(HumanText::message(HumanMessageKey::AliasOverrideSaved)),
        "reset" => Some(HumanText::message(HumanMessageKey::AliasOverrideReset)),
        _ if !inspection.is_assigned() => {
            Some(HumanText::message(HumanMessageKey::AliasProspective))
        }
        _ => None,
    };
    let document = HumanDocument::new(HumanText::message(title), status)
        .with_section(alias_summary_section(inspection));
    match operation {
        "preview" => alias_preview_document(document, inspection),
        "explain" => alias_explain_document(document, inspection),
        _ => document,
    }
}

fn alias_summary_section(inspection: &WorkspaceAliasInspection) -> HumanSection {
    HumanSection::new(None)
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Workspace),
            HumanText::literal(inspection.workspace().as_str()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::AutomaticAlias),
            HumanText::literal(inspection.automatic_alias().as_str()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::CustomAlias),
            HumanText::literal(
                inspection
                    .custom_alias()
                    .map_or("—", RepositoryAlias::as_str),
            ),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::EffectiveAlias),
            HumanText::literal(inspection.effective_alias().as_str()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::NamingPolicy),
            HumanText::literal(inspection.policy_version()),
            HumanTone::Dim,
        ))
}

fn alias_preview_document(
    document: HumanDocument,
    inspection: &WorkspaceAliasInspection,
) -> HumanDocument {
    inspection.candidates().iter().take(5).enumerate().fold(
        document.with_section(HumanSection::new(Some(HumanText::message(
            HumanMessageKey::Candidates,
        )))),
        |document, (index, candidate)| {
            document.with_section(HumanSection::new(None).with_message(HumanMessage::marked(
                format!("{}", index + 1),
                HumanText::literal(format!(
                    "{} · {} · {}",
                    candidate.alias(),
                    candidate.strategy().as_str(),
                    candidate.score()
                )),
                HumanTone::Plain,
            )))
        },
    )
}

fn alias_explain_document(
    document: HumanDocument,
    inspection: &WorkspaceAliasInspection,
) -> HumanDocument {
    let selected = inspection.selected_candidate();
    let detail = HumanSection::new(Some(HumanText::message(HumanMessageKey::AliasExplanation)))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::ProjectDisplayHint),
            HumanText::literal(inspection.analysis().normalized_name()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Tokens),
            HumanText::literal(inspection.analysis().tokens().join(" · ")),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::CandidateStrategy),
            HumanText::literal(
                selected.map_or("unavailable", |candidate| candidate.strategy().as_str()),
            ),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::CandidateScore),
            HumanText::literal(selected.map_or_else(
                || "unavailable".to_owned(),
                |candidate| candidate.score().to_string(),
            )),
            HumanTone::Plain,
        ));
    document.with_section(score_components_section(
        detail,
        selected.map(AliasCandidate::components),
    ))
}

fn score_components_section(
    section: HumanSection,
    components: Option<tabbeacon::repo::ScoreComponents>,
) -> HumanSection {
    let Some(components) = components else {
        return section.with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::CandidateComponents),
            HumanText::literal("unavailable"),
            HumanTone::Dim,
        ));
    };
    section
        .with_field(score_component_field(
            HumanMessageKey::TokenCoverage,
            components.token_coverage,
        ))
        .with_field(score_component_field(
            HumanMessageKey::AcronymPreservation,
            components.acronym_preservation,
        ))
        .with_field(score_component_field(
            HumanMessageKey::RecognizablePrefix,
            components.recognizable_prefix,
        ))
        .with_field(score_component_field(
            HumanMessageKey::BalancedRepresentation,
            components.balanced_representation,
        ))
        .with_field(score_component_field(
            HumanMessageKey::DisplayWidth,
            components.display_width,
        ))
        .with_field(score_component_field(
            HumanMessageKey::InformationLoss,
            components.information_loss,
        ))
        .with_field(score_component_field(
            HumanMessageKey::TrivialAliasPenalty,
            components.trivial_alias,
        ))
        .with_field(score_component_field(
            HumanMessageKey::RedundancyPenalty,
            components.redundancy,
        ))
        .with_field(score_component_field(
            HumanMessageKey::CollisionPressure,
            components.collision_pressure,
        ))
        .with_field(score_component_field(
            HumanMessageKey::StrategyAdjustment,
            components.strategy_adjustment,
        ))
        .with_field(score_component_field(
            HumanMessageKey::Total,
            components.total(),
        ))
}

fn score_component_field(key: HumanMessageKey, value: i32) -> HumanField {
    HumanField::new(
        None::<String>,
        HumanText::message(key),
        HumanText::literal(value.to_string()),
        HumanTone::Plain,
    )
}

fn print_alias_plain(operation: &str, inspection: &WorkspaceAliasInspection) {
    println!("ALIAS_SCHEMA_VERSION=2");
    println!("ALIAS_OPERATION={}", operation.to_ascii_uppercase());
    println!("WORKSPACE={}", inspection.workspace().as_str());
    println!("IDENTITY_CLASS={}", inspection.identity_class().as_str());
    println!("AUTOMATIC_ALIAS={}", inspection.automatic_alias());
    println!(
        "CUSTOM_ALIAS={}",
        inspection
            .custom_alias()
            .map_or("NONE", RepositoryAlias::as_str)
    );
    println!("EFFECTIVE_ALIAS={}", inspection.effective_alias());
    println!("NAMING_POLICY={}", inspection.policy_version());
    println!(
        "ASSIGNMENT_STATE={}",
        if inspection.is_assigned() {
            "ASSIGNED"
        } else {
            "PROSPECTIVE"
        }
    );
    if matches!(operation, "preview" | "explain") {
        for (index, candidate) in inspection.candidates().iter().take(5).enumerate() {
            let ordinal = index + 1;
            println!("CANDIDATE_{ordinal}_ALIAS={}", candidate.alias());
            println!(
                "CANDIDATE_{ordinal}_STRATEGY={}",
                candidate.strategy().as_str()
            );
            println!("CANDIDATE_{ordinal}_SCORE={}", candidate.score());
            println!(
                "CANDIDATE_{ordinal}_DISPLAY_WIDTH={}",
                candidate.display_width()
            );
        }
    }
    if operation == "explain" {
        println!("TOKENS={}", inspection.analysis().tokens().join(","));
        if let Some(candidate) = inspection.selected_candidate() {
            println!("SELECTED_CANDIDATE_ALIAS={}", candidate.alias());
            println!(
                "SELECTED_CANDIDATE_STRATEGY={}",
                candidate.strategy().as_str()
            );
            println!("SELECTED_CANDIDATE_SCORE={}", candidate.score());
            print_score_components_plain("SELECTED_CANDIDATE", candidate.components());
        } else {
            println!("SELECTED_CANDIDATE=UNAVAILABLE");
        }
    }
}

fn print_score_components_plain(prefix: &str, components: tabbeacon::repo::ScoreComponents) {
    println!("{prefix}_TOKEN_COVERAGE={}", components.token_coverage);
    println!(
        "{prefix}_ACRONYM_PRESERVATION={}",
        components.acronym_preservation
    );
    println!(
        "{prefix}_RECOGNIZABLE_PREFIX={}",
        components.recognizable_prefix
    );
    println!(
        "{prefix}_BALANCED_REPRESENTATION={}",
        components.balanced_representation
    );
    println!("{prefix}_DISPLAY_WIDTH={}", components.display_width);
    println!("{prefix}_INFORMATION_LOSS={}", components.information_loss);
    println!(
        "{prefix}_TRIVIAL_ALIAS_PENALTY={}",
        components.trivial_alias
    );
    println!("{prefix}_REDUNDANCY_PENALTY={}", components.redundancy);
    println!(
        "{prefix}_COLLISION_PRESSURE={}",
        components.collision_pressure
    );
    println!(
        "{prefix}_STRATEGY_ADJUSTMENT={}",
        components.strategy_adjustment
    );
    println!("{prefix}_TOTAL={}", components.total());
}

fn alias_json(operation: &str, inspection: &WorkspaceAliasInspection) -> serde_json::Value {
    let candidates = inspection
        .candidates()
        .iter()
        .take(5)
        .map(candidate_json)
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema": "tabbeacon-alias-v2",
        "operation": operation,
        "result": "success",
        "workspace": inspection.workspace().as_str(),
        "identity_class": inspection.identity_class().as_str(),
        "automatic_alias": inspection.automatic_alias().as_str(),
        "custom_alias": inspection.custom_alias().map(RepositoryAlias::as_str),
        "effective_alias": inspection.effective_alias().as_str(),
        "alias_source": if inspection.custom_alias().is_some() { "override" } else { "automatic" },
        "naming_policy": inspection.policy_version(),
        "assignment_state": if inspection.is_assigned() { "assigned" } else { "prospective" },
        "analysis": {
            "normalized_name": inspection.analysis().normalized_name(),
            "tokens": inspection.analysis().tokens(),
            "style_hints": inspection.analysis().style_hints().iter().map(|hint| hint.as_str()).collect::<Vec<_>>(),
        },
        "selected_candidate": inspection.selected_candidate().map(candidate_json),
        "candidates": candidates,
    })
}

fn candidate_json(candidate: &AliasCandidate) -> serde_json::Value {
    let components = candidate.components();
    serde_json::json!({
        "alias": candidate.alias().as_str(),
        "strategy": candidate.strategy().as_str(),
        "score": candidate.score(),
        "display_width": candidate.display_width(),
        "score_components": {
            "token_coverage": components.token_coverage,
            "acronym_preservation": components.acronym_preservation,
            "recognizable_prefix": components.recognizable_prefix,
            "balanced_representation": components.balanced_representation,
            "display_width": components.display_width,
            "information_loss": components.information_loss,
            "trivial_alias_penalty": components.trivial_alias,
            "redundancy_penalty": components.redundancy,
            "collision_pressure": components.collision_pressure,
            "strategy_adjustment": components.strategy_adjustment,
            "total": components.total(),
        },
    })
}

fn explain_title(output_mode: OutputMode, language: Option<InterfaceLanguage>) -> ExitCode {
    let diagnostics = collect_operational_diagnostics();
    let presentation = settings_store().ok().and_then(|store| {
        store
            .snapshot_read_only()
            .ok()
            .map(|snapshot| snapshot.settings())
    });
    let workspace = std::env::current_dir().ok().and_then(|cwd| {
        WorkspaceIdentityResolver::with_default_state_root()
            .ok()
            .and_then(|resolver| resolver.inspect_alias(cwd).ok())
    });
    let sessions = inspect_system_sessions();
    let explanation = TitleExplanation::from_observation(
        &diagnostics,
        presentation,
        workspace.as_ref(),
        &sessions,
    );
    match output_mode {
        OutputMode::Human => {
            print_human_document(&title_explanation_document(&explanation), language)
        }
        OutputMode::Plain => print_title_explanation_plain(&explanation),
        OutputMode::Json => match serde_json::to_string(&explanation) {
            Ok(value) => println!("{value}"),
            Err(error) => return management_error("TITLE_EXPLANATION", &error),
        },
    }
    ExitCode::SUCCESS
}

fn title_explanation_document(explanation: &TitleExplanation) -> HumanDocument {
    let mut section = HumanSection::new(None)
        .with_field(title_explanation_field(
            HumanMessageKey::Provider,
            explanation.provider,
        ))
        .with_field(title_explanation_field(
            HumanMessageKey::SemanticPhase,
            explanation.semantic_phase,
        ))
        .with_field(title_explanation_field(
            HumanMessageKey::Attention,
            explanation.attention,
        ))
        .with_field(title_explanation_field(
            HumanMessageKey::ActivityHealth,
            &explanation.activity_health,
        ))
        .with_field(title_explanation_field(
            HumanMessageKey::ActivityChannel,
            &explanation.activity_channel,
        ))
        .with_field(title_explanation_field(
            HumanMessageKey::SessionCorrelation,
            explanation.session_correlation,
        ))
        .with_field(title_explanation_field(
            HumanMessageKey::TitleOwner,
            &explanation.title_owner,
        ))
        .with_field(title_explanation_field(
            HumanMessageKey::CodexWriterState,
            &explanation.codex_writer_state,
        ))
        .with_field(title_explanation_field(
            HumanMessageKey::TitleAuthority,
            &explanation.title_authority,
        ))
        .with_field(title_explanation_field(
            HumanMessageKey::TitleConflict,
            &explanation.title_conflict,
        ))
        .with_field(title_explanation_field(
            HumanMessageKey::ProviderBadgePolicy,
            explanation.provider_badge_policy,
        ))
        .with_field(title_explanation_field(
            HumanMessageKey::ProviderBadgeValue,
            explanation.provider_badge_value,
        ));
    if let Some(workspace) = &explanation.workspace {
        section = section
            .with_field(title_explanation_field(
                HumanMessageKey::ProjectDisplayHint,
                &workspace.display_hint,
            ))
            .with_field(title_explanation_field(
                HumanMessageKey::IdentityClass,
                workspace.identity_class,
            ))
            .with_field(title_explanation_field(
                HumanMessageKey::RootBindingSource,
                workspace.root_binding_source,
            ))
            .with_field(title_explanation_field(
                HumanMessageKey::RootBindingStatus,
                workspace.root_binding_status,
            ))
            .with_field(title_explanation_field(
                HumanMessageKey::WorkspaceMismatch,
                workspace.workspace_mismatch_observation,
            ))
            .with_field(title_explanation_field(
                HumanMessageKey::AutomaticAlias,
                &workspace.automatic_alias,
            ))
            .with_field(title_explanation_field(
                HumanMessageKey::CustomAlias,
                workspace.override_alias.as_deref().unwrap_or("—"),
            ))
            .with_field(title_explanation_field(
                HumanMessageKey::EffectiveAlias,
                &workspace.effective_alias,
            ))
            .with_field(title_explanation_field(
                HumanMessageKey::AliasSource,
                workspace.alias_source,
            ))
            .with_field(title_explanation_field(
                HumanMessageKey::NamingPolicy,
                &workspace.naming_policy,
            ));
    }
    HumanDocument::new(HumanText::message(HumanMessageKey::WhyThisTitle), None)
        .with_section(section)
}

fn title_explanation_field(key: HumanMessageKey, value: impl Into<String>) -> HumanField {
    HumanField::new(
        None::<String>,
        HumanText::message(key),
        HumanText::literal(value.into()),
        HumanTone::Plain,
    )
}

fn print_title_explanation_plain(explanation: &TitleExplanation) {
    println!("TITLE_EXPLANATION_SCHEMA_VERSION=1");
    println!("PROVIDER={}", explanation.provider);
    println!("SEMANTIC_PHASE={}", explanation.semantic_phase);
    println!("ATTENTION={}", explanation.attention);
    println!("ACTIVITY_HEALTH={}", explanation.activity_health);
    println!("ACTIVITY_CHANNEL={}", explanation.activity_channel);
    println!("SESSION_CORRELATION={}", explanation.session_correlation);
    println!("TITLE_OWNER={}", explanation.title_owner);
    println!("CODEX_WRITER_STATE={}", explanation.codex_writer_state);
    println!("TITLE_AUTHORITY={}", explanation.title_authority);
    println!("TITLE_CONFLICT={}", explanation.title_conflict);
    println!(
        "PROVIDER_BADGE_POLICY={}",
        explanation.provider_badge_policy
    );
    println!("PROVIDER_BADGE_VALUE={}", explanation.provider_badge_value);
    if let Some(workspace) = &explanation.workspace {
        println!("PROJECT_DISPLAY_HINT={}", workspace.display_hint);
        println!("IDENTITY_CLASS={}", workspace.identity_class);
        println!("ROOT_BINDING_SOURCE={}", workspace.root_binding_source);
        println!("ROOT_BINDING_STATUS={}", workspace.root_binding_status);
        println!(
            "WORKSPACE_MISMATCH_OBSERVATION={}",
            workspace.workspace_mismatch_observation
        );
        println!("AUTOMATIC_ALIAS={}", workspace.automatic_alias);
        println!(
            "CUSTOM_ALIAS={}",
            workspace.override_alias.as_deref().unwrap_or("NONE")
        );
        println!("EFFECTIVE_ALIAS={}", workspace.effective_alias);
        println!("ALIAS_SOURCE={}", workspace.alias_source);
        println!("NAMING_POLICY={}", workspace.naming_policy);
    } else {
        println!("WORKSPACE=UNAVAILABLE");
    }
}

const fn alias_error_message_key(error: WorkspaceAliasError) -> HumanMessageKey {
    match error {
        WorkspaceAliasError::InvalidAlias => HumanMessageKey::AliasInvalid,
        WorkspaceAliasError::Collision => HumanMessageKey::AliasCollision,
        WorkspaceAliasError::Conflict => HumanMessageKey::AliasConflict,
        WorkspaceAliasError::Unavailable => HumanMessageKey::AliasUnavailable,
    }
}

fn convergence_matrix(json: bool) -> ExitCode {
    let matrix = tabbeacon::convergence::scenario_matrix();
    if json {
        return match serde_json::to_string(matrix) {
            Ok(value) => {
                println!("{value}");
                ExitCode::SUCCESS
            }
            Err(error) => management_error("CONVERGENCE_MATRIX", &error),
        };
    }
    println!(
        "CONVERGENCE_DEADLINE_MS={}",
        tabbeacon::convergence::CONVERGENCE_DEADLINE_MS
    );
    println!("CONVERGENCE_SCENARIOS={}", matrix.len());
    println!(
        "OWNED_UIA_SCENARIOS={}",
        tabbeacon::convergence::owned_uia_scenario_count()
    );
    println!("ELEVATED_OWNER_SCENARIOS=1");
    ExitCode::SUCCESS
}

fn convergence_verify(path: &std::path::Path, expected_head: &str) -> ExitCode {
    let run = match tabbeacon::convergence_evidence::load_convergence_run(path) {
        Ok(run) => run,
        Err(code) => {
            eprintln!("CONVERGENCE_VERIFY=FAIL");
            eprintln!("REASON={code}");
            return ExitCode::FAILURE;
        }
    };
    let mut verification = tabbeacon::convergence_evidence::verify_convergence_run(
        &run,
        tabbeacon::convergence::scenario_matrix(),
    );
    if run.expected_head != expected_head {
        verification.valid = false;
        verification
            .violations
            .push("caller_expected_head_mismatch".to_owned());
        verification.violations.sort();
        verification.violations.dedup();
    }
    match serde_json::to_string(&verification) {
        Ok(serialized) => println!("{serialized}"),
        Err(error) => return management_error("CONVERGENCE_VERIFY", &error),
    }
    if verification.valid {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn setup_codex(output_mode: OutputMode, language: Option<InterfaceLanguage>) -> ExitCode {
    let settings = settings_store().map_or_else(
        |_| PresentationSettings::default(),
        |store| store.load_or_default(),
    );
    let integration = match CodexIntegration::from_environment() {
        Ok(integration) => integration,
        Err(error) => return setup_management_error(&error, output_mode, language),
    };
    match integration.setup_with_title_ownership(settings.title().owns_tabbeacon_title()) {
        Ok(outcome) => print_setup_outcome(outcome, output_mode, language),
        Err(error) => setup_management_error(&error, output_mode, language),
    }
}

#[allow(clippy::too_many_lines)] // Coordinates the intentionally linear interactive setup flow.
fn guided_setup(quick: bool, full: bool, output: HumanOutputArgs) -> ExitCode {
    let output_mode = output.mode();
    let cli_language = output.language.preference();
    if !is_interactive_terminal() {
        return interactive_terminal_required(
            "SETUP",
            "guided setup requires an interactive terminal",
            "run tabbeacon setup from an interactive terminal, or use tabbeacon config and tabbeacon setup codex",
            output_mode,
            cli_language,
        );
    }
    let (store, snapshot, interface_store, interface_snapshot, integration, discovery) =
        match guided_setup_context(output_mode, cli_language) {
            Ok(context) => context,
            Err(exit_code) => return exit_code,
        };
    let before = snapshot.settings();
    let interface_before = interface_snapshot.preferences();
    let revisit_interface = should_revisit_interface_preferences(
        quick,
        full,
        snapshot.is_absent(),
        interface_snapshot.is_absent(),
    );
    let auto_locale = resolve_runtime_locale(cli_language, InterfaceLanguage::Auto).locale();
    let interface_draft =
        match prompt_setup_interface_draft(interface_before, revisit_interface, auto_locale) {
            Ok(Some(preferences)) => preferences,
            Ok(None) => {
                print_setup_cancelled(output_mode, cli_language);
                return ExitCode::SUCCESS;
            }
            Err(error) => return setup_input_error(&error, output_mode, cli_language),
        };
    // A newly staged `auto` must continue from process-local environment/OS
    // inputs, rather than consulting the still-unwritten previous preference.
    // Concrete choices deliberately win immediately for the rest of this
    // scrollback-oriented wizard.
    let selected_language = Some(match interface_draft.language() {
        InterfaceLanguage::Auto => {
            match resolve_runtime_locale(cli_language, InterfaceLanguage::Auto).locale() {
                ResolvedLocale::EnUs => InterfaceLanguage::EnUs,
                ResolvedLocale::ZhCn => InterfaceLanguage::ZhCn,
            }
        }
        language => language,
    });
    print_setup_discovery(&discovery, before, interface_draft, selected_language);
    print_setup_title_policy(
        &WindowsTerminalPolicyStore::from_environment().inspect(),
        selected_language,
    );

    if print_guided_setup_intro(quick, full, &discovery, output_mode, selected_language) {
        return ExitCode::SUCCESS;
    }

    let draft = match prompt_setup_draft_v3(before, selected_language) {
        Ok(Some(settings)) => settings,
        Ok(None) => {
            print_setup_cancelled(output_mode, selected_language);
            return ExitCode::SUCCESS;
        }
        Err(error) => return setup_input_error(&error, output_mode, selected_language),
    };
    let plan = GuidedSetupPlan::new(before, interface_before, discovery)
        .with_presentation_draft(draft)
        .with_interface_draft(interface_draft);
    print_setup_change_plan(&plan, selected_language);
    let preview_exit =
        print_preview_result(plan.preview_settings(), output_mode, selected_language);
    if preview_exit != ExitCode::SUCCESS {
        print_setup_preview_blocked(output_mode, selected_language);
        return preview_exit;
    }
    let decision = match prompt_setup_decision_v3(selected_language) {
        Ok(decision) => decision,
        Err(error) => return setup_input_error(&error, output_mode, selected_language),
    };
    match decision {
        SetupDecision::Cancel => {
            let _ = plan.cancel();
            print_setup_cancelled(output_mode, selected_language);
            ExitCode::SUCCESS
        }
        SetupDecision::Apply => match plan.apply(
            &store,
            &snapshot,
            &interface_store,
            &interface_snapshot,
            |owns_title| {
                integration
                    .setup_with_title_ownership(owns_title)
                    .map_err(|error| error.to_string())
            },
        ) {
            Ok(GuidedSetupApplyResult::Applied(outcome)) => {
                print_setup_applied(&store, plan.draft(), output_mode, selected_language);
                print_setup_outcome(outcome, output_mode, selected_language)
            }
            Ok(
                GuidedSetupApplyResult::SettingsConflict
                | GuidedSetupApplyResult::InterfaceConflict,
            ) => print_setup_settings_conflict(output_mode, selected_language),
            Ok(GuidedSetupApplyResult::SetupFailed {
                reason,
                settings_restored,
                interface_restored,
            }) => print_setup_apply_failure(
                &reason,
                settings_restored && interface_restored,
                output_mode,
                selected_language,
            ),
            Ok(GuidedSetupApplyResult::Cancelled) => unreachable!("apply path cannot cancel"),
            Err(error) => setup_management_error(&error, output_mode, selected_language),
        },
    }
}

fn print_guided_setup_intro(
    quick: bool,
    full: bool,
    discovery: &SetupDiscovery,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> bool {
    if quick
        && discovery.hooks() == tabbeacon::setup::HookSetupState::Current
        && discovery.profile_supported()
    {
        if output_mode == OutputMode::Plain {
            println!("SETUP=PASS");
            println!("SETUP_MODE=QUICK");
            println!("OWNER_ACTION=none");
        } else {
            print_human_document(
                &HumanDocument::new(HumanText::message(HumanMessageKey::SetupReady), None)
                    .with_section(HumanSection::new(None).with_message(HumanMessage::plain(
                        HumanText::message(HumanMessageKey::NoChangesNeeded),
                        HumanTone::Success,
                    ))),
                language,
            );
        }
        return true;
    }
    let mut section = HumanSection::new(None).with_message(HumanMessage::plain(
        HumanText::message(HumanMessageKey::WelcomeSetup),
        HumanTone::Plain,
    ));
    if full {
        section = section.with_message(HumanMessage::plain(
            HumanText::message(HumanMessageKey::FullSetup),
            HumanTone::Plain,
        ));
    } else if quick {
        section = section.with_message(HumanMessage::plain(
            HumanText::message(HumanMessageKey::QuickSetup),
            HumanTone::Plain,
        ));
    }
    print_human_document(
        &HumanDocument::new(HumanText::message(HumanMessageKey::Setup), None).with_section(section),
        language,
    );
    false
}

fn guided_setup_context(
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> Result<
    (
        PresentationSettingsStore,
        PresentationSettingsSnapshot,
        InterfacePreferencesStore,
        tabbeacon::interface_preferences::InterfacePreferencesSnapshot,
        CodexIntegration,
        SetupDiscovery,
    ),
    ExitCode,
> {
    let store =
        settings_store().map_err(|error| setup_management_error(&error, output_mode, language))?;
    let snapshot = store
        .snapshot_read_only()
        .map_err(|error| print_setup_snapshot_failure(&error, output_mode, language))?;
    let interface_store = InterfacePreferencesStore::from_environment()
        .map_err(|error| setup_management_error(&error, output_mode, language))?;
    let interface_snapshot = interface_store
        .snapshot_read_only()
        .map_err(|error| print_setup_snapshot_failure(&error, output_mode, language))?;
    let integration = CodexIntegration::from_environment()
        .map_err(|error| setup_management_error(&error, output_mode, language))?;
    let discovery = guided_setup_discovery(&integration, output_mode, language)?;
    Ok((
        store,
        snapshot,
        interface_store,
        interface_snapshot,
        integration,
        discovery,
    ))
}

fn guided_setup_discovery(
    integration: &CodexIntegration,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> Result<SetupDiscovery, ExitCode> {
    let binary_path = std::env::current_exe()
        .map_err(|error| setup_management_error(&error, output_mode, language))?;
    Ok(SetupDiscovery::from_doctor(
        env!("CARGO_PKG_VERSION"),
        binary_path,
        detect_windows_terminal(),
        &integration.doctor(),
    ))
}

fn print_setup_outcome(
    outcome: SetupOutcome,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    if output_mode != OutputMode::Plain {
        print_human_document(&setup_outcome_document(outcome), language);
        return ExitCode::SUCCESS;
    }

    println!("SETUP_IDEMPOTENCE=PASS");
    match outcome {
        SetupOutcome::InstalledTrustReviewRequired => {
            println!("CODEX_INTEGRATION=INSTALLED");
            println!("HOOK_TRUST=REVIEW_REQUIRED");
            println!(
                "OWNER_ACTION=launch codex, review TabBeacon hooks in /hooks, then run tabbeacon doctor"
            );
        }
        SetupOutcome::Upgraded => {
            println!("CODEX_INTEGRATION=UPGRADED");
            println!("HOOK_TRUST=REVIEW_REQUIRED");
            println!(
                "OWNER_ACTION=launch codex, review updated TabBeacon hooks in /hooks, then run tabbeacon doctor"
            );
        }
        SetupOutcome::AlreadyInstalled => {
            println!("CODEX_INTEGRATION=ALREADY_INSTALLED");
            println!("OWNER_ACTION=run tabbeacon doctor to verify hook trust and configuration");
        }
    }
    ExitCode::SUCCESS
}

fn setup_outcome_document(outcome: SetupOutcome) -> HumanDocument {
    let (summary, next) = match outcome {
        SetupOutcome::InstalledTrustReviewRequired => (
            HumanMessageKey::SetupInstalled,
            HumanMessageKey::SetupInstalledNext,
        ),
        SetupOutcome::Upgraded => (
            HumanMessageKey::SetupUpgraded,
            HumanMessageKey::SetupUpgradedNext,
        ),
        SetupOutcome::AlreadyInstalled => (
            HumanMessageKey::SetupAlreadyInstalled,
            HumanMessageKey::SetupAlreadyInstalledNext,
        ),
    };
    HumanDocument::new(HumanText::message(HumanMessageKey::Setup), None).with_section(
        HumanSection::new(None)
            .with_message(HumanMessage::plain(
                HumanText::message(summary),
                HumanTone::Success,
            ))
            .with_action(HumanAction::new(HumanText::message(next), HumanTone::Dim)),
    )
}

fn doctor(
    output_mode: OutputMode,
    probe_title: bool,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    let report = if probe_title {
        collect_operational_diagnostics_with_title_probe()
    } else {
        collect_operational_diagnostics()
    };
    if output_mode == OutputMode::Json {
        return match serde_json::to_string(&report.doctor) {
            Ok(json) => {
                println!("{json}");
                if report.doctor.is_failure() {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                }
            }
            Err(error) => management_error("DOCTOR", &error),
        };
    }
    if output_mode == OutputMode::Plain {
        for line in human_doctor_lines(&report.doctor) {
            println!("{line}");
        }
    } else {
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let presentation = human_runtime_presentation(language);
        print_human_lines(
            render_human_doctor(
                &report.doctor,
                &snapshot,
                presentation.locale,
                terminal_width(),
            ),
            presentation.color,
        );
    }
    if report.doctor.is_failure() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn status(output_mode: OutputMode, language: Option<InterfaceLanguage>) -> ExitCode {
    let report = collect_operational_diagnostics();
    if output_mode == OutputMode::Json {
        return match serde_json::to_string(&report) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => management_error("STATUS", &error),
        };
    }
    if output_mode == OutputMode::Plain {
        for line in human_status_lines(&report) {
            println!("{line}");
        }
    } else {
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        let presentation = human_runtime_presentation(language);
        print_human_lines(
            render_human_status(&report, &snapshot, presentation.locale, terminal_width()),
            presentation.color,
        );
    }
    ExitCode::SUCCESS
}

fn sessions(output_mode: OutputMode, language: Option<InterfaceLanguage>) -> ExitCode {
    let report = tabbeacon::activity::inspect_system_sessions();
    if output_mode == OutputMode::Json {
        return match serde_json::to_string(&report) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => management_error("SESSIONS", &error),
        };
    }

    if output_mode == OutputMode::Plain {
        println!("SESSIONS_SCHEMA_VERSION={}", report.schema_version);
        println!("SESSIONS_OBSERVATION={}", report.observation);
        println!("SESSIONS_HEALTH={}", report.health.as_str());
        println!("ACTIVE_SESSIONS={}", report.active_sessions);
        println!("STALE_SESSIONS={}", report.stale_sessions);
        println!("INVALID_LEASES={}", report.invalid_leases);
        for (index, session) in report.sessions.iter().enumerate() {
            let alias = serde_json::to_string(&session.workspace_alias)
                .unwrap_or_else(|_| "\"unavailable\"".to_owned());
            println!(
                "SESSION={}|workspace_alias={}|semantic_state={}|age_seconds={}|recency={}|worker_health={}",
                index + 1,
                alias,
                session.semantic_state,
                session.age_seconds,
                session.recency.as_str(),
                session.worker_health.as_str(),
            );
        }
        println!("SESSIONS_VIEW=PASS");
        println!("READ_ONLY=true");
        println!("RAW_NATIVE_SESSION_IDS=false");
        println!("PROMPT_CONTENT=false");
        println!("REMOTE_CONTROL=false");
        return ExitCode::SUCCESS;
    }

    print_human_document(&sessions_document(&report), language);
    ExitCode::SUCCESS
}

fn hooks(output_mode: OutputMode, language: Option<InterfaceLanguage>) -> ExitCode {
    let inventory = match CodexIntegration::from_environment() {
        Ok(integration) => integration.hook_inventory(),
        Err(_) => tabbeacon::hook_inventory::HookInventory::unavailable(),
    };
    match output_mode {
        OutputMode::Json => match serde_json::to_string(&inventory) {
            Ok(json) => println!("{json}"),
            Err(error) => return management_error("HOOKS", &error),
        },
        OutputMode::Plain => {
            for line in inventory.plain_lines() {
                println!("{line}");
            }
        }
        OutputMode::Human => {
            let presentation = human_runtime_presentation(language);
            println!("{}", inventory.human_table(presentation.locale));
        }
    }
    ExitCode::SUCCESS
}

fn upgrade_preflight(arguments: UpgradePreflightArgs) -> ExitCode {
    let report = inspect_system_upgrade_preflight(arguments.drain);
    match arguments.output.mode() {
        OutputMode::Json => match serde_json::to_string(&report) {
            Ok(json) => println!("{json}"),
            Err(error) => return management_error("UPGRADE_PREFLIGHT", &error),
        },
        OutputMode::Plain => print_upgrade_preflight_plain(&report),
        OutputMode::Human => print_upgrade_preflight_human(&report),
    }
    if report.process_inspection == UpgradeProcessInspection::Unavailable
        || report.target_executable.is_none()
    {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_upgrade_preflight_plain(report: &UpgradePreflight) {
    println!("UPGRADE_PREFLIGHT_SCHEMA_VERSION={}", report.schema_version);
    println!("TABBEACON_VERSION={}", report.tabbeacon_version);
    println!(
        "CURRENT_EXECUTABLE={}",
        report
            .current_executable
            .as_deref()
            .unwrap_or("unavailable")
    );
    println!("TARGET_SOURCE={}", report.target_source.as_str());
    println!(
        "TARGET_EXECUTABLE={}",
        report.target_executable.as_deref().unwrap_or("unavailable")
    );
    println!("PROCESS_INSPECTION={}", report.process_inspection.as_str());
    println!("WORKER_LEASE_HEALTH={}", report.worker_lease_health);
    println!("REPLACEABILITY={}", report.replaceability.as_str());
    println!("OWNED_WORKERS={}", report.proved_owned_worker_count());
    println!("AMBIGUOUS_PROCESSES={}", report.ambiguous_process_count());
    println!("DRAIN_REQUESTED={}", report.drain_requested);
    println!("DRAINED_OWNED_WORKERS={}", report.drained_owned_workers);
    println!(
        "UPGRADE_PREFLIGHT_DEFAULT_READ_ONLY={}",
        report.boundaries.default_read_only
    );
    println!(
        "EXPLICIT_DRAIN_ONLY={}",
        report.boundaries.explicit_drain_only
    );
    println!("RAW_COMMAND_LINES={}", report.boundaries.raw_command_lines);
    println!(
        "RAW_NATIVE_SESSION_IDS={}",
        report.boundaries.raw_native_session_ids
    );
    for worker in &report.workers {
        println!(
            "WORKER=process_id={}|ownership={}|drain={}",
            worker.process_id,
            worker.ownership.as_str(),
            worker.drain.as_str(),
        );
    }
    let disposition = if report.process_inspection == UpgradeProcessInspection::Unavailable
        || report.target_executable.is_none()
    {
        "UNPROVEN"
    } else {
        "PASS"
    };
    println!("UPGRADE_PREFLIGHT={disposition}");
}

fn print_upgrade_preflight_human(report: &UpgradePreflight) {
    print_human_tone(HumanTone::Plain, "Upgrade preflight");
    print_human_tone(
        HumanTone::Plain,
        format!("Version: {}", report.tabbeacon_version),
    );
    print_human_tone(
        HumanTone::Plain,
        format!(
            "Current executable: {}",
            report
                .current_executable
                .as_deref()
                .unwrap_or("unavailable")
        ),
    );
    print_human_tone(
        HumanTone::Plain,
        format!(
            "Upgrade target: {} ({})",
            report.target_executable.as_deref().unwrap_or("unavailable"),
            report.target_source.as_str()
        ),
    );
    print_human_tone(
        if report.replaceability == UpgradeReplaceability::Blocked {
            HumanTone::Attention
        } else {
            HumanTone::Plain
        },
        format!("Replaceability: {}", report.replaceability.as_str()),
    );
    print_human_tone(
        HumanTone::Plain,
        format!(
            "Proved owned workers: {}; preserved ambiguous processes: {}",
            report.proved_owned_worker_count(),
            report.ambiguous_process_count()
        ),
    );
    if report.drain_requested {
        print_human_tone(
            HumanTone::Plain,
            format!(
                "Explicit drain stopped {} proved worker(s).",
                report.drained_owned_workers
            ),
        );
    } else {
        print_human_tone(
            HumanTone::Dim,
            "Read-only. Use --drain only to stop freshly proven TabBeacon activity workers.",
        );
    }
}

fn sessions_document(report: &tabbeacon::activity::SessionsOverview) -> HumanDocument {
    let mut section = HumanSection::new(None)
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Active),
            HumanText::literal(report.active_sessions.to_string()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Stale),
            HumanText::literal(report.stale_sessions.to_string()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::InvalidLeases),
            HumanText::literal(report.invalid_leases.to_string()),
            HumanTone::Plain,
        ));
    if report.sessions.is_empty() {
        section = section.with_message(HumanMessage::plain(
            HumanText::message(HumanMessageKey::NoInspectableSessionLeases),
            HumanTone::Dim,
        ));
    } else {
        for session in &report.sessions {
            section = section.with_message(HumanMessage::plain(
                HumanText::literal(format!(
                    "{} — {} — {}s — {}",
                    session.workspace_alias,
                    session.semantic_state,
                    session.age_seconds,
                    session.worker_health.as_str().replace('_', " "),
                )),
                HumanTone::Plain,
            ));
        }
    }
    HumanDocument::new(HumanText::message(HumanMessageKey::Sessions), None).with_section(
        section.with_message(HumanMessage::plain(
            HumanText::message(HumanMessageKey::LeaseObservationOnly),
            HumanTone::Dim,
        )),
    )
}

/// Reports Windows Terminal policy without creating state or writing a tab.
fn title_policy_inspect(json: bool) -> ExitCode {
    let policy = WindowsTerminalPolicyStore::from_environment().inspect();
    if json {
        return match serde_json::to_string(&policy) {
            Ok(value) => {
                println!("{value}");
                ExitCode::SUCCESS
            }
            Err(error) => management_error("TITLE_POLICY_INSPECT", &error),
        };
    }
    print_title_policy("INSPECT", &policy, None);
    ExitCode::SUCCESS
}

/// Applies the explicit, profile-scoped Windows Terminal remediation.
fn title_policy_repair(json: bool) -> ExitCode {
    let store = WindowsTerminalPolicyStore::from_environment();
    let policy = store.inspect();
    match store.repair() {
        Ok(result) if json => match serde_json::to_string(&serde_json::json!({
            "policy": policy,
            "result": result,
        })) {
            Ok(value) => {
                println!("{value}");
                ExitCode::SUCCESS
            }
            Err(error) => management_error("TITLE_POLICY_REPAIR", &error),
        },
        Ok(result) => {
            print_title_policy("REPAIR", &policy, Some(result));
            ExitCode::SUCCESS
        }
        Err(error) => management_error("TITLE_POLICY_REPAIR", &error),
    }
}

/// Restores only an exact previously-owned policy target.
fn title_policy_restore(json: bool) -> ExitCode {
    let store = WindowsTerminalPolicyStore::from_environment();
    let policy = store.inspect();
    match store.restore() {
        Ok(result) if json => match serde_json::to_string(&serde_json::json!({
            "policy": policy,
            "result": result,
        })) {
            Ok(value) => {
                println!("{value}");
                ExitCode::SUCCESS
            }
            Err(error) => management_error("TITLE_POLICY_RESTORE", &error),
        },
        Ok(result) => {
            print_title_policy("RESTORE", &policy, Some(result));
            ExitCode::SUCCESS
        }
        Err(error) => management_error("TITLE_POLICY_RESTORE", &error),
    }
}

fn print_title_policy(
    operation: &str,
    policy: &tabbeacon::windows_terminal_policy::TitlePolicyDiagnostics,
    result: Option<tabbeacon::windows_terminal_policy::TitleRemediationResult>,
) {
    println!("TITLE_POLICY_OPERATION={operation}");
    println!("WT_SETTINGS_SOURCE={}", policy.settings_source.as_str());
    println!(
        "ACTIVE_PROFILE_RESOLUTION={}",
        policy.active_profile_resolution.as_str()
    );
    println!(
        "APPLICATION_TITLE_POLICY={}",
        policy.application_title_policy.as_str()
    );
    println!("TITLE_POLICY_SOURCE={}", policy.policy_source.as_str());
    println!("TITLE_REMEDIATION={}", policy.remediation.as_str());
    println!("TITLE_REMEDIATION_SCOPE={}", policy.remediation_scope);
    if let Some(result) = result {
        println!("REMEDIATION_RESULT={}", result.state.as_str());
        println!("SETTINGS_DOCUMENT_MODIFIED={}", result.document_modified);
        println!("USER_CONFIG_PRESERVED={}", result.user_config_preserved);
    }
    println!("PERSISTENT_REMEDIATION_EXPLICIT=true");
}

fn print_setup_title_policy(
    policy: &tabbeacon::windows_terminal_policy::TitlePolicyDiagnostics,
    language: Option<InterfaceLanguage>,
) {
    let mut section = HumanSection::new(None)
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Title),
            HumanText::literal(policy.application_title_policy.as_str()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Runtime),
            HumanText::literal(policy.policy_source.as_str()),
            HumanTone::Plain,
        ));
    if policy.remediation.as_str() == "available" {
        section = section.with_action(HumanAction::new(
            HumanText::literal("tabbeacon title-policy repair"),
            HumanTone::Attention,
        ));
    }
    print_human_document(
        &HumanDocument::new(
            HumanText::message(HumanMessageKey::WindowsTerminalTitlePolicy),
            None,
        )
        .with_section(section),
        language,
    );
}

fn uninstall_codex(output_mode: OutputMode) -> ExitCode {
    let integration = match CodexIntegration::from_environment() {
        Ok(integration) => integration,
        Err(error) => {
            return management_error_for_output(
                "UNINSTALL",
                &error,
                output_mode,
                Some(InterfaceLanguage::EnUs),
            );
        }
    };
    match integration.uninstall() {
        Ok(UninstallOutcome::Removed) => {
            if output_mode == OutputMode::Plain {
                println!("UNINSTALL_SAFETY=PASS");
                println!("CODEX_INTEGRATION=REMOVED");
                println!("OWNER_ACTION=none");
            } else {
                print_human_tone(
                    HumanTone::Success,
                    "TabBeacon removed its owned Codex integration.",
                );
            }
            ExitCode::SUCCESS
        }
        Ok(UninstallOutcome::NotInstalled) => {
            if output_mode == OutputMode::Plain {
                println!("UNINSTALL_SAFETY=PASS");
                println!("CODEX_INTEGRATION=NOT_INSTALLED");
                println!("OWNER_ACTION=none");
            } else {
                print_human_tone(
                    HumanTone::Success,
                    "No owned Codex integration is installed.",
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => management_error_for_output(
            "UNINSTALL",
            &error,
            output_mode,
            Some(InterfaceLanguage::EnUs),
        ),
    }
}

fn run_codex_hook() -> ExitCode {
    let mut input = Vec::new();
    let mut bounded = std::io::stdin().take(MAX_HOOK_INPUT_BYTES + 1);
    if bounded.read_to_end(&mut input).is_ok() && input.len() as u64 <= MAX_HOOK_INPUT_BYTES {
        let _ = CodexHookRuntime::dispatch_system(&input);
    }
    // Hook ingress is intentionally silent and fail open. In particular it
    // emits no hook control JSON and never blocks an agent operation.
    ExitCode::SUCCESS
}

fn config_show(output_mode: OutputMode, language: Option<InterfaceLanguage>) -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error_for_output("CONFIG", &error, output_mode, language),
    };
    let settings = match store.load() {
        Ok(settings) => settings,
        Err(error) => {
            if output_mode == OutputMode::Plain {
                eprintln!("CONFIG=WARNING");
                eprintln!("REASON={error}");
            } else {
                eprint_human_text(
                    HumanTone::Attention,
                    &HumanText::message(HumanMessageKey::SavedPresentationSettingsUnreadable),
                    language,
                );
            }
            PresentationSettings::default()
        }
    };
    print_settings(&store, settings, output_mode, language);
    ExitCode::SUCCESS
}

fn config_set(
    key: &str,
    value: &str,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error_for_output("CONFIG", &error, output_mode, language),
    };
    let current = match store.load() {
        Ok(settings) => settings,
        Err(error) => return management_error_for_output("CONFIG", &error, output_mode, language),
    };
    let updated = match key {
        "title" => TitleMode::parse(value).map(|mode| current.with_title(mode)),
        "tab-color" => TabColorMode::parse(value).map(|mode| current.with_tab_color(mode)),
        "activity" => ActivityMode::parse(value).map(|mode| current.with_activity(mode)),
        "spinner" => SpinnerPreset::parse(value).map(|preset| current.with_spinner(preset)),
        "theme" => PresentationTheme::parse(value).map(|theme| current.with_theme(theme)),
        _ => None,
    };
    let Some(updated) = updated else {
        print_config_failure(output_mode, "unsupported config key or value", language);
        print_config_choices(key, output_mode, language);
        return ExitCode::from(2);
    };
    persist_settings_change(&store, current, updated, output_mode, language)
}

fn config_reset(output_mode: OutputMode, language: Option<InterfaceLanguage>) -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error_for_output("CONFIG", &error, output_mode, language),
    };
    let current = store.load_or_default();
    let defaults = match store.reset() {
        Ok(settings) => settings,
        Err(error) => return management_error_for_output("CONFIG", &error, output_mode, language),
    };
    if current.title() != defaults.title() {
        match CodexIntegration::from_environment()
            .and_then(|integration| integration.reconcile_title_ownership(true))
        {
            Ok(outcome) if output_mode == OutputMode::Plain => {
                println!("CODEX_TITLE_OWNERSHIP={}", title_ownership_label(outcome));
            }
            Ok(_) => print_human_text(
                HumanTone::Dim,
                &HumanText::message(HumanMessageKey::TitleOwnershipReconciled),
                language,
            ),
            Err(error) => {
                return management_error_for_output("CONFIG", &error, output_mode, language);
            }
        }
    }
    if output_mode == OutputMode::Plain {
        println!("CONFIG=PASS");
    } else {
        print_human_document(
            &HumanDocument::new(
                HumanText::message(HumanMessageKey::PresentationSettingsReset),
                None,
            ),
            language,
        );
    }
    print_settings(&store, defaults, output_mode, language);
    ExitCode::SUCCESS
}

fn config_preset(
    name: &str,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error_for_output("CONFIG", &error, output_mode, language),
    };
    let current = match store.load() {
        Ok(settings) => settings,
        Err(error) => return management_error_for_output("CONFIG", &error, output_mode, language),
    };
    let Some(preset) = PresentationSettings::preset(name) else {
        print_config_failure(output_mode, "unsupported preset", language);
        if output_mode == OutputMode::Plain {
            eprintln!("PRESETS=native|minimal|balanced|terminal-ring|full");
        } else {
            eprint_human_text(
                HumanTone::Dim,
                &HumanText::message(HumanMessageKey::SupportedPresets),
                language,
            );
        }
        return ExitCode::from(2);
    };
    persist_settings_change(&store, current, preset, output_mode, language)
}

fn config_wizard(output_mode: OutputMode, language: Option<InterfaceLanguage>) -> ExitCode {
    if !is_interactive_terminal() {
        return interactive_terminal_required(
            "CONFIG",
            "config wizard requires an interactive terminal",
            "run tabbeacon config wizard from an interactive terminal, or use tabbeacon config set",
            output_mode,
            language,
        );
    }
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error_for_output("CONFIG", &error, output_mode, language),
    };
    let current = store.load_or_default();
    if output_mode == OutputMode::Plain {
        println!("TabBeacon presentation wizard (press Enter to keep each current value).");
    } else {
        print_human_text(
            HumanTone::Plain,
            &HumanText::message(HumanMessageKey::PresentationWizard),
            language,
        );
    }
    let title = match prompt_choice("title", current.title().as_str(), TitleMode::parse) {
        Ok(value) => value,
        Err(error) => return wizard_error(&error, output_mode, language),
    };
    let tab_color = match prompt_choice(
        "tab-color",
        current.tab_color().as_str(),
        TabColorMode::parse,
    ) {
        Ok(value) => value,
        Err(error) => return wizard_error(&error, output_mode, language),
    };
    let activity = match prompt_choice("activity", current.activity().as_str(), ActivityMode::parse)
    {
        Ok(value) => value,
        Err(error) => return wizard_error(&error, output_mode, language),
    };
    let spinner = match prompt_choice("spinner", current.spinner().as_str(), SpinnerPreset::parse) {
        Ok(value) => value,
        Err(error) => return wizard_error(&error, output_mode, language),
    };
    let theme = match prompt_choice("theme", current.theme().as_str(), PresentationTheme::parse) {
        Ok(value) => value,
        Err(error) => return wizard_error(&error, output_mode, language),
    };
    persist_settings_change(
        &store,
        current,
        PresentationSettings::new(title, tab_color, activity, spinner, theme),
        output_mode,
        language,
    )
}

#[allow(clippy::too_many_lines)] // One grouped document mirrors the visible Setup summary.
fn print_setup_discovery(
    discovery: &SetupDiscovery,
    settings: PresentationSettings,
    interface: InterfacePreferences,
    language: Option<InterfaceLanguage>,
) {
    let locale = human_runtime_presentation(language).locale;
    let terminal_state = match discovery.windows_terminal() {
        WindowsTerminalState::CurrentSession => HumanMessageKey::WindowsTerminalCurrentSession,
        WindowsTerminalState::NotCurrentSession => {
            HumanMessageKey::WindowsTerminalNotCurrentSession
        }
    };
    let profile_status = if discovery.profile_supported() {
        HumanMessageKey::Supported
    } else {
        HumanMessageKey::NotAdmitted
    };
    let environment = HumanSection::new(Some(HumanText::message(HumanMessageKey::Environment)))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::WindowsTerminal),
            HumanText::message(terminal_state),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Codex),
            HumanText::template(
                HumanMessageKey::SetupCodexSummary,
                [
                    discovery
                        .codex_version()
                        .unwrap_or(tabbeacon::human_presentation::catalog(
                            locale,
                            HumanMessageKey::Unavailable,
                        ))
                        .to_owned(),
                    discovery
                        .hook_profile()
                        .unwrap_or(tabbeacon::human_presentation::catalog(
                            locale,
                            HumanMessageKey::Unknown,
                        ))
                        .to_owned(),
                    tabbeacon::human_presentation::catalog(locale, profile_status).to_owned(),
                ],
            ),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::TabBeacon),
            HumanText::literal(discovery.tabbeacon_version()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Binary),
            HumanText::literal(discovery.binary_path().display().to_string()),
            HumanTone::Dim,
        ));
    let presentation = HumanSection::new(Some(HumanText::message(HumanMessageKey::Presentation)))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Title),
            human_title_text(settings.title()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::TabColor),
            human_tab_color_text(settings.tab_color()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Activity),
            human_activity_text(settings.activity()),
            HumanTone::Plain,
        ));
    let interface_section = HumanSection::new(Some(HumanText::message(HumanMessageKey::Interface)))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Language),
            human_interface_language_text(interface.language()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Color),
            human_color_text(interface.color()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::ReducedMotion),
            human_boolean_text(interface.reduced_motion()),
            HumanTone::Plain,
        ));
    print_human_document(
        &HumanDocument::new(HumanText::message(HumanMessageKey::Setup), None)
            .with_section(environment)
            .with_section(presentation)
            .with_section(interface_section),
        language,
    );
}

fn print_setup_change_plan(plan: &GuidedSetupPlan, language: Option<InterfaceLanguage>) {
    let section = HumanSection::new(Some(HumanText::message(HumanMessageKey::PlannedChanges)))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Title),
            HumanText::literal(format!(
                "{} → {}",
                plan.before().title(),
                plan.draft().title()
            )),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Language),
            HumanText::literal(format!(
                "{} → {}",
                plan.interface_before().language(),
                plan.interface_draft().language()
            )),
            HumanTone::Plain,
        ))
        .with_message(HumanMessage::plain(
            HumanText::message(HumanMessageKey::UnchangedOwnedState),
            HumanTone::Dim,
        ))
        .with_message(HumanMessage::plain(
            HumanText::message(HumanMessageKey::PreservedExternalSettings),
            HumanTone::Dim,
        ));
    print_human_document(
        &HumanDocument::new(HumanText::message(HumanMessageKey::Setup), None).with_section(section),
        language,
    );
}

struct DialoguerInput {
    locale: ResolvedLocale,
}

impl GuidedInput for DialoguerInput {
    fn select(&mut self, prompt: &str, items: &[&str], default: usize) -> Result<usize, String> {
        let localized_items = items
            .iter()
            .map(|item| localize_setup_choice(self.locale, item))
            .collect::<Vec<_>>();
        Select::new()
            .with_prompt(localize_setup_choice(self.locale, prompt))
            .items(&localized_items)
            .default(default)
            .interact()
            .map_err(|_| "guided setup selection was interrupted".to_owned())
    }
}

fn prompt_setup_draft_v3(
    current: PresentationSettings,
    language: Option<InterfaceLanguage>,
) -> Result<Option<PresentationSettings>, String> {
    let mut input = DialoguerInput {
        locale: human_runtime_presentation(language).locale,
    };
    choose_presentation(&mut input, current)
}

#[allow(clippy::fn_params_excessive_bools)] // Bounded setup admission facts are clearer as booleans.
const fn should_revisit_interface_preferences(
    quick: bool,
    full: bool,
    presentation_is_absent: bool,
    interface_is_absent: bool,
) -> bool {
    full || presentation_is_absent || (!quick && interface_is_absent)
}

fn prompt_setup_interface_draft(
    current: InterfacePreferences,
    revisit: bool,
    auto_locale: ResolvedLocale,
) -> Result<Option<InterfacePreferences>, String> {
    let mut input = DialoguerInput {
        locale: auto_locale,
    };
    choose_interface_preferences(&mut input, current, revisit, auto_locale)
}

fn prompt_setup_decision_v3(language: Option<InterfaceLanguage>) -> Result<SetupDecision, String> {
    let mut input = DialoguerInput {
        locale: human_runtime_presentation(language).locale,
    };
    match input.select("Apply staged setup", &["Apply", "Cancel"], 1)? {
        0 => Ok(SetupDecision::Apply),
        1 => Ok(SetupDecision::Cancel),
        _ => Err("invalid setup decision".to_owned()),
    }
}

fn localize_setup_choice(locale: ResolvedLocale, value: &str) -> &str {
    if locale == ResolvedLocale::EnUs {
        return value;
    }
    match value {
        "Choose presentation" => "选择外观呈现",
        "Recommended" => "推荐",
        "Minimal" => "简洁",
        "Full" => "完整",
        "Native" => "原生",
        "Customize" => "自定义",
        "Back" => "返回",
        "Preset" => "预设",
        "Use this preset" => "使用此预设",
        "Title" => "标题",
        "Tab color" => "标签颜色",
        "Activity" => "活动",
        "Spinner" => "旋转指示器",
        "Theme" => "主题",
        "Done" => "完成",
        "Title spinner" => "标题旋转指示器",
        "Title indicator" => "标题指示器",
        "Terminal ring" => "终端圆环",
        "Both" => "两者",
        "Off" => "关闭",
        "Muted Dark" => "低调深色",
        "Classic" => "经典",
        "Apply staged setup" => "应用暂存设置",
        "Apply" => "应用",
        "Cancel" => "取消",
        _ => value,
    }
}

fn setup_input_error(
    error: &str,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    if output_mode == OutputMode::Plain {
        eprintln!("SETUP=FAIL");
        eprintln!("REASON={error}");
        eprintln!("SETTINGS_UNCHANGED=true");
        eprintln!("CODEX_CONFIG_UNCHANGED=true");
        eprintln!("HOOKS_UNCHANGED=true");
    } else {
        eprint_human_text(
            HumanTone::Failure,
            &HumanText::template(HumanMessageKey::SetupInputFailed, [error]),
            language,
        );
        eprint_human_text(
            HumanTone::Dim,
            &HumanText::message(HumanMessageKey::NoSetupChangesMade),
            language,
        );
    }
    ExitCode::from(2)
}

fn apply_settings_change(
    store: &PresentationSettingsStore,
    before: PresentationSettings,
    after: PresentationSettings,
) -> io::Result<TitleOwnershipOutcome> {
    match store
        .save_if_unchanged(before, after)
        .map_err(io::Error::other)?
    {
        ConditionalSaveOutcome::Saved => {}
        ConditionalSaveOutcome::Conflict => return Err(settings_conflict_error()),
    }
    let title_outcome = if before.title() == after.title() {
        TitleOwnershipOutcome::AlreadyConfigured
    } else {
        match CodexIntegration::from_environment().and_then(|integration| {
            integration.reconcile_title_ownership(after.title().owns_tabbeacon_title())
        }) {
            Ok(outcome) => outcome,
            Err(error) => {
                let restored = matches!(
                    store.save_if_unchanged(after, before),
                    Ok(ConditionalSaveOutcome::Saved)
                );
                let reason = if restored {
                    error.to_string()
                } else {
                    format!(
                        "{error}; settings rollback refused because the document changed concurrently"
                    )
                };
                return Err(io::Error::other(reason));
            }
        }
    };
    Ok(title_outcome)
}

fn apply_control_center_settings_change(
    store: &PresentationSettingsStore,
    snapshot: &PresentationSettingsSnapshot,
    before: PresentationSettings,
    after: PresentationSettings,
) -> io::Result<(TitleOwnershipOutcome, PresentationSettingsSnapshot)> {
    apply_control_center_settings_change_with(store, snapshot, before, after, |owns_title| {
        CodexIntegration::from_environment()
            .and_then(|integration| integration.reconcile_title_ownership(owns_title))
            .map_err(|error| error.to_string())
    })
}

fn apply_control_center_settings_change_with(
    store: &PresentationSettingsStore,
    snapshot: &PresentationSettingsSnapshot,
    before: PresentationSettings,
    after: PresentationSettings,
    reconcile: impl FnOnce(bool) -> Result<TitleOwnershipOutcome, String>,
) -> io::Result<(TitleOwnershipOutcome, PresentationSettingsSnapshot)> {
    if snapshot.settings() != before {
        return Err(settings_conflict_error());
    }
    let receipt = match store
        .save_snapshot_if_unchanged(snapshot, after)
        .map_err(io::Error::other)?
    {
        SnapshotSaveOutcome::Saved(receipt) => receipt,
        SnapshotSaveOutcome::Conflict => return Err(settings_conflict_error()),
    };
    let title_outcome = if before.title() == after.title() {
        TitleOwnershipOutcome::AlreadyConfigured
    } else {
        match reconcile(after.title().owns_tabbeacon_title()) {
            Ok(outcome) => outcome,
            Err(error) => {
                let restored = matches!(
                    store.restore_snapshot_if_unchanged(&receipt, snapshot),
                    Ok(ConditionalSaveOutcome::Saved)
                );
                let reason = if restored {
                    error
                } else {
                    format!(
                        "{error}; settings rollback refused because the document changed concurrently"
                    )
                };
                return Err(io::Error::other(reason));
            }
        }
    };
    let next_snapshot = store.snapshot_read_only().map_err(io::Error::other)?;
    if next_snapshot.settings() != after {
        return Err(settings_conflict_error());
    }
    Ok((title_outcome, next_snapshot))
}

fn settings_conflict_error() -> io::Error {
    io::Error::other("settings changed concurrently; the stale draft was not applied")
}

fn persist_settings_change(
    store: &PresentationSettingsStore,
    before: PresentationSettings,
    after: PresentationSettings,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    let title_outcome = match apply_settings_change(store, before, after) {
        Ok(outcome) => outcome,
        Err(error) => return management_error_for_output("CONFIG", &error, output_mode, language),
    };
    if output_mode == OutputMode::Plain {
        println!("CONFIG=PASS");
        println!(
            "CODEX_TITLE_OWNERSHIP={}",
            title_ownership_label(title_outcome)
        );
    } else {
        print_human_document(
            &HumanDocument::new(
                HumanText::message(HumanMessageKey::PresentationSettingsUpdated),
                None,
            ),
            language,
        );
        print_human_text(
            HumanTone::Dim,
            &HumanText::message(HumanMessageKey::TitleOwnershipReconciled),
            language,
        );
    }
    print_settings(store, after, output_mode, language);
    ExitCode::SUCCESS
}

fn title_ownership_label(outcome: TitleOwnershipOutcome) -> &'static str {
    match outcome {
        TitleOwnershipOutcome::Updated => "UPDATED",
        TitleOwnershipOutcome::AlreadyConfigured => "ALREADY_CONFIGURED",
        TitleOwnershipOutcome::NotInstalled => "NOT_INSTALLED",
    }
}

fn print_settings(
    store: &PresentationSettingsStore,
    settings: PresentationSettings,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) {
    if output_mode != OutputMode::Plain {
        print_human_document(&presentation_settings_document(settings), language);
        return;
    }
    println!("CONFIG_PATH={}", store.path().display());
    println!("TITLE_MODE={}", settings.title());
    println!("TAB_COLOR_MODE={}", settings.tab_color());
    println!("ACTIVITY_MODE={}", settings.activity());
    println!("SPINNER_PRESET={}", settings.spinner());
    println!("THEME={}", settings.theme());
    println!("TITLE_SPINNER_FEASIBILITY=PRODUCTION");
}

fn presentation_settings_document(settings: PresentationSettings) -> HumanDocument {
    HumanDocument::new(
        HumanText::message(HumanMessageKey::PresentationSettings),
        None,
    )
    .with_section(
        HumanSection::new(None)
            .with_field(HumanField::new(
                None::<String>,
                HumanText::message(HumanMessageKey::Title),
                human_title_text(settings.title()),
                HumanTone::Plain,
            ))
            .with_field(HumanField::new(
                None::<String>,
                HumanText::message(HumanMessageKey::TabColor),
                human_tab_color_text(settings.tab_color()),
                HumanTone::Plain,
            ))
            .with_field(HumanField::new(
                None::<String>,
                HumanText::message(HumanMessageKey::Activity),
                human_activity_text(settings.activity()),
                HumanTone::Plain,
            ))
            .with_field(HumanField::new(
                None::<String>,
                HumanText::message(HumanMessageKey::Spinner),
                human_spinner_text(settings.spinner()),
                HumanTone::Plain,
            ))
            .with_field(HumanField::new(
                None::<String>,
                HumanText::message(HumanMessageKey::Theme),
                human_theme_text(settings.theme()),
                HumanTone::Plain,
            ))
            .with_message(HumanMessage::plain(
                HumanText::message(HumanMessageKey::UserLocalState),
                HumanTone::Dim,
            )),
    )
}

fn human_title_text(value: TitleMode) -> HumanText {
    HumanText::message(match value {
        TitleMode::TabBeacon => HumanMessageKey::TabBeacon,
        TitleMode::Native => HumanMessageKey::Native,
        TitleMode::Off => HumanMessageKey::Disabled,
    })
}

fn human_tab_color_text(value: TabColorMode) -> HumanText {
    HumanText::message(match value {
        TabColorMode::TabBeacon => HumanMessageKey::TabBeaconColors,
        TabColorMode::Native => HumanMessageKey::NativeColors,
        TabColorMode::Off => HumanMessageKey::Disabled,
    })
}

fn human_activity_text(value: ActivityMode) -> HumanText {
    HumanText::message(match value {
        ActivityMode::TitleSpinner => HumanMessageKey::TitleSpinner,
        ActivityMode::TitleIndicator => HumanMessageKey::TitleIndicator,
        ActivityMode::WindowsTerminalRing => HumanMessageKey::TerminalRing,
        ActivityMode::Both => HumanMessageKey::TitleSpinnerAndRing,
        ActivityMode::Native => HumanMessageKey::Native,
        ActivityMode::Off => HumanMessageKey::Disabled,
    })
}

fn human_spinner_text(value: SpinnerPreset) -> HumanText {
    HumanText::message(match value {
        SpinnerPreset::Codex => HumanMessageKey::Codex,
        SpinnerPreset::Braille => HumanMessageKey::BrailleSpinner,
        SpinnerPreset::Quadrant => HumanMessageKey::QuadrantSpinner,
        SpinnerPreset::Line => HumanMessageKey::LineSpinner,
        SpinnerPreset::Pulse => HumanMessageKey::PulseSpinner,
    })
}

fn human_theme_text(value: PresentationTheme) -> HumanText {
    HumanText::message(match value {
        PresentationTheme::MutedDark => HumanMessageKey::MutedDark,
        PresentationTheme::Classic => HumanMessageKey::ClassicTheme,
    })
}

fn human_interface_language_text(value: InterfaceLanguage) -> HumanText {
    HumanText::message(match value {
        InterfaceLanguage::Auto => HumanMessageKey::Auto,
        InterfaceLanguage::EnUs => HumanMessageKey::English,
        InterfaceLanguage::ZhCn => HumanMessageKey::SimplifiedChinese,
    })
}

fn human_color_text(value: HumanColor) -> HumanText {
    HumanText::message(match value {
        HumanColor::Auto => HumanMessageKey::Auto,
        HumanColor::Always => HumanMessageKey::Always,
        HumanColor::Never => HumanMessageKey::Never,
    })
}

fn human_boolean_text(value: bool) -> HumanText {
    HumanText::message(if value {
        HumanMessageKey::Enabled
    } else {
        HumanMessageKey::Disabled
    })
}

fn print_config_choices(key: &str, output_mode: OutputMode, language: Option<InterfaceLanguage>) {
    if output_mode != OutputMode::Plain {
        eprintln!(
            "{}",
            tabbeacon::human_presentation::catalog(
                human_runtime_presentation(language).locale,
                HumanMessageKey::UseConfigShow,
            )
        );
        return;
    }
    match key {
        "title" => eprintln!("TITLE_CHOICES=tabbeacon|native|off"),
        "tab-color" => eprintln!("TAB_COLOR_CHOICES=tabbeacon|native|off"),
        "activity" => {
            eprintln!("ACTIVITY_CHOICES=title-spinner|title-indicator|wt-ring|both|native|off");
        }
        "spinner" => eprintln!("SPINNER_CHOICES=codex|braille|quadrant|line|pulse"),
        "theme" => eprintln!("THEME_CHOICES=muted-dark|classic"),
        _ => {}
    }
}

fn print_config_failure(
    output_mode: OutputMode,
    reason: &str,
    language: Option<InterfaceLanguage>,
) {
    if output_mode == OutputMode::Plain {
        eprintln!("CONFIG=FAIL");
        eprintln!("REASON={reason}");
    } else {
        eprintln!(
            "{}: {reason}.",
            tabbeacon::human_presentation::catalog(
                human_runtime_presentation(language).locale,
                HumanMessageKey::ConfigurationCouldNotBeUpdated,
            )
        );
    }
}

fn print_setup_cancelled(output_mode: OutputMode, language: Option<InterfaceLanguage>) {
    if output_mode == OutputMode::Plain {
        println!("SETUP=PASS");
        println!("SETUP_RESULT=CANCELLED");
        println!("SETTINGS_UNCHANGED=true");
        println!("CODEX_CONFIG_UNCHANGED=true");
        println!("HOOKS_UNCHANGED=true");
        println!("OWNER_ACTION=none");
    } else {
        print_human_document(
            &HumanDocument::new(HumanText::message(HumanMessageKey::SetupCancelled), None)
                .with_section(HumanSection::new(None).with_message(HumanMessage::plain(
                    HumanText::message(HumanMessageKey::NoSetupChangesMade),
                    HumanTone::Attention,
                ))),
            language,
        );
    }
}

fn print_setup_applied(
    store: &PresentationSettingsStore,
    settings: PresentationSettings,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) {
    if output_mode == OutputMode::Plain {
        println!("SETUP=PASS");
        println!("SETUP_RESULT=APPLIED");
    } else {
        print_human_document(
            &HumanDocument::new(
                HumanText::message(HumanMessageKey::SetupChangesApplied),
                None,
            ),
            language,
        );
    }
    print_settings(store, settings, output_mode, language);
}

fn print_setup_preview_blocked(output_mode: OutputMode, language: Option<InterfaceLanguage>) {
    if output_mode == OutputMode::Plain {
        eprintln!("SETUP=BLOCKED");
        eprintln!("REASON=preview must succeed before setup can apply changes");
        eprintln!("SETTINGS_UNCHANGED=true");
        eprintln!("CODEX_CONFIG_UNCHANGED=true");
        eprintln!("HOOKS_UNCHANGED=true");
    } else {
        eprint_human_text(
            HumanTone::Failure,
            &HumanText::message(HumanMessageKey::SetupPreviewBlocked),
            language,
        );
        eprint_human_text(
            HumanTone::Dim,
            &HumanText::message(HumanMessageKey::NoSetupChangesMade),
            language,
        );
    }
}

fn print_setup_settings_conflict(
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    if output_mode == OutputMode::Plain {
        eprintln!("SETUP=BLOCKED");
        eprintln!("REASON=settings changed while guided setup was open");
        eprintln!("SETTINGS_UNCHANGED=true");
        eprintln!("CODEX_CONFIG_UNCHANGED=true");
        eprintln!("HOOKS_UNCHANGED=true");
    } else {
        eprint_human_text(
            HumanTone::Attention,
            &HumanText::message(HumanMessageKey::SetupSettingsChanged),
            language,
        );
        eprint_human_text(
            HumanTone::Dim,
            &HumanText::message(HumanMessageKey::ReviewSettingsAndRunSetupAgain),
            language,
        );
    }
    ExitCode::from(75)
}

fn print_setup_apply_failure(
    reason: &str,
    settings_restored: bool,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    if output_mode == OutputMode::Plain {
        eprintln!("SETUP=FAIL");
        eprintln!("REASON={reason}");
        eprintln!("SETTINGS_RESTORED={settings_restored}");
        eprintln!("CODEX_CONFIG_UNCHANGED=UNPROVEN");
        eprintln!("HOOKS_UNCHANGED=UNPROVEN");
    } else {
        eprint_human_text(
            HumanTone::Failure,
            &HumanText::template(HumanMessageKey::SetupCouldNotApply, [reason]),
            language,
        );
        if settings_restored {
            eprint_human_text(
                HumanTone::Plain,
                &HumanText::message(HumanMessageKey::PresentationSettingsRestored),
                language,
            );
        } else {
            eprint_human_text(
                HumanTone::Attention,
                &HumanText::message(HumanMessageKey::PresentationSettingsRestoreUnproven),
                language,
            );
        }
        eprint_human_text(
            HumanTone::Dim,
            &HumanText::message(HumanMessageKey::RunDoctorBeforeSetup),
            language,
        );
    }
    ExitCode::FAILURE
}

fn print_setup_snapshot_failure(
    error: &dyn std::error::Error,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    if output_mode == OutputMode::Plain {
        eprintln!("SETUP=FAIL");
        eprintln!("REASON={error}");
        eprintln!("SETTINGS_UNCHANGED=true");
    } else {
        eprint_human_text(
            HumanTone::Failure,
            &HumanText::template(HumanMessageKey::SetupCouldNotReadState, [error.to_string()]),
            language,
        );
        eprint_human_text(
            HumanTone::Dim,
            &HumanText::message(HumanMessageKey::NoSetupChangesMade),
            language,
        );
    }
    ExitCode::FAILURE
}

fn setup_management_error(
    error: &dyn std::error::Error,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    management_error_for_output("SETUP", error, output_mode, language)
}

#[derive(Clone, Copy)]
struct HumanRuntimePresentation {
    locale: ResolvedLocale,
    color: HumanColor,
}

fn human_runtime_presentation(language: Option<InterfaceLanguage>) -> HumanRuntimePresentation {
    let preferences = active_interface_preferences();
    HumanRuntimePresentation {
        locale: resolve_runtime_locale(language, preferences.language()).locale(),
        color: preferences.color(),
    }
}

fn active_interface_preferences() -> InterfacePreferences {
    InterfacePreferencesStore::from_environment()
        .map(|store| store.load_or_default())
        .unwrap_or_default()
}

fn print_human_document(document: &HumanDocument, language: Option<InterfaceLanguage>) {
    let presentation = human_runtime_presentation(language);
    print_human_lines(
        HumanRenderer::new(presentation.locale, terminal_width()).render(document),
        presentation.color,
    );
}

fn print_human_tone(tone: HumanTone, line: impl AsRef<str>) {
    print_human_tone_with_color(
        tone,
        line,
        active_interface_preferences().color(),
        io::stdout().is_terminal(),
        false,
    );
}

fn print_human_text(tone: HumanTone, text: &HumanText, language: Option<InterfaceLanguage>) {
    let presentation = human_runtime_presentation(language);
    print_human_tone_with_color(
        tone,
        render_human_text(presentation.locale, text),
        presentation.color,
        io::stdout().is_terminal(),
        false,
    );
}

fn eprint_human_text(tone: HumanTone, text: &HumanText, language: Option<InterfaceLanguage>) {
    let presentation = human_runtime_presentation(language);
    print_human_tone_with_color(
        tone,
        render_human_text(presentation.locale, text),
        presentation.color,
        io::stderr().is_terminal(),
        true,
    );
}

fn eprint_human_tone(tone: HumanTone, line: impl AsRef<str>) {
    print_human_tone_with_color(
        tone,
        line,
        active_interface_preferences().color(),
        io::stderr().is_terminal(),
        true,
    );
}

fn print_human_tone_with_color(
    tone: HumanTone,
    line: impl AsRef<str>,
    color: HumanColor,
    is_terminal: bool,
    stderr: bool,
) {
    let styled = style(
        tone,
        line.as_ref(),
        color_enabled(color, is_terminal, std::env::var_os("NO_COLOR").is_some()),
    );
    if stderr {
        eprintln!("{styled}");
    } else {
        println!("{styled}");
    }
}

fn print_human_lines(lines: impl IntoIterator<Item = HumanLine>, color: HumanColor) {
    for line in lines {
        print_human_tone_with_color(
            line.tone(),
            line.text(),
            color,
            io::stdout().is_terminal(),
            false,
        );
    }
}

fn prompt_choice<T: Copy>(
    name: &str,
    current: &str,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<T, String> {
    print!("{name} [{current}]: ");
    io::stdout()
        .flush()
        .map_err(|_| "cannot write wizard prompt".to_owned())?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|_| "cannot read wizard response".to_owned())?;
    let value = line.trim();
    parse(if value.is_empty() { current } else { value })
        .ok_or_else(|| format!("unsupported {name} choice"))
}

fn wizard_error(
    error: &str,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    if output_mode == OutputMode::Plain {
        eprintln!("CONFIG=FAIL");
        eprintln!("REASON={error}");
    } else {
        eprint_human_text(
            HumanTone::Failure,
            &HumanText::template(HumanMessageKey::ConfigurationInputFailed, [error]),
            language,
        );
    }
    ExitCode::from(2)
}

fn export_settings(
    destination: Option<&std::path::Path>,
    force: bool,
    output: HumanOutputArgs,
) -> ExitCode {
    let presentation_store = match settings_store() {
        Ok(store) => store,
        Err(error) => return transfer_failure("EXPORT", &error, output),
    };
    let interface_store = match interface_store() {
        Ok(store) => store,
        Err(error) => return transfer_failure("EXPORT", &error, output),
    };
    let workspace_store = match WorkspacePreferenceStore::from_environment() {
        Ok(store) => store,
        Err(error) => return transfer_failure("EXPORT", &error, output),
    };
    let presentation = match presentation_store.snapshot_read_only() {
        Ok(snapshot) => (!snapshot.is_absent()).then_some(snapshot.settings()),
        Err(error) => return transfer_failure("EXPORT", &error, output),
    };
    let interface = match interface_store.snapshot_read_only() {
        Ok(snapshot) => (!snapshot.is_absent()).then_some(snapshot.preferences()),
        Err(error) => return transfer_failure("EXPORT", &error, output),
    };
    let workspace = match workspace_store.snapshot_read_only() {
        Ok(snapshot) => snapshot.preferences().clone(),
        Err(error) => return transfer_failure("EXPORT", &error, output),
    };
    let document = SettingsExportV1::new(presentation, interface, &workspace);
    let bytes = match document.to_canonical_json() {
        Ok(bytes) => bytes,
        Err(error) => return transfer_failure("EXPORT", &error, output),
    };

    let Some(destination) = destination else {
        return match io::stdout()
            .write_all(&bytes)
            .and_then(|()| io::stdout().flush())
        {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::FAILURE,
        };
    };

    match write_export_file(destination, &bytes, force) {
        Ok(()) => {
            print_export_summary(&document, output);
            ExitCode::SUCCESS
        }
        Err(error) => transfer_failure("EXPORT", &error, output),
    }
}

#[allow(clippy::too_many_lines)]
fn import_settings(path: &std::path::Path, apply: bool, output: HumanOutputArgs) -> ExitCode {
    let Ok(file) = fs::File::open(path) else {
        return transfer_failure("IMPORT", &io::Error::other("input is unreadable"), output);
    };
    let mut bytes = Vec::new();
    let mut bounded = file.take(u64::try_from(MAX_EXPORT_BYTES + 1).unwrap_or(u64::MAX));
    if bounded.read_to_end(&mut bytes).is_err() {
        return transfer_failure("IMPORT", &io::Error::other("input is unreadable"), output);
    }
    let document = match SettingsExportV1::parse(&bytes) {
        Ok(document) => document,
        Err(error) => return transfer_failure("IMPORT", &error, output),
    };
    let presentation_store = match settings_store() {
        Ok(store) => store,
        Err(error) => return transfer_failure("IMPORT", &error, output),
    };
    let interface_store = match interface_store() {
        Ok(store) => store,
        Err(error) => return transfer_failure("IMPORT", &error, output),
    };
    let workspace_store = match WorkspacePreferenceStore::from_environment() {
        Ok(store) => store,
        Err(error) => return transfer_failure("IMPORT", &error, output),
    };
    let presentation_snapshot = match presentation_store.snapshot_read_only() {
        Ok(snapshot) => snapshot,
        Err(error) => return transfer_failure("IMPORT", &error, output),
    };
    let interface_snapshot = match interface_store.snapshot_read_only() {
        Ok(snapshot) => snapshot,
        Err(error) => return transfer_failure("IMPORT", &error, output),
    };
    let workspace_snapshot = match workspace_store.snapshot_read_only() {
        Ok(snapshot) => snapshot,
        Err(error) => return transfer_failure("IMPORT", &error, output),
    };
    let mut known_identities = workspace_snapshot.preferences().identities();
    let registry = match StableAliasRegistry::default_state_root() {
        Ok(root) => StableAliasRegistry::new(root),
        Err(error) => return transfer_failure("IMPORT", &error, output),
    };
    let mut generated_aliases = BTreeMap::new();
    match registry.assignments_read_only() {
        Ok(assignments) => {
            for (identity, assignment) in assignments {
                known_identities.insert(identity.clone());
                generated_aliases.insert(identity, assignment.generated_alias().clone());
            }
        }
        Err(error) => return transfer_failure("IMPORT", &error, output),
    }
    let plan = match document.import_plan(
        &presentation_snapshot,
        &interface_snapshot,
        &workspace_snapshot,
        &known_identities,
        &generated_aliases,
    ) {
        Ok(plan) => plan,
        Err(error) => return transfer_failure("IMPORT", &error, output),
    };

    print_import_summary(&plan, &document, None, output);
    if !plan.is_applicable() {
        return ExitCode::from(2);
    }

    let apply = if apply {
        true
    } else if is_interactive_terminal() {
        let prompt = tabbeacon::human_presentation::catalog(
            human_runtime_presentation(output.language.preference()).locale,
            HumanMessageKey::ImportConfirmApply,
        );
        match Confirm::new().with_prompt(prompt).default(false).interact() {
            Ok(true) => true,
            Ok(false) => {
                print_import_summary(&plan, &document, Some("cancelled"), output);
                return ExitCode::SUCCESS;
            }
            Err(_) => return ExitCode::from(2),
        }
    } else {
        return ExitCode::SUCCESS;
    };

    if !apply {
        return ExitCode::SUCCESS;
    }
    let outcome = apply_import_plan(
        &plan,
        &presentation_store,
        &presentation_snapshot,
        &interface_store,
        &interface_snapshot,
        &workspace_store,
        &workspace_snapshot,
    );
    print_import_summary(&plan, &document, Some(import_outcome_name(outcome)), output);
    match outcome {
        ImportApplyOutcome::Applied => ExitCode::SUCCESS,
        ImportApplyOutcome::Conflict
        | ImportApplyOutcome::RolledBack
        | ImportApplyOutcome::PartialState => ExitCode::from(2),
    }
}

fn import_outcome_name(outcome: ImportApplyOutcome) -> &'static str {
    match outcome {
        ImportApplyOutcome::Applied => "applied",
        ImportApplyOutcome::RolledBack => "rolled_back",
        ImportApplyOutcome::PartialState => "partial_state",
        ImportApplyOutcome::Conflict => "conflict",
    }
}

fn print_export_summary(document: &SettingsExportV1, output: HumanOutputArgs) {
    if output.mode() == OutputMode::Plain {
        println!("EXPORT=PASS");
        println!("EXPORT_SCHEMA=tabbeacon-export-v1");
        println!("PRESENTATION_EXPORTED={}", document.has_presentation());
        println!("INTERFACE_EXPORTED={}", document.has_interface());
        println!(
            "PORTABLE_WORKSPACE_ALIASES={}",
            document.portable_workspace_alias_count()
        );
        println!(
            "DEVICE_LOCAL_WORKSPACE_ALIASES_OMITTED={}",
            document.omitted_device_local_workspace_aliases()
        );
        return;
    }
    let document = HumanDocument::new(
        HumanText::message(HumanMessageKey::Export),
        Some(HumanText::message(HumanMessageKey::ExportWritten)),
    )
    .with_section(
        HumanSection::new(Some(HumanText::message(HumanMessageKey::Configuration)))
            .with_field(HumanField::new(
                None::<String>,
                HumanText::message(HumanMessageKey::Presentation),
                human_boolean_text(document.has_presentation()),
                HumanTone::Plain,
            ))
            .with_field(HumanField::new(
                None::<String>,
                HumanText::message(HumanMessageKey::Interface),
                human_boolean_text(document.has_interface()),
                HumanTone::Plain,
            ))
            .with_message(HumanMessage::plain(
                HumanText::template(
                    HumanMessageKey::DeviceLocalAliasesOmitted,
                    [document
                        .omitted_device_local_workspace_aliases()
                        .to_string()],
                ),
                HumanTone::Dim,
            )),
    );
    print_human_document(&document, output.language.preference());
}

fn print_import_summary(
    plan: &ImportPlan,
    document: &SettingsExportV1,
    outcome: Option<&str>,
    output: HumanOutputArgs,
) {
    if output.mode() == OutputMode::Plain {
        println!("IMPORT={}", outcome.unwrap_or("PREVIEW"));
        println!("IMPORT_SCHEMA=tabbeacon-export-v1");
        println!("PRESENTATION_CHANGES={}", plan.changes_presentation());
        println!("INTERFACE_CHANGES={}", plan.changes_interface());
        println!(
            "WORKSPACE_PREFERENCE_CHANGES={}",
            plan.changes_workspace_preferences()
        );
        println!("PORTABLE_WORKSPACE_MATCHES={}", plan.portable_matches());
        println!("PORTABLE_WORKSPACE_UNMATCHED={}", plan.unmatched_entries());
        println!(
            "DEVICE_LOCAL_WORKSPACE_ALIASES_OMITTED={}",
            document.omitted_device_local_workspace_aliases()
        );
        println!("ALIAS_IMPORT_CONFLICTS={}", plan.conflicts().len());
        println!("NON_TTY_MUTATION_REQUIRES_APPLY=true");
        return;
    }

    let status = match outcome {
        Some("applied") => HumanMessageKey::ImportApplied,
        Some("conflict") | None if !plan.is_applicable() => HumanMessageKey::ImportConflict,
        Some("rolled_back") => HumanMessageKey::ImportRolledBack,
        Some("partial_state") => HumanMessageKey::ImportPartialState,
        Some("cancelled") => HumanMessageKey::ImportCancelled,
        _ => HumanMessageKey::ImportPreview,
    };
    let tone = match outcome {
        Some("applied") => HumanTone::Success,
        Some("partial_state" | "conflict") => HumanTone::Failure,
        Some("rolled_back") => HumanTone::Attention,
        _ if !plan.is_applicable() => HumanTone::Failure,
        _ => HumanTone::Plain,
    };
    let section = HumanSection::new(Some(HumanText::message(HumanMessageKey::PlannedChanges)))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Presentation),
            human_boolean_text(plan.changes_presentation()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Interface),
            human_boolean_text(plan.changes_interface()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Workspace),
            human_boolean_text(plan.changes_workspace_preferences()),
            HumanTone::Plain,
        ))
        .with_message(HumanMessage::plain(
            HumanText::template(
                HumanMessageKey::PortableAliasesMatched,
                [plan.portable_matches().to_string()],
            ),
            HumanTone::Dim,
        ))
        .with_message(HumanMessage::plain(
            HumanText::template(
                HumanMessageKey::PortableAliasesUnmatched,
                [plan.unmatched_entries().to_string()],
            ),
            HumanTone::Dim,
        ))
        .with_message(HumanMessage::plain(
            HumanText::template(
                HumanMessageKey::DeviceLocalAliasesOmitted,
                [document
                    .omitted_device_local_workspace_aliases()
                    .to_string()],
            ),
            HumanTone::Dim,
        ));
    let section = if outcome.is_none() && plan.is_applicable() {
        section.with_message(HumanMessage::plain(
            HumanText::message(HumanMessageKey::ImportApplyRequired),
            tone,
        ))
    } else {
        section
    };
    let document = HumanDocument::new(
        HumanText::message(HumanMessageKey::Import),
        Some(HumanText::message(status)),
    )
    .with_section(section);
    print_human_document(&document, output.language.preference());
}

fn transfer_failure(
    operation: &str,
    error: &dyn std::error::Error,
    output: HumanOutputArgs,
) -> ExitCode {
    if output.mode() == OutputMode::Plain {
        eprintln!("{operation}=FAIL");
        eprintln!("REASON={error}");
    } else {
        let key = if operation == "EXPORT" {
            HumanMessageKey::Export
        } else {
            HumanMessageKey::Import
        };
        eprint_human_text(
            HumanTone::Failure,
            &HumanText::template(
                HumanMessageKey::OperationCouldNotComplete,
                [
                    render_human_text(
                        human_runtime_presentation(output.language.preference()).locale,
                        &HumanText::message(key),
                    ),
                    error.to_string(),
                ],
            ),
            output.language.preference(),
        );
    }
    ExitCode::FAILURE
}

fn preview(arguments: PreviewArgs) -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error("PREVIEW", &error),
    };
    let mut settings = store.load_or_default();
    if let Some(theme) = arguments.theme {
        settings = match PresentationTheme::parse(&theme) {
            Some(theme) => settings.with_theme(theme),
            None => return preview_choice_error("theme"),
        };
    }
    if let Some(spinner) = arguments.spinner {
        settings = match SpinnerPreset::parse(&spinner) {
            Some(spinner) => settings.with_spinner(spinner),
            None => return preview_choice_error("spinner"),
        };
    }
    print_preview_result(settings, OutputMode::Plain, None)
}

fn print_preview_result(
    settings: PresentationSettings,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    match render_preview(settings) {
        Ok(()) => {
            if output_mode == OutputMode::Plain {
                println!("PREVIEW=PASS");
                println!("TITLE_SPINNER_FEASIBILITY=PRODUCTION");
            } else {
                print_human_document(
                    &HumanDocument::new(
                        HumanText::message(HumanMessageKey::PreviewResult),
                        Some(HumanText::message(HumanMessageKey::Healthy)),
                    )
                    .with_section(HumanSection::new(None).with_message(
                        HumanMessage::plain(
                            HumanText::message(HumanMessageKey::NoChangesNeeded),
                            HumanTone::Dim,
                        ),
                    )),
                    language,
                );
            }
            ExitCode::SUCCESS
        }
        Err(reason) => {
            if output_mode == OutputMode::Plain {
                eprintln!("PREVIEW=BLOCKED");
                eprintln!("REASON={reason}");
            } else {
                eprint_human_text(
                    HumanTone::Failure,
                    &HumanText::template(HumanMessageKey::PreviewCouldNotComplete, [reason]),
                    language,
                );
            }
            ExitCode::from(78)
        }
    }
}

fn render_preview(settings: PresentationSettings) -> Result<(), &'static str> {
    #[cfg(windows)]
    let mut console = tabbeacon::console_output::open_owned_console()
        .map_err(|_| "owned terminal output is unavailable")?;
    #[cfg(not(windows))]
    let mut console = io::stdout();
    let renderer = WindowsTerminalRenderer::with_settings(
        WindowsTerminalCapabilities::new(std::env::var_os("WT_SESSION").is_some()),
        settings,
    );
    for (name, phase, attention) in [
        ("ready", Phase::Ready, Attention::None),
        ("working", Phase::Working, Attention::None),
        ("result-ready", Phase::WaitingUser, Attention::ResultReady),
        ("approval", Phase::WaitingUser, Attention::Approval),
        ("reset", Phase::Ended, Attention::None),
    ] {
        let title = format!("TabBeacon preview {name}");
        let action = PresentationPolicy::resolve(SemanticPresentationInput::new(
            phase,
            attention,
            Health::Normal,
            &title,
        ));
        if console
            .write_all(&renderer.render(&action))
            .and_then(|()| console.flush())
            .is_err()
        {
            return Err("terminal output stopped during preview");
        }
        thread::sleep(Duration::from_millis(450));
    }
    Ok(())
}

fn preview_choice_error(key: &str) -> ExitCode {
    eprintln!("PREVIEW=FAIL");
    eprintln!("REASON=unsupported {key} override");
    print_config_choices(key, OutputMode::Plain, None);
    ExitCode::from(2)
}

fn interface_show(output_mode: OutputMode, language: Option<InterfaceLanguage>) -> ExitCode {
    let store = match interface_store() {
        Ok(store) => store,
        Err(error) => {
            return management_error_for_output("INTERFACE", &error, output_mode, language);
        }
    };
    let preferences = match store.load_read_only() {
        Ok(preferences) => preferences,
        Err(error) => {
            return management_error_for_output("INTERFACE", &error, output_mode, language);
        }
    };
    if output_mode == OutputMode::Plain {
        print_interface_plain(store.path(), preferences, None);
    } else {
        print_human_document(&interface_preferences_document(preferences, None), language);
    }
    ExitCode::SUCCESS
}

fn interface_set(
    key: InterfacePreferenceKey,
    value: &str,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    let store = match interface_store() {
        Ok(store) => store,
        Err(error) => {
            return management_error_for_output("INTERFACE", &error, output_mode, language);
        }
    };
    let snapshot = match store.snapshot_read_only() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return management_error_for_output("INTERFACE", &error, output_mode, language);
        }
    };
    let preferences = snapshot.preferences();
    let replacement = match key {
        InterfacePreferenceKey::Language => {
            InterfaceLanguage::parse(value).map(|language| preferences.with_language(language))
        }
        InterfacePreferenceKey::Color => {
            HumanColor::parse(value).map(|color| preferences.with_color(color))
        }
        InterfacePreferenceKey::ReducedMotion => value
            .parse::<bool>()
            .ok()
            .map(|reduced_motion| preferences.with_reduced_motion(reduced_motion)),
    };
    let Some(replacement) = replacement else {
        return interface_value_error(key, value, output_mode, language);
    };
    match store.save_snapshot_if_unchanged(&snapshot, replacement) {
        Ok(InterfacePreferencesSnapshotSaveOutcome::Saved(_)) => {
            if output_mode == OutputMode::Plain {
                print_interface_plain(store.path(), replacement, Some("PASS"));
            } else {
                print_human_document(
                    &interface_preferences_document(
                        replacement,
                        Some(HumanMessageKey::InterfacePreferencesUpdated),
                    ),
                    language,
                );
            }
            ExitCode::SUCCESS
        }
        Ok(InterfacePreferencesSnapshotSaveOutcome::Conflict) => {
            if output_mode == OutputMode::Plain {
                eprintln!("INTERFACE=CONFLICT");
                eprintln!("INTERFACE_UNCHANGED=true");
            } else {
                eprint_human_tone(
                    HumanTone::Attention,
                    "Interface preferences changed while this request was open.",
                );
            }
            ExitCode::from(2)
        }
        Err(error) => management_error_for_output("INTERFACE", &error, output_mode, language),
    }
}

fn interface_preferences_document(
    preferences: InterfacePreferences,
    result: Option<HumanMessageKey>,
) -> HumanDocument {
    let section = HumanSection::new(None)
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Language),
            human_interface_language_text(preferences.language()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::Color),
            human_color_text(preferences.color()),
            HumanTone::Plain,
        ))
        .with_field(HumanField::new(
            None::<String>,
            HumanText::message(HumanMessageKey::ReducedMotion),
            human_boolean_text(preferences.reduced_motion()),
            HumanTone::Plain,
        ));
    let document = HumanDocument::new(
        HumanText::message(HumanMessageKey::InterfacePreferences),
        None,
    )
    .with_section(section);
    result.map_or(document.clone(), |message| {
        document.with_section(HumanSection::new(None).with_message(HumanMessage::plain(
            HumanText::message(message),
            HumanTone::Success,
        )))
    })
}

fn print_interface_plain(
    path: &std::path::Path,
    preferences: InterfacePreferences,
    result: Option<&str>,
) {
    if let Some(result) = result {
        println!("INTERFACE={result}");
    }
    println!("INTERFACE_PATH={}", path.display());
    println!("INTERFACE_LANGUAGE={}", preferences.language());
    println!("INTERFACE_COLOR={}", preferences.color());
    println!("INTERFACE_REDUCED_MOTION={}", preferences.reduced_motion());
}

fn interface_value_error(
    key: InterfacePreferenceKey,
    value: &str,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    if output_mode == OutputMode::Plain {
        eprintln!("INTERFACE=FAIL");
        eprintln!("REASON=unsupported {key:?} value: {value}");
    } else {
        eprint_human_text(
            HumanTone::Attention,
            &HumanText::template(
                HumanMessageKey::UnsupportedInterfacePreferenceValue,
                [value],
            ),
            language,
        );
    }
    ExitCode::from(2)
}

fn settings_store() -> Result<PresentationSettingsStore, tabbeacon::settings::SettingsError> {
    PresentationSettingsStore::from_environment()
}

fn interface_store()
-> Result<InterfacePreferencesStore, tabbeacon::interface_preferences::InterfacePreferencesError> {
    InterfacePreferencesStore::from_environment()
}

fn management_error(operation: &str, error: &dyn std::error::Error) -> ExitCode {
    eprintln!("{operation}=FAIL");
    eprintln!("REASON={error}");
    ExitCode::FAILURE
}

fn management_error_for_output(
    operation: &str,
    error: &dyn std::error::Error,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    if output_mode == OutputMode::Plain {
        return management_error(operation, error);
    }
    let presentation = human_runtime_presentation(language);
    let label = match operation {
        "SETUP" => tabbeacon::human_presentation::catalog(
            presentation.locale,
            HumanMessageKey::SetupOperation,
        ),
        "CONFIG" => tabbeacon::human_presentation::catalog(
            presentation.locale,
            HumanMessageKey::Configuration,
        ),
        "INTERFACE" => tabbeacon::human_presentation::catalog(
            presentation.locale,
            HumanMessageKey::InterfacePreferences,
        ),
        "UNINSTALL" => {
            tabbeacon::human_presentation::catalog(presentation.locale, HumanMessageKey::Uninstall)
        }
        _ => operation,
    };
    print_human_tone_with_color(
        HumanTone::Failure,
        render_human_text(
            presentation.locale,
            &HumanText::template(
                HumanMessageKey::OperationCouldNotComplete,
                [label.to_owned(), error.to_string()],
            ),
        ),
        presentation.color,
        io::stderr().is_terminal(),
        true,
    );
    ExitCode::FAILURE
}

/// Returns whether this process can safely offer an inline interactive flow.
///
/// G40 intentionally does not enter raw or alternate-screen terminal modes;
/// later UI goals must keep this check as their admission boundary.
fn is_interactive_terminal() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

fn interactive_terminal_required(
    operation: &str,
    reason: &str,
    next_action: &str,
    output_mode: OutputMode,
    language: Option<InterfaceLanguage>,
) -> ExitCode {
    if output_mode == OutputMode::Plain {
        eprintln!("{operation}=BLOCKED");
        eprintln!("REASON={reason}");
        eprintln!("SETTINGS_UNCHANGED=true");
        eprintln!("CODEX_CONFIG_UNCHANGED=true");
        eprintln!("HOOKS_UNCHANGED=true");
        eprintln!("NEXT_ACTION={next_action}");
    } else {
        let locale = human_runtime_presentation(language).locale;
        let label = match operation {
            "SETUP" => {
                tabbeacon::human_presentation::catalog(locale, HumanMessageKey::SetupOperation)
            }
            "CONFIG" => {
                tabbeacon::human_presentation::catalog(locale, HumanMessageKey::Configuration)
            }
            _ => operation,
        };
        eprint_human_text(
            HumanTone::Attention,
            &HumanText::template(HumanMessageKey::InteractiveTerminalRequired, [label]),
            language,
        );
        eprint_human_text(
            HumanTone::Dim,
            &HumanText::template(HumanMessageKey::NextAction, [next_action]),
            language,
        );
    }
    ExitCode::from(2)
}

fn completions(shell: clap_complete::Shell) -> ExitCode {
    let mut command = Cli::command();
    generate(shell, &mut command, "tabbeacon", &mut io::stdout());
    ExitCode::SUCCESS
}

fn ui() -> ExitCode {
    if !is_interactive_terminal() {
        println!("TABBEACON_UI=NON_INTERACTIVE");
        println!("NEXT_ACTION=use tabbeacon status --json or tabbeacon config commands");
        return ExitCode::SUCCESS;
    }
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error("UI", &error),
    };
    let interface_store = match InterfacePreferencesStore::from_environment() {
        Ok(store) => store,
        Err(error) => return management_error("UI", &error),
    };
    let refresh = match collect_control_center_refresh(&store, &interface_store, true) {
        Ok(refresh) => refresh,
        Err(error) => return management_error("UI", &error),
    };
    match tabbeacon::control_center::run(
        tabbeacon::control_center::ControlCenterApp::new(
            refresh.presentation,
            refresh.snapshot.clone(),
            refresh.overview.clone(),
        )
        .with_interface_preferences(refresh.interface)
        .with_locale(resolve_runtime_locale(None, refresh.interface.language()).locale())
        .with_refresh(refresh),
        |before, after| apply_control_center_drafts(&store, &interface_store, before, after),
        apply_control_center_workspace_override,
        || collect_control_center_refresh(&store, &interface_store, false),
        apply_control_center_repair,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => management_error("UI", &error),
    }
}

fn collect_control_center_refresh(
    settings_store: &PresentationSettingsStore,
    interface_store: &InterfacePreferencesStore,
    include_workspace: bool,
) -> io::Result<tabbeacon::control_center::ControlCenterRefresh> {
    let presentation = settings_store
        .snapshot_read_only()
        .map_err(io::Error::other)?
        .settings();
    let interface = interface_store
        .snapshot_read_only()
        .map_err(io::Error::other)?
        .preferences();
    let report = collect_operational_diagnostics();
    let workspace = include_workspace
        .then(|| {
            std::env::current_dir().ok().and_then(|cwd| {
                WorkspaceIdentityResolver::with_default_state_root()
                    .ok()
                    .and_then(|resolver| resolver.inspect_alias(cwd).ok())
            })
        })
        .flatten();
    let hooks = CodexIntegration::from_environment()
        .map(|integration| integration.hook_inventory())
        .unwrap_or_default();
    let sessions = inspect_system_sessions();
    let title_explanation = TitleExplanation::from_observation(
        &report,
        Some(presentation),
        workspace.as_ref(),
        &sessions,
    );
    Ok(tabbeacon::control_center::ControlCenterRefresh {
        presentation,
        interface,
        snapshot: ManagementSnapshot::from_diagnostics(&report),
        overview: tabbeacon::management::ManagementOverview::from_diagnostics(&report),
        workspace,
        sessions,
        hooks,
        title_explanation,
    })
}

fn apply_control_center_drafts(
    settings_store: &PresentationSettingsStore,
    interface_store: &InterfacePreferencesStore,
    before: tabbeacon::control_center::ControlCenterDraft,
    after: tabbeacon::control_center::ControlCenterDraft,
) -> io::Result<()> {
    let settings_snapshot = settings_store
        .snapshot_read_only()
        .map_err(io::Error::other)?;
    let interface_snapshot = interface_store
        .snapshot_read_only()
        .map_err(io::Error::other)?;
    if settings_snapshot.settings() != before.presentation
        || interface_snapshot.preferences() != before.interface
    {
        return Err(settings_conflict_error());
    }

    let interface_receipt = if before.interface == after.interface {
        None
    } else {
        match interface_store
            .save_snapshot_if_unchanged(&interface_snapshot, after.interface)
            .map_err(io::Error::other)?
        {
            InterfacePreferencesSnapshotSaveOutcome::Saved(receipt) => Some(receipt),
            InterfacePreferencesSnapshotSaveOutcome::Conflict => {
                return Err(settings_conflict_error());
            }
        }
    };

    // Verify the first per-store write before entering the second store. If
    // this readback cannot prove the Interface draft, no Presentation write
    // has occurred and the exact receipt can still compensate safely.
    if before.interface != after.interface {
        match interface_store.snapshot_read_only() {
            Ok(snapshot) if snapshot.preferences() == after.interface => {}
            Ok(_) => {
                let restored = interface_receipt.as_ref().is_some_and(|receipt| {
                    matches!(
                        interface_store.restore_snapshot_if_unchanged(receipt, &interface_snapshot),
                        Ok(InterfacePreferencesConditionalOutcome::Saved)
                    )
                });
                if restored {
                    return Err(interface_conflict_error());
                }
                return Err(io::Error::other(
                    "Interface preferences changed concurrently; rollback was refused",
                ));
            }
            Err(error) => {
                let restored = interface_receipt.as_ref().is_some_and(|receipt| {
                    matches!(
                        interface_store.restore_snapshot_if_unchanged(receipt, &interface_snapshot),
                        Ok(InterfacePreferencesConditionalOutcome::Saved)
                    )
                });
                let reason = if restored {
                    error.to_string()
                } else {
                    format!(
                        "{error}; Interface rollback refused because the document could not be verified"
                    )
                };
                return Err(io::Error::other(reason));
            }
        }
    }

    if before.presentation != after.presentation {
        match apply_control_center_settings_change(
            settings_store,
            &settings_snapshot,
            before.presentation,
            after.presentation,
        ) {
            Ok(_) => {}
            Err(error) => {
                if let Some(receipt) = interface_receipt.as_ref() {
                    let restored = matches!(
                        interface_store.restore_snapshot_if_unchanged(receipt, &interface_snapshot),
                        Ok(InterfacePreferencesConditionalOutcome::Saved)
                    );
                    if !restored {
                        return Err(io::Error::other(format!(
                            "{error}; Interface rollback refused because the document changed concurrently"
                        )));
                    }
                }
                return Err(error);
            }
        }
    }
    Ok(())
}

fn apply_control_center_workspace_override(
    before: Option<String>,
    after: Option<String>,
) -> io::Result<()> {
    let resolver =
        WorkspaceIdentityResolver::with_default_state_root().map_err(io::Error::other)?;
    let cwd = std::env::current_dir().map_err(io::Error::other)?;
    apply_control_center_workspace_override_with(&resolver, &cwd, before, after)
}

fn apply_control_center_repair(action_id: &str) -> io::Result<()> {
    let store = WindowsTerminalPolicyStore::from_environment();
    apply_control_center_repair_with(&store, action_id)
}

fn apply_control_center_repair_with(
    store: &WindowsTerminalPolicyStore,
    action_id: &str,
) -> io::Result<()> {
    if action_id != "terminal.title_policy_repair" {
        return Err(io::Error::other(
            "The requested repair is not admitted in Control Center",
        ));
    }
    if store.inspect().remediation != TitleRemediationState::Available {
        return Err(io::Error::other(
            "The previewed title-policy repair is no longer available; no change was made",
        ));
    }
    let result = store.repair().map_err(io::Error::other)?;
    if result.state != TitleRemediationState::Available
        || !result.document_modified
        || !result.user_config_preserved
    {
        return Err(io::Error::other(
            "The title-policy repair could not be verified; inspect diagnostics before retrying",
        ));
    }
    if store.inspect().remediation != TitleRemediationState::AlreadyOwned {
        return Err(io::Error::other(
            "The title-policy repair was written but post-apply ownership verification was not proven",
        ));
    }
    Ok(())
}

#[allow(clippy::needless_pass_by_value)] // The Control Center apply callback transfers its owned baseline snapshot.
fn apply_control_center_workspace_override_with(
    resolver: &WorkspaceIdentityResolver,
    cwd: &std::path::Path,
    before: Option<String>,
    after: Option<String>,
) -> io::Result<()> {
    let inspection = resolver.inspect_alias(cwd).map_err(io::Error::other)?;
    let observed = inspection
        .custom_alias()
        .map(|alias| alias.as_str().to_owned());
    if observed != before {
        return Err(settings_conflict_error());
    }
    match after {
        Some(alias) => resolver
            .set_alias_override(cwd, alias)
            .map_err(io::Error::other)?,
        None => resolver
            .reset_alias_override(cwd)
            .map_err(io::Error::other)?,
    };
    Ok(())
}

fn interface_conflict_error() -> io::Error {
    io::Error::other("Interface preferences changed concurrently; the stale draft was not applied")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[test]
    fn guided_setup_revisit_policy_distinguishes_fresh_quick_and_full_paths() {
        assert!(should_revisit_interface_preferences(
            false, false, true, false
        ));
        assert!(should_revisit_interface_preferences(
            false, false, false, true
        ));
        assert!(should_revisit_interface_preferences(
            false, true, false, false
        ));
        assert!(
            should_revisit_interface_preferences(true, false, true, false),
            "a fresh presentation setup remains language-first even in quick mode"
        );
        assert!(
            !should_revisit_interface_preferences(true, false, false, true),
            "quick setup keeps a valid implicit auto default prompt-free"
        );
        assert!(
            !should_revisit_interface_preferences(true, false, false, false),
            "returning quick setup has no language prompt"
        );
    }

    #[test]
    fn control_center_apply_uses_the_existing_typed_settings_store() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("tabbeacon-ui-apply-{unique}"));
        let store = PresentationSettingsStore::new(root.join("config.toml"));
        let before = PresentationSettings::default();
        let after = before.with_theme(PresentationTheme::Classic);

        apply_settings_change(&store, before, after).unwrap();

        assert_eq!(store.load().unwrap(), after);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn control_center_interface_only_apply_uses_its_separate_snapshot_store() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("tabbeacon-ui-interface-{unique}"));
        let settings_store = PresentationSettingsStore::new(root.join("config.toml"));
        let interface_store = InterfacePreferencesStore::new(root.join("interface.toml"));
        let settings_snapshot = settings_store.snapshot_read_only().unwrap();
        let interface_snapshot = interface_store.snapshot_read_only().unwrap();
        let before = tabbeacon::control_center::ControlCenterDraft {
            presentation: settings_snapshot.settings(),
            interface: interface_snapshot.preferences(),
        };
        let after = tabbeacon::control_center::ControlCenterDraft {
            presentation: before.presentation,
            interface: before.interface.with_language(InterfaceLanguage::ZhCn),
        };

        apply_control_center_drafts(&settings_store, &interface_store, before, after).unwrap();

        assert!(!settings_store.path().exists());
        assert_eq!(
            interface_store.load_read_only().unwrap().language(),
            InterfaceLanguage::ZhCn
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn control_center_combined_failure_restores_interface_before_preserving_settings_conflict() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("tabbeacon-ui-combined-{unique}"));
        let settings_store = PresentationSettingsStore::new(root.join("config.toml"));
        let interface_store = InterfacePreferencesStore::new(root.join("interface.toml"));
        let settings_snapshot = settings_store.snapshot_read_only().unwrap();
        let interface_snapshot = interface_store.snapshot_read_only().unwrap();
        let before = tabbeacon::control_center::ControlCenterDraft {
            presentation: settings_snapshot.settings(),
            interface: interface_snapshot.preferences(),
        };
        let concurrent = before.presentation.with_theme(PresentationTheme::Classic);
        settings_store.save(concurrent).unwrap();
        let after = tabbeacon::control_center::ControlCenterDraft {
            presentation: before.presentation.with_title(TitleMode::Native),
            interface: before.interface.with_language(InterfaceLanguage::ZhCn),
        };

        assert!(
            apply_control_center_drafts(&settings_store, &interface_store, before, after,).is_err()
        );
        assert_eq!(settings_store.load().unwrap(), concurrent);
        assert!(
            !interface_store.path().exists(),
            "the guarded Interface write is compensated when presentation conflicts"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn control_center_workspace_apply_refuses_a_collision_without_writing_the_target_override() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("tabbeacon-ui-workspace-{unique}"));
        let first = root.join("first");
        let second = root.join("second");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();
        let resolver = WorkspaceIdentityResolver::new(root.join("registry"));
        resolver.set_alias_override(&second, "TAKEN").unwrap();

        assert!(
            apply_control_center_workspace_override_with(
                &resolver,
                &first,
                None,
                Some("TAKEN".to_owned()),
            )
            .is_err()
        );
        assert!(
            resolver
                .inspect_alias(&first)
                .unwrap()
                .custom_alias()
                .is_none(),
            "a collided staged alias must not write a target preference"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn control_center_title_repair_requires_the_typed_preview_action_and_preserves_unrelated_state()
    {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("tabbeacon-ui-title-repair-{unique}"));
        let path = root.join("settings.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            &path,
            r#"{
  "profiles": { "list": [{ "guid": "{11111111-1111-1111-1111-111111111111}", "suppressApplicationTitle": true }] },
  "unknown": { "preserved": true }
}"#,
        )
        .unwrap();
        let store = WindowsTerminalPolicyStore::new_for_testing(
            vec![tabbeacon::windows_terminal_policy::SettingsCandidate::new(
                tabbeacon::windows_terminal_policy::WindowsTerminalInstallation::Stable,
                &path,
            )],
            root.join("state"),
            true,
            Some("{11111111-1111-1111-1111-111111111111}".to_owned()),
        );

        assert!(apply_control_center_repair_with(&store, "hooks.review_in_codex").is_err());
        assert_eq!(
            store.inspect().remediation,
            TitleRemediationState::Available
        );
        apply_control_center_repair_with(&store, "terminal.title_policy_repair").unwrap();
        assert_eq!(
            store.inspect().remediation,
            TitleRemediationState::AlreadyOwned
        );
        let updated = fs::read_to_string(&path).unwrap();
        assert!(updated.contains("\"suppressApplicationTitle\": false"));
        assert!(updated.contains("\"unknown\": { \"preserved\": true }"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn control_center_refresh_reads_injected_stores_without_creating_state() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("tabbeacon-ui-refresh-{unique}"));
        let settings_store = PresentationSettingsStore::new(root.join("config.toml"));
        let interface_store = InterfacePreferencesStore::new(root.join("interface.toml"));

        let refresh = collect_control_center_refresh(&settings_store, &interface_store, false)
            .expect("read-only refresh succeeds for absent injected state");

        assert_eq!(refresh.presentation, PresentationSettings::default());
        assert_eq!(refresh.interface, InterfacePreferences::default());
        assert!(!settings_store.path().exists());
        assert!(!interface_store.path().exists());
        assert!(
            !root.exists(),
            "refresh must not create a store parent or lock"
        );
    }

    #[test]
    fn control_center_apply_refuses_stale_state_and_preserves_concurrent_rollback_writes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("tabbeacon-ui-conflict-{unique}"));
        let store = PresentationSettingsStore::new(root.join("config.toml"));
        let before = PresentationSettings::default();
        let stale_snapshot = store.snapshot_read_only().unwrap();
        let concurrent = before.with_theme(PresentationTheme::Classic);
        store.save(concurrent).unwrap();

        let stale = apply_control_center_settings_change_with(
            &store,
            &stale_snapshot,
            before,
            before.with_title(TitleMode::Native),
            |_| panic!("a stale draft must not reach title reconciliation"),
        );
        assert!(stale.is_err());
        assert_eq!(store.load().unwrap(), concurrent);

        let current_snapshot = store.snapshot_read_only().unwrap();
        let after = concurrent.with_title(TitleMode::Native);
        let concurrent_after_write = concurrent.with_title(TitleMode::Off);
        let failed = apply_control_center_settings_change_with(
            &store,
            &current_snapshot,
            concurrent,
            after,
            |_| {
                store.save(concurrent_after_write).unwrap();
                Err("controlled title reconciliation failure".to_owned())
            },
        );
        assert!(failed.is_err());
        assert_eq!(store.load().unwrap(), concurrent_after_write);
        fs::remove_dir_all(root).unwrap();
    }
}
