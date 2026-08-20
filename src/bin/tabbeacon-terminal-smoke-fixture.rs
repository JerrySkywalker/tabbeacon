//! Feature-gated real-terminal lifecycle fixture for TB-G46 and TB-G60.

use std::{
    env, fs,
    io::IsTerminal,
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

use tabbeacon::{
    control_center::{ControlCenterApp, run_terminal_smoke_fixture},
    hook_inventory::{HookInventory, HookInventoryAvailability},
    management::{ManagementHealth, ManagementOverview, ManagementSnapshot},
    providers::codex::CodexIntegration,
    settings::PresentationSettings,
};

fn fixture_hook_inventory() -> Result<HookInventory, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "fixture clock is unavailable".to_owned())?
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "tabbeacon-g60-tui-hook-inventory-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(root.join("codex-home"))
        .map_err(|_| "owned fixture Hook root could not be created".to_owned())?;
    let result = (|| {
        // The adapter sees a valid Codex Hook document, but the arbitrary
        // command remains inside owned temporary fixture state and is never
        // rendered or recorded by the smoke receipt.
        fs::write(
            root.join("codex-home/hooks.json"),
            br#"{"hooks":{"PreToolUse":[{"hooks":[{"type":"command","command":"fixture-private-command"}]}]}}"#,
        )
        .map_err(|_| "owned fixture Hook document could not be written".to_owned())?;
        let executable = env::current_exe()
            .map_err(|_| "fixture executable could not be resolved".to_owned())?;
        let inventory =
            CodexIntegration::new(root.join("codex-home"), root.join("state"), executable)
                .hook_inventory();
        (inventory.availability == HookInventoryAvailability::Available)
            .then_some(inventory)
            .ok_or_else(|| "fixture provider Hook shape was not admitted".to_owned())
    })();
    let _ = fs::remove_dir_all(&root);
    result
}

fn main() -> ExitCode {
    if std::env::var_os("WT_SESSION").is_none()
        || !std::io::stdin().is_terminal()
        || !std::io::stdout().is_terminal()
    {
        eprintln!("WINDOWS_TERMINAL_TUI_SMOKE=FAIL");
        eprintln!("REASON=real interactive Windows Terminal required");
        return ExitCode::from(2);
    }

    let hook_inventory = match fixture_hook_inventory() {
        Ok(inventory) => inventory,
        Err(error) => {
            eprintln!("WINDOWS_TERMINAL_TUI_SMOKE=FAIL");
            eprintln!("REASON={error}");
            return ExitCode::FAILURE;
        }
    };
    let app = ControlCenterApp::new(
        PresentationSettings::default(),
        ManagementSnapshot {
            health: ManagementHealth::Healthy,
            issues: Vec::new(),
            recommended_actions: Vec::new(),
            change_plans: Vec::new(),
        },
        ManagementOverview::default(),
    )
    .with_hook_inventory(hook_inventory);
    match run_terminal_smoke_fixture(app) {
        Ok(report) => {
            if let Some(path) = std::env::var_os("TABBEACON_TUI_SMOKE_RESULT_PATH") {
                let result = format!(
                    "TUI_LIVE_REFRESH={}\nTUI_WORKSPACE_SESSIONS={}\nTUI_HOOK_INVENTORY={}\nTUI_HOOK_PROVIDER_ADAPTER=true\nTUI_HELP_OVERLAY={}\nTUI_LANGUAGE_LIVE_SWITCH={}\nTUI_INTERFACE_REVERT={}\nTUI_INTERFACE_STAGED_APPLY={}\nOWNER_MUTATIONS=none\n",
                    report.live_refresh_merged,
                    report.workspace_and_sessions_visited,
                    report.hook_inventory_visited,
                    report.help_overlay_exercised,
                    report.interface_locale_switched,
                    report.interface_draft_reverted,
                    report.interface_apply_staged,
                );
                if let Err(error) = std::fs::write(path, result) {
                    eprintln!("WINDOWS_TERMINAL_TUI_SMOKE=FAIL");
                    eprintln!("REASON=fixture result receipt could not be written: {error}");
                    return ExitCode::FAILURE;
                }
            }
            println!("WINDOWS_TERMINAL_TUI_SMOKE=PASS");
            println!("SCREENS_VISITED={}", report.screens_visited);
            println!("TUI_LIVE_REFRESH={}", report.live_refresh_merged);
            println!(
                "TUI_WORKSPACE_SESSIONS={}",
                report.workspace_and_sessions_visited
            );
            println!("TUI_HOOK_INVENTORY={}", report.hook_inventory_visited);
            println!("TUI_HOOK_PROVIDER_ADAPTER=true");
            println!("TUI_HELP_OVERLAY={}", report.help_overlay_exercised);
            println!("DRAFT_CHANGED={}", report.draft_changed);
            println!("DRAFT_REVERTED={}", report.draft_reverted);
            println!(
                "TUI_LANGUAGE_LIVE_SWITCH={}",
                report.interface_locale_switched
            );
            println!("TUI_INTERFACE_REVERT={}", report.interface_draft_reverted);
            println!(
                "TUI_INTERFACE_STAGED_APPLY={}",
                report.interface_apply_staged
            );
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

#[cfg(test)]
mod tests {
    use super::fixture_hook_inventory;
    use tabbeacon::hook_inventory::HookInventoryAvailability;

    #[test]
    fn fixture_uses_the_real_codex_adapter_without_command_exposure() {
        let inventory = fixture_hook_inventory().expect("owned fixture inventory is available");
        assert_eq!(inventory.availability, HookInventoryAvailability::Available);
        assert!(
            inventory
                .entries
                .iter()
                .any(|entry| entry.provider == "codex")
        );
        let json = serde_json::to_string(&inventory).expect("inventory serializes");
        assert!(!json.contains("fixture-private-command"));
    }
}
