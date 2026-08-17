use std::{
    io::{self, Read, Write},
    process::ExitCode,
    thread,
    time::Duration,
};

use tabbeacon::diagnostics::{
    collect_operational_diagnostics, collect_operational_diagnostics_with_title_probe,
    human_doctor_lines, human_status_lines,
};
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
        ActivityMode, PresentationSettings, PresentationSettingsStore, PresentationTheme,
        SpinnerPreset, TabColorMode, TitleMode,
    },
    windows_terminal_policy::WindowsTerminalPolicyStore,
};

const MAX_HOOK_INPUT_BYTES: u64 = 1024 * 1024;

fn main() -> ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [command, key_digest, generation, revision] if command == "__activity-worker-v1" => {
            if let (Ok(generation), Ok(revision)) =
                (generation.parse::<u64>(), revision.parse::<u64>())
            {
                run_activity_worker_system(key_digest, generation, revision);
            }
            ExitCode::SUCCESS
        }
        [command, run_id, hold_millis] if command == "__title-probe-fixture-v1" => {
            match hold_millis.parse::<u64>() {
                Ok(hold_millis) => {
                    match tabbeacon::visual::emit_title_authority_fixture(run_id, hold_millis) {
                        Ok(()) => ExitCode::SUCCESS,
                        Err(_) => ExitCode::FAILURE,
                    }
                }
                Err(_) => ExitCode::FAILURE,
            }
        }
        [
            command,
            worker_pid,
            key_digest,
            generation,
            revision,
            owner_sha256,
            expected_executable,
        ] if command == "__activity-cleanup-observer-v1" => {
            if let (Ok(worker_pid), Ok(generation), Ok(revision)) = (
                worker_pid.parse::<u32>(),
                generation.parse::<u64>(),
                revision.parse::<u64>(),
            ) {
                run_activity_cleanup_observer_system(
                    worker_pid,
                    key_digest,
                    generation,
                    revision,
                    owner_sha256,
                    expected_executable,
                );
            }
            ExitCode::SUCCESS
        }
        [command] if command == "setup" => guided_setup(),
        [command, provider] if command == "setup" && provider == "codex" => setup_codex(),
        [command] if command == "doctor" => doctor(false, false),
        [command, option] if command == "doctor" && option == "--json" => doctor(true, false),
        [command, option] if command == "doctor" && option == "--probe-title" => {
            doctor(false, true)
        }
        [command, first, second]
            if command == "doctor"
                && ((first == "--json" && second == "--probe-title")
                    || (first == "--probe-title" && second == "--json")) =>
        {
            doctor(true, true)
        }
        [command] if command == "status" => status(false),
        [command, option] if command == "status" && option == "--json" => status(true),
        [command, rest @ ..] if command == "title-policy" => title_policy_command(rest),
        [command, rest @ ..] if command == "convergence" => convergence_command(rest),
        [command, provider] if command == "uninstall" && provider == "codex" => uninstall_codex(),
        [command, provider] if command == "hook" && provider == "codex" => run_codex_hook(),
        [command, subcommand] if command == "config" && subcommand == "show" => config_show(),
        [command, subcommand] if command == "config" && subcommand == "reset" => config_reset(),
        [command, subcommand] if command == "config" && subcommand == "wizard" => config_wizard(),
        [command, subcommand, preset] if command == "config" && subcommand == "preset" => {
            config_preset(preset)
        }
        [command, subcommand, key, value] if command == "config" && subcommand == "set" => {
            config_set(key, value)
        }
        [command, rest @ ..] if command == "preview" => preview(rest),
        [] => {
            print_usage();
            ExitCode::SUCCESS
        }
        [help] if help == "--help" || help == "-h" => {
            print_usage();
            ExitCode::SUCCESS
        }
        [version] if version == "--version" || version == "-V" => {
            println!("tabbeacon {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn title_policy_command(arguments: &[String]) -> ExitCode {
    let json = arguments
        .get(1)
        .is_some_and(|argument| argument == "--json");
    match arguments.first().map(String::as_str) {
        Some("inspect") if arguments.len() == 1 || (arguments.len() == 2 && json) => {
            title_policy_inspect(json)
        }
        Some("repair") if arguments.len() == 1 || (arguments.len() == 2 && json) => {
            title_policy_repair(json)
        }
        Some("restore") if arguments.len() == 1 || (arguments.len() == 2 && json) => {
            title_policy_restore(json)
        }
        Some(_) | None => {
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn convergence_command(arguments: &[String]) -> ExitCode {
    if arguments.first().map(String::as_str) == Some("verify") {
        return convergence_verify(&arguments[1..]);
    }
    let json = arguments
        .get(1)
        .is_some_and(|argument| argument == "--json");
    if arguments.first().map(String::as_str) != Some("matrix")
        || !(arguments.len() == 1 || (arguments.len() == 2 && json))
    {
        print_usage();
        return ExitCode::from(2);
    }
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

fn convergence_verify(arguments: &[String]) -> ExitCode {
    let matrix_path = option_value(arguments, "--matrix");
    let expected_head = option_value(arguments, "--expected-head");
    if arguments.len() != 4 || matrix_path.is_none() || expected_head.is_none() {
        print_usage();
        return ExitCode::from(2);
    }
    let expected_head = expected_head.expect("checked above");
    let path = std::path::Path::new(matrix_path.expect("checked above"));
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
    if run.expected_head != *expected_head {
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

fn option_value<'a>(arguments: &'a [String], flag: &str) -> Option<&'a String> {
    arguments
        .iter()
        .position(|argument| argument == flag)
        .and_then(|index| arguments.get(index + 1))
}

fn setup_codex() -> ExitCode {
    let settings = settings_store().map_or_else(
        |_| PresentationSettings::default(),
        |store| store.load_or_default(),
    );
    let integration = match CodexIntegration::from_environment() {
        Ok(integration) => integration,
        Err(error) => return management_error("SETUP", &error),
    };
    match integration.setup_with_title_ownership(settings.title().owns_tabbeacon_title()) {
        Ok(outcome) => print_setup_outcome(outcome),
        Err(error) => management_error("SETUP", &error),
    }
}

fn guided_setup() -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error("SETUP", &error),
    };
    let snapshot = match store.snapshot_read_only() {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("SETUP=FAIL");
            eprintln!("REASON={error}");
            eprintln!("SETTINGS_UNCHANGED=true");
            return ExitCode::FAILURE;
        }
    };
    let before = snapshot.settings();
    let integration = match CodexIntegration::from_environment() {
        Ok(integration) => integration,
        Err(error) => return management_error("SETUP", &error),
    };
    let binary_path = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => return management_error("SETUP", &error),
    };
    let discovery = SetupDiscovery::from_doctor(
        env!("CARGO_PKG_VERSION"),
        binary_path,
        detect_windows_terminal(),
        &integration.doctor(),
    );
    print_setup_discovery(&discovery, before);
    print_setup_title_policy(&WindowsTerminalPolicyStore::from_environment().inspect());

    let draft = match prompt_setup_draft(before) {
        Ok(settings) => settings,
        Err(error) => return setup_input_error(&error),
    };
    let plan = SetupPlan::new(before, discovery).with_draft(draft);
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
    let decision = match prompt_setup_decision() {
        Ok(decision) => decision,
        Err(error) => return setup_input_error(&error),
    };
    match decision {
        SetupDecision::Cancel => {
            let _ = plan.cancel();
            println!("SETUP=PASS");
            println!("SETUP_RESULT=CANCELLED");
            println!("SETTINGS_UNCHANGED=true");
            println!("CODEX_CONFIG_UNCHANGED=true");
            println!("HOOKS_UNCHANGED=true");
            println!("OWNER_ACTION=none");
            ExitCode::SUCCESS
        }
        SetupDecision::Apply => match plan.apply(&store, &snapshot, |owns_title| {
            integration
                .setup_with_title_ownership(owns_title)
                .map_err(|error| error.to_string())
        }) {
            Ok(SetupApplyResult::Applied(outcome)) => {
                println!("SETUP=PASS");
                println!("SETUP_RESULT=APPLIED");
                print_settings(&store, plan.draft());
                print_setup_outcome(outcome)
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

fn print_setup_outcome(outcome: SetupOutcome) -> ExitCode {
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

fn doctor(json: bool, probe_title: bool) -> ExitCode {
    let report = if probe_title {
        collect_operational_diagnostics_with_title_probe()
    } else {
        collect_operational_diagnostics()
    };
    if json {
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
    for line in human_doctor_lines(&report.doctor) {
        println!("{line}");
    }
    if report.doctor.is_failure() {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn status(json: bool) -> ExitCode {
    let report = collect_operational_diagnostics();
    if json {
        return match serde_json::to_string(&report) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => management_error("STATUS", &error),
        };
    }
    for line in human_status_lines(&report) {
        println!("{line}");
    }
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

fn uninstall_codex() -> ExitCode {
    let integration = match CodexIntegration::from_environment() {
        Ok(integration) => integration,
        Err(error) => return management_error("UNINSTALL", &error),
    };
    match integration.uninstall() {
        Ok(UninstallOutcome::Removed) => {
            println!("UNINSTALL_SAFETY=PASS");
            println!("CODEX_INTEGRATION=REMOVED");
            println!("OWNER_ACTION=none");
            ExitCode::SUCCESS
        }
        Ok(UninstallOutcome::NotInstalled) => {
            println!("UNINSTALL_SAFETY=PASS");
            println!("CODEX_INTEGRATION=NOT_INSTALLED");
            println!("OWNER_ACTION=none");
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

fn config_show() -> ExitCode {
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
    print_settings(&store, settings);
    ExitCode::SUCCESS
}

fn config_set(key: &str, value: &str) -> ExitCode {
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
        eprintln!("CONFIG=FAIL");
        eprintln!("REASON=unsupported config key or value");
        print_config_choices(key);
        return ExitCode::from(2);
    };
    persist_settings_change(&store, current, updated)
}

fn config_reset() -> ExitCode {
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
            Ok(outcome) => println!("CODEX_TITLE_OWNERSHIP={}", title_ownership_label(outcome)),
            Err(error) => return management_error("CONFIG", &error),
        }
    }
    println!("CONFIG=PASS");
    print_settings(&store, defaults);
    ExitCode::SUCCESS
}

fn config_preset(name: &str) -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error("CONFIG", &error),
    };
    let current = match store.load() {
        Ok(settings) => settings,
        Err(error) => return management_error("CONFIG", &error),
    };
    let Some(preset) = PresentationSettings::preset(name) else {
        eprintln!("CONFIG=FAIL");
        eprintln!("REASON=unsupported preset");
        eprintln!("PRESETS=native|minimal|balanced|full");
        return ExitCode::from(2);
    };
    persist_settings_change(&store, current, preset)
}

fn config_wizard() -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error("CONFIG", &error),
    };
    let current = store.load_or_default();
    println!("TabBeacon v0.1 presentation wizard (press Enter to keep each current value).");
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
    )
}

fn print_setup_discovery(discovery: &SetupDiscovery, settings: PresentationSettings) {
    println!("TabBeacon Setup");
    println!();
    println!("Environment");
    println!(
        "  Windows Terminal   {}",
        discovery.windows_terminal().label()
    );
    println!(
        "  Codex              {} / profile={} / supported={}",
        discovery.codex_version().unwrap_or("unavailable"),
        discovery.hook_profile().unwrap_or("unknown"),
        discovery.profile_supported(),
    );
    println!("  TabBeacon          {}", discovery.tabbeacon_version());
    println!("  Binary             {}", discovery.binary_path().display());
    println!("  Hooks              {}", discovery.hooks().label());
    println!("  Doctor             {}", discovery.doctor_status());
    println!();
    println!("Presentation");
    println!("  Title              {}", settings.title());
    println!("  Tab color          {}", settings.tab_color());
    println!("  Activity           {}", settings.activity());
    println!("  Spinner            {}", settings.spinner());
    println!("  Theme              {}", settings.theme());
    println!();
}

fn prompt_setup_draft(current: PresentationSettings) -> Result<PresentationSettings, String> {
    println!("Presets: native | minimal | balanced | full | custom");
    let preset = prompt_line("preset [custom]: ")?;
    let base = match preset.trim() {
        "" | "custom" => current,
        name => PresentationSettings::preset(name)
            .ok_or_else(|| "unsupported preset choice".to_owned())?,
    };
    println!("Title choices: tabbeacon | native | off");
    let title = prompt_choice("title", base.title().as_str(), TitleMode::parse)?;
    println!("Tab-color choices: tabbeacon | native | off");
    let tab_color = prompt_choice("tab-color", base.tab_color().as_str(), TabColorMode::parse)?;
    println!("Activity choices: title-spinner | title-indicator | wt-ring | both | native | off");
    let activity = prompt_choice("activity", base.activity().as_str(), ActivityMode::parse)?;
    println!("Spinner choices: codex | braille | quadrant | line | pulse");
    let spinner = prompt_choice("spinner", base.spinner().as_str(), SpinnerPreset::parse)?;
    println!("Theme choices: muted-dark | classic");
    let theme = prompt_choice("theme", base.theme().as_str(), PresentationTheme::parse)?;
    Ok(PresentationSettings::new(
        title, tab_color, activity, spinner, theme,
    ))
}

fn prompt_setup_decision() -> Result<SetupDecision, String> {
    let decision = prompt_line("Decision [apply/cancel] (default cancel): ")?;
    match decision.trim() {
        "" | "cancel" => Ok(SetupDecision::Cancel),
        "apply" => Ok(SetupDecision::Apply),
        _ => Err("decision must be apply or cancel".to_owned()),
    }
}

fn prompt_line(prompt: &str) -> Result<String, String> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|_| "cannot write setup prompt".to_owned())?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|_| "cannot read setup response".to_owned())?;
    Ok(line)
}

fn setup_input_error(error: &str) -> ExitCode {
    eprintln!("SETUP=FAIL");
    eprintln!("REASON={error}");
    eprintln!("SETTINGS_UNCHANGED=true");
    eprintln!("CODEX_CONFIG_UNCHANGED=true");
    eprintln!("HOOKS_UNCHANGED=true");
    ExitCode::from(2)
}

fn persist_settings_change(
    store: &PresentationSettingsStore,
    before: PresentationSettings,
    after: PresentationSettings,
) -> ExitCode {
    if let Err(error) = store.save(after) {
        return management_error("CONFIG", &error);
    }
    let title_outcome = if before.title() == after.title() {
        TitleOwnershipOutcome::AlreadyConfigured
    } else {
        match CodexIntegration::from_environment().and_then(|integration| {
            integration.reconcile_title_ownership(after.title().owns_tabbeacon_title())
        }) {
            Ok(outcome) => outcome,
            Err(error) => {
                let _ = store.save(before);
                return management_error("CONFIG", &error);
            }
        }
    };
    println!("CONFIG=PASS");
    println!(
        "CODEX_TITLE_OWNERSHIP={}",
        title_ownership_label(title_outcome)
    );
    print_settings(store, after);
    ExitCode::SUCCESS
}

fn title_ownership_label(outcome: TitleOwnershipOutcome) -> &'static str {
    match outcome {
        TitleOwnershipOutcome::Updated => "UPDATED",
        TitleOwnershipOutcome::AlreadyConfigured => "ALREADY_CONFIGURED",
        TitleOwnershipOutcome::NotInstalled => "NOT_INSTALLED",
    }
}

