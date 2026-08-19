use std::{
    io::{self, IsTerminal, Read, Write},
    process::ExitCode,
    thread,
    time::Duration,
};

use clap::{CommandFactory, Parser};
use clap_complete::generate;
use dialoguer::Select;
use tabbeacon::cli::{
    Cli, Command, ConfigCommand, ConvergenceCommand, DoctorArgs, OutputMode, PreviewArgs, Provider,
    SetupCommand, TitlePolicyCommand,
};
use tabbeacon::diagnostics::{
    collect_operational_diagnostics, collect_operational_diagnostics_with_title_probe,
    human_doctor_lines, human_status_lines,
};
use tabbeacon::guided_setup::{GuidedInput, choose_presentation};
use tabbeacon::human_diagnostics::{
    human_doctor_lines as human_doctor_v2_lines, human_status_lines as human_status_v2_lines,
    terminal_width,
};
use tabbeacon::human_output::{HumanTone, auto_color_enabled, style};
use tabbeacon::management::ManagementSnapshot;
use tabbeacon::providers::codex::{
    CodexHookRuntime, CodexIntegration, SetupOutcome, TitleOwnershipOutcome, UninstallOutcome,
};
use tabbeacon::setup::{
    SetupApplyResult, SetupDecision, SetupDiscovery, SetupPlan, detect_windows_terminal,
};
use tabbeacon::{
    activity::{run_activity_cleanup_observer_system, run_activity_worker_system},
    core::{Attention, Health, Phase},
    presentation::{
        PresentationPolicy, SemanticPresentationInput, WindowsTerminalCapabilities,
        WindowsTerminalRenderer,
    },
    settings::{
        ActivityMode, ConditionalSaveOutcome, PresentationSettings, PresentationSettingsSnapshot,
        PresentationSettingsStore, PresentationTheme, SnapshotSaveOutcome, SpinnerPreset,
        TabColorMode, TitleMode,
    },
    windows_terminal_policy::WindowsTerminalPolicyStore,
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
        }) => guided_setup(quick, full, output.mode()),
        Some(Command::Setup {
            command: Some(SetupCommand::Codex),
            output,
            ..
        }) => setup_codex(output.mode()),
        Some(Command::Doctor(DoctorArgs {
            output,
            probe_title,
        })) => doctor(output.mode(), probe_title),
        Some(Command::Status(output)) => status(output.mode()),
        Some(Command::Sessions(output)) => sessions(output.mode()),
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
        Some(Command::Config { command, output }) => match command {
            ConfigCommand::Show => config_show(output.mode()),
            ConfigCommand::Set { key, value } => config_set(&key, &value, output.mode()),
            ConfigCommand::Preset { name } => config_preset(&name, output.mode()),
            ConfigCommand::Reset => config_reset(output.mode()),
            ConfigCommand::Wizard => config_wizard(output.mode()),
        },
        Some(Command::Preview(arguments)) => preview(arguments),
        Some(Command::Completions { shell }) => completions(shell),
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

fn setup_codex(output_mode: OutputMode) -> ExitCode {
    let settings = settings_store().map_or_else(
        |_| PresentationSettings::default(),
        |store| store.load_or_default(),
    );
    let integration = match CodexIntegration::from_environment() {
        Ok(integration) => integration,
        Err(error) => return management_error("SETUP", &error),
    };
    match integration.setup_with_title_ownership(settings.title().owns_tabbeacon_title()) {
        Ok(outcome) => print_setup_outcome(outcome, output_mode),
        Err(error) => management_error("SETUP", &error),
    }
}

