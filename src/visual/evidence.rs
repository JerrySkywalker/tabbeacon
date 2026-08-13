//! Structured, owned visual-evidence bundles.

use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    ColorMetrics, DesktopPreflight, RgbaFrame, ScreenRect, VisualDisposition, VisualError,
    VisualResult,
};

/// The assertion category represented in a structured evidence result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssertionKind {
    /// Interactive desktop preflight.
    Preflight,
    /// UIA target lookup.
    UiaTarget,
    /// UIA tab-title comparison.
    Title,
    /// Target-window capture validity.
    Capture,
    /// Color ROI classification.
    Color,
    /// Indeterminate-progress frame delta.
    Animation,
    /// Exact-head identity validation.
    ExactHead,
    /// Owned-session cleanup/reset attempt.
    Cleanup,
}

/// Root failure classification required by the repository quality gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FailureCategory {
    /// The presentation behavior itself disagreed with the contract.
    ProductCodeDefect,
    /// The visual test/harness contract could not exercise its own target.
    TestDefect,
    /// Desktop, runner, UIA, or capture prerequisites were unavailable.
    RunnerEnvironmentDefect,
    /// An external service or dependency prevented observation.
    ExternalDependency,
    /// Expected, checked-out, or evidence SHA identity was inconsistent.
    EvidenceMismatch,
    /// Observation was insufficient to classify more specifically.
    Unproven,
}

/// One machine-verifiable assertion outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertionResult {
    /// Assertion class.
    pub kind: AssertionKind,
    /// Explicit evidence disposition.
    pub disposition: VisualDisposition,
    /// Failure classification when the disposition is not `PASS`.
    pub failure_category: Option<FailureCategory>,
    /// Stable fixture name, if applicable.
    pub fixture: Option<String>,
    /// Non-sensitive diagnostic detail.
    pub detail: String,
}

impl AssertionResult {
    /// Creates an assertion with deterministic repository-gate classification.
    #[must_use]
    pub fn new(
        kind: AssertionKind,
        disposition: VisualDisposition,
        fixture: Option<String>,
        detail: String,
    ) -> Self {
        Self {
            kind,
            disposition,
            failure_category: failure_category(kind, disposition),
            fixture,
            detail,
        }
    }
}

fn failure_category(
    kind: AssertionKind,
    disposition: VisualDisposition,
) -> Option<FailureCategory> {
    if matches!(disposition, VisualDisposition::Pass) {
        return None;
    }
    if matches!(disposition, VisualDisposition::Unproven) {
        return Some(FailureCategory::Unproven);
    }
    match kind {
        AssertionKind::ExactHead => Some(FailureCategory::EvidenceMismatch),
        AssertionKind::Preflight | AssertionKind::UiaTarget | AssertionKind::Capture => {
            Some(FailureCategory::RunnerEnvironmentDefect)
        }
        AssertionKind::Title | AssertionKind::Color | AssertionKind::Animation => {
            Some(FailureCategory::ProductCodeDefect)
        }
        AssertionKind::Cleanup => Some(FailureCategory::TestDefect),
    }
}

/// Sanitized environment information that may be retained in visual evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineEnvironment {
    /// Opaque machine identity (normally `COMPUTERNAME`).
    pub machine: String,
    /// Windows product/version/build string.
    pub windows_version: String,
    /// Installed Windows Terminal version, or an explicit unavailable marker.
    pub terminal_version: String,
    /// Current process session ID or an explicit unavailable marker.
    pub session_id: String,
    /// Current session classification.
    pub session_kind: String,
    /// Input desktop/window-station accessibility summary.
    pub desktop: String,
    /// Recorded system DPI/scaling summary.
    pub dpi_scaling: String,
    /// Recorded virtual display geometry.
    pub display_geometry: Option<ScreenRect>,
    /// Rust toolchain reported by the harness.
    pub rust_toolchain: String,
}

