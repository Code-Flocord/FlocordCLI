use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn create_backup(discord_path: &PathBuf, app_asar: &PathBuf) -> bool {
    let backup_path = discord_path.join("FlocordBackup");

    if !backup_path.exists() {
        if let Err(error) = fs::create_dir_all(&backup_path) {
            println!("❌ Impossible de créer le dossier backup : {}", error);

            return false;
        }
    }

    let backup_file = backup_path.join("app.asar");

    if backup_file.exists() {
        println!("✔ Backup app.asar déjà présent.");

        return true;
    }

    println!("Copie de app.asar...");

    match fs::copy(app_asar, &backup_file) {
        Ok(_) => {
            println!("✔ app.asar sauvegardé : {}", backup_file.display());

            true
        }

        Err(_) => {
            println!("⚠ Copie classique refusée.");

            println!("Tentative avec PowerShell...");

            let source = app_asar.to_string_lossy();

            let destination = backup_file.to_string_lossy();

            let command = format!(
                "Copy-Item -Path '{}' -Destination '{}' -Force",
                source, destination
            );

            let result = Command::new("powershell")
                .args(["-Command", &command])
                .output();

            match result {
                Ok(output) if output.status.success() => {
                    println!("✔ app.asar sauvegardé.");

                    true
                }

                _ => {
                    println!("❌ Impossible de sauvegarder app.asar.");

                    println!("Fichier : {}", app_asar.display());

                    false
                }
            }
        }
    }
}
