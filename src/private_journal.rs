//! Dedicated, verified private directory for journals carrying raw local
//! configuration bytes. No journal writer may fall back to an ambient ACL.

use std::{fs, io, path::Path};

/// Creates the exact owned directory with a private ACL or verifies an
/// existing one before any sensitive journal bytes are written.
pub(crate) fn ensure_private_journal_dir(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    return windows_private::ensure(path);
    #[cfg(unix)]
    return unix_private::ensure(path);
    #[cfg(not(any(windows, unix)))]
    {
        let _ = path;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "private journal directory unavailable",
        ))
    }
}

/// Refuses an existing journal file whose owner or allow ACEs drifted beyond
/// the current user and SYSTEM. Call before reading and after every commit.
pub(crate) fn verify_private_journal_file(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    return windows_private::verify_file(path);
    #[cfg(unix)]
    return unix_private::verify_file(path);
    #[cfg(not(any(windows, unix)))]
    {
        let _ = path;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "private journal file unavailable",
        ))
    }
}

/// Seals a newly created owned journal file before writing, or immediately
/// after an atomic replacement inside the verified private directory.
pub(crate) fn seal_private_journal_file(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    verify_private_journal_file(path)
}

#[cfg(unix)]
#[allow(unsafe_code)]
mod unix_private {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    unsafe extern "C" {
        fn geteuid() -> u32;
    }

    pub(super) fn ensure(path: &Path) -> io::Result<()> {
        if !path.exists() {
            let mut builder = fs::DirBuilder::new();
            builder.mode(0o700);
            match builder.create(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        let metadata = fs::symlink_metadata(path)?;
        // SAFETY: geteuid has no arguments and only reads the process identity.
        let owner = unsafe { geteuid() };
        if !metadata.file_type().is_dir()
            || metadata.uid() != owner
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "journal directory is not private",
            ));
        }
        Ok(())
    }

    pub(super) fn verify_file(path: &Path) -> io::Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        // SAFETY: geteuid has no arguments and only reads the process identity.
        let owner = unsafe { geteuid() };
        if !metadata.file_type().is_file()
            || metadata.uid() != owner
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "journal file is not private",
            ));
        }
        Ok(())
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
#[allow(
    clippy::borrow_as_ptr,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)] // Win32 FFI uses raw out-pointers and fixed-width OS status/length types.
