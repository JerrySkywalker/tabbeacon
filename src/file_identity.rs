//! Internal identity observation for Preview-bound external configuration files.
//! It detects observed replacement; it is not a foreign-writer atomic commit.
use std::{fs, io, path::Path};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileIdentity {
    device: u64,
    index: u64,
}

impl FileIdentity {
    pub(crate) fn optional(path: &Path) -> io::Result<Option<Self>> {
        match Self::read(path) {
            Ok(identity) => Ok(Some(identity)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    #[cfg(windows)]
    #[allow(unsafe_code)] // Only the borrowed owned file handle is queried; no mutation.
    fn read(path: &Path) -> io::Result<Self> {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        use windows::Win32::{
            Foundation::HANDLE,
            Storage::FileSystem::{
                BY_HANDLE_FILE_INFORMATION, FILE_FLAG_OPEN_REPARSE_POINT,
                GetFileInformationByHandle,
            },
        };
        let file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
            .open(path)?;
        let mut info = BY_HANDLE_FILE_INFORMATION::default();
        // SAFETY: file owns the live handle and info is writable for the entire call.
        unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &raw mut info) }
            .map_err(io::Error::other)?;
        if info.dwFileAttributes & (0x0400 | 0x0010) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsafe configuration file identity",
            ));
        }
        Ok(Self {
            device: u64::from(info.dwVolumeSerialNumber),
            index: (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        })
    }

    #[cfg(unix)]
    fn read(path: &Path) -> io::Result<Self> {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsafe configuration file identity",
            ));
        }
        Ok(Self {
            device: metadata.dev(),
            index: metadata.ino(),
        })
    }
}
