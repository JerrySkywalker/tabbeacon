//! Product-owned bounded read-only command execution. No writer or terminal handle.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read},
    process::{Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
use terminal_ui_runtime::refresh::CollectionControl;

const OUTPUT_LIMIT: u64 = 64 * 1024;
static NONCE: AtomicU64 = AtomicU64::new(0);

/// Hashes an observed executable with bounded memory and cooperative control.
///
/// # Errors
/// Returns safe read, cancellation or 512 MiB byte-budget failure.
pub fn hash_observation_file(
    path: &std::path::Path,
    control: Option<&CollectionControl>,
) -> io::Result<String> {
    use sha2::{Digest, Sha256};
    if let Some(control) = control {
        control.check().map_err(io::Error::other)?;
    }
    let mut file =
        File::open(path).map_err(|_| io::Error::other("observation hash unavailable"))?;
    let mut hash = Sha256::new();
    let mut bytes = vec![0u8; 64 * 1024];
    let mut total = 0usize;
    loop {
        if let Some(control) = control {
            control.check().map_err(io::Error::other)?;
        }
        let count = file
            .read(&mut bytes)
            .map_err(|_| io::Error::other("observation hash failed"))?;
        if count == 0 {
            return Ok(format!("{:x}", hash.finalize()));
        }
        total = total.saturating_add(count);
        if total > 512 * 1024 * 1024 {
            return Err(io::Error::other("observation hash byte budget exceeded"));
        }
        hash.update(&bytes[..count]);
    }
}

/// Reads observation bytes in cooperative chunks without changing writer paths.
/// The caller retains its own path ownership/link validation.
///
/// # Errors
/// Returns safe I/O, cancellation or size-limit errors; absence is `None`.
pub fn read_observation_file(
    path: &std::path::Path,
    limit: usize,
    control: Option<&CollectionControl>,
) -> io::Result<Option<Vec<u8>>> {
    if let Some(control) = control {
        control.check().map_err(io::Error::other)?;
    }
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(io::Error::other("observation file unavailable")),
    };
    let mut bytes = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        if let Some(control) = control {
            control.check().map_err(io::Error::other)?;
        }
        let count = file
            .read(&mut chunk)
            .map_err(|_| io::Error::other("observation read failed"))?;
        if count == 0 {
            return Ok(Some(bytes));
        }
        if bytes.len().saturating_add(count) > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "observation exceeds byte budget",
            ));
        }
        bytes.extend_from_slice(&chunk[..count]);
    }
}

/// Safe observation failure; command output and private paths never enter errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeError {
    Unavailable,
    Timeout,
    Cancelled,
    OutputLimit,
    Io,
}

/// One successful owned-child completion, including nonzero exit status.
pub struct ProbeOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
}

/// Runs only the supplied noninteractive command; terminates/reaps only its Child.
/// File-backed stdout avoids pipe readers or descendant-held EOF blocking exit.
/// Retained output is bounded; output growth is checked every 5ms.
///
/// # Errors
/// Returns a safe failure on cancellation, deadline, output limit or OS failure.
pub fn bounded_output(
    command: &mut Command,
    timeout: Duration,
    control: Option<&CollectionControl>,
) -> Result<ProbeOutput, ProbeError> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    if let Some(control) = control {
        control.check().map_err(|error| control_error(&error))?;
    }
    let root = std::env::temp_dir().join(format!(
        "tabbeacon-observation-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        NONCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).map_err(|_| ProbeError::Io)?;
    let capture = root.join("stdout");
    let result = (|| {
        let output = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&capture)
            .map_err(|_| ProbeError::Io)?;
        let mut child = command
            .stdin(Stdio::null())
            .stdout(Stdio::from(output))
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| ProbeError::Unavailable)?;
        let started = Instant::now();
        let completion = loop {
            let failure = control
                .and_then(|c| c.check().err().as_ref().map(control_error))
                .or_else(|| (started.elapsed() >= timeout).then_some(ProbeError::Timeout))
                .or_else(|| {
                    fs::metadata(&capture)
                        .ok()
                        .filter(|m| m.len() > OUTPUT_LIMIT)
                        .map(|_| ProbeError::OutputLimit)
                });
            if let Some(error) = failure {
                break Err(error);
            }
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => thread::sleep(Duration::from_millis(5)),
                Err(_) => break Err(ProbeError::Io),
            }
        };
        if completion.is_err() {
            // Ownership never escapes; even a kill error is followed by actual reap.
            let _ = child.kill();
            child.wait().map_err(|_| ProbeError::Io)?;
        }
        let status = completion?;
        let mut bytes = Vec::new();
        File::open(&capture)
            .and_then(|f| f.take(OUTPUT_LIMIT + 1).read_to_end(&mut bytes))
            .map_err(|_| ProbeError::Io)?;
        if bytes.len() as u64 > OUTPUT_LIMIT {
            return Err(ProbeError::OutputLimit);
        }
        Ok(ProbeOutput {
            status,
            stdout: bytes,
        })
    })();
    // Only the uniquely created capture and empty owned directory are removed.
    let _ = fs::remove_file(capture);
    let _ = fs::remove_dir(root);
    result
}

fn control_error(error: &terminal_ui_runtime::refresh::CollectError) -> ProbeError {
    match error {
        terminal_ui_runtime::refresh::CollectError::Cancelled => ProbeError::Cancelled,
        _ => ProbeError::Timeout,
    }
}

impl From<ProbeError> for io::Error {
    fn from(value: ProbeError) -> Self {
        io::Error::other(format!("provider observation {value:?}"))
    }
}
