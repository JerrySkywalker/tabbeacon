//! Typed command-line grammar for the `TabBeacon` management surfaces.
//!
//! This module deliberately models parsing and output selection only. The
//! operational and ownership-aware implementation remains in the existing
//! domain modules so later human frontends do not need to parse stdout.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

/// Top-level `TabBeacon` command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "tabbeacon",
    version,
    about = "Live identity and status beacons for Codex CLI tabs in Windows Terminal.",
    after_help = "Common commands:\n  tabbeacon setup codex\n  tabbeacon status --json\n  tabbeacon sessions --json\n  tabbeacon doctor --json\n  tabbeacon config show\n  tabbeacon completions powershell"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// A named `TabBeacon` operation.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run inline guided setup, or install the Codex integration directly.
    Setup {
        #[command(subcommand)]
        command: Option<SetupCommand>,
        /// Visit only missing, stale, or action-required setup work.
        #[arg(long, conflicts_with = "full")]
        quick: bool,
        /// Revisit the complete guided setup flow.
        #[arg(long, conflicts_with = "quick")]
        full: bool,
        #[command(flatten)]
        output: HumanOutputArgs,
    },
    /// Diagnose the current installation.
    Doctor(DoctorArgs),
    /// Show the current operational state.
    Status(OutputArgs),
    /// Show privacy-preserving, read-only live session observations.
    Sessions(OutputArgs),
    /// Inspect or explicitly remediate Windows Terminal title ownership.
    #[command(name = "title-policy")]
    TitlePolicy {
        #[command(subcommand)]
        command: TitlePolicyCommand,
    },
    /// Inspect or verify the v0.3 convergence evidence matrix.
    Convergence {
        #[command(subcommand)]
        command: ConvergenceCommand,
    },
    /// Remove only the owned Codex integration declarations.
    Uninstall {
        provider: Provider,
        #[command(flatten)]
        output: HumanOutputArgs,
    },
    /// Receive a fail-open Codex hook payload from stdin.
    Hook { provider: Provider },
    /// Manage persisted presentation settings through the existing owner-aware store.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
        #[command(flatten)]
        output: HumanOutputArgs,
    },
    /// Render a temporary presentation preview without persisting a change.
    Preview(PreviewArgs),
    /// Emit a shell-completion script to stdout.
    Completions { shell: Shell },
    /// Reserve the Control Center command without entering a full-screen UI.
    Ui,
    /// Session-scoped internal activity worker.
    #[command(name = "__activity-worker-v1", hide = true)]
    ActivityWorker {
        key_digest: String,
        generation: String,
        revision: String,
    },
    /// Internal fixture for title-authority tests.
    #[command(name = "__title-probe-fixture-v1", hide = true)]
    TitleProbeFixture { run_id: String, hold_millis: String },
    /// Internal observer that cleans up activity-worker leases.
    #[command(name = "__activity-cleanup-observer-v1", hide = true)]
    ActivityCleanupObserver {
        worker_pid: String,
        key_digest: String,
        generation: String,
        revision: String,
        owner_sha256: String,
        expected_executable: String,
    },
}

/// Setup command variants.
#[derive(Debug, Subcommand)]
pub enum SetupCommand {
    /// Install or reconcile the Codex hook declarations.
    Codex,
}

/// Output options shared by observational commands.
#[derive(Clone, Copy, Debug, Args)]
pub struct OutputArgs {
    /// Emit the stable machine-readable JSON document.
    #[arg(long, conflicts_with = "plain")]
    pub json: bool,
    /// Emit the legacy key-value/check representation.
    #[arg(long, conflicts_with = "json")]
    pub plain: bool,
}

impl OutputArgs {
    /// Resolves the explicit output transport, defaulting to human-first output.
    #[must_use]
    pub const fn mode(self) -> OutputMode {
        if self.json {
            OutputMode::Json
        } else if self.plain {
            OutputMode::Plain
        } else {
            OutputMode::Human
        }
    }
}

/// The presentation transport requested by a command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputMode {
    /// Concise human-facing output. G40 preserves the existing text renderer.
    Human,
    /// Stable structured output for automation.
    Json,
    /// Legacy key-value/check output for compatibility.
    Plain,
}

/// An explicit legacy machine-output escape hatch for human-first commands.
#[derive(Clone, Copy, Debug, Args)]
pub struct HumanOutputArgs {
    /// Emit legacy key-value receipts for scripts that explicitly request them.
    #[arg(long, global = true)]
    pub plain: bool,
}

impl HumanOutputArgs {
    /// Resolves the default Human transport or the explicit legacy transport.
    #[must_use]
    pub const fn mode(self) -> OutputMode {
        if self.plain {
            OutputMode::Plain
        } else {
            OutputMode::Human
        }
    }
}

/// Doctor-specific observational flags.
#[derive(Clone, Copy, Debug, Args)]
pub struct DoctorArgs {
    #[command(flatten)]
    pub output: OutputArgs,
    /// Include the existing bounded visible-title probe.
    #[arg(long)]
    pub probe_title: bool,
}

/// Codex is the only production provider admitted in this train.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Provider {
    Codex,
}

