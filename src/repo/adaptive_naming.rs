use std::collections::BTreeSet;

use sha2::{Digest, Sha256};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::{CanonicalRepositoryIdentity, RepositoryAlias, RepositoryDisplayName};

/// Stable identifier for the pure, non-persisting adaptive naming policy.
pub const ADAPTIVE_NAMING_POLICY_ID: &str = "adaptive-v2";

const MAX_ALIAS_DISPLAY_WIDTH: usize = 20;
const TARGET_MIN_DISPLAY_WIDTH: usize = 3;
const TARGET_MAX_DISPLAY_WIDTH: usize = 8;
const MAX_CANDIDATES: usize = 64;

/// Input facts derived without filesystem, registry, terminal, or network access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameAnalysis {
    normalized_name: String,
    tokens: Vec<String>,
    style_hints: Vec<NameStyleHint>,
}

impl NameAnalysis {
    /// Returns the conservative NFC-normalized source name.
    #[must_use]
    pub fn normalized_name(&self) -> &str {
        &self.normalized_name
    }

    /// Returns deterministic, presentation-safe token boundaries.
    #[must_use]
    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }

    /// Returns detected naming-style facts in deterministic enum order.
    #[must_use]
    pub fn style_hints(&self) -> &[NameStyleHint] {
        &self.style_hints
    }
}

/// Style facts retained for explainable candidate selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NameStyleHint {
    /// A hyphen separated name was observed.
    KebabCase,
    /// An underscore separated name was observed.
    SnakeCase,
    /// A dot separated name was observed.
    DotSeparated,
    /// Whitespace separated words were observed.
    SpaceSeparated,
    /// A lower-to-upper word boundary was observed.
    CamelCase,
    /// An upper-to-lower word boundary was observed at the start of a word.
    PascalCase,
    /// An all-uppercase acronym was split from a following word.
    AcronymBoundary,
    /// A meaningful letter/digit boundary was observed.
    LetterDigitBoundary,
    /// A CJK ideograph run was observed.
    CjkRun,
    /// Latin-like and CJK segments were both observed.
    MixedCjkLatin,
    /// NFC altered the original spelling while preserving its meaning.
    NfcNormalized,
}

/// Deterministic candidate family, ordered by policy preference on score ties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AliasStrategy {
    /// One representative grapheme from each token.
    Initialism,
    /// Preserve an informative all-uppercase acronym before taking initials.
    AcronymAware,
    /// Grow a balanced, multi-token prefix into the readable target range.
    BalancedPrefix,
    /// Prefer a readable prefix for a single-token project or brand.
    SingleTokenReadablePrefix,
    /// Use a bounded Latin consonant skeleton when it remains recognisable.
    ConsonantSkeleton,
    /// Use a display-width-bounded prefix of a CJK token.
    UnicodePrefix,
    /// Preserve every token while giving the leading token another grapheme.
    TokenCompression,
    /// Deterministic identity-derived fallback after readable candidates.
    HashFallback,
}

impl AliasStrategy {
    #[must_use]
    const fn priority(self) -> u8 {
        match self {
            Self::AcronymAware => 0,
            Self::Initialism => 1,
            Self::SingleTokenReadablePrefix => 2,
            Self::UnicodePrefix => 3,
            Self::BalancedPrefix => 4,
            Self::TokenCompression => 5,
            Self::ConsonantSkeleton => 6,
            Self::HashFallback => 7,
        }
    }

    #[must_use]
    const fn label(self) -> &'static str {
        match self {
            Self::Initialism => "initialism",
            Self::AcronymAware => "acronym-aware",
            Self::BalancedPrefix => "balanced-prefix",
            Self::SingleTokenReadablePrefix => "single-token-readable-prefix",
            Self::ConsonantSkeleton => "consonant-skeleton",
            Self::UnicodePrefix => "unicode-prefix",
            Self::TokenCompression => "token-compression",
            Self::HashFallback => "hash-fallback",
        }
    }
}

/// Integer score components retained for stable, safe `alias explain` output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScoreComponents {
    /// Reward for source-token coverage.
    pub token_coverage: i32,
    /// Reward for preserving an all-uppercase acronym.
    pub acronym_preservation: i32,
    /// Reward for retaining a recognisable multi-grapheme prefix.
    pub recognizable_prefix: i32,
    /// Reward for representing more than one source token.
    pub balanced_representation: i32,
    /// Reward or penalty based on the compact display-width range.
    pub display_width: i32,
    /// Penalty for information omitted by compacting the source.
    pub information_loss: i32,
    /// Penalty for trivial one-character output from a nontrivial source.
    pub trivial_alias: i32,
    /// Penalty for visibly redundant output.
    pub redundancy: i32,
    /// Penalty when the supplied used-alias set already contains the candidate.
    pub collision_pressure: i32,
    /// Stable family-level preference; hash fallbacks intentionally rank last.
    pub strategy_adjustment: i32,
}

