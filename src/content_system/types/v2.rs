use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    base_product_id: String,
    client_id: Option<String>,
    client_secret: Option<String>,
    dependencies: Vec<String>,
    depots: Vec<ManifestDepot>,
    install_directory: String,
    platform: String,
    products: Vec<ManifestProduct>,
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ManifestDepot {
    size: u64,
    compressed_size: u64,
    languages: Vec<String>,
    manifest: String,
    product_id: String,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct ManifestProduct {
    name: String,
    #[serde(rename = "productId")]
    product_id: String,
    temp_executable: String,
    temp_arguments: String,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DepotDetails {
    depot: Depot,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Depot {
    items: Vec<DepotEntry>,
    small_files_container: Option<SmallFilesContainer>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum DepotEntry {
    #[serde(rename = "DepotFile")]
    File(DepotFile),
    #[serde(rename = "DepotDirectory")]
    Directory(DepotDirectory),
    #[serde(rename = "DepotLink")]
    Link(DepotLink),
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct DepotDirectory {
    path: String,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct DepotLink {
    path: String,
    target: String,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DepotFile {
    chunks: Vec<Chunk>,
    path: String,
    sfc_ref: Option<SmallFilesContainerRef>,
    sha256: Option<String>,
    md5: Option<String>,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    compressed_md5: String,
    md5: String,
    size: u64,
    compressed_size: u64,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct SmallFilesContainerRef {
    offset: u64,
    size: u64,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct SmallFilesContainer {
    chunks: Vec<Chunk>,
}
