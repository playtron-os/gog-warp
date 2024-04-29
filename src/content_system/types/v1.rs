use std::collections::HashMap;

use crate::content_system::languages;
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct Manifest {
    product: ManifestProduct,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
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
}

#[derive(Serialize, Deserialize, Debug)]
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
        size: String,
    },
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct SupportCommand {
    #[serde(deserialize_with = "languages::serde_language")]
    languages: Vec<String>,
    executable: String,
    #[serde(rename = "gameID")]
    game_id: String,
    argument: String,
    systems: Vec<String>,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct GameID {
    #[serde(rename = "gameID")]
    game_id: String,
    name: HashMap<String, String>,
    standalone: bool,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct DepotDetails {
    name: String,
    depot: Depot,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct Depot {
    files: Vec<DepotEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum DepotEntry {
    File(DepotFile),
    Directory(DepotDirectory),
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct DepotFile {
    offset: u64,
    hash: String,
    url: String,
    path: String,
    size: u64,
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug)]
pub struct DepotDirectory {
    directory: bool,
    path: String,
}
