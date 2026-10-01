//! Bounded, Windows-only recovery of the native Codex ancestor's console.
//!
//! No process enumeration, process memory/environment reads, protocol-stdout
//! writes, or provider changes. `WT_SESSION` comes from Codex's process
//! environment snapshot; OS ancestry and the installed native image
//! independently constrain where this Hook may attach.

#[cfg(windows)]
mod windows_route {
    use std::{env, fs, io, mem, path::PathBuf, time::Instant};

    use windows::Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_TIMEOUT},
        System::{
            Console::{
                AttachConsole, CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, FreeConsole,
                GetConsoleMode, GetConsoleProcessList, GetStdHandle, STD_ERROR_HANDLE,
                STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
            },
            Threading::{
                GetProcessTimes, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_INFORMATION,
                PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW,
                WaitForSingleObject,
            },
        },
    };

    const MAX_ANCESTORS: usize = 8;
    const MAX_CONSOLE_MEMBERS: usize = 256;

    #[repr(C)]
    #[derive(Default)]
    struct BasicProcessInformation {
        exit_status: usize,
        peb: usize,
        affinity: usize,
        priority: usize,
        pid: usize,
        parent_pid: usize,
    }

    // SAFETY: ProcessBasicInformation is fixed-size metadata. No process memory
    // is dereferenced; only the kernel's PID/parent relation is consumed.
    #[allow(unsafe_code)]
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQueryInformationProcess(
            process: HANDLE,
            class: u32,
            information: *mut BasicProcessInformation,
            length: u32,
            returned: *mut u32,
        ) -> i32;
    }

    #[allow(unsafe_code)]
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn ProcessIdToSessionId(pid: u32, session: *mut u32) -> i32;
    }

    struct Process(HANDLE);

    impl Drop for Process {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    /// Keeps the recovered console alive through Hook writes and worker spawn.
    /// Workers inherit this console with their existing explicit NUL handle list.
    pub(in crate::providers::codex) struct ConsoleRouteGuard {
        attached: bool,
        stdio: [HANDLE; 3],
    }

    impl ConsoleRouteGuard {
        #[allow(unsafe_code)]
        pub(in crate::providers::codex) fn acquire(session_id: &str) -> io::Result<Self> {
            Self::acquire_expected(
                session_id,
                super::super::capability::native_console_program(),
            )
        }

        #[allow(unsafe_code)]
        fn acquire_expected(session_id: &str, expected: Option<PathBuf>) -> io::Result<Self> {
            let stdio = unsafe {
                [
                    GetStdHandle(STD_INPUT_HANDLE),
                    GetStdHandle(STD_OUTPUT_HANDLE),
                    GetStdHandle(STD_ERROR_HANDLE),
                ]
            }
            .map(Result::unwrap_or_default);
            let unchanged = Self {
                attached: false,
                stdio,
            };
            let Some(expected) = expected else {
                return if current_console_vt_enabled() {
                    Ok(unchanged)
                } else {
                    Err(unavailable())
                };
            };
            let Some(ancestor) = proven_native_ancestor(&expected)? else {
                return if current_console_vt_enabled() {
                    Ok(unchanged)
                } else {
                    Err(unavailable())
                };
            };
            if console_contains(ancestor.pid) && current_console_vt_enabled() {
                return Ok(unchanged);
            }
            if !session_binding_matches(env::var("WT_SESSION").ok().as_deref(), session_id)
                || unsafe { WaitForSingleObject(ancestor.process.0, 0) } != WAIT_TIMEOUT
            {
                return Err(unavailable());
            }

            // FreeConsole affects this one-shot Hook only. In particular it
            // never detaches or changes the Codex process or Terminal settings.
            unsafe { FreeConsole() }.map_err(io::Error::other)?;
            let attached = unsafe { AttachConsole(ancestor.pid) }.is_ok();
            let guard = Self { attached, stdio };
            guard.restore_stdio()?;
            if !attached
                || unsafe { WaitForSingleObject(ancestor.process.0, 0) } != WAIT_TIMEOUT
                || !console_contains(ancestor.pid)
                || !current_console_vt_enabled()
            {
                return Err(unavailable());
            }
            Ok(guard)
        }

        #[allow(unsafe_code)]
        fn restore_stdio(&self) -> io::Result<()> {
            for (identifier, handle) in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE]
                .into_iter()
                .zip(self.stdio)
            {
                unsafe { SetStdHandle(identifier, handle) }.map_err(io::Error::other)?;
            }
            Ok(())
        }
    }

    impl Drop for ConsoleRouteGuard {
        #[allow(unsafe_code)]
        fn drop(&mut self) {
            if self.attached {
                let _ = unsafe { FreeConsole() };
                let _ = self.restore_stdio();
            }
        }
    }

    struct Ancestor {
        process: Process,
        pid: u32,
    }

    #[allow(unsafe_code)]
    fn proven_native_ancestor(expected: &PathBuf) -> io::Result<Option<Ancestor>> {
        let expected = fs::canonicalize(expected)?;
        let started = Instant::now();
        let mut pid = std::process::id();
        let mut child_created = u64::MAX;
        let mut seen = Vec::with_capacity(MAX_ANCESTORS);
        let mut own_session = 0;
        if unsafe { ProcessIdToSessionId(pid, &raw mut own_session) } == 0 {
            return Err(io::Error::last_os_error());
        }
        for _ in 0..MAX_ANCESTORS {
            if pid == 0 || seen.contains(&pid) || started.elapsed().as_millis() >= 100 {
                return Ok(None);
            }
            seen.push(pid);
            let process = Process(
                unsafe {
                    OpenProcess(
                        PROCESS_QUERY_INFORMATION
                            | PROCESS_QUERY_LIMITED_INFORMATION
                            | PROCESS_SYNCHRONIZE,
                        false,
                        pid,
                    )
                }
                .map_err(io::Error::other)?,
            );
            if unsafe { WaitForSingleObject(process.0, 0) } != WAIT_TIMEOUT {
                return Ok(None);
            }
            let mut session = 0;
            if unsafe { ProcessIdToSessionId(pid, &raw mut session) } == 0 {
                return Err(io::Error::last_os_error());
            }
            let mut created = FILETIME::default();
            let mut exit = FILETIME::default();
            let mut kernel = FILETIME::default();
            let mut user = FILETIME::default();
            unsafe {
                GetProcessTimes(
                    process.0,
                    &raw mut created,
                    &raw mut exit,
                    &raw mut kernel,
                    &raw mut user,
                )
            }
            .map_err(io::Error::other)?;
            let created =
                (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
            if session != own_session || created > child_created {
                return Ok(None);
            }
            child_created = created;
            let mut path = vec![0_u16; 32768].into_boxed_slice();
            let mut length = u32::try_from(path.len()).unwrap_or(0);
            unsafe {
                QueryFullProcessImageNameW(
                    process.0,
                    PROCESS_NAME_WIN32,
                    windows::core::PWSTR(path.as_mut_ptr()),
                    &raw mut length,
                )
            }
            .map_err(io::Error::other)?;
            let path = PathBuf::from(String::from_utf16_lossy(&path[..length as usize]));
            if pid != std::process::id()
                && fs::canonicalize(path).is_ok_and(|path| path == expected)
            {
                return Ok(Some(Ancestor { process, pid }));
            }
            let mut basic = BasicProcessInformation::default();
            let mut returned = 0;
            let status = unsafe {
                NtQueryInformationProcess(
                    process.0,
                    0,
                    &raw mut basic,
                    u32::try_from(mem::size_of::<BasicProcessInformation>()).unwrap_or(0),
                    &raw mut returned,
                )
            };
            if status < 0
                || basic.pid != pid as usize
                || returned as usize != mem::size_of::<BasicProcessInformation>()
            {
                return Ok(None);
            }
            pid = u32::try_from(basic.parent_pid).unwrap_or(0);
        }
        Ok(None)
    }

    #[allow(unsafe_code)]
    fn console_contains(pid: u32) -> bool {
        let mut members = [0_u32; MAX_CONSOLE_MEMBERS];
        let count = unsafe { GetConsoleProcessList(&mut members) } as usize;
        count > 0 && count <= members.len() && members[..count].contains(&pid)
    }

    #[allow(unsafe_code)]
    fn current_console_vt_enabled() -> bool {
        use std::{fs::OpenOptions, os::windows::io::AsRawHandle};
        let Ok(output) = OpenOptions::new().read(true).write(true).open("CONOUT$") else {
            return false;
        };
        let mut mode = CONSOLE_MODE::default();
        unsafe { GetConsoleMode(HANDLE(output.as_raw_handle()), &raw mut mode) }.is_ok()
            && mode.0 & ENABLE_VIRTUAL_TERMINAL_PROCESSING.0 != 0
    }

    fn unavailable() -> io::Error {
        io::Error::new(
            io::ErrorKind::NotConnected,
            "native Codex terminal route unproven",
        )
    }

    fn session_binding_matches(wt: Option<&str>, session: &str) -> bool {
        // Hooks::new snapshots vars_os; it does NOT set CODEX_THREAD_ID for
        // each thread. Such a variable may belong to an outer agent process.
        wt.is_some_and(|value| !value.trim().is_empty()) && !session.trim().is_empty()
    }

    #[cfg(test)]
    mod tests {
        use super::{ConsoleRouteGuard, console_contains, session_binding_matches};
        use std::{
            env,
            os::windows::process::CommandExt,
            process::{Command, Stdio},
        };
        use windows::Win32::System::Console::{
            GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
        };

        #[test]
        fn recovered_route_requires_terminal_and_normalized_provider_session() {
            assert!(session_binding_matches(Some("terminal"), "session"));
            for (wt, session) in [
                (None, "session"),
                (Some(" "), "session"),
                (Some("terminal"), ""),
                (Some("terminal"), " "),
            ] {
                assert!(!session_binding_matches(wt, session));
            }
        }

        // Re-exec isolates process-wide console association from the test suite.
        // The identity injection exists only inside this private unit test; the
        // shipped CLI always resolves the actual installed native Codex image.
        #[test]
        fn no_window_recovery_preserves_protocol_stdio_and_detaches() {
            const TEST: &str = "providers::codex::console_route::windows_route::tests::no_window_recovery_preserves_protocol_stdio_and_detaches";
            match env::var("TB_CONSOLE_ROUTE_TEST_ROLE").ok().as_deref() {
                Some("child") => child_route_check(),
                Some("host") => host_route_check(TEST),
                _ => {
                    let result = Command::new(env::current_exe().unwrap())
                        .args(["--exact", TEST, "--nocapture"])
                        .env("TB_CONSOLE_ROUTE_TEST_ROLE", "host")
                        .creation_flags(0x0800_0000)
                        .stdin(Stdio::null())
                        .output()
                        .unwrap();
                    assert!(
                        result.status.success(),
                        "owned console fixture: {}",
                        String::from_utf8_lossy(&result.stderr)
                    );
                    assert!(!result.stdout.contains(&0x1b), "no VT in protocol stdout");
                }
            }
        }

        #[allow(unsafe_code)]
        fn host_route_check(test: &str) {
            use std::{fs::OpenOptions, os::windows::io::AsRawHandle};
            use windows::Win32::{
                Foundation::HANDLE,
                System::Console::{
                    AllocConsole, CONSOLE_MODE, FreeConsole, GetConsoleMode, GetConsoleWindow,
                    SetConsoleMode,
                },
                UI::WindowsAndMessaging::{SW_HIDE, ShowWindow},
            };
            unsafe {
                FreeConsole().unwrap();
                AllocConsole().unwrap();
                let _ = ShowWindow(GetConsoleWindow(), SW_HIDE);
            }
            let output = OpenOptions::new()
                .read(true)
                .write(true)
                .open("CONOUT$")
                .unwrap();
            let handle = HANDLE(output.as_raw_handle());
            let mut mode = CONSOLE_MODE::default();
            unsafe {
                GetConsoleMode(handle, &raw mut mode).unwrap();
                SetConsoleMode(handle, CONSOLE_MODE(mode.0 | 4)).unwrap();
            }
            for invalid in [false, true] {
                let result = Command::new(env::current_exe().unwrap())
                    .args(["--exact", test, "--nocapture"])
                    .env("TB_CONSOLE_ROUTE_TEST_ROLE", "child")
                    .env("TB_CONSOLE_ROUTE_TEST_HOST", std::process::id().to_string())
                    .env(
                        "TB_CONSOLE_ROUTE_TEST_INVALID",
                        if invalid { "1" } else { "0" },
                    )
                    .env(
                        "WT_SESSION",
                        if invalid { " " } else { "owned-test-console" },
                    )
                    .env_remove("CODEX_THREAD_ID")
                    .creation_flags(0x0800_0000)
                    .stdin(Stdio::null())
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "owned child fixture: {}",
                    String::from_utf8_lossy(&result.stderr)
                );
                assert!(!result.stdout.contains(&0x1b));
            }
            drop(output);
            unsafe {
                FreeConsole().unwrap();
            }
        }

        #[allow(unsafe_code)]
        fn child_route_check() {
            let host: u32 = env::var("TB_CONSOLE_ROUTE_TEST_HOST")
                .unwrap()
                .parse()
                .unwrap();
            assert!(
                !console_contains(host),
                "CREATE_NO_WINDOW must isolate original console"
            );
            assert!(
                ConsoleRouteGuard::acquire_expected(
                    "owned-test-session",
                    Some(
                        std::path::PathBuf::from(env::var_os("SystemRoot").unwrap())
                            .join("System32")
                            .join("kernel32.dll")
                    ),
                )
                .is_err(),
                "a real unrelated image cannot confer ancestor authority"
            );
            assert!(!console_contains(host));
            let stdio = unsafe {
                [
                    GetStdHandle(STD_INPUT_HANDLE).unwrap(),
                    GetStdHandle(STD_OUTPUT_HANDLE).unwrap(),
                    GetStdHandle(STD_ERROR_HANDLE).unwrap(),
                ]
            };
            if env::var("TB_CONSOLE_ROUTE_TEST_INVALID").unwrap() == "1" {
                assert!(
                    ConsoleRouteGuard::acquire_expected(
                        "foreign-session",
                        Some(env::current_exe().unwrap())
                    )
                    .is_err()
                );
                assert!(!console_contains(host));
            } else {
                let guard = ConsoleRouteGuard::acquire_expected(
                    "owned-test-session",
                    Some(env::current_exe().unwrap()),
                )
                .unwrap();
                assert!(console_contains(host));
                let after = unsafe {
                    [
                        GetStdHandle(STD_INPUT_HANDLE).unwrap(),
                        GetStdHandle(STD_OUTPUT_HANDLE).unwrap(),
                        GetStdHandle(STD_ERROR_HANDLE).unwrap(),
                    ]
                };
                assert_eq!(stdio, after, "provider pipe handles retained");
                drop(guard);
                assert!(
                    !console_contains(host),
                    "only the Hook detaches at guard drop"
                );
            }
        }
    }
}

#[cfg(windows)]
pub(super) use windows_route::ConsoleRouteGuard;
