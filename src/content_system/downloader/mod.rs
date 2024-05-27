use std::path::PathBuf;

use crate::{
    errors::{dbuilder_error, io_error, not_ready_error},
    Core, Error,
};

use tokio::fs;

use super::types::Manifest;
use super::types::{v1, v2, DepotEntry};

mod allocator;
mod diff;
pub mod progress;

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
        let core = self.core.ok_or_else(dbuilder_error)?;
        let manifest = self.manifest.ok_or_else(dbuilder_error)?;
        let build_id = self.build_id.ok_or_else(dbuilder_error)?;
        let language = self.language.ok_or_else(dbuilder_error)?;
        let old_manifest = self.upgrade_from;
        let prev_build_id = self.prev_build_id;

        let install_path = match self.install_root {
            Some(ir) => ir.join(manifest.install_directory()),
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

        let dlcs = self.dlcs;
        let old_dlcs = self.old_dlcs;
        let verify = self.verify;

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
            download_report: None,
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
    manifest: Manifest,
    /// Build id of the new manifest
    build_id: String,
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

    download_report: Option<diff::DiffReport>,
}

impl Downloader {
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Fetches file lists and patches manifest
    pub async fn prepare(&mut self) -> Result<(), Error> {
        // Get depots for main manifest
        let depots = self
            .manifest
            .get_depots(self.core.reqwest_client(), &self.language, &self.dlcs)
            .await?;

        let old_depots = match &self.old_manifest {
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
        todo!();
    }

    fn get_file_root(&self, is_support: bool) -> &PathBuf {
        if is_support {
            &self.support_path
        } else {
            &self.install_path
        }
    }

    /// Execute the download.  
    /// Make sure to run this after [`Self::prepare`]
    pub async fn download(&mut self) -> Result<(), Error> {
        if self.download_report.is_none() {
            return Err(not_ready_error(
                "download not ready, did you forget Downloader::prepare()?",
            ));
        }

        let report = self.download_report.take().unwrap();

        for file_list in &report.download {
            for entry in &file_list.files {
                match entry {
                    DepotEntry::V1(v1::DepotEntry::Directory(dir)) => {
                        let file_path = self
                            .install_path
                            .join(dir.path().replace('\\', "/").trim_matches('/'));
                        if !file_path.exists() {
                            fs::create_dir_all(file_path).await.map_err(io_error)?;
                        }
                    }
                    DepotEntry::V2(v2::DepotEntry::Directory(dir)) => {
                        let file_path = self
                            .install_path
                            .join(dir.path().replace('\\', "/").trim_matches('/'));
                        if !file_path.exists() {
                            fs::create_dir_all(file_path).await.map_err(io_error)?;
                        }
                    }

                    DepotEntry::V1(v1::DepotEntry::File(file)) => {
                        let file_root = self.get_file_root(*file.support());
                        let file_path =
                            file_root.join(file.path().replace('\\', "/").trim_matches('/'));
                        fs::create_dir_all(file_path.parent().unwrap())
                            .await
                            .map_err(io_error)?;
                        let file_handle = fs::OpenOptions::new()
                            .append(true)
                            .create(true)
                            .open(file_path)
                            .await
                            .map_err(io_error)?;
                        allocator::allocate(file_handle, *file.size())?;
                    }

                    DepotEntry::V2(v2::DepotEntry::File(file)) => {
                        let is_support = file.flags().iter().any(|f| f == "support");
                        let file_root = self.get_file_root(is_support);
                        let file_path =
                            file_root.join(file.path().replace('\\', "/").trim_matches('/'));

                        fs::create_dir_all(file_path.parent().unwrap())
                            .await
                            .map_err(io_error)?;
                        let file_handle = fs::OpenOptions::new()
                            .append(true)
                            .create(true)
                            .open(file_path)
                            .await
                            .map_err(io_error)?;
                        let size: i64 = file
                            .chunks()
                            .iter()
                            .fold(0, |acc, chunk| acc + chunk.size());
                        allocator::allocate(file_handle, size)?;
                    }
                    DepotEntry::V2(v2::DepotEntry::Link(_))
                    | DepotEntry::V2(v2::DepotEntry::Diff(_)) => todo!(),
                }
            }
        }

        Ok(())
    }
}