/// Windows Terminal title-policy operations.
#[derive(Debug, Subcommand)]
pub enum TitlePolicyCommand {
    /// Inspect without mutating external settings.
    Inspect(JsonArgs),
    /// Apply the existing explicit ownership-safe repair.
    Repair(JsonArgs),
    /// Restore only an exact previously-owned repair.
    Restore(JsonArgs),
}

/// Commands whose historic machine interface is JSON-or-human.
#[derive(Clone, Copy, Debug, Args)]
pub struct JsonArgs {
    /// Emit the stable JSON document.
    #[arg(long)]
    pub json: bool,
}

/// Convergence evidence operations.
#[derive(Debug, Subcommand)]
pub enum ConvergenceCommand {
    /// Print the fixed evidence matrix.
    Matrix(JsonArgs),
    /// Verify one bounded evidence run against its candidate head.
    Verify {
        /// Path to the serialized convergence evidence run.
        #[arg(long)]
        matrix: PathBuf,
        /// Candidate head expected by the caller.
        #[arg(long)]
        expected_head: String,
    },
}

/// Existing presentation-setting operations.
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Print effective settings.
    Show,
    /// Set one direct automation value.
    Set { key: String, value: String },
    /// Apply one direct automation preset.
    Preset { name: String },
    /// Reset presentation settings to defaults.
    Reset,
    /// Run the legacy prompt-by-prompt settings wizard.
    Wizard,
}

/// Non-persistent preview overrides.
#[derive(Debug, Args)]
pub struct PreviewArgs {
    /// Override the preview theme.
    #[arg(long)]
    pub theme: Option<String>,
    /// Override the preview spinner.
    #[arg(long)]
    pub spinner: Option<String>,
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::{Cli, Command, ConvergenceCommand, OutputMode, SetupCommand};

    #[test]
    fn parses_supported_production_commands_with_typed_output_modes() {
        let status =
            Cli::try_parse_from(["tabbeacon", "status", "--plain"]).expect("plain status parses");
        let Command::Status(output) = status.command.expect("status command") else {
            panic!("status command is typed");
        };
        assert_eq!(output.mode(), OutputMode::Plain);

        let sessions =
            Cli::try_parse_from(["tabbeacon", "sessions", "--json"]).expect("JSON sessions parses");
        let Command::Sessions(output) = sessions.command.expect("sessions command") else {
            panic!("sessions command is typed");
        };
        assert_eq!(output.mode(), OutputMode::Json);

        let doctor = Cli::try_parse_from(["tabbeacon", "doctor", "--json", "--probe-title"])
            .expect("doctor options parse in either declared order");
        let Command::Doctor(doctor) = doctor.command.expect("doctor command") else {
            panic!("doctor command is typed");
        };
        assert_eq!(doctor.output.mode(), OutputMode::Json);
        assert!(doctor.probe_title);

        let setup = Cli::try_parse_from(["tabbeacon", "setup", "codex"])
            .expect("direct Codex setup parses");
        let Command::Setup { command, .. } = setup.command.expect("setup command") else {
            panic!("setup command is typed");
        };
        assert!(matches!(command, Some(SetupCommand::Codex)));

        let quick = Cli::try_parse_from(["tabbeacon", "setup", "--quick"])
            .expect("quick guided setup parses");
        let Command::Setup {
            command,
            quick,
            full,
            ..
        } = quick.command.expect("quick setup command")
        else {
            panic!("quick setup is typed");
        };
        assert!(command.is_none());
        assert!(quick);
        assert!(!full);

        let config = Cli::try_parse_from(["tabbeacon", "config", "show", "--plain"])
            .expect("legacy config output parses after the subcommand");
        let Command::Config { output, .. } = config.command.expect("config command") else {
            panic!("config output is typed");
        };
        assert_eq!(output.mode(), OutputMode::Plain);

        let uninstall = Cli::try_parse_from(["tabbeacon", "uninstall", "codex", "--plain"])
            .expect("legacy uninstall output parses after the provider");
        let Command::Uninstall { output, .. } = uninstall.command.expect("uninstall command")
        else {
            panic!("uninstall output is typed");
        };
        assert_eq!(output.mode(), OutputMode::Plain);
    }

    #[test]
    fn rejects_ambiguous_output_and_requires_convergence_flags() {
        assert!(Cli::try_parse_from(["tabbeacon", "status", "--json", "--plain"]).is_err());
        assert!(Cli::try_parse_from(["tabbeacon", "sessions", "--json", "--plain"]).is_err());
        assert!(Cli::try_parse_from(["tabbeacon", "convergence", "verify"]).is_err());

        let parsed = Cli::try_parse_from([
            "tabbeacon",
            "convergence",
            "verify",
            "--matrix",
            "receipt.json",
            "--expected-head",
            "abc123",
        ])
        .expect("convergence verifier parses its required values");
        assert!(matches!(
            parsed.command,
            Some(Command::Convergence {
                command: ConvergenceCommand::Verify { .. }
            })
        ));
    }

    #[test]
    fn hidden_runtime_commands_remain_parseable_but_are_not_in_help() {
        assert!(
            Cli::try_parse_from(["tabbeacon", "__activity-worker-v1", "digest", "1", "2",]).is_ok()
        );
        let help = Cli::command().render_help().to_string();
        assert!(!help.contains("__activity-worker-v1"));
    }
}
