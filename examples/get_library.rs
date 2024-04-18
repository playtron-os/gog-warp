use gog_warp::library::types::GalaxyPlatform;
use std::collections::HashMap;
use std::fs::{read, File};
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let core = gog_warp::Core::new();
    let data = read(".gog.token").expect("Failed to load token, use auth example first");
    let tokens_str = String::from_utf8(data).expect("Failed to parse Utf-8 sequence");
    // Load tokens into the core
    core.deserialize_tokens(&tokens_str)
        .expect("failed to load tokens");

    // If you don't care about data being potentially outdated
    // you can use
    // let products = core.get_owned_products().await?;
    // This endpoint has internal cache with TTL of 1h, if user just bought the game this may result in inconvenience

    // This gets games from all integrations connected to GOG Galaxy (set by client)
    // but it also includes GOG games that are automatically added/removed by the backend
    let games = core.get_galaxy_library().await?;
    println!("Got {} total games", games.len());
    let mut provider_map: HashMap<GalaxyPlatform, u32> = HashMap::new();

    for game in &games {
        // You can easily filter by game.platform_id
        let count = provider_map.get(&game.platform_id).cloned().unwrap_or(0);
        provider_map.insert(game.platform_id, count + 1);
    }

    println!("\nThis includes");
    for (key, value) in provider_map.iter() {
        println!("{}\t games from {:?}", value, key);
    }

    let data = core.serialize_tokens().unwrap();
    let mut file = File::create(".gog.token").expect("Failed to create file");
    file.write_all(data.as_bytes())
        .expect("Failed to write data");
    Ok(())
}
