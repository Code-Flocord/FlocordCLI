use crate::backup;
use crate::client::DiscordClient;
use crate::detect;
use crate::logger;
use crate::registry;

use std::fs;

/// Remet le Discord original en place et retire toute trace de Flocord dans le dossier courant.
pub fn uninstall(client: &DiscordClient) -> bool {
    say!("");
    say!("Client  : {} ({})", client.name, client.channel);

    let Some(info) = detect::detect(client) else {
        say!("❌ Aucun dossier Discord exploitable.");
        return false;
    };

    say!("Discord : {}", info.version);
    say!("");

    let resources = &info.resources;
    let app = &info.app_asar;
    let original = &info.original_asar;
    let saved = backup::backup_file(resources);

    let has_flocord = info.installed || original.exists() || app.is_dir();
    if !has_flocord {
        say!("✔ Flocord n'est pas installé sur ce Discord.");
        registry::unmark(&client.channel);
        return true;
    }

    // Retire l'asar (ou le dossier relais) Flocord
    let removed = if app.is_dir() { fs::remove_dir_all(app) } else if app.exists() { fs::remove_file(app) } else { Ok(()) };
    if let Err(error) = removed {
        say!("❌ Impossible de retirer app.asar : {}", error);
        return false;
    }

    // Remet l'original : _app.asar de préférence (c'est le fichier exact de Discord), sinon le backup
    if original.exists() {
        if let Err(error) = fs::rename(original, app) {
            say!("❌ Impossible de restaurer _app.asar : {}", error);
            return false;
        }
        say!("✔ Discord original restauré.");
    } else if saved.exists() {
        if let Err(error) = fs::copy(&saved, app) {
            say!("❌ Impossible de restaurer le backup : {}", error);
            return false;
        }
        say!("✔ Discord original restauré depuis le backup.");
    } else {
        say!("❌ Discord original introuvable. Réinstallez Discord.");
        return false;
    }

    for leftover in ["flocord.lock", "flocord_extract", "app_flocord.asar", "app.original.asar", "flocord_temp.asar"] {
        let path = resources.join(leftover);
        let _ = if path.is_dir() { fs::remove_dir_all(&path) } else { fs::remove_file(&path) };
    }

    registry::unmark(&client.channel);
    logger::write(&format!("Flocord désinstallé de {} {}", client.name, info.version));

    say!("");
    say!("\x1b[32m✔ Flocord désinstallé de {}.\x1b[0m", client.name);
    true
}
