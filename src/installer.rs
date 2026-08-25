use crate::backup;
use crate::client::DiscordClient;
use crate::detect;
use crate::logger;

use std::fs;
use std::process::Command;

static DESKTOP_ASAR: &[u8] = include_bytes!("../assets/desktop.asar");

pub fn install(client: &DiscordClient) {
    println!();
    println!("================================");
    println!("      Installation Flocord");
    println!("================================");
    println!();

    println!("Client  : {}", client.name);
    println!("Canal   : {}", client.channel);

    println!();
    println!("Vérification du client Discord...");

    if !client.executable.exists() {
        println!("❌ Exécutable Discord introuvable.");
        println!("{}", client.executable.display());
        return;
    }

    println!("✔ Exécutable trouvé.");

    let install = match detect::detect(client) {
        Some(value) => value,
        None => {
            println!("❌ Aucun Discord compatible trouvé.");
            return;
        }
    };

    println!("✔ Version ciblée : {}", install.version);
    println!("✔ Resources : {}", install.resources.display());

    if install.installed {
        println!();
        println!("✔ Flocord est déjà installé.");
        return;
    }

    println!();
    println!("Création du backup...");

    if !backup::create_backup(&install.resources, &install.app_asar) {
        println!("❌ Impossible de créer le backup.");
        return;
    }

    println!("✔ Backup prêt.");

    println!();
    println!("Préparation du Discord original...");

    let app_asar = &install.app_asar;
    let original_asar = install.resources.join("_app.asar");

    if original_asar.exists() {
        println!("✔ _app.asar déjà présent.");
    } else if app_asar.is_dir() {
        // Nouveau format Discord : app.asar est un dossier → renommer en _app.asar
        match fs::rename(app_asar, &original_asar) {
            Ok(_) => {
                println!("✔ _app.asar créé (dossier renommé).");
            }
            Err(_) => {
                let command = format!(
                    "Rename-Item -Path '{}' -NewName '_app.asar' -Force",
                    app_asar.to_string_lossy()
                );
                let result = Command::new("powershell")
                    .args(["-Command", &command])
                    .output();
                match result {
                    Ok(output) if output.status.success() => {
                        println!("✔ _app.asar créé (dossier renommé via PowerShell).");
                    }
                    _ => {
                        println!("❌ Impossible de renommer app.asar en _app.asar.");
                        return;
                    }
                }
            }
        }
    } else {
        // Format classique : app.asar est un fichier → copier en _app.asar
        match fs::copy(app_asar, &original_asar) {
            Ok(_) => {
                println!("✔ _app.asar créé.");
            }
            Err(_) => {
                println!("⚠ Copie directe refusée, tentative PowerShell...");
                let command = format!(
                    "Copy-Item -Path '{}' -Destination '{}' -Force",
                    app_asar.to_string_lossy(),
                    original_asar.to_string_lossy()
                );
                let result = Command::new("powershell")
                    .args(["-Command", &command])
                    .output();
                match result {
                    Ok(output) if output.status.success() => {
                        println!("✔ _app.asar créé.");
                    }
                    _ => {
                        println!("❌ Impossible de créer _app.asar.");
                        return;
                    }
                }
            }
        }
    }

    println!();
    println!("Installation de Flocord...");

    let asar_data = crate::updater::check_and_update(DESKTOP_ASAR);

    match fs::write(app_asar, &asar_data) {
        Ok(_) => {
            println!("✔ Flocord installé.");
        }

        Err(_) => {
            println!("⚠ Écriture directe refusée, tentative PowerShell...");

            let temp = install.resources.join("flocord_temp.asar");

            if fs::write(&temp, &asar_data).is_err() {
                println!("❌ Impossible d'écrire le fichier Flocord.");
                return;
            }

            let command = format!(
                "Move-Item -Path '{}' -Destination '{}' -Force",
                temp.to_string_lossy(),
                app_asar.to_string_lossy()
            );

            let result = Command::new("powershell")
                .args(["-Command", &command])
                .output();

            match result {
                Ok(output) if output.status.success() => {
                    println!("✔ Flocord installé.");
                }

                _ => {
                    println!("❌ Installation impossible.");
                    let _ = fs::remove_file(&temp);
                    return;
                }
            }
        }
    }

    let marker = install.resources.join("flocord.lock");

    if let Err(error) = fs::write(&marker, "Flocord installed") {
        println!("⚠ Impossible de créer le marqueur : {}", error);
    }

    logger::write(&format!(
        "Flocord installé sur {} {}",
        client.name, client.version
    ));

    println!();
    println!("================================");
    println!("   Installation terminée !");
    println!("================================");
    println!();
    println!("Vous pouvez maintenant lancer Discord.");
}
