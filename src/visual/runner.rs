//! Live Windows Terminal visual-harness orchestration.

use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
    process::{self, Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use super::uia::{OwnedTabActivation, OwnedTabTitleReader};
use super::{
    AnimationThreshold, AssertionKind, AssertionResult, Availability, CaptureBackend,
    ColorClassification, ColorMetrics, ColorSemantic, ColorTolerance, DesktopPreflight,
    EvidenceBundle, EvidenceIntegrity, EvidenceManifest, EvidenceWriter, FixtureDriver,
    MachineEnvironment, OwnedWindowCaptureTarget, PreflightBlocker, PreflightProbe,
    PrintWindowCaptureBackend, ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME, RgbaFrame, Roi, ScreenRect,
    SessionKind, TerminalTestSessionLauncher, UiaDump, VisualDisposition, VisualError,
    VisualResult, WindowsUiaLocator, assess_animation, classify_color_for_theme, matches_baseline,
    select_background_roi,
};

const LIVE_VISUAL_WORKER_BUDGET: Duration = Duration::from_secs(90);
const LIVE_VISUAL_WORKER_BUDGET_MILLIS: u64 = 90_000;
const LIVE_VISUAL_WORKER_STAGING_DIRECTORY: &str = ".tabbeacon-visual-worker";
const WORKER_POLL_INTERVAL: Duration = Duration::from_millis(25);
const WORKER_TERMINATION_BUDGET: Duration = Duration::from_secs(5);
const WORKER_PROCESS_QUERY_BUDGET: Duration = Duration::from_secs(3);
const WORKER_BUDGET_ENVIRONMENT_VARIABLE: &str = "TABBEACON_VISUAL_WORKER_BUDGET_MILLIS";
const WORKER_NONCE_ENVIRONMENT_VARIABLE: &str = "TABBEACON_VISUAL_WORKER_NONCE";

/// Inputs for one live visual-harness invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveVisualRunRequest {
    /// Candidate SHA that must match the checkout before a visual PASS is
    /// possible.
    pub expected_head: String,
    /// Unique evidence-directory and UIA correlation token.
    pub run_id: String,
    /// Root under which the owned run directory is created.
    pub evidence_root: PathBuf,
    /// Optional one-fixture smoke selection. `None` replays every G02 fixture.
    pub fixture_name: Option<String>,
}

