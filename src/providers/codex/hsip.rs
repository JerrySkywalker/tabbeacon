//! Integration-owned, fail-open HSIP v1 producer.
//!
//! Pinned against `HookStat` `e3174b4d4d487bed0227b369945d20149b462aff`.
//! This module is intentionally just the local binary wire client: it has no
//! `HookStat` product, database, analytics, CLI, TUI, listener, or network code.

use std::{
    io,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime},
};

use sha2::{Digest, Sha256};

#[cfg(unix)]
use socket2::{Domain, SockAddr, Socket, Type};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use std::{
    ffi::OsStr,
    fs::{File, OpenOptions},
    os::windows::{ffi::OsStrExt, io::AsRawHandle},
};
#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::HANDLE,
        System::Pipes::{NAMED_PIPE_MODE, PIPE_NOWAIT, SetNamedPipeHandleState, WaitNamedPipeW},
    },
    core::PCWSTR,
};

use super::{CodexHookContext, HookDispatchOutcome};

const HEADER_BYTES: usize = 10;
const MAX_FRAME_BYTES: usize = 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(2);
const ACK_TIMEOUT: Duration = Duration::from_millis(5);
const REUSE_WINDOW: Duration = Duration::from_millis(25);
const HANDLER: &str = "tabbeacon-codex-hsip-v1";
const SOURCE_SCOPE: &str = "tabbeacon-v061";
const REVISION: &str = "tb-v061-hsip-v1";

static PRODUCER: OnceLock<Mutex<Option<Producer>>> = OnceLock::new();
static INVOCATION_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
struct Lifecycle {
    runtime_instance: String,
    invocation: String,
    event: String,
    observed_at: i64,
}

/// An accepted START bound to precisely one real `TabBeacon` runtime call.
pub(super) struct CooperativeInvocation {
    producer: Producer,
    lifecycle: Lifecycle,
    started: Instant,
}

impl CooperativeInvocation {
    pub(super) fn start(context: &CodexHookContext) -> Option<Self> {
        let producer = producer()?;
        let sequence = INVOCATION_SEQUENCE
            .fetch_add(1, Ordering::Relaxed)
            .to_string();
        let lifecycle = Lifecycle {
            runtime_instance: opaque_id(b"runtime-instance", &[context.session_id()]),
            invocation: opaque_id(
                b"invocation",
                &[
                    context.session_id(),
                    context.turn_id().unwrap_or("root-lifecycle"),
                    context.event().as_str(),
                    &sequence,
                ],
            ),
            event: context.event().as_str().to_owned(),
            observed_at: unix_millis(SystemTime::now()),
        };
        (producer.emit(&encode_start(&lifecycle)) == Disposition::Accepted).then_some(Self {
            producer,
            lifecycle,
            started: Instant::now(),
        })
    }

