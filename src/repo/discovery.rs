use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use super::error::{RepositoryIdentityError, sanitized_detail};

const LAYOUT_ARGS: &[&str] = &[
    "rev-parse",
    "--path-format=absolute",
    "--show-toplevel",
    "--absolute-git-dir",
    "--git-common-dir",
];
const REMOTE_ARGS: &[&str] = &[
    "config",
    "--local",
    "--null",
    "--get-regexp",
    "^remote\\..*\\.url$",
];
const ROOT_COMMIT_ARGS: &[&str] = &["rev-list", "--max-parents=0", "--all"];

/// One locally configured Git remote URL.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepositoryRemote {
    name: String,
    url: String,
}

impl RepositoryRemote {
    /// Returns the local remote name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the configured URL exactly as local Git reported it.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }
}

/// Local Git evidence shared with canonicalization.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredRepository {
    /// Root of the current worktree.
    pub worktree_root: PathBuf,
    /// Git directory for this worktree.
    pub git_dir: PathBuf,
    /// Common Git directory shared by linked worktrees.
    pub git_common_dir: PathBuf,
    /// Locally configured remote URLs in deterministic order.
    pub remotes: Vec<RepositoryRemote>,
    /// Root commits reachable from local refs, sorted and deduplicated.
    pub root_commits: Vec<String>,
}

/// Runs a closed set of local-only Git metadata operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryDiscovery {
    git_executable: PathBuf,
}

impl Default for RepositoryDiscovery {
    fn default() -> Self {
        Self {
            git_executable: PathBuf::from("git"),
        }
    }
}

impl RepositoryDiscovery {
    /// Uses an explicit Git executable, primarily for hermetic callers/tests.
    #[must_use]
    pub fn with_git_executable(git_executable: impl Into<PathBuf>) -> Self {
        Self {
            git_executable: git_executable.into(),
        }
    }

    /// Discovers the ordinary or linked worktree containing `cwd`.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryIdentityError::NotRepository`] when layout discovery
    /// fails, or a typed metadata/I/O error for malformed local evidence.
    pub fn discover(
        &self,
        cwd: impl AsRef<Path>,
    ) -> Result<DiscoveredRepository, RepositoryIdentityError> {
        let cwd = cwd.as_ref();
        let layout = self
            .run(cwd, "layout", LAYOUT_ARGS, false)
            .map_err(|error| {
                if matches!(error, RepositoryIdentityError::Git { .. }) {
                    RepositoryIdentityError::NotRepository(cwd.to_path_buf())
                } else {
                    error
                }
            })?;
        let layout = decode_utf8(layout.stdout, "layout")?;
        let mut lines = layout.lines();
        let worktree_root = required_absolute_path(lines.next(), "worktree root")?;
        let git_dir = required_absolute_path(lines.next(), "Git directory")?;
        let git_common_dir = required_absolute_path(lines.next(), "Git common directory")?;
        if lines.next().is_some() {
            return Err(RepositoryIdentityError::InvalidGitMetadata(
                "layout operation returned unexpected extra lines".to_owned(),
            ));
        }

        let remote_output = self.run(cwd, "remotes", REMOTE_ARGS, true)?;
        let remotes = if remote_output.status.success() {
            parse_remotes(&remote_output.stdout)?
        } else {
            Vec::new()
        };
        let roots_output = self.run(cwd, "root-commits", ROOT_COMMIT_ARGS, false)?;
        let roots = decode_utf8(roots_output.stdout, "root commits")?;
        let mut root_commits = roots
            .lines()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if root_commits.iter().any(|value| !is_object_id(value)) {
            return Err(RepositoryIdentityError::InvalidGitMetadata(
                "root commit output contained a non-hex object ID".to_owned(),
            ));
        }
        root_commits.sort_unstable();
        root_commits.dedup();

        Ok(DiscoveredRepository {
            worktree_root,
            git_dir,
            git_common_dir,
            remotes,
            root_commits,
        })
    }