fn guided_setup(quick: bool, full: bool, output_mode: OutputMode) -> ExitCode {
    if !is_interactive_terminal() {
        return interactive_terminal_required(
            "SETUP",
            "guided setup requires an interactive terminal",
            "run tabbeacon setup from an interactive terminal, or use tabbeacon config and tabbeacon setup codex",
        );
    }
    let (store, snapshot, integration, discovery) = match guided_setup_context() {
        Ok(context) => context,
        Err(exit_code) => return exit_code,
    };
    let before = snapshot.settings();
    print_setup_discovery(&discovery, before);
    print_setup_title_policy(&WindowsTerminalPolicyStore::from_environment().inspect());

    if print_guided_setup_intro(quick, full, &discovery, output_mode) {
        return ExitCode::SUCCESS;
    }

    let draft = match prompt_setup_draft_v3(before) {
        Ok(Some(settings)) => settings,
        Ok(None) => {
            print_setup_cancelled(output_mode);
            return ExitCode::SUCCESS;
        }
        Err(error) => return setup_input_error(&error),
    };
    let plan = SetupPlan::new(before, discovery).with_draft(draft);
    print_setup_change_plan(&plan);
    println!("Preview");
    let preview_exit = print_preview_result(plan.preview_settings());
    if preview_exit != ExitCode::SUCCESS {
        eprintln!("SETUP=BLOCKED");
        eprintln!("REASON=preview must succeed before setup can apply changes");
        eprintln!("SETTINGS_UNCHANGED=true");
        eprintln!("CODEX_CONFIG_UNCHANGED=true");
        eprintln!("HOOKS_UNCHANGED=true");
        return preview_exit;
    }
    let decision = match prompt_setup_decision_v3() {
        Ok(decision) => decision,
        Err(error) => return setup_input_error(&error),
    };
    match decision {
        SetupDecision::Cancel => {
            let _ = plan.cancel();
            print_setup_cancelled(output_mode);
            ExitCode::SUCCESS
        }
        SetupDecision::Apply => match plan.apply(&store, &snapshot, |owns_title| {
            integration
                .setup_with_title_ownership(owns_title)
                .map_err(|error| error.to_string())
        }) {
            Ok(SetupApplyResult::Applied(outcome)) => {
                print_setup_applied(&store, plan.draft(), output_mode);
                print_setup_outcome(outcome, output_mode)
            }
            Ok(SetupApplyResult::SettingsConflict) => {
                eprintln!("SETUP=BLOCKED");
                eprintln!("REASON=settings changed while guided setup was open");
                eprintln!("SETTINGS_UNCHANGED=true");
                eprintln!("CODEX_CONFIG_UNCHANGED=true");
                eprintln!("HOOKS_UNCHANGED=true");
                ExitCode::from(75)
            }
            Ok(SetupApplyResult::SetupFailed {
                reason,
                settings_restored,
            }) => {
                eprintln!("SETUP=FAIL");
                eprintln!("REASON={reason}");
                eprintln!("SETTINGS_RESTORED={settings_restored}");
                eprintln!("CODEX_CONFIG_UNCHANGED=UNPROVEN");
                eprintln!("HOOKS_UNCHANGED=UNPROVEN");
                ExitCode::FAILURE
            }
            Ok(SetupApplyResult::Cancelled) => unreachable!("apply path cannot cancel"),
            Err(error) => management_error("SETUP", &error),
        },
    }
}

fn print_guided_setup_intro(
    quick: bool,
    full: bool,
    discovery: &SetupDiscovery,
    output_mode: OutputMode,
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
            print_human_tone(HumanTone::Success, "Setup is ready.");
            println!("No changes are needed.");
        }
        return true;
    }
    println!(
        "Welcome — setup keeps prompts, assistant output, and provider session data out of configuration."
    );
    if full {
        println!("Full setup — review the complete presentation flow.");
    } else if quick {
        println!("Quick setup — action-required sections only.");
    }
    false
}

fn guided_setup_context() -> Result<
    (
        PresentationSettingsStore,
        PresentationSettingsSnapshot,
        CodexIntegration,
        SetupDiscovery,
    ),
    ExitCode,
> {
    let store = settings_store().map_err(|error| management_error("SETUP", &error))?;
    let snapshot = store.snapshot_read_only().map_err(|error| {
        eprintln!("SETUP=FAIL");
        eprintln!("REASON={error}");
        eprintln!("SETTINGS_UNCHANGED=true");
        ExitCode::FAILURE
    })?;
    let integration =
        CodexIntegration::from_environment().map_err(|error| management_error("SETUP", &error))?;
    let discovery = guided_setup_discovery(&integration)?;
    Ok((store, snapshot, integration, discovery))
}

fn guided_setup_discovery(integration: &CodexIntegration) -> Result<SetupDiscovery, ExitCode> {
    let binary_path = std::env::current_exe().map_err(|error| management_error("SETUP", &error))?;
    Ok(SetupDiscovery::from_doctor(
        env!("CARGO_PKG_VERSION"),
        binary_path,
        detect_windows_terminal(),
        &integration.doctor(),
    ))
}

