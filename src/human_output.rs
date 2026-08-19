//! Small, auto-only terminal styling helpers for human-facing CLI output.
//!
//! This is deliberately presentation-only: JSON and `--plain` machine
//! transports never receive ANSI sequences, and output redirected away from a
//! terminal remains monochrome. Persistent color preferences belong to v0.5.

/// A restrained semantic treatment for a human-facing line.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HumanTone {
    /// No escape sequence; the text remains ordinary terminal output.
    Plain,
    /// A heading or compact product accent.
    Accent,
    /// A successful completion or healthy state.
    Success,
    /// An action the user should notice.
    Attention,
    /// A failed state.
    Failure,
    /// Supporting explanation or next-step detail.
    Dim,
}

/// Whether automatic human color is appropriate for this output target.
#[must_use]
pub const fn auto_color_enabled(stdout_is_terminal: bool, no_color_is_set: bool) -> bool {
    stdout_is_terminal && !no_color_is_set
}

/// Returns a styled human line only when automatic color is enabled.
#[must_use]
pub fn style(tone: HumanTone, value: &str, color_enabled: bool) -> String {
    if !color_enabled || tone == HumanTone::Plain {
        return value.to_owned();
    }
    let prefix = match tone {
        HumanTone::Plain => unreachable!("plain lines return before styling"),
        HumanTone::Accent => "\x1b[1;36m",
        HumanTone::Success => "\x1b[32m",
        HumanTone::Attention => "\x1b[33m",
        HumanTone::Failure => "\x1b[31m",
        HumanTone::Dim => "\x1b[2m",
    };
    format!("{prefix}{value}\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::{HumanTone, auto_color_enabled, style};

    #[test]
    fn auto_color_respects_terminal_and_no_color_boundaries() {
        assert!(auto_color_enabled(true, false));
        assert!(!auto_color_enabled(false, false));
        assert!(!auto_color_enabled(true, true));
    }

    #[test]
    fn styling_is_absent_for_plain_or_redirected_output() {
        assert_eq!(style(HumanTone::Success, "Ready", false), "Ready");
        assert_eq!(style(HumanTone::Plain, "Ready", true), "Ready");
        assert_eq!(
            style(HumanTone::Attention, "Review required", true),
            "\x1b[33mReview required\x1b[0m"
        );
    }
}
