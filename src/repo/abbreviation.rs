use std::{collections::BTreeSet, fmt};

use sha2::{Digest, Sha256};

use super::{CanonicalRepositoryIdentity, RepositoryDisplayName, RepositoryIdentityError};

const MAX_READABLE_CHARS: usize = 12;
const MAX_ALIAS_CHARS: usize = 20;

/// Checked, bounded human-short repository identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepositoryAlias(String);

impl RepositoryAlias {
    /// Creates a safe alias containing only alphanumerics and optional hyphens.
    ///
    /// # Errors
    ///
    /// Rejects empty, overlong, control-bearing, or punctuation-bearing values.
    pub fn new(value: impl Into<String>) -> Result<Self, RepositoryIdentityError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.chars().count() <= MAX_ALIAS_CHARS
            && value
                .chars()
                .all(|character| character.is_alphanumeric() || character == '-');
        if valid {
            Ok(Self(value))
        } else {
            Err(RepositoryIdentityError::InvalidIdentifier {
                kind: "repository alias",
                detail: "alias must be bounded alphanumerics with optional hyphens".to_owned(),
            })
        }
    }

    /// Returns the presentation-safe alias text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepositoryAlias {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Pure deterministic tokenization, expansion, and hash-fallback policy.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AbbreviationPolicy;

impl AbbreviationPolicy {
    /// Returns the shortest deterministic readable alias.
    #[must_use]
    pub fn base_alias(display_name: &RepositoryDisplayName) -> RepositoryAlias {
        Self::readable_candidates(display_name)
            .into_iter()
            .next()
            .unwrap_or_else(|| RepositoryAlias("R".to_owned()))
    }

    /// Returns ordered readable candidates followed by stable hash candidates.
    #[must_use]
    pub fn candidates(
        display_name: &RepositoryDisplayName,
        identity: &CanonicalRepositoryIdentity,
    ) -> Vec<RepositoryAlias> {
        let mut candidates = Self::readable_candidates(display_name);
        let mut seen = candidates.iter().cloned().collect::<BTreeSet<_>>();
        let base = candidates
            .first()
            .map_or_else(|| "R".to_owned(), |alias| alias.as_str().to_owned());
        let digest = format!("{:x}", Sha256::digest(identity.as_str().as_bytes()));
        for suffix_chars in 6..=16 {
            let suffix = &digest[..suffix_chars];
            let base_budget = MAX_ALIAS_CHARS.saturating_sub(suffix_chars + 1);
            let prefix = base.chars().take(base_budget.max(1)).collect::<String>();
            let value = format!("{prefix}-{suffix}");
            if let Ok(alias) = RepositoryAlias::new(value)
                && seen.insert(alias.clone())
            {
                candidates.push(alias);
            }
        }
        candidates
    }

    fn readable_candidates(display_name: &RepositoryDisplayName) -> Vec<RepositoryAlias> {
        let tokens = tokenize(display_name.as_str());
        if tokens.is_empty() {
            return vec![RepositoryAlias("R".to_owned())];
        }
        let mut widths = vec![1_usize; tokens.len()];
        let mut output = Vec::new();
        let mut seen = BTreeSet::new();
        loop {
            let rendered = render(&tokens, &widths);
            if rendered.chars().count() <= MAX_READABLE_CHARS
                && let Ok(alias) = RepositoryAlias::new(rendered)
                && seen.insert(alias.clone())
            {
                output.push(alias);
            }
            let mut advanced = false;
            for (width, token) in widths.iter_mut().zip(&tokens) {
                if *width < token.len() {
                    *width += 1;
                    advanced = true;
                    break;
                }
            }
            if !advanced {
                break;
            }
        }
        if output.is_empty() {
            output.push(RepositoryAlias("R".to_owned()));
        }
        output
    }
}

fn tokenize(value: &str) -> Vec<Vec<char>> {
    let characters = value.chars().collect::<Vec<_>>();
    let mut tokens = Vec::<Vec<char>>::new();
    let mut current = Vec::new();
    for (index, character) in characters.iter().copied().enumerate() {
        if !character.is_alphanumeric() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }
        let previous = current.last().copied();
        let next = characters.get(index + 1).copied();
        let lower_to_upper = previous
            .is_some_and(|value| value.is_lowercase() || value.is_numeric())
            && character.is_uppercase();
        let acronym_to_word = previous.is_some_and(char::is_uppercase)
            && character.is_uppercase()
            && next.is_some_and(char::is_lowercase);
        if (lower_to_upper || acronym_to_word) && !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
        current.push(character);
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn render(tokens: &[Vec<char>], widths: &[usize]) -> String {
    let mut rendered = String::new();
    for (token, width) in tokens.iter().zip(widths) {
        for character in token.iter().take(*width) {
            rendered.extend(character.to_uppercase());
        }
    }
    rendered.chars().take(MAX_READABLE_CHARS).collect()
}

#[cfg(test)]
mod tests {
    use super::AbbreviationPolicy;
    use crate::repo::{CanonicalRepositoryIdentity, RepositoryDisplayName};

    fn display(value: &str) -> RepositoryDisplayName {
        RepositoryDisplayName::new(value).expect("test display name is valid")
    }

    #[test]
    fn representative_initialisms_match_the_contract() {
        for (name, expected) in [
            ("jerry-dotfiles", "JD"),
            ("workstation-manager", "WM"),
            ("opencode-workspace-hub", "OWH"),
            ("jerry-proxy-control", "JPC"),
            ("OpenCode Workspace_Hub", "OCWH"),
        ] {
            assert_eq!(
                AbbreviationPolicy::base_alias(&display(name)).as_str(),
                expected
            );
        }
    }

    #[test]
    fn readable_expansion_precedes_stable_hash_fallback() {
        let identity = CanonicalRepositoryIdentity::new("remote:example/repo")
            .expect("test identity is valid");
        let candidates = AbbreviationPolicy::candidates(&display("repo"), &identity);
        assert_eq!(candidates[0].as_str(), "R");
        assert_eq!(candidates[1].as_str(), "RE");
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.as_str().starts_with("R-"))
        );
    }
}