/// Compact machine-readable outcome suitable for workflow-step output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiveVisualRunSummary {
    /// Overall live visual disposition.
    pub disposition: VisualDisposition,
    /// Candidate SHA supplied to this invocation.
    pub expected_head: String,
    /// SHA read from the checked-out repository.
    pub checked_out_head: String,
    /// SHA supported by visual evidence, present only on PASS.
    pub visual_head: Option<String>,
    /// Owned evidence directory.
    pub evidence_path: PathBuf,
    /// SHA-256 over deterministic evidence artifact records.
    pub evidence_tree_sha256: String,
    /// Preflight lane disposition.
    pub preflight: VisualDisposition,
    /// UIA target-resolution lane disposition.
    pub uia: VisualDisposition,
    /// Pixel-capture lane disposition.
    pub capture: VisualDisposition,
    /// UIA title lane disposition.
    pub title: VisualDisposition,
    /// Color-oracle lane disposition.
    pub color: VisualDisposition,
    /// Animation-oracle lane disposition.
    pub animation: VisualDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WorkerSupervisionEvidence {
    schema: String,
    execution: String,
    wall_clock_budget_millis: u64,
    deadline_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct WorkerAuthorization {
    schema: String,
    run_id: String,
    nonce: String,
    supervisor_process_id: u32,
}

/// Runs the trusted local visual fixture path through an owned, bounded worker
/// process.
///
/// UI Automation, foreground activation, and `PrintWindow` calls can block
/// synchronously. They therefore execute only in the helper process, which is
/// terminated by this outer supervisor if it exceeds its wall-clock budget.
/// The final evidence directory is created only by the supervisor after a
/// timeout, or by atomically promoting a fully finalized worker bundle.
///
/// # Errors
///
/// Returns infrastructure errors for invalid request paths or an inability to
/// preserve the required classified evidence. Individual desktop/UIA/capture
/// observations, including a worker deadline, are represented in the returned
/// classified summary rather than being converted into product failures.
pub fn run_live(request: &LiveVisualRunRequest) -> VisualResult<LiveVisualRunSummary> {
    let checked_out_head = checked_out_head()?;
    let fixture_names = selected_fixture_names(request)?;
    let exact_head = is_exact_sha(&request.expected_head)
        && is_exact_sha(&checked_out_head)
        && request.expected_head == checked_out_head;
    let (worker_root, worker_directory, final_directory) = worker_paths(request)?;
    let Ok(completion) = run_authorized_worker(request, &worker_root) else {
        return write_worker_failure(
            request,
            &checked_out_head,
            exact_head,
            &fixture_names,
            VisualDisposition::Blocked,
            "isolated visual worker could not be supervised to a classified completion",
        );
    };

    let mut summary = match completion {
        BoundedWorkerOutput::Completed => worker_evidence_summary(
            &worker_directory,
            request,
            &checked_out_head,
            &fixture_names,
        )
        .ok_or_else(|| {
            VisualError::Platform(
                "isolated visual worker did not produce a valid finalized evidence bundle"
                    .to_owned(),
            )
        })
        .or_else(|_| {
            write_worker_failure(
                request,
                &checked_out_head,
                exact_head,
                &fixture_names,
                VisualDisposition::Unproven,
                "isolated visual worker exited without a valid finalized evidence bundle",
            )
        })?,
        BoundedWorkerOutput::TimedOut => {
            return write_worker_failure(
                request,
                &checked_out_head,
                exact_head,
                &fixture_names,
                VisualDisposition::Blocked,
                "isolated visual worker exceeded its 90-second wall-clock budget and was terminated as an owned process tree",
            );
        }
        BoundedWorkerOutput::TerminationFailed => {
            return write_worker_failure(
                request,
                &checked_out_head,
                exact_head,
                &fixture_names,
                VisualDisposition::Blocked,
                "isolated visual worker could not be terminated as an owned process tree",
            );
        }
    };
    if fs::rename(&worker_directory, &final_directory).is_err() {
        return write_worker_failure(
            request,
            &checked_out_head,
            exact_head,
            &fixture_names,
            VisualDisposition::Unproven,
            "isolated visual worker evidence could not be atomically promoted",
        );
    }
    summary.evidence_path = final_directory;
    Ok(summary)
}

fn selected_fixture_names(request: &LiveVisualRunRequest) -> VisualResult<Vec<String>> {
    let driver = FixtureDriver::default();
    Ok(selected_replays(&driver, request)?
        .iter()
        .map(|replay| replay.case.fixture_name.clone())
        .collect())
}

fn worker_paths(request: &LiveVisualRunRequest) -> VisualResult<(PathBuf, PathBuf, PathBuf)> {
    let final_directory = request.evidence_root.join(&request.run_id);
    if final_directory.exists() {
        return Err(VisualError::EvidenceDirectoryExists(final_directory));
    }
    let worker_root = request
        .evidence_root
        .join(LIVE_VISUAL_WORKER_STAGING_DIRECTORY);
    let worker_directory = worker_root.join(&request.run_id);
    if worker_directory.exists() {
        return Err(VisualError::EvidenceDirectoryExists(worker_directory));
    }
    Ok((worker_root, worker_directory, final_directory))
}

fn run_authorized_worker(
    request: &LiveVisualRunRequest,
    worker_root: &Path,
) -> VisualResult<BoundedWorkerOutput> {
    let authorization = create_worker_authorization(worker_root, &request.run_id)?;
    let result = spawn_authorized_worker(request, worker_root, &authorization)
        .and_then(|worker| wait_for_bounded_worker(worker, LIVE_VISUAL_WORKER_BUDGET));
    clear_worker_authorization(&authorization);
    result
}

fn spawn_authorized_worker(
    request: &LiveVisualRunRequest,
    worker_root: &Path,
    authorization: &WorkerAuthorizationLease,
) -> VisualResult<Child> {
    let executable = env::current_exe().map_err(VisualError::Io)?;
    let mut worker = Command::new(executable);
    worker
        .arg("run-worker")
        .arg("--expected-head")
        .arg(&request.expected_head)
        .arg("--run-id")
        .arg(&request.run_id)
        .arg("--evidence-root")
        .arg(worker_root)
        .arg("--worker-authorization")
        .arg(&authorization.path);
    if let Some(fixture_name) = &request.fixture_name {
        worker.arg("--fixture").arg(fixture_name);
    }
    worker
        .env(
            WORKER_BUDGET_ENVIRONMENT_VARIABLE,
            LIVE_VISUAL_WORKER_BUDGET_MILLIS.to_string(),
        )
        .env(WORKER_NONCE_ENVIRONMENT_VARIABLE, &authorization.nonce)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(VisualError::Io)
}

/// Runs the visual observation inside the isolated helper process.
///
/// This function intentionally retains the former in-process behavior. It is
/// public only so the dedicated `tabbeacon-visual-fixture run-worker` command
/// can invoke it; product code must use [`run_live`] instead.
///
/// # Errors
///
/// Returns the classified filesystem, process, UIA, capture, or evidence error
/// that the worker observed before it could emit its summary.
pub fn run_live_in_worker(request: &LiveVisualRunRequest) -> VisualResult<LiveVisualRunSummary> {
    let checked_out_head = checked_out_head()?;
    let (environment, base_probe) = inspect_environment();
    let driver = FixtureDriver::default();
    let replays = selected_replays(&driver, request)?;
    let fixture_names = replays
        .iter()
        .map(|replay| replay.case.fixture_name.clone())
        .collect::<Vec<_>>();
    let writer = EvidenceWriter::create(&request.evidence_root, &request.run_id)?;

    let exact_head = is_exact_sha(&request.expected_head)
        && is_exact_sha(&checked_out_head)
        && request.expected_head == checked_out_head;
    let mut observation = Observation::new(DesktopPreflight::assess(base_probe), exact_head);
    observation.record_exact_head(&request.expected_head, &checked_out_head, exact_head);

    if !matches!(
        observation.preflight.disposition,
        VisualDisposition::Blocked
    ) && exact_head
    {
        let fixture_executable = env::current_exe().map_err(VisualError::Io)?;
        for replay in &replays {
            observe_replay(
                &writer,
                &fixture_executable,
                &request.run_id,
                replay,
                &mut observation,
            )?;
        }
    }

    observation.finalize_preflight(base_probe);
    let final_disposition = observation.disposition(exact_head);
    let visual_head =
        matches!(final_disposition, VisualDisposition::Pass).then_some(checked_out_head.clone());
    let manifest = EvidenceManifest {
        goal_id: "TB-G03".to_owned(),
        expected_head: request.expected_head.clone(),
        checked_out_head: checked_out_head.clone(),
        visual_head: visual_head.clone(),
        run_id: request.run_id.clone(),
        observed_at_unix_seconds: unix_seconds(),
        capture_backend: PrintWindowCaptureBackend.name().to_owned(),
        preflight: observation.preflight.clone(),
        environment: environment.clone(),
        window_geometry: observation.uia.window_bounds,
        fixtures: fixture_names,
        disposition: final_disposition,
    };
    let bundle = EvidenceBundle {
        manifest,
        assertions: observation.assertions,
        environment,
        uia: observation.uia,
        color_metrics: observation.metrics,
    };
    if let Some(supervision) = worker_supervision_evidence() {
        writer.write_json_document("worker-supervision.json", &supervision)?;
    }
    writer.write_bundle(&bundle)?;
    let integrity = writer.write_integrity_manifest()?;
    Ok(LiveVisualRunSummary {
        disposition: final_disposition,
        expected_head: request.expected_head.clone(),
        checked_out_head,
        visual_head,
        evidence_path: writer.directory().to_path_buf(),
        evidence_tree_sha256: integrity.tree_sha256,
        preflight: observation.preflight.disposition,
        uia: observation.lanes.uia(),
        capture: observation.lanes.capture(),
        title: observation.lanes.title(),
        color: observation.lanes.color(),
        animation: observation.lanes.animation(),
    })
}

enum BoundedWorkerOutput {
    Completed,
    TimedOut,
    TerminationFailed,
}

struct WorkerAuthorizationLease {
    path: PathBuf,
    nonce: String,
}

/// Waits for one directly spawned helper. At the deadline, it runs `taskkill`
/// only for the helper's direct PID and its descendants, then confirms bounded
/// direct-handle reaping without draining worker-owned output handles.
///
/// The directly spawned PID is sufficient ownership evidence for `taskkill
/// /T`. Process-tree inspection is deliberately not a completion precondition:
/// a stalled CIM query must not extend the worker deadline or turn an owned
/// timeout into an unbounded harness wait.
fn wait_for_bounded_worker(
    mut worker: Child,
    budget: Duration,
) -> VisualResult<BoundedWorkerOutput> {
    if wait_for_child_exit(&mut worker, budget)?.is_some() {
        return Ok(BoundedWorkerOutput::Completed);
    }
    if terminate_owned_worker_tree(&mut worker)? {
        Ok(BoundedWorkerOutput::TimedOut)
    } else {
        Ok(BoundedWorkerOutput::TerminationFailed)
    }
}

fn wait_for_child_exit(
    child: &mut Child,
    budget: Duration,
) -> VisualResult<Option<std::process::ExitStatus>> {
    let deadline = Instant::now() + budget;
    loop {
        if let Some(status) = child.try_wait().map_err(VisualError::Io)? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(WORKER_POLL_INTERVAL);
    }
}

fn terminate_owned_worker_tree(worker: &mut Child) -> VisualResult<bool> {
    let worker_pid = worker.id();
    let Ok(mut terminator) = Command::new("taskkill.exe")
        .args(["/PID", &worker_pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        let _ = terminate_direct_worker(worker);
        return Ok(false);
    };
    let terminated_tree = wait_for_child_exit(&mut terminator, WORKER_TERMINATION_BUDGET)?
        .is_some_and(|status| status.success());
    if !terminated_tree {
        let _ = terminator.kill();
        let _ = wait_for_child_exit(&mut terminator, WORKER_TERMINATION_BUDGET);
        let _ = terminate_direct_worker(worker);
        return Ok(false);
    }
    if !terminate_direct_worker(worker)? {
        return Ok(false);
    }
    Ok(true)
}

fn terminate_direct_worker(worker: &mut Child) -> VisualResult<bool> {
    if wait_for_child_exit(worker, WORKER_TERMINATION_BUDGET)?.is_some() {
        return Ok(true);
    }
    let _ = worker.kill();
    Ok(wait_for_child_exit(worker, WORKER_TERMINATION_BUDGET)?.is_some())
}

fn bounded_powershell_output(
    script: &str,
    budget: Duration,
) -> VisualResult<Option<std::process::Output>> {
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().map_err(VisualError::Io)?;
    if wait_for_child_exit(&mut child, budget)?.is_none() {
        let _ = child.kill();
        let _ = wait_for_child_exit(&mut child, WORKER_TERMINATION_BUDGET);
        return Ok(None);
    }
    child.wait_with_output().map(Some).map_err(VisualError::Io)
}

fn create_worker_authorization(
    worker_root: &Path,
    run_id: &str,
) -> VisualResult<WorkerAuthorizationLease> {
    if !is_safe_run_id(run_id) {
        return Err(VisualError::InvalidIdentifier(run_id.to_owned()));
    }
    fs::create_dir_all(worker_root)?;
    let path = worker_root.join(format!("{run_id}.authorization"));
    let nonce = worker_nonce();
    let bytes = serde_json::to_vec(&WorkerAuthorization {
        schema: "TABBEACON_VISUAL_WORKER_AUTHORIZATION_V1".to_owned(),
        run_id: run_id.to_owned(),
        nonce: nonce.clone(),
        supervisor_process_id: process::id(),
    })?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(VisualError::Io)?;
    file.write_all(&bytes).map_err(VisualError::Io)?;
    Ok(WorkerAuthorizationLease { path, nonce })
}

fn clear_worker_authorization(authorization: &WorkerAuthorizationLease) {
    let _ = fs::remove_file(&authorization.path);
    let _ = fs::remove_file(authorization.path.with_extension("consumed"));
}

fn worker_nonce() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{nanos:032x}-{:08x}", process::id())
}

fn is_safe_run_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

/// Confirms that the private worker command was launched by the outer
/// supervisor for this exact owned run before it performs any platform call.
///
/// # Errors
///
/// Returns an authorization error when the nonce or staging path does not
/// match the one-time supervisor record for this run.
pub fn authorize_live_worker(
    request: &LiveVisualRunRequest,
    authorization_path: &Path,
    nonce: &str,
) -> VisualResult<()> {
    let expected_path = request
        .evidence_root
        .join(format!("{}.authorization", request.run_id));
    if authorization_path != expected_path {
        return Err(VisualError::Platform(
            "visual worker authorization path did not match its owned staging run".to_owned(),
        ));
    }
    let authorization = consume_worker_authorization(authorization_path, request, nonce)?;
    let parent_process_id = worker_parent_process_id()?;
    if authorization.supervisor_process_id != parent_process_id
        || !parent_is_fixture_executable(parent_process_id)?
    {
        return Err(VisualError::Platform(
            "visual worker was not launched by the active fixture supervisor".to_owned(),
        ));
    }
    Ok(())
}

fn consume_worker_authorization(
    authorization_path: &Path,
    request: &LiveVisualRunRequest,
    nonce: &str,
) -> VisualResult<WorkerAuthorization> {
    let consumed_path = authorization_path.with_extension("consumed");
    fs::rename(authorization_path, &consumed_path).map_err(VisualError::Io)?;
    let result = (|| {
        let bytes = fs::read(&consumed_path).map_err(VisualError::Io)?;
        let authorization = serde_json::from_slice::<WorkerAuthorization>(&bytes)?;
        if authorization.schema != "TABBEACON_VISUAL_WORKER_AUTHORIZATION_V1"
            || authorization.run_id != request.run_id
            || authorization.nonce != nonce
        {
            return Err(VisualError::Platform(
                "visual worker authorization did not match the active supervisor run".to_owned(),
            ));
        }
        Ok(authorization)
    })();
    let _ = fs::remove_file(consumed_path);
    result
}

fn worker_parent_process_id() -> VisualResult<u32> {
    let process_id = process::id().to_string();
    let script = format!(
        "(Get-CimInstance Win32_Process -Filter 'ProcessId = {process_id}').ParentProcessId"
    );
    let output =
        bounded_powershell_output(&script, WORKER_PROCESS_QUERY_BUDGET)?.ok_or_else(|| {
            VisualError::Platform("visual worker parent process query timed out".to_owned())
        })?;
    if !output.status.success() {
        return Err(VisualError::Platform(
            "visual worker parent process query did not complete".to_owned(),
        ));
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse::<u32>()
        .map_err(|_| {
            VisualError::Platform("visual worker parent process was unavailable".to_owned())
        })
}

fn parent_is_fixture_executable(parent_process_id: u32) -> VisualResult<bool> {
    let script = format!(
        "(Get-CimInstance Win32_Process -Filter 'ProcessId = {parent_process_id}').ExecutablePath"
    );
    let output =
        bounded_powershell_output(&script, WORKER_PROCESS_QUERY_BUDGET)?.ok_or_else(|| {
            VisualError::Platform("visual worker parent identity query timed out".to_owned())
        })?;
    if !output.status.success() {
        return Err(VisualError::Platform(
            "visual worker parent identity query did not complete".to_owned(),
        ));
    }
    let observed = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let expected = env::current_exe().map_err(VisualError::Io)?;
    Ok(observed.eq_ignore_ascii_case(&expected.to_string_lossy()))
}

fn worker_evidence_summary(
    worker_directory: &Path,
    request: &LiveVisualRunRequest,
    checked_out_head: &str,
    fixture_names: &[String],
) -> Option<LiveVisualRunSummary> {
    let manifest = read_evidence_document::<EvidenceManifest>(worker_directory, "manifest.json")?;
    let assertions =
        read_evidence_document::<Vec<AssertionResult>>(worker_directory, "assertions.json")?;
    let integrity =
        read_evidence_document::<EvidenceIntegrity>(worker_directory, "integrity.json")?;
    let supervision = read_evidence_document::<WorkerSupervisionEvidence>(
        worker_directory,
        "worker-supervision.json",
    )?;
    worker_manifest_matches(
        &manifest,
        request,
        checked_out_head,
        fixture_names,
        &assertions,
    )
    .then_some(())?;
    worker_supervision_matches(&supervision).then_some(())?;
    evidence_integrity_matches(worker_directory, &integrity).then_some(())?;
    Some(LiveVisualRunSummary {
        disposition: manifest.disposition,
        expected_head: manifest.expected_head,
        checked_out_head: manifest.checked_out_head,
        visual_head: manifest.visual_head,
        evidence_path: worker_directory.to_path_buf(),
        evidence_tree_sha256: integrity.tree_sha256,
        preflight: manifest.preflight.disposition,
        uia: assertion_lane(&assertions, AssertionKind::UiaTarget),
        capture: assertion_lane(&assertions, AssertionKind::Capture),
        title: assertion_lane(&assertions, AssertionKind::Title),
        color: assertion_lane(&assertions, AssertionKind::Color),
        animation: assertion_lane(&assertions, AssertionKind::Animation),
    })
}

fn read_evidence_document<T: DeserializeOwned>(directory: &Path, name: &str) -> Option<T> {
    serde_json::from_slice(&fs::read(directory.join(name)).ok()?).ok()
}

fn worker_manifest_matches(
    manifest: &EvidenceManifest,
    request: &LiveVisualRunRequest,
    checked_out_head: &str,
    fixture_names: &[String],
    assertions: &[AssertionResult],
) -> bool {
    if manifest.goal_id != "TB-G03"
        || manifest.expected_head != request.expected_head
        || manifest.checked_out_head != checked_out_head
        || manifest.run_id != request.run_id
        || manifest.fixtures != fixture_names
        || manifest.preflight.disposition != assertion_lane(assertions, AssertionKind::Preflight)
        || !assertions.iter().any(|assertion| {
            assertion.kind == AssertionKind::ExactHead
                && assertion.disposition
                    == if request.expected_head == checked_out_head {
                        VisualDisposition::Pass
                    } else {
                        VisualDisposition::Fail
                    }
        })
    {
        return false;
    }
    if matches!(manifest.disposition, VisualDisposition::Pass) {
        manifest.validate_exact_heads_for_pass().is_ok()
            && assertions
                .iter()
                .all(|assertion| matches!(assertion.disposition, VisualDisposition::Pass))
    } else {
        manifest.visual_head.is_none()
    }
}

fn worker_supervision_matches(supervision: &WorkerSupervisionEvidence) -> bool {
    supervision
        == &WorkerSupervisionEvidence {
            schema: "TABBEACON_VISUAL_WORKER_SUPERVISION_V1".to_owned(),
            execution: "isolated-child-process".to_owned(),
            wall_clock_budget_millis: LIVE_VISUAL_WORKER_BUDGET_MILLIS,
            deadline_action: "terminate-direct-owned-worker".to_owned(),
        }
}

fn assertion_lane(assertions: &[AssertionResult], kind: AssertionKind) -> VisualDisposition {
    let mut lane = None;
    for assertion in assertions.iter().filter(|assertion| assertion.kind == kind) {
        merge(&mut lane, assertion.disposition);
    }
    lane.unwrap_or(VisualDisposition::Unproven)
}

fn evidence_integrity_matches(directory: &Path, integrity: &EvidenceIntegrity) -> bool {
    let Ok(entries) = fs::read_dir(directory) else {
        return false;
    };
    let mut actual = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else {
            return false;
        };
        let Ok(file_type) = entry.file_type() else {
            return false;
        };
        let Ok(name) = entry.file_name().into_string() else {
            return false;
        };
        if name == "integrity.json" {
            continue;
        }
        if !file_type.is_file() || !is_safe_evidence_file_name(&name) {
            return false;
        }
        let Ok(bytes) = fs::read(entry.path()) else {
            return false;
        };
        actual.push((
            name,
            u64::try_from(bytes.len()).ok(),
            format!("{:x}", Sha256::digest(bytes)),
        ));
    }
    actual.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    if integrity.algorithm != "SHA-256" || integrity.files.len() != actual.len() {
        return false;
    }
    let expected_matches = integrity
        .files
        .iter()
        .zip(&actual)
        .all(|(expected, actual)| {
            expected.name == actual.0
                && Some(expected.bytes) == actual.1
                && expected.sha256 == actual.2
        });
    expected_matches && evidence_tree_sha256(&integrity.files) == integrity.tree_sha256
}

fn is_safe_evidence_file_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn evidence_tree_sha256(files: &[super::EvidenceFileDigest]) -> String {
    let mut tree = Sha256::new();
    for file in files {
        tree.update(file.name.as_bytes());
        tree.update([0]);
        tree.update(file.bytes.to_string().as_bytes());
        tree.update([0]);
        tree.update(file.sha256.as_bytes());
        tree.update(*b"\n");
    }
    format!("{:x}", tree.finalize())
}

fn worker_supervision_evidence() -> Option<WorkerSupervisionEvidence> {
    let budget = env::var(WORKER_BUDGET_ENVIRONMENT_VARIABLE)
        .ok()?
        .parse::<u64>()
        .ok()?;
    Some(WorkerSupervisionEvidence {
        schema: "TABBEACON_VISUAL_WORKER_SUPERVISION_V1".to_owned(),
        execution: "isolated-child-process".to_owned(),
        wall_clock_budget_millis: budget,
        deadline_action: "terminate-direct-owned-worker".to_owned(),
    })
}

fn write_worker_failure(
    request: &LiveVisualRunRequest,
    checked_out_head: &str,
    exact_head: bool,
    fixture_names: &[String],
    disposition: VisualDisposition,
    detail: &str,
) -> VisualResult<LiveVisualRunSummary> {
    let writer = EvidenceWriter::create(&request.evidence_root, &request.run_id)?;
    let preflight = DesktopPreflight {
        disposition,
        blockers: matches!(disposition, VisualDisposition::Blocked)
            .then_some(PreflightBlocker::UnsupportedRuntime)
            .into_iter()
            .collect(),
        detail: detail.to_owned(),
    };
    let final_disposition = if exact_head {
        disposition
    } else {
        VisualDisposition::Fail
    };
    let fixture = fixture_names.first().cloned();
    let assertions = vec![
        AssertionResult::new(
            AssertionKind::Preflight,
            disposition,
            None,
            detail.to_owned(),
        ),
        AssertionResult::new(
            AssertionKind::ExactHead,
            if exact_head {
                VisualDisposition::Pass
            } else {
                VisualDisposition::Fail
            },
            None,
            format!(
                "expected={} checked_out={checked_out_head}",
                request.expected_head
            ),
        ),
        AssertionResult::new(
            AssertionKind::UiaTarget,
            disposition,
            fixture,
            detail.to_owned(),
        ),
    ];
    let environment = unobserved_worker_environment();
    let bundle = EvidenceBundle {
        manifest: EvidenceManifest {
            goal_id: "TB-G03".to_owned(),
            expected_head: request.expected_head.clone(),
            checked_out_head: checked_out_head.to_owned(),
            visual_head: None,
            run_id: request.run_id.clone(),
            observed_at_unix_seconds: unix_seconds(),
            capture_backend: PrintWindowCaptureBackend.name().to_owned(),
            preflight: preflight.clone(),
            environment: environment.clone(),
            window_geometry: None,
            fixtures: fixture_names.to_vec(),
            disposition: final_disposition,
        },
        assertions,
        environment,
        uia: empty_uia_dump(),
        color_metrics: Vec::new(),
    };
    writer.write_bundle(&bundle)?;
    let integrity = writer.write_integrity_manifest()?;
    Ok(LiveVisualRunSummary {
        disposition: final_disposition,
        expected_head: request.expected_head.clone(),
        checked_out_head: checked_out_head.to_owned(),
        visual_head: None,
        evidence_path: writer.directory().to_path_buf(),
        evidence_tree_sha256: integrity.tree_sha256,
        preflight: disposition,
        uia: disposition,
        capture: VisualDisposition::Unproven,
        title: VisualDisposition::Unproven,
        color: VisualDisposition::Unproven,
        animation: VisualDisposition::Unproven,
    })
}

fn unobserved_worker_environment() -> MachineEnvironment {
    MachineEnvironment {
        machine: "NOT_OBSERVED".to_owned(),
        windows_version: "NOT_OBSERVED".to_owned(),
        terminal_version: "NOT_OBSERVED".to_owned(),
        session_id: "NOT_OBSERVED".to_owned(),
        session_kind: "NOT_OBSERVED".to_owned(),
        desktop: "NOT_OBSERVED: isolated visual worker did not return evidence".to_owned(),
        dpi_scaling: "NOT_OBSERVED".to_owned(),
        display_geometry: None,
        rust_toolchain: "NOT_OBSERVED".to_owned(),
    }
}

fn observe_replay(
    writer: &EvidenceWriter,
    fixture_executable: &Path,
    run_id: &str,
    replay: &super::FixtureReplay,
    observation: &mut Observation,
) -> VisualResult<()> {
    let launcher = TerminalTestSessionLauncher::default();
    if let Err(error) = launcher.launch(fixture_executable, replay, run_id) {
        observation.record_uia_blocked(&replay.case.fixture_name, error.to_string());
        return Ok(());
    }
    let locator = WindowsUiaLocator;
    let target = match locate_activated_with_retry(locator, run_id, replay) {
        Ok(target) => target,
        Err(failure) => {
            if let Some(target) = failure.last_target {
                observation.record_target(writer, replay, &target)?;
                observation.record_capture_blocked(
                    &replay.case.fixture_name,
                    format!("owned-window activation was refused: {}", failure.error),
                );
            } else {
                observation
                    .record_uia_failure(&replay.case.fixture_name, failure.error.to_string());
            }
            return Ok(());
        }
    };
    observation.record_target(writer, replay, &target.dump)?;
    if replay.case.expects_title_animation
        && let Some(reader) = target.title_reader.as_ref()
    {
        observe_title_animation(writer, reader, replay, observation)?;
    }
    if !target_has_capturable_geometry(&target.dump) {
        observation.record_capture_blocked(
            &replay.case.fixture_name,
            "UIA did not provide capturable owned window/tab geometry",
        );
        return Ok(());
    }
    let Some((window_bounds, tab_bounds)) = capture_bounds(&target.dump) else {
        observation.record_capture_blocked(
            &replay.case.fixture_name,
            "UIA did not provide owned window/tab bounds",
        );
        return Ok(());
    };
    if !target
        .dump
        .activation
        .as_ref()
        .is_some_and(|activation| activation.set_foreground)
    {
        observation.record_capture_blocked(
            &replay.case.fixture_name,
            "owned Terminal window did not confirm foreground activation; visibility-dependent capture refused",
        );
        return Ok(());
    }
    let capture_target_result = match target.dump.native_window_id {
        Some(window_handle) => OwnedWindowCaptureTarget::new(window_handle, window_bounds),
        None => Err(VisualError::Platform(
            "owned UIA target did not expose a native HWND".to_owned(),
        )),
    };
    let capture_target = match capture_target_result {
        Ok(target) => target,
        Err(error) => {
            observation.record_capture_blocked(&replay.case.fixture_name, error.to_string());
            return Ok(());
        }
    };
    observe_capture(writer, replay, &capture_target, tab_bounds, observation)
}

struct ActivationRetryFailure {
    error: VisualError,
    last_target: Option<UiaDump>,
}

struct ActivatedTarget {
    dump: UiaDump,
    title_reader: Option<OwnedTabTitleReader>,
}

fn locate_activated_with_retry(
    locator: WindowsUiaLocator,
    run_id: &str,
    replay: &super::FixtureReplay,
) -> Result<ActivatedTarget, Box<ActivationRetryFailure>> {
    let mut last_target = None;
    let mut last_error = None;
    for _ in 0..20 {
        match locator
            .locate_and_activate_any_with_title_reader(run_id, &replay.case.expected_title_frames)
        {
            Ok(OwnedTabActivation::Activated {
                dump, title_reader, ..
            }) => {
                let target = ActivatedTarget {
                    dump,
                    title_reader: replay.case.expects_title_animation.then_some(title_reader),
                };
                // UIA title evidence is valid after exact owned-tab
                // correlation even if foreground activation or pixel capture
                // is unavailable. The caller applies the stricter capture
                // preconditions only to screenshot/color assertions.
                return Ok(target);
            }
            Ok(OwnedTabActivation::Refused { dump, detail }) => {
                last_target = Some(dump);
                last_error = Some(VisualError::Platform(detail));
            }
            Err(error) => last_error = Some(error),
        }
        thread::sleep(Duration::from_millis(200));
    }
    let error = last_error.unwrap_or_else(|| {
        let detail = last_target.clone().map_or_else(
            || "no activated UIA target was observed".to_owned(),
            |target| {
                format!(
                    "last target had activation={:?}; window={:?}; tab={:?}",
                    target.activation, target.window_bounds, target.tab_bounds
                )
            },
        );
        VisualError::Platform(format!(
            "owned-window activation produced no UIA observation: {detail}"
        ))
    });
    Err(Box::new(ActivationRetryFailure { error, last_target }))
}

fn observe_title_animation(
    writer: &EvidenceWriter,
    reader: &OwnedTabTitleReader,
    replay: &super::FixtureReplay,
    observation: &mut Observation,
) -> VisualResult<()> {
    // v0.3 requires at least three distinct working frames in one second. UIA
    // sampling stays intentionally incommensurate with the frame cadence.
    const TITLE_ANIMATION_OBSERVATION_BUDGET: Duration = Duration::from_secs(1);
    const MINIMUM_WORKING_TITLE_FRAMES: usize = 3;
    let observed = reader
        .observe_frames(
            &replay.case.expected_title_frames,
            TITLE_ANIMATION_OBSERVATION_BUDGET,
            MINIMUM_WORKING_TITLE_FRAMES,
        )
        .unwrap_or_default();
    writer.write_json_document(
        &format!("title-frames-{}.json", replay.case.fixture_name),
        &observed,
    )?;
    observation.record_title_animation(&replay.case.fixture_name, &observed);
    Ok(())
}

fn observe_capture(
    writer: &EvidenceWriter,
    replay: &super::FixtureReplay,
    capture_target: &OwnedWindowCaptureTarget,
    tab_bounds: ScreenRect,
    observation: &mut Observation,
) -> VisualResult<()> {
    let capture_backend = PrintWindowCaptureBackend;
    let frames = match capture_frames(&capture_backend, capture_target, 3) {
        Ok(frames) => frames,
        Err(error) => {
            observation.record_capture_blocked(&replay.case.fixture_name, error.to_string());
            return Ok(());
        }
    };
    let first_frame = frames
        .first()
        .cloned()
        .ok_or_else(|| VisualError::Platform("capture returned no frames".to_owned()))?;
    let tab_roi = match relative_roi(capture_target.window_bounds, tab_bounds, &first_frame) {
        Ok(tab_roi) => tab_roi,
        Err(error) => {
            observation.record_capture_blocked(
                &replay.case.fixture_name,
                format!("UIA geometry became non-capturable before frame sampling: {error}"),
            );
            return Ok(());
        }
    };
    write_capture_images(writer, replay, &first_frame, tab_roi)?;
    observation.record_capture_pass(&replay.case.fixture_name, capture_backend.name());
    observe_color(writer, replay, &first_frame, tab_roi, observation)?;
    if replay.case.expects_animation {
        observe_animation(writer, replay, &frames, tab_roi, observation)?;
    }
    Ok(())
}

fn write_capture_images(
    writer: &EvidenceWriter,
    replay: &super::FixtureReplay,
    frame: &RgbaFrame,
    tab_roi: Roi,
) -> VisualResult<()> {
    writer.write_png(
        &format!("full-window-{}-001", replay.case.fixture_name),
        frame,
    )?;
    writer.write_png(
        &format!("tab-{}", replay.case.fixture_name),
        &frame.crop(tab_roi)?,
    )?;
    Ok(())
}

fn observe_color(
    writer: &EvidenceWriter,
    replay: &super::FixtureReplay,
    frame: &RgbaFrame,
    tab_roi: Roi,
    observation: &mut Observation,
) -> VisualResult<()> {
    let (color_roi, color) = select_background_roi(frame, tab_roi)?;
    writer.write_png(
        &format!("tab-color-roi-{}", replay.case.fixture_name),
        &frame.crop(color_roi)?,
    )?;
    writer.write_json_document(
        &format!("color-metrics-{}.json", replay.case.fixture_name),
        &color,
    )?;
    observation.record_color(replay, color_roi, color);
    Ok(())
}

fn observe_animation(
    writer: &EvidenceWriter,
    replay: &super::FixtureReplay,
    frames: &[RgbaFrame],
    tab_roi: Roi,
    observation: &mut Observation,
) -> VisualResult<()> {
    let progress_roi = progress_roi(tab_roi)?;
    for (index, frame) in frames.iter().enumerate() {
        writer.write_png(
            &format!("progress-roi-{}-{:03}", replay.case.fixture_name, index + 1),
            &frame.crop(progress_roi)?,
        )?;
    }
    let (outcome, deltas) = assess_animation(frames, progress_roi, AnimationThreshold::default())?;
    writer.write_json_document(
        &format!("frame-delta-{}.json", replay.case.fixture_name),
        &deltas,
    )?;
    observation.record_animation(&replay.case.fixture_name, outcome, &deltas);
    Ok(())
}

fn capture_bounds(target: &UiaDump) -> Option<(ScreenRect, ScreenRect)> {
    target.window_bounds.zip(target.tab_bounds)
}

fn target_has_capturable_geometry(target: &UiaDump) -> bool {
    capture_bounds(target).is_some_and(|(window, tab)| {
        if window.width == 0 || window.height == 0 || tab.width == 0 || tab.height == 0 {
            return false;
        }
        let window_left = i64::from(window.left);
        let window_top = i64::from(window.top);
        let window_right = window_left + i64::from(window.width);
        let window_bottom = window_top + i64::from(window.height);
        let tab_left = i64::from(tab.left);
        let tab_top = i64::from(tab.top);
        let tab_right = tab_left + i64::from(tab.width);
        let tab_bottom = tab_top + i64::from(tab.height);
        tab_left >= window_left
            && tab_top >= window_top
            && tab_right <= window_right
            && tab_bottom <= window_bottom
    })
}

struct Observation {
    assertions: Vec<AssertionResult>,
    preflight: DesktopPreflight,
    uia: UiaDump,
    metrics: Vec<(String, ColorMetrics)>,
    baseline: Option<ColorMetrics>,
    saw_non_pass: bool,
    lanes: LaneDisposition,
}

impl Observation {
    fn new(preflight: DesktopPreflight, exact_head: bool) -> Self {
        let mut lanes = LaneDisposition::default();
        lanes.observe_preflight(preflight.disposition);
        let assertions = vec![AssertionResult::new(
            AssertionKind::Preflight,
            preflight.disposition,
            None,
            preflight.detail.clone(),
        )];
        Self {
            assertions,
            preflight,
            uia: empty_uia_dump(),
            metrics: Vec::new(),
            baseline: None,
            saw_non_pass: !exact_head,
            lanes,
        }
    }

    fn record_exact_head(&mut self, expected: &str, checked_out: &str, exact: bool) {
        self.assertions.push(AssertionResult::new(
            AssertionKind::ExactHead,
            if exact {
                VisualDisposition::Pass
            } else {
                VisualDisposition::Fail
            },
            None,
            format!("expected={expected} checked_out={checked_out}"),
        ));
    }

    fn record_uia_blocked(&mut self, fixture: &str, detail: String) {
        self.saw_non_pass = true;
        self.lanes.observe_uia(VisualDisposition::Blocked);
        self.assertions.push(AssertionResult::new(
            AssertionKind::UiaTarget,
            VisualDisposition::Blocked,
            Some(fixture.to_owned()),
            detail,
        ));
    }

    fn record_uia_failure(&mut self, fixture: &str, detail: String) {
        self.saw_non_pass = true;
        self.lanes.observe_uia(VisualDisposition::Fail);
        self.assertions.push(AssertionResult::new(
            AssertionKind::UiaTarget,
            VisualDisposition::Fail,
            Some(fixture.to_owned()),
            detail,
        ));
    }

    fn record_target(
        &mut self,
        writer: &EvidenceWriter,
        replay: &super::FixtureReplay,
        target: &UiaDump,
    ) -> VisualResult<()> {
        self.uia = target.clone();
        writer.write_json_document(&format!("uia-{}.json", replay.case.fixture_name), target)?;
        self.lanes.observe_uia(VisualDisposition::Pass);
        self.assertions.push(AssertionResult::new(
            AssertionKind::UiaTarget,
            VisualDisposition::Pass,
            Some(replay.case.fixture_name.clone()),
            "owned Windows Terminal window and exact tab resolved through UIA".to_owned(),
        ));
        let disposition = if replay.case.expected_title_frames.contains(&target.tab_name) {
            VisualDisposition::Pass
        } else {
            VisualDisposition::Fail
        };
        self.saw_non_pass |= !matches!(disposition, VisualDisposition::Pass);
        self.lanes.observe_title(disposition);
        self.assertions.push(AssertionResult::new(
            AssertionKind::Title,
            disposition,
            Some(replay.case.fixture_name.clone()),
            format!(
                "expected_titles={:?}; uia_title={}",
                replay.case.expected_title_frames, target.tab_name
            ),
        ));
        Ok(())
    }

    fn record_title_animation(&mut self, fixture: &str, observed: &[String]) {
        let disposition = if observed.len() >= 3 {
            VisualDisposition::Pass
        } else {
            VisualDisposition::Fail
        };
        self.saw_non_pass |= !matches!(disposition, VisualDisposition::Pass);
        self.lanes.observe_animation(disposition);
        self.assertions.push(AssertionResult::new(
            AssertionKind::Animation,
            disposition,
            Some(fixture.to_owned()),
            format!(
                "distinct_title_frames={}; required=3; observed={observed:?}",
                observed.len()
            ),
        ));
    }

    fn record_capture_blocked(&mut self, fixture: &str, detail: impl Into<String>) {
        self.saw_non_pass = true;
        self.preflight = capture_blocked_preflight();
        self.lanes.observe_capture(VisualDisposition::Blocked);
        self.assertions
            .push(capture_blocked_assertion(fixture, &detail.into()));
    }

    fn record_capture_pass(&mut self, fixture: &str, backend: &str) {
        self.lanes.observe_capture(VisualDisposition::Pass);
        self.assertions.push(AssertionResult::new(
            AssertionKind::Capture,
            VisualDisposition::Pass,
            Some(fixture.to_owned()),
            backend.to_owned(),
        ));
    }

    fn record_color(&mut self, replay: &super::FixtureReplay, roi: Roi, metrics: ColorMetrics) {
        let is_ready_baseline = matches!(replay.case.expected_color, ColorSemantic::Default)
            && replay.case.fixture_name == "ready";
        let disposition = if is_ready_baseline {
            self.baseline = Some(metrics.clone());
            VisualDisposition::Pass
        } else {
            evaluate_color(
                replay.case.expected_color,
                replay.case.theme,
                &metrics,
                self.baseline.as_ref(),
                ColorTolerance::default(),
            )
        };
        self.saw_non_pass |= !matches!(disposition, VisualDisposition::Pass);
        self.lanes.observe_color(disposition);
        self.assertions.push(AssertionResult::new(
            AssertionKind::Color,
            disposition,
            Some(replay.case.fixture_name.clone()),
            format!("roi={roi:?}; metrics={metrics:?}"),
        ));
        self.metrics
            .push((replay.case.fixture_name.clone(), metrics));
    }

    fn record_animation(
        &mut self,
        fixture: &str,
        outcome: super::AnimationOutcome,
        deltas: &[super::FrameDeltaMetrics],
    ) {
        let disposition = match outcome {
            super::AnimationOutcome::AnimationPresent => VisualDisposition::Pass,
            super::AnimationOutcome::AnimationAbsent => VisualDisposition::Fail,
            super::AnimationOutcome::UnprovenCapture => VisualDisposition::Unproven,
            super::AnimationOutcome::BlockedEnvironment => VisualDisposition::Blocked,
        };
        self.saw_non_pass |= !matches!(disposition, VisualDisposition::Pass);
        self.lanes.observe_animation(disposition);
        self.assertions.push(AssertionResult::new(
            AssertionKind::Animation,
            disposition,
            Some(fixture.to_owned()),
            format!("outcome={outcome:?}; deltas={deltas:?}"),
        ));
    }

    fn finalize_preflight(&mut self, base_probe: PreflightProbe) {
        self.preflight = match self.lanes.capture {
            Some(VisualDisposition::Pass) if !self.lanes.has_blocker() => {
                DesktopPreflight::assess(PreflightProbe {
                    capture: Availability::Available,
                    ..base_probe
                })
            }
            Some(VisualDisposition::Blocked) => capture_blocked_preflight(),
            _ => self.preflight.clone(),
        };
        self.lanes.preflight = Some(self.preflight.disposition);
        if let Some(assertion) = self
            .assertions
            .iter_mut()
            .find(|assertion| matches!(assertion.kind, AssertionKind::Preflight))
        {
            *assertion = AssertionResult::new(
                AssertionKind::Preflight,
                self.preflight.disposition,
                None,
                self.preflight.detail.clone(),
            );
        }
    }

    fn disposition(&self, exact_head: bool) -> VisualDisposition {
        if !exact_head || self.lanes.has_failure() {
            VisualDisposition::Fail
        } else if self.lanes.has_blocker() {
            VisualDisposition::Blocked
        } else if self.saw_non_pass || self.lanes.has_unproven() {
            VisualDisposition::Unproven
        } else {
            VisualDisposition::Pass
        }
    }
}

fn selected_replays(
    driver: &FixtureDriver,
    request: &LiveVisualRunRequest,
) -> VisualResult<Vec<super::FixtureReplay>> {
    let all = driver.all_cases(&request.run_id)?;
    match request.fixture_name.as_deref() {
        Some(ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME) => {
            let ready = all
                .into_iter()
                .find(|replay| replay.case.fixture_name == "ready")
                .ok_or_else(|| {
                    VisualError::Platform(
                        "root-workspace visual fixture requires the ready color baseline"
                            .to_owned(),
                    )
                })?;
            Ok(vec![
                ready,
                driver.root_workspace_anchor_replay(&request.run_id)?,
            ])
        }
        Some(name) => all
            .into_iter()
            .filter(|replay| replay.case.fixture_name == name)
            .collect::<Vec<_>>()
            .into_iter()
            .next()
            .map_or_else(
                || {
                    Err(VisualError::Platform(format!(
                        "unknown G02 fixture requested for visual run: {name}"
                    )))
                },
                |replay| Ok(vec![replay]),
            ),
        None => Ok(all),
    }
}

fn capture_frames(
    backend: &impl CaptureBackend,
    target: &OwnedWindowCaptureTarget,
    count: usize,
) -> VisualResult<Vec<RgbaFrame>> {
    let mut frames = Vec::with_capacity(count);
    for index in 0..count {
        frames.push(backend.capture(target)?);
        if index + 1 < count {
            thread::sleep(Duration::from_millis(300));
        }
    }
    Ok(frames)
}

fn relative_roi(window: ScreenRect, tab: ScreenRect, frame: &RgbaFrame) -> VisualResult<Roi> {
    let relative_left = tab
        .left
        .checked_sub(window.left)
        .ok_or(VisualError::InvalidRoi)?;
    let relative_top = tab
        .top
        .checked_sub(window.top)
        .ok_or(VisualError::InvalidRoi)?;
    let x = scale_to_frame(
        u32::try_from(relative_left).map_err(|_| VisualError::InvalidRoi)?,
        frame.width(),
        window.width,
    )
    .ok_or(VisualError::InvalidRoi)?;
    let y = scale_to_frame(
        u32::try_from(relative_top).map_err(|_| VisualError::InvalidRoi)?,
        frame.height(),
        window.height,
    )
    .ok_or(VisualError::InvalidRoi)?;
    let width =
        scale_to_frame(tab.width, frame.width(), window.width).ok_or(VisualError::InvalidRoi)?;
    let height =
        scale_to_frame(tab.height, frame.height(), window.height).ok_or(VisualError::InvalidRoi)?;
    Roi::new(x, y, width, height)
        .clip(frame.width(), frame.height())
        .ok_or(VisualError::InvalidRoi)
}

fn scale_to_frame(value: u32, frame_extent: u32, uia_extent: u32) -> Option<u32> {
    let denominator = u64::from(uia_extent);
    if denominator == 0 {
        return None;
    }
    let scaled = u64::from(value)
        .checked_mul(u64::from(frame_extent))?
        .checked_add(denominator / 2)?
        .checked_div(denominator)?;
    u32::try_from(scaled).ok()
}

fn progress_roi(tab: Roi) -> VisualResult<Roi> {
    let size = tab.height.clamp(16, 64) / 2;
    let x = tab.x.saturating_add(tab.height / 2);
    let y = tab.y.saturating_add(tab.height.saturating_sub(size) / 2);
    let width = size;
    let height = size;
    (width > 0 && height > 0)
        .then_some(Roi::new(x, y, width, height))
        .ok_or(VisualError::InvalidRoi)
}

fn evaluate_color(
    expected: ColorSemantic,
    theme: crate::settings::PresentationTheme,
    observed: &ColorMetrics,
    baseline: Option<&ColorMetrics>,
    tolerance: ColorTolerance,
) -> VisualDisposition {
    match expected {
        ColorSemantic::Default => baseline.map_or(VisualDisposition::Unproven, |baseline| {
            if matches_baseline(observed, baseline, tolerance) {
                VisualDisposition::Pass
            } else {
                VisualDisposition::Fail
            }
        }),
        ColorSemantic::Approval | ColorSemantic::Question => {
            match classify_color_for_theme(observed, tolerance, theme) {
                ColorClassification::Match(ColorSemantic::Approval | ColorSemantic::Question)
                | ColorClassification::Ambiguous(_) => VisualDisposition::Pass,
                ColorClassification::ContaminatedRoi | ColorClassification::Unclassified => {
                    VisualDisposition::Fail
                }
                ColorClassification::Match(_) => VisualDisposition::Fail,
            }
        }
        semantic => {
            if classify_color_for_theme(observed, tolerance, theme)
                == ColorClassification::Match(semantic)
            {
                VisualDisposition::Pass
            } else {
                VisualDisposition::Fail
            }
        }
    }
}

fn capture_blocked_preflight() -> DesktopPreflight {
    DesktopPreflight::assess(PreflightProbe {
        session: SessionKind::Interactive,
        desktop: Availability::Available,
        terminal: Availability::Available,
        uia: Availability::Available,
        capture: Availability::Unavailable,
    })
}

fn capture_blocked_assertion(fixture: &str, detail: &str) -> AssertionResult {
    AssertionResult::new(
        AssertionKind::Capture,
        VisualDisposition::Blocked,
        Some(fixture.to_owned()),
        detail.to_owned(),
    )
}

fn empty_uia_dump() -> UiaDump {
    UiaDump {
        window_name: String::new(),
        tab_name: String::new(),
        window_bounds: None,
        tab_bounds: None,
        native_window_handle: None,
        native_window_id: None,
        window_has_keyboard_focus: None,
        activation: None,
        detail: "no owned UIA target was resolved".to_owned(),
    }
}

fn checked_out_head() -> VisualResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(VisualError::Io)?;
    if !output.status.success() {
        return Err(VisualError::Platform(
            "git rev-parse HEAD failed in visual harness".to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn inspect_environment() -> (MachineEnvironment, PreflightProbe) {
    let (session_id, session_kind) = process_session();
    let terminal_available = Command::new("wt.exe").arg("--version").output().is_ok();
    let uia_available = WindowsUiaLocator::is_available();
    let desktop_available = matches!(session_kind, SessionKind::Interactive) && uia_available;
    let desktop_geometry = WindowsUiaLocator::desktop_geometry();
    let dpi = reported_dpi();
    let environment = MachineEnvironment {
        machine: env::var("COMPUTERNAME").unwrap_or_else(|_| "UNAVAILABLE".to_owned()),
        windows_version: powershell_value(
            "(Get-CimInstance Win32_OperatingSystem | ForEach-Object { $_.Caption + ' ' + $_.Version + ' build ' + $_.BuildNumber })",
        ),
        terminal_version: powershell_value(
            "(Get-AppxPackage -Name Microsoft.WindowsTerminal | Select-Object -First 1 -ExpandProperty Version)",
        ),
        session_id,
        session_kind: format!("{session_kind:?}"),
        desktop: if desktop_available {
            "UIA desktop root available".to_owned()
        } else {
            "UIA desktop root unavailable or noninteractive session".to_owned()
        },
        dpi_scaling: dpi.to_string(),
        display_geometry: desktop_geometry,
        rust_toolchain: command_value("rustc", &["--version"]),
    };
    let probe = PreflightProbe {
        session: session_kind,
        desktop: availability(desktop_available),
        terminal: availability(terminal_available),
        uia: availability(uia_available),
        capture: Availability::Unknown,
    };
    (environment, probe)
}

fn reported_dpi() -> u32 {
    powershell_value(
        "(Get-ItemProperty 'HKCU:\\Control Panel\\Desktop\\WindowMetrics' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty AppliedDPI)",
    )
    .parse::<u32>()
    .ok()
    .filter(|dpi| *dpi >= 96)
    .unwrap_or(96)
}

fn process_session() -> (String, SessionKind) {
    let process_id = process::id().to_string();
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {process_id}"), "/FO", "CSV", "/NH"])
        .output();
    let Ok(output) = output else {
        return ("UNAVAILABLE".to_owned(), SessionKind::Unknown);
    };
    let output = String::from_utf8_lossy(&output.stdout);
    let line = output.lines().find(|line| line.starts_with('"'));
    let Some(line) = line else {
        return ("UNAVAILABLE".to_owned(), SessionKind::Unknown);
    };
    let fields = line
        .trim()
        .trim_matches('"')
        .split("\",\"")
        .collect::<Vec<_>>();
    let session_name = fields.get(2).copied().unwrap_or_default();
    let session_id = fields.get(3).copied().unwrap_or("UNAVAILABLE").to_owned();
    let kind = if session_id == "0" {
        SessionKind::SessionZero
    } else if session_name.eq_ignore_ascii_case("services") {
        SessionKind::Service
    } else if session_id != "UNAVAILABLE" && !session_name.is_empty() {
        SessionKind::Interactive
    } else {
        SessionKind::Unknown
    };
    (session_id, kind)
}

fn powershell_value(command: &str) -> String {
    command_value(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", command],
    )
}

fn command_value(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "UNAVAILABLE".to_owned())
}

fn availability(value: bool) -> Availability {
    if value {
        Availability::Available
    } else {
        Availability::Unavailable
    }
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn is_exact_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Default)]
struct LaneDisposition {
    preflight: Option<VisualDisposition>,
    uia: Option<VisualDisposition>,
    capture: Option<VisualDisposition>,
    title: Option<VisualDisposition>,
    color: Option<VisualDisposition>,
    animation: Option<VisualDisposition>,
}

impl LaneDisposition {
    fn observe_preflight(&mut self, value: VisualDisposition) {
        merge(&mut self.preflight, value);
    }

    fn observe_uia(&mut self, value: VisualDisposition) {
        merge(&mut self.uia, value);
    }

    fn observe_capture(&mut self, value: VisualDisposition) {
        merge(&mut self.capture, value);
    }

    fn observe_title(&mut self, value: VisualDisposition) {
        merge(&mut self.title, value);
    }

    fn observe_color(&mut self, value: VisualDisposition) {
        merge(&mut self.color, value);
    }

    fn observe_animation(&mut self, value: VisualDisposition) {
        merge(&mut self.animation, value);
    }

    fn has_failure(&self) -> bool {
        self.values()
            .any(|value| matches!(value, VisualDisposition::Fail))
    }

    fn has_blocker(&self) -> bool {
        self.values()
            .any(|value| matches!(value, VisualDisposition::Blocked))
    }

    fn has_unproven(&self) -> bool {
        self.values()
            .any(|value| matches!(value, VisualDisposition::Unproven))
    }

    fn uia(&self) -> VisualDisposition {
        self.uia.unwrap_or(VisualDisposition::Unproven)
    }

    fn capture(&self) -> VisualDisposition {
        self.capture.unwrap_or(VisualDisposition::Unproven)
    }

    fn title(&self) -> VisualDisposition {
        self.title.unwrap_or(VisualDisposition::Unproven)
    }

    fn color(&self) -> VisualDisposition {
        self.color.unwrap_or(VisualDisposition::Unproven)
    }

    fn animation(&self) -> VisualDisposition {
        self.animation.unwrap_or(VisualDisposition::Unproven)
    }

    fn values(&self) -> impl Iterator<Item = VisualDisposition> {
        [
            self.preflight,
            self.uia,
            self.capture,
            self.title,
            self.color,
            self.animation,
        ]
        .into_iter()
        .flatten()
    }
}

fn merge(target: &mut Option<VisualDisposition>, incoming: VisualDisposition) {
    let current = target.unwrap_or(VisualDisposition::Pass);
    let rank = |value: VisualDisposition| match value {
        VisualDisposition::Pass => 0,
        VisualDisposition::Unproven => 1,
        VisualDisposition::Blocked => 2,
        VisualDisposition::Fail => 3,
    };
    if rank(incoming) >= rank(current) {
        *target = Some(incoming);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        process::{Command, Stdio},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    use sha2::{Digest, Sha256};

    use crate::visual::{
        EvidenceFileDigest, EvidenceIntegrity, FixtureDriver, ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME,
        Rgb,
    };

    use super::{
        BoundedWorkerOutput, LiveVisualRunRequest, RgbaFrame, Roi, ScreenRect, UiaDump,
        authorize_live_worker, clear_worker_authorization, consume_worker_authorization,
        create_worker_authorization, empty_uia_dump, evidence_integrity_matches, progress_roi,
        relative_roi, selected_replays, target_has_capturable_geometry, wait_for_bounded_worker,
    };

    #[test]
    fn tab_roi_uses_actual_capture_to_uia_window_mapping() {
        let frame = RgbaFrame::solid(1492, 966, Rgb::new(0, 0, 0)).expect("valid frame");
        let roi = relative_roi(
            ScreenRect::new(34, 40, 746, 483),
            ScreenRect::new(44, 49, 248, 32),
            &frame,
        )
        .expect("logically reported tab maps inside the physical capture");
        assert_eq!(roi, Roi::new(20, 18, 496, 64));

        let physically_reported = relative_roi(
            ScreenRect::new(67, 80, 1492, 966),
            ScreenRect::new(565, 97, 493, 64),
            &frame,
        )
        .expect("physically reported tab maps without duplicate DPI scaling");
        assert_eq!(physically_reported, Roi::new(498, 17, 493, 64));
    }

    #[test]
    fn capturable_geometry_rejects_zero_sized_or_off_window_tabs() {
        let valid = UiaDump {
            window_bounds: Some(ScreenRect::new(67, 80, 1492, 966)),
            tab_bounds: Some(ScreenRect::new(565, 97, 493, 64)),
            ..empty_uia_dump()
        };
        assert!(target_has_capturable_geometry(&valid));

        let zero_window = UiaDump {
            window_bounds: Some(ScreenRect::new(0, 0, 0, 0)),
            tab_bounds: Some(ScreenRect::new(-31_575, -31_983, 433, 64)),
            ..empty_uia_dump()
        };
        assert!(!target_has_capturable_geometry(&zero_window));

        let off_window_tab = UiaDump {
            window_bounds: Some(ScreenRect::new(0, 0, 100, 100)),
            tab_bounds: Some(ScreenRect::new(-1, 0, 50, 20)),
            ..empty_uia_dump()
        };
        assert!(!target_has_capturable_geometry(&off_window_tab));
    }

    #[test]
    fn progress_roi_selects_a_bounded_icon_square_inside_tab() {
        assert_eq!(
            progress_roi(Roi::new(20, 18, 496, 64)).expect("sufficient tab geometry"),
            Roi::new(52, 34, 32, 32)
        );
    }

    #[test]
    fn root_workspace_visual_fixture_keeps_the_ready_color_baseline() {
        let request = LiveVisualRunRequest {
            expected_head: "a".repeat(40),
            run_id: "TB59-anchor-selection".to_owned(),
            evidence_root: PathBuf::from("target/visual-worker-tests"),
            fixture_name: Some(ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME.to_owned()),
        };
        let replays = selected_replays(&FixtureDriver::default(), &request)
            .expect("G59 visual fixture selection is valid");
        let names = replays
            .iter()
            .map(|replay| replay.case.fixture_name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["ready", ROOT_WORKSPACE_ANCHOR_FIXTURE_NAME]);
    }

    #[test]
    fn bounded_worker_deadline_returns_a_non_success_classification() {
        let mut worker = Command::new("powershell.exe");
        worker
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$child = Start-Process powershell.exe -ArgumentList '-NoProfile', '-NonInteractive', '-Command', 'Start-Sleep -Seconds 10' -PassThru; Start-Sleep -Seconds 10",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let started = Instant::now();
        let outcome = wait_for_bounded_worker(
            worker.spawn().expect("starts the test-owned helper"),
            Duration::from_millis(500),
        )
        .expect("collects the terminated test-owned helper");
        assert!(
            matches!(
                outcome,
                BoundedWorkerOutput::TimedOut | BoundedWorkerOutput::TerminationFailed
            ),
            "a sleeping owned helper must never be reported as a successful completion"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the outer harness must retain an enforceable wall-clock boundary"
        );
    }

    #[test]
    fn worker_authorization_is_one_time_and_rejects_forgery() {
        let root = unique_test_root("authorization");
        let request = LiveVisualRunRequest {
            expected_head: "a".repeat(40),
            run_id: "TB03TEST-worker-authorization".to_owned(),
            evidence_root: root.clone(),
            fixture_name: Some("working".to_owned()),
        };
        let authorization = create_worker_authorization(&root, &request.run_id)
            .expect("creates owned authorization");
        let consumed =
            consume_worker_authorization(&authorization.path, &request, &authorization.nonce)
                .expect("the supervisor-issued authorization is consumable once");
        assert_eq!(consumed.run_id, request.run_id);
        assert!(
            consume_worker_authorization(&authorization.path, &request, &authorization.nonce)
                .is_err()
        );

        let forged = create_worker_authorization(&root, "TB03TEST-worker-forgery")
            .expect("creates separate owned authorization");
        assert!(consume_worker_authorization(&forged.path, &request, "wrong-nonce").is_err());
        assert!(
            authorize_live_worker(
                &request,
                &root.join("forged.authorization"),
                &authorization.nonce,
            )
            .is_err()
        );
        clear_worker_authorization(&authorization);
        clear_worker_authorization(&forged);
    }

    #[test]
    fn staged_integrity_rejects_unlisted_artifacts() {
        let directory = unique_test_root("integrity");
        fs::create_dir_all(&directory).expect("creates owned staging test directory");
        let bytes = b"owned-artifact";
        fs::write(directory.join("artifact.json"), bytes).expect("writes owned artifact");
        let integrity = EvidenceIntegrity {
            algorithm: "SHA-256".to_owned(),
            files: vec![EvidenceFileDigest {
                name: "artifact.json".to_owned(),
                bytes: u64::try_from(bytes.len()).expect("artifact length fits"),
                sha256: format!("{:x}", Sha256::digest(bytes)),
            }],
            tree_sha256: String::new(),
        };
        let integrity = EvidenceIntegrity {
            tree_sha256: super::evidence_tree_sha256(&integrity.files),
            ..integrity
        };
        assert!(evidence_integrity_matches(&directory, &integrity));
        fs::write(directory.join("unlisted.json"), b"unexpected").expect("adds test artifact");
        assert!(!evidence_integrity_matches(&directory, &integrity));
    }

    fn unique_test_root(kind: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock follows Unix epoch")
            .as_nanos();
        PathBuf::from("target")
            .join("visual-worker-tests")
            .join(format!("{kind}-{nonce}"))
    }
}
