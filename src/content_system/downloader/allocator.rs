use crate::errors::io_error;
use crate::Error;

use tokio::fs::File;

#[cfg(target_os = "linux")]
pub fn allocate(file: File, size: i64) -> Result<(), Error> {
    use std::os::fd::AsRawFd;

    if size == 0 {
        return Ok(());
    }

    let fd = file.as_raw_fd();
    let result = unsafe { libc::fallocate(fd, 0, 0, size) };

    if result != 0 {
        return Err(io_error("allocation error"));
    }

    Ok(())
}
