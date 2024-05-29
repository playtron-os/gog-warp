use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::content_system::secure_link;
use crate::errors::cancelled_error;
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
        let old_manifest = self.upgrade_from;
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

        Ok(Downloader {
            core,
            manifest,
            old_manifest,
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
    /// Whether to verify the files based on the manifest
    verify: bool,
    /// Manifest to use for dependencies
    dependency_manifest: Option<DependenciesManifest>,
    global_dependencies: Vec<String>,

    cancellation_token: CancellationToken,
    download_report: Option<diff::DiffReport>,
    max_speed: Mutex<i32>,
}

impl Downloader {
    pub fn builder() -> Builder {
        Builder::new()
    }

    // Stops the download
    pub fn get_cancellaction(&self) -> CancellationToken {
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

    /// Check if enough disk space is available for operation to complete safely
    pub async fn perform_safety_checks(&mut self) -> Result<(), Error> {
        let report = self.download_report.take().unwrap();
        let mut size_total: i64 = 0;
        // Since we want to allow the game to be playable after pausing the download
        // we are not subtracting deleted files sizes
        for list in &report.download {
            if let Some(sfc) = &list.sfc {
                size_total += sfc.chunks().first().unwrap().size();
            }
            for entry in &list.files {
                size_total += entry.size();
            }
        }

        println!("{:?}", size_total);
        self.download_report = Some(report);
        Ok(())
    }

    fn get_file_root(&self, is_support: bool) -> &PathBuf {
        if is_support {
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

        let report = self.download_report.clone().unwrap();
        let mut new_symlinks: Vec<(String, String)> = Vec::new();
        let mut ready_files: HashSet<String> = HashSet::new();
        let secure_links: Arc<Mutex<HashMap<String, Vec<Endpoint>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let mut file_count: u32 = 0;

        if !self.install_path.exists() {
            fs::create_dir_all(&self.install_path)
                .await
                .map_err(io_error)?;
        }

        for file_list in &report.download {
            if let Some(sfc) = &file_list.sfc {
                if sfc.chunks().len() != 1 {
                    log::warn!("sfc chunk count != 1");
                }
                let chunk = sfc.chunks().first().unwrap();
                let file_path = self.install_path.join(chunk.md5());
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
                        file_count += 1;
                        utils::allocate(file_handle, size).await?;
                    }
                    progress::DownloadFileStatus::Done => {
                        ready_files.insert(chunk.md5().clone());
                    }
                }
            }
            let mut contains_patches: bool = false;
            for entry in &file_list.files {
                if self.cancellation_token.is_cancelled() {
                    return Err(cancelled_error());
                }
                // TODO: Normalize the path to account for existing files on
                // case sensitive file systems
                // e.g Binaries/Game.exe -> binaries/Game.exe
                // In the future detect ext4 case-folding and use that as well
                let entry_path = entry.path();
                let entry_root = self.get_file_root(entry.is_support());
                let file_path = entry_root.join(&entry_path);
                if entry.is_dir() {
                    fs::create_dir_all(file_path).await.map_err(io_error)?;
                    continue;
                }

                let file_parent = file_path.parent().unwrap();
                if !file_parent.exists() {
                    fs::create_dir_all(&file_parent).await.map_err(io_error)?;
                }

                if matches!(entry, DepotEntry::V2(v2::DepotEntry::Diff(_))) {
                    contains_patches = true;
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
                        file_count += 1;
                        utils::allocate(file_handle, file_size).await?;
                    }
                    progress::DownloadFileStatus::Done => {
                        ready_files.insert(entry_path.clone());
                    }
                }
            }
            let mut secure_links = secure_links.lock().await;
            let mut product_id = file_list.product_id.clone();
            if contains_patches {
                product_id.push_str("patch");
            }
            let root = if contains_patches {
                "/patches/store"
            } else {
                ""
            };

            if !secure_links.contains_key(&product_id) {
                let endpoints = if product_id == "dependencies" {
                    secure_link::get_dependencies_link(self.core.reqwest_client()).await?
                } else {
                    let token = self.core.obtain_galaxy_token().await?;
                    secure_link::get_secure_link(
                        self.core.reqwest_client(),
                        manifest_version,
                        &product_id,
                        &token,
                        "/",
                        root,
                    )
                    .await?
                };
                secure_links.insert(product_id.clone(), endpoints);
            }
        }

        let file_semaphore = Arc::new(Semaphore::new(4));
        let chunk_semaphore = Arc::new(Semaphore::new(10));

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<()>();

        for list in &report.download {
            if let Some(sfc) = &list.sfc {
                let chunk = sfc.chunks().first().unwrap();
                if !ready_files.contains(chunk.md5()) {
                    let entry_root = self.get_file_root(false);
                    let file_path = entry_root.join(chunk.md5());
                    let file_permit = file_semaphore.clone().acquire_owned().await.unwrap();
                    let secure_links = secure_links.lock().await;
                    let endpoints = secure_links.get(&list.product_id).unwrap().clone();

                    let chunk_semaphore = chunk_semaphore.clone();
                    let chunks = sfc.chunks().clone();
                    let path = chunk.md5().clone();
                    let reqwest_client = self.core.reqwest_client().clone();
                    let tx = tx.clone();
                    tokio::spawn(worker::v2(
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
                    ));
                }
            }
            for file in &list.files {
                let file_path = file.path();
                if ready_files.contains(&file_path) {
                    continue;
                }
                let root = self.get_file_root(file.is_support());
                let file_path = root.join(file_path);
                match file {
                    DepotEntry::V2(v2_entry) => {
                        if let v2::DepotEntry::File(file) = &v2_entry {
                            if file.sfc_ref.is_some() {
                                log::debug!(
                                    "skipping {}, it will be extracted from sfc",
                                    file.path
                                );
                                file_count -= 1;
                                continue;
                            }
                        }
                        let file_permit = file_semaphore.clone().acquire_owned().await.unwrap();
                        let secure_links = secure_links.lock().await;
                        let endpoints = secure_links.get(&list.product_id).unwrap().clone();

                        let chunk_semaphore = chunk_semaphore.clone();
                        let reqwest_client = self.core.reqwest_client().clone();
                        let v2_entry = v2_entry.clone();
                        let tx = tx.clone();
                        tokio::spawn(worker::v2(
                            file_permit,
                            reqwest_client,
                            chunk_semaphore,
                            endpoints,
                            v2_entry,
                            file_path,
                            tx,
                        ));
                    }
                    _ => todo!(),
                }
            }
        }

        while file_count > 0 {
            if let Some(_) = rx.recv().await {
                file_count -= 1;
            }
        }

        // Finalize the download

        for list in &report.download {
            if let Some(sfc) = &list.sfc {
                let chunk = sfc.chunks().first().unwrap();
                let entry_root = self.get_file_root(false);
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
                            let entry_root = self.get_file_root(file.is_support());
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

        for (source, target) in &new_symlinks {
            utils::symlink(&self.install_path, source, target)?;
        }

        for file in &report.deleted {
            let file_path = file.path();
            let root = self.get_file_root(file.is_support());
            let file_path = root.join(file_path);
            if file_path.exists() {
                fs::remove_file(file_path).await.map_err(io_error)?;
            }
        }

        Ok(())
    }
}
