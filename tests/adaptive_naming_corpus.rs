use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use tabbeacon::repo::{
    AdaptiveNamingPolicy, AliasCandidate, AliasStrategy, CanonicalRepositoryIdentity, NameAnalysis,
    NameStyleHint, RepositoryAlias, RepositoryDisplayName, StableAliasRegistry,
};

const CORPUS: &str = include_str!("fixtures/adaptive-naming-v2/corpus-v1.json");
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Deserialize)]
struct NamingCorpus {
    schema: String,
    version: u32,
    cases: Vec<NamingCase>,
}

#[derive(Debug, Deserialize)]
struct NamingCase {
    id: String,
    family: String,
    raw: String,
    identity: String,
    #[serde(default)]
    tokens: Vec<String>,
    #[serde(default)]
    hints: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    used_aliases: Vec<String>,
    #[serde(default)]
    selected_nontrivial: bool,
    #[serde(default)]
    all_readable_used: bool,
    #[serde(default)]
    group: Option<String>,
}

#[test]
fn adaptive_naming_corpus_v1_is_versioned_deterministic_and_offline() {
    let corpus: NamingCorpus = serde_json::from_str(CORPUS).expect("valid naming corpus JSON");
    assert_corpus_shape(&corpus);
    let mut identifiers = BTreeSet::new();
    let mut grouped_initialisms = BTreeMap::<String, String>::new();
    for case in &corpus.cases {
        assert!(
            identifiers.insert(case.id.clone()),
            "duplicate corpus id: {}",
            case.id
        );
        validate_case(case, &mut grouped_initialisms);
    }
}

fn assert_corpus_shape(corpus: &NamingCorpus) {
    assert_eq!(corpus.schema, "tabbeacon.adaptive-naming-corpus");
    assert_eq!(corpus.version, 1);
    assert!(
        corpus.cases.len() >= 50,
        "G52 requires at least 50 corpus cases"
    );

    let required_families = BTreeSet::from([
        "legacy",
        "styles",
        "single",
        "token-count",
        "acronym-digit",
        "collision",
        "cjk",
        "mixed",
        "unicode",
        "width",
    ]);
    let actual_families = corpus
        .cases
        .iter()
        .map(|case| case.family.as_str())
        .collect::<BTreeSet<_>>();
    assert!(required_families.is_subset(&actual_families));
}

fn validate_case(case: &NamingCase, grouped_initialisms: &mut BTreeMap<String, String>) {
    let display = RepositoryDisplayName::new(&case.raw)
        .unwrap_or_else(|error| panic!("invalid display fixture {}: {error}", case.id));
    let identity = CanonicalRepositoryIdentity::new(&case.identity)
        .unwrap_or_else(|error| panic!("invalid identity fixture {}: {error}", case.id));
    let used = parse_used_aliases(case);
    let analysis = AdaptiveNamingPolicy::analyze(&display);
    assert_analysis_expectations(case, &analysis);
    let candidates = AdaptiveNamingPolicy::candidates(&display, &identity, &used);
    assert_candidate_expectations(case, &display, &identity, &used, &candidates);
    assert_selection_expectations(case, &display, &identity, &used, &candidates);
    record_equivalence_group(case, &candidates, grouped_initialisms);
}

fn parse_used_aliases(case: &NamingCase) -> BTreeSet<RepositoryAlias> {
    case.used_aliases
        .iter()
        .map(|alias| {
            RepositoryAlias::new(alias)
                .unwrap_or_else(|error| panic!("invalid used alias fixture {}: {error}", case.id))
        })
        .collect()
}

fn assert_analysis_expectations(case: &NamingCase, analysis: &NameAnalysis) {
    if !case.tokens.is_empty() {
        assert_eq!(
            analysis.tokens(),
            case.tokens,
            "token mismatch: {}",
            case.id
        );
    }
    for hint in &case.hints {
        assert!(
            analysis.style_hints().contains(&parse_hint(hint)),
            "missing style hint {hint} for {}",
            case.id
        );
    }
}

fn assert_candidate_expectations(
    case: &NamingCase,
    display: &RepositoryDisplayName,
    identity: &CanonicalRepositoryIdentity,
    used: &BTreeSet<RepositoryAlias>,
    candidates: &[AliasCandidate],
) {
    assert!(!candidates.is_empty(), "no candidates for {}", case.id);
    assert_eq!(
        candidates,
        AdaptiveNamingPolicy::candidates(display, identity, used),
        "non-deterministic candidates for {}",
        case.id
    );
    let aliases = candidates
        .iter()
        .map(|candidate| candidate.alias().clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        aliases.len(),
        candidates.len(),
        "duplicate alias: {}",
        case.id
    );
    assert!(candidates.iter().all(|candidate| {
        candidate.display_width() <= 20 && candidate.score() == candidate.components().total()
    }));
    assert_eq!(
        candidates.last().expect("candidate exists").strategy(),
        AliasStrategy::HashFallback,
        "hash fallback must stay last: {}",
        case.id
    );
    for alias in &case.aliases {
        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.alias().as_str() == alias),
            "required alias {alias} missing for {}",
            case.id
        );
    }
}

