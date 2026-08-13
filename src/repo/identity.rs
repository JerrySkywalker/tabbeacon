use std::{fmt, path::Path};

use sha2::{Digest, Sha256};

use super::{DiscoveredRepository, RepositoryIdentityError, discovery::RepositoryRemote};

const MAX_IDENTITY_CHARS: usize = 4096;
const MAX_DISPLAY_CHARS: usize = 256;

/// Opaque canonical repository key, separate from session/provider identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalRepositoryIdentity(String);

impl CanonicalRepositoryIdentity {
    /// Creates a checked canonical identity.
    ///
    /// # Errors
    ///
    /// Rejects empty, control-bearing, or unreasonably long values.
    pub fn new(value: impl Into<String>) -> Result<Self, RepositoryIdentityError> {
        let value = value.into();
        validate_identifier(&value, "canonical repository identity", MAX_IDENTITY_CHARS)?;
        Ok(Self(value))
    }

    /// Returns the stable opaque key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CanonicalRepositoryIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Safe human-oriented repository name used only as abbreviation input.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepositoryDisplayName(String);

impl RepositoryDisplayName {
    /// Sanitizes a local display hint into a bounded non-empty value.
    ///
    /// # Errors
    ///
    /// Rejects values that remain empty after sanitizing control characters.
    pub fn new(value: impl AsRef<str>) -> Result<Self, RepositoryIdentityError> {
        let sanitized = value
            .as_ref()
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
        let bounded = trimmed.chars().take(MAX_DISPLAY_CHARS).collect::<String>();
        validate_identifier(&bounded, "repository display name", MAX_DISPLAY_CHARS)?;
        Ok(Self(bounded))
    }

