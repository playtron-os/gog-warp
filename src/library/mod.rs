pub mod types;

use crate::auth::types::Token;
use crate::constants::domains::*;
use crate::errors::request_error;
use crate::library::types::{GalaxyLibraryItem, OwnedProductsResponse};
use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::{Client, Url};

pub(crate) async fn get_owned_licenses(
    client: &Client,
    token: Token,
) -> Result<Vec<u64>, crate::Error> {
    log::debug!("Getting owned licenses");
    let url = format!("{}/user/data/games", GOG_EMBED);
    let mut auth_header =
        HeaderValue::from_str(&format!("Bearer {}", token.access_token())).unwrap();
    auth_header.set_sensitive(true);
    let response = client
        .get(url)
        .header(AUTHORIZATION, auth_header)
        .send()
        .await
        .map_err(request_error)?;
    let list: OwnedProductsResponse = response.json().await.map_err(request_error)?;
    Ok(list.owned)
}

async fn get_galaxy_library_page(
    client: &Client,
    token: &Token,
    page_token: &Option<String>,
) -> Result<types::GalaxyLibraryResponse, crate::Error> {
    let url = format!("{}/users/{}/releases", GOG_GALAXY_LIBRARY, token.user_id());
    let mut url = Url::parse(&url).unwrap();
    log::debug!("Getting galaxy library page with token: {:?}", page_token);
    if let Some(next_page_token) = page_token {
        url.query_pairs_mut()
            .append_pair("page_token", next_page_token);
    }
    let mut auth_header =
        HeaderValue::from_str(&format!("Bearer {}", token.access_token())).unwrap();
    auth_header.set_sensitive(true);

    let response = client
        .get(url)
        .header(AUTHORIZATION, auth_header)
        .send()
        .await
        .map_err(request_error)?;
    let response = response.error_for_status().map_err(request_error)?;
    let data = response.json().await.map_err(request_error)?;
    Ok(data)
}

pub(crate) async fn get_galaxy_library(
    client: &Client,
    token: Token,
) -> Result<Vec<GalaxyLibraryItem>, crate::Error> {
    let mut library = Vec::new();
    let mut page_token: Option<String> = None;
    loop {
        let page = get_galaxy_library_page(client, &token, &page_token).await?;
        library.extend(page.items);
        if page.next_page_token.is_some() {
            page_token = page.next_page_token;
            continue;
        }
        break;
    }
    Ok(library)
}
