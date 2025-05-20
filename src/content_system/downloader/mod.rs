use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::content_system::secure_link;
use crate::errors::{cancelled_error, task_error};
use crate::{
    errors::{dbuilder_error, io_error, not_ready_error},
    Core, Error,
};

use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc::{error::TryRecvError, Receiver, Sender};
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

use self::progress::{load_chunk_state, DownloadState, WorkerUpdate};

use super::dependencies::DependenciesManifest;
use super::types::{traits::EntryUtils, Endpoint, Manifest};
use super::types::{v1, v2, DepotEntry};

mod diff;
mod patching;
pub mod progress;
mod utils;
mod verify;
mod worker;

#[derive(Default)]
pub struct Builder {
    core: Option<Core>,
    manifest: Option<Manifest>,
    build_id: Option<String>,
    upgrade_from: Option<Manifest>,
    prev_build_id: Option<String>,
    install_root: Option<PathBuf>,
    install_path: Option<PathBuf>,
    support_root: Option<PathBuf>,
    dependency_manifest: Option<DependenciesManifest>,
    global_dependencies: Vec<String>,
    old_global_dependencies: Vec<String>,
    language: Option<String>,
    old_language: Option<String>,
    dlcs: Vec<String>,
    old_dlcs: Vec<String>,
    verify: bool,
}

impl Builder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn build(self) -> Result<Downloader, Error> {
        if self.manifest.is_none() && self.dependency_manifest.is_none() {
            return Err(dbuilder_error());
        }
        let core = self.core.ok_or_else(dbuilder_error)?;
        let language = self.language.unwrap_or("en-US".to_owned());
        let mut old_manifest = self.upgrade_from;
        let prev_build_id = self.prev_build_id;

        let manifest = self.manifest;
        let install_directory = match &manifest {
            Some(m) => m.install_directory(),
            None => "".to_string(),
        };
        let install_path = match self.install_root {
            Some(ir) => ir.join(install_directory),
            None => self.install_path.ok_or_else(dbuilder_error)?,
        };

        let support_path = match self.support_root {
            Some(sp) => sp,
            None => install_path.join("gog-support"),
        };

        let support_path = match &manifest {
            Some(Manifest::V2(m)) => support_path.join(m.base_product_id()),
            Some(Manifest::V1(m)) => support_path.join(m.product().root_game_id()),
            _ => support_path,
        };

        let old_language = match self.old_language {
            Some(ol) => ol,
            None => language.clone(),
        };

        let build_id = self.build_id;
        let dlcs = self.dlcs;
        let old_dlcs = self.old_dlcs;
        let verify = self.verify;
        let dependency_manifest = self.dependency_manifest;
        let global_dependencies = self.global_dependencies;
        let old_global_dependencies = self.old_global_dependencies;

        if (!old_dlcs.is_empty() || language != old_language) && old_manifest.is_none() {
            old_manifest.clone_from(&manifest);
        }

        let (progress_channel_sender, progress_channel_receiver) = tokio::sync::mpsc::channel(5);