mod windows_private {
    use super::{Path, fs, io};
    use std::{
        ffi::OsStr,
        iter,
        mem::size_of,
        os::windows::{ffi::OsStrExt, fs::MetadataExt},
    };
    use windows::{
        Win32::{
            Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree},
            Security::{
                Authorization::{
                    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
                    ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW,
                    SE_FILE_OBJECT,
                },
                DACL_SECURITY_INFORMATION, GetTokenInformation, OWNER_SECURITY_INFORMATION,
                PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER, TokenUser,
            },
            Storage::FileSystem::CreateDirectoryW,
            System::Threading::{GetCurrentProcess, OpenProcessToken},
        },
        core::{BOOL, PCWSTR, PWSTR},
    };

    fn wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(iter::once(0)).collect()
    }

    fn current_user_sid() -> io::Result<String> {
        let mut token = HANDLE::default();
        // SAFETY: the token handle is initialized by OpenProcessToken and
        // closed here; the aligned buffer lives through both API calls.
        unsafe {
            OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token)
                .map_err(io::Error::other)?;
            let result = (|| {
                let mut size = 0;
                let _ = GetTokenInformation(token, TokenUser, None, 0, &mut size);
                if size < size_of::<TOKEN_USER>() as u32 {
                    return Err(io::Error::other("TokenUser size unavailable"));
                }
                let mut aligned = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
                GetTokenInformation(
                    token,
                    TokenUser,
                    Some(aligned.as_mut_ptr().cast()),
                    size,
                    &mut size,
                )
                .map_err(io::Error::other)?;
                let user = &*aligned.as_ptr().cast::<TOKEN_USER>();
                let mut text = PWSTR::null();
                ConvertSidToStringSidW(user.User.Sid, &mut text).map_err(io::Error::other)?;
                let value = text.to_string().map_err(io::Error::other);
                let _ = LocalFree(Some(HLOCAL(text.0.cast())));
                value
            })();
            let _ = CloseHandle(token);
            result
        }
    }

    fn descriptor_sddl(descriptor: PSECURITY_DESCRIPTOR) -> io::Result<String> {
        let mut text = PWSTR::null();
        // SAFETY: descriptor is a live security descriptor returned by a
        // Windows security API; the allocated result is freed immediately.
        unsafe {
            ConvertSecurityDescriptorToStringSecurityDescriptorW(
                descriptor,
                1,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut text,
                None,
            )
            .map_err(io::Error::other)?;
            let value = text.to_string().map_err(io::Error::other);
            let _ = LocalFree(Some(HLOCAL(text.0.cast())));
            value
        }
    }

    pub(super) fn ensure(path: &Path) -> io::Result<()> {
        let sid = current_user_sid()?;
        let sddl = format!("O:{sid}D:P(A;OICI;FA;;;{sid})(A;OICI;FA;;;SY)");
        let mut expected = PSECURITY_DESCRIPTOR::default();
        let sddl_wide = wide(OsStr::new(&sddl));
        let path_wide = wide(path.as_os_str());
        // SAFETY: wide strings are NUL terminated. Windows owns each returned
        // descriptor until LocalFree below. The explicit protected DACL
        // includes only this user and SYSTEM and is inheritable by journal files.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl_wide.as_ptr()),
                1,
                &mut expected,
                None,
            )
            .map_err(io::Error::other)?;
            let result = (|| {
                let attributes = SECURITY_ATTRIBUTES {
                    nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                    lpSecurityDescriptor: expected.0,
                    bInheritHandle: BOOL(0),
                };
                match CreateDirectoryW(PCWSTR(path_wide.as_ptr()), Some(&attributes)) {
                    Ok(()) => {}
                    Err(error) if (error.code().0 as u32 & 0xffff) == 183 => {}
                    Err(error) => return Err(io::Error::other(error)),
                }
                let metadata = fs::symlink_metadata(path)?;
                if !metadata.file_type().is_dir() || metadata.file_attributes() & 0x400 != 0 {
                    return Err(io::Error::other(
                        "journal directory is not an owned directory",
                    ));
                }
                let mut actual = PSECURITY_DESCRIPTOR::default();
                let status = GetNamedSecurityInfoW(
                    PCWSTR(path_wide.as_ptr()),
                    SE_FILE_OBJECT,
                    OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                    None,
                    None,
                    None,
                    None,
                    &mut actual,
                );
                if status.0 != 0 {
                    return Err(io::Error::from_raw_os_error(status.0 as i32));
                }
                let actual_sddl = descriptor_sddl(actual);
                let _ = LocalFree(Some(HLOCAL(actual.0)));
                if actual_sddl? != descriptor_sddl(expected)? {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "journal directory ACL is not private",
                    ));
                }
                Ok(())
            })();
            let _ = LocalFree(Some(HLOCAL(expected.0)));
            result
        }
    }

    pub(super) fn verify_file(path: &Path) -> io::Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() || metadata.file_attributes() & 0x400 != 0 {
            return Err(io::Error::other("journal file is not a regular owned file"));
        }
        let sid = current_user_sid()?;
        let path_wide = wide(path.as_os_str());
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        // SAFETY: the path is NUL terminated and the security descriptor is
        // returned by Windows, then freed after conversion.
        let sddl = unsafe {
            let status = GetNamedSecurityInfoW(
                PCWSTR(path_wide.as_ptr()),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                None,
                None,
                None,
                None,
                &mut descriptor,
            );
            if status.0 != 0 {
                return Err(io::Error::from_raw_os_error(status.0 as i32));
            }
            let text = descriptor_sddl(descriptor);
            let _ = LocalFree(Some(HLOCAL(descriptor.0)));
            text?
        };
        let (owner, dacl) = sddl.split_once("D:").ok_or_else(|| {
            io::Error::new(io::ErrorKind::PermissionDenied, "journal file DACL missing")
        })?;
        if owner != format!("O:{sid}") {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "journal file owner changed",
            ));
        }
        let mut user_allow = false;
        let mut system_allow = false;
        let mut entries = 0;
        for fragment in dacl.split('(').skip(1) {
            let entry = fragment
                .split_once(')')
                .map(|(value, _)| value)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::PermissionDenied, "journal file ACE invalid")
                })?;
            let fields = entry.split(';').collect::<Vec<_>>();
            if fields.len() != 6 || fields[0] != "A" {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "journal file ACE unsupported",
                ));
            }
            match fields[5] {
                trustee if trustee == sid => user_allow = true,
                "SY" => system_allow = true,
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "journal file ACL grants another principal",
                    ));
                }
            }
            entries += 1;
        }
        if entries != 2 || !user_allow || !system_allow {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "journal file ACL is incomplete",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_private_directory_is_verified_on_reopen() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("private-journal");
        ensure_private_journal_dir(&path).unwrap();
        ensure_private_journal_dir(&path).unwrap();
        let file = path.join("journal.json");
        fs::write(&file, b"synthetic local configuration bytes").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        }
        verify_private_journal_file(&file).unwrap();
        assert!(fs::metadata(path).unwrap().is_dir());
    }

    #[cfg(windows)]
    #[test]
    fn journal_file_with_external_read_ace_is_refused() {
        use std::process::Command;

        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("private-journal");
        ensure_private_journal_dir(&path).unwrap();
        let file = path.join("journal.json");
        fs::write(&file, b"synthetic local configuration bytes").unwrap();
        assert!(verify_private_journal_file(&file).is_ok());
        let status = Command::new("icacls")
            .arg(&file)
            .args(["/grant", "*S-1-1-0:R"])
            .output()
            .unwrap()
            .status;
        assert!(status.success());
        assert!(verify_private_journal_file(&file).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn journal_directory_with_external_read_ace_is_refused() {
        use std::process::Command;

        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("private-journal");
        ensure_private_journal_dir(&path).unwrap();
        let status = Command::new("icacls")
            .arg(&path)
            .args(["/grant", "*S-1-1-0:R"])
            .output()
            .unwrap()
            .status;
        assert!(status.success());
        assert!(ensure_private_journal_dir(&path).is_err());
    }
}
