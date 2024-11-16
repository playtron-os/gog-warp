use std::path::PathBuf;
use std::sync::Arc;

use futures::{StreamExt, TryStreamExt};
use reqwest::Client;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncSeekExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;
use tokio_util::compat::FuturesAsyncReadCompatExt;

use async_compression::tokio::bufread::ZlibDecoder;

use crate::content_system::types::Endpoint;
use crate::content_system::types::{v1, v2};
use crate::errors::io_error;
use crate::errors::request_error;
use crate::errors::zlib_error;
use crate::errors::EmptyResult;
use crate::utils::{assemble_url, hash_to_galaxy_path};

use super::progress::{load_chunk_state, write_chunk_state, WorkerUpdate};

const BUFFER_SIZE: usize = 256 * 1024;

//TODO: handle downloads gracefully

pub async fn v1(
    _permit: OwnedSemaphorePermit,
    reqwest_client: Client,
    endpoints: Vec<Endpoint>,
    entry: v1::DepotEntry,
    destination_path: PathBuf,
    result_report: UnboundedSender<WorkerUpdate>,
) -> EmptyResult {
    let file = if let v1::DepotEntry::File(f) = entry {
        f
    } else {
        return Ok(());
    };
    let download_path = format!("{}.download", destination_path.to_str().unwrap());
    let endpoint = endpoints.first().unwrap();
    let url = assemble_url(endpoint, "main.bin");

    let Some(offset) = *file.offset() else {
        log::warn!("Offset was not set for v1 file, this shouldn't happen!");
        return Ok(());
    };
    let end = offset + *file.size() - 1;

    let mut file_handle = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&download_path)
        .await
        .expect("Failed to open the file");

    let response = reqwest_client
        .get(url)
        .header("Range", format!("bytes={}-{}", offset, end))
        .send()
        .await
        .map_err(request_error)?;

    let mut stream = response.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(io_error)?;
        let _ = result_report.send(WorkerUpdate::Download(chunk.len()));
        file_handle.write_all(&chunk).await.map_err(io_error)?;
        let _ = result_report.send(WorkerUpdate::Write(chunk.len()));
    }

    file_handle.flush().await.map_err(io_error)?;
    drop(file_handle);

    tokio::fs::rename(download_path, destination_path)
        .await
        .map_err(io_error)?;
    Ok(())
}

pub async fn v2(
    _permit: OwnedSemaphorePermit,
    reqwest_client: Client,
    chunk_semaphore: Arc<Semaphore>,
    endpoints: Vec<Endpoint>,
    entry: v2::DepotEntry,
    destination_path: PathBuf,
    result_report: UnboundedSender<WorkerUpdate>,
) -> EmptyResult {
    let chunks = match &entry {
        v2::DepotEntry::File(file) => file.chunks.clone(),
        v2::DepotEntry::Diff(diff) => diff.chunks.clone(),
        _ => return Ok(()),
    };

    let download_path = format!("{}.download", destination_path.to_str().unwrap());
    let state_path = format!("{}.state", destination_path.to_str().unwrap());

    let mut file_handle = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&download_path)
        .await
        .map_err(io_error)?;

    let mut state_file: Option<File> = if chunks.len() > 1 {
        let new_handle = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&state_path)
            .await
            .map_err(io_error)?;
        Some(new_handle)
    } else {
        None
    };

    let endpoint = endpoints.first().unwrap();
    let mut handles: Vec<_> = Vec::new();

    let mut state = load_chunk_state(&state_path).await.unwrap_or_default();
    state.header.number_of_chunks = chunks.len() as u32;
    state.chunks.resize(chunks.len(), false);

    let mut offset: i64 = 0;
    for (index, chunk) in chunks.into_iter().enumerate() {
        let reqwest_client = reqwest_client.clone();
        let chunk_semaphore = chunk_semaphore.clone();
        let chunk_offset = offset;
        offset += chunk.size();
        if *state.chunks.get(index).unwrap_or(&false) {
            continue;
        }
        let result_report = result_report.clone();
        let chunk_handle = async move {
            let _permit = chunk_semaphore.acquire().await.unwrap();
            let galaxy_path = hash_to_galaxy_path(chunk.compressed_md5());
            let url = assemble_url(endpoint, &galaxy_path);

            tokio::spawn(async move {
                let response = reqwest_client
                    .get(url)
                    .send()
                    .await
                    .map_err(request_error)?;

                let chunk_data = response.bytes_stream();
                let chunk_data = chunk_data
                    .map_err(|e| futures::io::Error::new(futures::io::ErrorKind::Other, e))
                    .into_async_read();
                let reader = BufReader::with_capacity(BUFFER_SIZE, chunk_data.compat());
                let mut decompressed_data = ZlibDecoder::new(reader);
                let mut buffer = Vec::with_capacity((*chunk.size()).try_into().unwrap());
                decompressed_data
                    .read_to_end(&mut buffer)
                    .await
                    .map_err(zlib_error)?;

                let _ =
                    result_report.send(WorkerUpdate::Download(*chunk.compressed_size() as usize));
                Ok((buffer, index, chunk_offset))
            })
            .await
            .unwrap()
        };
        handles.push(chunk_handle)
    }

    let mut stream = futures::stream::iter(handles).buffer_unordered(6);

    while let Some(chunk) = stream.next().await {
        let (chunk, index, offset) = chunk?;
        file_handle
            .seek(std::io::SeekFrom::Start(offset.try_into().unwrap()))
            .await
            .map_err(io_error)?;
        file_handle.write_all(&chunk).await.map_err(io_error)?;
        let _ = result_report.send(WorkerUpdate::Write(chunk.len()));
        *state.chunks.get_mut(index).unwrap() = true;
        if let Some(state_file) = &mut state_file {
            write_chunk_state(state_file, &state)
                .await
                .map_err(io_error)?;
            state_file
                .seek(std::io::SeekFrom::Start(0))
                .await
                .map_err(io_error)?;
            state_file.flush().await.map_err(io_error)?;
        }
    }
    file_handle.flush().await.map_err(io_error)?;
    drop(file_handle);
    drop(state_file);

    tokio::fs::rename(download_path, destination_path)
        .await
        .map_err(io_error)?;
    let _ = tokio::fs::remove_file(state_path).await.map_err(io_error);
    Ok(())
}