impl ScoreComponents {
    #[must_use]
    pub const fn total(self) -> i32 {
        self.token_coverage
            + self.acronym_preservation
            + self.recognizable_prefix
            + self.balanced_representation
            + self.display_width
            + self.information_loss
            + self.trivial_alias
            + self.redundancy
            + self.collision_pressure
            + self.strategy_adjustment
    }
}

/// One safe, bounded, explainable alias candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasCandidate {
    alias: RepositoryAlias,
    strategy: AliasStrategy,
    score: i32,
    display_width: usize,
    components: ScoreComponents,
}

impl AliasCandidate {
    /// Returns the presentation-safe alias value.
    #[must_use]
    pub fn alias(&self) -> &RepositoryAlias {
        &self.alias
    }

    /// Returns the candidate family.
    #[must_use]
    pub const fn strategy(&self) -> AliasStrategy {
        self.strategy
    }

    /// Returns the integer total used for deterministic ordering.
    #[must_use]
    pub const fn score(&self) -> i32 {
        self.score
    }

    /// Returns the terminal display width of the alias.
    #[must_use]
    pub const fn display_width(&self) -> usize {
        self.display_width
    }

    /// Returns the individual integer scoring components.
    #[must_use]
    pub const fn components(&self) -> ScoreComponents {
        self.components
    }

    /// Returns a compact, non-sensitive summary suitable for future `alias explain` output.
    #[must_use]
    pub fn rationale(&self) -> String {
        format!(
            "strategy={} coverage={} acronym={} prefix={} balanced={} width={} loss={} collision={}",
            self.strategy.label(),
            self.components.token_coverage,
            self.components.acronym_preservation,
            self.components.recognizable_prefix,
            self.components.balanced_representation,
            self.components.display_width,
            self.components.information_loss,
            self.components.collision_pressure,
        )
    }
}

/// Pure deterministic adaptive naming engine; it has no persistence side effects.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AdaptiveNamingPolicy;

impl AdaptiveNamingPolicy {
    /// Returns the stable policy identifier.
    #[must_use]
    pub const fn policy_id() -> &'static str {
        ADAPTIVE_NAMING_POLICY_ID
    }

    /// Normalizes and tokenizes a display name using NFC and explicit style boundaries.
    #[must_use]
    pub fn analyze(display_name: &RepositoryDisplayName) -> NameAnalysis {
        analyze_name(display_name.as_str())
    }

    /// Returns ordered candidates, including deterministic hash fallbacks.
    ///
    /// `used_aliases` affects collision-pressure scoring but does not mutate it;
    /// callers can inspect the full explanation set before selecting one.
    #[must_use]
    pub fn candidates(
        display_name: &RepositoryDisplayName,
        identity: &CanonicalRepositoryIdentity,
        used_aliases: &BTreeSet<RepositoryAlias>,
    ) -> Vec<AliasCandidate> {
        let analysis = Self::analyze(display_name);
        let seeds = candidate_seeds(&analysis, identity);
        let (mut readable, hash_fallbacks): (Vec<_>, Vec<_>) = seeds
            .into_iter()
            .partition(|seed| seed.strategy != AliasStrategy::HashFallback);
        readable.truncate(MAX_CANDIDATES.saturating_sub(hash_fallbacks.len()));
        readable.extend(hash_fallbacks);
        let mut candidates = readable
            .into_iter()
            .filter_map(|seed| candidate_from_seed(&analysis, &seed, used_aliases))
            .collect::<Vec<_>>();
        candidates.sort_unstable_by(compare_candidates);
        let mut finalized = BTreeSet::new();
        candidates.retain(|candidate| finalized.insert(candidate.alias.clone()));
        candidates
    }

    /// Selects the first collision-free candidate from the stable ordering.
    #[must_use]
    pub fn select(
        display_name: &RepositoryDisplayName,
        identity: &CanonicalRepositoryIdentity,
        used_aliases: &BTreeSet<RepositoryAlias>,
    ) -> Option<AliasCandidate> {
        Self::candidates(display_name, identity, used_aliases)
            .into_iter()
            .find(|candidate| !used_aliases.contains(candidate.alias()))
    }
}

