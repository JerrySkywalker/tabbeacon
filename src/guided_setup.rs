//! Inline, scrollback-preserving selection flow for guided setup.

use crate::settings::{
    ActivityMode, PresentationSettings, PresentationTheme, SpinnerPreset, TabColorMode, TitleMode,
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
    use super::{GuidedInput, choose_presentation};
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
}