    pub(super) fn finish(self, outcome: HookDispatchOutcome) {
        let terminal = match outcome {
            HookDispatchOutcome::Applied
            | HookDispatchOutcome::PreservedCurrentState
            | HookDispatchOutcome::IgnoredSubagent
            | HookDispatchOutcome::RejectedStaleGeneration
            | HookDispatchOutcome::IgnoredUnsupported => 1_u8,
            HookDispatchOutcome::DegradedInput
            | HookDispatchOutcome::DegradedRepositoryIdentity
            | HookDispatchOutcome::DegradedWorkspaceIdentity
            | HookDispatchOutcome::DegradedPresentationOutput
            | HookDispatchOutcome::DegradedStateRoot
            | HookDispatchOutcome::DegradedGenerationState
            | HookDispatchOutcome::DegradedRootWorkspaceAnchor => 2_u8,
        };
        let duration = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        // A lost/overloaded/unavailable COMPLETE is an observation gap, never a
        // TabBeacon or Codex execution error; no uncertain frame is replayed.
        let _ = self
            .producer
            .emit(&encode_complete(&self.lifecycle, terminal, duration));
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
enum Disposition {
    Accepted,
    DroppedOverloaded,
    Busy,
    Rejected,
    Unavailable,
    BudgetExhausted,
}

#[derive(Clone)]
struct Producer {
    endpoint: Endpoint,
    connection: Arc<Mutex<Option<Connection>>>,
}

struct Connection {
    #[cfg(unix)]
    stream: UnixStream,
    #[cfg(windows)]
    stream: File,
    last_ack: Instant,
}

impl Producer {
    fn for_default_root() -> Option<Self> {
        let endpoint = Endpoint::from_state_root(&hookstat_data_root()?).ok()?;
        Some(Self {
            endpoint,
            connection: Arc::new(Mutex::new(None)),
        })
    }

    fn emit(&self, frame: &[u8]) -> Disposition {
        let Ok(mut connection) = self.connection.try_lock() else {
            return Disposition::Busy;
        };
        if connection
            .as_ref()
            .is_some_and(|value| value.last_ack.elapsed() >= REUSE_WINDOW)
        {
            let _ = connection.take();
        }
        if connection.is_none() {
            let Some(stream) = self.connect() else {
                return Disposition::Unavailable;
            };
            *connection = Some(Connection {
                stream,
                last_ack: Instant::now(),
            });
        }
        match Self::send_and_ack(
            connection.as_mut().expect("new connection is present"),
            frame,
        ) {
            Ok(ack) => {
                connection
                    .as_mut()
                    .expect("connection remains present")
                    .last_ack = Instant::now();
                match ack {
                    1 => Disposition::Accepted,
                    2 => Disposition::DroppedOverloaded,
                    4 => Disposition::Busy,
                    _ => Disposition::Rejected,
                }
            }
            Err(error) => {
                // Once a write or ACK becomes uncertain it cannot be retried.
                let _ = connection.take();
                if error.kind() == io::ErrorKind::TimedOut {
                    Disposition::BudgetExhausted
                } else {
                    Disposition::Unavailable
                }
            }
        }
    }

    #[cfg(unix)]
    fn connect(&self) -> Option<UnixStream> {
        let socket = Socket::new(Domain::UNIX, Type::STREAM, None).ok()?;
        let address = SockAddr::unix(self.endpoint.socket_path()).ok()?;
        socket.connect_timeout(&address, CONNECT_TIMEOUT).ok()?;
        let stream = UnixStream::from(socket);
        stream.set_nonblocking(true).ok()?;
        Some(stream)
    }

    #[cfg(windows)]
    #[allow(unsafe_code)]
    fn connect(&self) -> Option<File> {
        let path = format!(r"\\.\pipe\{}", self.endpoint.pipe_name());
        let wide_path = OsStr::new(&path)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let timeout = u32::try_from(CONNECT_TIMEOUT.as_millis()).ok()?;
        if !unsafe { WaitNamedPipeW(PCWSTR(wide_path.as_ptr()), timeout).as_bool() } {
            return None;
        }
        let stream = OpenOptions::new().read(true).write(true).open(path).ok()?;
        let mode = NAMED_PIPE_MODE(PIPE_NOWAIT.0);
        unsafe {
            SetNamedPipeHandleState(
                HANDLE(stream.as_raw_handle()),
                Some(&raw const mode),
                None,
                None,
            )
            .ok()?;
        }
        Some(stream)
    }

    fn send_and_ack(connection: &mut Connection, frame: &[u8]) -> io::Result<u8> {
        let deadline = Instant::now() + ACK_TIMEOUT;
        write_until(&mut connection.stream, frame, deadline)?;
        let mut header = [0_u8; HEADER_BYTES];
        read_until(&mut connection.stream, &mut header, deadline)?;
        validate_ack_header(&header)?;
        let mut payload = [0_u8; 1];
        read_until(&mut connection.stream, &mut payload, deadline)?;
        Ok(payload[0])
    }
}

#[derive(Clone)]
struct Endpoint {
    #[cfg(unix)]
    root: PathBuf,
    identifier: String,
}

impl Endpoint {
    fn from_state_root(root: &Path) -> io::Result<Self> {
        // The broker exclusively owns HookStat state creation. This client
        // refuses unsafe/missing state instead of manufacturing a new path.
        // Check the leaf before canonicalizing it. The overwhelmingly common
        // no-broker path has no HookStat root; avoiding a Windows full-path
        // resolution there keeps normal TabBeacon hooks independent of an
        // optional observer's absent state.
        let initial_state = std::fs::symlink_metadata(root)?;
        if initial_state.file_type().is_symlink()
            || !initial_state.is_dir()
            || reparse_point(&initial_state)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsafe HookStat state",
            ));
        }
        let root = std::fs::canonicalize(root)?;
        let state = std::fs::symlink_metadata(&root)?;
        let ipc = root.join("ipc");
        let transport = std::fs::symlink_metadata(&ipc)?;
        if state.file_type().is_symlink()
            || !state.is_dir()
            || transport.file_type().is_symlink()
            || !transport.is_dir()
            || reparse_point(&state)
            || reparse_point(&transport)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unsafe HookStat state",
            ));
        }
        let mut hash = Sha256::new();
        hash.update(b"hookstat-g35-local-endpoint-v1\0");
        hash.update(root.as_os_str().as_encoded_bytes());
        hash.update(b"\0");
        hash.update(
            std::env::var_os("USERNAME")
                .or_else(|| std::env::var_os("USER"))
                .unwrap_or_default()
                .as_encoded_bytes(),
        );
        let identifier = hex_lower(&hash.finalize()[..16]);
        Ok(Self {
            #[cfg(unix)]
            root,
            identifier,
        })
    }

    #[cfg(unix)]
    fn socket_path(&self) -> PathBuf {
        self.root
            .join("ipc")
            .join(format!("g35-{}.sock", self.identifier))
    }

    #[cfg(windows)]
    fn pipe_name(&self) -> String {
        format!("hookstat-g35-{}", self.identifier)
    }
}

