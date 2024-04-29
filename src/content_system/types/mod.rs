use std::{collections::HashMap, fmt::Display, io::Read};

use chrono::prelude::*;
use derive_getters::Getters;
use flate2::read::ZlibDecoder;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    constants::domains::GOG_CDN,
    errors::{json_error, request_error, zlib_error},
};

pub mod v1;
pub mod v2;

#[derive(Debug)]
pub enum DepotEntry {
    V1(v1::DepotEntry),
    V2(v2::DepotEntry),
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Manifest {
    V1(v1::Manifest),
    V2(v2::Manifest),
}

impl Manifest {
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
    pub fn install_size<I, V>(&self, language: &String, dlcs: I) -> (u64, u64)
    where
        I: IntoIterator<Item = V> + Copy,
        V: AsRef<str>,
    {
        let mut download_size: u64 = 0;
        let mut install_size: u64 = 0;

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
                            download_size += size.parse::<u64>().unwrap();
                            install_size += size.parse::<u64>().unwrap();
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

    pub async fn get_files<I, V>(
        &self,
        reqwest_client: &Client,
        language: &String,
        dlcs: I,
    ) -> Result<Vec<DepotEntry>, crate::Error>
    where
        I: IntoIterator<Item = V> + Copy,
        V: AsRef<str>,
    {
        match self {
            Self::V1(mv1) => {
                let mut files: Vec<DepotEntry> = Vec::new();
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
                        let new_files: Vec<DepotEntry> = json_data
                            .depot()
                            .files()
                            .iter()
                            .map(|f| DepotEntry::V1(f.clone()))
                            .collect();
                        files.extend(new_files);
                    }
                }

                Ok(files)
            }
            Self::V2(mv2) => {
                let root_game_id = mv2.base_product_id();
                let mut files = Vec::new();
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

                    zlib.read_to_end(&mut buffer).map_err(zlib_error)?;

                    let json_data: v2::DepotDetails =
                        serde_json::from_slice(&buffer).map_err(json_error)?;
                    let new_files: Vec<DepotEntry> = json_data
                        .depot()
                        .items()
                        .iter()
                        .map(|f| DepotEntry::V2(f.clone()))
                        .collect();
                    files.extend(new_files);
                }
                Ok(files)
            }
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
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

#[derive(Serialize, Deserialize, Getters, Debug)]
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

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct Endpoint {
    endpoint_name: String,
    url: String,
    url_format: String,
    parameters: HashMap<String, String>,
    priority: u32,
    max_fails: u32,
    supports_generation: Vec<u32>,
    fallback_only: bool,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct SizeInfo {
    disk_size: u64,
    download_size: u64,
}
