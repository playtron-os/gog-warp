use std::path::PathBuf;
use tokio::{fs, io::AsyncReadExt};

use crate::{
    errors::{io_error, serde_error},
    Platform,
};

use self::types::Task;

pub mod types;

pub async fn read_game_info<P>(
    game_directory: P,
    platform: Platform,
) -> Result<Vec<types::GameInfo>, crate::Error>
where
    P: Into<PathBuf>,
{
    let mut game_directory: PathBuf = game_directory.into();

    if matches!(platform, Platform::OsX) {
        game_directory.push("Contents/Resources");
    }

    let mut directory = fs::read_dir(game_directory).await.map_err(io_error)?;
    let mut infos = Vec::new();
    while let Some(entry) = directory.next_entry().await.map_err(io_error)? {
        let Ok(file_name) = entry.file_name().into_string() else {
            continue;
        };
        if file_name.starts_with("goggame-") && file_name.ends_with(".info") {
            let mut file = fs::OpenOptions::new()
                .read(true)
                .open(entry.path())
                .await
                .map_err(io_error)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents).await.map_err(io_error)?;
            let mut info: types::GameInfo = serde_json::from_str(&contents).map_err(serde_error)?;
            for task in info.play_tasks.iter_mut() {
                if let Task::File(file_task) = task {
                    if let Some(w_dir) = &file_task.working_dir {
                        if w_dir.is_empty() {
                            file_task.working_dir = None;
                        }
                    }
                }
            }
            infos.push(info);
        }
    }

    Ok(infos)
}
