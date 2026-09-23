// Récupère la dernière version de Flocord (version.json sur GitHub) et télécharge l'asar si l'embarqué est dépassé.
// version.json est signé : sans la clé privée de Flocord (gardée hors de GitHub), personne ne peut faire accepter
// un autre fichier à l'installeur, même avec un accès au dépôt ou aux releases.
//
// Confiance en deux étages : le client n'embarque que la clé RACINE (ROOT_KEY), gardée hors ligne. Le manifeste
// transporte la sous-clé de release courante et son epoch, certifiés par une délégation signée par la racine
// (flocord-key-v1). La sous-clé signe le manifeste. Si la sous-clé est perdue ou compromise, la racine délègue à
// une nouvelle sous-clé avec un epoch supérieur : les clients avancent un plancher d'epoch et refusent l'ancienne
// (révocation), sans que l'auto-update meure.
//
// La signature seule n'empêche pas de rejouer un ANCIEN manifeste valablement signé (dépôt compromis) pour geler
// un client ou le renvoyer vers une version passée. Deux garde-fous s'y ajoutent : le manifeste porte une date
// signée et il est refusé au-delà de MAX_AGE (fraîcheur), et le client mémorise la plus haute version déjà vue et
// refuse toute version inférieure (plancher anti-retour, fichier min-version).

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

/// Clé publique RACINE Flocord (ed25519), ancre de confiance des clients. Sa clé privée reste hors ligne
/// et ne sert qu'à signer les délégations de sous-clés.
const ROOT_KEY: [u8; 32] = [
    0x12, 0xd1, 0x64, 0xd1, 0xec, 0x1c, 0x55, 0x48, 0xfd, 0x3a, 0xb2, 0xfe, 0xfd, 0x8e, 0x5f, 0xf2,
    0x13, 0x89, 0x4d, 0x83, 0x91, 0xed, 0xfe, 0x51, 0x20, 0x9a, 0xd0, 0x10, 0x11, 0x1c, 0xd9, 0xab,
];

/// Version de l'asar embarqué (assets/desktop.asar)
static EMBEDDED_VERSION: &str = include_str!("../flocord.version");

/// Un manifeste plus vieux que ça est refusé, même correctement signé (protection anti-rejeu).
const MAX_AGE_SECS: i64 = 180 * 86_400;

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
    /// Date de signature (secondes Unix). Absente sur les anciens manifestes v1, alors refusés.
    #[serde(default)]
    pub date: Option<i64>,
    /// Sous-clé de release courante (hex), certifiée par la racine.
    #[serde(default)]
    pub key: String,
    /// Génération de la sous-clé : augmente à chaque rotation, sert de plancher anti-révocation.
    #[serde(default)]
    pub key_epoch: Option<u64>,
    /// Signature de la racine sur la délégation (flocord-key-v1).
    #[serde(default)]
    pub key_sig: String,
    // Le champ v1 `signature` du JSON n'est plus lu ici (il ne sert qu'aux anciens clients) : serde l'ignore.
    #[serde(default)]
    pub signature2: String,
}

impl VersionManifest {
    /// Message signé v2, ou None si le manifeste n'a pas de date (v1 pur : refusé par un client v2).
    fn message(&self) -> Option<String> {
        let date = self.date?;
        Some(format!(
            "flocord-manifest-v2\n{}\n{}\n{}\n{}\n{}\n{}",
            self.version, self.url, self.sha256, self.cli.as_deref().unwrap_or_default(), self.cli_sha256, date
        ))
    }

    /// Vrai si la racine délègue bien à la sous-clé annoncée ET que celle-ci signe le manifeste. Purement
    /// cryptographique : fraîcheur, plancher de version et plancher d'epoch sont contrôlés séparément dans
    /// latest_manifest, ce qui garde cette vérif indépendante du temps et de l'état persisté.
    fn verify(&self, root_key: &[u8; 32]) -> bool {
        let Some(epoch) = self.key_epoch else {
            return false;
        };
        let Some(subkey) = hex(&self.key).and_then(|bytes| <[u8; 32]>::try_from(bytes).ok()) else {
            return false;
        };
        let delegation = format!("flocord-key-v1\n{}\n{}", self.key, epoch);
        if !ed_verify(root_key, delegation.as_bytes(), &self.key_sig) {
            return false;
        }
        let Some(message) = self.message() else {
            return false;
        };
        ed_verify(&subkey, message.as_bytes(), &self.signature2)
    }
}

