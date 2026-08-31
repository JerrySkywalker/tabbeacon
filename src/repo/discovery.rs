use std::{
    collections::BTreeMap,
    env, fs,
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
const MAX_GIT_CONTROL_FILE_BYTES: u64 = 1024 * 1024;

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
        let mut discovered = self.discover_without_root_commits(cwd.as_ref())?;
        discovered.root_commits = self.discover_root_commits(cwd.as_ref())?;
        Ok(discovered)
    }

    /// Discovers the worktree layout and local remotes without walking history.
    ///
    /// Callers that establish a remote-backed identity can safely avoid the
    /// root-history query; callers needing a local-history fallback must call
    /// [`Self::discover_root_commits`] before canonicalization.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryIdentityError::NotRepository`] when `cwd` is not a
    /// Git worktree, or a typed metadata/I/O error for malformed local layout
    /// or remote evidence.
    pub fn discover_without_root_commits(
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

        Ok(DiscoveredRepository {
            worktree_root,
            git_dir,
            git_common_dir,
            remotes,
            root_commits: Vec::new(),
        })
    }

    /// Discovers ordinary worktree layout and local remote URLs without
    /// launching Git when the repository uses complete, direct metadata.
    ///
    /// Codex command Hooks have a strict one-second lifecycle. On Windows,
    /// launching `git.exe` twice for layout and remote configuration can
    /// consume that entire budget before presentation begins. A conventional
    /// `.git` directory or gitfile plus a direct local config is already the
    /// same offline authority those commands read, so use it in-process. Rare
    /// config/include layouts fall back to the established Git implementation.
    pub(super) fn discover_without_root_commits_for_hook(
        &self,
        cwd: impl AsRef<Path>,
    ) -> Result<DiscoveredRepository, RepositoryIdentityError> {
        let cwd = cwd.as_ref();
        match discover_process_free(cwd) {
            ProcessFreeDiscovery::Repository(discovered) => Ok(discovered),
            ProcessFreeDiscovery::NotRepository => {
                Err(RepositoryIdentityError::NotRepository(cwd.to_path_buf()))
            }
            ProcessFreeDiscovery::RequiresGit => self.discover_without_root_commits(cwd),
        }
    }

    /// Reads the local root-history fallback only when no usable remote
    /// identity was established.
    ///
    /// # Errors
    ///
    /// Returns a typed Git, UTF-8, or metadata error when the local root
    /// history cannot be read or contains an invalid object ID.
    pub fn discover_root_commits(
        &self,
        cwd: impl AsRef<Path>,
    ) -> Result<Vec<String>, RepositoryIdentityError> {
        let cwd = cwd.as_ref();
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
        Ok(root_commits)
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

enum ProcessFreeDiscovery {
    Repository(DiscoveredRepository),
    NotRepository,
    RequiresGit,
}

struct ProcessFreeLayout {
    worktree_root: PathBuf,
    git_dir: PathBuf,
    git_common_dir: PathBuf,
}

fn discover_process_free(cwd: &Path) -> ProcessFreeDiscovery {
    let Some(layout) = process_free_layout(cwd) else {
        return ProcessFreeDiscovery::NotRepository;
    };
    let Ok(layout) = layout else {
        return ProcessFreeDiscovery::RequiresGit;
    };
    let Ok(remotes) = process_free_remotes(&layout) else {
        return ProcessFreeDiscovery::RequiresGit;
    };
    ProcessFreeDiscovery::Repository(DiscoveredRepository {
        worktree_root: layout.worktree_root,
        git_dir: layout.git_dir,
        git_common_dir: layout.git_common_dir,
        remotes,
        root_commits: Vec::new(),
    })
}

fn process_free_layout(cwd: &Path) -> Option<Result<ProcessFreeLayout, ()>> {
    let mut candidate = if cwd.is_absolute() {
        cwd.to_path_buf()
    } else {
        let Ok(current) = env::current_dir() else {
            return Some(Err(()));
        };
        current.join(cwd)
    };
    if !candidate.is_dir() {
        return Some(Err(()));
    }

    loop {
        let marker = candidate.join(".git");
        match fs::symlink_metadata(&marker) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Some(Err(()));
                }
                let git_dir = if metadata.is_dir() {
                    marker
                } else if metadata.is_file() {
                    let Ok(contents) = read_bounded_utf8(&marker) else {
                        return Some(Err(()));
                    };
                    let mut lines = contents
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty());
                    let Some(git_dir_value) = lines
                        .next()
                        .and_then(|line| strip_ascii_prefix(line, "gitdir:").map(str::trim))
                        .filter(|value| !value.is_empty())
                    else {
                        return Some(Err(()));
                    };
                    if lines.next().is_some() {
                        return Some(Err(()));
                    }
                    let path = PathBuf::from(git_dir_value);
                    if path.is_absolute() {
                        path
                    } else {
                        candidate.join(path)
                    }
                } else {
                    return Some(Err(()));
                };
                let Ok(git_dir) = fs::canonicalize(git_dir) else {
                    return Some(Err(()));
                };
                if !git_dir.is_dir() {
                    return Some(Err(()));
                }
                let git_common_dir = match read_optional_bounded_utf8(&git_dir.join("commondir")) {
                    Ok(Some(value)) => {
                        let value = value.trim();
                        if value.is_empty() || value.lines().count() != 1 {
                            return Some(Err(()));
                        }
                        let path = PathBuf::from(value);
                        let path = if path.is_absolute() {
                            path
                        } else {
                            git_dir.join(path)
                        };
                        let Ok(path) = fs::canonicalize(path) else {
                            return Some(Err(()));
                        };
                        path
                    }
                    Ok(None) => git_dir.clone(),
                    Err(()) => return Some(Err(())),
                };
                if !git_common_dir.is_dir() {
                    return Some(Err(()));
                }
                return Some(Ok(ProcessFreeLayout {
                    worktree_root: candidate,
                    git_dir,
                    git_common_dir,
                }));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Some(Err(())),
        }
        if !candidate.pop() {
            return None;
        }
    }
}