fn print_setup_outcome(outcome: SetupOutcome, output_mode: OutputMode) -> ExitCode {
    if output_mode != OutputMode::Plain {
        match outcome {
            SetupOutcome::InstalledTrustReviewRequired => {
                print_human_tone(HumanTone::Success, "Codex integration installed.");
                print_human_tone(
                    HumanTone::Dim,
                    "Next: launch codex, review TabBeacon hooks in /hooks, then run tabbeacon doctor.",
                );
            }
            SetupOutcome::Upgraded => {
                print_human_tone(HumanTone::Success, "Codex integration upgraded.");
                print_human_tone(
                    HumanTone::Dim,
                    "Next: launch codex, review the updated TabBeacon hooks in /hooks, then run tabbeacon doctor.",
                );
            }
            SetupOutcome::AlreadyInstalled => {
                print_human_tone(
                    HumanTone::Success,
                    "Codex integration is already installed.",
                );
                print_human_tone(
                    HumanTone::Dim,
                    "Next: run tabbeacon doctor to verify hook trust and configuration.",
                );
            }
        }
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

fn doctor(output_mode: OutputMode, probe_title: bool) -> ExitCode {
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
    let lines = if output_mode == OutputMode::Plain {
        human_doctor_lines(&report.doctor)
    } else {
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        human_doctor_v2_lines(&report.doctor, &snapshot, terminal_width())
    };
    if output_mode == OutputMode::Plain {
        for line in lines {
            println!("{line}");
        }
    } else {
        print_human_lines(lines);
    }
    if report.doctor.is_failure() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn status(output_mode: OutputMode) -> ExitCode {
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
    let lines = if output_mode == OutputMode::Plain {
        human_status_lines(&report)
    } else {
        let snapshot = ManagementSnapshot::from_diagnostics(&report);
        human_status_v2_lines(&report, &snapshot, terminal_width())
    };
    if output_mode == OutputMode::Plain {
        for line in lines {
            println!("{line}");
        }
    } else {
        print_human_lines(lines);
    }
    ExitCode::SUCCESS
}

fn sessions(output_mode: OutputMode) -> ExitCode {
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

    print_human_tone(HumanTone::Accent, "Sessions");
    println!(
        "  {} current, {} stale, {} invalid lease{}",
        report.active_sessions,
        report.stale_sessions,
        report.invalid_leases,
        if report.invalid_leases == 1 { "" } else { "s" }
    );
    if report.sessions.is_empty() {
        print_human_tone(HumanTone::Dim, "  No inspectable session leases.");
    } else {
        for session in &report.sessions {
            println!(
                "  {} — {} — {}s — {}",
                session.workspace_alias,
                session.semantic_state,
                session.age_seconds,
                session.worker_health.as_str().replace('_', " "),
            );
        }
    }
    print_human_tone(
        HumanTone::Dim,
        "  Lease-based observation only; no process or session control.",
    );
    ExitCode::SUCCESS
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

fn print_setup_title_policy(policy: &tabbeacon::windows_terminal_policy::TitlePolicyDiagnostics) {
    println!("Windows Terminal title policy");
    println!(
        "  Application titles {}",
        policy.application_title_policy.as_str()
    );
    println!("  Policy source       {}", policy.policy_source.as_str());
    println!(
        "  Active profile      {}",
        policy.active_profile_resolution.as_str()
    );
    println!("  Remediation         {}", policy.remediation.as_str());
    if policy.remediation.as_str() == "available" {
        println!("  Next step           tabbeacon title-policy repair (explicit)");
    }
    println!();
}

fn uninstall_codex(output_mode: OutputMode) -> ExitCode {
    let integration = match CodexIntegration::from_environment() {
        Ok(integration) => integration,
        Err(error) => return management_error("UNINSTALL", &error),
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
        Err(error) => management_error("UNINSTALL", &error),
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

fn config_show(output_mode: OutputMode) -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error("CONFIG", &error),
    };
    let settings = match store.load() {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("CONFIG=WARNING");
            eprintln!("REASON={error}");
            PresentationSettings::default()
        }
    };
    print_settings(&store, settings, output_mode);
    ExitCode::SUCCESS
}

fn config_set(key: &str, value: &str, output_mode: OutputMode) -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error("CONFIG", &error),
    };
    let current = match store.load() {
        Ok(settings) => settings,
        Err(error) => return management_error("CONFIG", &error),
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
        print_config_failure(output_mode, "unsupported config key or value");
        print_config_choices(key, output_mode);
        return ExitCode::from(2);
    };
    persist_settings_change(&store, current, updated, output_mode)
}

fn config_reset(output_mode: OutputMode) -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error("CONFIG", &error),
    };
    let current = store.load_or_default();
    let defaults = match store.reset() {
        Ok(settings) => settings,
        Err(error) => return management_error("CONFIG", &error),
    };
    if current.title() != defaults.title() {
        match CodexIntegration::from_environment()
            .and_then(|integration| integration.reconcile_title_ownership(true))
        {
            Ok(outcome) if output_mode == OutputMode::Plain => {
                println!("CODEX_TITLE_OWNERSHIP={}", title_ownership_label(outcome));
            }
            Ok(_) => print_human_tone(HumanTone::Dim, "Title ownership was reconciled."),
            Err(error) => return management_error("CONFIG", &error),
        }
    }
    if output_mode == OutputMode::Plain {
        println!("CONFIG=PASS");
    } else {
        print_human_tone(
            HumanTone::Success,
            "Presentation settings reset to defaults.",
        );
    }
    print_settings(&store, defaults, output_mode);
    ExitCode::SUCCESS
}

