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
