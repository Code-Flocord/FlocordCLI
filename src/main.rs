use std::process::Command;
use std::sync::mpsc;

mod args;
mod autorepair;
mod backup;
mod client;
mod detect;
mod discord;
mod installer;
mod logger;
mod openasar;
mod openasar_detect;
mod process;
mod registry;
mod repair;
mod selector;
mod selfupdate;
mod status;
mod ui;
mod uninstall;
mod updater;
mod version;

use args::Action;
use client::DiscordClient;
use ui::{DIM, RESET, VIOLET};

const SUPPORT_URL: &str = "https://discord.gg/CH45T3PH5r";

fn main() {
    selfupdate::cleanup();
    ui::enable_ansi();
    logger::init();

    let args = args::parse();

    match args.action {
        Some(Action::Help) => println!("{}", args::help()),
        Some(action) => headless(action, &args),
        None => interactive(),
    }
}

// ---------- Mode ligne de commande ----------

fn headless(action: Action, args: &args::Args) {
    match action {
        Action::Status => {
            for entry in status::all() {
                println!("{} ({}) : {}", entry.client.name, entry.client.channel, entry.state.label());
            }
        }
        Action::EnableProtection => {
            autorepair::enable();
        }
        Action::DisableProtection => {
            autorepair::disable();
        }
        Action::Repair if args.silent && args.channel.is_none() => autorepair::run_silent(),
        Action::Install | Action::Repair | Action::Uninstall => {
            let clients: Vec<DiscordClient> = discord::find_discord()
                .into_iter()
                .filter(|c| args.channel.as_deref().map(|ch| c.channel.to_lowercase() == ch).unwrap_or(true))
                .collect();

            if clients.is_empty() {
                println!("Aucun Discord trouvé.");
                return;
            }

            for client in clients {
                run_action(action, &client, args.silent);
            }
        }
        Action::Help => {}
    }
}

/// Ferme Discord si besoin, exécute l'action, relance Discord. Retourne true si l'action a réussi.
fn run_action(action: Action, client: &DiscordClient, silent: bool) -> bool {
    let Some(was_running) = ensure_closed(client, silent) else {
        println!("Opération annulée.");
        return false;
    };

    let ok = match action {
        Action::Install => installer::install(client, false),
        Action::Repair => repair::repair(client),
        Action::Uninstall => uninstall::uninstall(client),
        _ => false,
    };

    if !ok {
        println!();
        println!("❌ L'opération a échoué. Journal : {}", logger::log_file().display());
        return false;
    }

    if silent {
        if was_running {
            process::launch_discord(&client.path, &client.executable);
        }
    } else if ui::confirm(&format!("Relancer {} maintenant ?", client.name)) {
        if process::launch_discord(&client.path, &client.executable) {
            println!("✔ {} relancé.", client.name);
        } else {
            println!("❌ Impossible de relancer {}.", client.name);
        }
    }

    true
}

/// S'assure que Discord est fermé. Some(true) s'il tournait, Some(false) sinon, None si l'utilisateur annule.
fn ensure_closed(client: &DiscordClient, silent: bool) -> Option<bool> {
    if !process::is_process_running(&client.path) {
        return Some(false);
    }

    if !silent {
        println!();
        println!("⚠ {} est ouvert et doit être fermé pour continuer.", client.name);
        if !ui::confirm(&format!("Fermer {} maintenant ?", client.name)) {
            return None;
        }
    }

    process::close_discord(&client.path);
    std::thread::sleep(std::time::Duration::from_secs(2));

    if process::is_process_running(&client.path) {
        println!("❌ {} est toujours ouvert. Fermez-le manuellement puis réessayez.", client.name);
        return None;
    }

    println!("✔ {} fermé.", client.name);
    Some(true)
}

// ---------- Menu interactif ----------

