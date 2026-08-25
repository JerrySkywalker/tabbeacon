//! Provider visual identity is separate from runtime and workspace identity.
//!
//! The registry exposes only fixed product-owned metadata. It never accepts an
//! executable, image path, URL, or unbounded provider-supplied title payload.

/// Optional declarative native-icon metadata for a provider.
///
/// This is deliberately an identifier rather than a path or image payload.
/// A terminal backend must separately establish that it can render the named
/// asset before using it. No native provider assets are shipped yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NativeIconSpec {
    asset_id: &'static str,
}

impl NativeIconSpec {
    /// Stable product-owned asset identifier.
    #[must_use]
    pub const fn asset_id(self) -> &'static str {
        self.asset_id
    }
}

/// Provider identity metadata independent from runtime and workspace state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderVisualIdentity {
    provider_id: &'static str,
    short_name: &'static str,
    accessible_name: &'static str,
    title_mark: &'static str,
    text_fallback: &'static str,
    native_icon_spec: Option<NativeIconSpec>,
}

impl ProviderVisualIdentity {
    /// Resolves a fixed visual identity without retaining an unknown input.
    ///
    /// Unknown provider identifiers deterministically receive the same safe
    /// text-only fallback; their raw identifier is never rendered into a title.
    #[must_use]
    pub fn for_provider_id(provider_id: &str) -> Self {
        match provider_id {
            "codex" => Self::codex(),
            "agy" => Self::agy(),
            _ => Self::unknown(),
        }
    }

    /// Codex product identity.
    #[must_use]
    pub const fn codex() -> Self {
        Self {
            provider_id: "codex",
            short_name: "Codex",
            accessible_name: "Codex provider",
            title_mark: "C",
            text_fallback: "Codex",
            native_icon_spec: None,
        }
    }

    /// Agy product identity.
    #[must_use]
    pub const fn agy() -> Self {
        Self {
            provider_id: "agy",
            short_name: "Agy",
            accessible_name: "Agy provider",
            title_mark: "A",
            text_fallback: "Agy",
            native_icon_spec: None,
        }
    }

    /// Deterministic identity used for an unknown provider.
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            provider_id: "unknown",
            short_name: "Unknown",
            accessible_name: "Unknown provider",
            title_mark: "?",
            text_fallback: "Unknown",
            native_icon_spec: None,
        }
    }

    /// Stable registered provider identifier.
    #[must_use]
    pub const fn provider_id(self) -> &'static str {
        self.provider_id
    }

    /// Compact, user-visible provider name.
    #[must_use]
    pub const fn short_name(self) -> &'static str {
        self.short_name
    }

    /// Screen-reader-safe provider name.
    #[must_use]
    pub const fn accessible_name(self) -> &'static str {
        self.accessible_name
    }

    /// Abstract one-character provider mark for capable future backends.
    #[must_use]
    pub const fn title_mark(self) -> &'static str {
        self.title_mark
    }

    /// Stable text used by the production title-mark fallback.
    #[must_use]
    pub const fn text_fallback(self) -> &'static str {
        self.text_fallback
    }

    /// Optional native-icon metadata; absence always permits title fallback.
    #[must_use]
    pub const fn native_icon_spec(self) -> Option<NativeIconSpec> {
        self.native_icon_spec
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderVisualIdentity;

    #[test]
    fn known_and_unknown_identities_are_fixed_and_text_safe() {
        let codex = ProviderVisualIdentity::for_provider_id("codex");
        let agy = ProviderVisualIdentity::for_provider_id("agy");
        let unknown = ProviderVisualIdentity::for_provider_id("untrusted\x1b]0;payload");

        assert_eq!((codex.short_name(), codex.title_mark()), ("Codex", "C"));
        assert_eq!((agy.short_name(), agy.title_mark()), ("Agy", "A"));
        assert_eq!(unknown, ProviderVisualIdentity::unknown());
        assert_eq!(unknown.text_fallback(), "Unknown");
        assert!(unknown.native_icon_spec().is_none());
    }
}
