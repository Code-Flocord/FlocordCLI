#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod say;

mod args;
mod autorepair;
mod backup;
mod cli;
mod client;
mod detect;
mod discord;
mod gui;
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

fn main() {
    selfupdate::cleanup();
    let args = args::parse();

    // Double-clic : interface graphique. Avec des arguments (ou --cli) : console.
    if args.action.is_none() && !args.cli {
        logger::init();
        gui::run();
        return;
    }

    // En mode silencieux (protection automatique au démarrage) aucune console ne doit apparaître
    if !args.silent {
        ui::attach_console();
        ui::enable_ansi();
    }
    logger::init();
    cli::run(args);
}
