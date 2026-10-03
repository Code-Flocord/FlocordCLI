use crate::client::DiscordClient;
use crate::status::ClientStatus;
use crate::ui::{self, DIM, RESET, VIOLET};

/// Choix d'un ou plusieurs Discord parmi ceux détectés, avec l'état de Flocord sur chacun
pub fn select(entries: &[ClientStatus]) -> Option<Vec<DiscordClient>> {
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
    if entries.len() > 1 {
        println!("  {}[A]{} Tous les Discord", VIOLET, RESET);
    }
    println!("  {}[0]{} Retour", VIOLET, RESET);
    println!();

    let input = ui::prompt("> ");
    if entries.len() > 1 && input.eq_ignore_ascii_case("a") {
        return Some(entries.iter().map(|e| e.client.clone()).collect());
    }

    let choice: usize = input.parse().ok()?;
    if choice == 0 || choice > entries.len() {
        return None;
    }

    Some(vec![entries[choice - 1].client.clone()])
}