fn config_preset(name: &str, output_mode: OutputMode) -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error("CONFIG", &error),
    };
    let current = match store.load() {
        Ok(settings) => settings,
        Err(error) => return management_error("CONFIG", &error),
    };
    let Some(preset) = PresentationSettings::preset(name) else {
        print_config_failure(output_mode, "unsupported preset");
        if output_mode == OutputMode::Plain {
            eprintln!("PRESETS=native|minimal|balanced|full");
        } else {
            eprintln!("Supported presets: native, minimal, balanced, full.");
        }
        return ExitCode::from(2);
    };
    persist_settings_change(&store, current, preset, output_mode)
}

fn config_wizard(output_mode: OutputMode) -> ExitCode {
    if !is_interactive_terminal() {
        return interactive_terminal_required(
            "CONFIG",
            "config wizard requires an interactive terminal",
            "run tabbeacon config wizard from an interactive terminal, or use tabbeacon config set",
        );
    }
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error("CONFIG", &error),
    };
    let current = store.load_or_default();
    println!("TabBeacon presentation wizard (press Enter to keep each current value).");
    let title = match prompt_choice("title", current.title().as_str(), TitleMode::parse) {
        Ok(value) => value,
        Err(error) => return wizard_error(&error),
    };
    let tab_color = match prompt_choice(
        "tab-color",
        current.tab_color().as_str(),
        TabColorMode::parse,
    ) {
        Ok(value) => value,
        Err(error) => return wizard_error(&error),
    };
    let activity = match prompt_choice("activity", current.activity().as_str(), ActivityMode::parse)
    {
        Ok(value) => value,
        Err(error) => return wizard_error(&error),
    };
    let spinner = match prompt_choice("spinner", current.spinner().as_str(), SpinnerPreset::parse) {
        Ok(value) => value,
        Err(error) => return wizard_error(&error),
    };
    let theme = match prompt_choice("theme", current.theme().as_str(), PresentationTheme::parse) {
        Ok(value) => value,
        Err(error) => return wizard_error(&error),
    };
    persist_settings_change(
        &store,
        current,
        PresentationSettings::new(title, tab_color, activity, spinner, theme),
        output_mode,
    )
}

fn print_setup_discovery(discovery: &SetupDiscovery, settings: PresentationSettings) {
    print_human_tone(HumanTone::Accent, "TabBeacon Setup");
    println!();
    print_human_tone(HumanTone::Accent, "Environment");
    println!(
        "  Windows Terminal   {}",
        discovery.windows_terminal().label()
    );
    println!(
        "  Codex              {} — {} ({})",
        discovery.codex_version().unwrap_or("unavailable"),
        discovery.hook_profile().unwrap_or("unknown"),
        if discovery.profile_supported() {
            "supported"
        } else {
            "not admitted"
        },
    );
    println!("  TabBeacon          {}", discovery.tabbeacon_version());
    println!("  Binary             {}", discovery.binary_path().display());
    println!("  Hooks              {}", discovery.hooks().label());
    println!("  Doctor             {}", discovery.doctor_status());
    println!();
    print_human_tone(HumanTone::Accent, "Presentation");
    println!("  Title              {}", settings.title());
    println!("  Tab color          {}", settings.tab_color());
    println!("  Activity           {}", settings.activity());
    println!("  Spinner            {}", settings.spinner());
    println!("  Theme              {}", settings.theme());
    println!();
}

