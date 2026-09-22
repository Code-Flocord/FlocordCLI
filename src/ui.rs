// Petites briques d'affichage et de saisie du menu.

use std::io::{self, Write};

pub const VIOLET: &str = "\x1b[35m";
pub const DIM: &str = "\x1b[90m";
pub const RESET: &str = "\x1b[0m";

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetStdHandle(handle: u32) -> *mut std::ffi::c_void;
    fn GetConsoleMode(handle: *mut std::ffi::c_void, mode: *mut u32) -> i32;
    fn SetConsoleMode(handle: *mut std::ffi::c_void, mode: u32) -> i32;
}

/// Active les séquences ANSI (couleurs) dans la console classique de Windows
pub fn enable_ansi() {
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

    unsafe {
        let handle = GetStdHandle(STD_OUTPUT_HANDLE);
        let mut mode = 0u32;
        if !handle.is_null() && GetConsoleMode(handle, &mut mode) != 0 {
            SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}

pub fn clear_screen() {
    print!("\x1B[2J\x1B[1;1H");
}

pub fn banner(version: &str) {
    println!("{}", VIOLET);
    println!(
        r#"▄▀▀▀█ ▀     ▄▀▀▀▄ ▄▀▀▀█ ▄▀▀▀▄ ▀▀▀▀▄ ▀▀▀▀▄
▄▀▀   █   ▄ ▄   █ ▄   ▄ ▄   █ █▀▀▀▄ █   █
▀      ▀▀▀▀  ▀▀▀   ▀▀▀▀  ▀▀▀  ▀   ▀ ▀▀▀▀"#
    );
    println!("{}", RESET);
    println!("  Flocord Installer v{}   {}github.com/Code-Flocord{}", version, DIM, RESET);
    println!("{}", rule());
}

pub fn rule() -> String {
    format!("{}{}{}", DIM, "─".repeat(52), RESET)
}

pub fn section(title: &str) {
    println!();
    println!("{}── {} {}", VIOLET, title, RESET);
}

pub fn read_line() -> String {
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
    input.trim().to_string()
}

pub fn prompt(text: &str) -> String {
    print!("{}", text);
    let _ = io::stdout().flush();
    read_line()
}

pub fn confirm(question: &str) -> bool {
    println!();
    println!("{}", question);
    println!("{}[1]{} Oui    {}[2]{} Non", VIOLET, RESET, VIOLET, RESET);
    prompt("> ") == "1"
}

pub fn pause() {
    println!();
    prompt(&format!("{}Entrée pour revenir au menu{}", DIM, RESET));
}
