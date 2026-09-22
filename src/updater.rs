// Récupère la dernière version de Flocord (version.json sur GitHub) et télécharge l'asar si l'embarqué est dépassé.

use std::fs;
use std::io::Read;

use crate::say;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

const VERSION_URL: &str =
    "https://raw.githubusercontent.com/Code-Flocord/FlocordCLI/master/version.json";

static EMBEDDED_MANIFEST: &str = include_str!("../version.json");

#[derive(Deserialize, Clone)]
pub struct VersionManifest {
    /// Version de Flocord (l'asar)
    pub version: String,
    pub url: String,
    /// Version de l'installeur, quand elle diffère de celle de Flocord
    #[serde(default)]
    pub cli: Option<String>,
}

/// Version de l'installeur lui-même (Cargo.toml)
pub fn cli_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub struct Payload {
    pub bytes: Vec<u8>,
    pub version: String,
}

pub fn embedded_version() -> String {
    serde_json::from_str::<VersionManifest>(EMBEDDED_MANIFEST)
        .map(|m| m.version)
        .unwrap_or_else(|_| "0.0.0".to_string())
}

fn cache_path() -> PathBuf {
    crate::registry::data_dir().join("desktop.asar")
}

pub fn version_gt(a: &str, b: &str) -> bool {
    crate::detect::parse_version(a) > crate::detect::parse_version(b)
}

pub fn http(timeout: u64) -> Option<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout))
        .user_agent(format!("FlocordCLI/{}", embedded_version()))
        .build()
        .ok()
}

/// Dernier manifest publié, ou None hors ligne
pub fn latest_manifest() -> Option<VersionManifest> {
    let text = http(6)?.get(VERSION_URL).send().ok()?.text().ok()?;
    serde_json::from_str(&text).ok()
}

/// Télécharge un fichier en affichant une barre de progression
pub fn download(url: &str, label: &str) -> Option<Vec<u8>> {
    let client = http(60)?;
    let mut response = client.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }

    let total = response.content_length().unwrap_or(0);
    let mut bytes: Vec<u8> = Vec::with_capacity(total as usize);
    let mut buffer = [0u8; 64 * 1024];
    let mut last_percent = 101;

    loop {
        let read = response.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);

        if total > 0 {
            let percent = (bytes.len() as u64 * 100 / total) as usize;
            if percent != last_percent {
                last_percent = percent;
                say::progress(
                    percent as f32 / 100.0,
                    format!("{}  {:.1} / {:.1} Mo", label, bytes.len() as f64 / 1_048_576.0, total as f64 / 1_048_576.0)
                );
            }
        }
    }

    say::progress_done();
    Some(bytes)
}

/// L'asar à installer : la dernière version en ligne si elle est plus récente que l'embarquée, sinon l'embarquée.
pub fn check_and_update(embedded: &[u8]) -> Payload {
    let embedded_version = embedded_version();
    let fallback = || Payload { bytes: embedded.to_vec(), version: embedded_version.clone() };

    say!("Vérification des mises à jour Flocord...");

    let Some(manifest) = latest_manifest() else {
        say!("Hors ligne : version embarquée (v{}) utilisée.", embedded_version);
        return fallback();
    };

    if !version_gt(&manifest.version, &embedded_version) {
        say!("Flocord v{} à jour.", embedded_version);
        return fallback();
    }

    say!("Mise à jour disponible : v{} → v{}", embedded_version, manifest.version);

    let cache = cache_path();
    let cache_version = cache.with_extension("version");
    if fs::read_to_string(&cache_version).map(|v| v.trim() == manifest.version).unwrap_or(false) {
        if let Ok(bytes) = fs::read(&cache) {
            say!("✔ v{} déjà téléchargée.", manifest.version);
            return Payload { bytes, version: manifest.version };
        }
    }

    let Some(bytes) = download(&manifest.url, "Téléchargement") else {
        say!("⚠ Téléchargement impossible, version embarquée utilisée.");
        return fallback();
    };

    let _ = fs::create_dir_all(crate::registry::data_dir());
    let _ = fs::write(&cache, &bytes);
    let _ = fs::write(&cache_version, &manifest.version);

    say!("✔ Flocord v{} prêt.", manifest.version);
    Payload { bytes, version: manifest.version }
}
