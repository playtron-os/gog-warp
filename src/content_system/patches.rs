use async_compression::tokio::bufread::ZlibDecoder;
use derive_getters::Getters;
use reqwest::Client;
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use url::Url;

use crate::constants::domains::{GOG_CDN, GOG_CONTENT_SYSTEM};
use crate::errors::{json_error, request_error, zlib_error};

use super::types::v2::{DepotDetails, ManifestDepot};
use super::types::{DepotEntries, DepotEntry, Manifest};

#[derive(Deserialize, Getters, Debug)]
pub struct PatchIndex {
    id: String,
    from_build_id: String,
    to_build_id: String,
    link: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PatchProduct {
    name: String,
    product_id: String,
}

#[derive(Deserialize, Getters, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PatchDepots {
    base_product_id: String,
    client_id: String,
    client_secret: String,
    algorithm: String,
    #[serde(rename = "buildId_source")]
    build_id_source: String,
    #[serde(rename = "buildId_target")]
    build_id_target: String,
    products: Vec<PatchProduct>,
    depots: Vec<ManifestDepot>,
}

pub async fn get_patches(
    reqwest_client: &Client,
    manifest: &Manifest,
    build_id: &String,
    old_manifest: &Option<Manifest>,
    old_build_id: Option<String>,
    dlcs: Vec<String>,
    new_language: &String,
    old_language: &String,
) -> Result<Option<DepotEntries>, crate::Error> {
    if old_manifest.is_none() || old_build_id.is_none() {
        return Ok(None);
    }
    if let Manifest::V1(_) = manifest {
        return Ok(None);
    }
    if let Some(Manifest::V1(_)) = old_manifest {
        return Ok(None);
    }

    let product_id = manifest.product_id();

    let index_url = format!("{}/products/{}/patches", GOG_CONTENT_SYSTEM, product_id);
    let index_url = Url::parse_with_params(
        &index_url,
        [
            ("_version", "4"),
            ("from_build_id", &old_build_id.unwrap()),
            ("to_build_id", build_id),
        ],
    )
    .unwrap();
    let response = reqwest_client
        .get(index_url)
        .send()
        .await
        .map_err(request_error)?;

    if response.status().as_u16() == 404 {
        return Ok(None);
    }
    let index: PatchIndex = response.json().await.map_err(request_error)?;
    let depots_res = reqwest_client
        .get(index.link())
        .send()
        .await
        .map_err(request_error)?;

    let depots: PatchDepots = {
        let data = depots_res.bytes().await.map_err(request_error)?;
        let mut zlib = ZlibDecoder::new(&data[..]);
        let mut buffer = Vec::new();
        zlib.read_to_end(&mut buffer).await.map_err(zlib_error)?;
        serde_json::from_slice(&buffer).map_err(json_error)?
    };

    let wanted_depots: Vec<&ManifestDepot> = depots
        .depots()
        .iter()
        .filter(|d| {
            // Check if product matches root or one of previously installed dlcs
            (d.product_id() == &product_id || dlcs.contains(d.product_id()))
                && (d.languages().iter().any(|l| l == "*") // Check if depot is for all languages
                    || (d.languages().contains(old_language) // Or check if both 
                        && d.languages().contains(new_language))) // languages match the depot
        })
        .collect();

    let mut file_patches: DepotEntries = DepotEntries::new();
    for depot in wanted_depots {
        let url = format!(
            "{}/content-system/v2/patches/meta/{}",
            GOG_CDN,
            crate::utils::hash_to_galaxy_path(depot.manifest())
        );
        let response = reqwest_client
            .get(url)
            .send()
            .await
            .map_err(request_error)?;
        let details: DepotDetails = {
            let data = response.bytes().await.map_err(request_error)?;
            let mut zlib = ZlibDecoder::new(&data[..]);
            let mut buffer = Vec::new();
            zlib.read_to_end(&mut buffer).await.map_err(zlib_error)?;
            serde_json::from_slice(&buffer).map_err(json_error)?
        };

        let mut patches = details
            .depot()
            .items()
            .iter()
            .map(|e| DepotEntry::V2(e.clone()))
            .collect::<Vec<DepotEntry>>();
        if let Some(prev_patches) = file_patches.get(depot.product_id()) {
            patches.extend(prev_patches.clone());
        }
        file_patches.insert(depot.product_id().clone(), patches);
    }

    Ok(Some(file_patches))
}
