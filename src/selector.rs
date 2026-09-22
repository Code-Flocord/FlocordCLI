use crate::client::DiscordClient;
use crate::status::ClientStatus;
use crate::ui::{self, DIM, RESET, VIOLET};

/// Choix d'un Discord parmi plusieurs, avec l'état de Flocord sur chacun
pub fn select(entries: &[ClientStatus]) -> Option<DiscordClient> {
    println!();
    for (index, entry) in entries.iter().enumerate() {
        println!(
            "  {}[{}]{} {:<14}{}{:<14}{} {}",
            VIOLET,
            index + 1,
            RESET,
            entry.client.name,
            DIM,
            entry.client.version,
            RESET,
            entry.state.label()
        );
    }
    println!("  {}[0]{} Retour", VIOLET, RESET);
    println!();

    let choice: usize = ui::prompt("> ").parse().ok()?;
    if choice == 0 || choice > entries.len() {
        return None;
    }

    Some(entries[choice - 1].client.clone())
}
