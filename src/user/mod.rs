pub mod types;

use crate::auth::types::Token;
use crate::constants::domains::GOG_EMBED;
use crate::errors::request_error;
use crate::utils::reqwest_exponential_backoff;
use crate::Error;
use reqwest::Client;

pub(crate) async fn get_user_data(client: &Client, token: Token) -> Result<types::UserData, Error> {
    let url = format!("{}/userData.json", GOG_EMBED);
    let response = reqwest_exponential_backoff(client.get(url).bearer_auth(token.access_token()))
        .await
        .map_err(request_error)?;
    let data = response.json().await.map_err(request_error)?;
    Ok(data)
}