    /// Returns the sanitized human name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepositoryDisplayName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Canonical identity plus the safe name used for local alias allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalizedRepository {
    /// Stable repository key.
    pub identity: CanonicalRepositoryIdentity,
    /// Human-safe abbreviation input.
    pub display_name: RepositoryDisplayName,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NormalizedRemote {
    canonical: String,
    display_name: String,
}

/// Normalizes a configured remote without DNS or network access.
///
/// Common HTTPS, `ssh://`, and SCP-like SSH spellings converge on a
/// scheme-neutral `host/path` value. Local paths use a hashed, path-local key.
///
/// # Errors
///
/// Returns a typed identifier error for empty, hostile, or malformed input.
pub fn normalize_remote_url(
    value: &str,
    worktree_root: impl AsRef<Path>,
) -> Result<(String, RepositoryDisplayName), RepositoryIdentityError> {
    let normalized = normalize_remote(value, worktree_root.as_ref())?;
    Ok((
        normalized.canonical,
        RepositoryDisplayName::new(normalized.display_name)?,
    ))
}

/// Selects canonical evidence in origin/other-remote/local-fingerprint order.
///
/// # Errors
///
/// Returns a typed error when no safe display name or identity can be derived.
pub fn canonicalize_repository(
    discovered: &DiscoveredRepository,
) -> Result<CanonicalizedRepository, RepositoryIdentityError> {
    if let Some(remote) = select_remote(&discovered.remotes, &discovered.worktree_root) {
        return Ok(CanonicalizedRepository {
            identity: CanonicalRepositoryIdentity::new(format!("remote:{}", remote.canonical))?,
            display_name: RepositoryDisplayName::new(remote.display_name)?,
        });
    }

    let display_name = fallback_display_name(&discovered.worktree_root)?;
    let identity = if discovered.root_commits.is_empty() {
        let path = normalize_local_path(&discovered.git_common_dir);
        format!("local-unborn:{}", hex_sha256(path.as_bytes()))
    } else {
        let mut digest = Sha256::new();
        digest.update(b"tabbeacon-local-root-commits-v1\0");
        for root in &discovered.root_commits {
            digest.update(root.as_bytes());
            digest.update([0]);
        }
        format!("local-roots:{:x}", digest.finalize())
    };
    Ok(CanonicalizedRepository {
        identity: CanonicalRepositoryIdentity::new(identity)?,
        display_name,
    })
}

fn select_remote(remotes: &[RepositoryRemote], worktree_root: &Path) -> Option<NormalizedRemote> {
    let mut ordered = remotes.iter().collect::<Vec<_>>();
    ordered.sort_unstable_by(|left, right| {
        let left_origin = left.name() != "origin";
        let right_origin = right.name() != "origin";
        left_origin
            .cmp(&right_origin)
            .then(left.name().cmp(right.name()))
            .then(left.url().cmp(right.url()))
    });
    for remote in ordered {
        if let Ok(normalized) = normalize_remote(remote.url(), worktree_root) {
            return Some(normalized);
        }
    }
    None
}

fn normalize_remote(
    value: &str,
    worktree_root: &Path,
) -> Result<NormalizedRemote, RepositoryIdentityError> {
    let value = value.trim();
    validate_identifier(value, "remote URL", MAX_IDENTITY_CHARS)?;
    if let Some((scheme, remainder)) = value.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        return match scheme.as_str() {
            "http" | "https" | "ssh" | "git" => normalize_network_remote(&scheme, remainder),
            "file" => normalize_file_remote(remainder, worktree_root, true),
            _ => Err(invalid_identifier("remote URL", "unsupported URL scheme")),
        };
    }
    if let Some(remote) = normalize_scp_remote(value)? {
        return Ok(remote);
    }
    normalize_file_remote(value, worktree_root, false)
}

fn normalize_network_remote(
    scheme: &str,
    remainder: &str,
) -> Result<NormalizedRemote, RepositoryIdentityError> {
    let (authority, path) = remainder
        .split_once('/')
        .ok_or_else(|| invalid_identifier("remote URL", "network URL lacks a repository path"))?;
    let (host, port) = parse_authority(authority)?;
    let path = normalize_repo_path(path)?;
    let default_port = matches!(
        (scheme, port.as_deref()),
        ("ssh", Some("22")) | ("https", Some("443")) | ("http", Some("80")) | ("git", Some("9418"))
    );
    let host_port = match port {
        Some(port) if !default_port => format!("{host}:{port}"),
        _ => host,
    };
    let display_name = final_path_component(&path)?;
    Ok(NormalizedRemote {
        canonical: format!("{host_port}/{path}"),
        display_name,
    })
}

fn normalize_scp_remote(value: &str) -> Result<Option<NormalizedRemote>, RepositoryIdentityError> {
    if value.len() >= 3
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value.as_bytes()[1] == b':'
        && matches!(value.as_bytes()[2], b'/' | b'\\')
    {
        return Ok(None);
    }
    let Some((authority, path)) = value.split_once(':') else {
        return Ok(None);
    };
    if authority.contains('/') || authority.contains('\\') || path.is_empty() {
        return Ok(None);
    }
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = normalize_host(host)?;
    let path = normalize_repo_path(path)?;
    let display_name = final_path_component(&path)?;
    Ok(Some(NormalizedRemote {
        canonical: format!("{host}/{path}"),
        display_name,
    }))
}

fn normalize_file_remote(
    value: &str,
    worktree_root: &Path,
    from_file_url: bool,
) -> Result<NormalizedRemote, RepositoryIdentityError> {
    let value = if from_file_url
        && cfg!(windows)
        && value.starts_with('/')
        && value.as_bytes().get(2) == Some(&b':')
    {
        &value[1..]
    } else {
        value
    };
    let decoded = value.replace('/', std::path::MAIN_SEPARATOR_STR);
    let path = Path::new(&decoded);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        worktree_root.join(path)
    };
    let normalized_path = normalize_local_path(&path);
    let display_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(strip_dot_git)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| invalid_identifier("remote URL", "local path lacks a safe name"))?;
    Ok(NormalizedRemote {
        canonical: format!("local-file:{}", hex_sha256(normalized_path.as_bytes())),
        display_name: display_name.to_owned(),
    })
}

fn parse_authority(authority: &str) -> Result<(String, Option<String>), RepositoryIdentityError> {
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, value)| value);
    if host_port.starts_with('[') {
        let close = host_port
            .find(']')
            .ok_or_else(|| invalid_identifier("remote URL", "unterminated IPv6 authority"))?;
        let host = normalize_host(&host_port[..=close])?;
        let port = host_port[close + 1..]
            .strip_prefix(':')
            .filter(|value| !value.is_empty())
            .map(validate_port)
            .transpose()?;
        return Ok((host, port));
    }
    if let Some((host, port)) = host_port.rsplit_once(':')
        && port.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Ok((normalize_host(host)?, Some(validate_port(port)?)));
    }
    Ok((normalize_host(host_port)?, None))
}

fn normalize_host(host: &str) -> Result<String, RepositoryIdentityError> {
    let host = host.trim().to_lowercase();
    if host.is_empty()
        || host.chars().any(char::is_whitespace)
        || host.contains('/')
        || host.contains('\\')
    {
        Err(invalid_identifier("remote URL", "invalid host"))
    } else {
        Ok(host)
    }
}

