use gog_warp::{Downloader, Platform};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Init core
    let core = gog_warp::Core::new();
    println!("Created warp instance");

    let builds = core
        .get_builds("2034949552", Platform::Windows, None)
        .await?;
    println!("Got builds");

    let mut builds = builds.items().iter().filter(|b| b.branch().is_none());
    let latest = builds.next().unwrap();
    println!("Picked latest build {}", latest.build_id());

    let latest_manifest = core.get_manifest(&latest).await?;
    println!("Got manifest");

    let home = env::var("HOME").unwrap();

    let mut downloader = Downloader::builder()
        .core(core)
        .language("en-US".to_string())
        .install_root(format!("{}/Games/warptest", home).into())
        .manifest(latest_manifest, latest.build_id())
        .build()?;
    println!("Built downloader");

    downloader.prepare().await?;
    println!("Download prepared");

    downloader.download().await?;

    Ok(())
}
