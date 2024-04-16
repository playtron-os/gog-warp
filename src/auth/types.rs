use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Token {
    access_token: String,
    refresh_token: String,
    user_id: String,
    expires_in: u32,
    token_type: String,
    scope: String,
    session_id: String,
}

impl Token {
    pub fn access_token(&self) -> &str {
        &self.access_token
    }

    pub fn refresh_token(&self) -> &str {
        &self.refresh_token
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    pub fn expires_in(&self) -> u32 {
        self.expires_in
    }
}
