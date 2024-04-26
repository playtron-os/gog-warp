use std::{collections::HashMap, fmt::Display};

use chrono::prelude::*;
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

pub mod v1;
pub mod v2;

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Manifest {
    V1(v1::Manifest),
    V2(v2::Manifest),
}

impl Manifest {
    pub fn install_directory(&self) -> String {
        match self {
            Self::V1(mv1) => mv1.product().install_directory().clone(),
            Self::V2(mv2) => mv2.install_directory().clone(),
        }
    }

    pub fn languages(&self) -> Vec<String> {
        let mut manifest_languages: Vec<String> = Vec::new();
        match self {
            Self::V1(mv1) => {
                for depot in mv1.product().depots() {
                    if let v1::ManifestDepot::Files { languages, .. } = depot {
                        let new_languages: Vec<String> = languages
                            .iter()
                            .filter(|lang| lang.to_lowercase() != "neutral")
                            .filter(|lang| !manifest_languages.contains(lang))
                            .map(|lang| lang.clone())
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
                        .map(|lang| lang.clone())
                        .collect();
                    manifest_languages.extend(new_languages);
                }
            }
        }
        manifest_languages
            .iter()
            .map(|lang| super::languages::get_language(lang).unwrap())
            .map(|lang| lang.code.to_string())
            .collect()
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
