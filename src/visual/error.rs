//! Error types for deterministic visual-test infrastructure.

use std::{error::Error, fmt, io, path::PathBuf};

/// A fallible result produced by visual-test infrastructure.
pub type VisualResult<T> = Result<T, VisualError>;

/// A classified infrastructure error that must not be mistaken for a product
/// presentation assertion.
#[derive(Debug)]
pub enum VisualError {
    /// A captured frame did not contain exactly four bytes per pixel.
    InvalidFrame {
        /// Frame width in pixels.
        width: u32,
        /// Frame height in pixels.
        height: u32,
        /// Actual byte count.
        bytes: usize,
    },
    /// A requested ROI was empty or outside its source frame.
    InvalidRoi,
    /// Frames cannot be compared because their dimensions differ.
    InconsistentFrames {
        /// Dimensions of the first frame.
        first: (u32, u32),
        /// Dimensions of the second frame.
        second: (u32, u32),
    },
    /// A run ID or artifact name is not safe for a controlled evidence path.
    InvalidIdentifier(String),
    /// An evidence directory already exists and must not be overwritten.
    EvidenceDirectoryExists(PathBuf),
    /// An individual artifact already exists and must not be overwritten.
    EvidenceArtifactExists(PathBuf),
    /// The expected, checked-out, and visual heads are not equal.
    ExactHeadMismatch {
        /// Expected candidate SHA.
        expected: String,
        /// SHA checked out by the harness.
        checked_out: String,
        /// SHA recorded by visual evidence, when available.
        visual: Option<String>,
    },
    /// A platform adapter could not satisfy a precondition.
    Platform(String),
    /// A filesystem operation failed while handling owned evidence.
    Io(io::Error),
    /// Evidence JSON could not be encoded or decoded.
    Json(serde_json::Error),
    /// PNG evidence could not be encoded.
    Png(png::EncodingError),
}

impl fmt::Display for VisualError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFrame {
                width,
                height,
                bytes,
            } => write!(
                formatter,
                "invalid RGBA frame: {width}x{height} requires four bytes per pixel, got {bytes} bytes"
            ),
            Self::InvalidRoi => formatter.write_str("ROI is empty or outside its source frame"),
            Self::InconsistentFrames { first, second } => write!(
                formatter,
                "inconsistent frame dimensions: {}x{} versus {}x{}",
                first.0, first.1, second.0, second.1
            ),
            Self::InvalidIdentifier(value) => {
                write!(formatter, "unsafe evidence identifier: {value}")
            }
            Self::EvidenceDirectoryExists(path) => {
                write!(
                    formatter,
                    "evidence directory already exists: {}",
                    path.display()
                )
            }
            Self::EvidenceArtifactExists(path) => {
                write!(
                    formatter,
                    "evidence artifact already exists: {}",
                    path.display()
                )
            }
            Self::ExactHeadMismatch {
                expected,
                checked_out,
                visual,
            } => write!(
                formatter,
                "exact-head mismatch: expected={expected} checked_out={checked_out} visual={}",
                visual.as_deref().unwrap_or("N/A")
            ),
            Self::Platform(message) => write!(formatter, "platform precondition: {message}"),
            Self::Io(error) => write!(formatter, "evidence I/O error: {error}"),
            Self::Json(error) => write!(formatter, "evidence JSON error: {error}"),
            Self::Png(error) => write!(formatter, "evidence PNG error: {error}"),
        }
    }
}

impl Error for VisualError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Png(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for VisualError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for VisualError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<png::EncodingError> for VisualError {
    fn from(error: png::EncodingError) -> Self {
        Self::Png(error)
    }
}
