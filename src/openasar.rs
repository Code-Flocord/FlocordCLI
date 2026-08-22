use crate::openasar_detect;
use crate::client::DiscordClient;

use std::fs;
use std::path::PathBuf;

pub fn install(client: &DiscordClient) {

    println!();
    println!("================================");
    println!("     Installation OpenAsar");
    println!("================================");
    println!();

    let resources = match find_resources(client) {

        Some(path) => path,

        None => {
            println!("❌ Resources introuvable.");
            return;
        }

    };

    let app_asar = resources.join("app.asar");

    if !app_asar.exists() {

        println!("❌ app.asar introuvable.");
        return;

    }

    println!("✔ Resources : {}", resources.display());

    if openasar_detect::detect(&resources) {

        println!();
        println!("✔ OpenAsar est déjà installé.");
        return;

    }

    println!();
    println!("Création du backup OpenAsar...");

    let backup_folder = resources.join("OpenAsarBackup");

    if !backup_folder.exists() {

        if let Err(error) = fs::create_dir_all(&backup_folder) {

            println!("❌ Impossible de créer le backup OpenAsar : {}", error);
            return;

        }

    }

    let backup_file = backup_folder.join("app.asar");

    if !backup_file.exists() {

        if let Err(error) = fs::copy(&app_asar, &backup_file) {

            println!("❌ Backup OpenAsar impossible : {}", error);
            return;

        }

        println!("✔ Backup OpenAsar créé.");

    } else {

        println!("✔ Backup OpenAsar déjà présent.");

    }

    let openasar = PathBuf::from("assets/openasar.asar");

    if !openasar.exists() {

        println!("❌ assets/openasar.asar introuvable.");
        return;

    }

    println!();
    println!("Installation OpenAsar...");

    let backup_original = resources.join("app.original.asar");

    if app_asar.exists() && !backup_original.exists() {

        if let Err(error) = fs::copy(&app_asar, &backup_original) {

            println!("❌ Sauvegarde app.asar impossible : {}", error);
            return;

        }

    }

    if let Err(error) = fs::copy(&openasar, &app_asar) {

        println!("❌ Installation OpenAsar impossible : {}", error);
        return;

    }

    println!("✔ OpenAsar installé.");

    let lock = resources.join("openasar.lock");

    if let Err(error) = fs::write(&lock, "OpenAsar installed") {

        println!("⚠ Impossible de créer le marqueur : {}", error);

    }

    crate::logger::write(&format!(
        "OpenAsar installé sur {} {}",
        client.name,
        client.version
    ));

}

pub fn uninstall(client: &DiscordClient) {

    println!();
    println!("================================");
    println!("    Désinstallation OpenAsar");
    println!("================================");
    println!();

    let resources = match find_resources(client) {

        Some(path) => path,

        None => {
            println!("❌ Resources introuvable.");
            return;
        }

    };

    if !openasar_detect::detect(&resources) {

        println!();
        println!("✔ OpenAsar n'est pas installé.");
        return;

    }

    let app_asar = resources.join("app.asar");

    let backup = resources
        .join("OpenAsarBackup")
        .join("app.asar");

    if !backup.exists() {

        println!("❌ Backup introuvable.");
        return;

    }

    println!("Restauration du backup...");

    if let Err(error) = fs::copy(&backup, &app_asar) {

        println!("❌ Restauration impossible : {}", error);
        return;

    }

    println!("✔ OpenAsar supprimé.");

    let lock = resources.join("openasar.lock");

    if lock.exists() {

        let _ = fs::remove_file(lock);

    }

    crate::logger::write(&format!(
        "OpenAsar désinstallé sur {} {}",
        client.name,
        client.version
    ));

}

fn find_resources(client: &DiscordClient) -> Option<PathBuf> {

    let base = client.executable.parent()?.parent()?;

    let entries = fs::read_dir(base).ok()?;

    for entry in entries {

        let path = entry.ok()?.path();

        if path.is_dir() {

            let resources = path.join("resources");

            if resources.join("app.asar").exists() {

                return Some(resources);

            }

        }

    }

    None

}