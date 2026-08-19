//! Inline, scrollback-preserving selection flow for guided setup.

use crate::settings::{
    ActivityMode, PresentationSettings, PresentationTheme, SpinnerPreset, TabColorMode, TitleMode,
};
use crate::{
    human_presentation::ResolvedLocale,
    interface_preferences::{HumanColor, InterfaceLanguage, InterfacePreferences},
};

/// Selection boundary that keeps the setup flow deterministic in tests.
pub trait GuidedInput {
    /// Selects one visible item without asking callers to type internal enums.
    ///
    /// # Errors
    ///
    /// Returns a bounded input-adapter error when no visible choice can be read.
    fn select(&mut self, prompt: &str, items: &[&str], default: usize) -> Result<usize, String>;
}

/// Resolves a complete presentation draft without persisting any state.
///
/// Presets are atomic: choosing `Use this preset` returns immediately without
/// reopening each individual appearance field.
///
/// # Errors
///
/// Returns a bounded selection error when the input adapter is interrupted or
/// returns an index outside the offered visible choices.
pub fn choose_presentation(
    input: &mut impl GuidedInput,
    current: PresentationSettings,
) -> Result<Option<PresentationSettings>, String> {
    loop {
        let choice = input.select(
            "Choose presentation",
            &[
                "Recommended",
                "Minimal",
                "Full",
                "Native",
                "Customize",
                "Back",
            ],
            0,
        )?;
        match choice {
            0..=3 => {
                let preset = match choice {
                    0 => "balanced",
                    1 => "minimal",
                    2 => "full",
                    3 => "native",
                    _ => unreachable!("preset choice is bounded"),
                };
                let draft = PresentationSettings::preset(preset)
                    .ok_or_else(|| "guided setup preset is unavailable".to_owned())?;
                match input.select("Preset", &["Use this preset", "Customize", "Back"], 0)? {
                    0 => return Ok(Some(draft)),
                    1 => return customize(input, draft).map(Some),
                    2 => {}
                    _ => return Err("invalid preset decision".to_owned()),
                }
            }
            4 => return customize(input, current).map(Some),
            5 => return Ok(None),
            _ => return Err("invalid presentation choice".to_owned()),
        }
    }
}

/// Resolves the bounded Interface draft before any normal setup summary is
/// rendered. The initial language selector is intentionally bilingual so a
/// fresh user never needs to understand the current locale to proceed.
///
/// `revisit` is true for fresh setup and explicit full setup. A returning
/// quick setup can preserve its current valid Interface draft without an
/// unnecessary prompt.
///
/// # Errors
///
/// Returns a bounded selection error when the input adapter is interrupted or
/// returns an index outside the offered visible choices.
pub fn choose_interface_preferences(
    input: &mut impl GuidedInput,
    current: InterfacePreferences,
    revisit: bool,
    auto_locale: ResolvedLocale,
) -> Result<Option<InterfacePreferences>, String> {
    if !revisit {
        return Ok(Some(current));
    }
    let language = match input.select(
        "Language / 语言",
        &["Auto / 自动", "简体中文", "English", "Back / 返回"],
        language_index(current.language()),
    )? {
        0 => InterfaceLanguage::Auto,
        1 => InterfaceLanguage::ZhCn,
        2 => InterfaceLanguage::EnUs,
        3 => return Ok(None),
        _ => return Err("invalid Interface language choice".to_owned()),
    };
    let locale = concrete_locale(language, auto_locale);
    let color = match input.select(
        label(locale, "Color", "颜色"),
        color_choices(locale),
        color_index(current.color()),
    )? {
        0 => HumanColor::Auto,
        1 => HumanColor::Always,
        2 => HumanColor::Never,
        _ => return Err("invalid Interface color choice".to_owned()),
    };
    let reduced_motion = match input.select(
        label(locale, "Reduced motion", "减少动画"),
        boolean_choices(locale),
        usize::from(current.reduced_motion()),
    )? {
        0 => false,
        1 => true,
        _ => return Err("invalid reduced-motion choice".to_owned()),
    };
    Ok(Some(InterfacePreferences::new(
        language,
        color,
        reduced_motion,
    )))
}

const fn concrete_locale(
    language: InterfaceLanguage,
    auto_locale: ResolvedLocale,
) -> ResolvedLocale {
    match language {
        InterfaceLanguage::ZhCn => ResolvedLocale::ZhCn,
        InterfaceLanguage::Auto => auto_locale,
        InterfaceLanguage::EnUs => ResolvedLocale::EnUs,
    }
}

