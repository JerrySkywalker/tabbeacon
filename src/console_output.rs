//! Windows console output that preserves Unicode terminal-title semantics.
//!
//! Hook stdin/stdout can be redirected by the provider, so the production
//! path writes terminal bytes through `CONOUT$`. On legacy Windows console code
//! pages, writing UTF-8 title bytes through that handle corrupts braille and
//! status glyphs. This adapter sends the renderer's typed OSC-0 title through
//! a Unicode VT console write, while preserving all non-title VT bytes.

use std::{
    fs::File,
    io::{self, Write},
};

#[cfg(windows)]
use std::{fs::OpenOptions, io::Error, os::windows::io::AsRawHandle};

#[cfg(windows)]
use windows::Win32::{
    Foundation::HANDLE,
    System::Console::{
        CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, SetConsoleMode,
        WriteConsoleW,
    },
};

const TITLE_PREFIX: &[u8] = b"\x1b]0;";
const STRING_TERMINATOR: &[u8] = b"\x1b\\";

/// An owned terminal-output sink that routes renderer title frames through the
/// Windows Unicode title channel.
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
        let mut sink = ConsoleOutputSink {
            output: &mut self.output,
        };
        drain_title_sequences(&mut self.pending, final_flush, &mut sink)
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
#[cfg(windows)]
pub fn open_owned_console() -> io::Result<OwnedConsole> {
    OpenOptions::new()
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

trait TitleSequenceSink {
    fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()>;

    fn write_title(&mut self, title: &str) -> io::Result<()>;
}

struct ConsoleOutputSink<'a> {
    output: &'a mut File,
}

impl TitleSequenceSink for ConsoleOutputSink<'_> {
    fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.output.write_all(bytes)
    }

    fn write_title(&mut self, title: &str) -> io::Result<()> {
        write_owned_console_title(self.output, title)
    }
}

fn drain_title_sequences(
    pending: &mut Vec<u8>,
    final_flush: bool,
    sink: &mut impl TitleSequenceSink,
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
            sink.write_title(title)?;
            pending.drain(..title_end + STRING_TERMINATOR.len());
            continue;
        }

        if let Some(title_offset) = find_bytes(pending, TITLE_PREFIX) {
            let before_title = pending[..title_offset].to_vec();
            sink.write_bytes(&before_title)?;
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
            sink.write_bytes(&passthrough)?;
            pending.drain(..bytes_to_write);
        }
        if final_flush && !pending.is_empty() {
            let passthrough = std::mem::take(pending);
            sink.write_bytes(&passthrough)?;
        }
        return Ok(());
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn unicode_title_sequence(title: &str) -> Vec<u16> {
    format!("\x1b]0;{title}\x1b\\").encode_utf16().collect()
}

#[cfg(windows)]
fn write_owned_console_title(output: &File, title: &str) -> io::Result<()> {
    let handle = HANDLE(output.as_raw_handle());
    let mut original_mode = CONSOLE_MODE(0);
    // SAFETY: `output` owns a live `CONOUT$` handle for this synchronous call,
    // and `original_mode` is a valid writable out-parameter.
    #[allow(unsafe_code)]
    unsafe {
        GetConsoleMode(handle, &mut original_mode).map_err(Error::from)?;
    }
    let temporary_mode = original_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING;
    let mode_changed = temporary_mode != original_mode;
    if mode_changed {
        // SAFETY: the owned console handle is valid and the saved mode was
        // obtained from this same handle immediately above.
        #[allow(unsafe_code)]
        unsafe {
            SetConsoleMode(handle, temporary_mode).map_err(Error::from)?;
        }
    }

    let title_sequence = unicode_title_sequence(title);
    let mut remaining = title_sequence.as_slice();
    let write_result = (|| -> io::Result<()> {
        while !remaining.is_empty() {
            let mut written = 0_u32;
            // SAFETY: `remaining` is an owned contiguous UTF-16 slice that
            // remains valid for this synchronous write; the handle is owned by
            // `output`, and `written` is a valid writable out-parameter.
            #[allow(unsafe_code)]
            unsafe {
                WriteConsoleW(handle, remaining, Some(&mut written), None).map_err(Error::from)?;
            }
            let written = usize::try_from(written).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::WriteZero,
                    "console reported an invalid write",
                )
            })?;
            if written == 0 || written > remaining.len() {
                return Err(io::Error::new(
                    io::ErrorKind::WriteZero,
                    "console did not write the complete title sequence",
                ));
            }
            remaining = &remaining[written..];
        }
        Ok(())
    })();

    let restore_result = if mode_changed {
        // SAFETY: restore exactly the mode captured from this live owned
        // console handle before the scoped VT title write.
        #[allow(unsafe_code)]
        unsafe {
            SetConsoleMode(handle, original_mode).map_err(Error::from)
        }
    } else {
        Ok(())
    };
    write_result.and(restore_result)
}

#[cfg(not(windows))]
fn write_owned_console_title(_output: &File, _title: &str) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "owned Windows console title output is unavailable",
    ))
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::{TitleSequenceSink, drain_title_sequences, unicode_title_sequence};

    #[derive(Default)]
    struct RecordedSink {
        titles: Vec<String>,
        passthrough: Vec<u8>,
    }

    impl TitleSequenceSink for RecordedSink {
        fn write_bytes(&mut self, bytes: &[u8]) -> io::Result<()> {
            self.passthrough.extend_from_slice(bytes);
            Ok(())
        }

        fn write_title(&mut self, title: &str) -> io::Result<()> {
            self.titles.push(title.to_owned());
            Ok(())
        }
    }

    #[test]
    fn unicode_title_sequence_is_a_complete_wide_osc_payload() {
        let units = unicode_title_sequence("⠋ WORK");

        assert_eq!(
            String::from_utf16(&units).expect("title payload is valid UTF-16"),
            "\x1b]0;⠋ WORK\x1b\\"
        );
    }

    #[test]
    fn title_sequences_preserve_other_vt_bytes() {
        let mut pending = b"\x1b]0;\xE2\xA0\x8B WORK\x1b\\\x1b]9;4;3;0\x1b\\".to_vec();
        let mut sink = RecordedSink::default();

        drain_title_sequences(&mut pending, true, &mut sink)
            .expect("valid renderer title is drained");

        assert_eq!(sink.titles, ["⠋ WORK"]);
        assert_eq!(sink.passthrough, b"\x1b]9;4;3;0\x1b\\");
        assert!(pending.is_empty());
    }

    #[test]
    fn split_title_sequence_waits_for_its_terminator() {
        let mut pending = b"\x1b]0;\xE2\xA0".to_vec();
        let mut sink = RecordedSink::default();

        drain_title_sequences(&mut pending, false, &mut sink)
            .expect("incomplete title remains buffered");
        assert!(sink.titles.is_empty());
        assert!(sink.passthrough.is_empty());

        pending.extend_from_slice(b"\x8B WORK\x1b\\");
        drain_title_sequences(&mut pending, true, &mut sink).expect("completed title is drained");

        assert_eq!(sink.titles, ["⠋ WORK"]);
        assert!(sink.passthrough.is_empty());
        assert!(pending.is_empty());
    }
}
