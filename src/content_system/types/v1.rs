use std::collections::HashMap;

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
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum ManifestDepot {
    Files {
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
