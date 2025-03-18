use std::path::{Path, PathBuf};

use super::{
    diff::DiffReport,
    progress::{write_chunk_state, FileDownloadState},
};
use crate::{
    content_system::{
        downloader::progress::DownloadFileStatus,
        types::{traits::EntryUtils, v1, v2, DepotEntry},
    },
    errors::{io_error, EmptyResult},
};
use md5::{Digest, Md5};
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt},
};

const READ_CHUNK_SIZE: usize = 1024 * 1024;

async fn calculate_md5(
    file: &mut File,
    offset: i64,
    size: Option<i64>,
) -> tokio::io::Result<String> {
    file.seek(std::io::SeekFrom::Start(offset as u64)).await?;
    let mut read = 0;
    let mut md5 = Md5::new();
    while size.is_none_or(|s| (s as usize) > read) {
        let mut buffer = vec![0; READ_CHUNK_SIZE];
        let chk_size = file.read(&mut buffer).await?;
        if chk_size == 0 {
            break;
        }
        read += chk_size;
        md5.update(&buffer[..chk_size]);
    }
    Ok(format!("{:0x}", md5.finalize()))
}

async fn verify_v2_chunk_state(
    file_path: &Path,
    chunks: &[v2::Chunk],
    state: &[bool],
) -> tokio::io::Result<(bool, Vec<bool>)> {
    if !file_path.exists() {
        return Ok((false, vec![]));
    }
    let mut new_state: Vec<bool> = Vec::with_capacity(chunks.len());
    let mut file_h = OpenOptions::new().read(true).open(file_path).await?;
    let mut offset = 0;
    let mut correct = true;
    for (indx, chunk) in chunks.iter().enumerate() {
        let preexisting_state = state.is_empty() || state.get(indx).is_some_and(|x| *x);
        if !preexisting_state {
            new_state.push(false);
            offset += chunk.size();
            continue;
        }
        let calculated_md5 = calculate_md5(&mut file_h, offset, Some(*chunk.size())).await;
        let chunk_entry_error = calculated_md5.ok().is_none_or(|hash| &hash != chunk.md5());
        if chunk_entry_error {
            correct = false;
        }
        new_state.push(!chunk_entry_error);
        offset += chunk.size();
    }
    Ok((correct, new_state))
}

async fn write_new_chunk_state(path: &Path, new_state: Vec<bool>) -> EmptyResult {
    let state_path = format!("{}.state", path.to_str().unwrap());
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(state_path)
        .await
        .map_err(io_error)?;
    let state = FileDownloadState {
        chunks: new_state,
        ..Default::default()
    };
    write_chunk_state(&mut file, &state).await?;
    Ok(())
}

impl super::Downloader {
    // Verify files that are to be downloaded and update their state appropriately,
    // returns new diffreport in case patched files are to be re-downloaded
    pub async fn update_state(&self) -> DiffReport {
        let current_report = self.download_report.as_ref().unwrap();

        for list in &current_report.download {
            if let Some(sfc) = &list.sfc {
                let chunk = sfc.chunks().first().unwrap();
                let entry_root = self.get_file_root(false, &list.product_id, false);
                let file_path = entry_root.join(chunk.md5());
                if let DownloadFileStatus::Done = self.get_file_status(&file_path).await {
                    let mut offset = 0;
                    let file = OpenOptions::new().read(true).open(&file_path).await;
                    if let Ok(mut file) = file {
                        for chunk in sfc.chunks() {
                            let size = *chunk.size();
                            let chunk_md5 = calculate_md5(&mut file, offset, Some(size)).await;
                            if chunk_md5
                                .ok()
                                .is_none_or(|calculated_hash| &calculated_hash != chunk.md5())
                            {
                                drop(file);
                                if let Err(err) = tokio::fs::rename(
                                    &file_path,
                                    format!("{}.download", &file_path.display()),
                                )
                                .await
                                {
                                    log::error!("Failed to rename file {:?} {}", file_path, err);
                                }
                                break;
                            }
                            offset += size;
                        }
                    }
                }
            }

            for file in &list.files {
                let _ = self.verify_depot_entry_state(&list.product_id, file).await;
            }
        }
        current_report.clone()
    }

