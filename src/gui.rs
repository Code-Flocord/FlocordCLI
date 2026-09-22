// Interface graphique : assistant en étapes (Discord → action → exécution → résultat) dans une fenêtre
// sans bordure au verre acrylique, avec un encart latéral (support, GitHub, journal, protection).

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Layout, Margin, Pos2, Rect, RichText, Sense, Shadow, Stroke,
    StrokeKind, Vec2, ViewportBuilder, ViewportCommand,
};

use crate::args::Action;
use crate::client::DiscordClient;
use crate::say::{self, Step};
use crate::status::{self, State};
use crate::{autorepair, cli, installer, logger, process, repair, selfupdate, uninstall, updater};

// ---------- Palette ----------
const ACCENT: Color32 = Color32::from_rgb(139, 92, 246);
const ACCENT_LIGHT: Color32 = Color32::from_rgb(196, 181, 253);
const TEXT: Color32 = Color32::from_rgb(236, 233, 246);
const MUTED: Color32 = Color32::from_rgb(168, 159, 196);
const OK: Color32 = Color32::from_rgb(74, 222, 128);
const WARN: Color32 = Color32::from_rgb(251, 191, 36);
const DANGER: Color32 = Color32::from_rgb(248, 113, 113);

const WINDOW: Vec2 = Vec2::new(880.0, 540.0);
const SIDEBAR: f32 = 236.0;
const RADIUS: u8 = 16;

fn glass(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(28, 20, 44, alpha)
}

