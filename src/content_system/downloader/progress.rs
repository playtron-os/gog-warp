pub(crate) enum DownloadFileStatus {
    NotInitialized,
    Allocated,
    Partial(Vec<bool>), // Chunks that are downloaded
    PatchDownloaded,
    Done,
}

#[derive(Debug)]
pub enum DownloadState {
    Preparing,
    Allocating(f32),
    Downloading(DownloadProgress),
    Finished,
}

#[derive(Default, Debug, Clone)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub written: u64,
    pub total_download: u64,
    pub total_size: u64,
    pub avg_network: f32,
    pub avg_disk: f32,
}

pub enum WorkerUpdate {
    Download(usize),
    Write(usize),
}

use crate::errors::{io_error, serde_error, EmptyResult};
use serde::{Deserialize, Serialize};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};

#[derive(Default, Serialize, Deserialize)]
pub(crate) struct FileDownloadState {
    pub(crate) header: DownloadStateHeader,
    pub(crate) chunks: Vec<bool>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct DownloadStateHeader {
    pub(crate) version: u8,
    pub(crate) number_of_chunks: u32,
}

impl Default for DownloadStateHeader {
    fn default() -> Self {
        Self {
            version: 1,
            number_of_chunks: 0,
        }
    }
}

pub(crate) async fn load_chunk_state(state_file: &str) -> Result<FileDownloadState, crate::Error> {
    let mut file = OpenOptions::new()
        .read(true)
        .open(state_file)
        .await
        .map_err(io_error)?;
    let mut buffer: Vec<u8> = Vec::new();
    file.read_to_end(&mut buffer).await.map_err(io_error)?;

    let new_state: FileDownloadState = bincode::deserialize(&buffer).map_err(serde_error)?;
    Ok(new_state)
}

pub(crate) async fn write_chunk_state(
    state_file: &mut File,
    state: &FileDownloadState,
) -> EmptyResult {
    let new_buffer: Vec<u8> = bincode::serialize(state).map_err(serde_error)?;
    state_file.write_all(&new_buffer).await.map_err(io_error)?;
    Ok(())
}
