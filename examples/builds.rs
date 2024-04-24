use gog_warp::content_system::types::{BuildResponse, Manifest, Platform};

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

    let bg3_builds = core
        .get_builds("1456460669", Platform::Windows, None)
        .await?;

    list_builds("Baldur's Gate 3", &bg3_builds);
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

    match manifest {
        Manifest::V1(manifest) => {
            let install_directory = manifest.product().install_directory();
            println!("Install directory {}", install_directory);
        }
        Manifest::V2(manifest) => {
            let install_directory = manifest.install_directory();
            println!("Install directory {}", install_directory);
        }
    }

    Ok(())
}
