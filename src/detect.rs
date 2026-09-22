use crate::client::DiscordClient;

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct InstallInfo {
    /// Version du dossier app-X.Y.Z le plus récent
    pub version: String,
    pub resources: PathBuf,
    pub app_asar: PathBuf,
    pub original_asar: PathBuf,
    pub installed: bool,
    /// Version de Flocord lue dans flocord.lock (si installé)
    pub flocord_version: Option<String>,
}

pub fn parse_version(v: &str) -> (u64, u64, u64) {
    let mut parts = v.trim_start_matches("app-").split('.');
    let mut next = || parts.next().and_then(|s| s.parse().ok()).unwrap_or(0u64);
    (next(), next(), next())
}

/// Tous les dossiers app-X.Y.Z du client, du plus ancien au plus récent
fn version_folders(client: &DiscordClient) -> Vec<PathBuf> {
    let mut folders: Vec<PathBuf> = fs::read_dir(&client.path)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir() && p.file_name().map(|n| n.to_string_lossy().starts_with("app-")).unwrap_or(false))
                .collect()
        })
        .unwrap_or_default();

    folders.sort_by_key(|p| parse_version(&p.file_name().unwrap_or_default().to_string_lossy()));
    folders
}

fn read_lock(resources: &Path) -> Option<String> {
    let text = fs::read_to_string(resources.join("flocord.lock")).ok()?;
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    // Les anciens installeurs écrivaient "Flocord installed" sans version
    Some(if text.starts_with(|c: char| c.is_ascii_digit()) { text.to_string() } else { "?".to_string() })
}

pub fn detect(client: &DiscordClient) -> Option<InstallInfo> {
    let newest = version_folders(client).pop()?;
    let resources = newest.join("resources");
    let app_asar = resources.join("app.asar");
    let original_asar = resources.join("_app.asar");

    if !app_asar.exists() && !original_asar.exists() {
        return None;
    }

    // Un app.asar fichier à côté d'un _app.asar = Flocord en place (le marqueur peut manquer si
    // l'asar a été posé par la mise à jour automatique du client)
    let flocord_version = read_lock(&resources);
    let installed = app_asar.is_file() && original_asar.exists();

    Some(InstallInfo {
        version: newest.file_name()?.to_string_lossy().replace("app-", ""),
        resources,
        app_asar,
        original_asar,
        installed,
        flocord_version,
    })
}

/// Version d'un ancien dossier app-X.Y.Z où Flocord était installé (Discord s'est mis à jour depuis)
pub fn previous_install(client: &DiscordClient) -> Option<String> {
    let mut folders = version_folders(client);
    folders.pop(); // le plus récent est celui qu'on inspecte déjà

    folders
        .into_iter()
        .rev()
        .find(|folder| folder.join("resources").join("flocord.lock").exists())
        .map(|folder| folder.file_name().unwrap_or_default().to_string_lossy().replace("app-", ""))
}
