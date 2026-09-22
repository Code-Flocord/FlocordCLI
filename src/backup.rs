use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn backup_file(resources: &Path) -> PathBuf {
    resources.join("FlocordBackup").join("app.asar")
}

/// Copie le fichier Discord original (jamais l'asar Flocord) dans FlocordBackup/app.asar, une seule fois.
pub fn create_backup(resources: &Path, original: &Path) -> bool {
    let backup = backup_file(resources);

    if backup.exists() {
        say!("✔ Backup déjà présent.");
        return true;
    }

    if let Some(folder) = backup.parent() {
        if let Err(error) = fs::create_dir_all(folder) {
            say!("❌ Impossible de créer le dossier backup : {}", error);
            return false;
        }
    }

    if fs::copy(original, &backup).is_ok() {
        say!("✔ Discord original sauvegardé.");
        return true;
    }

    // Certains antivirus bloquent la copie directe : PowerShell passe généralement
    let command = format!(
        "Copy-Item -LiteralPath '{}' -Destination '{}' -Force",
        original.to_string_lossy(),
        backup.to_string_lossy()
    );
    let ok = Command::new("powershell")
        .args(["-NoProfile", "-Command", &command])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if ok {
        say!("✔ Discord original sauvegardé.");
    } else {
        say!("❌ Impossible de sauvegarder {}", original.display());
    }
    ok
}
