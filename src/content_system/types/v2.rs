use crate::content_system::languages;
use derive_getters::{Dissolve, Getters};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    base_product_id: String,
    client_id: Option<String>,
    client_secret: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    depots: Vec<ManifestDepot>,
    install_directory: String,
    platform: String,
    products: Vec<ManifestProduct>,
    #[serde(default)]
    script_interpreter: bool,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Serialize, Deserialize, Getters, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ManifestDepot {
    size: i64,
    compressed_size: i64,
    #[serde(default)]
    is_gog_depot: bool,
    #[serde(deserialize_with = "languages::serde_language")]
    languages: Vec<String>,
    manifest: String,
    product_id: String,
}

#[derive(Serialize, Deserialize, Getters, Debug, Clone)]
pub struct ManifestProduct {
    name: String,
    #[serde(rename = "productId")]
    product_id: String,
    temp_executable: String,
    temp_arguments: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DepotDetails {
    pub(crate) depot: Depot,
}

#[derive(Serialize, Deserialize, Getters, Dissolve, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Depot {
    items: Vec<DepotEntry>,
    small_files_container: Option<SmallFilesContainer>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(tag = "type")]
pub enum DepotEntry {
    #[serde(rename = "DepotFile")]
    File(DepotFile),
    #[serde(rename = "DepotDirectory")]
    Directory(DepotDirectory),
    #[serde(rename = "DepotLink")]
    Link(DepotLink),
    #[serde(rename = "DepotDiff")]
    Diff(DepotDiff),
}

impl super::traits::EntryUtils for DepotEntry {
    fn path(&self) -> String {
        match self {
            Self::File(f) => f.path(),
            Self::Directory(d) => d.path(),
            Self::Link(l) => l.path(),
            Self::Diff(d) => d.path_source(),
        }
        .replace('\\', "/")
        .trim_matches('/')
        .to_string()
    }

    fn compressed_size(&self) -> i64 {
        match self {
            Self::File(f) => f
                .chunks()
                .iter()
                .fold(0, |acc, ch| acc + ch.compressed_size),
            Self::Diff(f) => f
                .chunks()
                .iter()
                .fold(0, |acc, ch| acc + ch.compressed_size),
            _ => 0,
        }
    }

    fn size(&self) -> i64 {
        match self {
            Self::File(f) => f.chunks().iter().fold(0, |acc, ch| acc + ch.size),
            Self::Diff(f) => f.chunks().iter().fold(0, |acc, ch| acc + ch.size),
            _ => 0,
        }
    }

    fn is_dir(&self) -> bool {
        matches!(self, Self::Directory(_))
    }

    fn is_support(&self) -> bool {
        match self {
            Self::File(f) => f.flags().iter().any(|f| f == "support"),
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct DepotDirectory {
    path: String,
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct DepotLink {
    path: String,
    target: String,
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DepotFile {
    pub(crate) chunks: Vec<Chunk>,
    pub(crate) path: String,
    pub(crate) sfc_ref: Option<SmallFilesContainerRef>,
    pub(crate) sha256: Option<String>,
    pub(crate) md5: Option<String>,
    #[serde(default)]
    pub(crate) flags: Vec<String>,
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct DepotDiff {
    pub(crate) md5_source: String,
    pub(crate) md5_target: String,
    pub(crate) path_source: String,
    pub(crate) path_target: String,
    pub(crate) md5: String,
    pub(crate) chunks: Vec<Chunk>,
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Chunk {
    compressed_md5: String,
    md5: String,
    size: i64,
    compressed_size: i64,
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct SmallFilesContainerRef {
    offset: u64,
    size: u64,
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct SmallFilesContainer {
    chunks: Vec<Chunk>,
}
