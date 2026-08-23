use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

const VERSION_URL: &str =
    "https://raw.githubusercontent.com/Code-Flocord/FlocordCLI/master/version.json";

pub const EMBEDDED_VERSION: &str = "1.0.3";

#[derive(Deserialize)]
struct VersionManifest {
    version: String,
    url: String,
}

fn cache_path() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(local).join("Flocord").join("desktop.asar")
}

fn version_gt(a: &str, b: &str) -> bool {
    let parse = |v: &str| -> (u32, u32, u32) {
        let p: Vec<u32> = v.split('.').map(|x| x.parse().unwrap_or(0)).collect();
        (
            p.first().copied().unwrap_or(0),
            p.get(1).copied().unwrap_or(0),
            p.get(2).copied().unwrap_or(0),
        )
    };
    parse(a) > parse(b)
}

pub fn check_and_update(embedded: &[u8]) -> Vec<u8> {
    print!("  Vérification des mises à jour Flocord...");

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            println!(" erreur réseau.");
            return embedded.to_vec();
        }
    };

    let text = match client.get(VERSION_URL).send().and_then(|r| r.text()) {
        Ok(t) => t,
        Err(_) => {
            println!(" hors ligne, version embarquée utilisée.");
            return embedded.to_vec();
        }
    };

    let manifest: VersionManifest = match serde_json::from_str(&text) {
        Ok(m) => m,
        Err(_) => {
            println!(" manifest invalide.");
            return embedded.to_vec();
        }
    };

    if !version_gt(&manifest.version, EMBEDDED_VERSION) {
        println!(" à jour (v{}).", EMBEDDED_VERSION);
        return embedded.to_vec();
    }

    println!();
    println!(
        "  Mise à jour disponible : v{} → v{}",
        EMBEDDED_VERSION, manifest.version
    );
    print!("  Téléchargement...");

    let bytes = match client.get(&manifest.url).send().and_then(|r| r.bytes()) {
        Ok(b) => b.to_vec(),
        Err(_) => {
            println!(" échec, version embarquée utilisée.");
            return embedded.to_vec();
        }
    };

    let cache = cache_path();

    if let Some(parent) = cache.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let _ = fs::write(&cache, &bytes);

    println!(" OK (v{}).", manifest.version);

    bytes
}