fn print_settings(store: &PresentationSettingsStore, settings: PresentationSettings) {
    println!("CONFIG_PATH={}", store.path().display());
    println!("TITLE_MODE={}", settings.title());
    println!("TAB_COLOR_MODE={}", settings.tab_color());
    println!("ACTIVITY_MODE={}", settings.activity());
    println!("SPINNER_PRESET={}", settings.spinner());
    println!("THEME={}", settings.theme());
    println!("TITLE_SPINNER_FEASIBILITY=PRODUCTION");
}

fn print_config_choices(key: &str) {
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

fn preview(arguments: &[String]) -> ExitCode {
    let store = match settings_store() {
        Ok(store) => store,
        Err(error) => return management_error("PREVIEW", &error),
    };
    let mut settings = store.load_or_default();
    let mut index = 0_usize;
    while index < arguments.len() {
        let Some(value) = arguments.get(index + 1) else {
            eprintln!("PREVIEW=FAIL");
            eprintln!("REASON=preview override needs a value");
            return ExitCode::from(2);
        };
        match arguments[index].as_str() {
            "--theme" => match PresentationTheme::parse(value) {
                Some(theme) => settings = settings.with_theme(theme),
                None => return preview_choice_error("theme"),
            },
            "--spinner" => match SpinnerPreset::parse(value) {
                Some(spinner) => settings = settings.with_spinner(spinner),
                None => return preview_choice_error("spinner"),
            },
            _ => {
                eprintln!("PREVIEW=FAIL");
                eprintln!("REASON=unsupported preview option");
                return ExitCode::from(2);
            }
        }
        index += 2;
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
    print_config_choices(key);
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

fn print_usage() {
    println!("TabBeacon {}", env!("CARGO_PKG_VERSION"));
    println!("Usage:");
    println!("  tabbeacon setup");
    println!("  tabbeacon setup codex");
    println!("  tabbeacon doctor");
    println!("  tabbeacon doctor --json");
    println!("  tabbeacon doctor --probe-title [--json]");
    println!("  tabbeacon status");
    println!("  tabbeacon status --json");
    println!("  tabbeacon title-policy inspect [--json]");
    println!("  tabbeacon title-policy repair [--json]");
    println!("  tabbeacon title-policy restore [--json]");
    println!("  tabbeacon convergence matrix [--json]");
    println!("  tabbeacon convergence verify --matrix <path> --expected-head <sha>");
    println!("  tabbeacon uninstall codex");
    println!("  tabbeacon preview [--theme <muted-dark|classic>] [--spinner <preset>]");
    println!("  tabbeacon config show|reset|wizard");
    println!("  tabbeacon config set <title|tab-color|activity|spinner|theme> <value>");
    println!("  tabbeacon config preset <native|minimal|balanced|full>");
}