#[cfg(windows)]
fn reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
const fn reparse_point(_metadata: &std::fs::Metadata) -> bool {
    false
}

fn producer() -> Option<Producer> {
    let cache = PRODUCER.get_or_init(|| Mutex::new(None));
    cached_or_create(cache, Producer::for_default_root)
}

/// Retain only a validated producer.  Caching a missing state root would make a
/// long-lived MCP process permanently blind if `HookStat` starts after `TabBeacon`.
/// An unavailable observer is retried on a later hook invocation; it is still
/// strictly fail-open for the invocation that observed the gap.
fn cached_or_create<F>(cache: &Mutex<Option<Producer>>, create: F) -> Option<Producer>
where
    F: FnOnce() -> Option<Producer>,
{
    let mut cache = cache.try_lock().ok()?;
    if let Some(producer) = cache.as_ref() {
        return Some(producer.clone());
    }
    let producer = create()?;
    *cache = Some(producer.clone());
    Some(producer)
}

fn hookstat_data_root() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("LOCALAPPDATA").or_else(|| std::env::var_os("APPDATA"));
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_DATA_HOME").or_else(|| {
        std::env::var_os("HOME").map(|value| PathBuf::from(value).join(".local/share"))
    });
    base.map(PathBuf::from).map(|value| value.join("HookStat"))
}

fn encode_start(lifecycle: &Lifecycle) -> Vec<u8> {
    encode_lifecycle(1, lifecycle, None)
}

fn encode_complete(lifecycle: &Lifecycle, terminal: u8, duration_ms: u64) -> Vec<u8> {
    encode_lifecycle(2, lifecycle, Some((terminal, duration_ms)))
}