        Ok(Downloader {
            core,
            manifest,
            old_manifest,
            tmp_path: install_path.join("!Temp"),
            install_path,
            support_path,
            language,
            old_language,
            dlcs,
            old_dlcs,
            verify,
            build_id,
            prev_build_id,
            progress_channel_sender,
            progress_channel_receiver: Some(progress_channel_receiver),
            cancellation_token: CancellationToken::new(),
            global_dependencies,
            old_global_dependencies,
            dependency_manifest,
            download_report: None,
            max_speed: Mutex::new(-1),
        })
    }

    /// Required: a clone of core for [`Downloader`] to access the tokens
    pub fn core(mut self, core: Core) -> Self {
        self.core = Some(core);
        self
    }

    /// Required: manifest of the wanted build
    pub fn manifest(mut self, manifest: Manifest, build_id: &str) -> Self {
        self.manifest = Some(manifest);
        self.build_id = Some(build_id.to_string());
        self
    }

    /// Required: language to download
    pub fn language(mut self, language: String) -> Self {
        self.language = Some(language);
        self
    }

    /// Optional: language that the game was downloaded with previously  
    /// if not specified the language from [`Self::language`] will be used
    pub fn old_language(mut self, language: String) -> Self {
        self.old_language = Some(language);
        self
    }

    /// Optional: provide an old manifest to execute a update/downgrade
    pub fn upgrade_from(mut self, manifest: Manifest, build_id: &str) -> Self {
        self.upgrade_from = Some(manifest);
        self.prev_build_id = Some(build_id.to_string());
        self
    }

    /// Optional: List of DLC ids that were installed previously
    pub fn old_dlcs(mut self, dlcs: Vec<String>) -> Self {
        self.old_dlcs = dlcs;
        self
    }

    /// Root directory for the insallation, the install directory will be appended  
    /// If you want to provide custom directory name, use [`Self::install_path`]
    /// One install_* needs to be provided
    pub fn install_root(mut self, install_root: PathBuf) -> Self {
        self.install_root = Some(install_root);
        self
    }

    /// Root directory for the insallation, the install directory will not be appended
    pub fn install_path(mut self, install_path: PathBuf) -> Self {
        self.install_path = Some(install_path);
        self
    }

    /// Optional: List of DLC ids that are to be installed
    pub fn dlcs(mut self, dlcs: Vec<String>) -> Self {
        self.dlcs = dlcs;
        self
    }

    /// A root directory where support files will be stored  
    /// The structure will look as follows
    /// ```text
    /// support_root/
    /// └── baseGameId
    ///     ├── anotherProductId
    ///     └── productId
    /// ```
    /// Otherwise a `gog-support` directory will be created in game directory
    pub fn support_root(mut self, support_root: PathBuf) -> Self {
        self.support_root = Some(support_root);
        self
    }

    /// Enable dependencies support by providing the manifest
    pub fn game_dependencies(mut self, dependencies_manifest: DependenciesManifest) -> Self {
        self.dependency_manifest = Some(dependencies_manifest);
        self
    }

    /// Allows to download only global dependencies
    /// When this is provided you can safely not provide any other parameter
    pub fn global_dependencies(
        mut self,
        dependencies_manifest: DependenciesManifest,
        dependencies: Vec<String>,
    ) -> Self {
        self.dependency_manifest = Some(dependencies_manifest);
        self.global_dependencies = dependencies;
        self
    }

    /// When provided allows to delete unused dependencies.
    /// To be used together with [`Self::global_dependencies`]
    pub fn old_global_dependencies(mut self, dependencies: Vec<String>) -> Self {
        self.old_global_dependencies = dependencies;
        self
    }

    /// Makes downloader verify the files from [`Self::manifest`]
    /// and download only invalid/missing ones
    pub fn verify(mut self) -> Self {
        self.verify = true;
        self
    }
}

/// The main component responsible for downloading game files
pub struct Downloader {
    /// A warp Core
    core: Core,
    /// Manifest to upgrade to
    manifest: Option<Manifest>,
    /// Build id of the new manifest
    build_id: Option<String>,
    /// Language that we target
    language: String,
    /// Language previously installed
    old_language: String,
    /// Manifest of the previously installed version
    old_manifest: Option<Manifest>,
    /// Previously installed build_id
    prev_build_id: Option<String>,
    /// DLCs targetted in the new build
    dlcs: Vec<String>,
    /// DLCs previously installed
    old_dlcs: Vec<String>,
    /// Path of game files installation
    install_path: PathBuf,
    /// Path of support files
    support_path: PathBuf,
    /// Tmp path for update files
    tmp_path: PathBuf,
    /// Whether to verify the files based on the manifest
    verify: bool,
    /// Manifest to use for dependencies
    dependency_manifest: Option<DependenciesManifest>,
    /// Global dependencies to upgrade to
    global_dependencies: Vec<String>,
    /// Global dependencies to upgrade from
    old_global_dependencies: Vec<String>,

    progress_channel_sender: Sender<DownloadState>,
    progress_channel_receiver: Option<Receiver<DownloadState>>,

    cancellation_token: CancellationToken,
    download_report: Option<diff::DiffReport>,
    max_speed: Mutex<i32>,
}

