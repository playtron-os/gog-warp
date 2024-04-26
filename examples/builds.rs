use gog_warp::content_system::types::{BuildResponse, Platform};

fn list_builds(name: &str, builds: &BuildResponse) {
    println!("Listing available builds for {}", name);
    for build in builds.items() {
        println!(
            "version: {}; branch: {:?}; date: {}, public: {}",
            build.version_name(),
            build.branch(),
            build.date_published().format("%Y-%m-%d"),
            build.public()
        );
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Init core
    let core = gog_warp::Core::new();

    let witcher_builds = core
        .get_builds("1207658924", Platform::Windows, None)
        .await?;

    list_builds("The Witcher", &witcher_builds);
    let build = witcher_builds
        .items()
        .iter()
        .find(|b| *b.generation() == 1)
        .expect("No v1 builds");
    let manifest = core
        .get_manifest(&build)
        .await
        .expect("Failed to get manifest");
    println!("Install directory: {}", manifest.install_directory());
    println!("Languages: {:?}", manifest.languages());

    println!("");

    let cyberpunk_builds = core
        .get_builds("1423049311", Platform::Windows, None)
        .await?;

    list_builds("Cyberpunk 2077", &cyberpunk_builds);

    // Let's pick a build
    // Latest one from production branch
    let build = cyberpunk_builds
        .items()
        .iter()
        .find(|item| item.branch().is_none())
        .expect("Build not found");

    let manifest = core
        .get_manifest(&build)
        .await
        .expect("Failed to get manifest");

    let install_dir = manifest.install_directory();
    println!("Install Dir: {}", install_dir);

    let languages = manifest.languages();
    println!("Languages: {:?}", languages);

    Ok(())
}