fn accent_alpha(alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(139, 92, 246, alpha)
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

/// Une ligne de journal : couleur selon le préfixe, texte sans le pictogramme (absent des polices embarquées)
fn classify(line: &str) -> (Color32, String) {
    for (prefix, color) in [("❌", DANGER), ("⚠", WARN), ("✔", OK)] {
        if let Some(rest) = line.strip_prefix(prefix) {
            return (color, rest.trim_start().to_string());
        }
    }
    (MUTED, line.to_string())
}

// ---------- État ----------
struct Run {
    title: String,
    lines: Vec<String>,
    progress: Option<(f32, String)>,
    rx: Receiver<Step>,
    done_rx: Receiver<(bool, bool)>,
    started: Instant,
}

struct Outcome {
    title: String,
    lines: Vec<String>,
    ok: bool,
    relaunch: Option<DiscordClient>,
}

enum Screen {
    PickClient,
    PickAction,
    Running(Run),
    Done(Outcome),
}

pub struct App {
    entries: Vec<status::ClientStatus>,
    selected: Option<usize>,
    action: Option<Action>,
    screen: Screen,
    protection: bool,
    newer: Option<Option<String>>,
    update_rx: Receiver<Option<String>>,
    toast: Option<(String, Instant)>,
    /// Discord ouvert ? (client, résultat, date) — rafraîchi toutes les 2 s, jamais à chaque image
    running_cache: Option<(String, bool, Instant)>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Flou du bureau derrière la fenêtre (Windows 10 1809+ ; simple flou sinon)
        if window_vibrancy::apply_acrylic(cc, Some((20, 14, 32, 140))).is_err() {
            let _ = window_vibrancy::apply_blur(cc, Some((20, 14, 32, 200)));
        }

        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::TRANSPARENT;
        visuals.window_fill = Color32::TRANSPARENT;
        visuals.override_text_color = Some(TEXT);
        visuals.widgets.noninteractive.bg_stroke = Stroke::NONE;
        cc.egui_ctx.set_visuals(visuals);

        let (tx, update_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(selfupdate::available());
        });

        autorepair::refresh_if_enabled();

        let entries = status::all();
        Self {
            selected: if entries.len() == 1 { Some(0) } else { None },
            entries,
            action: None,
            screen: Screen::PickClient,
            protection: autorepair::is_enabled(),
            newer: None,
            update_rx,
            toast: None,
            running_cache: None,
        }
    }

    fn refresh(&mut self) {
        self.entries = status::all();
        self.protection = autorepair::is_enabled();
        if self.selected.map(|i| i >= self.entries.len()).unwrap_or(true) {
            self.selected = if self.entries.len() == 1 { Some(0) } else { None };
        }
    }

    fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    /// Vérifie si le Discord choisi tourne, au plus une fois toutes les 2 secondes (tasklist est coûteux)
    fn is_running(&mut self, client: &DiscordClient) -> bool {
        if let Some((name, running, at)) = &self.running_cache {
            if name == &client.name && at.elapsed() < Duration::from_secs(2) {
                return *running;
            }
        }
        let running = process::is_process_running(&client.path);
        self.running_cache = Some((client.name.clone(), running, Instant::now()));
        running
    }

    fn client(&self) -> Option<&DiscordClient> {
        self.selected.and_then(|i| self.entries.get(i)).map(|e| &e.client)
    }

    /// Lance l'action choisie dans un thread ; Discord est fermé si besoin (l'utilisateur a été prévenu).
    fn start(&mut self, ctx: &egui::Context) {
        let (Some(client), Some(action)) = (self.client().cloned(), self.action) else { return; };

        let title = match action {
            Action::Install => format!("Installation sur {}", client.name),
            Action::Repair => format!("Réparation de {}", client.name),
            Action::Uninstall => format!("Désinstallation de {}", client.name),
            _ => client.name.clone(),
        };

        let (tx, rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        say::set_sink(Some(tx));

        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let was_running = process::is_process_running(&client.path);
            if was_running {
                say!("Fermeture de {}...", client.name);
                process::close_discord(&client.path);
                std::thread::sleep(Duration::from_secs(2));
            }

            let ok = match action {
                Action::Install => installer::install(&client, false),
                Action::Repair => repair::repair(&client),
                Action::Uninstall => uninstall::uninstall(&client),
                _ => false,
            };

            say::set_sink(None);
            let _ = done_tx.send((ok, was_running));
            ctx.request_repaint();
        });

        self.screen = Screen::Running(Run { title, lines: Vec::new(), progress: None, rx, done_rx, started: Instant::now() });
    }

    fn poll(&mut self, ctx: &egui::Context) {
        // Test automatisé : FLOCORD_AUTOSTART=repair:PTB déroule l'assistant tout seul
        if let Ok(spec) = std::env::var("FLOCORD_AUTOSTART") {
            unsafe { std::env::remove_var("FLOCORD_AUTOSTART") };
            if let Some((action, channel)) = spec.split_once(':') {
                self.selected = self.entries.iter().position(|e| e.client.channel.eq_ignore_ascii_case(channel));
                if action == "pick" {
                    self.screen = Screen::PickAction;
                } else {
                    self.action = Some(match action { "install" => Action::Install, "uninstall" => Action::Uninstall, _ => Action::Repair });
                    if self.selected.is_some() {
                        self.start(ctx);
                    }
                }
            }
        }

        if self.newer.is_none() {
            if let Ok(result) = self.update_rx.try_recv() {
                self.newer = Some(result);
            }
        }

        let mut outcome: Option<Outcome> = None;
        let current = self.client().cloned();
        if let Screen::Running(run) = &mut self.screen {
            while let Ok(step) = run.rx.try_recv() {
                match step {
                    Step::Log(line) => {
                        let line = say::strip_ansi(&line);
                        if !line.trim().is_empty() {
                            run.lines.push(line);
                        }
                    }
                    Step::Progress(fraction, label) => run.progress = Some((fraction, label)),
                }
            }
            if let Ok((ok, was_running)) = run.done_rx.try_recv() {
                let relaunch = if was_running && ok { current.clone() } else { None };
                outcome = Some(Outcome { title: run.title.clone(), lines: run.lines.clone(), ok, relaunch });
            }
            ctx.request_repaint_after(Duration::from_millis(80));
        }
        if let Some(outcome) = outcome {
            self.screen = Screen::Done(outcome);
            self.refresh();
        }
    }
}

// ---------- Rendu ----------
impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &root.ctx().clone();
        self.poll(ctx);

        let frame = egui::Frame::new()
            .fill(Color32::from_rgba_unmultiplied(22, 15, 38, 215))
            .stroke(Stroke::new(1.0, accent_alpha(90)))
            .corner_radius(CornerRadius::same(RADIUS))
            .inner_margin(Margin::same(0));

        egui::CentralPanel::default().frame(frame).show(root, |ui| {
            let full = ui.max_rect();
            paint_backdrop(ui, full);

            self.titlebar(ui, ctx, full);

            let body = Rect::from_min_max(full.min + Vec2::new(0.0, 46.0), full.max);
            let side = Rect::from_min_max(body.min, Pos2::new(body.min.x + SIDEBAR, body.max.y));
            let main = Rect::from_min_max(Pos2::new(side.max.x, body.min.y), body.max);

            let mut side_ui = ui.new_child(egui::UiBuilder::new().max_rect(side).layout(Layout::top_down(Align::Min)));
            self.sidebar(&mut side_ui);

            let mut main_ui = ui.new_child(egui::UiBuilder::new().max_rect(main.shrink2(Vec2::new(24.0, 6.0))).layout(Layout::top_down(Align::Min)));
            match &self.screen {
                Screen::PickClient => self.pick_client(&mut main_ui),
                Screen::PickAction => self.pick_action(&mut main_ui, ctx),
                Screen::Running(_) => self.running(&mut main_ui, ctx),
                Screen::Done(_) => self.done(&mut main_ui, ctx),
            }

            self.toast_overlay(ui, full);
        });
    }
}

