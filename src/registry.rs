// Mémorise sur quels canaux Discord l'utilisateur a installé Flocord (%LOCALAPPDATA%\Flocord\installed.json).
// La protection automatique ne répare que ces canaux : une désinstallation volontaire n'est jamais annulée.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
struct Registry {
    channels: Vec<String>,
}

pub fn data_dir() -> PathBuf {
    let local = std::env::var("LOCALAPPDATA").unwrap_or_default();
    PathBuf::from(local).join("Flocord")
}

fn file() -> PathBuf {
    data_dir().join("installed.json")
}

fn load() -> Registry {
    fs::read_to_string(file())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(registry: &Registry) {
    let _ = fs::create_dir_all(data_dir());
    if let Ok(text) = serde_json::to_string_pretty(registry) {
        let _ = fs::write(file(), text);
    }
}

pub fn mark(channel: &str) {
    let mut registry = load();
    if !registry.channels.iter().any(|c| c == channel) {
        registry.channels.push(channel.to_string());
        save(&registry);
    }
}

pub fn unmark(channel: &str) {
    let mut registry = load();
    registry.channels.retain(|c| c != channel);
    save(&registry);
}

pub fn marked() -> Vec<String> {
    load().channels
}
