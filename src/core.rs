use crate::auth::types::Token;
use crate::{auth, constants, errors};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Entry point
#[derive(Clone)]
pub struct Core {
    tokens: Arc<Mutex<HashMap<String, auth::types::Token>>>,
    reqwest_client: reqwest::Client,
}

impl Core {
    pub fn new() -> Self {
        let client = reqwest::Client::default();
        Self {
            tokens: Arc::new(Mutex::new(HashMap::new())),
            reqwest_client: client,
        }
    }

    pub fn serialize_tokens(&mut self, new_tokens: &str) -> errors::EmptyResult {
        let new_tokens: HashMap<String, auth::types::Token> =
            serde_json::from_str(new_tokens).map_err(|err| errors::json_error(err))?;
        let mut tokens = self.tokens.lock();
        tokens.clear();
        tokens.extend(new_tokens);
        Ok(())
    }

    pub fn deserialize_tokens(&self) -> Result<String, errors::Error> {
        let tokens = self.tokens.lock();
        serde_json::to_string(&*tokens).map_err(|err| errors::json_error(err))
    }

    fn get_galaxy_token(&self) -> Option<Token> {
        self.tokens.lock().get(constants::GALAXY_CLIENT_ID).cloned()
    }

    pub fn ensure_auth(&self) -> errors::EmptyResult {
        if self.get_galaxy_token().is_none() {
            return Err(errors::not_logged_in_error());
        }
        Ok(())
    }

    /// Finishes the auth flow, obtaining the token for Galaxy `CLIENT_ID`
    pub async fn get_token_with_code(&mut self, code: String) -> errors::EmptyResult {
        let token = auth::get_token_with_code(&self.reqwest_client, &code).await?;
        let mut tokens = self.tokens.lock();
        tokens.clear();
        tokens.insert(constants::GALAXY_CLIENT_ID.to_string(), token);
        Ok(())
    }

    /// Get owned products list, includes bundles and DLCs
    ///
    /// Requires authentication
    pub async fn get_owned_products(&self) -> Result<Vec<u64>, errors::Error> {
        self.ensure_auth()?;
        let token = self.get_galaxy_token().unwrap();
        todo!()
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
        let mut core = Core::new();
        core.serialize_tokens(tokens)
            .expect("Failed to serialize test token");
        core.ensure_auth().expect("Failed to ensure authentication");
        let deserialized = core
            .deserialize_tokens()
            .expect("Failed to deserialize tokens");
        assert!(deserialized.starts_with(r#"{"46899977096215655":{"#));
    }
}