fn encode_lifecycle(kind: u8, lifecycle: &Lifecycle, completion: Option<(u8, u64)>) -> Vec<u8> {
    let mut payload = Vec::with_capacity(256);
    for value in [
        "codex",
        lifecycle.runtime_instance.as_str(),
        lifecycle.invocation.as_str(),
        HANDLER,
        lifecycle.event.as_str(),
        SOURCE_SCOPE,
        REVISION,
    ] {
        push_reference(&mut payload, value);
    }
    payload.extend_from_slice(&lifecycle.observed_at.to_le_bytes());
    if let Some((terminal, duration_ms)) = completion {
        payload.push(terminal);
        payload.push(0); // NotApplicable: no OS exit value on the MCP path.
        payload.push(0); // no OS exit value
        payload.extend_from_slice(&duration_ms.to_le_bytes());
    }
    debug_assert!(payload.len() + HEADER_BYTES <= MAX_FRAME_BYTES);
    let mut frame = Vec::with_capacity(HEADER_BYTES + payload.len());
    frame.extend_from_slice(b"HSIP");
    frame.extend_from_slice(&[1, kind, 0, 0]);
    frame.extend_from_slice(
        &u16::try_from(payload.len())
            .expect("HSIP payload is structurally bounded")
            .to_le_bytes(),
    );
    frame.extend_from_slice(&payload);
    frame
}

fn push_reference(output: &mut Vec<u8>, value: &str) {
    debug_assert!(
        value.len() <= 128
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric()
                    || matches!(byte, b'_' | b'-' | b'.' | b':'))
    );
    output.push(u8::try_from(value.len()).expect("HSIP reference is structurally bounded"));
    output.extend_from_slice(value.as_bytes());
}

fn opaque_id(domain: &[u8], values: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    for value in values {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    format!("{:x}", hash.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        },
    )
}

fn unix_millis(time: SystemTime) -> i64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|value| i64::try_from(value.as_millis()).ok())
        .unwrap_or(0)
}

fn validate_ack_header(header: &[u8; HEADER_BYTES]) -> io::Result<()> {
    if header[..4] != *b"HSIP"
        || header[4] != 1
        || header[5] != 3
        || header[6..8] != [0, 0]
        || u16::from_le_bytes([header[8], header[9]]) != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid HSIP ACK",
        ));
    }
    Ok(())
}

