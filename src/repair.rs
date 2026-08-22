use crate::client::DiscordClient;
use crate::detect;
use crate::installer;

use std::fs;


pub fn repair(client: &DiscordClient) {

    println!();
    println!("================================");
    println!("      Réparation Flocord");
    println!("================================");
    println!();


    let status = match detect::detect(client) {

        Some(value) => value,

        None => {

            println!("❌ Impossible de détecter Discord.");
            return;

        }

    };



    if !status.installed {

        println!("⚠ Flocord n'est pas installé.");
        println!("Lancement de l'installation...");

        installer::install(client);

        return;

    }



    println!("✔ Flocord détecté.");

    println!();
    println!("Restauration propre...");



    let backup = status
        .resources
        .join("FlocordBackup")
        .join("app.asar");


    let app_asar = status.resources.join("app.asar");



    if !backup.exists() {

        println!("❌ Backup introuvable.");
        return;

    }



    if let Err(error) = fs::copy(&backup, &app_asar) {

        println!("❌ Restauration impossible : {}", error);
        return;

    }


    println!("✔ Backup restauré.");



    // Suppression du marqueur d'installation
    let marker = status.resources.join("flocord.lock");

    if marker.exists() {

        match fs::remove_file(&marker) {

            Ok(_) => {
                println!("✔ Marqueur Flocord supprimé.");
            }

            Err(error) => {
                println!("⚠ Impossible de supprimer le marqueur : {}", error);
            }

        }

    }



    // Nettoyage ancienne extraction
    let extract = status.resources.join("flocord_extract");

    if extract.exists() {

        match fs::remove_dir_all(&extract) {

            Ok(_) => {
                println!("✔ Ancienne extraction supprimée.");
            }

            Err(error) => {
                println!("⚠ Nettoyage extraction impossible : {}", error);
            }

        }

    }



    println!();
    println!("Réinstallation Flocord...");


    installer::install(client);


}