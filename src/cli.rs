//! Typed command-line grammar for the `TabBeacon` management surfaces.
//!
//! This module deliberately models parsing and output selection only. The
//! operational and ownership-aware implementation remains in the existing
//! domain modules so later human frontends do not need to parse stdout.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;

use crate::interface_preferences::InterfaceLanguage;

/// Top-level `TabBeacon` command-line interface.
#[derive(Debug, Parser)]
#[command(
    name = "tabbeacon",
    version,
    about = "Live identity and status beacons for Codex CLI tabs in Windows Terminal.",
    after_help = "Common commands:\n  tabbeacon setup codex\n  tabbeacon status --json\n  tabbeacon sessions --json\n  tabbeacon hooks --json\n  tabbeacon doctor --json\n  tabbeacon config show\n  tabbeacon alias show\n  tabbeacon completions powershell"
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
    /// Preview or explicitly restore only provably missing owned integration declarations.
    Repair {
        #[command(subcommand)]
        command: RepairCommand,
    },
    /// Diagnose the current installation.
    Doctor(DoctorArgs),
    /// Show the current operational state.
    Status(OutputArgs),
    /// Show privacy-preserving, read-only live session observations.
    Sessions(OutputArgs),
    /// Inspect the provider-neutral, command-redacted Hook inventory.
    Hooks(OutputArgs),
    /// Run bounded, no-mutation Agy qualification helpers; this never enables Agy.
    Agy {
        #[command(subcommand)]
        command: AgyPreadmissionCommand,
    },
    /// Diagnose whether a local package upgrade is blocked by a live owned worker.
    #[command(name = "upgrade-preflight")]
    UpgradePreflight(UpgradePreflightArgs),
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
    /// Session-scoped internal MCP Hook transport for admitted Codex profiles.
    #[command(name = "__mcp-hook-stdio-v1", hide = true)]
    McpHookStdio,
    /// Manage persisted presentation settings through the existing owner-aware store.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
        #[command(flatten)]
        output: HumanOutputArgs,
    },
    /// Manage user-local Human language, color, and reduced-motion preferences.
    Interface {
        #[command(subcommand)]
        command: InterfaceCommand,
        #[command(flatten)]
        output: HumanOutputArgs,
    },
    /// Inspect or explicitly override the local workspace alias.
    Alias {
        #[command(subcommand)]
        command: Option<AliasCommand>,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Explain a bounded, read-only presentation decision.
    Explain {
        #[command(subcommand)]
        command: ExplainCommand,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Export portable user configuration as canonical tabbeacon-export-v1 JSON.
    Export {
        /// Write the canonical document to a new file instead of stdout.
        #[arg(long = "output", value_name = "PATH")]
        destination: Option<PathBuf>,
        /// Explicitly replace an existing requested export file.
        #[arg(long, requires = "destination")]
        force: bool,
        #[command(flatten)]
        output: HumanOutputArgs,
    },
    /// Preview or explicitly apply a portable user-configuration import.
    Import {
        /// Bounded canonical tabbeacon-export-v1 JSON document to inspect.
        path: PathBuf,
        /// Apply the displayed plan; non-interactive imports never mutate without it.
        #[arg(long)]
        apply: bool,
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
    /// Refuse until an Owner-approved Agy setup profile exists.
    Agy,
}

/// Explicit ownership-safe repair operations.
#[derive(Debug, Subcommand)]
pub enum RepairCommand {
    /// Preview or restore missing exact Codex Hook declarations.
    Codex {
        /// Perform the displayed Hook-only repair after a fresh ownership preflight.
        #[arg(long)]
        apply: bool,
        /// Exact `TARGET_DIGEST` emitted by the read-only repair preview.
        #[arg(long, value_name = "SHA256", requires = "apply")]
        expected_target_digest: Option<String>,
        #[command(flatten)]
        output: OutputArgs,
    },
}

/// Explicitly pre-admission Agy qualification operations.
#[derive(Debug, Subcommand)]
pub enum AgyPreadmissionCommand {
    /// Run the cohesive disposable G64 qualification workflow.
    Qualification {
        #[command(subcommand)]
        command: AgyQualificationCommand,
    },
    /// Print the Owner-present G64 qualification plan without running Agy.
    Plan(OutputArgs),
    /// Compare a direct `agy --version` result with the separately audited docs version.
    Version {
        /// Version emitted by a direct, non-authenticating `agy --version` command.
        #[arg(long)]
        observed: Option<String>,
        /// Version heading recorded from the current official Agy documentation.
        #[arg(long)]
        documented: Option<String>,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Read one title/status JSON payload from stdin and print a content-minimal record.
    #[command(name = "title-state")]
    TitleState(OutputArgs),
    /// Read one Hook JSON payload from stdin and print a content-minimal record.
    #[command(name = "hook-state")]
    HookState {
        /// Known Hook event category; unknown events are intentionally not accepted here.
        #[arg(value_enum)]
        event: AgyHookEventArgument,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Internal disposable callback protocol harness: stdout is always one plain fallback title.
    #[command(name = "__title-callback-v1", hide = true)]
    TitleCallback,
}

/// Disposable Agy qualification workflow.
#[derive(Debug, Subcommand)]
pub enum AgyQualificationCommand {
    /// Show whether a managed disposable qualification workspace exists.
    Status {
        #[command(flatten)]
        workspace: AgyQualificationWorkspaceArgs,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Show the future Owner-present qualification plan without changing state.
    Plan(OutputArgs),
    /// Initialize a new disposable managed qualification workspace.
    Init {
        #[command(flatten)]
        workspace: AgyQualificationWorkspaceArgs,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Invoke only literal `agy --version` and record bounded facts.
    Probe {
        #[command(flatten)]
        workspace: AgyQualificationWorkspaceArgs,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Read one title/status JSON payload from stdin and persist only minimized facts.
    #[command(name = "record-title")]
    RecordTitle {
        #[command(flatten)]
        workspace: AgyQualificationWorkspaceArgs,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Read one Hook JSON payload from stdin and persist only minimized facts.
    #[command(name = "record-hook")]
    RecordHook {
        #[arg(value_enum)]
        event: AgyHookEventArgument,
        #[command(flatten)]
        workspace: AgyQualificationWorkspaceArgs,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Inspect accumulated minimized observations without showing raw events.
    Inspect {
        #[command(flatten)]
        workspace: AgyQualificationWorkspaceArgs,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Compile a stable unreviewed capability candidate.
    Profile {
        #[command(flatten)]
        workspace: AgyQualificationWorkspaceArgs,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Produce a pending Owner G64 review packet.
    Review {
        #[command(flatten)]
        workspace: AgyQualificationWorkspaceArgs,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Remove only a positively identified managed qualification workspace.
    Clean {
        /// Confirm deletion of the managed disposable workspace.
        #[arg(long)]
        confirm: bool,
        #[command(flatten)]
        workspace: AgyQualificationWorkspaceArgs,
        #[command(flatten)]
        output: OutputArgs,
    },
    /// Protocol callback: record minimized state, then emit one plain fallback title.
    #[command(name = "__title-callback-v1", hide = true)]
    TitleCallback {
        #[command(flatten)]
        workspace: AgyQualificationWorkspaceArgs,
    },
    /// Fail-open Hook callback: unknown events are not parsed or retained as raw payloads.
    #[command(name = "__hook-callback-v1", hide = true)]
    HookCallback {
        event: String,
        #[command(flatten)]
        workspace: AgyQualificationWorkspaceArgs,
    },
}

/// Optional explicit disposable root; otherwise user-local `TabBeacon` state is used.
#[derive(Clone, Debug, Args)]
pub struct AgyQualificationWorkspaceArgs {
    /// Absolute disposable root ending in `agy` or `tabbeacon-agy-qualification-*`.
    #[arg(long, value_name = "PATH")]
    pub root: Option<PathBuf>,
}

/// Known Agy Hook event spellings offered by the qualification command.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum AgyHookEventArgument {
    #[value(name = "pre-tool-use")]
    PreToolUse,
    #[value(name = "post-tool-use")]
    PostToolUse,
    #[value(name = "pre-invocation")]
    PreInvocation,
    #[value(name = "post-invocation")]
    PostInvocation,
    Stop,
}

impl AgyHookEventArgument {
    /// Exact external Hook event spelling passed to the content-minimizing recorder.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PreInvocation => "PreInvocation",
            Self::PostInvocation => "PostInvocation",
            Self::Stop => "Stop",
        }
    }
}

/// Output options shared by observational commands.
#[derive(Clone, Copy, Debug, Args)]
pub struct OutputArgs {
    /// Emit the stable machine-readable JSON document.
    #[arg(long, global = true, conflicts_with = "plain")]
    pub json: bool,
    /// Emit the legacy key-value/check representation.
    #[arg(long, global = true, conflicts_with = "json")]
    pub plain: bool,
    #[command(flatten)]
    pub language: LanguageArgs,
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
    #[command(flatten)]
    pub language: LanguageArgs,
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

/// Explicit Human locale selection shared by Human-capable commands.
#[derive(Clone, Copy, Debug, Args)]
pub struct LanguageArgs {
    /// Select Human language (`auto`, `en-US`, or `zh-CN`).
    #[arg(long, global = true, value_enum, value_name = "LOCALE")]
    pub lang: Option<LanguageArgument>,
}

impl LanguageArgs {
    /// Returns the typed override; `auto` intentionally continues precedence.
    #[must_use]
    pub const fn preference(self) -> Option<InterfaceLanguage> {
        match self.lang {
            Some(language) => Some(language.preference()),
            None => None,
        }
    }
}

/// One explicit CLI language selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum LanguageArgument {
    /// Continue through environment, user preference, and OS sources.
    Auto,
    /// Use English Human text.
    #[value(name = "en-US")]
    EnUs,
    /// Use Simplified Chinese Human text.
    #[value(name = "zh-CN")]
    ZhCn,
}

impl LanguageArgument {
    /// Converts the command-line spelling into the shared typed preference.
    #[must_use]
    pub const fn preference(self) -> InterfaceLanguage {
        match self {
            Self::Auto => InterfaceLanguage::Auto,
            Self::EnUs => InterfaceLanguage::EnUs,
            Self::ZhCn => InterfaceLanguage::ZhCn,
        }
    }
}

#[cfg(test)]
mod upgrade_preflight_tests {
    use clap::Parser;

    use super::{Cli, Command, OutputMode};

    #[test]
    fn upgrade_preflight_keeps_drain_explicit_and_machine_output_typed() {
        let default = Cli::try_parse_from(["tabbeacon", "upgrade-preflight"])
            .expect("default preflight parses");
        let Some(Command::UpgradePreflight(arguments)) = default.command else {
            panic!("upgrade preflight command is selected");
        };
        assert!(!arguments.drain);
        assert_eq!(arguments.output.mode(), OutputMode::Human);

        let drained = Cli::try_parse_from(["tabbeacon", "upgrade-preflight", "--drain", "--json"])
            .expect("explicit drain parses");
        let Some(Command::UpgradePreflight(arguments)) = drained.command else {
            panic!("upgrade preflight command is selected");
        };
        assert!(arguments.drain);
        assert_eq!(arguments.output.mode(), OutputMode::Json);
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
    /// Execute one exact owned Hook declaration with isolated temporary state.
    #[arg(long)]
    pub probe_hook_runtime: bool,
}

/// Arguments for the package-upgrade preflight.
#[derive(Clone, Copy, Debug, Args)]
pub struct UpgradePreflightArgs {
    /// Stop only processes freshly proven to be active `TabBeacon` activity workers.
    #[arg(long)]
    pub drain: bool,
    #[command(flatten)]
    pub output: OutputArgs,
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

/// User-local Interface preference operations.
#[derive(Debug, Subcommand)]
pub enum InterfaceCommand {
    /// Show effective user-local Interface preferences.
    Show,
    /// Set one Interface preference atomically.
    Set {
        /// Preference to update.
        key: InterfacePreferenceKey,
        /// Supported value for that preference.
        value: String,
    },
}

/// Device-local workspace alias operations.
#[derive(Debug, Subcommand)]
pub enum AliasCommand {
    /// Show the safe effective alias summary without creating local state.
    Show,
    /// Show bounded Adaptive Naming v2 suggestions without creating local state.
    Preview,
    /// Explain safe Adaptive Naming v2 inputs and scoring without creating state.
    Explain,
    /// Persist one explicit device-local workspace alias.
    Set { alias: String },
    /// Remove only this workspace's explicit device-local alias override.
    Reset,
}

/// Read-only explainability surfaces.
#[derive(Debug, Subcommand)]
pub enum ExplainCommand {
    /// Explain the safe workspace, presentation, and title-provenance facts.
    Title,
}

/// One typed Interface preference key.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum InterfacePreferenceKey {
    /// Human language policy.
    Language,
    /// Human terminal color policy.
    Color,
    /// Future Human animation reduction policy.
    #[value(name = "reduced-motion")]
    ReducedMotion,
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

    use super::{
        AgyHookEventArgument, AgyPreadmissionCommand, AgyQualificationCommand, AliasCommand, Cli,
        Command, ConvergenceCommand, InterfaceCommand, InterfacePreferenceKey, OutputMode,
        SetupCommand,
    };

    #[test]
    #[allow(clippy::too_many_lines)]
    fn parses_supported_production_commands_with_typed_output_modes() {
        let status =
            Cli::try_parse_from(["tabbeacon", "status", "--plain"]).expect("plain status parses");
        let Command::Status(output) = status.command.expect("status command") else {
            panic!("status command is typed");
        };
        assert_eq!(output.mode(), OutputMode::Plain);

        let alias = Cli::try_parse_from(["tabbeacon", "alias", "show", "--json"])
            .expect("alias JSON output parses after subcommand");
        let Command::Alias { command, output } = alias.command.expect("alias command") else {
            panic!("alias command is typed");
        };
        assert!(matches!(command, Some(AliasCommand::Show)));
        assert_eq!(output.mode(), OutputMode::Json);

        let bare_alias =
            Cli::try_parse_from(["tabbeacon", "alias"]).expect("bare alias command parses");
        let Command::Alias { command, output } = bare_alias.command.expect("alias command") else {
            panic!("alias command is typed");
        };
        assert!(command.is_none());
        assert_eq!(output.mode(), OutputMode::Human);

        let set_alias = Cli::try_parse_from(["tabbeacon", "alias", "set", "BEACON", "--plain"])
            .expect("alias set plain output parses after subcommand");
        let Command::Alias { command, output } = set_alias.command.expect("alias command") else {
            panic!("alias command is typed");
        };
        assert!(matches!(command, Some(AliasCommand::Set { .. })));
        assert_eq!(output.mode(), OutputMode::Plain);

        let localized = Cli::try_parse_from(["tabbeacon", "status", "--lang", "zh-CN"])
            .expect("exact BCP-47 status locale parses");
        let Command::Status(output) = localized.command.expect("localized status command") else {
            panic!("localized status command is typed");
        };
        assert!(output.language.preference().is_some());

        let sessions =
            Cli::try_parse_from(["tabbeacon", "sessions", "--json"]).expect("JSON sessions parses");
        let Command::Sessions(output) = sessions.command.expect("sessions command") else {
            panic!("sessions command is typed");
        };
        assert_eq!(output.mode(), OutputMode::Json);

        let hooks = Cli::try_parse_from(["tabbeacon", "hooks", "--plain"])
            .expect("plain Hook inventory parses");
        let Command::Hooks(output) = hooks.command.expect("Hooks command is typed") else {
            panic!("Hooks command is typed");
        };
        assert_eq!(output.mode(), OutputMode::Plain);

        let doctor = Cli::try_parse_from([
            "tabbeacon",
            "doctor",
            "--json",
            "--probe-title",
            "--probe-hook-runtime",
        ])
        .expect("doctor options parse in either declared order");
        let Command::Doctor(doctor) = doctor.command.expect("doctor command") else {
            panic!("doctor command is typed");
        };
        assert_eq!(doctor.output.mode(), OutputMode::Json);
        assert!(doctor.probe_title);
        assert!(doctor.probe_hook_runtime);

        let setup = Cli::try_parse_from(["tabbeacon", "setup", "codex"])
            .expect("direct Codex setup parses");
        let Command::Setup { command, .. } = setup.command.expect("setup command") else {
            panic!("setup command is typed");
        };
        assert!(matches!(command, Some(SetupCommand::Codex)));

        let agy_plan = Cli::try_parse_from(["tabbeacon", "agy", "plan", "--json"])
            .expect("Agy pre-admission plan parses");
        let Command::Agy { command } = agy_plan.command.expect("Agy command") else {
            panic!("Agy command is typed");
        };
        assert!(
            matches!(command, AgyPreadmissionCommand::Plan(output) if output.mode() == OutputMode::Json)
        );

        let agy_hook =
            Cli::try_parse_from(["tabbeacon", "agy", "hook-state", "post-tool-use", "--plain"])
                .expect("Agy Hook qualifier parses");
        let Command::Agy { command } = agy_hook.command.expect("Agy Hook command") else {
            panic!("Agy Hook command is typed");
        };
        assert!(matches!(
            command,
            AgyPreadmissionCommand::HookState {
                event: AgyHookEventArgument::PostToolUse,
                output,
            } if output.mode() == OutputMode::Plain
        ));

        let agy_inspect = Cli::try_parse_from([
            "tabbeacon",
            "agy",
            "qualification",
            "inspect",
            "--root",
            "qualification-root",
            "--json",
        ])
        .expect("cohesive Agy qualification command parses");
        let Command::Agy {
            command: AgyPreadmissionCommand::Qualification { command },
        } = agy_inspect.command.expect("Agy command")
        else {
            panic!("nested Agy qualification command is typed");
        };
        assert!(matches!(
            command,
            AgyQualificationCommand::Inspect { output, .. }
                if output.mode() == OutputMode::Json
        ));

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

        let interface = Cli::try_parse_from(["tabbeacon", "interface", "set", "language", "zh-CN"])
            .expect("Interface set parses");
        let Command::Interface { command, .. } = interface.command.expect("Interface command")
        else {
            panic!("Interface command is typed");
        };
        assert!(matches!(
            command,
            InterfaceCommand::Set {
                key: InterfacePreferenceKey::Language,
                ..
            }
        ));

        let export =
            Cli::try_parse_from(["tabbeacon", "export", "--output", "backup.json", "--force"])
                .expect("export file options parse");
        assert!(matches!(
            export.command,
            Some(Command::Export { force: true, .. })
        ));

        let import = Cli::try_parse_from(["tabbeacon", "import", "backup.json", "--apply"])
            .expect("explicit import apply parses");
        assert!(matches!(
            import.command,
            Some(Command::Import { apply: true, .. })
        ));

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
        assert!(Cli::try_parse_from(["tabbeacon", "status", "--lang", "fr-FR"]).is_err());
        assert!(Cli::try_parse_from(["tabbeacon", "sessions", "--json", "--plain"]).is_err());
        assert!(Cli::try_parse_from(["tabbeacon", "hooks", "--json", "--plain"]).is_err());
        assert!(Cli::try_parse_from(["tabbeacon", "alias", "show", "--json", "--plain"]).is_err());
        assert!(Cli::try_parse_from(["tabbeacon", "convergence", "verify"]).is_err());
        assert!(Cli::try_parse_from(["tabbeacon", "export", "--force"]).is_err());

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
