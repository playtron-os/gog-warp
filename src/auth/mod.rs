use crate::auth::types::Token;
use crate::errors::{json_error, request_error};
use crate::Error;
use reqwest::{Client, Url};

// Utilities for authorization
pub mod types;

pub(crate) async fn get_token_with_code(client: &Client, code: &str) -> Result<Token, Error> {
    let url = Url::parse_with_params("https://auth.gog.com/token?client_id=46899977096215655&client_secret=9d85c43b1482497dbbce61f6e4aa173a433796eeae2ca8c5f6129f2dc4de46d9&grant_type=authorization_code&redirect_uri=https://embed.gog.com/on_login_success?origin=client", [("code", code)]).unwrap();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| request_error(err))?;
    let response = response
        .error_for_status()
        .map_err(|err| request_error(err))?;
    let new_token: Token = response.json().await.map_err(|err| json_error(err))?;
    Ok(new_token)
}

pub(crate) async fn get_token_for(client_id: &str, client_secret: &str, token: &types::Token) {
    todo!()
}
