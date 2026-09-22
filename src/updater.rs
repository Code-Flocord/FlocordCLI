// Récupère la dernière version de Flocord (version.json sur GitHub) et télécharge l'asar si l'embarqué est dépassé.
// version.json est signé : sans la clé privée de Flocord (gardée hors de GitHub), personne ne peut faire accepter
// un autre fichier à l'installeur, même avec un accès au dépôt ou aux releases.

use std::fs;
use std::io::Read;

use crate::say;
use std::path::PathBuf;
use std::time::Duration;

use ed25519_dalek::{Signature, VerifyingKey};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const VERSION_URL: &str =
    "https://raw.githubusercontent.com/Code-Flocord/FlocordCLI/master/version.json";

/// Clé publique de publication Flocord (ed25519)
const SIGNING_KEY: [u8; 32] = [
    0x1c, 0x80, 0x84, 0xd8, 0x28, 0x7c, 0xd4, 0x4c, 0x93, 0x61, 0x32, 0x97, 0x08, 0x6e, 0x1d, 0x2a,
    0x0a, 0xd6, 0x42, 0x1b, 0x07, 0x0d, 0xe9, 0xab, 0xb4, 0xbb, 0x09, 0xc5, 0xd0, 0x13, 0xc8, 0x56,
];

/// Version de l'asar embarqué (assets/desktop.asar)
static EMBEDDED_VERSION: &str = include_str!("../flocord.version");

#[derive(Deserialize, Clone)]
pub struct VersionManifest {
    /// Version de Flocord (l'asar)
    pub version: String,
    pub url: String,
    /// Version de l'installeur, quand elle diffère de celle de Flocord
    #[serde(default)]
    pub cli: Option<String>,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub cli_sha256: String,
    #[serde(default)]
    pub signature: String,
}

impl VersionManifest {
    fn verify(&self, key: &[u8; 32]) -> bool {
        let message = format!(
            "flocord-manifest-v1\n{}\n{}\n{}\n{}\n{}",
            self.version, self.url, self.sha256, self.cli.as_deref().unwrap_or_default(), self.cli_sha256
        );
        let Some(signature) = hex(&self.signature).and_then(|bytes| <[u8; 64]>::try_from(bytes).ok()) else {
            return false;
        };
        VerifyingKey::from_bytes(key)
            .map(|key| key.verify_strict(message.as_bytes(), &Signature::from_bytes(&signature)).is_ok())
            .unwrap_or(false)
    }
}

fn hex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect()
}

pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{:02x}", b)).collect()
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
    EMBEDDED_VERSION.trim().to_string()
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
        .user_agent(format!("FlocordCLI/{}", cli_version()))
        .build()
        .ok()
}

/// Dernier manifest publié et correctement signé, ou None hors ligne
pub fn latest_manifest() -> Option<VersionManifest> {
    let text = http(6)?.get(VERSION_URL).send().ok()?.text().ok()?;
    let manifest: VersionManifest = serde_json::from_str(&text).ok()?;
    if !manifest.verify(&SIGNING_KEY) {
        crate::logger::write("version.json ignoré : signature absente ou invalide");
        return None;
    }
    Some(manifest)
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
    if let Ok(bytes) = fs::read(&cache) {
        if sha256(&bytes) == manifest.sha256 {
            say!("✔ v{} déjà téléchargée.", manifest.version);
            return Payload { bytes, version: manifest.version };
        }
    }

    let Some(bytes) = download(&manifest.url, "Téléchargement") else {
        say!("⚠ Téléchargement impossible, version embarquée utilisée.");
        return fallback();
    };

    if sha256(&bytes) != manifest.sha256 {
        crate::logger::write(&format!("desktop.asar v{} rejeté : empreinte différente de la version signée", manifest.version));
        say!("⚠ Fichier téléchargé différent de la version signée, version embarquée utilisée.");
        return fallback();
    }

    let _ = fs::create_dir_all(crate::registry::data_dir());
    let _ = fs::write(&cache, &bytes);

    say!("✔ Flocord v{} prêt.", manifest.version);
    Payload { bytes, version: manifest.version }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Clé de test uniquement : sa clé privée a été jetée après avoir signé tests/signed-manifest.json
    const TEST_KEY: [u8; 32] = [
        0x8a, 0x43, 0xf1, 0x11, 0x90, 0x7a, 0x14, 0xd2, 0x19, 0x0a, 0x0e, 0x2f, 0x4f, 0xf2, 0xad, 0x62,
        0xe9, 0x47, 0x7b, 0xbe, 0xd0, 0x10, 0xfe, 0x01, 0x4e, 0x5b, 0x70, 0x48, 0x3c, 0x19, 0x08, 0xb8,
    ];

    fn manifest() -> VersionManifest {
        serde_json::from_str(include_str!("../tests/signed-manifest.json")).unwrap()
    }

    #[test]
    fn accepts_signed_manifest() {
        assert!(manifest().verify(&TEST_KEY));
    }

    #[test]
    fn rejects_other_key() {
        assert!(!manifest().verify(&SIGNING_KEY));
    }

    #[test]
    fn rejects_any_changed_field() {
        let changes: [fn(&mut VersionManifest); 5] = [
            |m| m.version = "9.9.9".into(),
            |m| m.url = "https://example.com/desktop.asar".into(),
            |m| m.sha256 = "0".repeat(64),
            |m| m.cli = Some("9.9.9".into()),
            |m| m.cli_sha256 = "0".repeat(64),
        ];
        for change in changes {
            let mut m = manifest();
            change(&mut m);
            assert!(!m.verify(&TEST_KEY));
        }
    }

    #[test]
    fn rejects_missing_or_malformed_signature() {
        let mut m = manifest();
        for signature in [String::new(), "zz".repeat(64), "é".repeat(64), "00".repeat(64)] {
            m.signature = signature;
            assert!(!m.verify(&TEST_KEY));
        }
    }

    #[test]
    fn unsigned_legacy_manifest_parses_but_is_rejected() {
        let legacy: VersionManifest = serde_json::from_str(r#"{ "version": "2.8.3", "cli": "2.8.3", "url": "https://x" }"#).unwrap();
        assert!(!legacy.verify(&SIGNING_KEY));
    }

    #[test]
    fn sha256_matches_known_value() {
        assert_eq!(sha256(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }

    #[test]
    fn versions_compare_numerically() {
        assert!(version_gt("2.10.0", "2.9.9"));
        assert!(version_gt("3.0.0", "2.99.99"));
        assert!(!version_gt("2.8.3", "2.8.3"));
        assert!(!version_gt("2.8.2", "2.8.3"));
    }
}