fn process_free_remotes(layout: &ProcessFreeLayout) -> Result<Vec<RepositoryRemote>, ()> {
    let Some(config) = read_optional_bounded_utf8(&layout.git_common_dir.join("config"))? else {
        return Ok(Vec::new());
    };
    parse_process_free_remotes(&config)
}

fn read_bounded_utf8(path: &Path) -> Result<String, ()> {
    let metadata = fs::metadata(path).map_err(|_| ())?;
    if !metadata.is_file() || metadata.len() > MAX_GIT_CONTROL_FILE_BYTES {
        return Err(());
    }
    fs::read_to_string(path).map_err(|_| ())
}

fn read_optional_bounded_utf8(path: &Path) -> Result<Option<String>, ()> {
    match fs::metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() || metadata.len() > MAX_GIT_CONTROL_FILE_BYTES {
                return Err(());
            }
            fs::read_to_string(path).map(Some).map_err(|_| ())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(()),
    }
}

fn parse_process_free_remotes(config: &str) -> Result<Vec<RepositoryRemote>, ()> {
    let mut section = String::new();
    let mut subsection = None::<String>;
    let mut remotes = Vec::new();
    for raw_line in config.trim_start_matches('\u{feff}').lines() {
        if raw_line.trim_end().ends_with('\\') {
            return Err(());
        }
        let line = strip_git_config_comment(raw_line)?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            let (next_section, next_subsection) = parse_git_config_section(line)?;
            if next_section.eq_ignore_ascii_case("include")
                || next_section.eq_ignore_ascii_case("includeif")
            {
                return Err(());
            }
            section = next_section;
            subsection = next_subsection;
            continue;
        }

        let (name, value) = parse_git_config_assignment(line)?;
        if section.eq_ignore_ascii_case("include")
            || section.eq_ignore_ascii_case("includeif")
            || (section.is_empty() && name.eq_ignore_ascii_case("include.path"))
        {
            return Err(());
        }
        if section.eq_ignore_ascii_case("extensions")
            && name.eq_ignore_ascii_case("worktreeconfig")
            && git_config_truthy(&value)
        {
            return Err(());
        }
        if section.eq_ignore_ascii_case("remote") && name.eq_ignore_ascii_case("url") {
            let remote_name = subsection
                .as_ref()
                .filter(|name| !name.is_empty())
                .ok_or(())?;
            if !value.trim().is_empty() {
                remotes.push(RepositoryRemote {
                    name: remote_name.clone(),
                    url: value.trim().to_owned(),
                });
            }
        }
    }
    remotes.sort_unstable();
    remotes.dedup();
    Ok(remotes)
}

fn strip_git_config_comment(line: &str) -> Result<String, ()> {
    let mut result = String::with_capacity(line.len());
    let mut quoted = false;
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            result.push('\\');
            result.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == '"' {
            quoted = !quoted;
            result.push(character);
            continue;
        }
        if !quoted && matches!(character, '#' | ';') {
            if result.trim().is_empty()
                || result.chars().next_back().is_some_and(char::is_whitespace)
            {
                break;
            }
            // Ambiguous comment placement is uncommon and not worth
            // interpreting differently from Git on the Hook fast path.
            return Err(());
        }
        result.push(character);
    }
    if quoted || escaped {
        return Err(());
    }
    Ok(result)
}

fn parse_git_config_section(line: &str) -> Result<(String, Option<String>), ()> {
    let body = line
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .map(str::trim)
        .ok_or(())?;
    if body.is_empty() {
        return Err(());
    }
    if let Some((section, subsection)) = body.split_once(char::is_whitespace) {
        let subsection = subsection.trim();
        if !subsection.starts_with('"') || !subsection.ends_with('"') {
            return Err(());
        }
        let subsection = parse_git_config_value(subsection)?;
        return Ok((section.to_owned(), Some(subsection)));
    }
    if let Some((section, subsection)) = body.split_once('.')
        && section.eq_ignore_ascii_case("remote")
    {
        return Ok((section.to_owned(), Some(subsection.to_owned())));
    }
    Ok((body.to_owned(), None))
}

