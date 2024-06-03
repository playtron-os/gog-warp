use gog_warp::{Downloader, Platform};
use std::env;
use std::fs::read;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Init core
    let core = gog_warp::Core::new();
    println!("Created warp instance");
    let data = read(".gog.token").expect("Failed to load token, use auth example first");
    let tokens_str = String::from_utf8(data).expect("Failed to parse Utf-8 sequence");
    // Load tokens into the core
    core.deserialize_tokens(&tokens_str)
        .expect("failed to load tokens");

    let builds = core
        .get_builds("1207659234", Platform::Windows, None)
        .await?;
    println!("Got builds");

    let mut builds = builds.items().iter().filter(|b| b.branch().is_none());
    let latest = builds.next().unwrap();
    println!("Picked latest build {}", latest.build_id());

    let latest_manifest = core.get_manifest(&latest).await?;
    println!("Got manifest");

    let dependencies_manifest = core.get_dependencies_manifest().await?;

    let home = env::var("HOME").unwrap();

    let mut downloader = Downloader::builder()
        .core(core)
        .language("en-US".to_string())
        .install_root(format!("{}/Games/warptest", home).into())
        .support_root(format!("{}/Games/warptest/support", home).into())
        .manifest(latest_manifest, latest.build_id())
        .game_dependencies(dependencies_manifest)
        .build()?;
    println!("Built downloader");

    downloader.prepare().await?;
    println!("Download prepared");

    let required_space = downloader.get_requied_space().await?;
    println!(
        "This operation requires {} additional disk space",
        required_space
    );
    let token = downloader.get_cancellation();
    let task = tokio::spawn(async move { downloader.download().await });

    task.await??;

    Ok(())
}