/// Vérifie une signature ed25519 hex détachée sur un message pour une clé publique donnée.
fn ed_verify(key: &[u8; 32], message: &[u8], signature_hex: &str) -> bool {
    let Some(signature) = hex(signature_hex).and_then(|bytes| <[u8; 64]>::try_from(bytes).ok()) else {
        return false;
    };
    VerifyingKey::from_bytes(key)
        .map(|key| key.verify_strict(message, &Signature::from_bytes(&signature)).is_ok())
        .unwrap_or(false)
}

/// Vrai tant que le manifeste n'est pas plus vieux que MAX_AGE. Une horloge en retard ne fait que rendre
/// le manifeste « plus récent » (now - date négatif), jamais périmé : on ne gèle pas sur une horloge cassée.
fn fresh(date: i64, now: i64) -> bool {
    now - date <= MAX_AGE_SECS
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn floor_path() -> PathBuf {
    crate::registry::data_dir().join("min-version")
}

/// Plus haute version déjà acceptée, ou None si le fichier est absent ou illisible (on repart alors sans
/// plancher : une corruption fait perdre la protection, jamais bloquer les mises à jour légitimes).
fn stored_floor() -> Option<String> {
    let text = fs::read_to_string(floor_path()).ok()?;
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn raise_floor(version: &str) {
    if let Some(floor) = stored_floor() {
        if !version_gt(version, &floor) {
            return;
        }
    }
    let _ = fs::create_dir_all(crate::registry::data_dir());
    let _ = fs::write(floor_path(), version);
}

fn epoch_floor_path() -> PathBuf {
    crate::registry::data_dir().join("min-key-epoch")
}

/// Plus haut epoch de sous-clé déjà accepté, ou None si absent/illisible (on repart alors sans plancher).
fn stored_epoch() -> Option<u64> {
    fs::read_to_string(epoch_floor_path()).ok()?.trim().parse().ok()
}

fn raise_epoch(epoch: u64) {
    if let Some(floor) = stored_epoch() {
        if epoch <= floor {
            return;
        }
    }
    let _ = fs::create_dir_all(crate::registry::data_dir());
    let _ = fs::write(epoch_floor_path(), epoch.to_string());
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

/// Dernier manifest publié, correctement signé, frais et non inférieur au plancher. None hors ligne ou rejeté.
pub fn latest_manifest() -> Option<VersionManifest> {
    let text = http(6)?.get(VERSION_URL).send().ok()?.text().ok()?;
    let manifest: VersionManifest = serde_json::from_str(&text).ok()?;
    if !manifest.verify(&ROOT_KEY) {
        crate::logger::write("version.json ignoré : délégation ou signature absente ou invalide");
        return None;
    }
    if !manifest.date.map(|date| fresh(date, now_secs())).unwrap_or(false) {
        crate::logger::write("version.json ignoré : manifeste périmé (protection anti-rejeu)");
        return None;
    }
    let epoch = manifest.key_epoch?;
    if let Some(floor) = stored_epoch() {
        if epoch < floor {
            crate::logger::write("version.json ignoré : sous-clé révoquée (epoch inférieur au plancher)");
            return None;
        }
    }
    if let Some(floor) = stored_floor() {
        if version_gt(&floor, &manifest.version) {
            crate::logger::write("version.json ignoré : version inférieure au plancher anti-retour");
            return None;
        }
    }
    raise_epoch(epoch);
    raise_floor(&manifest.version);
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

    // Clé RACINE de test uniquement : sa clé privée a été jetée après avoir signé tests/signed-manifest.json
    const TEST_ROOT_KEY: [u8; 32] = [
        0xff, 0xd2, 0x2b, 0x93, 0x65, 0x3f, 0x42, 0x0f, 0xff, 0x65, 0x4a, 0xce, 0x6e, 0xb9, 0x97, 0x1e,
        0x4c, 0xd8, 0x3c, 0xef, 0xda, 0xfe, 0x7f, 0xe0, 0xa8, 0xc6, 0x6b, 0x8a, 0x89, 0x0c, 0x98, 0xe4,
    ];

    // Une autre clé racine valide, pour vérifier qu'un manifeste délégué par une racine n'est pas accepté par une autre.
    const OTHER_KEY: [u8; 32] = [
        0xec, 0x3c, 0xf1, 0x66, 0x4e, 0x95, 0x06, 0x09, 0xcf, 0x58, 0x1b, 0xf9, 0x68, 0x6c, 0x13, 0x1a,
        0x50, 0xf3, 0xd6, 0x60, 0x5b, 0xcb, 0x72, 0x74, 0x05, 0x65, 0x33, 0xe4, 0xba, 0x1f, 0xea, 0x76,
    ];

    fn manifest() -> VersionManifest {
        serde_json::from_str(include_str!("../tests/signed-manifest.json")).unwrap()
    }

    #[test]
    fn accepts_signed_manifest() {
        assert!(manifest().verify(&TEST_ROOT_KEY));
    }

    #[test]
    fn rejects_other_root_key() {
        assert!(!manifest().verify(&OTHER_KEY));
        assert!(!manifest().verify(&ROOT_KEY)); // le placeholder tout à zéro ne valide rien non plus
    }

    #[test]
    fn rejects_any_changed_field() {
        let changes: [fn(&mut VersionManifest); 8] = [
            |m| m.version = "9.9.9".into(),
            |m| m.url = "https://example.com/desktop.asar".into(),
            |m| m.sha256 = "0".repeat(64),
            |m| m.cli = Some("9.9.9".into()),
            |m| m.cli_sha256 = "0".repeat(64),
            |m| m.date = Some(0),
            |m| m.key = "00".repeat(32),   // sous-clé échangée : la délégation ne colle plus
            |m| m.key_epoch = Some(42),    // epoch trafiqué : la délégation ne colle plus
        ];
        for change in changes {
            let mut m = manifest();
            change(&mut m);
            assert!(!m.verify(&TEST_ROOT_KEY));
        }
    }

    #[test]
    fn rejects_missing_or_malformed_signature() {
        for field in ["signature2", "key_sig"] {
            for value in [String::new(), "zz".repeat(64), "é".repeat(64), "00".repeat(64)] {
                let mut m = manifest();
                match field {
                    "signature2" => m.signature2 = value,
                    _ => m.key_sig = value,
                }
                assert!(!m.verify(&TEST_ROOT_KEY));
            }
        }
    }

    #[test]
    fn rejects_manifest_without_date_or_epoch() {
        let mut m = manifest();
        m.date = None;
        assert!(!m.verify(&TEST_ROOT_KEY));
        let mut m = manifest();
        m.key_epoch = None;
        assert!(!m.verify(&TEST_ROOT_KEY));
    }

    #[test]
    fn unsigned_legacy_manifest_parses_but_is_rejected() {
        // Un manifeste v1 pur (ni délégation, ni date, ni signature2) est refusé sans repli.
        let legacy: VersionManifest = serde_json::from_str(r#"{ "version": "2.8.3", "cli": "2.8.3", "url": "https://x" }"#).unwrap();
        assert!(!legacy.verify(&OTHER_KEY));
        assert!(!legacy.verify(&TEST_ROOT_KEY));
    }

    #[test]
    fn stale_manifest_is_not_fresh() {
        let now = 1_800_000_000;
        assert!(fresh(now, now));
        assert!(fresh(now - MAX_AGE_SECS, now));
        assert!(!fresh(now - MAX_AGE_SECS - 1, now));
        assert!(fresh(now + 10_000, now)); // horloge en retard : jamais périmé
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