const fn label(
    locale: ResolvedLocale,
    english: &'static str,
    chinese: &'static str,
) -> &'static str {
    match locale {
        ResolvedLocale::EnUs => english,
        ResolvedLocale::ZhCn => chinese,
    }
}

const fn color_choices(locale: ResolvedLocale) -> &'static [&'static str] {
    match locale {
        ResolvedLocale::EnUs => &["Auto", "Always", "Never"],
        ResolvedLocale::ZhCn => &["自动", "始终", "从不"],
    }
}

const fn boolean_choices(locale: ResolvedLocale) -> &'static [&'static str] {
    match locale {
        ResolvedLocale::EnUs => &["Off", "On"],
        ResolvedLocale::ZhCn => &["关闭", "开启"],
    }
}

const fn language_index(language: InterfaceLanguage) -> usize {
    match language {
        InterfaceLanguage::Auto => 0,
        InterfaceLanguage::ZhCn => 1,
        InterfaceLanguage::EnUs => 2,
    }
}

const fn color_index(color: HumanColor) -> usize {
    match color {
        HumanColor::Auto => 0,
        HumanColor::Always => 1,
        HumanColor::Never => 2,
    }
}

fn customize(
    input: &mut impl GuidedInput,
    mut draft: PresentationSettings,
) -> Result<PresentationSettings, String> {
    loop {
        match input.select(
            "Customize presentation",
            &["Title", "Tab color", "Activity", "Spinner", "Theme", "Done"],
            5,
        )? {
            0 => {
                draft = draft.with_title(select_value(
                    input,
                    "Title",
                    &["TabBeacon", "Native terminal", "Off"],
                    &[TitleMode::TabBeacon, TitleMode::Native, TitleMode::Off],
                )?);
            }
            1 => {
                draft = draft.with_tab_color(select_value(
                    input,
                    "Tab color",
                    &["TabBeacon", "Native terminal", "Off"],
                    &[
                        TabColorMode::TabBeacon,
                        TabColorMode::Native,
                        TabColorMode::Off,
                    ],
                )?);
            }
            2 => {
                draft = draft.with_activity(select_value(
                    input,
                    "Activity",
                    &[
                        "Title spinner",
                        "Title indicator",
                        "Terminal ring",
                        "Both",
                        "Native",
                        "Off",
                    ],
                    &[
                        ActivityMode::TitleSpinner,
                        ActivityMode::TitleIndicator,
                        ActivityMode::WindowsTerminalRing,
                        ActivityMode::Both,
                        ActivityMode::Native,
                        ActivityMode::Off,
                    ],
                )?);
            }
            3 => {
                draft = draft.with_spinner(select_value(
                    input,
                    "Spinner",
                    &["Codex", "Braille", "Quadrant", "Line", "Pulse"],
                    &[
                        SpinnerPreset::Codex,
                        SpinnerPreset::Braille,
                        SpinnerPreset::Quadrant,
                        SpinnerPreset::Line,
                        SpinnerPreset::Pulse,
                    ],
                )?);
            }
            4 => {
                draft = draft.with_theme(select_value(
                    input,
                    "Theme",
                    &["Muted Dark", "Classic"],
                    &[PresentationTheme::MutedDark, PresentationTheme::Classic],
                )?);
            }
            5 => return Ok(draft),
            _ => return Err("invalid customize choice".to_owned()),
        }
    }
}

