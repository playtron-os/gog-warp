use crate::errors::io_error;
use crate::Error;

use tokio::fs::File;

#[cfg(target_os = "linux")]
pub async fn allocate(file: File, size: i64) -> Result<(), Error> {
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};

    if size == 0 {
        return Ok(());
    }
    let metadata = file.metadata().await.map_err(io_error)?;
    tokio::task::spawn_blocking(move || {
        let fd = file.as_raw_fd();
        let result = if metadata.size() as i64 > size {
            unsafe { libc::ftruncate(fd, size) }
        } else {
            unsafe { libc::fallocate(fd, 0, 0, size) }
        };

        if result != 0 {
            return Err(io_error("allocation error"));
        }
        Ok(())
    })
    .await
    .map_err(io_error)?
}

#[cfg(target_os = "macos")]
pub async fn allocate(file: File, size: i64) -> Result<(), Error> {
    use std::io;
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};

    if size == 0 {
        return Ok(());
    }
    let metadata = file.metadata().await.map_err(io_error)?;
    tokio::task::spawn_blocking(move || {
        let fd = file.as_raw_fd();
        if metadata.size() as i64 > size {
            // Shrink: ftruncate, same as the Linux arm.
            let result = unsafe { libc::ftruncate(fd, size) };
            if result != 0 {
                return Err(io_error("allocation error"));
            }
            return Ok(());
        }

        // Grow: prefer contiguous pre-allocation (F_PREALLOCATE), fall back to
        // any-space allocation, and finally to a plain extend — the download
        // writes every byte anyway; allocation is an optimization, not a
        // correctness requirement.
        unsafe {
            let mut st: libc::fstore_t = std::mem::zeroed();
            st.fst_flags = libc::F_ALLOCATECONTIG;
            st.fst_posmode = libc::F_PEOFPOSMODE;
            st.fst_offset = 0;
            st.fst_length = size;
            if libc::fcntl(fd, libc::F_PREALLOCATE, &st) == 0 {
                return Ok(());
            }
            st.fst_flags = libc::F_ALLOCATEALL;
            if libc::fcntl(fd, libc::F_PREALLOCATE, &st) == 0 {
                return Ok(());
            }
        }

        use std::os::fd::FromRawFd;
        // SAFETY: `fd` is owned for the remainder of this closure; `file` was
        // moved in and is not used afterwards.
        let std_file = unsafe { std::fs::File::from_raw_fd(fd) };
        let res = std_file.set_len(size as u64).map_err(|_| io_error("allocation error"));
        std::mem::forget(std_file);
        res
    })
    .await
    .map_err(io_error)?
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub async fn allocate(file: File, size: i64) -> Result<(), Error> {
    log::error!("File pre-allocation is not implemented on this platform yet.");
    Err(io_error("pre allocation not implemented"))
}

#[cfg(unix)]
pub fn symlink(path: &str, target: &str) -> Result<(), Error> {
    std::os::unix::fs::symlink(target, path).map_err(io_error)
}

#[cfg(not(unix))]
pub fn symlink(path: &String, target: &String) -> Result<(), Error> {
    // Symlinks are not available on older versions of Windows, and if they are they require elevated
    // privileges. Thus we ignore any symlinks.
    // In general no one should ever install a depot with symlinks in it on Windows.
    Ok(())
}