/// Compact UIA data from the positively owned Windows Terminal test target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiaDump {
    /// Owned window accessible name.
    pub window_name: String,
    /// UIA tab accessible name used for the title assertion.
    pub tab_name: String,
    /// Owned window bounds when UIA supplied them.
    pub window_bounds: Option<ScreenRect>,
    /// Target tab bounds when UIA supplied them.
    pub tab_bounds: Option<ScreenRect>,
    /// Native window handle rendered as a diagnostic string, if available.
    pub native_window_handle: Option<String>,
    /// Whether UIA reported keyboard focus on the top-level owned window when
    /// it was resolved. Windows Terminal can retain focus in a child element,
    /// so this is diagnostic only; capture relies on the recorded successful
    /// owned-window foreground activation.
    pub window_has_keyboard_focus: Option<bool>,
    /// Result of the narrowly-scoped owned-window activation attempted before
    /// visibility-dependent capture. `None` means no activation was attempted.
    pub activation: Option<WindowActivation>,
    /// Compact diagnostic notes; never a full desktop traversal.
    pub detail: String,
}

/// Auditable result of activating only the UIA-correlated fixture window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowActivation {
    /// Whether Windows accepted the foreground request for the owned window.
    pub set_foreground: bool,
    /// Whether UI Automation accepted the focus request for the owned window.
    pub set_focus: bool,
}

/// Exact-head and environment identity for one visual evidence directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceManifest {
    /// Governed goal ID.
    pub goal_id: String,
    /// Candidate SHA supplied to the harness.
    pub expected_head: String,
    /// SHA observed by the running checkout before test code executes.
    pub checked_out_head: String,
    /// SHA eligible to support visual PASS; absent when visual observation did
    /// not execute successfully.
    pub visual_head: Option<String>,
    /// Unique, safe test-run identifier.
    pub run_id: String,
    /// UTC Unix seconds recorded by the caller.
    pub observed_at_unix_seconds: u64,
    /// Capture backend name.
    pub capture_backend: String,
    /// Interactive desktop preflight.
    pub preflight: DesktopPreflight,
    /// Sanitized machine/runtime environment.
    pub environment: MachineEnvironment,
    /// Fixed/recorded owned Terminal window geometry.
    pub window_geometry: Option<ScreenRect>,
    /// G02 fixture names represented by this evidence directory.
    pub fixtures: Vec<String>,
    /// Overall run disposition.
    pub disposition: VisualDisposition,
}

impl EvidenceManifest {
    /// Validates the exact-head invariant needed before visual PASS.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::ExactHeadMismatch`] unless expected, checked-out,
    /// and visual heads are all present and exactly equal.
    pub fn validate_exact_heads_for_pass(&self) -> VisualResult<()> {
        let visual_matches = self
            .visual_head
            .as_deref()
            .is_some_and(|visual| visual == self.expected_head);
        if is_exact_sha(&self.expected_head)
            && is_exact_sha(&self.checked_out_head)
            && self.visual_head.as_deref().is_some_and(is_exact_sha)
            && self.checked_out_head == self.expected_head
            && visual_matches
        {
            Ok(())
        } else {
            Err(VisualError::ExactHeadMismatch {
                expected: self.expected_head.clone(),
                checked_out: self.checked_out_head.clone(),
                visual: self.visual_head.clone(),
            })
        }
    }
}

/// Complete structured evidence written as four deterministic JSON documents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceBundle {
    /// Run identity and preflight context.
    pub manifest: EvidenceManifest,
    /// Machine assertion outcomes.
    pub assertions: Vec<AssertionResult>,
    /// Environment record duplicated as a standalone review file.
    pub environment: MachineEnvironment,
    /// Compact target-only UIA dump.
    pub uia: UiaDump,
    /// Color metrics keyed by stable fixture name.
    pub color_metrics: Vec<(String, ColorMetrics)>,
}

