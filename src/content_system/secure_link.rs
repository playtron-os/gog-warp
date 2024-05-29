use reqwest::{header::HeaderValue, Client};
use serde::Deserialize;
use url::Url;

use crate::auth::types::Token;
use crate::constants::domains::GOG_CONTENT_SYSTEM;
use crate::errors::{request_error, unauthorized_error};

use super::types::Endpoint;

#[derive(Deserialize, Debug)]
pub struct SecureLinkResponse {
    product_id: u32,
    urls: Vec<Endpoint>,
}

pub async fn get_secure_link(
    reqwest_client: &Client,
    version: u8,
    product_id: &str,
    token: &Token,
    path: &str,
    root: &str,
) -> Result<Vec<Endpoint>, crate::Error> {
    let url = format!(
        "{}/products/{}/secure_link?_version=2&path={}",
        GOG_CONTENT_SYSTEM, product_id, path
    );
    let mut params: Vec<(&str, &str)> = vec![];

    if version == 2 {
        params.extend_from_slice(&[("generation", "2")]);
    } else if version == 1 {
        params.extend_from_slice(&[("type", "depot")]);
    }

    if !root.is_empty() {
        params.push(("root", root));
    }

    let url = Url::parse_with_params(&url, params).unwrap();
    let mut auth_header =
        HeaderValue::from_str(&format!("Bearer {}", token.access_token())).unwrap();
    auth_header.set_sensitive(true);
    let response = reqwest_client
        .get(url)
        .header("Authorization", auth_header)
        .send()
        .await
        .map_err(request_error)?;

    if response.status().as_u16() == 401 {
        return Err(unauthorized_error());
    }

    let data: SecureLinkResponse = response.json().await.map_err(request_error)?;

    Ok(data
        .urls
        .iter()
        .filter(|u| {
            u.supports_generation()
                .iter()
                .any(|g| *g == (version as u32))
        })
        .cloned()
        .collect())
}