fn validate_port(port: &str) -> Result<String, RepositoryIdentityError> {
    let number = port
        .parse::<u16>()
        .map_err(|_| invalid_identifier("remote URL", "invalid port"))?;
    if number == 0 {
        Err(invalid_identifier("remote URL", "port must be nonzero"))
    } else {
        Ok(number.to_string())
    }
}

fn normalize_repo_path(path: &str) -> Result<String, RepositoryIdentityError> {
    let replaced = path.replace('\\', "/");
    let mut parts = Vec::new();
    for part in replaced.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                return Err(invalid_identifier(
                    "remote URL",
                    "parent path is not allowed",
                ));
            }
            value => parts.push(value),
        }
    }
    if parts.is_empty() {
        return Err(invalid_identifier("remote URL", "repository path is empty"));
    }
    let last = parts.pop().expect("nonempty path parts");
    let last = strip_dot_git(last);
    if last.is_empty() {
        return Err(invalid_identifier("remote URL", "repository name is empty"));
    }
    parts.push(last);
    let normalized = parts.join("/");
    validate_identifier(&normalized, "remote repository path", MAX_IDENTITY_CHARS)?;
    Ok(normalized)
}

fn strip_dot_git(value: &str) -> &str {
    value
        .get(..value.len().saturating_sub(4))
        .filter(|_| value.to_ascii_lowercase().ends_with(".git"))
        .unwrap_or(value)
}

fn final_path_component(path: &str) -> Result<String, RepositoryIdentityError> {
    path.rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid_identifier("remote URL", "repository name is empty"))
}

fn fallback_display_name(path: &Path) -> Result<RepositoryDisplayName, RepositoryIdentityError> {
    let value = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("repository");
    RepositoryDisplayName::new(value)
}

fn normalize_local_path(path: &Path) -> String {
    let value = path.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        value.to_lowercase()
    } else {
        value
    }
}

fn validate_identifier(
    value: &str,
    kind: &'static str,
    max_chars: usize,
) -> Result<(), RepositoryIdentityError> {
    let length = value.chars().count();
    if value.trim().is_empty() {
        Err(invalid_identifier(kind, "value is empty"))
    } else if value.chars().any(char::is_control) {
        Err(invalid_identifier(kind, "control characters are forbidden"))
    } else if length > max_chars {
        Err(invalid_identifier(kind, "value exceeds the length limit"))
    } else {
        Ok(())
    }
}

fn invalid_identifier(kind: &'static str, detail: &str) -> RepositoryIdentityError {
    RepositoryIdentityError::InvalidIdentifier {
        kind,
        detail: detail.to_owned(),
    }
}

fn hex_sha256(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{RepositoryDisplayName, normalize_remote_url};

    #[test]
    fn common_https_and_ssh_forms_converge() {
        let root = Path::new("C:/offline/test");
        let urls = [
            "https://github.com/JerrySkywalker/tabbeacon.git",
            "ssh://git@github.com:22/JerrySkywalker/tabbeacon.git/",
            "git@GITHUB.COM:JerrySkywalker/tabbeacon.git",
        ];
        let normalized = urls.map(|url| normalize_remote_url(url, root).expect("valid remote").0);
        assert!(normalized.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(normalized[0], "github.com/JerrySkywalker/tabbeacon");
    }

    #[test]
    fn nondefault_ports_remain_distinct() {
        let root = Path::new("C:/offline/test");
        let normal = normalize_remote_url("ssh://git@example.com/repo.git", root)
            .expect("valid remote")
            .0;
        let custom = normalize_remote_url("ssh://git@example.com:2222/repo.git", root)
            .expect("valid remote")
            .0;
        assert_ne!(normal, custom);
    }

    #[test]
    fn absolute_local_path_and_file_url_converge() {
        let root = std::env::temp_dir().join("tabbeacon-local-remote-root");
        let repository = root.join("team").join("repo.git");
        let path_remote = repository.to_string_lossy();
        let file_remote = format!("file://{}", path_remote.replace('\\', "/"));
        let path_identity = normalize_remote_url(&path_remote, &root)
            .expect("absolute local path is valid")
            .0;
        let file_identity = normalize_remote_url(&file_remote, &root)
            .expect("equivalent file URL is valid")
            .0;
        assert_eq!(path_identity, file_identity);
    }

    #[test]
    fn hostile_display_controls_are_sanitized() {
        let name = RepositoryDisplayName::new("repo\u{1b}[31m")
            .expect("controls are replaced, not retained");
        assert_eq!(name.as_str(), "repo [31m");
    }
}
