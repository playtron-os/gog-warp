use crate::errors::io_error;
use crate::Error;
use std::path::Path;

use tokio::fs::File;

#[cfg(target_os = "linux")]
pub async fn allocate(file: File, size: i64) -> Result<(), Error> {
    use std::os::fd::AsRawFd;

    if size == 0 {
        return Ok(());
    }
    tokio::task::spawn_blocking(move || {
        let fd = file.as_raw_fd();
        let result = unsafe { libc::fallocate(fd, 0, 0, size) };

        if result != 0 && result != libc::EOPNOTSUPP {
            return Err(io_error("allocation error"));
        }
        Ok(())
    })
    .await
    .map_err(io_error)?
}

#[cfg(not(target_os = "linux"))]
pub async fn allocate(file: File, size: i64) -> Result<(), Error> {
    log::warn!("File pre-allocation is not implemented on this platform yet.");
    Ok(())
}

#[cfg(unix)]
pub fn symlink(install_root: &Path, path: &str, target: &str) -> Result<(), Error> {
    use libc::{open, symlinkat, O_DIRECTORY};
    use std::ffi::CString;

    let install_root_path = CString::new(install_root.to_str().unwrap()).map_err(io_error)?;
    let c_path = CString::new(path).map_err(io_error)?;
    let c_target = CString::new(target).map_err(io_error)?;
    let directory_fd = unsafe { open(install_root_path.as_ptr(), O_DIRECTORY) };

    if directory_fd == -1 {
        let error = unsafe { *libc::__errno_location() };
        return Err(io_error(format!("io error: {}", error)));
    }

    let ret = unsafe { symlinkat(c_target.as_ptr(), directory_fd, c_path.as_ptr()) };

    if ret == -1 {
        let error = unsafe { *libc::__errno_location() };
        return Err(io_error(format!("io error: {}", error)));
    }

    unsafe { libc::close(directory_fd) };

    Ok(())
}

#[cfg(not(unix))]
pub fn symlink(path: &String, target: &String) -> Result<(), Error> {
    // Symlinks are not available on Windows, and if they are they require elevated
    // privileges. Thus we ignore any symlinks.
    // In general no one should ever install a depot with symlinks in it on Windows.
    // But well...
    Ok(())
}
