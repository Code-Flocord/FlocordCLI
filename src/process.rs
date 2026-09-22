use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Commande console lancée sans fenêtre : l'installeur est une application fenêtrée,
/// sans ce drapeau chaque tasklist / taskkill / powershell ferait apparaître une console.
pub fn hidden(program: &str) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

/// Chemin entre apostrophes pour une commande PowerShell. Une apostrophe dans le chemin
/// (C:\Users\O'Neil) se double, sinon elle fermerait la chaîne.
pub fn ps_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

/// Ouvre une URL ou un fichier avec l'application par défaut, sans console
pub fn open(target: &str) {
    let _ = hidden("cmd").args(["/C", "start", "", target]).spawn();
}

/// Le processus porte le nom du dossier d'installation : Discord.exe, DiscordPTB.exe,
/// DiscordCanary.exe, DiscordDevelopment.exe
fn get_process_name(path: &PathBuf) -> String {
    let folder = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();

    if folder.to_lowercase().starts_with("discord") {
        return format!("{}.exe", folder);
    }

    "Discord.exe".to_string()
}

pub fn is_process_running(discord_path: &PathBuf) -> bool {
    let process_name = get_process_name(discord_path);

    let output = hidden("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {}", process_name)])
        .output();

    match output {
        Ok(output) => {
            let result = String::from_utf8_lossy(&output.stdout).to_lowercase();

            result.contains(&process_name.to_lowercase())
        }

        Err(_) => false,
    }
}

pub fn launch_discord(discord_path: &PathBuf, executable: &PathBuf) -> bool {
    let updater = discord_path.join("Update.exe");

    let result = if updater.exists() {
        Command::new(&updater)
            .args(["--processStart", &get_process_name(discord_path)])
            .spawn()
    } else {
        Command::new(executable).spawn()
    };

    result.is_ok()
}

pub fn close_discord(discord_path: &PathBuf) -> bool {
    let process_name = get_process_name(discord_path);

    println!("Fermeture du processus : {}", process_name);

    let result = hidden("taskkill")
        .args(["/IM", &process_name, "/F"])
        .output();

    match result {
        Ok(output) => output.status.success(),

        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::ps_quote;
    use std::path::Path;

    #[test]
    fn quotes_paths_for_powershell() {
        assert_eq!(ps_quote(Path::new(r"C:\Users\Sk\app.asar")), r"'C:\Users\Sk\app.asar'");
        assert_eq!(ps_quote(Path::new(r"C:\Users\O'Neil\app.asar")), r"'C:\Users\O''Neil\app.asar'");
    }
}