fn print_setup_change_plan(plan: &SetupPlan) {
    print_human_tone(HumanTone::Accent, "Planned changes");
    println!(
        "  Title              {} -> {}",
        plan.before().title(),
        plan.draft().title()
    );
    println!(
        "  Tab color          {} -> {}",
        plan.before().tab_color(),
        plan.draft().tab_color()
    );
    println!(
        "  Activity           {} -> {}",
        plan.before().activity(),
        plan.draft().activity()
    );
    println!(
        "  Spinner            {} -> {}",
        plan.before().spinner(),
        plan.draft().spinner()
    );
    println!(
        "  Theme              {} -> {}",
        plan.before().theme(),
        plan.draft().theme()
    );
    println!("  Unchanged owned state: existing ownership checks remain in effect.");
    println!(
        "  TabBeacon will not touch unrelated Codex, Windows Terminal, or PowerShell settings."
    );
    if plan.discovery().hooks() == tabbeacon::setup::HookSetupState::ReviewRequired {
        println!(
            "  Manual follow-up: launch codex, open /hooks, and review TabBeacon definitions."
        );
    }
}

struct DialoguerInput;

impl GuidedInput for DialoguerInput {
    fn select(&mut self, prompt: &str, items: &[&str], default: usize) -> Result<usize, String> {
        Select::new()
            .with_prompt(prompt)
            .items(items)
            .default(default)
            .interact()
            .map_err(|_| "guided setup selection was interrupted".to_owned())
    }
}

fn prompt_setup_draft_v3(
    current: PresentationSettings,
) -> Result<Option<PresentationSettings>, String> {
    let mut input = DialoguerInput;
    choose_presentation(&mut input, current)
}

fn prompt_setup_decision_v3() -> Result<SetupDecision, String> {
    let mut input = DialoguerInput;
    match input.select("Apply staged setup", &["Apply", "Cancel"], 1)? {
        0 => Ok(SetupDecision::Apply),
        1 => Ok(SetupDecision::Cancel),
        _ => Err("invalid setup decision".to_owned()),
    }
}

