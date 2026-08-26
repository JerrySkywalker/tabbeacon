//! Windows console output that preserves Unicode terminal-title semantics.
//!
//! Hook stdin/stdout can be redirected by the provider, so the production
//! path writes terminal bytes through `CONOUT$`. On legacy Windows console code
//! pages, writing UTF-8 title bytes through that handle corrupts braille and
//! status glyphs. This adapter sends the renderer's typed OSC-0 title as UTF-16
//! through the already-enabled VT console path, while preserving all non-title
//! VT bytes.

use std::{
    fs::File,
    io::{self, Write},
};

#[cfg(windows)]
use std::{
    fs::OpenOptions,
    io::Error,
    os::windows::io::{AsRawHandle, RawHandle},
};

#[cfg(windows)]
use windows::Win32::{
    Foundation::HANDLE,
    System::Console::{
        CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, WriteConsoleW,
    },
};

const TITLE_PREFIX: &[u8] = b"\x1b]0;";
const STRING_TERMINATOR: &[u8] = b"\x1b\\";

/// An owned terminal-output sink that routes renderer title frames through the
/// Windows Unicode VT title channel.
#[derive(Debug)]
pub struct OwnedConsole {
    output: File,
    pending: Vec<u8>,
}

impl OwnedConsole {
    #[cfg(windows)]
    fn new(output: File) -> Self {
        Self {
            output,
            pending: Vec::new(),
        }
    }

    fn drain_pending(&mut self, final_flush: bool) -> io::Result<()> {
        #[cfg(windows)]
        let title_output = self.output.as_raw_handle();
        #[cfg(not(windows))]
        let title_output = ();
        let output = &mut self.output;
        drain_title_sequences(
            &mut self.pending,
            final_flush,
            &mut |bytes| output.write_all(bytes),
            &mut |title| write_owned_terminal_title_vt(title_output, title),
        )
    }
}

impl Write for OwnedConsole {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(bytes);
        self.drain_pending(false)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.drain_pending(true)?;
        self.output.flush()
    }
}

/// Opens the current terminal's owned output handle.
///
/// On Windows, this intentionally bypasses redirected hook stdout while
/// retaining Unicode title output through [`OwnedConsole`].
///
/// # Errors
///
/// Returns an error when the current process cannot open the owned console,
/// or when the platform does not provide one.
#[cfg(windows)]
pub fn open_owned_console() -> io::Result<OwnedConsole> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONOUT$")
        .map(OwnedConsole::new)
}

/// Returns the platform's fail-open unsupported result outside Windows.
#[cfg(not(windows))]
pub fn open_owned_console() -> io::Result<OwnedConsole> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "owned Windows console output is unavailable",
    ))
}

fn drain_title_sequences(
    pending: &mut Vec<u8>,
    final_flush: bool,
    write_bytes: &mut impl FnMut(&[u8]) -> io::Result<()>,
    set_title: &mut impl FnMut(&str) -> io::Result<()>,
) -> io::Result<()> {
    loop {
        if pending.starts_with(TITLE_PREFIX) {
            let title_start = TITLE_PREFIX.len();
            let Some(terminator_offset) = find_bytes(&pending[title_start..], STRING_TERMINATOR)
            else {
                return if final_flush {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "terminal title sequence ended before its string terminator",
                    ))
                } else {
                    Ok(())
                };
            };
            let title_end = title_start + terminator_offset;
            let title = std::str::from_utf8(&pending[title_start..title_end]).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "terminal title sequence was not valid UTF-8",
                )
            })?;
            set_title(title)?;
            pending.drain(..title_end + STRING_TERMINATOR.len());
            continue;
        }

        if let Some(title_offset) = find_bytes(pending, TITLE_PREFIX) {
            let before_title = pending[..title_offset].to_vec();
            write_bytes(&before_title)?;
            pending.drain(..title_offset);
            continue;
        }

        let retained_prefix = TITLE_PREFIX
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, _)| {
                pending
                    .ends_with(&TITLE_PREFIX[..index.saturating_add(1)])
                    .then_some(index.saturating_add(1))
            })
            .unwrap_or(0);
        let bytes_to_write = pending.len().saturating_sub(retained_prefix);
        if bytes_to_write > 0 {
            let passthrough = pending[..bytes_to_write].to_vec();
            write_bytes(&passthrough)?;
            pending.drain(..bytes_to_write);
        }
        if final_flush && !pending.is_empty() {
            let passthrough = std::mem::take(pending);
            write_bytes(&passthrough)?;
        }
        return Ok(());
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn owned_terminal_title_sequence(title: &str) -> String {
    format!("\x1b]0;{title}\x1b\\")
}