#[derive(Debug, Clone)]
struct CandidateSeed {
    value: String,
    strategy: AliasStrategy,
    covered_tokens: usize,
    preserved_acronyms: usize,
    prefix_graphemes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentKind {
    Lower,
    Upper,
    Digit,
    Cjk,
    Letter,
}

fn analyze_name(value: &str) -> NameAnalysis {
    let normalized_name = value.nfc().collect::<String>();
    let graphemes = normalized_name.graphemes(true).collect::<Vec<_>>();
    let mut hints = BTreeSet::new();
    if normalized_name != value {
        hints.insert(NameStyleHint::NfcNormalized);
    }
    if value.contains('-') {
        hints.insert(NameStyleHint::KebabCase);
    }
    if value.contains('_') {
        hints.insert(NameStyleHint::SnakeCase);
    }
    if value.contains('.') {
        hints.insert(NameStyleHint::DotSeparated);
    }
    if value.chars().any(char::is_whitespace) {
        hints.insert(NameStyleHint::SpaceSeparated);
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut previous = None;
    let mut saw_cjk = false;
    let mut saw_non_cjk = false;
    for (index, grapheme) in graphemes.iter().enumerate() {
        let Some(kind) = segment_kind(grapheme) else {
            flush_token(&mut current, &mut tokens);
            previous = None;
            continue;
        };
        let next = graphemes.get(index + 1).and_then(|next| segment_kind(next));
        if kind == SegmentKind::Cjk {
            saw_cjk = true;
            hints.insert(NameStyleHint::CjkRun);
        } else {
            saw_non_cjk = true;
        }
        if let Some(previous_kind) = previous
            && split_before(previous_kind, kind, next)
        {
            if previous_kind == SegmentKind::Lower && kind == SegmentKind::Upper {
                hints.insert(NameStyleHint::CamelCase);
            }
            if previous_kind == SegmentKind::Upper
                && kind == SegmentKind::Upper
                && next.is_some_and(|next| matches!(next, SegmentKind::Lower | SegmentKind::Letter))
            {
                hints.insert(NameStyleHint::AcronymBoundary);
            }
            if matches!(
                (previous_kind, kind),
                (
                    SegmentKind::Digit,
                    SegmentKind::Lower | SegmentKind::Upper | SegmentKind::Letter
                ) | (
                    SegmentKind::Lower | SegmentKind::Upper | SegmentKind::Letter,
                    SegmentKind::Digit
                )
            ) {
                hints.insert(NameStyleHint::LetterDigitBoundary);
            }
            flush_token(&mut current, &mut tokens);
        }
        if previous.is_none() && kind == SegmentKind::Upper {
            hints.insert(NameStyleHint::PascalCase);
        }
        current.push_str(grapheme);
        previous = Some(kind);
    }
    flush_token(&mut current, &mut tokens);
    if saw_cjk && saw_non_cjk {
        hints.insert(NameStyleHint::MixedCjkLatin);
    }
    NameAnalysis {
        normalized_name,
        tokens,
        style_hints: hints.into_iter().collect(),
    }
}

fn segment_kind(grapheme: &str) -> Option<SegmentKind> {
    let character = grapheme
        .chars()
        .find(|character| character.is_alphanumeric())?;
    if is_cjk(character) {
        Some(SegmentKind::Cjk)
    } else if character.is_numeric() {
        Some(SegmentKind::Digit)
    } else if character.is_lowercase() {
        Some(SegmentKind::Lower)
    } else if character.is_uppercase() {
        Some(SegmentKind::Upper)
    } else {
        Some(SegmentKind::Letter)
    }
}

fn is_cjk(character: char) -> bool {
    matches!(
        u32::from(character),
        0x3040..=0x30ff
            | 0x31f0..=0x31ff
            | 0x3400..=0x4dbf
            | 0x4e00..=0x9fff
            | 0xac00..=0xd7af
            | 0xf900..=0xfaff
            | 0x20000..=0x2ebef
    )
}

fn split_before(previous: SegmentKind, current: SegmentKind, next: Option<SegmentKind>) -> bool {
    if previous == SegmentKind::Cjk || current == SegmentKind::Cjk {
        return previous != current;
    }
    if previous == SegmentKind::Lower && current == SegmentKind::Upper {
        return true;
    }
    if matches!(
        (previous, current),
        (
            SegmentKind::Digit,
            SegmentKind::Lower | SegmentKind::Upper | SegmentKind::Letter
        ) | (
            SegmentKind::Lower | SegmentKind::Upper | SegmentKind::Letter,
            SegmentKind::Digit
        )
    ) {
        return true;
    }
    previous == SegmentKind::Upper
        && current == SegmentKind::Upper
        && next.is_some_and(|next| matches!(next, SegmentKind::Lower | SegmentKind::Letter))
}

fn flush_token(current: &mut String, tokens: &mut Vec<String>) {
    if !current.is_empty() {
        tokens.push(std::mem::take(current));
    }
}

fn candidate_seeds(
    analysis: &NameAnalysis,
    identity: &CanonicalRepositoryIdentity,
) -> Vec<CandidateSeed> {
    if analysis.tokens.is_empty() {
        let mut generated = Vec::new();
        let mut emitted_values = BTreeSet::new();
        append_seed(
            &mut generated,
            &mut emitted_values,
            CandidateSeed {
                value: "R".to_owned(),
                strategy: AliasStrategy::Initialism,
                covered_tokens: 0,
                preserved_acronyms: 0,
                prefix_graphemes: 1,
            },
        );
        for seed in hash_fallback_seeds("R", identity, 0) {
            append_seed(&mut generated, &mut emitted_values, seed);
        }
        return generated;
    }
    let mut generated = Vec::new();
    let mut emitted_values = BTreeSet::new();
    let token_count = analysis.tokens.len();
    append_initialism_seeds(&mut generated, &mut emitted_values, analysis, token_count);
    append_multi_token_seeds(&mut generated, &mut emitted_values, analysis, token_count);
    append_single_token_seeds(&mut generated, &mut emitted_values, analysis, token_count);
    append_unicode_prefix_seeds(&mut generated, &mut emitted_values, analysis);
    let hash_base = generated
        .first()
        .map_or_else(|| "R".to_owned(), |seed| seed.value.clone());
    for seed in hash_fallback_seeds(&hash_base, identity, token_count) {
        append_seed(&mut generated, &mut emitted_values, seed);
    }
    generated
}

fn append_initialism_seeds(
    generated: &mut Vec<CandidateSeed>,
    emitted_values: &mut BTreeSet<String>,
    analysis: &NameAnalysis,
    token_count: usize,
) {
    append_seed(
        generated,
        emitted_values,
        CandidateSeed {
            value: initials(&analysis.tokens),
            strategy: AliasStrategy::Initialism,
            covered_tokens: token_count,
            preserved_acronyms: 0,
            prefix_graphemes: 1,
        },
    );
    let preserved_acronyms = analysis
        .tokens
        .iter()
        .filter(|token| is_acronym(token))
        .count();
    if preserved_acronyms > 0 {
        let value = analysis
            .tokens
            .iter()
            .map(|token| {
                if is_acronym(token) {
                    uppercase(token)
                } else {
                    token_representative(token)
                }
            })
            .collect();
        append_seed(
            generated,
            emitted_values,
            CandidateSeed {
                value,
                strategy: AliasStrategy::AcronymAware,
                covered_tokens: token_count,
                preserved_acronyms,
                prefix_graphemes: 1,
            },
        );
    }
}

fn append_multi_token_seeds(
    generated: &mut Vec<CandidateSeed>,
    emitted_values: &mut BTreeSet<String>,
    analysis: &NameAnalysis,
    token_count: usize,
) {
    if token_count <= 1 {
        return;
    }
    for target_width in [4, 6, 8] {
        append_seed(
            generated,
            emitted_values,
            CandidateSeed {
                value: balanced_prefix(&analysis.tokens, target_width),
                strategy: AliasStrategy::BalancedPrefix,
                covered_tokens: token_count,
                preserved_acronyms: 0,
                prefix_graphemes: 2,
            },
        );
    }
    let value = format!(
        "{}-{}",
        prefix_by_graphemes(&analysis.tokens[0], 2),
        analysis
            .tokens
            .iter()
            .skip(1)
            .map(|token| token_representative(token))
            .collect::<String>()
    );
    append_seed(
        generated,
        emitted_values,
        CandidateSeed {
            value,
            strategy: AliasStrategy::TokenCompression,
            covered_tokens: token_count,
            preserved_acronyms: 0,
            prefix_graphemes: 2,
        },
    );
}

fn append_single_token_seeds(
    generated: &mut Vec<CandidateSeed>,
    emitted_values: &mut BTreeSet<String>,
    analysis: &NameAnalysis,
    token_count: usize,
) {
    if token_count != 1 {
        return;
    }
    let token = &analysis.tokens[0];
    for grapheme_count in [3, 4, 6, 8] {
        append_seed(
            generated,
            emitted_values,
            CandidateSeed {
                value: prefix_by_graphemes(token, grapheme_count),
                strategy: AliasStrategy::SingleTokenReadablePrefix,
                covered_tokens: 1,
                preserved_acronyms: usize::from(is_acronym(token)),
                prefix_graphemes: grapheme_count,
            },
        );
    }
    if let Some(value) = consonant_skeleton(token) {
        append_seed(
            generated,
            emitted_values,
            CandidateSeed {
                value,
                strategy: AliasStrategy::ConsonantSkeleton,
                covered_tokens: 1,
                preserved_acronyms: 0,
                prefix_graphemes: 0,
            },
        );
    }
}

fn append_unicode_prefix_seeds(
    generated: &mut Vec<CandidateSeed>,
    emitted_values: &mut BTreeSet<String>,
    analysis: &NameAnalysis,
) {
    for token in analysis.tokens.iter().filter(|token| contains_cjk(token)) {
        for width in [4, 6, 8] {
            let value = prefix_by_display_width(token, width);
            let prefix_graphemes = value.graphemes(true).count();
            append_seed(
                generated,
                emitted_values,
                CandidateSeed {
                    value,
                    strategy: AliasStrategy::UnicodePrefix,
                    covered_tokens: 1,
                    preserved_acronyms: 0,
                    prefix_graphemes,
                },
            );
        }
    }
}

fn append_seed(
    generated: &mut Vec<CandidateSeed>,
    emitted_values: &mut BTreeSet<String>,
    mut candidate_seed: CandidateSeed,
) {
    candidate_seed.value = normalize_alias_value(&candidate_seed.value);
    if !candidate_seed.value.is_empty() && emitted_values.insert(candidate_seed.value.clone()) {
        generated.push(candidate_seed);
    }
}

fn hash_fallback_seeds(
    base: &str,
    identity: &CanonicalRepositoryIdentity,
    covered_tokens: usize,
) -> Vec<CandidateSeed> {
    let digest = format!("{:x}", Sha256::digest(identity.as_str().as_bytes()));
    [6_usize, 8, 10, 12, 14, 16]
        .into_iter()
        .filter_map(|suffix_length| {
            let suffix = digest.get(..suffix_length)?;
            let prefix_width = MAX_ALIAS_DISPLAY_WIDTH.saturating_sub(suffix_length + 1);
            let prefix = prefix_by_display_width(base, prefix_width.max(1));
            Some(CandidateSeed {
                value: format!("{prefix}-{suffix}"),
                strategy: AliasStrategy::HashFallback,
                covered_tokens,
                preserved_acronyms: 0,
                prefix_graphemes: 0,
            })
        })
        .collect()
}

fn candidate_from_seed(
    analysis: &NameAnalysis,
    seed: &CandidateSeed,
    used_aliases: &BTreeSet<RepositoryAlias>,
) -> Option<AliasCandidate> {
    let value = limit_alias(&seed.value);
    let alias = RepositoryAlias::new(value).ok()?;
    let display_width = UnicodeWidthStr::width(alias.as_str());
    if display_width > MAX_ALIAS_DISPLAY_WIDTH {
        return None;
    }
    let components = score_components(analysis, seed, &alias, display_width, used_aliases);
    Some(AliasCandidate {
        alias,
        strategy: seed.strategy,
        score: components.total(),
        display_width,
        components,
    })
}

fn score_components(
    analysis: &NameAnalysis,
    seed: &CandidateSeed,
    alias: &RepositoryAlias,
    display_width: usize,
    used_aliases: &BTreeSet<RepositoryAlias>,
) -> ScoreComponents {
    let token_count = analysis.tokens.len().max(1);
    let token_coverage = i32::try_from(seed.covered_tokens.min(token_count) * 48 / token_count)
        .expect("small bounded score");
    let acronym_preservation =
        i32::try_from(seed.preserved_acronyms * 12).expect("small bounded score");
    let recognizable_prefix = if seed.prefix_graphemes >= 3 { 12 } else { 0 };
    let balanced_representation = if seed.covered_tokens > 1 { 10 } else { 0 };
    let display_width = display_width_score(display_width);
    let source_width = UnicodeWidthStr::width(analysis.normalized_name.as_str());
    let loss = source_width.saturating_sub(UnicodeWidthStr::width(alias.as_str()));
    let information_loss = -i32::try_from(loss.min(24)).expect("bounded loss score");
    let source_nontrivial =
        analysis.tokens.len() > 1 || analysis.normalized_name.graphemes(true).count() > 1;
    let trivial_alias = if source_nontrivial && UnicodeWidthStr::width(alias.as_str()) <= 1 {
        -55
    } else {
        0
    };
    let redundancy = if is_redundant(alias.as_str()) { -8 } else { 0 };
    let collision_pressure = if used_aliases.contains(alias) { -80 } else { 0 };
    let strategy_adjustment = match seed.strategy {
        AliasStrategy::AcronymAware => 15,
        AliasStrategy::Initialism => 14,
        AliasStrategy::SingleTokenReadablePrefix => 12,
        AliasStrategy::UnicodePrefix => 11,
        AliasStrategy::BalancedPrefix => 5,
        AliasStrategy::TokenCompression => 3,
        AliasStrategy::ConsonantSkeleton => 1,
        AliasStrategy::HashFallback => -200,
    };
    ScoreComponents {
        token_coverage,
        acronym_preservation,
        recognizable_prefix,
        balanced_representation,
        display_width,
        information_loss,
        trivial_alias,
        redundancy,
        collision_pressure,
        strategy_adjustment,
    }
}

fn display_width_score(width: usize) -> i32 {
    if (TARGET_MIN_DISPLAY_WIDTH..=TARGET_MAX_DISPLAY_WIDTH).contains(&width) {
        20
    } else if width < TARGET_MIN_DISPLAY_WIDTH {
        -i32::try_from((TARGET_MIN_DISPLAY_WIDTH - width) * 5).expect("small width score")
    } else {
        -i32::try_from((width - TARGET_MAX_DISPLAY_WIDTH) * 5).expect("small width score")
    }
}

fn is_redundant(value: &str) -> bool {
    let mut graphemes = value.graphemes(true);
    let Some(first) = graphemes.next() else {
        return false;
    };
    let mut count = 1_usize;
    for grapheme in graphemes {
        if grapheme != first {
            return false;
        }
        count += 1;
    }
    count > 2
}

fn compare_candidates(left: &AliasCandidate, right: &AliasCandidate) -> std::cmp::Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| width_distance(left.display_width).cmp(&width_distance(right.display_width)))
        .then_with(|| left.strategy.priority().cmp(&right.strategy.priority()))
        .then_with(|| left.alias.as_str().cmp(right.alias.as_str()))
}

