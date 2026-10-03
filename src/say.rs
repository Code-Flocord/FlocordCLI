// Sortie des modules métier, écrite sur la console.

use std::io::{self, Write};

pub fn say(message: impl Into<String>) {
    println!("{}", message.into());
}

/// Barre de progression sur la console
pub fn progress(fraction: f32, label: String) {
    let filled = (fraction * 25.0) as usize;
    print!("\r  {} [{}{}] {:>3}%", label, "█".repeat(filled), "░".repeat(25 - filled), (fraction * 100.0) as u32);
    let _ = io::stdout().flush();
}

pub fn progress_done() {
    println!();
}

/// Retire les séquences de couleur ANSI (pour le journal)
pub fn strip_ansi(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for n in chars.by_ref() {
                if n == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[macro_export]
macro_rules! say {
    ($($arg:tt)*) => { $crate::say::say(format!($($arg)*)) };
}
