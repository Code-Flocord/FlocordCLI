use crate::client::DiscordClient;
use crate::detect;

use std::fs;

pub fn uninstall(client: &DiscordClient) {
    println!();
    println!("================================");
    println!("     Désinstallation Flocord");
    println!("================================");
    println!();

    let install = match detect::detect(client) {
        Some(value) => value,

        None => {
            println!("❌ Aucun Discord compatible trouvé.");
            return;
        }
    };

    println!("Client  : {}", client.name);
    println!("Version : {}", install.version);
    println!();

    if !install.installed {
        println!("✔ Flocord n'est pas installé.");
        return;
    }

    let resources = install.resources;

    let app_asar = resources.join("app.asar");

    let backup = resources
        .join("FlocordBackup")
        .join("app.asar");

    if !backup.exists() {
        println!("❌ Backup introuvable.");
        return;
    }

    if app_asar.exists() {
        if let Err(error) = fs::remove_file(&app_asar) {
            println!("❌ Impossible de supprimer app.asar : {}", error);
            return;
        }
    }

    if let Err(error) = fs::copy(&backup, &app_asar) {
        println!("❌ Impossible de restaurer le backup : {}", error);
        return;
    }

    // Suppression de _app.asar (Discord original utilisé par le patcher)
    let original_asar = resources.join("_app.asar");

    if original_asar.exists() {
        let _ = fs::remove_file(&original_asar);
    }

    // Nettoyage des artefacts d'anciennes installations
    let extract = resources.join("flocord_extract");

    if extract.exists() {
        let _ = fs::remove_dir_all(&extract);
    }

    let temp = resources.join("app_flocord.asar");

    if temp.exists() {
        let _ = fs::remove_file(&temp);
    }

    let app_original = resources.join("app.original.asar");

    if app_original.exists() {
        let _ = fs::remove_file(&app_original);
    }

    let marker = resources.join("flocord.lock");

    if marker.exists() {

    if let Err(error) = fs::remove_file(&marker) {

        println!("⚠ Impossible de supprimer le marqueur Flocord : {}", error);

    } else {

        println!("✔ Marqueur Flocord supprimé.");

    }

}

    println!();
    println!("✔ Flocord désinstallé.");

    crate::logger::write(&format!(
    "Flocord désinstallé sur {} {}",
    client.name,
    client.version
));
}