    fn run(
        &self,
        cwd: &Path,
        operation: &'static str,
        args: &[&str],
        allow_no_match: bool,
    ) -> Result<Output, RepositoryIdentityError> {
        let output = Command::new(&self.git_executable)
            .arg("-C")
            .arg(cwd)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "Never")
            .output()
            .map_err(|error| RepositoryIdentityError::Git {
                operation,
                detail: sanitized_detail(&error.to_string()),
            })?;
        if output.status.success() || (allow_no_match && output.status.code() == Some(1)) {
            Ok(output)
        } else {
            Err(RepositoryIdentityError::Git {
                operation,
                detail: sanitized_detail(&String::from_utf8_lossy(&output.stderr)),
            })
        }
    }
}

fn required_absolute_path(
    value: Option<&str>,
    kind: &'static str,
) -> Result<PathBuf, RepositoryIdentityError> {
    let value = value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RepositoryIdentityError::InvalidGitMetadata(format!("missing {kind}")))?;
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(RepositoryIdentityError::InvalidGitMetadata(format!(
            "{kind} was not absolute"
        )))
    }
}

fn parse_remotes(bytes: &[u8]) -> Result<Vec<RepositoryRemote>, RepositoryIdentityError> {
    let text = std::str::from_utf8(bytes).map_err(|_| {
        RepositoryIdentityError::InvalidGitMetadata("remote configuration was not UTF-8".to_owned())
    })?;
    let mut grouped = BTreeMap::<String, Vec<String>>::new();
    for record in text.split('\0').filter(|record| !record.is_empty()) {
        let (key, url) = record.split_once('\n').ok_or_else(|| {
            RepositoryIdentityError::InvalidGitMetadata(
                "remote configuration record lacked a key/value boundary".to_owned(),
            )
        })?;
        let lower = key.to_ascii_lowercase();
        let name = lower
            .strip_prefix("remote.")
            .and_then(|value| value.strip_suffix(".url"))
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                RepositoryIdentityError::InvalidGitMetadata(
                    "remote configuration contained an invalid key".to_owned(),
                )
            })?;
        if !url.trim().is_empty() {
            grouped
                .entry(name.to_owned())
                .or_default()
                .push(url.trim().to_owned());
        }
    }
    let mut remotes = grouped
        .into_iter()
        .flat_map(|(name, mut urls)| {
            urls.sort_unstable();
            urls.dedup();
            urls.into_iter().map(move |url| RepositoryRemote {
                name: name.clone(),
                url,
            })
        })
        .collect::<Vec<_>>();
    remotes.sort_unstable();
    Ok(remotes)
}

fn decode_utf8(bytes: Vec<u8>, kind: &str) -> Result<String, RepositoryIdentityError> {
    String::from_utf8(bytes).map_err(|_| {
        RepositoryIdentityError::InvalidGitMetadata(format!("{kind} output was not UTF-8"))
    })
}

fn is_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::{LAYOUT_ARGS, REMOTE_ARGS, ROOT_COMMIT_ARGS, parse_remotes};

    #[test]
    fn admitted_git_operations_are_local_only() {
        for args in [LAYOUT_ARGS, REMOTE_ARGS, ROOT_COMMIT_ARGS] {
            let joined = args.join(" ").to_ascii_lowercase();
            for forbidden in [
                "ls-remote",
                "fetch",
                "pull",
                "push",
                "clone",
                "github",
                "http://",
                "https://",
            ] {
                assert!(
                    !joined.contains(forbidden),
                    "forbidden Git operation: {joined}"
                );
            }
        }
    }

    #[test]
    fn remote_records_are_sorted_and_deduplicated() {
        let parsed = parse_remotes(
            b"remote.zeta.url\nssh://host/z/repo.git\0remote.origin.url\nhttps://host/o/repo.git\0remote.origin.url\nhttps://host/o/repo.git\0",
        )
        .expect("valid local remote records");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name(), "origin");
        assert_eq!(parsed[1].name(), "zeta");
    }
}
