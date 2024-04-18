use crate::auth::get_token_for;
use crate::auth::types::Token;
use crate::constants::{GALAXY_CLIENT_ID, GALAXY_CLIENT_SECRET};
use crate::library::types::GalaxyLibraryItem;
use crate::{auth, errors, user};
use chrono::Utc;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use crate::user::types::UserData;

/// Entry point
#[derive(Clone)]
pub struct Core {
    tokens: Arc<Mutex<HashMap<String, Token>>>,
    reqwest_client: reqwest::Client,
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

impl Core {
    pub fn new() -> Self {
        let client = reqwest::Client::default();
        Self {
            tokens: Arc::new(Mutex::new(HashMap::new())),
            reqwest_client: client,
        }
    }

    pub fn deserialize_tokens(&self, new_tokens: &str) -> errors::EmptyResult {
        let new_tokens: HashMap<String, Token> =
            serde_json::from_str(new_tokens).map_err(errors::json_error)?;
        let mut tokens = self.tokens.lock();
        tokens.clear();
        tokens.extend(new_tokens);
        Ok(())
    }

    pub fn serialize_tokens(&self) -> Result<String, errors::Error> {
        let tokens = self.tokens.lock();
        serde_json::to_string(&*tokens).map_err(errors::json_error)
    }

    fn get_token(&self, client_id: &str) -> Option<Token> {
        self.tokens.lock().get(client_id).cloned()
    }

    /// Returns error if
    pub fn ensure_auth(&self) -> errors::EmptyResult {
        if self.get_token(GALAXY_CLIENT_ID).is_none() {
            return Err(errors::not_logged_in_error());
        }
        Ok(())
    }

    /// Get token for specified client_id and secret
    pub async fn obtain_token(
        &self,
        client_id: &str,
        client_secret: &str,
    ) -> Result<Token, errors::Error> {
        self.ensure_auth()?;
        match self.get_token(client_id) {
            Some(token) => {
                log::debug!(
                    "Found token for {}: {}",
                    client_id,
                    &token.access_token()[..4]
                );
                // Re-use existing token
                let expires_in: i64 = (*token.expires_in()).into();
                let current_time = Utc::now().timestamp();
                let login_time = token.login_time().timestamp();
                if login_time + expires_in < current_time {
                    log::debug!("Refreshing token for client {}", client_id);
                    let new_token =
                        get_token_for(&self.reqwest_client, client_id, client_secret, token)
                            .await?;

                    self.tokens
                        .lock()
                        .insert(GALAXY_CLIENT_ID.to_string(), new_token.clone());
                    return Ok(new_token);
                }

                Ok(token)
            }
            None => {
                log::debug!("Getting new token for client {}", client_id);
                // Get new token
                let galaxy_token = self.get_token(GALAXY_CLIENT_ID).unwrap();
                let new_token =
                    get_token_for(&self.reqwest_client, client_id, client_secret, galaxy_token)
                        .await?;

                self.tokens
                    .lock()
                    .insert(GALAXY_CLIENT_ID.to_string(), new_token.clone());
                Ok(new_token)
            }
        }
    }

    /// Refreshes the main token when needed and returns it
    /// This is basically Core::obtain_token with Galaxy credentials
    pub async fn obtain_galaxy_token(&self) -> Result<Token, errors::Error> {
        self.obtain_token(GALAXY_CLIENT_ID, GALAXY_CLIENT_SECRET)
            .await
    }

    /// Finishes the auth flow, obtaining the token for Galaxy `CLIENT_ID`
    /// Previously stored tokens will be cleared
    pub async fn get_token_with_code(&self, code: String) -> errors::EmptyResult {
        log::debug!("Requesting token with code {}", &code[..4]);
        let token = auth::get_token_with_code(&self.reqwest_client, &code).await?;
        let mut tokens = self.tokens.lock();
        tokens.clear();
        tokens.insert(GALAXY_CLIENT_ID.to_string(), token);
        Ok(())
    }

    /// Get owned products list, includes bundles and DLCs
    /// Don't use this function when reacting to galaxy-library event
    /// Requires authentication
    pub async fn get_owned_products(&self) -> Result<Vec<u64>, errors::Error> {
        self.ensure_auth()?;
        let token = self.obtain_galaxy_token().await?;
        crate::library::get_owned_licenses(&self.reqwest_client, token).await
    }

    /// List of games from all integrations linked to GOG Galaxy
    /// Recommended way to get games after receiving galaxy-library event
    /// Requires authentication
    pub async fn get_galaxy_library(&self) -> Result<Vec<GalaxyLibraryItem>, errors::Error> {
        self.ensure_auth()?;
        let token = self.obtain_galaxy_token().await?;
        log::debug!("Getting galaxy library");
        crate::library::get_galaxy_library(&self.reqwest_client, token).await
    }

    /// Get user and friend information
    /// Requires authentication
    pub async fn get_user_data(&self) -> Result<UserData, errors::Error> {
        self.ensure_auth()?;
        let token = self.obtain_galaxy_token().await?;
        user::get_user_data(&self.reqwest_client, token).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_auth_panic() {
        let core = Core::new();
        core.ensure_auth()
            .expect_err("Galaxy client_id token found");
    }

    #[test]
    fn auth_success() {
        let tokens = r#" {"46899977096215655": {"access_token": "123", "refresh_token": "1234", "user_id": "321", "expires_in": 3600, "token_type": "bearer", "scope": "", "session_id": "567"}} "#;
        let core = Core::new();
        core.deserialize_tokens(tokens)
            .expect("Failed to serialize test token");
        core.ensure_auth().expect("Failed to ensure authentication");
        let deserialized = core
            .serialize_tokens()
            .expect("Failed to deserialize tokens");
        assert!(deserialized.starts_with(r#"{"46899977096215655":{"#));
    }

    #[test]
    fn auth_error() {
        let core = Core::new();
        core.deserialize_tokens(r#"{"123":{"access_token": "123", "refresh_token": "1234", "user_id": "321", "expires_in": 3600, "token_type": "bearer", "scope": "", "session_id": "567"}}"#).unwrap();
        core.ensure_auth().expect_err("Expected not logged result");
    }
}
