// Arguments en ligne de commande : sans argument, le menu interactif s'affiche.

#[derive(Clone, Copy, PartialEq)]
pub enum Action {
    Install,
    Repair,
    Uninstall,
    Status,
    EnableProtection,
    DisableProtection,
    Help,
}

pub struct Args {
    pub action: Option<Action>,
    /// Aucune question : Discord est fermé puis relancé automatiquement si besoin.
    pub silent: bool,
    /// Canal ciblé (stable / ptb / canary). Sans canal : tous les Discord trouvés.
    pub channel: Option<String>,
    /// Menu console au lieu de l'interface graphique
    pub cli: bool,
}

pub fn parse() -> Args {
    let mut args = Args { action: None, silent: false, channel: None, cli: false };
    let mut list = std::env::args().skip(1);

    while let Some(arg) = list.next() {
        match arg.as_str() {
            "--install" => args.action = Some(Action::Install),
            "--repair" => args.action = Some(Action::Repair),
            "--uninstall" => args.action = Some(Action::Uninstall),
            "--status" => args.action = Some(Action::Status),
            "--enable-protection" => args.action = Some(Action::EnableProtection),
            "--disable-protection" => args.action = Some(Action::DisableProtection),
            "--silent" | "-s" => args.silent = true,
            "--channel" | "-c" => args.channel = list.next().map(|c| c.to_lowercase()),
            "--cli" => args.cli = true,
            "--help" | "-h" | "/?" => args.action = Some(Action::Help),
            _ => {}
        }
    }

    args
}

pub fn help() -> &'static str {
    "Utilisation : FlocordCLI.exe [action] [options]

Actions :
  --install              Installe Flocord
  --repair               Répare (réinstalle) Flocord
  --uninstall            Désinstalle Flocord et restaure Discord
  --status               Affiche l'état de chaque Discord
  --enable-protection    Active la réparation automatique au démarrage de Windows
  --disable-protection   Désactive la réparation automatique

Options :
  --channel <stable|ptb|canary>   Cible un seul Discord (sinon : tous)
  --silent, -s                    Aucune question, Discord est fermé et relancé si besoin

Sans argument : interface graphique. --cli : menu console."
}
