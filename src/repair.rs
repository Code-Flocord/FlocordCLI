use crate::client::DiscordClient;
use crate::installer;
use crate::logger;

/// Réparer = réinstaller proprement sur le dossier Discord le plus récent, quel que soit l'état actuel.
pub fn repair(client: &DiscordClient) -> bool {
    let ok = installer::install(client, true);
    logger::write(&format!("Réparation {} : {}", client.name, if ok { "OK" } else { "échec" }));
    ok
}
