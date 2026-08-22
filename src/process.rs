use std::path::PathBuf;
use std::process::Command;

fn get_process_name(path: &PathBuf) -> String {
    let path_string = path.to_string_lossy().to_lowercase();

    if path_string.contains("discordptb") {
        return "DiscordPTB.exe".to_string();
    }

    if path_string.contains("discordcanary") {
        return "DiscordCanary.exe".to_string();
    }

    if path_string.contains("\\discord\\") {
        return "Discord.exe".to_string();
    }

    "Discord.exe".to_string()
}

pub fn is_process_running(discord_path: &PathBuf) -> bool {
    let process_name = get_process_name(discord_path);

    let output = Command::new("tasklist")
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

pub fn close_discord(discord_path: &PathBuf) -> bool {
    let process_name = get_process_name(discord_path);

    println!("Fermeture du processus : {}", process_name);

    let result = Command::new("taskkill")
        .args(["/IM", &process_name, "/F"])
        .output();

    match result {
        Ok(output) => output.status.success(),

        Err(_) => false,
    }
}