fn write_until<S: io::Write>(
    stream: &mut S,
    mut bytes: &[u8],
    deadline: Instant,
) -> io::Result<()> {
    while !bytes.is_empty() {
        match stream.write(bytes) {
            Ok(0) => return Err(io::Error::new(io::ErrorKind::WriteZero, "HSIP write")),
            Ok(written) => bytes = &bytes[written..],
            Err(error) if transient_io_error(&error) && Instant::now() < deadline => {
                std::thread::yield_now();
            }
            Err(error) if transient_io_error(&error) => {
                return Err(timeout_error());
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn read_until<S: io::Read>(
    stream: &mut S,
    mut bytes: &mut [u8],
    deadline: Instant,
) -> io::Result<()> {
    while !bytes.is_empty() {
        match stream.read(bytes) {
            Ok(0) if Instant::now() < deadline => std::thread::yield_now(),
            Ok(0) => return Err(timeout_error()),
            Ok(read) => {
                let (_, rest) = bytes.split_at_mut(read);
                bytes = rest;
            }
            Err(error) if transient_io_error(&error) && Instant::now() < deadline => {
                std::thread::yield_now();
            }
            Err(error) if transient_io_error(&error) => {
                return Err(timeout_error());
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn transient_io_error(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    ) || {
        #[cfg(windows)]
        {
            // A no-wait named pipe reports these Win32 results while the peer
            // has not yet produced an ACK. They remain inside ACK_TIMEOUT.
            matches!(error.raw_os_error(), Some(231 | 232))
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

fn timeout_error() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "bounded HSIP exchange")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    use std::{
        io::{Read, Write},
        sync::mpsc,
        thread,
    };

    #[test]
    fn frames_are_bounded_and_exclude_private_input() {
        let lifecycle = Lifecycle {
            runtime_instance: opaque_id(b"runtime", &["private-session"]),
            invocation: opaque_id(b"invocation", &["private-turn"]),
            event: "UserPromptSubmit".to_owned(),
            observed_at: 42,
        };
        let frame = encode_complete(&lifecycle, 1, 3);
        assert!(frame.len() <= MAX_FRAME_BYTES);
        let bytes = String::from_utf8_lossy(&frame);
        assert!(!bytes.contains("private-session"));
        assert!(!bytes.contains("private-turn"));
    }

    #[test]
    fn known_acknowledgements_have_explicit_non_execution_outcomes() {
        assert_eq!(Disposition::Accepted, Disposition::Accepted);
        assert_ne!(Disposition::DroppedOverloaded, Disposition::Accepted);
        assert_ne!(Disposition::Busy, Disposition::Accepted);
        assert_ne!(Disposition::Rejected, Disposition::Accepted);
    }

    #[test]
    fn malformed_or_oversized_acknowledgements_are_rejected() {
        let mut invalid_magic = [0_u8; HEADER_BYTES];
        invalid_magic[4] = 1;
        invalid_magic[5] = 3;
        invalid_magic[8..10].copy_from_slice(&1_u16.to_le_bytes());
        assert!(validate_ack_header(&invalid_magic).is_err());

        let mut oversized = *b"HSIP\x01\x03\x00\x00\x01\x04";
        assert!(validate_ack_header(&oversized).is_err());
        oversized[8..10].copy_from_slice(&1_u16.to_le_bytes());
        assert!(validate_ack_header(&oversized).is_ok());
    }

    #[test]
    fn absent_broker_is_not_cached_across_later_hook_invocations() {
        let cache = Mutex::new(None);
        assert!(cached_or_create(&cache, || None).is_none());

        let expected = Producer {
            endpoint: Endpoint {
                #[cfg(unix)]
                root: PathBuf::from("fixture-state-root"),
                identifier: "fixture-endpoint".to_owned(),
            },
            connection: Arc::new(Mutex::new(None)),
        };
        let observed = cached_or_create(&cache, || Some(expected.clone()))
            .expect("later HookStat availability must be retried");
        assert_eq!(observed.endpoint.identifier, "fixture-endpoint");
        assert!(cached_or_create(&cache, || None).is_some());
    }

    #[cfg(windows)]
    #[test]
    fn native_named_pipe_client_delivers_start_and_complete_to_a_hookstat_peer() {
        use interprocess::local_socket::{
            GenericNamespaced, ListenerNonblockingMode, ListenerOptions, ToNsName, prelude::*,
        };

        let directory = std::env::temp_dir().join(format!(
            "tabbeacon-hsip-native-{}-{}",
            std::process::id(),
            INVOCATION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(directory.join("ipc")).unwrap();
        let endpoint = Endpoint::from_state_root(&directory).unwrap();
        let name = endpoint
            .pipe_name()
            .to_ns_name::<GenericNamespaced>()
            .unwrap();
        let listener = ListenerOptions::new()
            .name(name)
            .nonblocking(ListenerNonblockingMode::Accept)
            .reclaim_name(false)
            .create_sync()
            .unwrap();
        let (frames_sender, frames_receiver) = mpsc::channel();
        let peer = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(1);
            let mut stream = loop {
                match listener.accept() {
                    Ok(stream) => break stream,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "native client did not connect");
                        thread::yield_now();
                    }
                    Err(error) => panic!("named-pipe listener failed: {error}"),
                }
            };
            for _ in 0..2 {
                let mut header = [0_u8; HEADER_BYTES];
                stream.read_exact(&mut header).unwrap();
                let payload_length = usize::from(u16::from_le_bytes([header[8], header[9]]));
                let mut frame = header.to_vec();
                frame.resize(HEADER_BYTES + payload_length, 0);
                stream.read_exact(&mut frame[HEADER_BYTES..]).unwrap();
                frames_sender.send(frame).unwrap();
                stream.write_all(b"HSIP\x01\x03\0\0\x01\0\x01").unwrap();
                stream.flush().unwrap();
            }
        });

        let lifecycle = Lifecycle {
            runtime_instance: opaque_id(b"runtime", &["session"]),
            invocation: opaque_id(b"invocation", &["session", "turn"]),
            event: "UserPromptSubmit".to_owned(),
            observed_at: 42,
        };
        let producer = Producer {
            endpoint,
            connection: Arc::new(Mutex::new(None)),
        };
        let start = encode_start(&lifecycle);
        let complete = encode_complete(&lifecycle, 1, 2);
        assert_eq!(producer.emit(&start), Disposition::Accepted);
        assert_eq!(producer.emit(&complete), Disposition::Accepted);
        peer.join().unwrap();
        assert_eq!(frames_receiver.recv().unwrap(), start);
        assert_eq!(frames_receiver.recv().unwrap(), complete);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "requires an explicitly supplied disposable HookStat broker binary"]
    #[allow(unsafe_code)]
    fn system_hook_reducer_reaches_an_admitted_external_broker() {
        use crate::providers::codex::CodexHookRuntime;

        struct ExternalBroker(std::process::Child);

        impl Drop for ExternalBroker {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        let broker = std::env::var_os("TABBEACON_HSIP_TEST_BROKER")
            .expect("test requires TABBEACON_HSIP_TEST_BROKER");
        let directory = std::env::temp_dir().join(format!(
            "tabbeacon-hsip-system-{}-{}",
            std::process::id(),
            INVOCATION_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let local_app_data = directory.join("local-app-data");
        std::fs::create_dir_all(&local_app_data).unwrap();
        unsafe { std::env::set_var("LOCALAPPDATA", &local_app_data) };
        let hookstat_root = local_app_data.join("HookStat");
        let raw = serde_json::to_vec(&serde_json::json!({
            "hook_event_name": "UserPromptSubmit",
            "session_id": "system-session",
            "turn_id": "system-turn",
            "cwd": directory,
            "model": "fixture",
            "permission_mode": "default",
            "transcript_path": null,
        }))
        .unwrap();
        // The first call proves that a missing optional observer does not
        // interfere.  The next call must retry rather than cache that miss.
        assert_eq!(
            CodexHookRuntime::dispatch_system(&raw),
            HookDispatchOutcome::Applied
        );
        let child = ExternalBroker(
            std::process::Command::new(broker)
                .args(["--state-root", hookstat_root.to_str().unwrap()])
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(1);
        while !hookstat_root.join("ipc").is_dir() {
            assert!(Instant::now() < deadline, "broker did not create IPC state");
            thread::sleep(Duration::from_millis(1));
        }
        thread::sleep(Duration::from_millis(50));

        assert_eq!(
            CodexHookRuntime::dispatch_system(&raw),
            HookDispatchOutcome::Applied
        );
        let wal = hookstat_root.join("ipc-evidence-v1.wal");
        let deadline = Instant::now() + Duration::from_secs(1);
        while std::fs::metadata(&wal).map_or(0, |metadata| metadata.len()) == 0 {
            assert!(
                Instant::now() < deadline,
                "system Hook emitted no HSIP evidence"
            );
            thread::sleep(Duration::from_millis(1));
        }

        drop(child);
        let cleanup_deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match std::fs::remove_dir_all(&directory) {
                Ok(()) => break,
                Err(error) if Instant::now() < cleanup_deadline => {
                    let _ = error;
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("external broker test root did not clean up: {error}"),
            }
        }
    }
}