fn paint_backdrop(ui: &mut egui::Ui, rect: Rect) {
    let painter = ui.painter().with_clip_rect(rect);
    radial_glow(&painter, rect.left_top() + Vec2::new(40.0, 10.0), 380.0, accent_alpha(85));
    radial_glow(&painter, rect.right_bottom() - Vec2::new(90.0, 30.0), 320.0, Color32::from_rgba_unmultiplied(217, 70, 239, 50));
}

/// Dégradé radial doux (éventail de triangles du centre coloré vers un bord transparent)
fn radial_glow(painter: &egui::Painter, center: Pos2, radius: f32, color: Color32) {
    let mut mesh = egui::Mesh::default();
    let steps = 64u32;
    mesh.colored_vertex(center, color);
    for i in 0..=steps {
        let angle = i as f32 / steps as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(center + Vec2::angled(angle) * radius, Color32::TRANSPARENT);
    }
    for i in 1..=steps {
        mesh.add_triangle(0, i, i + 1);
    }
    painter.add(egui::Shape::mesh(mesh));
}

impl App {
    fn titlebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, full: Rect) {
        let rect = Rect::from_min_size(full.min, Vec2::new(full.width(), 46.0));
        let response = ui.interact(rect, ui.id().with("titlebar"), Sense::click_and_drag());
        if response.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        let painter = ui.painter();
        paint_logo(painter, rect.left_center() + Vec2::new(30.0, 0.0), 11.0);
        painter.text(rect.left_center() + Vec2::new(48.0, 0.0), Align2::LEFT_CENTER, "Flocord Installer", FontId::proportional(14.0), TEXT);
        painter.text(rect.left_center() + Vec2::new(174.0, 1.0), Align2::LEFT_CENTER, format!("v{}  ·  Flocord v{}", updater::cli_version(), updater::embedded_version()), FontId::monospace(11.0), MUTED);

        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(Layout::right_to_left(Align::Center)));
        child.add_space(10.0);
        if window_button(&mut child, true, DANGER).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if window_button(&mut child, false, MUTED).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
    }

    /// Encart latéral : liens, protection automatique, mise à jour de l'installeur
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        egui::Frame::new()
            .fill(glass(120))
            .stroke(Stroke::new(1.0, accent_alpha(50)))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin::same(16))
            .outer_margin(Margin { left: 18, right: 6, top: 4, bottom: 18 })
            .show(ui, |ui| {
                ui.set_width(SIDEBAR - 24.0 - 32.0);
                ui.set_min_height(ui.available_height() - 22.0);

                ui.label(RichText::new("FLOCORD").size(11.0).strong().color(ACCENT_LIGHT));
                ui.label(RichText::new("Client Discord modifié : thème Glass, 350+ plugins, mises à jour automatiques.").size(11.5).color(MUTED));
                ui.add_space(14.0);

                side_link(ui, "Serveur support", "Aide, FAQ, annonces", cli::SUPPORT_URL);
                side_link(ui, "GitHub", "Code source et tickets", "https://github.com/Code-Flocord/Flocord");
                side_link(ui, "Journal", "installer.log sur le Bureau", &logger::log_file().to_string_lossy());

                ui.add_space(14.0);
                divider(ui);
                ui.add_space(12.0);

                ui.label(RichText::new("Protection automatique").size(13.0).strong().color(TEXT));
                ui.label(RichText::new("Réinstalle Flocord au démarrage de Windows quand une mise à jour Discord l'a effacé.").size(11.0).color(MUTED));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let mut on = self.protection;
                    if toggle(ui, &mut on).changed() {
                        let ok = if on { autorepair::enable() } else { autorepair::disable() };
                        if ok {
                            self.protection = on;
                            self.toast(if on { "Protection automatique activée" } else { "Protection automatique désactivée" });
                        } else {
                            self.toast("Impossible de modifier la protection (voir le journal)");
                        }
                    }
                    ui.add_space(4.0);
                    ui.label(RichText::new(if self.protection { "Activée" } else { "Désactivée" }).size(12.0).color(if self.protection { OK } else { MUTED }));
                });

                ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                    match &self.newer {
                        Some(Some(version)) => {
                            let version = version.clone();
                            if pill_button(ui, &format!("Mettre à jour (v{})", version), ACCENT, true).clicked() {
                                let (tx, _rx) = mpsc::channel();
                                say::set_sink(Some(tx));
                                let ok = selfupdate::run(&version);
                                say::set_sink(None);
                                if !ok {
                                    self.toast("Mise à jour de l'installeur impossible");
                                }
                            }
                            ui.label(RichText::new("Nouvelle version de l'installeur").size(11.0).color(WARN));
                        }
                        Some(None) => {
                            ui.label(RichText::new("Installeur à jour").size(11.0).color(MUTED));
                        }
                        None => {
                            ui.label(RichText::new("Vérification des mises à jour…").size(11.0).color(MUTED));
                            ui.ctx().request_repaint_after(Duration::from_millis(300));
                        }
                    }
                });
            });
    }

    // ----- Étape 1 : choisir le Discord -----
    fn pick_client(&mut self, ui: &mut egui::Ui) {
        steps_header(ui, 0);
        ui.label(RichText::new("Quel Discord ?").size(20.0).strong().color(TEXT));
        ui.label(RichText::new("Flocord s'installe sur la version la plus récente du client choisi.").size(12.0).color(MUTED));
        ui.add_space(12.0);

        let mut clicked: Option<usize> = None;
        egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show(ui, |ui| {
            for (index, entry) in self.entries.iter().enumerate() {
                let selected = self.selected == Some(index);
                let response = option_card(ui, selected, true, |ui| {
                    ui.horizontal(|ui| {
                        let (dot, _) = ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 19.0, accent_alpha(if selected { 90 } else { 40 }));
                        ui.painter().text(dot.center(), Align2::CENTER_CENTER, channel_initial(&entry.client.channel), FontId::proportional(15.0), if selected { Color32::WHITE } else { ACCENT_LIGHT });
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&entry.client.name).size(15.0).strong().color(TEXT));
                                ui.label(RichText::new(&entry.client.version).size(11.5).color(MUTED).monospace());
                            });
                            state_pill(ui, &entry.state);
                        });
                    });
                });
                if response.clicked() {
                    clicked = Some(index);
                }
                ui.add_space(8.0);
            }

            if self.entries.is_empty() {
                egui::Frame::new().fill(glass(90)).corner_radius(CornerRadius::same(14)).inner_margin(Margin::same(20)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Aucun Discord trouvé. Installe Discord (Stable, PTB ou Canary) puis relance cet installeur.").color(MUTED));
                });
            }
        });
        if let Some(index) = clicked {
            self.selected = Some(index);
        }

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.set_min_width(ui.available_width());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if pill_button_enabled(ui, "Continuer", ACCENT, true, self.selected.is_some()).clicked() {
                        self.action = None;
                        self.screen = Screen::PickAction;
                    }
                });
            });
        });
    }

    // ----- Étape 2 : choisir l'action -----
    fn pick_action(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        steps_header(ui, 1);
        let Some(entry) = self.selected.and_then(|i| self.entries.get(i)) else {
            self.screen = Screen::PickClient;
            return;
        };
        let client = entry.client.clone();
        let state_installed = entry.state.is_installed();
        let needs_repair = matches!(entry.state, State::Lost(_) | State::Relay);
        let has_flocord = state_installed || needs_repair;

        ui.label(RichText::new(format!("Que faire sur {} ?", client.name)).size(20.0).strong().color(TEXT));
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Discord {}", client.version)).size(12.0).color(MUTED));
            state_pill(ui, &entry.state);
        });
        ui.add_space(12.0);

        let options: [(Action, &str, &str, bool, bool); 3] = [
            (Action::Install, "Installer Flocord", if state_installed { "Déjà installé sur ce Discord." } else { "Sauvegarde le Discord original, puis installe la dernière version de Flocord." }, !state_installed, !has_flocord),
            (Action::Repair, "Réparer", if needs_repair { "Recommandé : remet Flocord en place sur la version actuelle de Discord." } else { "Réinstalle proprement Flocord, utile si quelque chose ne va pas." }, has_flocord, needs_repair),
            (Action::Uninstall, "Désinstaller", "Retire Flocord et restaure le Discord d'origine à l'identique.", has_flocord, false),
        ];

        let mut pick: Option<Action> = None;
        for (action, title, description, enabled, recommended) in options {
            let selected = self.action == Some(action);
            let response = option_card(ui, selected, enabled, |ui| {
                ui.horizontal(|ui| {
                    radio_dot(ui, selected, enabled);
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(title).size(14.5).strong().color(if enabled { TEXT } else { MUTED }));
                            if recommended && enabled {
                                tag(ui, "Recommandé", OK);
                            }
                        });
                        ui.label(RichText::new(description).size(11.5).color(MUTED));
                    });
                });
            });
            if response.clicked() && enabled {
                pick = Some(action);
            }
            ui.add_space(8.0);
        }
        if let Some(action) = pick {
            self.action = Some(action);
        }

        let running = self.is_running(&client);
        ui.ctx().request_repaint_after(Duration::from_secs(2));

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.set_min_width(ui.available_width());
                if pill_button(ui, "Retour", MUTED, false).clicked() {
                    self.screen = Screen::PickClient;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let label = if running { "Fermer Discord et lancer" } else { "Lancer" };
                    if pill_button_enabled(ui, label, ACCENT, true, self.action.is_some()).clicked() {
                        self.start(ctx);
                    }
                });
            });
            if running {
                ui.add_space(6.0);
                ui.label(RichText::new(format!("{} est ouvert : il sera fermé pendant l'opération, tu pourras le relancer à la fin.", client.name)).size(11.5).color(WARN));
            }
        });
    }

    // ----- Étape 3 : exécution -----
    fn running(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        steps_header(ui, 2);
        let Screen::Running(run) = &self.screen else { return; };
        ui.horizontal(|ui| {
            spinner(ui, run.started.elapsed().as_secs_f32());
            ui.label(RichText::new(&run.title).size(20.0).strong().color(TEXT));
        });
        ui.add_space(12.0);
        log_lines(ui, &run.lines, 9);
        if let Some((fraction, label)) = &run.progress {
            ui.add_space(10.0);
            ui.label(RichText::new(label).size(11.5).color(MUTED));
            progress_bar(ui, *fraction);
        }
        ctx.request_repaint_after(Duration::from_millis(60));
    }

    // ----- Étape 4 : résultat -----
    fn done(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        steps_header(ui, 3);
        let Screen::Done(outcome) = &self.screen else { return; };
        ui.horizontal(|ui| {
            result_icon(ui, outcome.ok);
            ui.label(RichText::new(if outcome.ok { format!("{} : terminé", outcome.title) } else { format!("{} : échec", outcome.title) }).size(20.0).strong().color(TEXT));
        });
        ui.add_space(12.0);
        log_lines(ui, &outcome.lines, 8);
        if !outcome.ok {
            ui.add_space(6.0);
            ui.label(RichText::new("Le détail est dans le journal (encart de gauche). Le serveur support peut t'aider avec ce fichier.").size(11.5).color(WARN));
        }

        let relaunch = outcome.relaunch.clone();
        let mut back = false;
        let mut launched: Option<DiscordClient> = None;

        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.set_min_width(ui.available_width());
                if pill_button(ui, "Retour au début", MUTED, false).clicked() {
                    back = true;
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if let Some(client) = &relaunch {
                        if pill_button(ui, &format!("Relancer {}", client.name), ACCENT, true).clicked() {
                            launched = Some(client.clone());
                        }
                    }
                });
            });
        });

        if let Some(client) = launched {
            process::launch_discord(&client.path, &client.executable);
            self.toast(format!("{} relancé", client.name));
            if let Screen::Done(outcome) = &mut self.screen {
                outcome.relaunch = None;
            }
        }
        if back {
            self.action = None;
            self.screen = Screen::PickClient;
        }
    }

    fn toast_overlay(&mut self, ui: &mut egui::Ui, full: Rect) {
        let Some((text, since)) = &self.toast else { return; };
        if since.elapsed() > Duration::from_secs(3) {
            self.toast = None;
            return;
        }
        let painter = ui.painter();
        let galley = painter.layout_no_wrap(text.clone(), FontId::proportional(12.5), TEXT);
        let size = galley.size() + Vec2::new(28.0, 16.0);
        let rect = Rect::from_center_size(Pos2::new(full.center().x + SIDEBAR / 2.0, full.bottom() - 30.0), size);
        painter.rect(rect, CornerRadius::same(20), glass(235), Stroke::new(1.0, accent_alpha(120)), StrokeKind::Inside);
        painter.galley(rect.min + Vec2::new(14.0, 8.0), galley, TEXT);
        ui.ctx().request_repaint_after(Duration::from_millis(200));
    }
}

