//! Versioned, portable user-settings document primitives for G54.
//!
//! This module is deliberately pure: it serializes only typed, user-owned
//! configuration and never reads or writes a store. The command layer owns
//! preview, snapshots, compensation, and explicit Apply semantics.

use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    interface_preferences::{HumanColor, InterfaceLanguage, InterfacePreferences},
    repo::WorkspacePreferences,
    settings::{
        ActivityMode, PresentationSettings, PresentationTheme, SpinnerPreset, TabColorMode,
        TitleMode,
    },
};

/// Stable schema identifier for portable user configuration exports.
pub const EXPORT_SCHEMA_V1: &str = "tabbeacon-export-v1";
/// Hard bound before JSON parsing so an import cannot become an unbounded log
/// or arbitrary system image.
pub const MAX_EXPORT_BYTES: usize = 1024 * 1024;

/// Safe pure-document failure. Store and CLI layers map their own I/O errors.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum SettingsTransferError {
    /// Input exceeds the fixed bounded import size.
    Oversize,
    /// The document is malformed, unsupported, or violates this schema.
    InvalidDocument,
}

impl fmt::Display for SettingsTransferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Oversize => "the TabBeacon export document exceeds the supported size",
            Self::InvalidDocument => "the TabBeacon export document is malformed or unsupported",
        })
    }
}

impl std::error::Error for SettingsTransferError {}

