use gog_warp::content_system::downloader::progress::DownloadState;
use gog_warp::{Downloader, Platform};
use indicatif::ProgressStyle;
use std::env;
use std::fs::read;
use tokio::io::AsyncWriteExt;

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

    // Pick a build, unset branch == master
    let mut builds = builds.items().iter().filter(|b| b.branch().is_none());
    let latest = builds.next().unwrap();
    println!("Picked latest build {}", latest.build_id());

    // Obtain the manifest, you should store it for later along with the build_id
    let latest_manifest = core.get_manifest(&latest).await?;
    println!("Got manifest");

    // Providing dependencies manifest enables gog-warp to obtain game scoped dependencies
    // like DOSBOX, ScummVM, language_setup and similar utilities
    let dependencies_manifest = core.get_dependencies_manifest().await?;

    let home = env::var("HOME").unwrap();

    let mut downloader = Downloader::builder()
        .core(core.clone())
        .language("en-US".to_string())
        .install_root(format!("{}/Games/warptest", home).into())
        // Support directory often contains installer files or default config files
        // The subdirectory named after game id will be created
        .support_root(format!("{}/Games/warptest/support", home).into())
        .manifest(latest_manifest, latest.build_id())
        .game_dependencies(dependencies_manifest)
        .build()?;
    println!("Built downloader");

    // Obtain file lists and calculate diffs
    downloader.prepare().await?;
    println!("Download prepared");

    // Check for pre-existing files and return how much additional space is required
    // Here you should check if you have enough free disk space
    let required_space = downloader.get_required_space().await?;
    println!(
        "This operation requires {} additional disk space",
        required_space
    );
    // If you want to listen for progress details, use this async channel
    let mut reciever = downloader.take_progress_reciever().unwrap();

    let progress = indicatif::ProgressBar::new(0);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] {bar:50.cyan/blue} {binary_bytes}/{binary_total_bytes}")
            .unwrap(),
    );

    progress.enable_steady_tick(std::time::Duration::from_secs(1));

    let token = downloader.get_cancellation();
    let task = tokio::spawn(async move { downloader.download().await });

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen to ctrlc event");
        token.cancel();
    });

    while let Some(message) = reciever.recv().await {
        if let DownloadState::Downloading(state) = message {
            progress.set_length(state.total_size);
            progress.set_position(state.written);
        }
    }
    progress.finish_and_clear();
    if let Err(err) = task.await? {
        println!("Error in downloader: {}", err);
    }

    let data = core.serialize_tokens().unwrap();
    let mut file = tokio::fs::File::create(".gog.token").await?;
    file.write_all(data.as_bytes()).await?;

    Ok(())
}