// ---------- Widgets ----------
const STEPS: [&str; 4] = ["Discord", "Action", "Exécution", "Terminé"];

fn steps_header(ui: &mut egui::Ui, current: usize) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        for (i, name) in STEPS.iter().enumerate() {
            let active = i == current;
            let past = i < current;
            let color = if active { ACCENT_LIGHT } else if past { OK } else { MUTED };
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
            ui.painter().circle(dot.center(), 8.0, if active { ACCENT } else if past { with_alpha(OK, 60) } else { Color32::TRANSPARENT }, Stroke::new(1.5, color));
            ui.painter().text(dot.center(), Align2::CENTER_CENTER, (i + 1).to_string(), FontId::proportional(10.0), if active { Color32::WHITE } else { color });
            ui.label(RichText::new(*name).size(11.5).color(color));
            if i + 1 < STEPS.len() {
                let (line, _) = ui.allocate_exact_size(Vec2::new(28.0, 2.0), Sense::hover());
                ui.painter().rect_filled(line, 1.0, if past { with_alpha(OK, 120) } else { with_alpha(MUTED, 60) });
            }
        }
    });
    ui.add_space(14.0);
}

/// Carte cliquable : bordure violette quand sélectionnée, grisée quand désactivée
fn option_card(ui: &mut egui::Ui, selected: bool, enabled: bool, content: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let stroke = if selected { Stroke::new(1.5, ACCENT) } else { Stroke::new(1.0, accent_alpha(45)) };
    let fill = if selected { with_alpha(ACCENT, 34) } else { glass(if enabled { 120 } else { 60 }) };

    let response = egui::Frame::new()
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::symmetric(16, 12))
        .shadow(if selected { Shadow { offset: [0, 6], blur: 20, spread: 0, color: accent_alpha(70) } } else { Shadow::NONE })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            content(ui);
        })
        .response;

    let response = ui.interact(response.rect, response.id.with("card"), if enabled { Sense::click() } else { Sense::hover() });
    if enabled && response.hovered() && !selected {
        ui.painter().rect_stroke(response.rect, CornerRadius::same(14), Stroke::new(1.0, accent_alpha(120)), StrokeKind::Inside);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    response
}