#[cfg(windows)]
fn write_owned_terminal_title_vt(output: RawHandle, title: &str) -> io::Result<()> {
    let handle = HANDLE(output);
    let mut mode = CONSOLE_MODE(0);
    #[allow(unsafe_code)]
    unsafe {
        GetConsoleMode(handle, &raw mut mode).map_err(Error::from)?;
    }
    if mode.0 & ENABLE_VIRTUAL_TERMINAL_PROCESSING.0 == 0 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "owned console does not already enable virtual-terminal processing",
        ));
    }

    let sequence = owned_terminal_title_sequence(title);
    let wide: Vec<u16> = sequence.encode_utf16().collect();
    let mut written = 0;
    // SAFETY: `wide` owns the UTF-16 data for this synchronous write. `handle`
    // is borrowed from `OwnedConsole::output` and remains valid for this call.
    #[allow(unsafe_code)]
    unsafe {
        WriteConsoleW(handle, &wide, Some(&raw mut written), None).map_err(Error::from)?;
    }
    if written != u32::try_from(wide.len()).unwrap_or(u32::MAX) {
        return Err(io::Error::new(
            io::ErrorKind::WriteZero,
            "owned console wrote an incomplete terminal-title sequence",
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn write_owned_terminal_title_vt(_output: (), _title: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "owned Windows terminal title output is unavailable",
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        STRING_TERMINATOR, TITLE_PREFIX, drain_title_sequences, owned_terminal_title_sequence,
    };
    use crate::presentation::{MAX_TITLE_SCALARS, TerminalTitle};

    #[test]
    fn title_sequences_use_unicode_wide_vt_channel_and_preserve_other_vt_bytes() {
        let mut pending = b"\x1b]0;\xE2\xA0\x8B WORK\x1b\\\x1b]9;4;3;0\x1b\\".to_vec();
        let mut titles = Vec::new();
        let mut passthrough = Vec::new();

        drain_title_sequences(
            &mut pending,
            true,
            &mut |bytes| {
                passthrough.extend_from_slice(bytes);
                Ok(())
            },
            &mut |title| {
                titles.push(title.to_owned());
                Ok(())
            },
        )
        .expect("valid renderer title is drained");

        assert_eq!(titles, ["⠋ WORK"]);
        assert_eq!(passthrough, b"\x1b]9;4;3;0\x1b\\");
        assert!(pending.is_empty());
    }

    #[test]
    fn split_title_sequence_waits_for_its_terminator() {
        let mut pending = b"\x1b]0;\xE2\xA0".to_vec();
        let mut titles = Vec::new();
        let mut passthrough = Vec::new();

        drain_title_sequences(
            &mut pending,
            false,
            &mut |bytes| {
                passthrough.extend_from_slice(bytes);
                Ok(())
            },
            &mut |title| {
                titles.push(title.to_owned());
                Ok(())
            },
        )
        .expect("incomplete title remains buffered");
        assert!(titles.is_empty());
        assert!(passthrough.is_empty());

        pending.extend_from_slice(b"\x8B WORK\x1b\\");
        drain_title_sequences(
            &mut pending,
            true,
            &mut |bytes| {
                passthrough.extend_from_slice(bytes);
                Ok(())
            },
            &mut |title| {
                titles.push(title.to_owned());
                Ok(())
            },
        )
        .expect("completed title is drained");

        assert_eq!(titles, ["⠋ WORK"]);
        assert!(passthrough.is_empty());
        assert!(pending.is_empty());
    }

    #[test]
    fn unicode_title_vt_uses_one_admitted_osc_envelope() {
        assert_eq!(
            owned_terminal_title_sequence("Codex ⠋ OWH"),
            "\x1b]0;Codex ⠋ OWH\x1b\\"
        );
    }

    #[test]
    fn title_sanitization_stays_bounded_before_wide_vt_transport() {
        let title = TerminalTitle::new(&("界".repeat(MAX_TITLE_SCALARS + 1) + "\u{0007}"));
        assert_eq!(title.as_str().chars().count(), MAX_TITLE_SCALARS);
        assert!(!title.as_str().chars().any(char::is_control));
        let sequence = owned_terminal_title_sequence(title.as_str());
        assert!(sequence.as_bytes().starts_with(TITLE_PREFIX));
        assert!(sequence.as_bytes().ends_with(STRING_TERMINATOR));
    }

    #[test]
    fn short_lived_hook_title_must_not_use_process_scoped_setconsole_title() {
        let mut pending = b"\x1b]0;Codex \xE2\xA0\x8B OWH\x1b\\".to_vec();
        let mut titles = Vec::new();
        let mut passthrough = Vec::new();
        drain_title_sequences(
            &mut pending,
            true,
            &mut |bytes| {
                passthrough.extend_from_slice(bytes);
                Ok(())
            },
            &mut |title| {
                titles.push(owned_terminal_title_sequence(title));
                Ok(())
            },
        )
        .expect("the title is emitted through its admitted OSC envelope");

        assert_eq!(titles, ["\x1b]0;Codex ⠋ OWH\x1b\\"]);
        assert!(passthrough.is_empty());
    }
}
