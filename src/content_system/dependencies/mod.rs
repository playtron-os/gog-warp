use async_compression::tokio::bufread::ZlibDecoder;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

use super::languages;
use super::types::{v2, DepotEntry, FileList};
use crate::constants::domains::GOG_CDN;
use crate::errors::{request_error, serde_error, zlib_error};

pub const DEPENDENCIES_URL: &str =
    "https://content-system.gog.com/dependencies/repository?generation=2";

#[derive(Serialize, Deserialize, Debug)]
pub struct DependenciesManifest {
    pub depots: Vec<DependencyDepot>,
}

impl DependenciesManifest {
    /// Function to get depots for dependencies that are in a list  
    /// when `global` is set to true, only global dependencies will be returned,
    /// otherwise only dependencies meant for game directory
    pub async fn get_depots(
        &self,
        reqwest_client: Client,
        wanted_dependencies: &[String],
        global: bool,
    ) -> Result<Vec<FileList>, crate::Error> {
        let mut lists = Vec::new();
        for depot in &self.depots {
            let is_global = !depot.executable.path.is_empty();
            if global ^ is_global {
                continue;
            }

            if wanted_dependencies
                .iter()
                .any(|dep| &depot.dependency_id == dep)
            {
                let galaxy_path = crate::utils::hash_to_galaxy_path(&depot.manifest);
                let url = format!(
                    "{}/content-system/v2/dependencies/meta/{}",
                    GOG_CDN, galaxy_path
                );
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
                    serde_json::from_slice(&buffer).map_err(serde_error)?;
                let (entries, _sfc) = json_data.depot.dissolve();
                let entries = entries.into_iter().map(DepotEntry::V2).collect();
                let mut f_list = FileList::new(depot.dependency_id.clone(), entries);
                f_list.is_dependency = true;
                lists.push(f_list);
            }
        }
        Ok(lists)
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DependencyDepot {
    pub compressed_size: u64,
    pub size: u64,
    pub dependency_id: String,
    pub executable: DependencyExecutable,
    pub internal: bool,
    #[serde(deserialize_with = "languages::serde_language")]
    pub languages: Vec<String>,
    pub manifest: String,
    pub readable_name: String,
    pub signature: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DependencyExecutable {
    pub arguments: String,
    pub path: String,
}

#[derive(Deserialize)]
struct Repository {
    repository_manifest: String,
    build_id: String,
    generation: u32,
}

pub async fn get_manifest(reqwest_client: Client) -> Result<DependenciesManifest, crate::Error> {
    let response = reqwest_client
        .get(DEPENDENCIES_URL)
        .send()
        .await
        .map_err(request_error)?;
    let repo: Repository = response.json().await.map_err(request_error)?;

    let response = reqwest_client
        .get(repo.repository_manifest)
        .send()
        .await
        .map_err(request_error)?;

    let manifest_raw = response.bytes().await.map_err(request_error)?;
    let mut zlib = ZlibDecoder::new(&manifest_raw[..]);
    let mut buffer = Vec::new();
    zlib.read_to_end(&mut buffer).await.map_err(zlib_error)?;
    let manifest: DependenciesManifest = serde_json::from_slice(&buffer).map_err(serde_error)?;
    Ok(manifest)
}
