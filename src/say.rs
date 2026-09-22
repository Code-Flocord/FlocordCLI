// Sortie des modules métier : écrite sur la console et, quand l'interface graphique écoute, envoyée à sa file.

use std::io::{self, Write};
use std::sync::mpsc::Sender;
use std::sync::Mutex;

pub enum Step {
    Log(String),
    /// Progression d'un téléchargement (0..=1) et libellé
    Progress(f32, String),
}

static SINK: Mutex<Option<Sender<Step>>> = Mutex::new(None);

pub fn set_sink(sender: Option<Sender<Step>>) {
    *SINK.lock().unwrap() = sender;
}

fn gui_listening() -> bool {
    SINK.lock().unwrap().is_some()
}

pub fn say(message: impl Into<String>) {
    let message = message.into();
    if let Some(sender) = SINK.lock().unwrap().as_ref() {
        let _ = sender.send(Step::Log(message.clone()));
    }
    println!("{}", message);
}

/// Progression : barre sur la console, événement pour l'interface
pub fn progress(fraction: f32, label: String) {
    if let Some(sender) = SINK.lock().unwrap().as_ref() {
        let _ = sender.send(Step::Progress(fraction, label));
        return;
    }
    {
        let filled = (fraction * 25.0) as usize;
        print!("\r  {} [{}{}] {:>3}%", label, "█".repeat(filled), "░".repeat(25 - filled), (fraction * 100.0) as u32);
        let _ = io::stdout().flush();
    }
}

pub fn progress_done() {
    if !gui_listening() {
        println!();
    }
}

/// Retire les séquences de couleur ANSI (pour le journal et l'interface graphique)
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
