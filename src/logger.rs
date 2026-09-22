// Journal dans "Flocord Logs\installer.log" sur le Bureau (c'est le fichier demandé sur le serveur support).
// Le journal est conservé entre les lancements et tourne au-delà de 512 Ko.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_SIZE: u64 = 512 * 1024;

pub fn log_folder() -> PathBuf {
    let profile = std::env::var("USERPROFILE").unwrap_or_default();
    PathBuf::from(profile).join("Desktop").join("Flocord Logs")
}

pub fn log_file() -> PathBuf {
    log_folder().join("installer.log")
}

pub fn init() {
    let _ = fs::create_dir_all(log_folder());

    let file = log_file();
    if fs::metadata(&file).map(|m| m.len() > MAX_SIZE).unwrap_or(false) {
        let _ = fs::rename(&file, log_folder().join("installer.old.log"));
    }

    let args: Vec<String> = std::env::args().skip(1).collect();
    write(&format!("Flocord Installer v{} (Flocord v{}) démarré {}", crate::updater::cli_version(), crate::updater::embedded_version(), args.join(" ")));
}

pub fn write(message: &str) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_file()) {
        let _ = writeln!(file, "[{}] {}", timestamp(), message);
    }
}

/// Date et heure UTC lisibles, sans dépendance
fn timestamp() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;

    // Algorithme "civil from days" (Howard Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC", y, m, d, rem / 3600, (rem % 3600) / 60, rem % 60)
}
