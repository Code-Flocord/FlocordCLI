// Pas de windows_subsystem = "windows" ici : c'était nécessaire quand cet exécutable pouvait lancer
// une interface graphique (sans console) ; maintenant qu'il est purement console, le garder forçait
// un rattachement manuel au terminal (AttachConsole/AllocConsole) fragile — notamment avec PowerShell,
// qui ne considère pas un exécutable "fenêtré" comme bloquant et rend la main avant que la console ne
// soit vraiment prête, si bien qu'une touche tapée juste après filait au prompt PowerShell plutôt qu'à
// nous. En sous-système console normal, Windows attache stdin/stdout correctement dès le départ, et
// PowerShell/cmd attendent la fin du programme comme pour n'importe quel outil en ligne de commande.
// Le lancement silencieux au démarrage de Windows (protection automatique) reste sans fenêtre visible :
// c'est le script .vbs qui le lance avec windowStyle=0, indépendamment du sous-système de l'exe.

#[macro_use]
mod say;

mod args;
mod autorepair;
mod backup;
mod cli;
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

fn main() {
    selfupdate::cleanup();
    let args = args::parse();

    ui::enable_ansi();
    logger::init();
    cli::run(args);
}