fn radio_dot(ui: &mut egui::Ui, selected: bool, enabled: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
    let color = if !enabled { with_alpha(MUTED, 90) } else if selected { ACCENT } else { MUTED };
    ui.painter().circle_stroke(rect.center(), 8.0, Stroke::new(1.5, color));
    if selected {
        ui.painter().circle_filled(rect.center(), 4.5, ACCENT);
    }
}

fn tag(ui: &mut egui::Ui, text: &str, color: Color32) {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(10.5), color);
    let (rect, _) = ui.allocate_exact_size(galley.size() + Vec2::new(14.0, 6.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(10), with_alpha(color, 34));
    ui.painter().galley(rect.min + Vec2::new(7.0, 3.0), galley, color);
}

fn side_link(ui: &mut egui::Ui, title: &str, subtitle: &str, target: &str) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 42.0), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(10), accent_alpha(26));
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let color = if response.hovered() { ACCENT_LIGHT } else { TEXT };
    ui.painter().text(rect.min + Vec2::new(10.0, 12.0), Align2::LEFT_CENTER, title, FontId::proportional(13.0), color);
    ui.painter().text(rect.min + Vec2::new(10.0, 29.0), Align2::LEFT_CENTER, subtitle, FontId::proportional(10.5), MUTED);
    ui.painter().text(rect.right_center() - Vec2::new(10.0, 0.0), Align2::RIGHT_CENTER, "›", FontId::proportional(16.0), MUTED);
    if response.clicked() {
        open(target);
    }
}

