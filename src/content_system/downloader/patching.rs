use tokio::sync::mpsc::UnboundedSender;

use crate::errors::{io_error, xdelta_error, EmptyResult};
use crate::xdelta::*;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::mem;

use super::progress::WorkerUpdate;

const BUFFER_SIZE: u32 = 1 << 18; // 256KB, the default for xdelta3 is 16KB

struct Xd3Stream {
    inner: xd3_stream,
}

impl Xd3Stream {
    pub fn new() -> Self {
        Self {
            inner: unsafe { mem::zeroed() },
        }
    }

    fn config(&mut self, config: &mut xd3_config) -> i32 {
        unsafe { xd3_config_stream(&mut self.inner, config) }
    }

    fn set_source(&mut self, source: &mut xd3_source) -> i32 {
        unsafe { xd3_set_source(&mut self.inner, source) }
    }

    fn update_flags(&mut self, flags: i32) {
        self.inner.flags |= flags;
    }
}

impl Drop for Xd3Stream {
    fn drop(&mut self) {
        unsafe { xd3_close_stream(&mut self.inner) };
        unsafe { xd3_free_stream(&mut self.inner) };
    }
}

pub fn patch_file(
    mut input: File,
    mut src: File,
    mut out: File,
    result_report: UnboundedSender<WorkerUpdate>,
) -> EmptyResult {
    let mut stream: Xd3Stream = Xd3Stream::new();
    let mut source: xd3_source = unsafe { mem::zeroed() };

    let mut config: xd3_config = unsafe { mem::zeroed() };
    config.winsize = BUFFER_SIZE;
    config.flags = xd3_flags::XD3_ADLER32 as i32;

    stream.config(&mut config);

    let mut source_block = [0; BUFFER_SIZE as usize];
    source.blksize = BUFFER_SIZE;
    source.curblk = source_block.as_ptr();

    src.seek(std::io::SeekFrom::Start(0)).map_err(io_error)?;

    source.onblk = src.read(&mut source_block).map_err(io_error)? as u32;
    source.curblkno = 0;

    stream.set_source(&mut source);

    input.seek(std::io::SeekFrom::Start(0)).map_err(io_error)?;
    let mut input_buffer = [0; BUFFER_SIZE as usize];
    let mut input_buf_read: u32;
    loop {
        input_buf_read = input.read(&mut input_buffer).map_err(io_error)? as u32;
        if input_buf_read < BUFFER_SIZE {
            stream.update_flags(xd3_flags::XD3_FLUSH as i32);
        }

        stream.inner.next_in = input_buffer.as_ptr();
        stream.inner.avail_in = input_buf_read;

        loop {
            let ret: xd3_rvalues = unsafe { mem::transmute(xd3_decode_input(&mut stream.inner)) };

            use xd3_rvalues::*;
            match ret {
                XD3_INPUT => break,
                XD3_OUTPUT => {
                    let output_buffer = unsafe {
                        std::slice::from_raw_parts(
                            stream.inner.next_out,
                            stream.inner.avail_out as usize,
                        )
                    };
                    let _ = result_report.send(WorkerUpdate::Write(output_buffer.len()));
                    out.write_all(output_buffer).map_err(io_error)?;
                    stream.inner.avail_out = 0;
                }
                XD3_GETSRCBLK => {
                    let block_size: u64 = source.blksize.into();
                    let block_offset = block_size * source.getblkno;
                    src.seek(std::io::SeekFrom::Start(block_offset))
                        .map_err(io_error)?;

                    source.onblk = src.read(&mut source_block).map_err(io_error)? as u32;
                    source.curblk = source_block.as_ptr();
                    source.curblkno = source.getblkno;
                }
                XD3_GOTHEADER | XD3_WINSTART | XD3_WINFINISH => (),
                _ => {
                    let msg = if !stream.inner.msg.is_null() {
                        unsafe { std::ffi::CStr::from_ptr(stream.inner.msg) }
                            .to_str()
                            .unwrap()
                            .to_string()
                    } else {
                        String::new()
                    };

                    return Err(xdelta_error(msg));
                }
            }
        }

        if input_buf_read != BUFFER_SIZE {
            break;
        }
    }

    out.flush().map_err(io_error)?;

    Ok(())
}
