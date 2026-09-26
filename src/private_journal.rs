//! Dedicated, verified private directory for journals carrying raw local
//! configuration bytes. No journal writer may fall back to an ambient ACL.

use std::{fs, io, path::Path};

use std::fs::File;

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
    #[cfg(windows)]
    return windows_private::seal_file(path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
        return verify_private_journal_file(path);
    }
    #[cfg(not(any(windows, unix)))]
    verify_private_journal_file(path)
}

/// Seals an already opened, newly created journal handle before any raw
/// configuration bytes reach it. Atomic writers expose their temporary file
/// through this handle while its final path does not yet exist.
pub(crate) fn seal_private_journal_handle(file: &File, parent: &Path) -> io::Result<()> {
    #[cfg(windows)]
    return windows_private::seal_handle(file, parent);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        ensure_private_journal_dir(parent)?;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
        let metadata = file.metadata()?;
        // SAFETY: geteuid has no arguments and only reads process identity.
        let owner = unsafe { unix_private::geteuid() };
        if !metadata.is_file()
            || metadata.uid() != owner
            || metadata.permissions().mode() & 0o077 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "journal handle is not private",
            ));
        }
        return Ok(());
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (file, parent);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "private journal handle unavailable",
        ))
    }
}

#[cfg(unix)]
#[allow(unsafe_code)]
mod unix_private {
    use super::*;
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    unsafe extern "C" {
        pub(super) fn geteuid() -> u32;
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
    use super::{File, Path, fs, io};
    use std::{
        ffi::{OsStr, OsString},
        iter,
        mem::size_of,
        os::windows::{
            ffi::{OsStrExt, OsStringExt},
            fs::MetadataExt,
            io::AsRawHandle,
        },
    };
    use windows::{
        Win32::{
            Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree},
            Security::{
                Authorization::{
                    ConvertSecurityDescriptorToStringSecurityDescriptorW, ConvertSidToStringSidW,
                    ConvertStringSecurityDescriptorToSecurityDescriptorW, ConvertStringSidToSidW,
                    GetNamedSecurityInfoW, GetSecurityInfo, SE_FILE_OBJECT, SetNamedSecurityInfoW,
                },
                DACL_SECURITY_INFORMATION, EqualSid, GetTokenInformation,
                OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES,
                TOKEN_QUERY, TOKEN_USER, TokenUser,
            },
            Storage::FileSystem::{CreateDirectoryW, GetFinalPathNameByHandleW, VOLUME_NAME_DOS},
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

    fn sid_text_equals(left: &str, right: &str) -> io::Result<bool> {
        let mut left_sid = PSID::default();
        let mut right_sid = PSID::default();
        let left_text = wide(OsStr::new(left));
        let right_text = wide(OsStr::new(right));
        // SAFETY: both inputs are terminated SDDL trustees and both allocated
        // SIDs are freed even if the second conversion fails.
        unsafe {
            ConvertStringSidToSidW(PCWSTR(left_text.as_ptr()), &mut left_sid)
                .map_err(io::Error::other)?;
            let result = ConvertStringSidToSidW(PCWSTR(right_text.as_ptr()), &mut right_sid)
                .map(|()| EqualSid(left_sid, right_sid).is_ok())
                .map_err(io::Error::other);
            let _ = LocalFree(Some(HLOCAL(left_sid.0.cast())));
            if !right_sid.0.is_null() {
                let _ = LocalFree(Some(HLOCAL(right_sid.0.cast())));
            }
            result
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

    pub(super) fn seal_file(path: &Path) -> io::Result<()> {
        // Elevated Windows tokens may create a file owned by Administrators
        // even inside our private directory. First prove the inherited DACL
        // is private, then assign the current token user as owner. The file
        // still contains no bytes when first created; atomic replacements
        // inherit the same directory DACL before their owner is corrected.
        ensure(
            path.parent()
                .ok_or_else(|| io::Error::other("journal parent missing"))?,
        )?;
        verify_file_acl(path, false)?;
        let sid = current_user_sid()?;
        let sid_text = wide(OsStr::new(&sid));
        let path_text = wide(path.as_os_str());
        let mut parsed_sid = PSID::default();
        // SAFETY: both strings are terminated, and parsed_sid is freed after
        // SetNamedSecurityInfoW consumes it.
        unsafe {
            ConvertStringSidToSidW(PCWSTR(sid_text.as_ptr()), &mut parsed_sid)
                .map_err(io::Error::other)?;
            let status = SetNamedSecurityInfoW(
                PWSTR(path_text.as_ptr().cast_mut()),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION,
                Some(parsed_sid),
                None,
                None,
                None,
            );
            let _ = LocalFree(Some(HLOCAL(parsed_sid.0.cast())));
            if status.0 != 0 {
                return Err(io::Error::from_raw_os_error(status.0 as i32));
            }
        }
        verify_file(path)
    }

    pub(super) fn seal_handle(file: &File, parent: &Path) -> io::Result<()> {
        ensure(parent)?;
        verify_handle_acl(file, false)?;
        // AtomicWriteFile opens its temporary handle for writing but without
        // WRITE_OWNER. Resolve the name from that already opened handle, prove
        // it is still inside the exact private parent, then use the path API
        // to set ownership before writing any bytes.
        let mut path_buffer = vec![0u16; 32_768];
        // SAFETY: the result is written only into the supplied buffer.
        let length = unsafe {
            GetFinalPathNameByHandleW(
                HANDLE(file.as_raw_handle()),
                &mut path_buffer,
                VOLUME_NAME_DOS,
            )
        } as usize;
        if length == 0 || length >= path_buffer.len() {
            return Err(io::Error::other("journal temporary path unavailable"));
        }
        let temporary_path = std::path::PathBuf::from(OsString::from_wide(&path_buffer[..length]));
        let actual_parent = temporary_path
            .parent()
            .ok_or_else(|| io::Error::other("journal temporary parent missing"))?;
        if fs::canonicalize(actual_parent)? != fs::canonicalize(parent)? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "journal temporary file left private directory",
            ));
        }
        seal_file(&temporary_path)?;
        verify_handle_acl(file, true)
    }

    fn verify_handle_acl(file: &File, require_user_owner: bool) -> io::Result<()> {
        let metadata = file.metadata()?;
        if !metadata.file_type().is_file() || metadata.file_attributes() & 0x400 != 0 {
            return Err(io::Error::other("journal handle is not a regular file"));
        }
        let sid = current_user_sid()?;
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let mut owner = PSID::default();
        // SAFETY: the descriptor returned for this owned file handle remains
        // alive until after its owner and DACL have been examined.
        unsafe {
            let status = GetSecurityInfo(
                HANDLE(file.as_raw_handle()),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                Some(&mut owner),
                None,
                None,
                None,
                Some(&mut descriptor),
            );
            if status.0 != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("journal handle ACL read failed: {}", status.0),
                ));
            }
            let result = (|| {
                let sddl = descriptor_sddl(descriptor)?;
                let owner_matches = if require_user_owner {
                    let sid_text = wide(OsStr::new(&sid));
                    let mut expected = PSID::default();
                    ConvertStringSidToSidW(PCWSTR(sid_text.as_ptr()), &mut expected)
                        .map_err(io::Error::other)?;
                    let equal = EqualSid(owner, expected).is_ok();
                    let _ = LocalFree(Some(HLOCAL(expected.0.cast())));
                    equal
                } else {
                    true
                };
                verify_sddl_acl(&sddl, &sid, owner_matches)
            })();
            let _ = LocalFree(Some(HLOCAL(descriptor.0)));
            result
        }
    }

    pub(super) fn verify_file(path: &Path) -> io::Result<()> {
        verify_file_acl(path, true)
    }

    fn verify_file_acl(path: &Path, require_user_owner: bool) -> io::Result<()> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() || metadata.file_attributes() & 0x400 != 0 {
            return Err(io::Error::other("journal file is not a regular owned file"));
        }
        let sid = current_user_sid()?;
        let path_wide = wide(path.as_os_str());
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let mut actual_owner = PSID::default();
        // SAFETY: the path is NUL terminated and the security descriptor is
        // returned by Windows, then freed after conversion.
        let (sddl, owner_matches) = unsafe {
            let status = GetNamedSecurityInfoW(
                PCWSTR(path_wide.as_ptr()),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                Some(&mut actual_owner),
                None,
                None,
                None,
                &mut descriptor,
            );
            if status.0 != 0 {
                return Err(io::Error::from_raw_os_error(status.0 as i32));
            }
            let result = (|| {
                let text = descriptor_sddl(descriptor)?;
                let owner_matches = if require_user_owner {
                    let sid_text = wide(OsStr::new(&sid));
                    let mut expected_owner = PSID::default();
                    ConvertStringSidToSidW(PCWSTR(sid_text.as_ptr()), &mut expected_owner)
                        .map_err(io::Error::other)?;
                    let matches = EqualSid(actual_owner, expected_owner).is_ok();
                    let _ = LocalFree(Some(HLOCAL(expected_owner.0.cast())));
                    matches
                } else {
                    true
                };
                Ok::<_, io::Error>((text, owner_matches))
            })();
            let _ = LocalFree(Some(HLOCAL(descriptor.0)));
            result?
        };
        verify_sddl_acl(&sddl, &sid, owner_matches)
    }

    fn verify_sddl_acl(sddl: &str, sid: &str, owner_matches: bool) -> io::Result<()> {
        let (_, dacl) = sddl.split_once("D:").ok_or_else(|| {
            io::Error::new(io::ErrorKind::PermissionDenied, "journal file DACL missing")
        })?;
        if !owner_matches {
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
            if sid_text_equals(fields[5], sid)? {
                user_allow = true;
            } else if sid_text_equals(fields[5], "SY")? {
                system_allow = true;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "journal file ACL grants another principal",
                ));
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
    use std::io::Write;

    fn write_private_fixture(parent: &Path, path: &Path) {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(path)
            .unwrap();
        seal_private_journal_handle(&file, parent).unwrap();
        file.write_all(b"synthetic local configuration bytes")
            .unwrap();
        file.sync_all().unwrap();
    }

    #[test]
    fn atomic_temporary_journal_is_private_before_write() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("private-journal");
        ensure_private_journal_dir(&parent).unwrap();
        let path = parent.join("journal.json");
        let mut file = atomic_write_file::AtomicWriteFile::options()
            .open(&path)
            .unwrap();
        seal_private_journal_handle(file.as_file(), &parent).unwrap();
        file.write_all(b"synthetic configuration").unwrap();
        file.commit().unwrap();
        verify_private_journal_file(&path).unwrap();
    }

    #[test]
    fn owned_private_directory_is_verified_on_reopen() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("private-journal");
        ensure_private_journal_dir(&path).unwrap();
        ensure_private_journal_dir(&path).unwrap();
        let file = path.join("journal.json");
        write_private_fixture(&path, &file);
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
        write_private_fixture(&path, &file);
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
