//! Feature-gated real-terminal lifecycle fixture for TB-G46.

use std::{io::IsTerminal, process::ExitCode};

use tabbeacon::{
    control_center::{ControlCenterApp, run_terminal_smoke_fixture},
    management::{ManagementHealth, ManagementOverview, ManagementSnapshot},
    settings::PresentationSettings,
};

fn main() -> ExitCode {
    if std::env::var_os("WT_SESSION").is_none()
        || !std::io::stdin().is_terminal()
        || !std::io::stdout().is_terminal()
    {
        eprintln!("WINDOWS_TERMINAL_TUI_SMOKE=FAIL");
        eprintln!("REASON=real interactive Windows Terminal required");
        return ExitCode::from(2);
    }

    let app = ControlCenterApp::new(
        PresentationSettings::default(),
        ManagementSnapshot {
            health: ManagementHealth::Healthy,
            issues: Vec::new(),
            recommended_actions: Vec::new(),
            change_plans: Vec::new(),
        },
        ManagementOverview::default(),
    );
    match run_terminal_smoke_fixture(app) {
        Ok(report) => {
            println!("WINDOWS_TERMINAL_TUI_SMOKE=PASS");
            println!("SCREENS_VISITED={}", report.screens_visited);
            println!("DRAFT_CHANGED={}", report.draft_changed);
            println!("DRAFT_REVERTED={}", report.draft_reverted);
            println!("CLEAN_QUIT={}", report.clean_quit);
            println!("TUI_EXIT_RESTORES_TERMINAL=true");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("WINDOWS_TERMINAL_TUI_SMOKE=FAIL");
            eprintln!("REASON={error}");
            ExitCode::FAILURE
        }
    }
}
