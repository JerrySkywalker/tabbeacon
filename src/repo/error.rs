use std::{fmt, io, path::PathBuf};

/// Failure from offline repository identity resolution.
#[derive(Debug)]
pub enum RepositoryIdentityError {
    /// The configured Git executable could not start or a local Git operation
    /// failed.
    Git {
        /// Fixed local-only operation name.
        operation: &'static str,
        /// Exit status or process-start detail with control characters removed.
        detail: String,
    },
    /// The supplied cwd is not inside a discoverable Git worktree.
    NotRepository(PathBuf),
    /// Local Git returned malformed or internally inconsistent metadata.
    InvalidGitMetadata(String),
    /// A canonical identity, display name, or alias violated its type contract.
    InvalidIdentifier {
        /// Identifier category.
        kind: &'static str,
        /// Non-sensitive reason.
        detail: String,
    },
    /// No appropriate per-user application-data location was available.
    StateRootUnavailable,
    /// Published registry files existed but no safe state could be recovered.
    CorruptRegistry(String),
    /// Deterministic alias candidates were exhausted.
    AliasExhausted,
    /// A filesystem operation failed.
    Io(io::Error),
    /// Registry serialization or parsing failed.
    Json(serde_json::Error),
}

impl fmt::Display for RepositoryIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Git { operation, detail } => {
                write!(
                    formatter,
                    "local Git operation {operation} failed: {detail}"
                )
            }
            Self::NotRepository(path) => {
                write!(formatter, "not inside a Git repository: {}", path.display())
            }
            Self::InvalidGitMetadata(detail) => {
                write!(formatter, "invalid local Git metadata: {detail}")
            }
            Self::InvalidIdentifier { kind, detail } => {
                write!(formatter, "invalid {kind}: {detail}")
            }
            Self::StateRootUnavailable => {
                formatter.write_str("TabBeacon per-user state root is unavailable")
            }
            Self::CorruptRegistry(detail) => write!(formatter, "corrupt alias registry: {detail}"),
            Self::AliasExhausted => formatter.write_str("stable alias candidates were exhausted"),
            Self::Io(error) => write!(formatter, "repository identity I/O failed: {error}"),
            Self::Json(error) => write!(formatter, "repository identity JSON failed: {error}"),
        }
    }
}

impl std::error::Error for RepositoryIdentityError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for RepositoryIdentityError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for RepositoryIdentityError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub(crate) fn sanitized_detail(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let trimmed = sanitized.trim();
    if trimmed.is_empty() {
        "no diagnostic output".to_owned()
    } else {
        trimmed.chars().take(512).collect()
    }
}
