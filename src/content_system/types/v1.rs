use std::collections::HashMap;

use crate::content_system::languages;
use derive_getters::{Dissolve, Getters};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters, Debug, Clone)]
pub struct Manifest {
    product: ManifestProduct,
    #[serde(flatten)]
    unknown_fields: HashMap<String, serde_json::Value>
}

#[derive(Serialize, Deserialize, Getters, Debug, Clone)]
pub struct ManifestProduct {
    timestamp: u32,
    depots: Vec<ManifestDepot>,
    support_commands: Vec<SupportCommand>,
    #[serde(rename = "installDirectory")]
    install_directory: String,
    #[serde(rename = "gameIDs")]
    game_ids: Vec<GameID>,
    #[serde(rename = "rootGameID")]
    root_game_id: String,
    #[serde(flatten)]
    unknown_fields: HashMap<String, serde_json::Value>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ManifestDepot {
    Files {
        #[serde(deserialize_with = "languages::serde_language")]
        languages: Vec<String>,
        size: String,
        #[serde(rename = "gameIDs")]
        game_ids: Vec<String>,
        systems: Vec<String>,
        manifest: String,
    },
    Redist {
        redist: String,
        size: Option<String>,
        #[serde(rename = "targetDir")]
        target_dir: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Getters, Debug, Clone)]
pub struct SupportCommand {
    #[serde(deserialize_with = "languages::serde_language")]
    languages: Vec<String>,
    executable: String,
    #[serde(rename = "gameID")]
    game_id: String,
    argument: String,
    systems: Vec<String>,
}

#[derive(Serialize, Deserialize, Getters, Debug, Clone)]
pub struct GameID {
    #[serde(rename = "gameID")]
    game_id: String,
    name: HashMap<String, String>,
    standalone: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DepotDetails {
    pub(crate) depot: Depot,
}

#[derive(Serialize, Deserialize, Dissolve, Debug)]
pub struct Depot {
    name: String,
    files: Vec<DepotEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum DepotEntry {
    File(DepotFile),
    Directory(DepotDirectory),
}

impl super::traits::EntryUtils for DepotEntry {
    fn path(&self) -> String {
        match self {
            Self::File(f) => f.path(),
            Self::Directory(d) => d.path(),
        }
        .replace('\\', "/")
        .trim_matches('/')
        .to_string()
    }
    fn compressed_size(&self) -> i64 {
        self.size()
    }
    fn size(&self) -> i64 {
        match self {
            Self::File(f) => f.size,
            _ => 0,
        }
    }
    fn is_support(&self) -> bool {
        match self {
            Self::File(f) => *f.support(),
            _ => false,
        }
    }
    fn is_dir(&self) -> bool {
        matches!(self, Self::Directory(_))
    }
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct DepotFile {
    path: String,
    size: i64,
    offset: Option<i64>,
    url: Option<String>,
    #[serde(default)]
    hash: String,
    #[serde(default)]
    support: bool,
    #[serde(default)]
    executable: bool,
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct DepotDirectory {
    directory: bool,
    path: String,
}