impl Downloader {
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Returns a cancellation token that allows to stop the download
    pub fn get_cancellation(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    /// Returns a receiver for progress events
    /// leaving None in it's place, meaning this
    /// function will return Some only once
    pub fn take_progress_receiver(&mut self) -> Option<Receiver<DownloadState>> {
        self.progress_channel_receiver.take()
    }

    pub async fn set_max_speed(&self, speed: i32) {
        *self.max_speed.lock().await = speed;
    }

    /// Fetches file lists and patches manifest
    pub async fn prepare(&mut self) -> Result<(), Error> {
        let _ = self
            .progress_channel_sender
            .send(DownloadState::Preparing)
            .await;
        // Get depots for main manifest
        let mut depots = match &self.manifest {
            Some(m) => {
                log::trace!("Getting depots for main manifest");
                m.get_depots(self.core.reqwest_client(), &self.language, &self.dlcs)
                    .await?
            }
            None => Vec::new(),
        };

        let mut old_depots = match &self.old_manifest {
            Some(om) => {
                log::trace!("Getting depots for old manifest");
                om.get_depots(
                    self.core.reqwest_client(),
                    &self.old_language,
                    &self.old_dlcs,
                )
                .await?
            }
            None => Vec::new(),
        };

        if let Some(dm) = &self.dependency_manifest {
            let reqwest_client = self.core.reqwest_client();
            if let Some(manifest) = &self.manifest {
                log::trace!("Collecting dependencies depots");
                let mut dependencies = manifest.dependencies();
                if manifest.needs_isi() {
                    dependencies.push("ISI".to_string());
                }
                let new_deps = dm
                    .get_depots(reqwest_client.clone(), &dependencies, false)
                    .await?;
                depots.extend(new_deps);
            }

            if let Some(om) = &self.old_manifest {
                let mut dependencies = om.dependencies();
                if om.needs_isi() {
                    dependencies.push("ISI".to_string());
                }
                let old_deps = dm
                    .get_depots(reqwest_client.clone(), &dependencies, false)
                    .await?;
                old_depots.extend(old_deps);
            }

            if !self.global_dependencies.is_empty() {
                log::trace!("Collecting global dependencies depots");
                let global_deps = dm
                    .get_depots(reqwest_client.clone(), &self.global_dependencies, true)
                    .await?;
                depots.extend(global_deps);
            }

            if !self.old_global_dependencies.is_empty() {
                let old_global_deps = dm
                    .get_depots(reqwest_client.clone(), &self.old_global_dependencies, true)
                    .await?;
                old_depots.extend(old_global_deps);
            }
        }

        let re_used_dlcs: Vec<String> = self
            .dlcs
            .iter()
            .filter(|d| self.old_dlcs.contains(d))
            .cloned()
            .collect();

        let patches = super::patches::get_patches(
            self.core.reqwest_client(),
            &self.manifest,
            &self.build_id,
            &self.old_manifest,
            self.prev_build_id.clone(),
            re_used_dlcs,
            &self.language,
            &self.old_language,
        )
        .await?;

        let results = diff::diff(depots, old_depots, patches.unwrap_or_default());
        self.download_report = Some(results);
        Ok(())
    }

    /// Return space required for operation to complete, takes in account pre-allocated files
    /// You should check if you have enough space before calling `download`
    pub async fn get_required_space(&mut self) -> Result<i64, Error> {
        let report = self.download_report.take().unwrap();
        let mut size_total: i64 = 0;
        // Since we want to allow the game to be playable after pausing the update
        // we are not subtracting deleted files sizes
        for list in &report.download {
            if let Some(sfc) = &list.sfc {
                let file_root = self.get_file_root(false, &list.product_id, false);
                let chunk = sfc.chunks().first().unwrap();
                let file_path = file_root.join(chunk.md5());
                let status = self.get_file_status(&file_path).await;
                if matches!(status, progress::DownloadFileStatus::NotInitialized) {
                    size_total += sfc.chunks().first().unwrap().size();
                }
            }
            for entry in &list.files {
                if entry.is_dir() {
                    continue;
                }
                let file_root = self.get_file_root(entry.is_support(), &list.product_id, false);
                let file_path = file_root.join(entry.path());
                let status = self.get_file_status(&file_path).await;

                if matches!(status, progress::DownloadFileStatus::NotInitialized) {
                    size_total += entry.size();
                }
            }
        }

        for patch in &report.patches {
            let file_root = self.get_file_root(false, &patch.product_id, false);
            let file_path = file_root.join(patch.diff.path());
            let status = self.get_file_status(&file_path).await;
            if matches!(status, progress::DownloadFileStatus::NotInitialized) {
                size_total += patch.diff.size() + patch.destination_file.size();
            }
        }

        self.download_report = Some(report);
        Ok(size_total)
    }

    fn get_file_root(
        &self,
        is_support: bool,
        product_id: &str,
        final_destination: bool,
    ) -> PathBuf {
        if self.old_manifest.is_some() && !final_destination {
            self.tmp_path.clone()
        } else if is_support {
            if matches!(self.manifest, Some(Manifest::V2(_))) {
                self.support_path.join(product_id)
            } else {
                self.support_path.clone()
            }
        } else {
            self.install_path.clone()
        }
    }

    async fn get_file_status(&self, path: &Path) -> progress::DownloadFileStatus {
        if path.exists() {
            return progress::DownloadFileStatus::Done;
        }
        let state_file = format!("{}.state", path.to_str().unwrap());
        let state_file_path = PathBuf::from(&state_file);

        if state_file_path.exists() {
            let file_state = load_chunk_state(&state_file).await;
            if let Ok(file_state) = file_state {
                return progress::DownloadFileStatus::Partial(file_state.chunks);
            }
        }

        let allocation_file = format!("{}.download", path.to_str().unwrap());
        let allocation_file = PathBuf::from(allocation_file);

        if allocation_file.exists() {
            progress::DownloadFileStatus::Allocated
        } else {
            let diff_file = format!("{}.diff", path.to_str().unwrap());
            let download_diff_file = format!("{}.diff.download", path.to_str().unwrap());
            let diff_file = PathBuf::from(diff_file);
            let download_diff_file = PathBuf::from(download_diff_file);
            if diff_file.exists() {
                return progress::DownloadFileStatus::PatchDownloaded;
            } else if download_diff_file.exists() {
                return progress::DownloadFileStatus::Allocated;
            }
            progress::DownloadFileStatus::NotInitialized
        }
    }

    async fn set_file_status(
        &self,
        path: &Path,
        from_status: progress::DownloadFileStatus,
        to_status: progress::DownloadFileStatus,
    ) {
        let from_path: Option<PathBuf> = from_status.path_with_state(path);
        let to_path: Option<PathBuf> = to_status.path_with_state(path);

        if let (Some(from_p), Some(to_p)) = (from_path, to_path) {
            log::info!("Renaming {:?} to {:?}", from_p, to_p);
            if let Err(err) = tokio::fs::rename(&from_p, &to_p).await {
                log::error!(
                    "Failed to rename file {} to {} {:?}",
                    from_p.display(),
                    to_p.display(),
                    err
                );
            }
        }
    }

    /// Execute the download.  
    /// Make sure to run this after [`Self::prepare`]
    pub async fn download(&self) -> Result<(), Error> {
        let mut should_verify = self.verify;
        if self.download_report.is_none() {
            return Err(not_ready_error(
                "download not ready, did you forget Downloader::prepare()?",
            ));
        }

        let manifest_version = if let Some(Manifest::V1(_)) = &self.manifest {
            1
        } else {
            2
        };

        let timestamp = self
            .manifest
            .as_ref()
            .and_then(|m| m.repository_timestamp());

        let install_root = self.get_file_root(false, "0", false);
        if !install_root.exists() {
            fs::create_dir_all(&install_root).await.map_err(io_error)?;
        }

        log::info!("Checking for interrupted downloads");
        if let Some(build_id) = &self.build_id {
            let build_state_path = install_root.join(".gog-warp-build");
            // Check if file indicating the build we previously installed exists
            // For updates this will only affect !Temp subdirectory
            if build_state_path.exists() {
                // If it does, compare builds
                // and reset the download if they dont match
                let mut file = fs::OpenOptions::new()
                    .read(true)
                    .open(&build_state_path)
                    .await
                    .map_err(io_error)?;
                let mut buffer = String::new();
                file.read_to_string(&mut buffer).await.map_err(io_error)?;
                let current_build = buffer.trim();
                if current_build != build_id {
                    log::warn!("Found different download in progress, verifying the state");
                    should_verify = true;
                }
            }

            let mut file = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(build_state_path)
                .await
                .map_err(io_error)?;

            file.write_all(build_id.as_bytes())
                .await
                .map_err(io_error)?;
            file.flush().await.map_err(io_error)?;
        }

        let report = if should_verify {
            log::debug!("Verifying download state");
            self.update_state().await
        } else {
            self.download_report.clone().unwrap()
        };
        let mut new_symlinks: Vec<(String, String)> = Vec::new();
        let mut ready_files: HashSet<String> = HashSet::new();
        let mut ready_patches: HashSet<String> = HashSet::new();
        let secure_links: Arc<Mutex<HashMap<String, Vec<Endpoint>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let mut download_progress: progress::DownloadProgress = Default::default();

        let mut allocated_files: u32 = 0;

        log::info!("Allocating disk space");
        // Allocate disk space, generate secure links and restore progress state
        for file_list in &report.download {
            if let Some(sfc) = &file_list.sfc {
                if sfc.chunks().len() != 1 {
                    log::warn!("sfc chunk count != 1");
                }
                let chunk = sfc.chunks().first().unwrap();
                let install_root = self.get_file_root(false, &file_list.product_id, false);
                let file_path = install_root.join(chunk.md5());
                let size = *chunk.size();
                download_progress.total_download += *chunk.compressed_size() as u64;
                download_progress.total_size += size as u64;
                match self.get_file_status(&file_path).await {
                    progress::DownloadFileStatus::NotInitialized
                    | progress::DownloadFileStatus::Allocated => {
                        let download_path = format!("{}.download", file_path.to_str().unwrap());
                        let file_handle = fs::OpenOptions::new()
                            .create(true)
                            .truncate(false)
                            .write(true)
                            .open(download_path)
                            .await
                            .map_err(io_error)?;
                        utils::allocate(file_handle, size).await?;
                    }
                    _ => {
                        download_progress.downloaded += *chunk.compressed_size() as u64;
                        download_progress.written += *chunk.size() as u64;
                        ready_files.insert(chunk.md5().clone());
                    }
                }
            }
            for entry in &file_list.files {
                // TODO: Normalize the path to account for existing files on
                // case sensitive file systems
                // e.g Binaries/Game.exe -> binaries/Game.exe
                // In the future detect ext4 case-folding and use that as well
                let entry_path = entry.path();
                let entry_root =
                    self.get_file_root(entry.is_support(), &file_list.product_id, false);
                let file_path = entry_root.join(&entry_path);
                if entry.is_dir() {
                    fs::create_dir_all(file_path).await.map_err(io_error)?;
                    allocated_files += 1;
                    continue;
                }

                let file_parent = file_path.parent().unwrap();
                if !file_parent.exists() {
                    fs::create_dir_all(&file_parent).await.map_err(io_error)?;
                }

                let file_size = entry.size();
                let is_sfc_contained = if let DepotEntry::V2(v2::DepotEntry::File(f)) = &entry {
                    f.sfc_ref.is_some()
                } else {
                    false
                };

                if !is_sfc_contained {
                    download_progress.total_download += entry.compressed_size() as u64;
                }
                download_progress.total_size += file_size as u64;

                if file_size == 0 {
                    match entry {
                        DepotEntry::V1(v1::DepotEntry::File(_))
                        | DepotEntry::V2(v2::DepotEntry::File(_)) => {
                            fs::OpenOptions::new()
                                .create(true)
                                .truncate(false)
                                .write(true)
                                .open(&file_path)
                                .await
                                .map_err(io_error)?;
                            ready_files.insert(entry_path.clone());
                        }

                        DepotEntry::V2(v2::DepotEntry::Link(link)) => {
                            let link_path = link.path();
                            let target_path = link.target();
                            let link_root = self.get_file_root(false, &file_list.product_id, true);
                            let link_path = link_root.join(link_path);
                            let link_path = link_path.to_str().unwrap();
                            new_symlinks.push((link_path.to_owned(), target_path.to_owned()));
                        }

                        _ => (),
                    }
                    allocated_files += 1;
                    continue;
                }

                match self.get_file_status(&file_path).await {
                    progress::DownloadFileStatus::NotInitialized
                    | progress::DownloadFileStatus::Allocated => {
                        let allocation_file = format!("{}.download", file_path.to_str().unwrap());
                        let file_handle = fs::OpenOptions::new()
                            .create(true)
                            .truncate(false)
                            .write(true)
                            .open(allocation_file)
                            .await
                            .map_err(io_error)?;
                        utils::allocate(file_handle, file_size).await?;
                    }
                    progress::DownloadFileStatus::Partial(chunks_state) => {
                        if let DepotEntry::V2(v2::DepotEntry::File(f)) = entry {
                            for (index, chunk) in f.chunks.iter().enumerate() {
                                if *chunks_state.get(index).unwrap_or(&false) {
                                    download_progress.downloaded += *chunk.compressed_size() as u64;
                                    download_progress.written += *chunk.size() as u64;
                                }
                            }
                        }
                    }
                    _ => {
                        if !is_sfc_contained {
                            download_progress.downloaded += entry.compressed_size() as u64;
                        }
                        download_progress.written += file_size as u64;
                        ready_files.insert(entry_path.clone());
                    }
                }
                allocated_files += 1;
            }
            let allocation_progress = allocated_files as f32 / report.number_of_files as f32;
            let _ = self
                .progress_channel_sender
                .try_send(DownloadState::Allocating(allocation_progress));

            let mut secure_links = secure_links.lock().await;
            let product_id = file_list.product_id();

            let path = if manifest_version == 2 {
                "/".to_owned()
            } else {
                format!("/windows/{}", timestamp.unwrap())
            };

            if let std::collections::hash_map::Entry::Vacant(e) =
                secure_links.entry(product_id.clone())
            {
                log::debug!(
                    "Getting the secure link for {} is_dep: {}",
                    product_id,
                    file_list.is_dependency
                );
                let endpoints = if file_list.is_dependency {
                    secure_link::get_dependencies_link(self.core.reqwest_client()).await?
                } else {
                    let token = self.core.obtain_galaxy_token().await?;
                    secure_link::get_secure_link(
                        self.core.reqwest_client(),
                        manifest_version,
                        &product_id,
                        &token,
                        &path,
                        "",
                    )
                    .await?
                };
                e.insert(endpoints);
            }
        }

        log::info!("Processing patches");
        for patch in &report.patches {
            let entry = &patch.diff;
            let entry_path = entry.path();
            let entry_root = self.get_file_root(entry.is_support(), &patch.product_id, false);
            let file_path = entry_root.join(&entry_path);
            let file_parent = file_path.parent().unwrap();
            if !file_parent.exists() {
                fs::create_dir_all(&file_parent).await.map_err(io_error)?;
            }
            let product_id = format!("{}patch", patch.product_id);

            let mut secure_links = secure_links.lock().await;
            if !secure_links.contains_key(&product_id) {
                let token = self.core.obtain_galaxy_token().await?;
                log::info!("Getting patch secure_link for {}", patch.product_id);
                let endpoints = secure_link::get_secure_link(
                    self.core.reqwest_client(),
                    manifest_version,
                    &patch.product_id,
                    &token,
                    "/",
                    "/patches/store",
                )
                .await?;
                secure_links.insert(product_id.clone(), endpoints);
            }

            download_progress.total_download += entry.compressed_size() as u64;
            download_progress.total_size += entry.size() as u64;
            download_progress.total_size += patch.destination_file.size() as u64;

            match self.get_file_status(&file_path).await {
                progress::DownloadFileStatus::NotInitialized
                | progress::DownloadFileStatus::Allocated => {
                    let file_path = file_path.to_str().unwrap();
                    let allocation_file = format!("{}.diff.download", file_path);
                    let file_handle = fs::OpenOptions::new()
                        .create(true)
                        .truncate(false)
                        .write(true)
                        .open(allocation_file)
                        .await
                        .map_err(io_error)?;
                    utils::allocate(file_handle, patch.diff.size()).await?;
                    let file_handle = fs::OpenOptions::new()
                        .create(true)
                        .truncate(false)
                        .write(true)
                        .open(format!("{}.patched", file_path))
                        .await
                        .map_err(io_error)?;
                    utils::allocate(file_handle, patch.destination_file.size()).await?;
                }
                progress::DownloadFileStatus::Done => {
                    download_progress.downloaded += entry.compressed_size() as u64;
                    download_progress.written += patch.destination_file.size() as u64;
                    ready_files.insert(entry_path.clone());
                }
                progress::DownloadFileStatus::PatchDownloaded => {
                    download_progress.downloaded += entry.compressed_size() as u64;
                    ready_patches.insert(entry_path.clone());
                }
                progress::DownloadFileStatus::Partial(chunks_state) => {
                    if let v2::DepotEntry::File(f) = entry {
                        for (index, chunk) in f.chunks.iter().enumerate() {
                            if *chunks_state.get(index).unwrap_or(&false) {
                                download_progress.downloaded += *chunk.compressed_size() as u64;
                                download_progress.written += *chunk.size() as u64;
                            }
                        }
                    }
                }
            }
        }

        let download_progress = Arc::new(Mutex::new(download_progress));

        let file_semaphore = Arc::new(Semaphore::new(3));
        let chunk_semaphore = Arc::new(Semaphore::new(6));

        // TODO: Handle download speed reports
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<WorkerUpdate>();
        let mut handles = tokio::task::JoinSet::new();

        let report_download_progress = download_progress.clone();
        let progress_channel_sender = self.progress_channel_sender.clone();
        let progress_report = tokio::spawn(async move {
            let mut timestamp = tokio::time::Instant::now();
            let one_sec = tokio::time::Duration::from_secs(1);

            loop {
                match rx.try_recv() {
                    Ok(message) => {
                        let mut progress = report_download_progress.lock().await;
                        match message {
                            WorkerUpdate::Download(size) => progress.downloaded += size as u64,
                            WorkerUpdate::Write(size) => progress.written += size as u64,
                        }
                        if timestamp.elapsed() > tokio::time::Duration::from_millis(500) {
                            timestamp = tokio::time::Instant::now();
                            let _ = progress_channel_sender
                                .try_send(DownloadState::Downloading((*progress).clone()));
                        }
                    }
                    Err(TryRecvError::Disconnected) => break,
                    Err(TryRecvError::Empty) => tokio::time::sleep(Duration::from_millis(50)).await,
                }
            }
            let progress = report_download_progress.lock().await;
            let _ = progress_channel_sender
                .send_timeout(DownloadState::Downloading((*progress).clone()), one_sec)
                .await;
            let _ = progress_channel_sender
                .send_timeout(DownloadState::Finished, one_sec)
                .await;
        });

        // Spawn download tasks
        for list in &report.download {
            if let Some(sfc) = &list.sfc {
                let chunk = sfc.chunks().first().unwrap();
                if !ready_files.contains(chunk.md5()) {
                    let entry_root = self.get_file_root(false, &list.product_id, false);
                    let file_path = entry_root.join(chunk.md5());

                    let product_id = list.product_id();
                    let secure_links = secure_links.clone();
                    let chunk_semaphore = chunk_semaphore.clone();
                    let chunks = sfc.chunks().clone();
                    let file_semaphore = file_semaphore.clone();
                    let path = chunk.md5().clone();
                    let reqwest_client = self.core.reqwest_client().clone();
                    let tx = tx.clone();
                    handles.spawn(async move {
                        let file_permit = file_semaphore.acquire_owned().await.unwrap();
                        let secure_links = secure_links.lock().await;
                        let endpoints = secure_links.get(&product_id).unwrap().clone();
                        drop(secure_links);

                        worker::v2(
                            file_permit,
                            reqwest_client,
                            chunk_semaphore,
                            endpoints,
                            v2::DepotEntry::File(v2::DepotFile {
                                chunks,
                                path,
                                sfc_ref: None,
                                md5: None,
                                sha256: None,
                                flags: Vec::new(),
                            }),
                            file_path,
                            tx,
                        )
                        .await
                    });
                }
            }
            for file in &list.files {
                let file_path = file.path();
                if ready_files.contains(&file_path) || file.is_dir() {
                    continue;
                }
                let root = self.get_file_root(file.is_support(), &list.product_id, false);
                let file_path = root.join(file_path);
                match file {
                    DepotEntry::V2(v2_entry) => {
                        if let v2::DepotEntry::File(file) = &v2_entry {
                            if file.sfc_ref.is_some() {
                                log::debug!(
                                    "skipping {}, it will be extracted from sfc",
                                    file.path
                                );
                                continue;
                            }
                        }
                        let file_semaphore = file_semaphore.clone();
                        let secure_links = secure_links.clone();

                        let chunk_semaphore = chunk_semaphore.clone();
                        let reqwest_client = self.core.reqwest_client().clone();
                        let v2_entry = v2_entry.clone();
                        let tx = tx.clone();
                        let product_id = list.product_id();
                        handles.spawn(async move {
                            let file_permit = file_semaphore.clone().acquire_owned().await.unwrap();
                            let secure_links = secure_links.lock().await;
                            let endpoints = secure_links.get(&product_id).unwrap().clone();
                            drop(secure_links);

                            worker::v2(
                                file_permit,
                                reqwest_client,
                                chunk_semaphore,
                                endpoints,
                                v2_entry,
                                file_path,
                                tx,
                            )
                            .await
                        });
                    }
                    DepotEntry::V1(v1_entry) => {
                        let file_semaphore = file_semaphore.clone();
                        let secure_links = secure_links.clone();

                        let product_id = list.product_id();
                        let reqwest_client = self.core.reqwest_client().clone();
                        let v1_entry = v1_entry.clone();
                        let tx = tx.clone();
                        handles.spawn(async move {
                            let file_permit = file_semaphore.clone().acquire_owned().await.unwrap();
                            let secure_links = secure_links.lock().await;
                            let endpoints = secure_links.get(&product_id).unwrap().clone();
                            drop(secure_links);
                            worker::v1(
                                file_permit,
                                reqwest_client,
                                endpoints,
                                v1_entry,
                                file_path,
                                tx,
                            )
                            .await
                        });
                    }
                }
            }
        }

        for patch in &report.patches {
            let file = &patch.diff;
            let entry_path = file.path();
            if ready_patches.contains(&entry_path) {
                continue;
            }
            let entry_path = format!("{}.diff", entry_path);
            let entry_root = self.get_file_root(file.is_support(), &patch.product_id, false);
            let file_path = entry_root.join(&entry_path);

            let file_semaphore = file_semaphore.clone();
            let secure_links = secure_links.clone();

            let chunk_semaphore = chunk_semaphore.clone();
            let reqwest_client = self.core.reqwest_client().clone();
            let v2_entry = file.clone();
            let tx = tx.clone();
            let product_id = format!("{}patch", patch.product_id);
            handles.spawn(async move {
                let file_permit = file_semaphore.clone().acquire_owned().await.unwrap();
                let secure_links = secure_links.lock().await;
                let endpoints = secure_links.get(&product_id).unwrap().clone();
                drop(secure_links);

                worker::v2(
                    file_permit,
                    reqwest_client,
                    chunk_semaphore,
                    endpoints,
                    v2_entry,
                    file_path,
                    tx,
                )
                .await
            });
        }

        loop {
            tokio::select! {
                result = handles.join_next() => {
                    match result {
                        Some(result) => {
                            let join_res = result.map_err(task_error);
                            if join_res.is_err() {
                                handles.shutdown().await;
                            }
                            let task_res = join_res?;
                            if task_res.is_err() {
                                handles.shutdown().await;
                            }
                            task_res?;
                        },
                        None => break
                    }
                }
                _ = self.cancellation_token.cancelled() => {
                    handles.shutdown().await;
                    return Err(cancelled_error());
                }
            }
        }

        // Finalize the download

        // Extract the small file container
        for list in &report.download {
            if let Some(sfc) = &list.sfc {
                let chunk = sfc.chunks().first().unwrap();
                let entry_root = self.get_file_root(false, &list.product_id, false);
                let sfc_path = entry_root.join(chunk.md5());

                let mut sfc_handle = fs::OpenOptions::new()
                    .read(true)
                    .open(&sfc_path)
                    .await
                    .map_err(io_error)?;

                for file in &list.files {
                    if let DepotEntry::V2(v2::DepotEntry::File(v2_file)) = &file {
                        if let Some(sfc_ref) = &v2_file.sfc_ref {
                            let file_path = file.path();
                            let entry_root =
                                self.get_file_root(file.is_support(), &list.product_id, false);
                            let file_path = entry_root.join(file_path);
                            if matches!(
                                self.get_file_status(&file_path).await,
                                progress::DownloadFileStatus::Done
                            ) {
                                continue;
                            }
                            let download_path = format!("{}.download", file_path.to_str().unwrap());
                            sfc_handle
                                .seek(std::io::SeekFrom::Start(*sfc_ref.offset()))
                                .await
                                .map_err(io_error)?;

                            let mut file_handle = fs::OpenOptions::new()
                                .write(true)
                                .truncate(false)
                                .open(&download_path)
                                .await
                                .map_err(io_error)?;

                            let mut buffer =
                                Vec::with_capacity((*sfc_ref.size()).try_into().unwrap());
                            sfc_handle.read_buf(&mut buffer).await.map_err(io_error)?;
                            file_handle.write_all(&buffer).await.map_err(io_error)?;
                            let _ = tx.send(WorkerUpdate::Write(buffer.len()));
                            file_handle.flush().await.map_err(io_error)?;

                            drop(file_handle);
                            fs::rename(download_path, file_path)
                                .await
                                .map_err(io_error)?;
                        }
                    }
                }
                drop(sfc_handle);
                fs::remove_file(sfc_path).await.map_err(io_error)?;
            }
        }

        // Patch files
        for patch in &report.patches {
            if let v2::DepotEntry::Diff(_diff) = &patch.diff {
                let file_path = patch.diff.path();
                let tmp_root = self.get_file_root(false, &patch.product_id, false);
                let dst_root = self.get_file_root(false, &patch.product_id, true);

                let diff_path = format!("{}.diff", file_path);

                let source_file_path = dst_root.join(&file_path);
                let diff_file_path = tmp_root.join(diff_path);
                let target_file_path = tmp_root.join(format!("{}.patched", file_path));

                let input_file = std::fs::OpenOptions::new()
                    .read(true)
                    .open(&diff_file_path)
                    .map_err(io_error)?;

                let src_file = std::fs::OpenOptions::new()
                    .read(true)
                    .open(source_file_path)
                    .map_err(io_error)?;

                let target_file = std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(false)
                    .open(&target_file_path)
                    .map_err(io_error)?;

                let tx = tx.clone();
                tokio::task::spawn_blocking(|| {
                    patching::patch_file(input_file, src_file, target_file, tx)
                })
                .await
                .unwrap()?;
                fs::rename(target_file_path, tmp_root.join(file_path))
                    .await
                    .map_err(io_error)?;
                fs::remove_file(diff_file_path).await.map_err(io_error)?;
            }
        }

        // Move tmp files to their destination
        if self.old_manifest.is_some() {
            for list in &report.download {
                for file in &list.files {
                    let file_path = file.path();
                    let tmp_entry_root =
                        self.get_file_root(file.is_support(), &list.product_id, false);
                    let dst_entry_root =
                        self.get_file_root(file.is_support(), &list.product_id, true);

                    let final_path = dst_entry_root.join(&file_path);

                    let parent = final_path.parent().unwrap();
                    if !parent.exists() {
                        fs::create_dir_all(parent).await.map_err(io_error)?;
                    }

                    fs::rename(tmp_entry_root.join(&file_path), final_path)
                        .await
                        .map_err(io_error)?;
                }
            }
            for entry in &report.patches {
                let file = &entry.diff;
                let file_path = file.path();
                let tmp_entry_root =
                    self.get_file_root(file.is_support(), &entry.product_id, false);
                let dst_entry_root = self.get_file_root(file.is_support(), &entry.product_id, true);

                let final_path = dst_entry_root.join(&file_path);

                let parent = final_path.parent().unwrap();
                if !parent.exists() {
                    fs::create_dir_all(parent).await.map_err(io_error)?;
                }

                fs::rename(tmp_entry_root.join(&file_path), final_path)
                    .await
                    .map_err(io_error)?;
            }
            let tmp_root = self.get_file_root(false, "0", false);
            fs::remove_dir_all(tmp_root).await.map_err(io_error)?;
        }

        drop(tx);
        if let Err(err) = progress_report.await {
            log::debug!("Failed to wait for the progress {}", err);
        }

        for (source, target) in &new_symlinks {
            log::debug!("Creating symlink {} -> {}", source, target);
            utils::symlink(source, target)?;
        }

        for file in &report.deleted {
            let file_path = file.path();
            let root = self.get_file_root(file.is_support(), "0", true);
            let file_path = root.join(file_path);
            if file_path.exists() {
                log::debug!("Removing {:?}", file_path);
                fs::remove_file(file_path).await.map_err(io_error)?;
            }
        }

        let build_id_file = self
            .get_file_root(false, "0", false)
            .join(".gog-warp-build");
        if build_id_file.exists() {
            fs::remove_file(build_id_file).await.map_err(io_error)?;
        }

        Ok(())
    }
}
