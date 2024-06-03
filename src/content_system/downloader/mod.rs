use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::content_system::secure_link;
use crate::errors::{cancelled_error, task_error};
use crate::{
    errors::{dbuilder_error, io_error, not_ready_error},
    Core, Error,
};

use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

use super::dependencies::DependenciesManifest;
use super::types::{traits::EntryUtils, Endpoint, Manifest};
use super::types::{v1, v2, DepotEntry};

mod diff;
mod patching;
pub mod progress;
mod utils;
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
    /// and download invalid/missing ones
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

    cancellation_token: CancellationToken,
    download_report: Option<diff::DiffReport>,
    max_speed: Mutex<i32>,
}

impl Downloader {
    pub fn builder() -> Builder {
        Builder::new()
    }

    // Stops the download
    pub fn get_cancellation(&self) -> CancellationToken {
        self.cancellation_token.clone()
    }

    pub async fn set_max_speed(&self, speed: i32) {
        *self.max_speed.lock().await = speed;
    }

    /// Fetches file lists and patches manifest
    pub async fn prepare(&mut self) -> Result<(), Error> {
        // Get depots for main manifest
        let mut depots = match &self.manifest {
            Some(m) => {
                m.get_depots(self.core.reqwest_client(), &self.language, &self.dlcs)
                    .await?
            }
            None => Vec::new(),
        };

        let mut old_depots = match &self.old_manifest {
            Some(om) => {
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
                let new_deps = dm
                    .get_depots(reqwest_client.clone(), &manifest.dependencies(), false)
                    .await?;
                depots.extend(new_deps);
            }

            if let Some(om) = &self.old_manifest {
                let old_deps = dm
                    .get_depots(reqwest_client.clone(), &om.dependencies(), false)
                    .await?;
                old_depots.extend(old_deps);
            }

            if !self.global_dependencies.is_empty() {
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

    /// Return space required for operation to complete takes in account pre-allocated files
    pub async fn get_requied_space(&mut self) -> Result<i64, Error> {
        let report = self.download_report.take().unwrap();
        let mut size_total: i64 = 0;
        // Since we want to allow the game to be playable after pausing the update
        // we are not subtracting deleted files sizes
        for list in &report.download {
            if let Some(sfc) = &list.sfc {
                let file_root = self.get_file_root(false, false);
                let chunk = sfc.chunks().first().unwrap();
                let file_path = file_root.join(chunk.md5());
                let status = self.get_file_status(&file_path);
                if matches!(status, progress::DownloadFileStatus::NotInitialized) {
                    size_total += sfc.chunks().first().unwrap().size();
                }
            }
            for entry in &list.files {
                if entry.is_dir() {
                    continue;
                }
                let file_root = self.get_file_root(entry.is_support(), false);
                let file_path = file_root.join(entry.path());
                let status = self.get_file_status(&file_path);

                if matches!(status, progress::DownloadFileStatus::NotInitialized) {
                    size_total += entry.size();
                }
            }
        }

        for patch in &report.patches {
            let file_root = self.get_file_root(false, false);
            let file_path = file_root.join(patch.diff.path());
            let status = self.get_file_status(&file_path);
            if matches!(status, progress::DownloadFileStatus::NotInitialized) {
                size_total += patch.diff.size() + patch.destination_file.size();
            }
        }

        self.download_report = Some(report);
        Ok(size_total)
    }

    fn get_file_root(&self, is_support: bool, final_destination: bool) -> &PathBuf {
        if self.old_manifest.is_some() && !final_destination {
            &self.tmp_path
        } else if is_support {
            &self.support_path
        } else {
            &self.install_path
        }
    }

    fn get_file_status(&self, path: &Path) -> progress::DownloadFileStatus {
        if path.exists() {
            return progress::DownloadFileStatus::Done;
        }
        let allocation_file = format!("{}.download", path.to_str().unwrap());
        let allocation_file = PathBuf::from(allocation_file);

        if allocation_file.exists() {
            progress::DownloadFileStatus::Allocated
        } else {
            let diff_file = format!("{}.diff", path.to_str().unwrap());
            let diff_file = PathBuf::from(diff_file);
            if diff_file.exists() {
                return progress::DownloadFileStatus::PatchDownloaded;
            }
            progress::DownloadFileStatus::NotInitialized
        }
    }

    /// Execute the download.  
    /// Make sure to run this after [`Self::prepare`]
    pub async fn download(&self) -> Result<(), Error> {
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

        let timestamp = if let Some(manifest) = &self.manifest {
            manifest.repository_timestamp()
        } else {
            None
        };

        let report = self.download_report.clone().unwrap();
        let mut new_symlinks: Vec<(String, String)> = Vec::new();
        let mut ready_files: HashSet<String> = HashSet::new();
        let mut ready_patches: HashSet<String> = HashSet::new();
        let secure_links: Arc<Mutex<HashMap<String, Vec<Endpoint>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let install_root = self.get_file_root(false, false);
        if !install_root.exists() {
            fs::create_dir_all(install_root).await.map_err(io_error)?;
        }

        // Allocate disk space and generate secure links
        for file_list in &report.download {
            if let Some(sfc) = &file_list.sfc {
                if sfc.chunks().len() != 1 {
                    log::warn!("sfc chunk count != 1");
                }
                let chunk = sfc.chunks().first().unwrap();
                let install_root = self.get_file_root(false, false);
                let file_path = install_root.join(chunk.md5());
                let size = *chunk.size();
                match self.get_file_status(&file_path) {
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
                let entry_root = self.get_file_root(entry.is_support(), false);
                let file_path = entry_root.join(&entry_path);
                if entry.is_dir() {
                    fs::create_dir_all(file_path).await.map_err(io_error)?;
                    continue;
                }

                let file_parent = file_path.parent().unwrap();
                if !file_parent.exists() {
                    fs::create_dir_all(&file_parent).await.map_err(io_error)?;
                }

                let file_size = entry.size();
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
                        }

                        DepotEntry::V2(v2::DepotEntry::Link(link)) => {
                            let link_path = link.path();
                            let target_path = link.target();
                            let link_root = self.get_file_root(false, true);
                            let link_path = link_root.join(link_path);
                            let link_path = link_path.to_str().unwrap();
                            new_symlinks.push((link_path.to_owned(), target_path.to_owned()));
                        }

                        _ => (),
                    }
                    continue;
                }

                match self.get_file_status(&file_path) {
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
                    _ => {
                        ready_files.insert(entry_path.clone());
                    }
                }
            }

            let mut secure_links = secure_links.lock().await;
            let product_id = &file_list.product_id;

            let path = if manifest_version == 2 {
                "/".to_owned()
            } else {
                format!("/windows/{}", timestamp.unwrap())
            };

            if !secure_links.contains_key(product_id) {
                let endpoints = if product_id == "dependencies" {
                    secure_link::get_dependencies_link(self.core.reqwest_client()).await?
                } else {
                    let token = self.core.obtain_galaxy_token().await?;
                    secure_link::get_secure_link(
                        self.core.reqwest_client(),
                        manifest_version,
                        product_id,
                        &token,
                        &path,
                        "",
                    )
                    .await?
                };
                secure_links.insert(product_id.clone(), endpoints);
            }
        }

        for patch in &report.patches {
            let entry = &patch.diff;
            let entry_path = entry.path();
            let entry_root = self.get_file_root(entry.is_support(), false);
            let file_path = entry_root.join(&entry_path);
            let file_parent = file_path.parent().unwrap();
            if !file_parent.exists() {
                fs::create_dir_all(&file_parent).await.map_err(io_error)?;
            }
            let product_id = format!("{}patch", patch.product_id);

            let mut secure_links = secure_links.lock().await;
            if !secure_links.contains_key(&product_id) {
                let token = self.core.obtain_galaxy_token().await?;
                let endpoints = secure_link::get_secure_link(
                    self.core.reqwest_client(),
                    manifest_version,
                    &product_id,
                    &token,
                    "/",
                    "/patches/store",
                )
                .await?;
                secure_links.insert(product_id.clone(), endpoints);
            }

            match self.get_file_status(&file_path) {
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
                    ready_files.insert(entry_path.clone());
                }
                progress::DownloadFileStatus::PatchDownloaded => {
                    ready_patches.insert(entry_path.clone());
                }
            }
        }

        let file_semaphore = Arc::new(Semaphore::new(4));
        let chunk_semaphore = Arc::new(Semaphore::new(8));

        // TODO: Handle download speed and progress reports
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        let mut handles: Vec<_> = Vec::new();

        // Spawn download tasks
        for list in &report.download {
            if let Some(sfc) = &list.sfc {
                let chunk = sfc.chunks().first().unwrap();
                if !ready_files.contains(chunk.md5()) {
                    let entry_root = self.get_file_root(false, false);
                    let file_path = entry_root.join(chunk.md5());

                    let product_id = list.product_id.clone();
                    let secure_links = secure_links.clone();
                    let chunk_semaphore = chunk_semaphore.clone();
                    let chunks = sfc.chunks().clone();
                    let file_semaphore = file_semaphore.clone();
                    let path = chunk.md5().clone();
                    let reqwest_client = self.core.reqwest_client().clone();
                    let tx = tx.clone();
                    handles.push(tokio::spawn(async move {
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
                    }));
                }
            }
            for file in &list.files {
                let file_path = file.path();
                if ready_files.contains(&file_path) || file.is_dir() {
                    continue;
                }
                let root = self.get_file_root(file.is_support(), false);
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
                        let product_id = list.product_id.clone();
                        handles.push(tokio::spawn(async move {
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
                        }));
                    }
                    DepotEntry::V1(v1_entry) => {
                        let file_semaphore = file_semaphore.clone();
                        let secure_links = secure_links.clone();

                        let product_id = list.product_id.clone();
                        let reqwest_client = self.core.reqwest_client().clone();
                        let v1_entry = v1_entry.clone();
                        let tx = tx.clone();
                        handles.push(tokio::spawn(async move {
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
                        }));
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
            let entry_root = self.get_file_root(file.is_support(), false);
            let file_path = entry_root.join(&entry_path);

            let file_semaphore = file_semaphore.clone();
            let secure_links = secure_links.clone();

            let chunk_semaphore = chunk_semaphore.clone();
            let reqwest_client = self.core.reqwest_client().clone();
            let v2_entry = file.clone();
            let tx = tx.clone();
            let product_id = format!("{}patch", patch.product_id);
            handles.push(tokio::spawn(async move {
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
            }));
        }

        futures::future::try_join_all(handles)
            .await
            .map_err(task_error)?;

        // Finalize the download

        // Extract the small file container
        for list in &report.download {
            if let Some(sfc) = &list.sfc {
                let chunk = sfc.chunks().first().unwrap();
                let entry_root = self.get_file_root(false, false);
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
                            let entry_root = self.get_file_root(file.is_support(), false);
                            let file_path = entry_root.join(file_path);
                            if matches!(
                                self.get_file_status(&file_path),
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
                let tmp_root = self.get_file_root(false, false);
                let dst_root = self.get_file_root(false, true);

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

                tokio::task::spawn_blocking(|| {
                    patching::patch_file(input_file, src_file, target_file)
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
                    let tmp_entry_root = self.get_file_root(file.is_support(), false);
                    let dst_entry_root = self.get_file_root(file.is_support(), true);

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
                let tmp_entry_root = self.get_file_root(file.is_support(), false);
                let dst_entry_root = self.get_file_root(file.is_support(), true);

                let final_path = dst_entry_root.join(&file_path);

                let parent = final_path.parent().unwrap();
                if !parent.exists() {
                    fs::create_dir_all(parent).await.map_err(io_error)?;
                }

                fs::rename(tmp_entry_root.join(&file_path), final_path)
                    .await
                    .map_err(io_error)?;
            }
            let tmp_root = self.get_file_root(false, false);
            fs::remove_dir_all(tmp_root).await.map_err(io_error)?;
        }

        for (source, target) in &new_symlinks {
            utils::symlink(source, target)?;
        }

        for file in &report.deleted {
            let file_path = file.path();
            let root = self.get_file_root(file.is_support(), true);
            let file_path = root.join(file_path);
            if file_path.exists() {
                fs::remove_file(file_path).await.map_err(io_error)?;
            }
        }

        Ok(())
    }
}
