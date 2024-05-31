use std::{collections::HashMap, fmt::Display};

use async_compression::tokio::bufread::ZlibDecoder;
use chrono::prelude::*;
use derive_getters::Getters;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

use crate::{
    constants::domains::GOG_CDN,
    errors::{json_error, request_error, zlib_error},
};

pub(crate) mod traits;
pub mod v1;
pub mod v2;

#[derive(Debug, Clone)]
pub struct FileList {
    pub(crate) product_id: String,
    pub(crate) files: Vec<DepotEntry>,
    pub(crate) sfc: Option<v2::SmallFilesContainer>,
}

impl FileList {
    pub fn new(product_id: String, files: Vec<DepotEntry>) -> Self {
        Self {
            product_id,
            files,
            sfc: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum DepotEntry {
    V1(v1::DepotEntry),
    V2(v2::DepotEntry),
}

impl traits::EntryUtils for DepotEntry {
    fn path(&self) -> String {
        match self {
            Self::V1(v1) => traits::EntryUtils::path(v1),
            Self::V2(v2) => traits::EntryUtils::path(v2),
        }
    }
    fn size(&self) -> i64 {
        match self {
            Self::V1(v1) => traits::EntryUtils::size(v1),
            Self::V2(v2) => traits::EntryUtils::size(v2),
        }
    }

    fn is_dir(&self) -> bool {
        match self {
            Self::V1(v1) => traits::EntryUtils::is_dir(v1),
            Self::V2(v2) => traits::EntryUtils::is_dir(v2),
        }
    }

    fn is_support(&self) -> bool {
        match self {
            Self::V1(v1) => traits::EntryUtils::is_support(v1),
            Self::V2(v2) => traits::EntryUtils::is_support(v2),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Manifest {
    V1(v1::Manifest),
    V2(v2::Manifest),
}

impl Manifest {
    /// Returns base game id
    pub fn product_id(&self) -> String {
        match self {
            Self::V1(mv1) => mv1.product().root_game_id().clone(),
            Self::V2(mv2) => mv2.base_product_id().clone(),
        }
    }

    /// For V1 builds used to prepare secure links
    pub fn repository_timestamp(&self) -> Option<u32> {
        if let Self::V1(mv1) = self {
            return Some(*mv1.product().timestamp());
        }
        None
    }

    /// Gets game install directory name
    pub fn install_directory(&self) -> String {
        match self {
            Self::V1(mv1) => mv1.product().install_directory().clone(),
            Self::V2(mv2) => mv2.install_directory().clone(),
        }
    }

    /// Lists available DLC ids for this build
    pub fn dlcs(&self) -> Vec<String> {
        match self {
            Self::V1(mv1) => mv1
                .product()
                .game_ids()
                .iter()
                .map(|p| p.game_id().to_owned())
                .filter(|p| p != mv1.product().root_game_id())
                .collect(),
            Self::V2(mv2) => mv2
                .products()
                .iter()
                .map(|p| p.product_id().to_owned())
                .filter(|p| p != mv2.base_product_id())
                .collect(),
        }
    }

    /// Lists dependencies ids required by the game  
    /// Note: Some dependencies are to be downloaded into game directory e.g DOSBOX.
    pub fn dependencies(&self) -> Vec<String> {
        match self {
            Self::V1(mv1) => {
                let mut dependencies = Vec::new();

                for depot in mv1.product().depots() {
                    if let v1::ManifestDepot::Redist { redist, .. } = depot {
                        dependencies.push(redist.to_owned());
                    }
                }

                dependencies
            }
            Self::V2(mv2) => mv2.dependencies().clone(),
        }
    }

    /// Returns languages supported by the build
    pub fn languages(&self) -> Vec<String> {
        let mut manifest_languages: Vec<String> = Vec::new();
        match self {
            Self::V1(mv1) => {
                for depot in mv1.product().depots() {
                    if let v1::ManifestDepot::Files { languages, .. } = depot {
                        let new_languages: Vec<String> = languages
                            .iter()
                            .filter(|lang| lang.to_lowercase() != "*")
                            .filter(|lang| !manifest_languages.contains(lang))
                            .cloned()
                            .collect();

                        manifest_languages.extend(new_languages);
                    }
                }
            }
            Self::V2(mv2) => {
                for depot in mv2.depots() {
                    let new_languages: Vec<String> = depot
                        .languages()
                        .iter()
                        .filter(|lang| lang.as_str() != "*")
                        .filter(|lang| !manifest_languages.contains(lang))
                        .cloned()
                        .collect();
                    manifest_languages.extend(new_languages);
                }
            }
        }
        manifest_languages
    }

    /// Returns a tuple of (compressed_size, decompressed_size)
    /// based on wanted language and dlcs
    /// This consists of game files alone
    /// The actual download size may slightly differ depending on the implementation
    // TODO: Mention dependencies system
    pub fn install_size<I, V>(&self, language: &String, dlcs: I) -> (i64, i64)
    where
        I: IntoIterator<Item = V> + Copy,
        V: AsRef<str>,
    {
        let mut download_size: i64 = 0;
        let mut install_size: i64 = 0;

        match self {
            Self::V1(mv1) => {
                let root_game_id = mv1.product().root_game_id();
                for depot in mv1.product().depots() {
                    if let v1::ManifestDepot::Files {
                        size,
                        languages,
                        game_ids,
                        ..
                    } = depot
                    {
                        // Check if depot is on wanted DLC list or if it's a base game
                        if !game_ids.contains(root_game_id)
                            && !dlcs
                                .into_iter()
                                .any(|dlc| game_ids.iter().any(|id| id == dlc.as_ref()))
                        {
                            continue;
                        }
                        if languages.contains(&"*".to_string()) || languages.contains(language) {
                            download_size += size.parse::<i64>().unwrap();
                            install_size += size.parse::<i64>().unwrap();
                        }
                    }
                }
            }
            Self::V2(mv2) => {
                let root_game_id = mv2.base_product_id();
                for depot in mv2.depots() {
                    // Check if depot is on wanted DLC list or if it's a base game
                    if depot.product_id() != root_game_id
                        && !dlcs
                            .into_iter()
                            .any(|dlc| depot.product_id() == dlc.as_ref())
                    {
                        continue;
                    }

                    if depot.languages().contains(&"*".to_string())
                        || depot.languages().contains(language)
                    {
                        download_size += depot.compressed_size();
                        install_size += depot.size();
                    }
                }
            }
        }

        (download_size, install_size)
    }

    pub async fn get_depots<I, V>(
        &self,
        reqwest_client: &Client,
        language: &String,
        dlcs: I,
    ) -> Result<Vec<FileList>, crate::Error>
    where
        I: IntoIterator<Item = V> + Copy,
        V: AsRef<str>,
    {
        let mut depots = Vec::new();
        match self {
            Self::V1(mv1) => {
                let root_game_id = mv1.product().root_game_id();
                for depot in mv1.product().depots() {
                    if let v1::ManifestDepot::Files {
                        languages,
                        game_ids,
                        manifest,
                        ..
                    } = depot
                    {
                        // Check if depot is on wanted DLC list or if it's a base game
                        if !game_ids.contains(root_game_id)
                            && !dlcs
                                .into_iter()
                                .any(|dlc| game_ids.iter().any(|id| id == dlc.as_ref()))
                        {
                            continue;
                        }

                        if !languages.contains(&"*".to_string()) && !languages.contains(language) {
                            continue;
                        }

                        let url = format!(
                            "{}/content-system/v1/manifests/{}/windows/{}/{}",
                            GOG_CDN,
                            game_ids.first().unwrap(),
                            mv1.product().timestamp(),
                            manifest
                        );
                        let response = reqwest_client
                            .get(url)
                            .send()
                            .await
                            .map_err(request_error)?;

                        let json_data: v1::DepotDetails =
                            response.json().await.map_err(request_error)?;
                        let files = json_data
                            .depot
                            .dissolve()
                            .1
                            .into_iter()
                            .map(DepotEntry::V1)
                            .collect();

                        depots.push(FileList::new(game_ids.first().unwrap().to_string(), files));
                    }
                }
            }
            Self::V2(mv2) => {
                let root_game_id = mv2.base_product_id();
                for depot in mv2.depots() {
                    // Check if depot is on wanted DLC list or if it's a base game
                    if depot.product_id() != root_game_id
                        && !dlcs
                            .into_iter()
                            .any(|dlc| depot.product_id() == dlc.as_ref())
                    {
                        continue;
                    }

                    if !depot.languages().contains(&"*".to_string())
                        && !depot.languages().contains(language)
                    {
                        continue;
                    }

                    let galaxy_path = crate::utils::hash_to_galaxy_path(depot.manifest());
                    let url = format!("{}/content-system/v2/meta/{}", GOG_CDN, galaxy_path);
                    let response = reqwest_client
                        .get(url)
                        .send()
                        .await
                        .map_err(request_error)?;
                    let compressed_manifest = response.bytes().await.map_err(request_error)?;

                    let mut zlib = ZlibDecoder::new(&compressed_manifest[..]);
                    let mut buffer = Vec::new();

                    zlib.read_to_end(&mut buffer).await.map_err(zlib_error)?;

                    let json_data: v2::DepotDetails =
                        serde_json::from_slice(&buffer).map_err(json_error)?;
                    let (entries, sfc) = json_data.depot.dissolve();
                    let entries = entries.into_iter().map(DepotEntry::V2).collect();
                    let mut f_list = FileList::new(depot.product_id().to_owned(), entries);
                    f_list.sfc = sfc;
                    depots.push(f_list);
                }
            }
        }
        Ok(depots)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Windows,
    OsX,
    //Linux
}

impl Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Self::Windows => f.write_str("windows"),
            Self::OsX => f.write_str("osx"),
            //Self::Linux => f.write_str("linux"),
        }
    }
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct BuildResponse {
    total_count: u32,
    count: u32,
    items: Vec<Build>,
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct Build {
    build_id: String,
    product_id: String,
    os: Platform,
    branch: Option<String>,
    version_name: String,
    tags: Vec<String>,
    public: bool,
    date_published: DateTime<Utc>,
    generation: u32,
    urls: Vec<Endpoint>,
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct Endpoint {
    pub(crate) endpoint_name: String,
    #[serde(default)]
    pub(crate) url: String,
    pub(crate) url_format: String,
    pub(crate) parameters: HashMap<String, serde_json::Value>,
    pub(crate) priority: u32,
    pub(crate) max_fails: u32,
    pub(crate) supports_generation: Vec<u32>,
    pub(crate) fallback_only: bool,
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct SizeInfo {
    disk_size: u64,
    download_size: u64,
}
