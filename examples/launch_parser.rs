use std::fs::read;

#[tokio::main]
async fn main() {
    let core = gog_warp::Core::new();
    let data = read(".gog.token").expect("Failed to load token, use auth example first");
    let tokens_str = String::from_utf8(data).expect("Failed to parse Utf-8 sequence");
    // Load tokens into the core
    core.deserialize_tokens(&tokens_str)
        .expect("failed to load tokens");

    let info = gog_warp::gameplay::read_game_info(
        std::env::args()
            .nth(1)
            .expect("Expected argument to game directory"),
        gog_warp::Platform::Windows,
    )
    .await
    .expect("Failed to parse");

    println!("{info:#?}");
}