fn interactive() {
    autorepair::refresh_if_enabled();

    // La vérification de mise à jour se fait en arrière-plan pour ne pas ralentir l'ouverture du menu
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(selfupdate::available());
    });
    let mut update_check: Option<Option<String>> = None;

    loop {
        if update_check.is_none() {
            if let Ok(result) = receiver.try_recv() {
                update_check = Some(result);
            }
        }
        let newer = update_check.clone().flatten();

        ui::clear_screen();
        ui::banner(&updater::embedded_version());
        overview();

        println!();
        match &newer {
            Some(v) => println!("  {}⬆ Installeur v{} disponible (choix 9){}", VIOLET, v, RESET),
            None if update_check.is_none() => println!("  {}Vérification des mises à jour...{}", DIM, RESET),
            None => println!("  {}Installeur à jour{}", DIM, RESET),
        }

        let protection = if autorepair::is_enabled() { "\x1b[32mactivée\x1b[0m" } else { "\x1b[90mdésactivée\x1b[0m" };

        println!();
        println!("  {}[1]{} Installer Flocord", VIOLET, RESET);
        println!("  {}[2]{} Réparer Flocord", VIOLET, RESET);
        println!("  {}[3]{} Désinstaller Flocord", VIOLET, RESET);
        println!("  {}[4]{} Protection automatique ({})", VIOLET, RESET, protection);
        println!("  {}[5]{} Installer OpenAsar", VIOLET, RESET);
        println!("  {}[6]{} Désinstaller OpenAsar", VIOLET, RESET);
        println!("  {}[7]{} Serveur support Discord", VIOLET, RESET);
        println!("  {}[8]{} Ouvrir le journal", VIOLET, RESET);
        if newer.is_some() {
            println!("  {}[9]{} Mettre à jour l'installeur", VIOLET, RESET);
        }
        println!("  {}[0]{} Quitter", VIOLET, RESET);
        println!();

        match ui::prompt("> ").as_str() {
            "1" => with_client("Installation", |c| run_action(Action::Install, c, false)),
            "2" => with_client("Réparation", |c| run_action(Action::Repair, c, false)),
            "3" => with_client("Désinstallation", |c| run_action(Action::Uninstall, c, false)),
            "4" => toggle_protection(),
            "5" => with_client("OpenAsar", |c| {
                ensure_closed(c, false).is_some() && {
                    openasar::install(c);
                    true
                }
            }),
            "6" => with_client("OpenAsar", |c| {
                ensure_closed(c, false).is_some() && {
                    openasar::uninstall(c);
                    true
                }
            }),
            "7" => open(SUPPORT_URL),
            "8" => open(&logger::log_file().to_string_lossy()),
            "9" if newer.is_some() => {
                selfupdate::run(newer.as_deref().unwrap_or_default());
                ui::pause();
            }
            "0" | "q" => break,
            _ => {}
        }
    }
}

fn overview() {
    ui::section("Discord détectés");

    let entries = status::all();
    if entries.is_empty() {
        println!("  Aucun Discord trouvé dans %LOCALAPPDATA%.");
        return;
    }

    for entry in entries {
        println!(
            "  {:<14}{}{:<14}{} {}",
            format!("{}", entry.client.name),
            DIM,
            entry.client.version,
            RESET,
            entry.state.label()
        );
    }
}

fn with_client(title: &str, action: impl Fn(&DiscordClient) -> bool) {
    ui::section(title);

    let entries = status::all();
    let client = match entries.len() {
        0 => {
            println!("  Aucun Discord trouvé.");
            ui::pause();
            return;
        }
        1 => entries.into_iter().next().map(|e| e.client),
        _ => selector::select(&entries),
    };

    let Some(client) = client else {
        return;
    };

    action(&client);
    ui::pause();
}

fn toggle_protection() {
    ui::section("Protection automatique");
    if autorepair::is_enabled() {
        println!("  Au démarrage de Windows, Flocord est réparé automatiquement si Discord s'est mis à jour.");
        if ui::confirm("Désactiver la protection automatique ?") {
            autorepair::disable();
        }
    } else {
        println!("  Quand Discord se met à jour, il efface Flocord. La protection le réinstalle");
        println!("  toute seule au démarrage de Windows, sans rien demander.");
        if registry::marked().is_empty() {
            println!();
            println!("  ⚠ Installez d'abord Flocord : la protection ne s'applique qu'aux Discord installés par cet outil.");
        }
        if ui::confirm("Activer la protection automatique ?") {
            autorepair::enable();
        }
    }
    ui::pause();
}

fn open(target: &str) {
    let _ = Command::new("cmd").args(["/C", "start", "", target]).spawn();
}
