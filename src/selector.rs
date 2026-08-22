use std::io::{self, Write};

use crate::client::DiscordClient;

pub fn select_discord(targets: &Vec<DiscordClient>) -> Option<DiscordClient> {
    println!();
    println!("================================");
    println!("     Clients Discord disponibles");
    println!("================================");
    println!();

    for (index, client) in targets.iter().enumerate() {
        println!("[{}] {}", index + 1, client.name);

        println!("    Canal : {}", client.channel);

        println!("    Version : {}", client.version);

        println!();
    }

    println!("[0] Retour");
    println!();

    print!("Choix : ");
    io::stdout().flush().unwrap();

    let mut input = String::new();

    io::stdin().read_line(&mut input).expect("Erreur lecture");

    let choice: usize = match input.trim().parse() {
        Ok(value) => value,

        Err(_) => {
            println!("Choix invalide.");

            return None;
        }
    };

    if choice == 0 || choice > targets.len() {
        return None;
    }

    Some(targets[choice - 1].clone())
}
