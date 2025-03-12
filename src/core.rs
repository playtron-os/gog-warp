use crate::auth::get_token_for;
use crate::auth::types::Token;
use crate::constants::{GALAXY_CLIENT_ID, GALAXY_CLIENT_SECRET};
use crate::content_system::dependencies::{self, DependenciesManifest};
use crate::content_system::types::{Build, BuildResponse, Manifest, Platform};
use crate::errors::{maximum_retries_error, serde_error, zlib_error};
use crate::library::types::GalaxyLibraryItem;
use crate::user::types::UserData;
use crate::utils::reqwest_exponential_backoff;
use crate::{auth, content_system, errors, user};
use chrono::Utc;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::AsyncReadExt;

#[derive(Clone)]
pub enum CoreEvent {
    TokenRefreshed((String, String)),
}

/// Library entry point  
/// It's job is to manage authentication and provide nice wrapper arround available endpoints
pub struct Core {
    tokens: Arc<Mutex<HashMap<String, Token>>>,
    reqwest_client: reqwest::Client,
    tx: tokio::sync::broadcast::Sender<CoreEvent>,
    _rx: tokio::sync::broadcast::Receiver<CoreEvent>,
}

impl Default for Core {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Core {
    fn clone(&self) -> Self {
        Core {
            tokens: self.tokens.clone(),
            reqwest_client: self.reqwest_client.clone(),
            tx: self.tx.clone(),
            _rx: self.tx.subscribe(),
        }
    }
}

impl Core {
    pub fn new() -> Self {
        let client = reqwest::Client::builder().no_gzip().build().unwrap();
        let (tx, rx) = tokio::sync::broadcast::channel::<CoreEvent>(128);

        Self {
            tokens: Arc::new(Mutex::new(HashMap::new())),
            reqwest_client: client,
            tx,
            _rx: rx,
        }
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<CoreEvent> {
        self.tx.subscribe()
    }

    pub fn reqwest_client(&self) -> &reqwest::Client {
        &self.reqwest_client
    }

    pub fn deserialize_tokens(&self, new_tokens: &str) -> errors::EmptyResult {
        let new_tokens: HashMap<String, Token> =
            serde_json::from_str(new_tokens).map_err(errors::serde_error)?;
        let mut tokens = self.tokens.lock();
        tokens.clear();
        tokens.extend(new_tokens);
        Ok(())
    }

    pub fn serialize_tokens(&self) -> Result<String, errors::Error> {
        let tokens = self.tokens.lock();
        serde_json::to_string(&*tokens).map_err(errors::serde_error)
    }

    fn get_token(&self, client_id: &str) -> Option<Token> {
        self.tokens.lock().get(client_id).cloned()
    }

    /// Returns error if auth galaxy token isn't in store
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
        let galaxy_token = self.get_token(GALAXY_CLIENT_ID).unwrap();
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
                        get_token_for(&self.reqwest_client, client_id, client_secret, galaxy_token)
                            .await?;

                    self.tokens
                        .lock()
                        .insert(client_id.to_string(), new_token.clone());

                    _ = self.tx.send(CoreEvent::TokenRefreshed((
                        new_token.access_token().clone(),
                        new_token.refresh_token().clone(),
                    )));

                    return Ok(new_token);
                }

                Ok(token)
            }
            None => {
                log::debug!("Getting new token for client {}", client_id);
                // Get new token
                let new_token =
                    get_token_for(&self.reqwest_client, client_id, client_secret, galaxy_token)
                        .await?;

                self.tokens
                    .lock()
                    .insert(client_id.to_string(), new_token.clone());

                _ = self.tx.send(CoreEvent::TokenRefreshed((
                    new_token.access_token().clone(),
                    new_token.refresh_token().clone(),
                )));

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

    /// Finishes the auth flow, obtaining the token for [`GALAXY_CLIENT_ID`]  
    /// Previously stored tokens will be cleared
    pub async fn get_token_with_code(&self, code: String) -> errors::EmptyResult {
        log::debug!("Requesting token with code {}", code);
        let token = auth::get_token_with_code(&self.reqwest_client, &code).await?;

        {
            let mut tokens = self.tokens.lock();
            tokens.clear();
            tokens.insert(GALAXY_CLIENT_ID.to_string(), token.clone());
        }

        _ = self.tx.send(CoreEvent::TokenRefreshed((
            token.access_token().clone(),
            token.refresh_token().clone(),
        )));

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

    /// List of games and DLCs from all integrations linked to GOG Galaxy  
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

    /// Get available builds from content-system  
    /// Authorization for this call is optional  
    ///
    /// * `password`- allows private branches to be accessed
    ///
    /// If the [`BuildResponse::total_count`] is 0, the OS is not supported by the game
    pub async fn get_builds(
        &self,
        product_id: &str,
        platform: Platform,
        password: Option<String>,
    ) -> Result<BuildResponse, errors::Error> {
        let token: Option<Token> = match self.ensure_auth() {
            Ok(_) => {
                let token = self.obtain_galaxy_token().await?;
                Some(token)
            }
            Err(_) => None,
        };

        content_system::get_builds(&self.reqwest_client, product_id, platform, token, password)
            .await
    }

    /// Get manifest for the build obtained with [`Core::get_builds`]
    pub async fn get_manifest(&self, build: &Build) -> Result<Manifest, errors::Error> {
        for endpoint in build.urls() {
            let response =
                reqwest_exponential_backoff(self.reqwest_client.get(endpoint.url())).await;
            if let Ok(res) = response {
                if res.status().as_u16() != 200 {
                    continue;
                }
                if let Ok(data) = res.bytes().await {
                    if *build.generation() == 1 {
                        let manifest: Manifest =
                            serde_json::from_slice(&data).map_err(serde_error)?;
                        return Ok(manifest);
                    }
                    let mut zlib = async_compression::tokio::bufread::ZlibDecoder::new(&data[..]);
                    let mut buffer = Vec::new();
                    zlib.read_to_end(&mut buffer).await.map_err(zlib_error)?;
                    let manifest: Manifest =
                        serde_json::from_slice(buffer.as_slice()).map_err(serde_error)?;
                    return Ok(manifest);
                }
            }
        }
        Err(maximum_retries_error())
    }

    /// Get dependencies manifest
    pub async fn get_dependencies_manifest(&self) -> Result<DependenciesManifest, errors::Error> {
        dependencies::get_manifest(self.reqwest_client.clone()).await
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
