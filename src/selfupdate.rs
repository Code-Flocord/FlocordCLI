// Mise à jour de l'installeur lui-même depuis les releases GitHub de FlocordCLI.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::logger;
use crate::updater;

fn release_url(version: &str) -> String {
    format!("https://github.com/Code-Flocord/FlocordCLI/releases/download/v{}/FlocordCLI.exe", version)
}

fn old_exe(exe: &PathBuf) -> PathBuf {
    exe.with_extension("old.exe")
}

/// Supprime l'ancien exécutable laissé par une mise à jour précédente
pub fn cleanup() {
    if let Ok(exe) = std::env::current_exe() {
        let old = old_exe(&exe);
        if old.exists() {
            let _ = fs::remove_file(old);
        }
    }
}

/// Version plus récente de l'installeur disponible en ligne, s'il y en a une
pub fn available() -> Option<String> {
    let manifest = updater::latest_manifest()?;
    let latest = manifest.cli.unwrap_or(manifest.version);
    if updater::version_gt(&latest, updater::cli_version()) {
        Some(latest)
    } else {
        None
    }
}

/// Télécharge la nouvelle version, remplace l'exécutable courant et le relance.
pub fn run(version: &str) -> bool {
    let Ok(exe) = std::env::current_exe() else {
        say!("❌ Chemin de l'installeur introuvable.");
        return false;
    };

    say!("");
    let Some(bytes) = updater::download(&release_url(version), "Installeur") else {
        say!("❌ Téléchargement impossible.");
        return false;
    };

    if bytes.len() < 1_000_000 || !bytes.starts_with(b"MZ") {
        say!("❌ Fichier téléchargé invalide.");
        return false;
    }

    // Windows autorise le renommage d'un exécutable en cours d'exécution, pas son écrasement
    let old = old_exe(&exe);
    let _ = fs::remove_file(&old);
    if let Err(error) = fs::rename(&exe, &old) {
        say!("❌ Impossible de remplacer l'installeur : {}", error);
        return false;
    }

    if let Err(error) = fs::write(&exe, &bytes) {
        say!("❌ Écriture impossible : {}", error);
        let _ = fs::rename(&old, &exe);
        return false;
    }

    logger::write(&format!("Installeur mis à jour vers v{}", version));
    say!("✔ Installeur v{} installé, relance...", version);

    let args: Vec<String> = std::env::args().skip(1).collect();
    let _ = Command::new(&exe).args(args).spawn();
    std::process::exit(0);
}
