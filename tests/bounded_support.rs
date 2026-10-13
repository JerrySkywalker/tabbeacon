//! Read-only CLI Help and fixed bilingual risk transport. No product stores or writers.
use std::process::Command;
use tabbeacon::{
    human_output::{HumanTone, style},
    human_presentation::{HumanMessageKey, ResolvedLocale, catalog},
};

#[test]
fn explicit_write_help_exposes_external_race_and_crash_limits() {
    for args in [
        ["title-policy", "repair", "--help"],
        ["title-policy", "restore", "--help"],
        ["alias", "set", "--help"],
        ["alias", "reset", "--help"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_tabbeacon"))
            .args(args)
            .env("NO_COLOR", "1")
            .output()
            .unwrap();
        assert!(output.status.success());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains("external edits"), "{args:?}: {help}");
        assert!(help.contains("writer races"));
        assert!(help.contains("crash atomicity are unsupported"));
        assert!(!help.contains('\u{1b}'));
        assert!(!help.contains("FULL_CAS_PASS"));
    }
}

#[test]
fn localized_write_limits_remain_textual_and_private_data_free_without_color() {
    for (locale, refusal, limitation, evidence) in [
        (
            ResolvedLocale::EnUs,
            "drift is refused",
            "not atomic",
            "keeps evidence",
        ),
        (
            ResolvedLocale::ZhCn,
            "漂移即拒绝",
            "不保证原子性",
            "保留证据",
        ),
    ] {
        let notice = catalog(locale, HumanMessageKey::BoundedWriteRisk);
        let output = style(HumanTone::Attention, notice, false);
        assert!(output.contains(refusal));
        assert!(output.contains(limitation));
        assert!(output.contains(evidence));
        assert!(!output.contains('\u{1b}'));
        for sensitive in ["C:\\Users", "token=", "command=", "payload=", "TEST_PASS"] {
            assert!(!output.contains(sensitive));
        }
    }
}
