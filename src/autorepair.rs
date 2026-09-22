// Protection automatique : au démarrage de Windows, un lanceur invisible exécute
// une copie de l'installeur avec `--repair --silent`. Seuls les canaux enregistrés
// (installés via le CLI, jamais désinstallés) sont réparés, et uniquement s'ils en ont besoin.

use std::fs;
use std::path::PathBuf;

use crate::logger;
use crate::process;
use crate::registry;
use crate::repair;
use crate::status;
use crate::updater;

fn launcher() -> PathBuf {
    let appdata = std::env::var("APPDATA").unwrap_or_default();
    PathBuf::from(appdata)
        .join("Microsoft\\Windows\\Start Menu\\Programs\\Startup")
        .join("FlocordProtection.vbs")
}

fn installed_exe() -> PathBuf {
    registry::data_dir().join("FlocordCLI.exe")
}

pub fn is_enabled() -> bool {
    launcher().exists()
}

/// Copie l'installeur courant dans %LOCALAPPDATA%\Flocord s'il est absent ou plus ancien
fn refresh_exe() -> Result<(), String> {
    let current = std::env::current_exe().map_err(|e| e.to_string())?;
    let target = installed_exe();
    if current == target {
        return Ok(());
    }

    let version_file = target.with_extension("version");
    let up_to_date = target.exists()
        && fs::read_to_string(&version_file).map(|v| v.trim() == updater::embedded_version()).unwrap_or(false);
    if up_to_date {
        return Ok(());
    }

    fs::create_dir_all(registry::data_dir()).map_err(|e| e.to_string())?;
    fs::copy(&current, &target).map_err(|e| e.to_string())?;
    fs::write(&version_file, updater::embedded_version()).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn enable() -> bool {
    if let Err(error) = refresh_exe() {
        println!("❌ Impossible de copier l'installeur : {}", error);
        return false;
    }

    let script = format!(
        "' Flocord : répare Flocord si Discord s'est mis à jour\r\nWScript.Sleep 20000\r\nCreateObject(\"WScript.Shell\").Run \"\"\"{}\"\" --repair --silent\", 0, False\r\n",
        installed_exe().display()
    );

    match fs::write(launcher(), script) {
        Ok(_) => {
            logger::write("Protection automatique activée");
            println!("✔ Protection automatique activée : Flocord sera réparé à chaque démarrage de Windows si besoin.");
            true
        }
        Err(error) => {
            println!("❌ Impossible de créer le lanceur : {}", error);
            false
        }
    }
}

pub fn disable() -> bool {
    match fs::remove_file(launcher()) {
        Ok(_) => {
            logger::write("Protection automatique désactivée");
            println!("✔ Protection automatique désactivée.");
            true
        }
        Err(_) if !is_enabled() => {
            println!("✔ La protection automatique n'était pas activée.");
            true
        }
        Err(error) => {
            println!("❌ Impossible de retirer le lanceur : {}", error);
            false
        }
    }
}

/// Si la protection est active, garde sa copie de l'installeur à jour (appelé à chaque lancement normal)
pub fn refresh_if_enabled() {
    if is_enabled() {
        let _ = refresh_exe();
    }
}

/// Exécution silencieuse au démarrage : répare uniquement ce qui doit l'être.
pub fn run_silent() {
    logger::write("Protection automatique : vérification");
    let marked = registry::marked();

    for entry in status::all() {
        if !marked.iter().any(|c| c == &entry.client.channel) {
            continue;
        }
        if entry.state.is_installed() {
            logger::write(&format!("{} : Flocord OK", entry.client.name));
            continue;
        }

        logger::write(&format!("{} : {} → réparation", entry.client.name, strip_ansi(&entry.state.label())));

        let was_running = process::is_process_running(&entry.client.path);
        if was_running {
            process::close_discord(&entry.client.path);
            std::thread::sleep(std::time::Duration::from_secs(2));
        }

        let ok = repair::repair(&entry.client);

        if was_running && ok {
            process::launch_discord(&entry.client.path, &entry.client.executable);
            logger::write(&format!("{} relancé", entry.client.name));
        }
    }
}

fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for n in chars.by_ref() {
                if n == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}
