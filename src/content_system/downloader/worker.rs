use std::path::PathBuf;
use std::sync::Arc;

use futures::{StreamExt, TryStreamExt};
use reqwest::Client;
use tokio::fs::OpenOptions;
use tokio::io::AsyncReadExt;
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

//TODO: handle downloads gracefully

pub async fn v1(
    _permit: OwnedSemaphorePermit,
    reqwest_client: Client,
    endpoints: Vec<Endpoint>,
    entry: v1::DepotEntry,
    destination_path: PathBuf,
    result_report: UnboundedSender<()>,
) -> EmptyResult {
    let file = if let v1::DepotEntry::File(f) = entry {
        f
    } else {
        return Ok(());
    };
    let download_path = format!("{}.download", destination_path.to_str().unwrap());
    let endpoint = endpoints.first().unwrap();
    let url = assemble_url(endpoint, "main.bin");

    let offset = *file.offset();
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

    let stream = response
        .bytes_stream()
        .map_err(|e| futures::io::Error::new(futures::io::ErrorKind::Other, e))
        .into_async_read();

    let mut reader = BufReader::with_capacity(512 * 1024, stream.compat());

    tokio::io::copy(&mut reader, &mut file_handle)
        .await
        .map_err(io_error)?;

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
    result_report: UnboundedSender<()>,
) -> EmptyResult {
    let chunks = match &entry {
        v2::DepotEntry::File(file) => file.chunks.clone(),
        v2::DepotEntry::Diff(diff) => diff.chunks.clone(),
        _ => return Ok(()),
    };

    let download_path = format!("{}.download", destination_path.to_str().unwrap());
    // TODO: Keep track of the state
    let state_path = format!("{}.state", destination_path.to_str().unwrap());

    let mut file_handle = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&download_path)
        .await
        .expect("Failed to open the file");

    let endpoint = endpoints.first().unwrap();
    let mut handles: Vec<_> = Vec::new();

    for chunk in chunks.into_iter() {
        let reqwest_client = reqwest_client.clone();
        let chunk_semaphore = chunk_semaphore.clone();
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
                let reader = BufReader::with_capacity(1024 * 512, chunk_data.compat());
                let mut decompressed_data = ZlibDecoder::new(reader);
                let mut buffer = Vec::with_capacity((*chunk.size()).try_into().unwrap());
                decompressed_data
                    .read_to_end(&mut buffer)
                    .await
                    .map_err(zlib_error)?;
                Ok(buffer)
            })
            .await
            .unwrap()
        };
        handles.push(chunk_handle)
    }

    let mut stream = futures::stream::iter(handles).buffered(3);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file_handle.write_all(&chunk).await.map_err(io_error)?;
    }

    drop(file_handle);

    tokio::fs::rename(download_path, destination_path)
        .await
        .map_err(io_error)?;
    Ok(())
}
