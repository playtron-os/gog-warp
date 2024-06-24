use chrono::prelude::*;
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Getters, Debug)]
pub struct Token {
    access_token: String,
    refresh_token: String,
    user_id: String,
    expires_in: u32,
    token_type: String,
    scope: Option<String>,
    session_id: String,
    #[serde(default = "Utc::now", with = "chrono::serde::ts_seconds")]
    login_time: DateTime<Utc>,
}
