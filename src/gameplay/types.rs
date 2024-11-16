use crate::content_system::languages;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GameInfo {
    pub client_id: Option<String>,
    pub game_id: String,
    pub language: String,
    #[serde(deserialize_with = "languages::serde_language", default)]
    pub languages: Vec<String>,
    pub name: String,
    pub play_tasks: Vec<Task>,
    pub root_game_id: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum Task {
    #[serde(rename = "FileTask")]
    File(FileTask),
    #[serde(rename = "URLTask")]
    Url(UrlTask),
}

#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskCategory {
    Launcher,
    Game,
    Document,
    Tool,
    #[default]
    #[serde(other)]
    Other,
}

#[derive(Deserialize, Debug)]
pub struct UrlTask {
    #[serde(default)]
    pub category: TaskCategory,
    #[serde(deserialize_with = "languages::serde_language", default)]
    pub languages: Vec<String>,
    pub name: Option<String>,

    pub link: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FileTask {
    #[serde(default)]
    pub category: TaskCategory,
    #[serde(deserialize_with = "languages::serde_language", default)]
    pub languages: Vec<String>,
    pub name: Option<String>,

    #[serde(default)]
    pub is_primary: bool,
    pub path: String,
    pub working_dir: Option<String>,
    pub arguments: Option<String>,
    pub compatibility_flags: Option<String>,
}