fn divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, accent_alpha(50));
}

fn paint_logo(painter: &egui::Painter, center: Pos2, radius: f32) {
    painter.circle_filled(center, radius, ACCENT);
    let s = radius * 2.0;
    let origin = center - Vec2::splat(radius);
    let bar = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(origin + Vec2::new(x0 * s, y0 * s), origin + Vec2::new(x1 * s, y1 * s));
    painter.rect_filled(bar(0.30, 0.22, 0.44, 0.80), 0.0, Color32::WHITE);
    painter.rect_filled(bar(0.30, 0.22, 0.74, 0.35), 0.0, Color32::WHITE);
    painter.rect_filled(bar(0.30, 0.47, 0.66, 0.59), 0.0, Color32::WHITE);
}

fn window_button(ui: &mut egui::Ui, close: bool, hover: Color32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(34.0, 28.0), Sense::click());
    let color = if response.hovered() { hover } else { MUTED };
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(8), accent_alpha(30));
    }
    let c = rect.center();
    let stroke = Stroke::new(1.4, color);
    if close {
        ui.painter().line_segment([c + Vec2::new(-4.5, -4.5), c + Vec2::new(4.5, 4.5)], stroke);
        ui.painter().line_segment([c + Vec2::new(-4.5, 4.5), c + Vec2::new(4.5, -4.5)], stroke);
    } else {
        ui.painter().line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(5.0, 0.0)], stroke);
    }
    response
}