fn setup_input_error(error: &str) -> ExitCode {
    eprintln!("SETUP=FAIL");
    eprintln!("REASON={error}");
    eprintln!("SETTINGS_UNCHANGED=true");
    eprintln!("CODEX_CONFIG_UNCHANGED=true");
    eprintln!("HOOKS_UNCHANGED=true");
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
) -> ExitCode {
    let title_outcome = match apply_settings_change(store, before, after) {
        Ok(outcome) => outcome,
        Err(error) => return management_error("CONFIG", &error),
    };
    if output_mode == OutputMode::Plain {
        println!("CONFIG=PASS");
        println!(
            "CODEX_TITLE_OWNERSHIP={}",
            title_ownership_label(title_outcome)
        );
    } else {
        print_human_tone(HumanTone::Success, "Presentation settings updated.");
        print_human_tone(HumanTone::Dim, "Title ownership was reconciled safely.");
    }
    print_settings(store, after, output_mode);
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
) {
    if output_mode != OutputMode::Plain {
        print_human_tone(HumanTone::Accent, "Presentation settings");
        println!("  Title       {}", settings.title());
        println!("  Tab color   {}", settings.tab_color());
        println!("  Activity    {}", settings.activity());
        println!("  Spinner     {}", settings.spinner());
        println!("  Theme       {}", settings.theme());
        print_human_tone(
            HumanTone::Dim,
            "Settings are stored only in your user-local TabBeacon state.",
        );
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

fn print_config_choices(key: &str, output_mode: OutputMode) {
    if output_mode != OutputMode::Plain {
        eprintln!("Use tabbeacon config show for the current settings and supported values.");
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

fn print_config_failure(output_mode: OutputMode, reason: &str) {
    if output_mode == OutputMode::Plain {
        eprintln!("CONFIG=FAIL");
        eprintln!("REASON={reason}");
    } else {
        eprintln!("Configuration could not be updated: {reason}.");
    }
}

fn print_setup_cancelled(output_mode: OutputMode) {
    if output_mode == OutputMode::Plain {
        println!("SETUP=PASS");
        println!("SETUP_RESULT=CANCELLED");
        println!("SETTINGS_UNCHANGED=true");
        println!("CODEX_CONFIG_UNCHANGED=true");
        println!("HOOKS_UNCHANGED=true");
        println!("OWNER_ACTION=none");
    } else {
        print_human_tone(HumanTone::Attention, "Setup cancelled.");
        println!("No settings, Codex configuration, or hooks were changed.");
    }
}

fn print_setup_applied(
    store: &PresentationSettingsStore,
    settings: PresentationSettings,
    output_mode: OutputMode,
) {
    if output_mode == OutputMode::Plain {
        println!("SETUP=PASS");
        println!("SETUP_RESULT=APPLIED");
    } else {
        print_human_tone(HumanTone::Success, "Setup changes applied.");
    }
    print_settings(store, settings, output_mode);
}

fn print_human_tone(tone: HumanTone, line: impl AsRef<str>) {
    let color_enabled = auto_color_enabled(
        io::stdout().is_terminal(),
        std::env::var_os("NO_COLOR").is_some(),
    );
    println!("{}", style(tone, line.as_ref(), color_enabled));
}

fn print_human_lines(lines: impl IntoIterator<Item = String>) {
    for line in lines {
        let trimmed = line.trim_start();
        let tone = if trimmed.starts_with("TabBeacon ") || trimmed == "Sessions" {
            HumanTone::Accent
        } else if trimmed.starts_with("OK ") || trimmed.starts_with("No action required") {
            HumanTone::Success
        } else if trimmed.starts_with("X ") {
            HumanTone::Failure
        } else if trimmed.starts_with('!') || trimmed == "Attention" {
            HumanTone::Attention
        } else if trimmed.starts_with("Why:")
            || trimmed.starts_with("Next:")
            || trimmed.starts_with("Lease-based observation")
        {
            HumanTone::Dim
        } else {
            HumanTone::Plain
        };
        print_human_tone(tone, line);
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

fn wizard_error(error: &str) -> ExitCode {
    eprintln!("CONFIG=FAIL");
    eprintln!("REASON={error}");
    ExitCode::from(2)
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
    print_preview_result(settings)
}

fn print_preview_result(settings: PresentationSettings) -> ExitCode {
    match render_preview(settings) {
        Ok(()) => {
            println!("PREVIEW=PASS");
            println!("TITLE_SPINNER_FEASIBILITY=PRODUCTION");
            ExitCode::SUCCESS
        }
        Err(reason) => {
            eprintln!("PREVIEW=BLOCKED");
            eprintln!("REASON={reason}");
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
    print_config_choices(key, OutputMode::Plain);
    ExitCode::from(2)
}

fn settings_store() -> Result<PresentationSettingsStore, tabbeacon::settings::SettingsError> {
    PresentationSettingsStore::from_environment()
}

fn management_error(operation: &str, error: &dyn std::error::Error) -> ExitCode {
    eprintln!("{operation}=FAIL");
    eprintln!("REASON={error}");
    ExitCode::FAILURE
}

/// Returns whether this process can safely offer an inline interactive flow.
///
/// G40 intentionally does not enter raw or alternate-screen terminal modes;
/// later UI goals must keep this check as their admission boundary.
fn is_interactive_terminal() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

fn interactive_terminal_required(operation: &str, reason: &str, next_action: &str) -> ExitCode {
    eprintln!("{operation}=BLOCKED");
    eprintln!("REASON={reason}");
    eprintln!("SETTINGS_UNCHANGED=true");
    eprintln!("CODEX_CONFIG_UNCHANGED=true");
    eprintln!("HOOKS_UNCHANGED=true");
    eprintln!("NEXT_ACTION={next_action}");
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
    let mut settings_snapshot = match store.snapshot_read_only() {
        Ok(snapshot) => snapshot,
        Err(error) => return management_error("UI", &error),
    };
    let settings = settings_snapshot.settings();
    let report = collect_operational_diagnostics();
    let snapshot = ManagementSnapshot::from_diagnostics(&report);
    let overview = tabbeacon::management::ManagementOverview::from_diagnostics(&report);
    match tabbeacon::control_center::run(
        tabbeacon::control_center::ControlCenterApp::new(settings, snapshot, overview),
        |before, after| {
            let (_, next_snapshot) =
                apply_control_center_settings_change(&store, &settings_snapshot, before, after)?;
            settings_snapshot = next_snapshot;
            Ok(())
        },
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => management_error("UI", &error),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

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
