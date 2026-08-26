use crate::client::DiscordClient;

use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct InstallInfo {

    pub version: String,

    pub resources: PathBuf,

    pub app_asar: PathBuf,

    pub installed: bool,

}

pub fn detect(client: &DiscordClient) -> Option<InstallInfo> {

    let base = client.executable.parent()?.parent()?;

    let entries = fs::read_dir(base).ok()?;

    let mut installs: Vec<InstallInfo> = Vec::new();

    for entry in entries {

        let entry = entry.ok()?;

        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let folder = path.file_name()?.to_string_lossy();

        if !folder.starts_with("app-") {
            continue;
        }

        let resources = path.join("resources");

        let app_asar = resources.join("app.asar");
        let original_asar = resources.join("_app.asar");

        // Accept folders where Flocord is installed (app.asar exists)
        // OR where a fresh install is possible (_app.asar exists as the original Discord)
        if !app_asar.exists() && !original_asar.exists() {
            continue;
        }

        installs.push(
            InstallInfo {

                version: folder.replace("app-", ""),

                resources,

                app_asar,

                installed: false,

            }
        );

    }

    if installs.is_empty() {

        return None;

    }

    installs.sort_by(|a, b| {
        let parse = |v: &str| -> (u64, u64, u64) {
            let mut parts = v.split('.');
            let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0u64);
            let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0u64);
            let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0u64);
            (major, minor, patch)
        };
        parse(&a.version).cmp(&parse(&b.version))
    });

    let mut install = installs.pop().unwrap();

    install.installed = detect_flocord(&install);

    Some(install)

}

fn detect_flocord(info: &InstallInfo) -> bool {
    info.resources.join("flocord.lock").exists()
}