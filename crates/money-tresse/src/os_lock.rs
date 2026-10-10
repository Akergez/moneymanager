//! The same nonblocking exclusive `flock` used by Tresse's Unix CLI.
//!
//! Rust's standard file locking is unsupported on Android. Use the native
//! syscall on Android and Linux, where we also test CLI interoperability.
//! Closing the file releases the lock, including when a process dies.

use std::fs::{File, TryLockError};

#[cfg(any(target_os = "linux", target_os = "android"))]
pub(crate) fn try_lock(file: &File) -> Result<(), TryLockError> {
    use std::io::{Error, ErrorKind};
    use std::os::fd::AsRawFd;

    loop {
        // SAFETY: the borrowed File keeps its descriptor open for this call;
        // flock does not retain Rust pointers or take ownership of the fd.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 {
            return Ok(());
        }
        let error = Error::last_os_error();
        match error.kind() {
            ErrorKind::Interrupted => continue,
            ErrorKind::WouldBlock => return Err(TryLockError::WouldBlock),
            _ => return Err(TryLockError::Error(error)),
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub(crate) fn try_lock(file: &File) -> Result<(), TryLockError> {
    file.try_lock()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_lock_blocks_another_open_and_is_released_on_close() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lock");
        let first = File::create(&path).unwrap();
        let second = File::options().read(true).write(true).open(&path).unwrap();
        try_lock(&first).unwrap();
        assert!(matches!(try_lock(&second), Err(TryLockError::WouldBlock)));
        drop(first);
        try_lock(&second).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn native_and_standard_cli_locks_exclude_each_other() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("lock");
        let native = File::create(&path).unwrap();
        let cli = File::options().read(true).write(true).open(&path).unwrap();
        try_lock(&native).unwrap();
        assert!(matches!(cli.try_lock(), Err(TryLockError::WouldBlock)));
        drop(native);
        cli.try_lock().unwrap();
        let native = File::options().read(true).write(true).open(&path).unwrap();
        assert!(matches!(try_lock(&native), Err(TryLockError::WouldBlock)));
        drop(cli);
        try_lock(&native).unwrap();
    }
}
