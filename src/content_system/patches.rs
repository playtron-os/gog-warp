use async_compression::tokio::bufread::ZlibDecoder;
use derive_getters::Getters;
use reqwest::Client;
use serde::Deserialize;
use tokio::io::AsyncReadExt;
use url::Url;

use crate::constants::domains::{GOG_CDN, GOG_CONTENT_SYSTEM};
use crate::errors::{request_error, serde_error, zlib_error};

use super::types::v2::{DepotDetails, ManifestDepot};
use super::types::{DepotEntry, FileList, Manifest};

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
    manifest: &Option<Manifest>,
    build_id: &Option<String>,
    old_manifest: &Option<Manifest>,
    old_build_id: Option<String>,
    dlcs: Vec<String>,
    new_language: &String,
    old_language: &String,
) -> Result<Option<Vec<FileList>>, crate::Error> {
    if manifest.is_none() || build_id.is_none() {
        return Ok(None);
    }
    if old_manifest.is_none() || old_build_id.is_none() {
        return Ok(None);
    }
    if let Some(Manifest::V1(_)) = manifest {
        return Ok(None);
    }
    if let Some(Manifest::V1(_)) = old_manifest {
        return Ok(None);
    }

    let build_id = build_id.clone().unwrap();

    let product_id = if let Some(manifest) = manifest {
        manifest.product_id()
    } else {
        return Ok(None);
    };

    let index_url = format!("{}/products/{}/patches", GOG_CONTENT_SYSTEM, product_id);
    let index_url = Url::parse_with_params(
        &index_url,
        [
            ("_version", "4"),
            ("from_build_id", &old_build_id.unwrap()),
            ("to_build_id", &build_id),
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
        match serde_json::from_slice(&buffer) {
            Ok(d) => d,
            Err(_) => return Ok(None), // Most likely an empty patch
        }
    };

    // Assert that the algorithm is the one we support
    if depots.algorithm != "xdelta3" {
        return Ok(None);
    }

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

    let mut file_patches: Vec<FileList> = Vec::new();
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
            serde_json::from_slice(&buffer).map_err(serde_error)?
        };

        let patches = details
            .depot
            .dissolve()
            .0
            .into_iter()
            .map(DepotEntry::V2)
            .collect::<Vec<DepotEntry>>();
        file_patches.push(FileList::new(depot.product_id().to_owned(), patches));
    }

    Ok(Some(file_patches))
}
