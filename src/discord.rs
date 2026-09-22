use std::path::PathBuf;

use crate::client::DiscordClient;
use crate::version;

/// Les quatre canaux Discord installables sous %LOCALAPPDATA% : (dossier, nom du canal)
const CANDIDATES: [(&str, &str); 4] = [
    ("Discord", "Stable"),
    ("DiscordPTB", "PTB"),
    ("DiscordCanary", "Canary"),
    ("DiscordDevelopment", "Development"),
];

pub fn find_discord() -> Vec<DiscordClient> {
    let mut clients = Vec::new();

    let Ok(local) = std::env::var("LOCALAPPDATA") else {
        return clients;
    };
    let local = PathBuf::from(local);

    for (folder, channel) in CANDIDATES {
        let path = local.join(folder);
        if !path.exists() {
            continue;
        }

        let Some(version_path) = version::find_discord_version(&path) else {
            continue;
        };

        let version = version_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .replace("app-", "");

        // L'exécutable porte le nom du dossier : Discord.exe, DiscordPTB.exe, DiscordCanary.exe...
        let executable = version_path.join(format!("{}.exe", folder));
        if !executable.exists() {
            continue;
        }

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