fn pill_button(ui: &mut egui::Ui, text: &str, color: Color32, primary: bool) -> egui::Response {
    pill_button_enabled(ui, text, color, primary, true)
}

fn pill_button_enabled(ui: &mut egui::Ui, text: &str, color: Color32, primary: bool, enabled: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(12.5), TEXT);
    let size = galley.size() + Vec2::new(26.0, 14.0);
    let (rect, response) = ui.allocate_exact_size(size, if enabled { Sense::click() } else { Sense::hover() });
    let hovered = enabled && response.hovered();

    let (fill, stroke, fg) = if !enabled {
        (with_alpha(MUTED, 22), Stroke::new(1.0, with_alpha(MUTED, 60)), with_alpha(MUTED, 140))
    } else if primary {
        let fill = if hovered { ACCENT_LIGHT.linear_multiply(0.9) } else { color };
        (fill, Stroke::new(1.0, with_alpha(ACCENT_LIGHT, 150)), Color32::WHITE)
    } else {
        (with_alpha(color, if hovered { 60 } else { 28 }), Stroke::new(1.0, with_alpha(color, 140)), if hovered { TEXT } else { color.lerp_to_gamma(TEXT, 0.35) })
    };

    if primary && enabled {
        ui.painter().rect_filled(rect.translate(Vec2::new(0.0, 4.0)), CornerRadius::same(20), accent_alpha(if hovered { 90 } else { 60 }));
        ui.painter().rect(rect, CornerRadius::same(20), fill, stroke, StrokeKind::Inside);
        ui.painter().rect_filled(Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.center().y)), CornerRadius { nw: 20, ne: 20, sw: 0, se: 0 }, Color32::from_rgba_unmultiplied(255, 255, 255, 18));
    } else {
        ui.painter().rect(rect, CornerRadius::same(20), fill, stroke, StrokeKind::Inside);
    }
    ui.painter().galley(rect.min + Vec2::new(13.0, 7.0), galley, fg);
    ui.add_space(8.0);
    if enabled { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response }
}

fn toggle(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(44.0, 24.0), Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let t = ui.ctx().animate_bool(response.id, *on);
    let fill = Color32::from_rgba_unmultiplied(70, 60, 100, 200).lerp_to_gamma(ACCENT, t);
    ui.painter().rect_filled(rect, CornerRadius::same(12), fill);
    let knob = Pos2::new(rect.left() + 12.0 + t * 20.0, rect.center().y);
    ui.painter().circle_filled(knob, 9.0, Color32::WHITE);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn state_pill(ui: &mut egui::Ui, state: &State) {
    let (text, color) = match state {
        State::Installed(v) => (format!("Flocord v{} installé", v), OK),
        State::Lost(v) => (format!("Perdu après une mise à jour Discord (était sur {})", v), WARN),
        State::Relay => ("Mode relais — réparation conseillée".to_string(), WARN),
        State::NotInstalled => ("Non installé".to_string(), MUTED),
        State::Unknown => ("État inconnu".to_string(), MUTED),
    };
    let galley = ui.painter().layout_no_wrap(text, FontId::proportional(11.5), color);
    let (rect, _) = ui.allocate_exact_size(galley.size() + Vec2::new(22.0, 8.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(12), with_alpha(color, 34));
    ui.painter().circle_filled(rect.left_center() + Vec2::new(9.0, 0.0), 3.0, color);
    ui.painter().galley(rect.min + Vec2::new(16.0, 4.0), galley, color);
}

fn log_lines(ui: &mut egui::Ui, lines: &[String], max: usize) {
    egui::Frame::new().fill(Color32::from_rgba_unmultiplied(8, 5, 14, 150)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(170.0);
        let start = lines.len().saturating_sub(max);
        for line in &lines[start..] {
            let (color, text) = classify(line);
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 3.0, color);
                ui.label(RichText::new(text).size(12.0).monospace().color(color));
            });
        }
    });
}

