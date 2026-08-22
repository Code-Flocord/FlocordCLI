use std::fs;
use std::path::PathBuf;

pub fn find_discord_version(discord_path: &PathBuf) -> Option<PathBuf> {
    let mut versions: Vec<PathBuf> = Vec::new();

    if let Ok(entries) = fs::read_dir(discord_path) {
        for entry in entries.flatten() {
            let path = entry.path();

            if let Some(name) = path.file_name() {
                let name = name.to_string_lossy();

                if name.starts_with("app-") {
                    versions.push(path);
                }
            }
        }
    }

    versions.sort();

    versions.pop()
}
