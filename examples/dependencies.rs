use gog_warp::{content_system::dependencies::get_manifest, Downloader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let core = gog_warp::Core::new();

    let manifest = get_manifest(core.reqwest_client().clone()).await?;
    println!("got manifest");

    let home = std::env::var("HOME").unwrap();
    let mut downloader = Downloader::builder()
        .core(core.clone())
        .install_root(format!("{}/Games/warptest/redist", home).into())
        .global_dependencies(
            manifest,
            [
                "ISI".to_owned(),
                "DirectX".to_owned(),
                "MSVC2019".to_owned(),
                "MSVC2019_x64".to_owned(),
            ]
            .to_vec(),
        )
        .build()?;

    downloader.prepare().await?;
    let required_space = downloader.get_required_space().await?;
    println!("required space {}", required_space);
    downloader.download().await?;

    Ok(())
}
