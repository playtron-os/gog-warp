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

#[cfg(not(target_os = "linux"))]
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
