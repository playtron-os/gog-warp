use reqwest::{Client, Url};

use crate::auth::types::Token;
use crate::constants::domains::GOG_CONTENT_SYSTEM;
use crate::errors::request_error;

pub mod dependencies;
#[cfg(feature = "downloader")]
pub mod downloader;
pub mod languages;
pub mod patches;
pub mod secure_link;
#[cfg(test)]
mod tests;
pub mod types;

pub(crate) async fn get_builds(
    client: &Client,
    product_id: &str,
    platform: types::Platform,
    token: Option<Token>,
    password: Option<String>,
) -> Result<types::BuildResponse, crate::Error> {
    let mut params = vec![
        ("generation".to_string(), "2".to_string()),
        ("_version".to_string(), "2".to_string()),
    ];
    if let Some(passwd) = password {
        params.push(("password".to_string(), passwd));
    }
    let url = format!(
        "{}/products/{}/os/{}/builds",
        GOG_CONTENT_SYSTEM, product_id, platform
    );

    let url = Url::parse_with_params(&url, params).unwrap();
    let mut request = client.get(url);
    if let Some(token) = token {
        request = request.bearer_auth(token.access_token());
    }

    let response = request.send().await.map_err(request_error)?;
    let data = response.json().await.map_err(request_error)?;

    Ok(data)
}

/// A utility for checking for available custom EULAs
///
/// language_code of `en-US` should be used as a fallback if
/// another preferred language wasn't found
pub async fn custom_eula(
    client: &Client,
    product_id: &str,
    platform: types::Platform,
    language_code: Option<String>,
) -> Option<String> {
    let language_code = language_code.unwrap_or(String::from("en-US"));
    let custom_eula_url = format!(
        "{}/open_link/download?path=content-system/v2/eulas/custom_eula/{}/{}/eula_{}",
        GOG_CONTENT_SYSTEM, product_id, platform, language_code
    );
    let game_eula_url = format!(
        "{}/open_link/download?path=content-system/v2/eulas/{}/{}/eula_{}",
        GOG_CONTENT_SYSTEM, product_id, platform, language_code
    );
    let (custom_eula_res, game_eula_res) = tokio::join!(
        client.head(&custom_eula_url).send(),
        client.head(&game_eula_url).send()
    );

    if let Ok(custom_eula_res) = custom_eula_res {
        if custom_eula_res.status().as_u16() == 200 {
            return Some(custom_eula_url);
        }
    }

    if let Ok(game_eula_res) = game_eula_res {
        if game_eula_res.status().as_u16() == 200 {
            return Some(game_eula_url);
        }
    }

    None
}