/// Canonical locale-independent portable configuration document.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsExportV1 {
    schema: String,
    presentation: Option<PresentationExport>,
    interface: Option<InterfaceExport>,
    workspace_aliases: BTreeMap<String, String>,
    omitted_device_local_workspace_aliases: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresentationExport {
    title: String,
    tab_color: String,
    activity: String,
    spinner: String,
    theme: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InterfaceExport {
    language: String,
    color: String,
    reduced_motion: bool,
}

impl SettingsExportV1 {
    /// Builds a portable document. Ordinary-directory aliases are deliberately
    /// omitted: their identities are local absolute-path derivatives and must
    /// never be represented as cross-device portable state.
    #[must_use]
    pub fn new(
        presentation: Option<PresentationSettings>,
        interface: Option<InterfacePreferences>,
        workspace_preferences: &WorkspacePreferences,
    ) -> Self {
        let mut workspace_aliases = BTreeMap::new();
        let mut omitted_device_local_workspace_aliases = 0;
        for (identity, alias) in workspace_preferences.overrides() {
            if identity.as_str().starts_with("dir-v1:") {
                omitted_device_local_workspace_aliases += 1;
            } else {
                workspace_aliases.insert(
                    portable_workspace_key(identity.as_str()),
                    alias.as_str().to_owned(),
                );
            }
        }
        Self {
            schema: EXPORT_SCHEMA_V1.to_owned(),
            presentation: presentation.map(PresentationExport::from),
            interface: interface.map(InterfaceExport::from),
            workspace_aliases,
            omitted_device_local_workspace_aliases,
        }
    }

    /// Produces deterministic, locale-independent canonical JSON bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization cannot produce a valid bounded document.
    pub fn to_canonical_json(&self) -> Result<Vec<u8>, SettingsTransferError> {
        serde_json::to_vec_pretty(self).map_err(|_| SettingsTransferError::InvalidDocument)
    }

    /// Parses a bounded document and refuses a future/unknown schema.
    ///
    /// # Errors
    ///
    /// Returns an error for oversize, malformed, unknown-schema, or invalid typed values.
    pub fn parse(bytes: &[u8]) -> Result<Self, SettingsTransferError> {
        if bytes.len() > MAX_EXPORT_BYTES {
            return Err(SettingsTransferError::Oversize);
        }
        let document: Self =
            serde_json::from_slice(bytes).map_err(|_| SettingsTransferError::InvalidDocument)?;
        if document.schema != EXPORT_SCHEMA_V1 {
            return Err(SettingsTransferError::InvalidDocument);
        }
        document.validate()?;
        Ok(document)
    }

    /// Typed presentation draft, when the portable document carries one.
    ///
    /// # Errors
    ///
    /// Returns an error if stored presentation tokens are unsupported.
    pub fn presentation(&self) -> Result<Option<PresentationSettings>, SettingsTransferError> {
        self.presentation
            .as_ref()
            .map(PresentationExport::settings)
            .transpose()
    }

    /// Typed interface draft, when the portable document carries one.
    ///
    /// # Errors
    ///
    /// Returns an error if stored Interface tokens are unsupported.
    pub fn interface(&self) -> Result<Option<InterfacePreferences>, SettingsTransferError> {
        self.interface
            .as_ref()
            .map(InterfaceExport::preferences)
            .transpose()
    }

    /// Portable Git identity-digest override map. The map has no raw identity,
    /// workspace root, or display path.
    #[must_use]
    pub fn workspace_aliases(&self) -> &BTreeMap<String, String> {
        &self.workspace_aliases
    }

    /// Number of truthful ordinary-directory omissions.
    #[must_use]
    pub const fn omitted_device_local_workspace_aliases(&self) -> usize {
        self.omitted_device_local_workspace_aliases
    }

    fn validate(&self) -> Result<(), SettingsTransferError> {
        let _ = self.presentation()?;
        let _ = self.interface()?;
        if self.workspace_aliases.iter().any(|(key, alias)| {
            key.len() != 64
                || !key.bytes().all(|byte| byte.is_ascii_hexdigit())
                || crate::repo::RepositoryAlias::new(alias.clone()).is_err()
        }) {
            return Err(SettingsTransferError::InvalidDocument);
        }
        Ok(())
    }
}

impl From<PresentationSettings> for PresentationExport {
    fn from(value: PresentationSettings) -> Self {
        Self {
            title: value.title().as_str().to_owned(),
            tab_color: value.tab_color().as_str().to_owned(),
            activity: value.activity().as_str().to_owned(),
            spinner: value.spinner().as_str().to_owned(),
            theme: value.theme().as_str().to_owned(),
        }
    }
}

impl PresentationExport {
    fn settings(&self) -> Result<PresentationSettings, SettingsTransferError> {
        Ok(PresentationSettings::new(
            TitleMode::parse(&self.title).ok_or(SettingsTransferError::InvalidDocument)?,
            TabColorMode::parse(&self.tab_color).ok_or(SettingsTransferError::InvalidDocument)?,
            ActivityMode::parse(&self.activity).ok_or(SettingsTransferError::InvalidDocument)?,
            SpinnerPreset::parse(&self.spinner).ok_or(SettingsTransferError::InvalidDocument)?,
            PresentationTheme::parse(&self.theme).ok_or(SettingsTransferError::InvalidDocument)?,
        ))
    }
}

impl From<InterfacePreferences> for InterfaceExport {
    fn from(value: InterfacePreferences) -> Self {
        Self {
            language: value.language().as_str().to_owned(),
            color: value.color().as_str().to_owned(),
            reduced_motion: value.reduced_motion(),
        }
    }
}

impl InterfaceExport {
    fn preferences(&self) -> Result<InterfacePreferences, SettingsTransferError> {
        Ok(InterfacePreferences::new(
            InterfaceLanguage::parse(&self.language)
                .ok_or(SettingsTransferError::InvalidDocument)?,
            HumanColor::parse(&self.color).ok_or(SettingsTransferError::InvalidDocument)?,
            self.reduced_motion,
        ))
    }
}

/// Computes the portable matching authority without serializing its input.
#[must_use]
pub fn portable_workspace_key(canonical_identity: &str) -> String {
    format!("{:x}", Sha256::digest(canonical_identity.as_bytes()))
}

#[cfg(test)]
mod tests {
    use crate::{
        interface_preferences::{HumanColor, InterfaceLanguage, InterfacePreferences},
        repo::{CanonicalRepositoryIdentity, RepositoryAlias, WorkspacePreferences},
        settings::{
            ActivityMode, PresentationSettings, PresentationTheme, SpinnerPreset, TabColorMode,
            TitleMode,
        },
    };

    use super::{
        EXPORT_SCHEMA_V1, MAX_EXPORT_BYTES, SettingsExportV1, SettingsTransferError,
        portable_workspace_key,
    };

    #[test]
    fn canonical_export_round_trips_typed_preferences_without_private_identity() {
        let git = CanonicalRepositoryIdentity::new("remote:example/tabbeacon").unwrap();
        let directory = CanonicalRepositoryIdentity::new("dir-v1:private-local-path-hash").unwrap();
        let preferences = WorkspacePreferences::default()
            .with_override(git.clone(), RepositoryAlias::new("TB").unwrap())
            .with_override(directory, RepositoryAlias::new("LOCAL").unwrap());
        let settings = PresentationSettings::new(
            TitleMode::Native,
            TabColorMode::Off,
            ActivityMode::Both,
            SpinnerPreset::Braille,
            PresentationTheme::Classic,
        );
        let interface = InterfacePreferences::new(InterfaceLanguage::ZhCn, HumanColor::Never, true);
        let document = SettingsExportV1::new(Some(settings), Some(interface), &preferences);
        let first = document.to_canonical_json().unwrap();
        let parsed = SettingsExportV1::parse(&first).unwrap();
        assert_eq!(parsed.to_canonical_json().unwrap(), first);
        assert_eq!(parsed.presentation().unwrap(), Some(settings));
        assert_eq!(parsed.interface().unwrap(), Some(interface));
        assert_eq!(
            parsed
                .workspace_aliases()
                .get(&portable_workspace_key(git.as_str()))
                .map(String::as_str),
            Some("TB")
        );
        assert_eq!(parsed.omitted_device_local_workspace_aliases(), 1);
        let text = String::from_utf8(first).unwrap();
        assert!(text.contains(EXPORT_SCHEMA_V1));
        assert!(!text.contains(git.as_str()));
        assert!(!text.contains("dir-v1:"));
    }

    #[test]
    fn malformed_oversize_and_unknown_versions_fail_closed() {
        assert_eq!(
            SettingsExportV1::parse(br#"{"schema":"tabbeacon-export-v2"}"#),
            Err(SettingsTransferError::InvalidDocument)
        );
        assert_eq!(
            SettingsExportV1::parse(&vec![b' '; MAX_EXPORT_BYTES + 1]),
            Err(SettingsTransferError::Oversize)
        );
    }
}
