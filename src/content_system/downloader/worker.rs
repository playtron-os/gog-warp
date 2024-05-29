use std::path::PathBuf;
use std::sync::Arc;

use reqwest::Client;
use tokio::fs::OpenOptions;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;

use async_compression::tokio::bufread::ZlibDecoder;

use crate::content_system::types::v2;
use crate::content_system::types::Endpoint;
use crate::utils::{assemble_url, hash_to_galaxy_path};

//TODO: handle downloads gracefully

pub async fn v2(
    _permit: OwnedSemaphorePermit,
    reqwest_client: Client,
    chunk_semaphore: Arc<Semaphore>,
    endpoints: Vec<Endpoint>,
    entry: v2::DepotEntry,
    destination_path: PathBuf,
    result_report: UnboundedSender<()>,
) {
    let chunks = match entry {
        v2::DepotEntry::File(file) => file.chunks,
        v2::DepotEntry::Diff(diff) => diff.chunks,
        _ => return,
    };

    let download_path = format!("{}.download", destination_path.to_str().unwrap());

    let mut file_handle = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(&download_path)
        .await
        .expect("Failed to open the file");

    let endpoint = endpoints.first().unwrap();

    for chunk in chunks {
        let _permit = chunk_semaphore.acquire().await.unwrap();
        let galaxy_path = hash_to_galaxy_path(chunk.compressed_md5());
        let url = assemble_url(endpoint, &galaxy_path);

        let response = reqwest_client
            .get(url)
            .send()
            .await
            .expect("Failed to make a request");

        let chunk = response.bytes().await.expect("Failed to get chunk");

        let mut decoder = ZlibDecoder::new(&chunk[..]);
        tokio::io::copy(&mut decoder, &mut file_handle)
            .await
            .expect("Failed to write to file");
    }

    drop(file_handle);

    tokio::fs::rename(download_path, destination_path)
        .await
        .expect("Failed to rename");

    let _ = result_report.send(());
}