fn width_distance(width: usize) -> usize {
    if width < TARGET_MIN_DISPLAY_WIDTH {
        TARGET_MIN_DISPLAY_WIDTH - width
    } else {
        width.saturating_sub(TARGET_MAX_DISPLAY_WIDTH)
    }
}

fn initials(tokens: &[String]) -> String {
    tokens
        .iter()
        .map(|token| token_representative(token))
        .collect()
}

fn token_representative(token: &str) -> String {
    if token.chars().all(char::is_numeric) {
        prefix_by_graphemes(token, 4)
    } else {
        first_grapheme_upper(token)
    }
}

fn first_grapheme_upper(value: &str) -> String {
    value
        .graphemes(true)
        .next()
        .map_or_else(String::new, uppercase)
}

fn uppercase(value: &str) -> String {
    value.chars().flat_map(char::to_uppercase).collect()
}

fn prefix_by_graphemes(value: &str, count: usize) -> String {
    uppercase(&value.graphemes(true).take(count).collect::<String>())
}

fn prefix_by_display_width(value: &str, max_width: usize) -> String {
    let mut output = String::new();
    let mut width = 0_usize;
    for grapheme in value.graphemes(true) {
        let upper = uppercase(grapheme);
        let grapheme_width = UnicodeWidthStr::width(upper.as_str());
        if width + grapheme_width > max_width {
            break;
        }
        width += grapheme_width;
        output.push_str(&upper);
    }
    output
}

