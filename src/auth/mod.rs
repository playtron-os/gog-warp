use crate::auth::types::Token;
use crate::constants::GALAXY_CLIENT_ID;
use crate::errors::{invalid_session_error, request_error, unauthorized_error};
use crate::utils::reqwest_exponential_backoff;
use crate::Error;
use reqwest::{Client, Url};

// Utilities for authorization
pub mod types;

pub(crate) async fn get_token_with_code(client: &Client, code: &str) -> Result<Token, Error> {
    let url = Url::parse_with_params("https://auth.gog.com/token?client_id=46899977096215655&client_secret=9d85c43b1482497dbbce61f6e4aa173a433796eeae2ca8c5f6129f2dc4de46d9&grant_type=authorization_code&redirect_uri=https://embed.gog.com/on_login_success?origin=client", [("code", code)]).unwrap();
    let response = reqwest_exponential_backoff(client.get(url))
        .await
        .map_err(request_error)?;
    let response = response.error_for_status().map_err(request_error)?;
    let new_token: Token = response.json().await.map_err(request_error)?;
    Ok(new_token)
}

pub(crate) async fn get_token_for(
    client: &Client,
    client_id: &str,
    client_secret: &str,
    galaxy_token: Token,
) -> Result<Token, Error> {
    let mut params = vec![
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("refresh_token", galaxy_token.refresh_token()),
    ];
    if client_id != GALAXY_CLIENT_ID {
        params.push(("without_new_session", "1"));
    }
    let url = Url::parse_with_params(
        "https://auth.gog.com/token?grant_type=refresh_token",
        params,
    )
    .unwrap();

    let response = reqwest_exponential_backoff(client.get(url))
        .await
        .map_err(request_error)?;
    if response.status().as_u16() == 401 {
        return Err(unauthorized_error());
    }
    if response.status().as_u16() == 400 {
        return Err(invalid_session_error());
    }
    let response = response.error_for_status().map_err(request_error)?;
    let new_token = response.json().await.map_err(request_error)?;
    Ok(new_token)
}
