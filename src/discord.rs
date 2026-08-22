use std::path::PathBuf;

use crate::client::DiscordClient;
use crate::version;

pub fn find_discord() -> Vec<DiscordClient> {
    let mut clients = Vec::new();

    let local = match std::env::var("LOCALAPPDATA") {
        Ok(value) => PathBuf::from(value),

        Err(_) => return clients,
    };

    let candidates = [
        ("Discord", "Stable"),
        ("DiscordPTB", "PTB"),
        ("DiscordCanary", "Canary"),
    ];

    for (folder, channel) in candidates {
        let path = local.join(folder);

        if !path.exists() {
            continue;
        }

        let version_path = match version::find_discord_version(&path) {
            Some(value) => value,

            None => continue,
        };

        let version = version_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .replace("app-", "");

        let executable = match folder {
            "Discord" => version_path.join("Discord.exe"),

            "DiscordPTB" => version_path.join("DiscordPTB.exe"),

            "DiscordCanary" => version_path.join("DiscordCanary.exe"),

            _ => continue,
        };

        clients.push(DiscordClient {
            name: folder.to_string(),

            channel: channel.to_string(),

            path,

            version,

            executable,
        });
    }

    clients
}
