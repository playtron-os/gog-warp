use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::{Client, Url};

use crate::auth::types::Token;
use crate::constants::domains::GOG_CONTENT_SYSTEM;
use crate::errors::request_error;

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
        let mut header =
            HeaderValue::from_str(&format!("Bearer {}", token.access_token())).unwrap();
        header.set_sensitive(true);
        request = request.header(AUTHORIZATION, header);
    }

    let response = request.send().await.map_err(request_error)?;
    let data = response.json().await.map_err(request_error)?;

    Ok(data)
}
