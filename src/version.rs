use std::fs;
use std::path::PathBuf;

use crate::detect::parse_version;

pub fn find_discord_version(discord_path: &PathBuf) -> Option<PathBuf> {
    let mut versions: Vec<PathBuf> = fs::read_dir(discord_path)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.file_name().map(|n| n.to_string_lossy().starts_with("app-")).unwrap_or(false))
                .collect()
        })
        .unwrap_or_default();

    versions.sort_by_key(|p| parse_version(&p.file_name().unwrap_or_default().to_string_lossy()));
    versions.pop()
}
