use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UserData {
    country: String,
    currencies: Vec<Currency>,
    selected_currency: Currency,
    preferred_language: Language,
    rating_brand: String,
    checksum: UserDataChecksums,
    updates: UserDataUpdates,
    user_id: String,
    username: String,
    galaxy_user_id: String,
    email: String,
    avatar: String,
    wishlisted_items: u32,
    friends: Vec<UserDataFriend>,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct Currency {
    code: String,
    symbol: String,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct Language {
    code: String,
    name: String,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
pub struct UserDataChecksums {
    cart: Option<String>,
    games: Option<String>,
    wishlist: Option<String>,
    reviews_votes: Option<String>,
    games_rating: Option<String>,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UserDataUpdates {
    messages: u32,
    pending_friend_requests: u32,
    unread_chat_messages: u32,
    products: u32,
    forum: u32,
    total: u32,
}

#[derive(Serialize, Deserialize, Getters, Debug)]
#[serde(rename_all = "camelCase")]
pub struct UserDataFriend {
    username: String,
    user_since: i64,
    galaxy_id: String,
    avatar: String,
}
