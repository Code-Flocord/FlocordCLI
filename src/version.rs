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

    versions.sort_by(|a, b| {
        let parse = |p: &std::path::PathBuf| -> (u64, u64, u64) {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            let v = name.trim_start_matches("app-");
            let mut parts = v.split('.');
            let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0u64);
            let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0u64);
            let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0u64);
            (major, minor, patch)
        };
        parse(a).cmp(&parse(b))
    });

    versions.pop()
}
