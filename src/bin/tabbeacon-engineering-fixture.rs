//! Feature-gated safe engineering host of the actual product Control Center event loop.
//! In-memory synthetic observations, no product store, Provider or Hook execution.
use std::{
    io::{self, IsTerminal},
    time::Duration,
};
use tabbeacon::human_presentation::ResolvedLocale;
use tabbeacon::{
    activity::{
        SessionOverview, SessionRecency, SessionWorkerHealth, SessionWorkspaceObservability,
        SessionsOverview,
    },
    control_center::{BoundedRefreshCollector, ControlCenterApp, ControlCenterRefresh},
    hook_inventory::{
        HookCurrentness, HookHandlerKind, HookInventory, HookInventoryEntry, HookOwner,
        HookSourceKind, HookTrustState,
    },
    interface_preferences::{InterfaceLanguage, InterfacePreferences},
    management::{
        ActionSafety, ChangePlan, HealthIssue, HealthSeverity, ManagementHealth,
        ManagementOverview, ManagementSnapshot,
    },
    presentation_policy::{
        ApplicationStatus, CliTarget, PresentationCapabilities, PresentationOverride,
        resolve_presentation,
    },
    providers::registry::ProviderRegistry,
    settings::PresentationSettings,
    title_explanation::TitleExplanation,
};
use terminal_ui_contract::keymap::KeymapProfile;
const TB_HEAD: &str = match option_env!("TB_ENGINEERING_SOURCE_HEAD") {
    Some(head) => head,
    None => "UNBOUND_BUILD",
};
const TUC_HEAD: &str = match option_env!("TB_ENGINEERING_TUC_HEAD") {
    Some(head) => head,
    None => "VERIFY_CARGO_LOCK",
};
fn fixture() -> ControlCenterRefresh {
    let presentation = PresentationSettings::default();
    let integrations = ProviderRegistry::default();
    let snapshot=ManagementSnapshot{
        health:ManagementHealth::Warning,
        issues:(0..24).map(|i|HealthIssue{id:format!("synthetic.{i}"),severity:HealthSeverity::Warning,title:format!("Fixture row {i} / 合成事实 {i}"),explanation:"Synthetic observation. Delivery, trust and production authorization remain UNPROVEN. / 合成观测，实机投递与权限未验证。".into(),remediation:None}).collect(),
        recommended_actions:vec![],
        change_plans:vec![ChangePlan{expected_repair:None,action_id:"terminal.title_policy_repair".into(),safety:ActionSafety::PreviewableSafeRepair,proposed_changes:vec!["SYNTHETIC PREVIEW ONLY / 合成预览".into()],protected_state:vec!["Host has no product writer / 无产品写入句柄".into()],manual_follow_up:vec!["Owner approval requires separate production evidence".into()]}],
    };
    let provider_capabilities = vec![
        (CliTarget::Codex, PresentationCapabilities::NONE),
        (CliTarget::Agy, PresentationCapabilities::NONE),
        (CliTarget::Cursor, PresentationCapabilities::NONE),
    ];
    let provider_presentation = provider_capabilities
        .iter()
        .map(|(p, c)| {
            (
                *p,
                resolve_presentation(
                    presentation,
                    PresentationOverride::default(),
                    *c,
                    ApplicationStatus::Unproven,
                ),
            )
        })
        .collect();
    let hooks = HookInventory::available(
        (0..30)
            .map(|i| {
                HookInventoryEntry::new(
                    "fixture",
                    format!("synthetic_event_{i}"),
                    HookOwner::UnownedOrAmbiguous,
                    false,
                    HookTrustState::UnsupportedOrUnavailable,
                    HookCurrentness::UnsupportedOrUnavailable,
                    HookSourceKind::UnsupportedOrUnavailable,
                    HookHandlerKind::Unsupported,
                    Some(3),
                    "0".repeat(64),
                )
            })
            .collect(),
    );
    let sessions = SessionsOverview {
        observation: "SYNTHETIC_TERMINAL_FIXTURE",
        sessions: (0..24)
            .map(|i| SessionOverview {
                workspace_alias: format!("FIXTURE{i}"),
                provider: "fixture".into(),
                semantic_state: "synthetic_unknown".into(),
                age_seconds: i,
                recency: SessionRecency::Aging,
                worker_health: SessionWorkerHealth::StaleLease,
                workspace_observability: SessionWorkspaceObservability::default(),
            })
            .collect(),
        ..SessionsOverview::default()
    };
    let overview = ManagementOverview {
        tabbeacon_version: "0.9.0 synthetic engineering host".into(),
        hook_trust: "UNPROVEN; no trust reads".into(),
        ..ManagementOverview::default()
    };
    ControlCenterRefresh {
        presentation,
        provider_presentation,
        provider_capabilities,
        interface: InterfacePreferences::default(),
        snapshot,
        overview,
        workspace: None,
        sessions,
        hooks,
        doctor_matrix: tabbeacon::doctor_matrix::DoctorMatrix::from_registry(&integrations),
        integrations,
        title_explanation: TitleExplanation::default(),
    }
}
struct FixtureOptions {
    profile: KeymapProfile,
    language: InterfaceLanguage,
    slow_ms: u64,
    keymap: Option<String>,
    interactive: bool,
    self_check: bool,
    output: Option<&'static str>,
    artifact: std::path::PathBuf,
}
fn options() -> io::Result<FixtureOptions> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut profile = KeymapProfile::Standard;
    let mut language = InterfaceLanguage::EnUs;
    let mut slow_ms = 0u64;
    let mut keymap = None;
    let mut interactive = false;
    let mut self_check = false;
    let mut output = None;
    let mut artifact = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--artifact-dir" => {
                i += 1;
                artifact = Some(std::path::PathBuf::from(args.get(i).ok_or_else(|| {
                    io::Error::other("explicit owned artifact path required")
                })?));
            }
            "--timing" => output = Some("software-trace.json"),
            "--interactive" => interactive = true,
            "--self-check" => self_check = true,
            "--profile" => {
                i += 1;
                profile = match args.get(i).map(String::as_str) {
                    Some("standard") => KeymapProfile::Standard,
                    Some("vim") => KeymapProfile::Vim,
                    _ => return Err(io::Error::other("profile requires standard or vim")),
                };
            }
            "--locale" => {
                i += 1;
                language = match args.get(i).map(String::as_str) {
                    Some("en-US") => InterfaceLanguage::EnUs,
                    Some("zh-CN") => InterfaceLanguage::ZhCn,
                    _ => return Err(io::Error::other("locale requires en-US or zh-CN")),
                };
            }
            "--slow-ms" => {
                i += 1;
                slow_ms = args
                    .get(i)
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| io::Error::other("slow-ms requires integer"))?;
                if slow_ms > 10000 {
                    return Err(io::Error::other("slow-ms limit is 10000"));
                }
            }
            "--keymap-file" => {
                i += 1;
                keymap = Some(
                    args.get(i)
                        .ok_or_else(|| io::Error::other("explicit keymap path required"))?
                        .clone(),
                );
            }
            _ => return Err(io::Error::other("unknown artifact host argument")),
        }
        i += 1;
    }
    let artifact = artifact.ok_or_else(|| {
        io::Error::other("--artifact-dir is required; no ambient store or output")
    })?;
    let artifact = artifact.canonicalize()?;
    if !artifact.join("ENGINEERING-FIXTURE-OWNED").is_file() {
        return Err(io::Error::other("owned fixture marker required"));
    }
    Ok(FixtureOptions {
        profile,
        language,
        slow_ms,
        keymap,
        interactive,
        self_check,
        output,
        artifact,
    })
}
fn main() -> io::Result<()> {
    let FixtureOptions {
        profile,
        language,
        slow_ms,
        keymap,
        interactive,
        self_check,
        output,
        artifact,
    } = options()?;
    let mut value = fixture();
    value.interface = value.interface.with_language(language);
    let mut app = ControlCenterApp::new(
        value.presentation,
        value.snapshot.clone(),
        value.overview.clone(),
    )
    .with_refresh(value.clone())
    .with_locale(if language == InterfaceLanguage::ZhCn {
        ResolvedLocale::ZhCn
    } else {
        ResolvedLocale::EnUs
    });
    if let Some(output) = output {
        app = app.with_software_timing(artifact.join(output));
    }
    app.set_keymap_profile(profile).map_err(io::Error::other)?;
    if let Some(path) = keymap {
        use std::io::Read;
        let mut text = String::new();
        std::fs::File::open(path)?
            .take(65537)
            .read_to_string(&mut text)?;
        if text.len() > 65536 {
            return Err(io::Error::other("keymap exceeds 64 KiB"));
        }
        // A named Custom measurement must prove admission, rather than measure fallback.
        app.load_keymap_toml(&text).map_err(io::Error::other)?;
        println!("EXPLICIT_CUSTOM_MAP_VALIDATION=PASS");
    }
    if self_check {
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(100, 30))
            .expect("TestBackend construction is infallible");
        terminal
            .draw(|frame| tabbeacon::control_center::render(frame, &app))
            .expect("TestBackend draw is infallible");
        println!(
            "SELF_CHECK=PASS SYNTHETIC_TESTBACKEND=true TB_HEAD={TB_HEAD} TUC_HEAD={TUC_HEAD} WRITER_AUTHORITY=NONE COLLECTOR=IN_MEMORY_SLOW_COOPERATIVE REAL_HUMAN_KEYBOARD=UNPROVEN REAL_HUMAN_MOUSE=UNPROVEN"
        );
        return Ok(());
    }
    if !interactive
        || std::env::var_os("WT_SESSION").is_none()
        || !io::stdin().is_terminal()
        || !io::stdout().is_terminal()
    {
        return Err(io::Error::other(
            "Caller must explicitly run --interactive in an owned Windows Terminal; owned fixture console required",
        ));
    }
    let collector = BoundedRefreshCollector(
        move |control: &terminal_ui_runtime::refresh::CollectionControl| {
            control
                .wait_for(Duration::from_millis(slow_ms))
                .map_err(io::Error::other)?;
            Ok(value.clone())
        },
    );
    let denied = || {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Artifact host has no persistent Apply/Repair/Trust/Save authority; terminal restored",
        )
    };
    let result = tabbeacon::control_center::run(
        app,
        |_, _| Err(denied()),
        |_, _, _| Err(denied()),
        collector,
        |_, _| Err(denied()),
        |_, _, _, _| Err(denied()),
    );
    println!(
        "OWNER_HOST_EXIT={:?} WRITER_AUTHORITY=NONE TB_HEAD={TB_HEAD} TUC_HEAD={TUC_HEAD} INPUT_SOURCE=SYNTHETIC_WHEN_AUTOMATED HUMAN_INPUT=UNPROVEN",
        result.as_ref().map(|()| "OK")
    );
    result
}
