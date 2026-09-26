//! Bounded acquisition for short-lived, cross-process state locks.

use std::{
    fs::{self, File},
    io, thread,
    time::{Duration, Instant},
};

pub(crate) fn try_lock_file_with_budget(lock: &File, budget: Duration) -> io::Result<()> {
    let deadline = Instant::now() + budget;
    loop {
        match lock.try_lock() {
            Ok(()) => return Ok(()),
            Err(fs::TryLockError::WouldBlock) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(fs::TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "TabBeacon state lock remained busy",
                ));
            }
            Err(error) => return Err(error.into()),
        }
    }
}