fn parse_git_config_assignment(line: &str) -> Result<(String, String), ()> {
    let (name, value) = if let Some((name, value)) = line.split_once('=') {
        (name.trim(), value.trim())
    } else if let Some(index) = line.find(char::is_whitespace) {
        (line[..index].trim(), line[index..].trim())
    } else {
        (line.trim(), "true")
    };
    if name.is_empty() {
        return Err(());
    }
    Ok((name.to_owned(), parse_git_config_value(value)?))
}

fn parse_git_config_value(value: &str) -> Result<String, ()> {
    let mut result = String::with_capacity(value.len());
    let mut quoted = false;
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            result.push(match character {
                'n' => '\n',
                't' => '\t',
                'b' => '\u{0008}',
                '\\' => '\\',
                '"' => '"',
                _ => return Err(()),
            });
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
        } else if character == '"' {
            quoted = !quoted;
        } else {
            result.push(character);
        }
    }
    if quoted || escaped {
        return Err(());
    }
    Ok(result.trim().to_owned())
}

fn git_config_truthy(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "yes" | "on" | "1"
    )
}

fn strip_ascii_prefix<'a>(value: &'a str, prefix: &str) -> Option<&'a str> {
    value
        .get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .map(|_| &value[prefix.len()..])
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
    use std::{
        fs,
        process::Command,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        LAYOUT_ARGS, ProcessFreeDiscovery, REMOTE_ARGS, ROOT_COMMIT_ARGS, RepositoryDiscovery,
        discover_process_free, parse_process_free_remotes, parse_remotes,
    };

    fn test_root(label: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("test clock follows Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tabbeacon-discovery-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("test root creates");
        root
    }

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

    #[test]
    fn process_free_remote_config_is_sorted_deduplicated_and_unquoted() {
        let parsed = parse_process_free_remotes(
            "[remote \"zeta\"]\n\turl = \"ssh://host/z/repo.git\"\n\
             [remote \"origin\"]\n\turl = https://host/o/repo.git # primary\n\
             \turl = https://host/o/repo.git\n",
        )
        .expect("direct local config is supported");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].name(), "origin");
        assert_eq!(parsed[0].url(), "https://host/o/repo.git");
        assert_eq!(parsed[1].name(), "zeta");
        assert_eq!(parsed[1].url(), "ssh://host/z/repo.git");
    }

    #[test]
    fn process_free_remote_config_defers_indirection_and_ambiguous_comments_to_git() {
        for config in [
            "[include]\n\tpath = ../shared.config\n",
            "[includeIf \"gitdir:~/work/\"]\n\tpath = ../shared.config\n",
            "[extensions]\n\tworktreeConfig = true\n",
            "[remote \"origin\"]\n\turl = ssh://host/repo;variant\n",
        ] {
            assert!(
                parse_process_free_remotes(config).is_err(),
                "unsupported local config must use Git: {config}"
            );
        }
    }

    #[test]
    fn process_free_discovery_supports_linked_worktree_gitfiles() {
        let root = test_root("linked-worktree");
        let worktree = root.join("worktree");
        let nested = worktree.join("nested");
        let git_dir = root.join("admin");
        let common_dir = root.join("common");
        fs::create_dir_all(&nested).expect("linked worktree creates");
        fs::create_dir_all(&git_dir).expect("linked Git directory creates");
        fs::create_dir_all(&common_dir).expect("common Git directory creates");
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", git_dir.display()),
        )
        .expect("gitfile writes");
        fs::write(git_dir.join("commondir"), "../common\n").expect("commondir writes");
        fs::write(
            common_dir.join("config"),
            "[remote \"origin\"]\n\turl = https://example.invalid/acme/linked.git\n",
        )
        .expect("common config writes");

        let ProcessFreeDiscovery::Repository(discovered) = discover_process_free(&nested) else {
            panic!("complete linked-worktree metadata resolves without Git");
        };
        assert_eq!(discovered.worktree_root, worktree);
        assert_eq!(
            discovered.git_dir,
            fs::canonicalize(git_dir).expect("Git directory canonicalizes")
        );
        assert_eq!(
            discovered.git_common_dir,
            fs::canonicalize(common_dir).expect("common directory canonicalizes")
        );
        assert_eq!(discovered.remotes.len(), 1);
        assert_eq!(discovered.remotes[0].name(), "origin");

        fs::remove_dir_all(root).expect("owned test root removes");
    }

    #[test]
    fn remote_backed_discovery_can_omit_root_history() {
        let root = test_root("remote-without-history");
        let initialize = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .expect("Git initializes test repository");
        assert!(initialize.success());
        let remote = Command::new("git")
            .args([
                "remote",
                "add",
                "origin",
                "https://example.invalid/acme/fixture.git",
            ])
            .current_dir(&root)
            .status()
            .expect("Git configures local test remote");
        assert!(remote.success());

        let discovered = RepositoryDiscovery::default()
            .discover_without_root_commits(&root)
            .expect("layout and local remotes discover without history walk");

        assert_eq!(discovered.remotes.len(), 1);
        assert!(discovered.root_commits.is_empty());
        fs::remove_dir_all(root).expect("owned test root removes");
    }
}