fn select_value<T: Copy>(
    input: &mut impl GuidedInput,
    prompt: &str,
    labels: &[&str],
    values: &[T],
) -> Result<T, String> {
    let choice = input.select(prompt, labels, 0)?;
    values
        .get(choice)
        .copied()
        .ok_or_else(|| "invalid guided setup choice".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{GuidedInput, choose_interface_preferences, choose_presentation};
    use crate::human_presentation::ResolvedLocale;
    use crate::interface_preferences::{HumanColor, InterfaceLanguage, InterfacePreferences};
    use crate::settings::{PresentationSettings, SpinnerPreset, TitleMode};

    struct ScriptedInput {
        choices: Vec<usize>,
        prompts: Vec<String>,
    }

    impl ScriptedInput {
        fn new(choices: &[usize]) -> Self {
            Self {
                choices: choices.iter().rev().copied().collect(),
                prompts: Vec::new(),
            }
        }
    }

    impl GuidedInput for ScriptedInput {
        fn select(
            &mut self,
            prompt: &str,
            _items: &[&str],
            _default: usize,
        ) -> Result<usize, String> {
            self.prompts.push(prompt.to_owned());
            self.choices
                .pop()
                .ok_or_else(|| "missing scripted choice".to_owned())
        }
    }

    #[test]
    fn recommended_preset_is_atomic() {
        let mut input = ScriptedInput::new(&[0, 0]);
        let result =
            choose_presentation(&mut input, PresentationSettings::preset("native").unwrap())
                .unwrap();
        assert_eq!(result, Some(PresentationSettings::default()));
        assert_eq!(input.prompts, ["Choose presentation", "Preset"]);
    }

    #[test]
    fn explicit_customize_uses_human_labels_and_keeps_draft_in_memory() {
        let mut input = ScriptedInput::new(&[4, 0, 1, 3, 1, 5]);
        let result = choose_presentation(&mut input, PresentationSettings::default())
            .unwrap()
            .unwrap();
        assert_eq!(result.title(), TitleMode::Native);
        assert_eq!(result.spinner(), SpinnerPreset::Braille);
        assert!(
            input
                .prompts
                .iter()
                .any(|prompt| prompt == "Customize presentation")
        );
    }

    #[test]
    fn back_cancels_without_a_draft() {
        let mut input = ScriptedInput::new(&[5]);
        assert_eq!(
            choose_presentation(&mut input, PresentationSettings::default()).unwrap(),
            None
        );
    }

    #[test]
    fn fresh_interface_choice_starts_bilingual_then_uses_the_selected_language() {
        let mut input = ScriptedInput::new(&[1, 2, 1]);
        let result = choose_interface_preferences(
            &mut input,
            InterfacePreferences::default(),
            true,
            ResolvedLocale::EnUs,
        )
        .expect("choices resolve")
        .expect("back was not selected");
        assert_eq!(result.language(), InterfaceLanguage::ZhCn);
        assert_eq!(result.color(), HumanColor::Never);
        assert!(result.reduced_motion());
        assert_eq!(input.prompts[0], "Language / 语言");
        assert_eq!(input.prompts[1], "颜色");
        assert_eq!(input.prompts[2], "减少动画");
    }

    #[test]
    fn fresh_english_interface_choice_localizes_following_prompts_to_english() {
        let mut input = ScriptedInput::new(&[2, 0, 0]);
        let result = choose_interface_preferences(
            &mut input,
            InterfacePreferences::default(),
            true,
            ResolvedLocale::EnUs,
        )
        .expect("choices resolve")
        .expect("back was not selected");
        assert_eq!(result.language(), InterfaceLanguage::EnUs);
        assert_eq!(
            input.prompts,
            ["Language / 语言", "Color", "Reduced motion"]
        );
    }

    #[test]
    fn returning_quick_setup_keeps_the_current_interface_draft_without_prompting() {
        let current = InterfacePreferences::new(InterfaceLanguage::EnUs, HumanColor::Always, true);
        let mut input = ScriptedInput::new(&[]);
        assert_eq!(
            choose_interface_preferences(&mut input, current, false, ResolvedLocale::EnUs)
                .expect("quick draft"),
            Some(current)
        );
        assert!(input.prompts.is_empty());
    }

    #[test]
    fn bilingual_back_cancels_before_any_draft_is_created() {
        let mut input = ScriptedInput::new(&[3]);
        assert_eq!(
            choose_interface_preferences(
                &mut input,
                InterfacePreferences::default(),
                true,
                ResolvedLocale::EnUs,
            )
            .expect("back resolves"),
            None
        );
    }

    #[test]
    fn auto_language_uses_the_resolved_chinese_fallback_for_following_prompts() {
        let mut input = ScriptedInput::new(&[0, 0, 0]);
        let result = choose_interface_preferences(
            &mut input,
            InterfacePreferences::default(),
            true,
            ResolvedLocale::ZhCn,
        )
        .expect("choices resolve")
        .expect("back was not selected");
        assert_eq!(result.language(), InterfaceLanguage::Auto);
        assert_eq!(input.prompts, ["Language / 语言", "颜色", "减少动画"]);
    }
}
