// État de Flocord sur chaque Discord détecté.

use crate::client::DiscordClient;
use crate::detect;
use crate::discord;

pub enum State {
    Installed(String),
    /// Discord s'est mis à jour : Flocord était sur l'ancienne version, plus sur la nouvelle
    Lost(String),
    /// Le relais interne de Flocord a patché le nouveau dossier (dossier app/ + _app.asar) : à consolider
    Relay,
    NotInstalled,
    Unknown,
}

pub struct ClientStatus {
    pub client: DiscordClient,
    pub state: State,
}

impl State {
    pub fn is_installed(&self) -> bool {
        matches!(self, State::Installed(_))
    }

    pub fn label(&self) -> String {
        match self {
            State::Installed(v) => format!("\x1b[32mFlocord v{} installé\x1b[0m", v),
            State::Lost(v) => format!("\x1b[33m⚠ Flocord perdu après une mise à jour Discord (était sur {})\x1b[0m", v),
            State::Relay => "\x1b[33m⚠ Flocord en mode relais, réparation conseillée\x1b[0m".to_string(),
            State::NotInstalled => "\x1b[90mFlocord non installé\x1b[0m".to_string(),
            State::Unknown => "\x1b[90mÉtat inconnu\x1b[0m".to_string(),
        }
    }
}

pub fn of(client: &DiscordClient) -> State {
    let Some(info) = detect::detect(client) else {
        return State::Unknown;
    };

    if info.installed {
        return State::Installed(info.flocord_version.unwrap_or_else(|| "?".to_string()));
    }

    if info.app_asar.is_dir() && info.original_asar.exists() {
        return State::Relay;
    }

    match detect::previous_install(client) {
        Some(previous) => State::Lost(previous),
        None => State::NotInstalled,
    }
}

pub fn all() -> Vec<ClientStatus> {
    discord::find_discord()
        .into_iter()
        .map(|client| ClientStatus { state: of(&client), client })
        .collect()
}
