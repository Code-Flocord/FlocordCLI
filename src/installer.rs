use crate::backup;
use crate::client::DiscordClient;
use crate::detect;
use crate::logger;
use crate::registry;
use crate::updater;

use std::fs;
use std::path::Path;

static DESKTOP_ASAR: &[u8] = include_bytes!("../assets/desktop.asar");

fn remove_any(path: &Path) -> Result<(), String> {
    let result = if path.is_dir() { fs::remove_dir_all(path) } else if path.exists() { fs::remove_file(path) } else { Ok(()) };
    if result.is_ok() {
        return Ok(());
    }

    let command = format!("Remove-Item -LiteralPath {} -Recurse -Force", crate::process::ps_quote(path));
    let ok = crate::process::hidden("powershell")
        .args(["-NoProfile", "-Command", &command])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if ok { Ok(()) } else { Err(format!("impossible de supprimer {}", path.display())) }
}

fn write_asar(target: &Path, resources: &Path, data: &[u8]) -> Result<(), String> {
    if fs::write(target, data).is_ok() {
        return Ok(());
    }

    let temp = resources.join("flocord_temp.asar");
    fs::write(&temp, data).map_err(|e| format!("écriture impossible : {}", e))?;

    let command = format!(
        "Move-Item -LiteralPath {} -Destination {} -Force",
        crate::process::ps_quote(&temp),
        crate::process::ps_quote(target)
    );
    let ok = crate::process::hidden("powershell")
        .args(["-NoProfile", "-Command", &command])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if ok {
        Ok(())
    } else {
        let _ = fs::remove_file(&temp);
        Err("écriture de app.asar refusée".to_string())
    }
}

/// Installe (ou réinstalle avec `force`) Flocord sur le dossier Discord le plus récent.
/// Gère tous les états : Discord vierge, Flocord déjà présent, dossier relais app/ laissé par une mise à jour.
pub fn install(client: &DiscordClient, force: bool) -> bool {
    say!("");
    say!("Client  : {} ({})", client.name, client.channel);

    if !client.executable.exists() {
        say!("❌ Exécutable Discord introuvable : {}", client.executable.display());
        return false;
    }

    let Some(info) = detect::detect(client) else {
        say!("❌ Aucun dossier Discord exploitable.");
        return false;
    };

    say!("Discord : {}", info.version);

    if info.installed && !force {
        say!("");
        say!("✔ Flocord v{} est déjà installé.", info.flocord_version.unwrap_or_default());
        registry::mark(&client.channel);
        return true;
    }

    let resources = &info.resources;
    let app = &info.app_asar;
    let original = &info.original_asar;

    say!("");
    say!("Préparation du Discord original...");

    if original.exists() {
        say!("✔ _app.asar présent.");
        if !backup::create_backup(resources, original) {
            return false;
        }
    } else if app.is_file() {
        if !backup::create_backup(resources, app) {
            return false;
        }
        if let Err(error) = fs::rename(app, original) {
            say!("❌ Impossible de renommer app.asar en _app.asar : {}", error);
            return false;
        }
        say!("✔ _app.asar créé.");
    } else {
        // app.asar est un dossier sans original à côté : on repart du backup
        let saved = backup::backup_file(resources);
        if !saved.exists() {
            say!("❌ Discord original introuvable (ni _app.asar, ni backup). Réinstallez Discord.");
            return false;
        }
        if let Err(error) = fs::copy(&saved, original) {
            say!("❌ Restauration du backup impossible : {}", error);
            return false;
        }
        say!("✔ _app.asar restauré depuis le backup.");
    }

    if app.exists() {
        if let Err(error) = remove_any(app) {
            say!("❌ {}", error);
            return false;
        }
    }

    say!("");
    say!("Installation de Flocord...");

    let payload = updater::check_and_update(DESKTOP_ASAR);

    if let Err(error) = write_asar(app, resources, &payload.bytes) {
        say!("❌ {}", error);
        return false;
    }

    if let Err(error) = fs::write(resources.join("flocord.lock"), &payload.version) {
        say!("⚠ Impossible d'écrire le marqueur : {}", error);
    }

    for leftover in ["flocord_extract", "app_flocord.asar", "app.original.asar", "flocord_temp.asar"] {
        let _ = remove_any(&resources.join(leftover));
    }

    registry::mark(&client.channel);
    logger::write(&format!("Flocord v{} installé sur {} {}", payload.version, client.name, info.version));

    say!("");
    say!("\x1b[32m✔ Flocord v{} installé sur {}.\x1b[0m", payload.version, client.name);
    true
}
