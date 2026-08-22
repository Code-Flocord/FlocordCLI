use std::io::{self, Write};

mod client;
mod discord;
mod logger;
mod process;
mod selector;
mod version;

mod backup;
mod installer;
mod repair;
mod detect;
mod uninstall;
mod openasar;
mod openasar_detect;
mod updater;

fn main() {
    logger::init();

    clear_screen();
    banner();

    loop {
        println!();
        println!("Que voulez-vous faire ?");
        println!();

        println!("\x1b[35m[1]\x1b[0m Détecter Discord");
        println!("\x1b[35m[2]\x1b[0m Installer Flocord");
        println!("\x1b[35m[3]\x1b[0m Réparer Flocord");
        println!("\x1b[35m[4]\x1b[0m Désinstaller Flocord");
        println!("\x1b[35m[5]\x1b[0m Installer OpenAsar");
        println!("\x1b[35m[6]\x1b[0m Désinstaller OpenAsar");
        println!("\x1b[35m[7]\x1b[0m Quitter");

        println!();

        print!("> ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();

        io::stdin().read_line(&mut choice).expect("Erreur lecture");

        match choice.trim() {
            "1" => detect_discord(),

            "2" => install(),

            "3" => repair(),

            "4" => uninstall(),

            "5" => openasar_install(),

            "6" => openasar_uninstall(),

            "7" => {
                println!("Fermeture de Flocord Installer...");
                break;
            }

            _ => {
                println!("Choix invalide.");
            }
        }
    }
}

fn banner() {
    println!("\x1b[35m");

    println!(
        r#"
▄▀▀▀█ ▀     ▄▀▀▀▄ ▄▀▀▀█ ▄▀▀▀▄ ▀▀▀▀▄ ▀▀▀▀▄
▄▀▀   █   ▄ ▄   █ ▄   ▄ ▄   █ █▀▀▀▄ █   █
▀      ▀▀▀▀  ▀▀▀   ▀▀▀▀  ▀▀▀  ▀   ▀ ▀▀▀▀
"#
    );

    println!("\x1b[0m");

    println!("============================================");
    println!("          Flocord Installer v0.1.0");
    println!("============================================");
}

fn clear_screen() {
    print!("\x1B[2J\x1B[1;1H");
}

fn install() {
    println!();
    println!("================================");
    println!("       Installation Flocord");
    println!("================================");

    let targets = discord::find_discord();

    if targets.is_empty() {
        println!("Aucun client Discord trouvé.");
        return;
    }

    let selected = match selector::select_discord(&targets) {
        Some(client) => client,

        None => {
            println!("Installation annulée.");
            return;
        }
    };

    println!();

    println!("Client sélectionné :");
    println!("  Nom : {}", selected.name);
    println!("  Canal : {}", selected.channel);
    println!("  Version : {}", selected.version);
    println!("  Chemin : {}", selected.path.display());
    println!("  Executable : {}", selected.executable.display());

    logger::write(&format!(
        "Client sélectionné : {} {}",
        selected.name, selected.version
    ));

    println!();

    if process::is_process_running(&selected.path) {
        println!("⚠ {} est actuellement ouvert.", selected.name);

        println!();

        println!("{} doit être fermé avant l'installation.", selected.name);

        println!();

        println!("[1] Fermer {} et continuer", selected.name);

        println!("[2] Annuler");

        print!("> ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();

        io::stdin().read_line(&mut choice).unwrap();

        match choice.trim() {
            "1" => {
                println!();

                println!("Fermeture de Discord...");

                if process::close_discord(&selected.path) {
                    std::thread::sleep(std::time::Duration::from_secs(2));

                    if process::is_process_running(&selected.path) {
                        println!("❌ {} est toujours ouvert.", selected.name);

                        println!("Fermez-le manuellement avant de continuer.");

                        return;
                    }

                    println!("✔ {} fermé.", selected.name);

                    logger::write(&format!("{} fermé avec succès", selected.name));
                } else {
                    println!("❌ Impossible de fermer {}.", selected.name);

                    println!("Fermez Discord manuellement avant de continuer.");

                    return;
                }
            }

            _ => {
                println!("Installation annulée.");
                return;
            }
        }
    } else {
        println!("✔ {} est déjà fermé.", selected.name);

        logger::write(&format!("{} déjà fermé", selected.name));
    }

    println!();

    installer::install(&selected);

    logger::write("Préparation installation terminée");
}

fn repair() {
    println!();
    println!("================================");
    println!("       Réparation Flocord");
    println!("================================");

    let targets = discord::find_discord();

    if targets.is_empty() {
        println!("Aucun client Discord trouvé.");
        return;
    }

    let selected = match selector::select_discord(&targets) {
        Some(client) => client,

        None => {
            println!("Réparation annulée.");
            return;
        }
    };

    println!();

    println!("Client sélectionné :");
    println!("  Nom : {}", selected.name);
    println!("  Canal : {}", selected.channel);
    println!("  Version : {}", selected.version);
    println!("  Chemin : {}", selected.path.display());
    println!("  Executable : {}", selected.executable.display());

    logger::write(&format!(
        "Client sélectionné : {} {}",
        selected.name, selected.version
    ));

    println!();

    if process::is_process_running(&selected.path) {
        println!("⚠ {} est actuellement ouvert.", selected.name);

        println!();

        println!("{} doit être fermé avant la réparation.", selected.name);

        println!();

        println!("[1] Fermer {} et continuer", selected.name);
        println!("[2] Annuler");

        print!("> ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();

        io::stdin().read_line(&mut choice).unwrap();

        match choice.trim() {
            "1" => {
                println!();

                println!("Fermeture de Discord...");

                if process::close_discord(&selected.path) {
                    std::thread::sleep(std::time::Duration::from_secs(2));

                    if process::is_process_running(&selected.path) {
                        println!("❌ {} est toujours ouvert.", selected.name);
                        println!("Fermez-le manuellement avant de continuer.");
                        return;
                    }

                    println!("✔ {} fermé.", selected.name);

                    logger::write(&format!("{} fermé avec succès", selected.name));
                } else {
                    println!("❌ Impossible de fermer {}.", selected.name);
                    println!("Fermez Discord manuellement avant de continuer.");
                    return;
                }
            }

            _ => {
                println!("Réparation annulée.");
                return;
            }
        }
    } else {
        println!("✔ {} est déjà fermé.", selected.name);

        logger::write(&format!("{} déjà fermé", selected.name));
    }

    println!();

    repair::repair(&selected);

    logger::write("Réparation terminée");
}

fn uninstall() {
    println!();
    println!("================================");
    println!("     Désinstallation Flocord");
    println!("================================");

    let targets = discord::find_discord();

    if targets.is_empty() {
        println!("Aucun client Discord trouvé.");
        return;
    }

    let selected = match selector::select_discord(&targets) {
        Some(client) => client,

        None => {
            println!("Désinstallation annulée.");
            return;
        }
    };

    println!();

    println!("Client sélectionné :");
    println!("  Nom : {}", selected.name);
    println!("  Canal : {}", selected.channel);
    println!("  Version : {}", selected.version);
    println!("  Chemin : {}", selected.path.display());
    println!("  Executable : {}", selected.executable.display());

    logger::write(&format!(
        "Client sélectionné : {} {}",
        selected.name, selected.version
    ));

    println!();

    if process::is_process_running(&selected.path) {
        println!("⚠ {} est actuellement ouvert.", selected.name);

        println!();

        println!("{} doit être fermé avant la désinstallation.", selected.name);

        println!();

        println!("[1] Fermer {} et continuer", selected.name);
        println!("[2] Annuler");

        print!("> ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();

        io::stdin().read_line(&mut choice).unwrap();

        match choice.trim() {
            "1" => {
                println!();

                println!("Fermeture de Discord...");

                if process::close_discord(&selected.path) {
                    std::thread::sleep(std::time::Duration::from_secs(2));

                    if process::is_process_running(&selected.path) {
                        println!("❌ {} est toujours ouvert.", selected.name);
                        println!("Fermez-le manuellement avant de continuer.");
                        return;
                    }

                    println!("✔ {} fermé.", selected.name);

                    logger::write(&format!("{} fermé avec succès", selected.name));
                } else {
                    println!("❌ Impossible de fermer {}.", selected.name);
                    println!("Fermez Discord manuellement avant de continuer.");
                    return;
                }
            }

            _ => {
                println!("Désinstallation annulée.");
                return;
            }
        }
    } else {
        println!("✔ {} est déjà fermé.", selected.name);

        logger::write(&format!("{} déjà fermé", selected.name));
    }

    println!();

    uninstall::uninstall(&selected);

    logger::write("Désinstallation terminée");
}

fn openasar_install() {

    let targets = discord::find_discord();


    if targets.is_empty() {

        println!("Aucun Discord trouvé.");
        return;

    }


    let selected = match selector::select_discord(&targets) {

        Some(client) => client,

        None => return,

    };


    if process::is_process_running(&selected.path) {

        process::close_discord(&selected.path);

    }


    openasar::install(&selected);

}



fn openasar_uninstall() {

    let targets = discord::find_discord();


    if targets.is_empty() {

        println!("Aucun Discord trouvé.");
        return;

    }


    let selected = match selector::select_discord(&targets) {

        Some(client) => client,

        None => return,

    };


    if process::is_process_running(&selected.path) {

        process::close_discord(&selected.path);

    }


    openasar::uninstall(&selected);

}

fn detect_discord() {
    println!();
    println!("================================");
    println!("        Discord détectés");
    println!("================================");
    println!();

    let clients = discord::find_discord();

    if clients.is_empty() {
        println!("Aucun Discord trouvé.");
        return;
    }

    for client in clients {
        println!("✔ {}", client.name);

        println!("  Canal : {}", client.channel);

        println!("  Chemin :");

        println!("  {}", client.path.display());

        println!();

        println!("  Version : {}", client.version);

        println!("  Executable :");

        println!("  {}", client.executable.display());

        println!();

        println!("-------------------------");
    }
}