/// SHA-256 digest for one owned evidence artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFileDigest {
    /// Flat, safe filename relative to the evidence directory.
    pub name: String,
    /// Exact artifact byte count.
    pub bytes: u64,
    /// Lowercase SHA-256 hex digest of the artifact bytes.
    pub sha256: String,
}

/// Deterministic integrity record for an evidence directory.
///
/// `integrity.json` is deliberately excluded from `files`: it contains the
/// digest record itself. `tree_sha256` is SHA-256 over sorted
/// `name`, byte-count, and content-digest records separated by NUL bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceIntegrity {
    /// Digest algorithm used for every entry and the tree record.
    pub algorithm: String,
    /// Every pre-existing regular artifact in deterministic filename order.
    pub files: Vec<EvidenceFileDigest>,
    /// Lowercase SHA-256 of the deterministic artifact-record sequence.
    pub tree_sha256: String,
}

impl EvidenceBundle {
    /// Validates that a claimed PASS has both exact heads and PASS assertions.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::ExactHeadMismatch`] when a PASS lacks the
    /// required exact-head identity, or [`VisualError::Platform`] when a PASS
    /// contains a non-PASS assertion.
    pub fn validate(&self) -> VisualResult<()> {
        if matches!(self.manifest.disposition, VisualDisposition::Pass) {
            self.manifest.validate_exact_heads_for_pass()?;
            if self
                .assertions
                .iter()
                .any(|assertion| !matches!(assertion.disposition, VisualDisposition::Pass))
            {
                return Err(VisualError::Platform(
                    "PASS evidence contains a non-PASS assertion".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

/// Writes only the dedicated evidence directory for one positively owned run.
#[derive(Debug)]
pub struct EvidenceWriter {
    directory: PathBuf,
}

impl EvidenceWriter {
    /// Creates a fresh, empty run directory without overwriting prior evidence.
    ///
    /// # Errors
    ///
    /// Returns [`VisualError::InvalidIdentifier`] for unsafe run IDs,
    /// [`VisualError::EvidenceDirectoryExists`] when the exact target already
    /// exists, or [`VisualError::Io`] for filesystem failures.
    pub fn create(root: impl AsRef<Path>, run_id: &str) -> VisualResult<Self> {
        if !is_safe_component(run_id) {
            return Err(VisualError::InvalidIdentifier(run_id.to_owned()));
        }
        let root = root.as_ref();
        fs::create_dir_all(root)?;
        let directory = root.join(run_id);
        match fs::create_dir(&directory) {
            Ok(()) => Ok(Self { directory }),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(VisualError::EvidenceDirectoryExists(directory))
            }
            Err(error) => Err(VisualError::Io(error)),
        }
    }

    /// Returns the owned evidence directory.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// Writes the four JSON documents required for one evidence bundle.
    ///
    /// # Errors
    ///
    /// Returns JSON or filesystem errors from writing the owned directory.
    pub fn write_bundle(&self, bundle: &EvidenceBundle) -> VisualResult<()> {
        bundle.validate()?;
        self.write_json("manifest.json", &bundle.manifest)?;
        self.write_json("assertions.json", &bundle.assertions)?;
        self.write_json("environment.json", &bundle.environment)?;
        self.write_json("uia.json", &bundle.uia)?;
        self.write_json("color-metrics.json", &bundle.color_metrics)
    }

    /// Writes the final deterministic integrity record for all prior artifacts.
    ///
    /// Call this only after every evidence PNG and JSON artifact is written.
    /// The generated `integrity.json` is intentionally excluded from its own
    /// tree to avoid a self-referential digest.
    ///
    /// # Errors
    ///
    /// Returns an error if the owned directory contains anything other than
    /// regular, safe, flat files or if `integrity.json` already exists.
    pub fn write_integrity_manifest(&self) -> VisualResult<EvidenceIntegrity> {
        let integrity_path = self.directory.join("integrity.json");
        if integrity_path.exists() {
            return Err(VisualError::EvidenceArtifactExists(integrity_path));
        }

        let mut files = Vec::new();
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if !file_type.is_file() {
                return Err(VisualError::Platform(
                    "owned evidence directory contains a non-file artifact".to_owned(),
                ));
            }
            let name = entry.file_name().into_string().map_err(|_| {
                VisualError::InvalidIdentifier("non-Unicode evidence filename".to_owned())
            })?;
            if !is_safe_component(&name) || name == "integrity.json" {
                return Err(VisualError::InvalidIdentifier(name));
            }
            let bytes = fs::read(entry.path())?;
            files.push(EvidenceFileDigest {
                name,
                bytes: u64::try_from(bytes.len()).map_err(|_| {
                    VisualError::Platform("evidence file length exceeds u64".to_owned())
                })?,
                sha256: hex_sha256(&bytes),
            });
        }
        files.sort_unstable_by(|left, right| left.name.cmp(&right.name));

        let mut tree = Sha256::new();
        for file in &files {
            tree.update(file.name.as_bytes());
            tree.update([0]);
            tree.update(file.bytes.to_string().as_bytes());
            tree.update([0]);
            tree.update(file.sha256.as_bytes());
            tree.update([b'\n']);
        }
        let integrity = EvidenceIntegrity {
            algorithm: "SHA-256".to_owned(),
            files,
            tree_sha256: format!("{:x}", tree.finalize()),
        };
        self.write_json("integrity.json", &integrity)?;
        Ok(integrity)
    }

    /// Writes a named JSON diagnostic within this owned evidence directory.
    ///
    /// # Errors
    ///
    /// Returns an identifier, serialization, or filesystem error without
    /// writing outside the owned directory or overwriting an existing artifact.
    pub fn write_json_document<T: Serialize>(&self, name: &str, value: &T) -> VisualResult<()> {
        self.write_json(name, value)
    }

    /// Writes a lossless RGBA PNG under a validated evidence artifact name.
    ///
    /// # Errors
    ///
    /// Returns an identifier, PNG, or filesystem error without writing outside
    /// the owned evidence directory.
    pub fn write_png(&self, name: &str, frame: &RgbaFrame) -> VisualResult<PathBuf> {
        let path = self.artifact_path(name, "png")?;
        let file = Self::create_new_file(&path)?;
        let writer = BufWriter::new(file);
        let mut encoder = png::Encoder::new(writer, frame.width(), frame.height());
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut png_writer = encoder.write_header()?;
        png_writer.write_image_data(frame.pixels())?;
        png_writer.finish()?;
        Ok(path)
    }

    fn write_json<T: Serialize>(&self, name: &str, value: &T) -> VisualResult<()> {
        let path = self.artifact_path(name, "json")?;
        let bytes = serde_json::to_vec_pretty(value)?;
        let mut file = Self::create_new_file(&path)?;
        file.write_all(&bytes)?;
        Ok(())
    }

    fn create_new_file(path: &Path) -> VisualResult<File> {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
        {
            Ok(file) => Ok(file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(VisualError::EvidenceArtifactExists(path.to_path_buf()))
            }
            Err(error) => Err(VisualError::Io(error)),
        }
    }

    fn artifact_path(&self, name: &str, extension: &str) -> VisualResult<PathBuf> {
        let path = Path::new(name);
        let is_json_name = extension == "json"
            && Path::new(name)
                .extension()
                .is_some_and(|value| value.eq_ignore_ascii_case("json"));
        if !is_safe_component(name)
            || path.components().count() != 1
            || (!is_json_name && name.contains('.'))
        {
            return Err(VisualError::InvalidIdentifier(name.to_owned()));
        }
        let filename = if is_json_name {
            name.to_owned()
        } else {
            format!("{name}.{extension}")
        };
        Ok(self.directory.join(filename))
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn is_safe_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

fn is_exact_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
