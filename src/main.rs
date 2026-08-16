use std::{
    fs::OpenOptions,
    io::{self, Read, Write},
    process::ExitCode,
    thread,
    time::Duration,
};

use tabbeacon::diagnostics::{
    collect_operational_diagnostics, human_doctor_lines, human_status_lines,
};
use tabbeacon::providers::codex::{
    CodexHookRuntime, CodexIntegration, SetupOutcome, TitleOwnershipOutcome, UninstallOutcome,
};
use tabbeacon::setup::{
    SetupApplyResult, SetupDecision, SetupDiscovery, SetupPlan, detect_windows_terminal,
};
use tabbeacon::{
    activity::run_activity_worker_system,
    core::{Attention, Health, Phase},
    presentation::{
        PresentationPolicy, SemanticPresentationInput, WindowsTerminalCapabilities,
        WindowsTerminalRenderer,
    },
    settings::{
        ActivityMode, PresentationSettings, PresentationSettingsStore, PresentationTheme,
        SpinnerPreset, TabColorMode, TitleMode,
    },
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
        [command] if command == "setup" => guided_setup(),
        [command, provider] if command == "setup" && provider == "codex" => setup_codex(),
        [command] if command == "doctor" => doctor(false),
        [command, option] if command == "doctor" && option == "--json" => doctor(true),
        [command] if command == "status" => status(false),
        [command, option] if command == "status" && option == "--json" => status(true),
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

fn doctor(json: bool) -> ExitCode {
    let report = collect_operational_diagnostics();
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
    let mut console = open_preview_console().map_err(|_| "owned terminal output is unavailable")?;
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

#[cfg(windows)]
fn open_preview_console() -> io::Result<std::fs::File> {
    OpenOptions::new().write(true).open("CONOUT$")
}

#[cfg(not(windows))]
fn open_preview_console() -> io::Result<std::io::Stdout> {
    Ok(io::stdout())
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
    println!("  tabbeacon status");
    println!("  tabbeacon status --json");
    println!("  tabbeacon uninstall codex");
    println!("  tabbeacon preview [--theme <muted-dark|classic>] [--spinner <preset>]");
    println!("  tabbeacon config show|reset|wizard");
    println!("  tabbeacon config set <title|tab-color|activity|spinner|theme> <value>");
    println!("  tabbeacon config preset <native|minimal|balanced|full>");
}