fn balanced_prefix(tokens: &[String], target_width: usize) -> String {
    let graphemes = tokens
        .iter()
        .map(|token| token.graphemes(true).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let mut lengths = tokens
        .iter()
        .map(|token| {
            if token.chars().all(char::is_numeric) {
                token.graphemes(true).count().min(4)
            } else {
                1
            }
        })
        .collect::<Vec<_>>();
    let mut cursor = 0_usize;
    loop {
        let rendered = graphemes
            .iter()
            .zip(&lengths)
            .map(|(token, length)| {
                uppercase(&token.iter().take(*length).copied().collect::<String>())
            })
            .collect::<String>();
        let rendered_width = UnicodeWidthStr::width(rendered.as_str());
        if rendered_width >= target_width || rendered_width >= MAX_ALIAS_DISPLAY_WIDTH {
            return rendered;
        }
        let mut advanced = false;
        for offset in 0..graphemes.len() {
            let index = (cursor + offset) % graphemes.len();
            if lengths[index] < graphemes[index].len() {
                lengths[index] += 1;
                cursor = (index + 1) % graphemes.len();
                advanced = true;
                break;
            }
        }
        if !advanced {
            return rendered;
        }
    }
}

fn consonant_skeleton(token: &str) -> Option<String> {
    if !token
        .chars()
        .all(|character| character.is_ascii_alphanumeric())
    {
        return None;
    }
    let mut skeleton = String::new();
    let mut last_emitted = None;
    for character in token.chars().filter(|character| {
        character.is_ascii_digit()
            || !matches!(character.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u')
    }) {
        let upper = character.to_ascii_uppercase();
        if last_emitted != Some(upper) {
            skeleton.push(upper);
            last_emitted = Some(upper);
        }
    }
    (skeleton.graphemes(true).count() >= 3).then_some(skeleton)
}

fn is_acronym(token: &str) -> bool {
    let letters = token
        .chars()
        .filter(|character| character.is_alphabetic())
        .collect::<Vec<_>>();
    letters.len() >= 2 && letters.iter().all(|character| character.is_uppercase())
}

fn contains_cjk(value: &str) -> bool {
    value.chars().any(is_cjk)
}

fn normalize_alias_value(value: &str) -> String {
    value.chars().fold(String::new(), |mut output, character| {
        if is_combining_mark(character) {
            use std::fmt::Write as _;
            let _ = write!(output, "M{:X}", u32::from(character));
        } else if character.is_alphanumeric() || character == '-' {
            output.push(character);
        }
        output
    })
}

fn limit_alias(value: &str) -> String {
    let mut output = String::new();
    let mut width = 0_usize;
    let mut chars = 0_usize;
    for grapheme in value.graphemes(true) {
        let grapheme_chars = grapheme.chars().count();
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if chars + grapheme_chars > 20 || width + grapheme_width > MAX_ALIAS_DISPLAY_WIDTH {
            break;
        }
        chars += grapheme_chars;
        width += grapheme_width;
        output.push_str(grapheme);
    }
    output.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        ADAPTIVE_NAMING_POLICY_ID, AdaptiveNamingPolicy, AliasCandidate, AliasStrategy,
        NameStyleHint, prefix_by_display_width,
    };
    use crate::repo::{CanonicalRepositoryIdentity, RepositoryAlias, RepositoryDisplayName};
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    fn display(value: &str) -> RepositoryDisplayName {
        RepositoryDisplayName::new(value).expect("valid display fixture")
    }

    fn identity(value: &str) -> CanonicalRepositoryIdentity {
        CanonicalRepositoryIdentity::new(format!("remote:example/{value}"))
            .expect("valid identity fixture")
    }

    #[test]
    fn policy_is_offline_deterministic_and_explainable() {
        let display = display("OpenCodeWorkspaceHub");
        let identity = identity("opencode-workspace-hub");
        let first = AdaptiveNamingPolicy::candidates(&display, &identity, &BTreeSet::new());
        let second = AdaptiveNamingPolicy::candidates(&display, &identity, &BTreeSet::new());
        assert_eq!(AdaptiveNamingPolicy::policy_id(), ADAPTIVE_NAMING_POLICY_ID);
        assert_eq!(first, second);
        assert!(
            first
                .iter()
                .all(|candidate| !candidate.rationale().is_empty())
        );
        assert!(
            first
                .iter()
                .all(|candidate| candidate.score() == candidate.components().total())
        );
    }

    #[test]
    fn tokenizes_declared_styles_and_acronym_boundaries() {
        let analysis = AdaptiveNamingPolicy::analyze(&display("XMLHttpRequest-v2_航天器"));
        assert_eq!(
            analysis.tokens(),
            ["XML", "Http", "Request", "v", "2", "航天器"]
        );
        assert!(
            analysis
                .style_hints()
                .contains(&NameStyleHint::AcronymBoundary)
        );
        assert!(
            analysis
                .style_hints()
                .contains(&NameStyleHint::LetterDigitBoundary)
        );
        assert!(analysis.style_hints().contains(&NameStyleHint::CjkRun));
        assert!(
            analysis
                .style_hints()
                .contains(&NameStyleHint::MixedCjkLatin)
        );
    }

    #[test]
    fn nfc_and_graphemes_are_preserved_without_split_output() {
        let composed = display("café-tools");
        let decomposed = display("cafe\u{301}-tools");
        let identity = identity("cafe-tools");
        let composed_analysis = AdaptiveNamingPolicy::analyze(&composed);
        let decomposed_analysis = AdaptiveNamingPolicy::analyze(&decomposed);
        assert_eq!(
            composed_analysis.normalized_name(),
            decomposed_analysis.normalized_name()
        );
        assert_eq!(composed_analysis.tokens(), decomposed_analysis.tokens());
        let candidates = AdaptiveNamingPolicy::candidates(&decomposed, &identity, &BTreeSet::new());
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.display_width() <= 20)
        );
        let combining = prefix_by_display_width("e\u{301}clair", 1);
        assert_eq!(combining, "E\u{301}");
        assert_eq!(combining.graphemes(true).count(), 1);
        let zwj = "👩\u{200d}💻";
        assert_eq!(
            prefix_by_display_width(&format!("{zwj}tools"), UnicodeWidthStr::width(zwj)),
            zwj
        );
    }

    #[test]
    fn nontrivial_single_tokens_are_not_selected_as_one_character() {
        let candidate = AdaptiveNamingPolicy::select(
            &display("tabbeacon"),
            &identity("tabbeacon"),
            &BTreeSet::new(),
        )
        .expect("candidate exists");
        assert!(candidate.display_width() >= 3);
        assert_ne!(candidate.alias().as_str(), "T");
    }

    #[test]
    fn declared_candidate_families_are_generated_without_unbounded_search() {
        let used = BTreeSet::new();
        let multi = AdaptiveNamingPolicy::candidates(
            &display("opencode-workspace-hub"),
            &identity("opencode-workspace-hub"),
            &used,
        );
        let multi_strategies = multi
            .iter()
            .map(AliasCandidate::strategy)
            .collect::<BTreeSet<_>>();
        assert!(multi_strategies.contains(&AliasStrategy::Initialism));
        assert!(multi_strategies.contains(&AliasStrategy::BalancedPrefix));
        assert!(multi_strategies.contains(&AliasStrategy::TokenCompression));
        assert!(multi_strategies.contains(&AliasStrategy::HashFallback));

        let single = AdaptiveNamingPolicy::candidates(
            &display("tabbeacon"),
            &identity("tabbeacon-families"),
            &used,
        );
        assert!(
            single
                .iter()
                .any(|candidate| candidate.strategy() == AliasStrategy::SingleTokenReadablePrefix)
        );
        assert!(
            single
                .iter()
                .any(|candidate| candidate.strategy() == AliasStrategy::ConsonantSkeleton)
        );
        assert!(
            single
                .iter()
                .any(|candidate| candidate.alias().as_str() == "TAB")
        );
        assert!(
            single
                .iter()
                .any(|candidate| candidate.alias().as_str() == "TBCN")
        );

        let acronym = AdaptiveNamingPolicy::candidates(
            &display("XMLHttpRequest"),
            &identity("xml-http-request"),
            &used,
        );
        assert!(
            acronym
                .iter()
                .any(|candidate| candidate.strategy() == AliasStrategy::AcronymAware)
        );
    }

    #[test]
    fn cjk_prefixes_fit_display_width_and_hash_fallback_stays_last() {
        let display = display("航天器设计工具");
        let identity = identity("spacecraft-design");
        let candidates = AdaptiveNamingPolicy::candidates(&display, &identity, &BTreeSet::new());
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.strategy() == AliasStrategy::UnicodePrefix)
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.display_width() <= 20)
        );
        assert_eq!(
            candidates.last().expect("candidates exist").strategy(),
            AliasStrategy::HashFallback
        );
    }

    #[test]
    fn used_aliases_are_skipped_and_hash_fallback_is_stable() {
        let display = display("jerry-proxy-control");
        let identity = identity("jerry-proxy-control");
        let initial = AdaptiveNamingPolicy::candidates(&display, &identity, &BTreeSet::new());
        let used = initial
            .iter()
            .filter(|candidate| candidate.strategy() != AliasStrategy::HashFallback)
            .map(|candidate| candidate.alias().clone())
            .collect::<BTreeSet<RepositoryAlias>>();
        let selected = AdaptiveNamingPolicy::select(&display, &identity, &used)
            .expect("stable hash fallback remains");
        assert_eq!(selected.strategy(), AliasStrategy::HashFallback);
        assert!(!used.contains(selected.alias()));
        let mut first_hash_used = used;
        first_hash_used.insert(selected.alias().clone());
        let next = AdaptiveNamingPolicy::select(&display, &identity, &first_hash_used)
            .expect("next deterministic hash fallback remains");
        assert_eq!(next.strategy(), AliasStrategy::HashFallback);
        assert_ne!(next.alias(), selected.alias());
    }

    #[test]
    fn large_cjk_seed_sets_keep_hash_fallbacks_and_final_aliases_unique() {
        let source = (0_u32..30)
            .map(|index| {
                (0_u32..4)
                    .map(|offset| {
                        char::from_u32(0x4e00 + index * 4 + offset)
                            .expect("CJK fixture scalar is valid")
                    })
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("-");
        let display = display(&source);
        let identity = identity("large-cjk-seed-set");
        let candidates = AdaptiveNamingPolicy::candidates(&display, &identity, &BTreeSet::new());
        let aliases = candidates
            .iter()
            .map(|candidate| candidate.alias().clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(aliases.len(), candidates.len());
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.strategy() == AliasStrategy::HashFallback)
        );
    }

    #[test]
    fn all_used_candidates_return_none_without_registry_state() {
        let display = display("collision-safe-name");
        let identity = identity("collision-safe-name");
        let used = AdaptiveNamingPolicy::candidates(&display, &identity, &BTreeSet::new())
            .into_iter()
            .map(|candidate| candidate.alias().clone())
            .collect::<BTreeSet<_>>();
        assert!(AdaptiveNamingPolicy::select(&display, &identity, &used).is_none());
    }

    #[test]
    fn non_composable_combining_marks_remain_distinguishable() {
        let display = display("a\u{0315}lab");
        let candidates = AdaptiveNamingPolicy::candidates(
            &display,
            &identity("combining-mark"),
            &BTreeSet::new(),
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.alias().as_str().contains("M315"))
        );
    }

    #[test]
    fn all_unicode_mark_categories_remain_distinguishable() {
        assert!(unicode_normalization::char::is_combining_mark('\u{064b}'));
        let display = display("a\u{064b}lab");
        let candidates = AdaptiveNamingPolicy::candidates(
            &display,
            &identity("arabic-combining-mark"),
            &BTreeSet::new(),
        );
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.alias().as_str().contains("M64B")),
            "aliases={:?}",
            candidates
                .iter()
                .map(|candidate| candidate.alias().as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn punctuation_only_display_names_keep_the_readable_r_fallback() {
        let display = display("---");
        let identity = identity("punctuation-only");
        let candidates = AdaptiveNamingPolicy::candidates(&display, &identity, &BTreeSet::new());
        assert!(candidates.iter().any(|candidate| {
            candidate.alias().as_str() == "R" && candidate.strategy() == AliasStrategy::Initialism
        }));
        assert_eq!(
            AdaptiveNamingPolicy::select(&display, &identity, &BTreeSet::new())
                .expect("readable fallback candidate")
                .alias()
                .as_str(),
            "R"
        );
    }
}
