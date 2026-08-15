use std::{io::Read, process::ExitCode};

use tabbeacon::providers::codex::{
    CodexHookRuntime, CodexIntegration, DoctorStatus, SetupOutcome, UninstallOutcome,
};

const MAX_HOOK_INPUT_BYTES: u64 = 1024 * 1024;

fn main() -> ExitCode {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    match arguments.as_slice() {
        [command, provider] if command == "setup" && provider == "codex" => setup_codex(),
        [command] if command == "doctor" => doctor(),
        [command, provider] if command == "uninstall" && provider == "codex" => uninstall_codex(),
        [command, provider] if command == "hook" && provider == "codex" => run_codex_hook(),
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
    let integration = match CodexIntegration::from_environment() {
        Ok(integration) => integration,
        Err(error) => return management_error("SETUP", &error),
    };
    match integration.setup() {
        Ok(SetupOutcome::InstalledTrustReviewRequired) => {
            println!("SETUP_IDEMPOTENCE=PASS");
            println!("CODEX_INTEGRATION=INSTALLED");
            println!("HOOK_TRUST=REVIEW_REQUIRED");
            println!("OWNER_ACTION=launch codex and trust the TabBeacon hooks in /hooks");
            ExitCode::SUCCESS
        }
        Ok(SetupOutcome::Upgraded) => {
            println!("SETUP_IDEMPOTENCE=PASS");
            println!("CODEX_INTEGRATION=UPGRADED");
            println!("HOOK_TRUST=REVIEW_REQUIRED");
            println!("OWNER_ACTION=launch codex and trust the updated TabBeacon hooks in /hooks");
            ExitCode::SUCCESS
        }
        Ok(SetupOutcome::AlreadyInstalled) => {
            println!("SETUP_IDEMPOTENCE=PASS");
            println!("CODEX_INTEGRATION=ALREADY_INSTALLED");
            println!("OWNER_ACTION=run tabbeacon doctor to verify hook trust");
            ExitCode::SUCCESS
        }
        Err(error) => management_error("SETUP", &error),
    }
}

fn doctor() -> ExitCode {
    let integration = match CodexIntegration::from_environment() {
        Ok(integration) => integration,
        Err(error) => return management_error("DOCTOR", &error),
    };
    let report = integration.doctor();
    for check in report.checks() {
        println!(
            "CHECK={} STATUS={} SUMMARY={}",
            check.id(),
            check.status(),
            check.summary()
        );
    }
    println!("DOCTOR={}", report.overall());
    match report.overall() {
        DoctorStatus::Pass | DoctorStatus::Warning => ExitCode::SUCCESS,
        DoctorStatus::Fail => ExitCode::FAILURE,
    }
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

fn management_error(operation: &str, error: &dyn std::error::Error) -> ExitCode {
    eprintln!("{operation}=FAIL");
    eprintln!("REASON={error}");
    ExitCode::FAILURE
}

fn print_usage() {
    println!("TabBeacon {}", env!("CARGO_PKG_VERSION"));
    println!("Usage:");
    println!("  tabbeacon setup codex");
    println!("  tabbeacon doctor");
    println!("  tabbeacon uninstall codex");
}