fn assert_selection_expectations(
    case: &NamingCase,
    display: &RepositoryDisplayName,
    identity: &CanonicalRepositoryIdentity,
    used: &BTreeSet<RepositoryAlias>,
    candidates: &[AliasCandidate],
) {
    let selected = AdaptiveNamingPolicy::select(display, identity, used);
    if case.selected_nontrivial {
        assert!(
            selected
                .as_ref()
                .is_some_and(|candidate| candidate.display_width() >= 2),
            "trivial selection for {}",
            case.id
        );
    }
    if case.all_readable_used {
        let all_readable_used = candidates
            .iter()
            .filter(|candidate| candidate.strategy() != AliasStrategy::HashFallback)
            .map(|candidate| candidate.alias().clone())
            .collect::<BTreeSet<_>>();
        let fallback = AdaptiveNamingPolicy::select(display, identity, &all_readable_used)
            .expect("hash fallback is available after readable candidates");
        assert_eq!(
            fallback.strategy(),
            AliasStrategy::HashFallback,
            "{}",
            case.id
        );
    }
}

fn record_equivalence_group(
    case: &NamingCase,
    candidates: &[AliasCandidate],
    grouped_initialisms: &mut BTreeMap<String, String>,
) {
    let Some(group) = &case.group else {
        return;
    };
    let initialism = candidates
        .iter()
        .find(|candidate| candidate.strategy() == AliasStrategy::Initialism)
        .expect("every corpus case retains an initialism")
        .alias()
        .as_str()
        .to_owned();
    if let Some(previous) = grouped_initialisms.insert(group.clone(), initialism.clone()) {
        assert_eq!(previous, initialism, "equivalence mismatch for {group}");
    }
}

fn parse_hint(value: &str) -> NameStyleHint {
    match value {
        "kebab_case" => NameStyleHint::KebabCase,
        "snake_case" => NameStyleHint::SnakeCase,
        "dot_separated" => NameStyleHint::DotSeparated,
        "space_separated" => NameStyleHint::SpaceSeparated,
        "camel_case" => NameStyleHint::CamelCase,
        "pascal_case" => NameStyleHint::PascalCase,
        "acronym_boundary" => NameStyleHint::AcronymBoundary,
        "letter_digit_boundary" => NameStyleHint::LetterDigitBoundary,
        "cjk_run" => NameStyleHint::CjkRun,
        "mixed_cjk_latin" => NameStyleHint::MixedCjkLatin,
        "nfc_normalized" => NameStyleHint::NfcNormalized,
        other => panic!("unknown corpus style hint: {other}"),
    }
}

#[test]
fn adaptive_policy_does_not_mutate_existing_registry_state() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after Unix epoch")
        .as_nanos();
    let root = env::temp_dir().join(format!(
        "tabbeacon-g52-pure-policy-{}-{nonce}",
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let state = root.join("state");
    let registry = StableAliasRegistry::new(&state);
    let existing_identity = CanonicalRepositoryIdentity::new("remote:corpus.invalid/existing")
        .expect("existing identity is valid");
    let existing_name =
        RepositoryDisplayName::new("existing-repository").expect("existing display name is valid");
    registry
        .resolve(&existing_identity, &existing_name)
        .expect("temporary legacy registry is initialized");
    let before = registry_files(&state);

    let display = RepositoryDisplayName::new("adaptive-workspace-v2")
        .expect("adaptive display name is valid");
    let identity = CanonicalRepositoryIdentity::new("remote:corpus.invalid/adaptive-workspace-v2")
        .expect("adaptive identity is valid");
    let _ = AdaptiveNamingPolicy::candidates(&display, &identity, &BTreeSet::new());
    let _ = AdaptiveNamingPolicy::select(&display, &identity, &BTreeSet::new());

    assert_eq!(before, registry_files(&state));
    fs::remove_dir_all(root).expect("temporary registry is removed");
}

fn registry_files(root: &std::path::Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(root)
        .expect("registry root reads")
        .map(|entry| {
            let entry = entry.expect("registry entry reads");
            let name = entry.file_name().to_string_lossy().into_owned();
            let bytes = fs::read(entry.path()).expect("registry file reads");
            (name, bytes)
        })
        .collect()
}
