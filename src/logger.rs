use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn log_folder() -> PathBuf {
    let desktop =
        std::env::var("USERPROFILE").expect("Impossible de trouver le profil utilisateur");

    PathBuf::from(desktop).join("Desktop").join("Flocord Logs")
}

pub fn init() {
    let folder = log_folder();

    if !folder.exists() {
        let _ = fs::create_dir_all(&folder);
    }

    let log_file = folder.join("installer.log");

    // Supprime l'ancien log
    if log_file.exists() {
        let _ = fs::remove_file(&log_file);
    }

    let mut file = File::create(&log_file).expect("Impossible de créer le fichier log");

    let _ = writeln!(file, "[{}] Flocord Installer démarré", timestamp());
}

pub fn write(message: &str) {
    let file_path = log_folder().join("installer.log");

    let mut file = OpenOptions::new()
        .append(true)
        .open(file_path)
        .expect("Impossible d'ouvrir le log");

    let _ = writeln!(file, "[{}] {}", timestamp(), message);
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}
