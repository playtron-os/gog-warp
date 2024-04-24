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