    /// Verify the state is valid by calculating hashes of chunks to see if
    /// these are the expected versions of files.
    /// if not, make sure to update said state
    async fn verify_depot_entry_state(
        &self,
        product_id: &str,
        depot_entry: &DepotEntry,
    ) -> tokio::io::Result<()> {
        let entry_root = self.get_file_root(depot_entry.is_support(), product_id, false);
        let file_path = entry_root.join(depot_entry.path());
        match self.get_file_status(&file_path).await {
            DownloadFileStatus::Done => {
                match depot_entry {
                    DepotEntry::V1(v1::DepotEntry::File(file)) => {
                        let mut file_h = OpenOptions::new().read(true).open(&file_path).await?;
                        let calculated_md5 = calculate_md5(&mut file_h, 0, None).await;
                        drop(file_h);
                        if calculated_md5.ok().is_none_or(|hash| &hash != file.hash()) {
                            self.set_file_status(
                                &file_path,
                                DownloadFileStatus::Done,
                                DownloadFileStatus::Allocated,
                            )
                            .await;
                        }
                    }
                    DepotEntry::V2(v2::DepotEntry::File(file)) => {
                        let (correct, new_state) =
                            verify_v2_chunk_state(&file_path, file.chunks(), &[]).await?;
                        if !correct {
                            log::info!("file {} corrupted", file_path.display());
                            self.set_file_status(
                                &file_path,
                                DownloadFileStatus::Done,
                                DownloadFileStatus::Allocated,
                            )
                            .await;
                            if file.chunks.len() > 1 {
                                let _ = write_new_chunk_state(&file_path, new_state).await;
                            }
                        }
                    }
                    DepotEntry::V2(v2::DepotEntry::Diff(file)) => {
                        // Done Diff means the final file is ready
                        let mut file_h = OpenOptions::new().read(true).open(&file_path).await?;
                        let calculated_md5 = calculate_md5(&mut file_h, 0, None).await;
                        drop(file_h);
                        if calculated_md5
                            .ok()
                            .is_none_or(|hash| &hash != file.md5_target())
                        {
                            let _ = tokio::fs::remove_file(&file_path).await;
                        }
                    }
                    _ => (),
                }
            }
            DownloadFileStatus::Partial(state) => {
                match depot_entry {
                    DepotEntry::V2(v2::DepotEntry::File(file)) => {
                        let file_path = format!("{}.download", file_path.to_str().unwrap());
                        let file_path = PathBuf::from(file_path);
                        let (correct, new_state) =
                            verify_v2_chunk_state(&file_path, file.chunks(), &state).await?;
                        if !correct {
                            let _ = write_new_chunk_state(&file_path, new_state).await;
                        }
                    }
                    // FIXME: Partial Diff detection is not implemented in get_file_status yet
                    DepotEntry::V2(v2::DepotEntry::Diff(file)) => {
                        // Partial Diff - patch itself is download partially
                        let diff_file = format!("{}.diff.download", file_path.to_str().unwrap());
                        let diff_file = PathBuf::from(diff_file);
                        let (correct, new_state) =
                            verify_v2_chunk_state(&diff_file, file.chunks(), &state).await?;
                        if !correct {
                            let diff_file = format!("{}.diff", file_path.to_str().unwrap());
                            let diff_file = PathBuf::from(diff_file);
                            let _ = write_new_chunk_state(&diff_file, new_state).await;
                        }
                    }
                    _ => (),
                }
            }
            DownloadFileStatus::PatchDownloaded => {
                if let DepotEntry::V2(v2::DepotEntry::Diff(file)) = depot_entry {
                    // Patch is ready - still needs to be applied to file
                    let diff_file = format!("{}.diff", file_path.to_str().unwrap());
                    let diff_file = PathBuf::from(diff_file);
                    let (correct, new_state) =
                        verify_v2_chunk_state(&diff_file, file.chunks(), &[]).await?;
                    if !correct {
                        self.set_file_status(
                            &diff_file,
                            DownloadFileStatus::PatchDownloaded,
                            DownloadFileStatus::Allocated,
                        )
                        .await;
                        let _ = write_new_chunk_state(&diff_file, new_state).await;
                    }
                }
            }
            // Other types already mean the file has to be downloaded
            _ => (),
        }
        Ok(())
    }
}