fn progress_bar(ui: &mut egui::Ui, fraction: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 8.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same(4), Color32::from_rgba_unmultiplied(255, 255, 255, 20));
    let filled = Rect::from_min_size(rect.min, Vec2::new(rect.width() * fraction.clamp(0.0, 1.0), rect.height()));
    ui.painter().rect_filled(filled, CornerRadius::same(4), ACCENT);
}

/// Coche verte ou croix rouge dessinée au trait
fn result_icon(ui: &mut egui::Ui, ok: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
    let c = rect.center();
    let color = if ok { OK } else { DANGER };
    ui.painter().circle_filled(c, 12.0, with_alpha(color, 40));
    let stroke = Stroke::new(2.0, color);
    if ok {
        ui.painter().line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(-1.5, 3.5)], stroke);
        ui.painter().line_segment([c + Vec2::new(-1.5, 3.5), c + Vec2::new(5.0, -4.0)], stroke);
    } else {
        ui.painter().line_segment([c + Vec2::new(-4.0, -4.0), c + Vec2::new(4.0, 4.0)], stroke);
        ui.painter().line_segment([c + Vec2::new(-4.0, 4.0), c + Vec2::new(4.0, -4.0)], stroke);
    }
}

fn spinner(ui: &mut egui::Ui, t: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
    let center = rect.center();
    let n = 12;
    for i in 0..n {
        let angle = i as f32 / n as f32 * std::f32::consts::TAU - t * 4.0;
        let fade = ((i as f32 / n as f32) * 255.0) as u8;
        let p = center + Vec2::angled(angle) * 8.0;
        ui.painter().circle_filled(p, 2.2, Color32::from_rgba_unmultiplied(196, 181, 253, fade.max(30)));
    }
}

fn channel_initial(channel: &str) -> &'static str {
    match channel {
        "PTB" => "P",
        "Canary" => "C",
        _ => "S",
    }
}

fn open(target: &str) {
    process::open(target);
}

fn icon() -> egui::IconData {
    let size = 64usize;
    let mut rgba = vec![0u8; size * size * 4];
    let c = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 + 0.5 - c;
            let dy = y as f32 + 0.5 - c;
            let inside = dx * dx + dy * dy <= (c - 1.0) * (c - 1.0);
            let u = (x as f32 + 0.5) / size as f32;
            let v = (y as f32 + 0.5) / size as f32;
            let f = (0.30..=0.44).contains(&u) && (0.22..=0.80).contains(&v)
                || (0.30..=0.74).contains(&u) && (0.22..=0.35).contains(&v)
                || (0.30..=0.66).contains(&u) && (0.47..=0.59).contains(&v);
            let i = (y * size + x) * 4;
            if inside {
                let (r, g, b) = if f { (255, 255, 255) } else { (139, 92, 246) };
                rgba[i..i + 4].copy_from_slice(&[r, g, b, 255]);
            }
        }
    }
    egui::IconData { rgba, width: size as u32, height: size as u32 }
}

pub fn run() {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("Flocord Installer")
            .with_inner_size(WINDOW)
            .with_min_inner_size(WINDOW)
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(true)
            .with_icon(Arc::new(icon())),
        ..Default::default()
    };

    if let Err(error) = eframe::run_native("Flocord Installer", options, Box::new(|cc| Ok(Box::new(App::new(cc))))) {
        logger::write(&format!("Interface graphique impossible : {}", error));
        // Sans interface graphique (pilote ou session distante), retour au menu console
        cli::interactive();
    }
}
