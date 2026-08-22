use std::path::PathBuf;

#[derive(Clone)]
pub struct DiscordClient {
    pub name: String,

    pub channel: String,

    pub path: PathBuf,

    pub version: String,

    pub executable: PathBuf,
}
