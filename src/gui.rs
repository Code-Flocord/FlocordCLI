// Interface graphique : fenêtre sans bordure, flou acrylique du bureau, panneaux violets translucides.

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Id, Layout, Margin, Pos2, Rect, RichText, Sense, Shadow,
    Stroke, StrokeKind, Vec2, ViewportBuilder, ViewportCommand,
};

use crate::args::Action;
use crate::client::DiscordClient;
use crate::say::{self, Step};
use crate::status::{self, State};
use crate::{autorepair, cli, installer, logger, process, repair, selfupdate, uninstall, updater};

// ---------- Palette ----------
const ACCENT: Color32 = Color32::from_rgb(139, 92, 246);
const ACCENT_DARK: Color32 = Color32::from_rgb(109, 40, 217);
const ACCENT_LIGHT: Color32 = Color32::from_rgb(196, 181, 253);
const TEXT: Color32 = Color32::from_rgb(236, 233, 246);
const MUTED: Color32 = Color32::from_rgb(168, 159, 196);
const OK: Color32 = Color32::from_rgb(74, 222, 128);
const WARN: Color32 = Color32::from_rgb(251, 191, 36);
const DANGER: Color32 = Color32::from_rgb(248, 113, 113);

const WINDOW: Vec2 = Vec2::new(780.0, 484.0);
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
enum Job {
    Running { title: String, lines: Vec<String>, progress: Option<(f32, String)>, rx: Receiver<Step>, done_rx: Receiver<(bool, bool)>, started: Instant },
    Finished { title: String, lines: Vec<String>, ok: bool, offer_relaunch: Option<DiscordClient> },
}

enum Dialog {
    CloseDiscord { action: Action, client: DiscordClient },
}

pub struct App {
    entries: Vec<status::ClientStatus>,
    protection: bool,
    newer: Option<Option<String>>,
    update_rx: Receiver<Option<String>>,
    job: Option<Job>,
    dialog: Option<Dialog>,
    toast: Option<(String, Instant)>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Flou du bureau derrière la fenêtre (Windows 10 1809+ ; Mica ou opaque sinon)
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

        Self {
            entries: status::all(),
            protection: autorepair::is_enabled(),
            newer: None,
            update_rx,
            job: None,
            dialog: None,
            toast: None,
        }
    }

    fn refresh(&mut self) {
        self.entries = status::all();
        self.protection = autorepair::is_enabled();
    }

    fn toast(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    /// Lance une action dans un thread ; Discord est fermé sans question (le dialogue a déjà eu lieu).
    fn start(&mut self, ctx: &egui::Context, action: Action, client: DiscordClient) {
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

        self.job = Some(Job::Running { title, lines: Vec::new(), progress: None, rx, done_rx, started: Instant::now() });
    }

    fn request(&mut self, ctx: &egui::Context, action: Action, client: DiscordClient) {
        if process::is_process_running(&client.path) {
            self.dialog = Some(Dialog::CloseDiscord { action, client });
        } else {
            self.start(ctx, action, client);
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        // Test automatisé : FLOCORD_AUTOSTART=repair:PTB lance l'action au premier rendu
        if let Ok(spec) = std::env::var("FLOCORD_AUTOSTART") {
            unsafe { std::env::remove_var("FLOCORD_AUTOSTART") };
            if let Some((action, channel)) = spec.split_once(':') {
                let action = match action { "install" => Action::Install, "uninstall" => Action::Uninstall, _ => Action::Repair };
                if let Some(entry) = self.entries.iter().find(|e| e.client.channel.eq_ignore_ascii_case(channel)) {
                    let client = entry.client.clone();
                    self.request(ctx, action, client);
                }
            }
        }

        if self.newer.is_none() {
            if let Ok(result) = self.update_rx.try_recv() {
                self.newer = Some(result);
            }
        }

        let mut finished: Option<Job> = None;
        if let Some(Job::Running { title, lines, progress, rx, done_rx, .. }) = &mut self.job {
            while let Ok(step) = rx.try_recv() {
                match step {
                    Step::Log(line) => {
                        let line = say::strip_ansi(&line);
                        if !line.trim().is_empty() {
                            lines.push(line);
                        }
                    }
                    Step::Progress(fraction, label) => *progress = Some((fraction, label)),
                }
            }
            if let Ok((ok, was_running)) = done_rx.try_recv() {
                let client = if was_running && ok { self.entries.iter().find(|e| title.ends_with(&e.client.name)).map(|e| e.client.clone()) } else { None };
                finished = Some(Job::Finished { title: title.clone(), lines: lines.clone(), ok, offer_relaunch: client });
            }
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        if let Some(job) = finished {
            self.job = Some(job);
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

            ui.vertical(|ui| {
                self.titlebar(ui, ctx);
                ui.add_space(6.0);
                egui::Frame::new().inner_margin(Margin::symmetric(24, 4)).show(ui, |ui| {
                    self.header(ui, ctx);
                    ui.add_space(14.0);
                    self.clients(ui, ctx);
                    ui.add_space(14.0);
                    self.footer(ui, ctx);
                });
            });

            if self.job.is_some() {
                self.job_overlay(ui, ctx, full);
            }
            if self.dialog.is_some() {
                self.dialog_overlay(ui, ctx, full);
            }
            self.toast_overlay(ui, full);
        });
    }
}

fn paint_backdrop(ui: &mut egui::Ui, rect: Rect) {
    let painter = ui.painter().with_clip_rect(rect);
    // Halo violet en haut à gauche et rosé en bas à droite, comme le thème du client
    radial_glow(&painter, rect.left_top() + Vec2::new(60.0, 20.0), 360.0, accent_alpha(80));
    radial_glow(&painter, rect.right_bottom() - Vec2::new(100.0, 40.0), 300.0, Color32::from_rgba_unmultiplied(217, 70, 239, 50));
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
    fn titlebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 44.0), Sense::click_and_drag());
        if response.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        let painter = ui.painter();
        paint_logo(painter, rect.left_center() + Vec2::new(28.0, 0.0), 11.0);
        painter.text(rect.left_center() + Vec2::new(46.0, 0.0), Align2::LEFT_CENTER, "Flocord Installer", FontId::proportional(14.0), TEXT);
        painter.text(rect.left_center() + Vec2::new(172.0, 1.0), Align2::LEFT_CENTER, format!("v{}", updater::embedded_version()), FontId::monospace(11.0), MUTED);

        // Boutons fenêtre
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(Layout::right_to_left(Align::Center)));
        child.add_space(10.0);
        if window_button(&mut child, true, DANGER).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if window_button(&mut child, false, MUTED).clicked() {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
    }

    fn header(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Discord détectés").size(20.0).strong().color(TEXT));
                let subtitle = match self.entries.len() {
                    0 => "Aucun Discord trouvé dans ce compte Windows.".to_string(),
                    n => format!("{} client{} · Flocord s'installe sur la version la plus récente de chacun.", n, if n > 1 { "s" } else { "" }),
                };
                ui.label(RichText::new(subtitle).size(12.5).color(MUTED));
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                match &self.newer {
                    Some(Some(version)) => {
                        let version = version.clone();
                        if pill_button(ui, &format!("Mettre à jour l'installeur (v{})", version), ACCENT, true).clicked() {
                            let (tx, _rx) = mpsc::channel();
                            say::set_sink(Some(tx));
                            let ok = selfupdate::run(&version);
                            say::set_sink(None);
                            if !ok {
                                self.toast("Mise à jour de l'installeur impossible");
                            }
                        }
                    }
                    Some(None) => {
                        ui.label(RichText::new("Installeur à jour").size(12.0).color(MUTED));
                    }
                    None => {
                        ui.label(RichText::new("Vérification…").size(12.0).color(MUTED));
                        ctx.request_repaint_after(Duration::from_millis(300));
                    }
                }
            });
        });
    }

    fn clients(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut requests: Vec<(Action, DiscordClient)> = Vec::new();

        egui::ScrollArea::vertical().max_height(290.0).auto_shrink([false, true]).show(ui, |ui| {
            for entry in &self.entries {
                let card = egui::Frame::new()
                    .fill(glass(120))
                    .stroke(Stroke::new(1.0, accent_alpha(60)))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(Margin::symmetric(18, 14))
                    .shadow(Shadow { offset: [0, 8], blur: 24, spread: 0, color: Color32::from_rgba_unmultiplied(20, 8, 40, 90) });

                card.show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        // Pastille canal
                        let (dot, _) = ui.allocate_exact_size(Vec2::splat(38.0), Sense::hover());
                        ui.painter().circle_filled(dot.center(), 19.0, accent_alpha(40));
                        ui.painter().text(dot.center(), Align2::CENTER_CENTER, channel_initial(&entry.client.channel), FontId::proportional(15.0), ACCENT_LIGHT);
                        ui.add_space(8.0);

                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&entry.client.name).size(15.5).strong().color(TEXT));
                                ui.label(RichText::new(&entry.client.version).size(11.5).color(MUTED).monospace());
                            });
                            state_pill(ui, &entry.state);
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let client = entry.client.clone();
                            match &entry.state {
                                State::Installed(_) => {
                                    if pill_button(ui, "Désinstaller", DANGER, false).clicked() {
                                        requests.push((Action::Uninstall, client.clone()));
                                    }
                                    if pill_button(ui, "Réparer", ACCENT, false).clicked() {
                                        requests.push((Action::Repair, client));
                                    }
                                }
                                State::Lost(_) | State::Relay => {
                                    if pill_button(ui, "Désinstaller", DANGER, false).clicked() {
                                        requests.push((Action::Uninstall, client.clone()));
                                    }
                                    if pill_button(ui, "Réparer", ACCENT, true).clicked() {
                                        requests.push((Action::Repair, client));
                                    }
                                }
                                State::NotInstalled | State::Unknown => {
                                    if pill_button(ui, "Installer Flocord", ACCENT, true).clicked() {
                                        requests.push((Action::Install, client));
                                    }
                                }
                            }
                        });
                    });
                });
                ui.add_space(10.0);
            }

            if self.entries.is_empty() {
                egui::Frame::new().fill(glass(90)).corner_radius(CornerRadius::same(14)).inner_margin(Margin::same(24)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("Installe Discord (Stable, PTB ou Canary) puis relance cet installeur.").color(MUTED));
                });
            }
        });

        for (action, client) in requests {
            self.request(ctx, action, client);
        }
    }

    fn footer(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        egui::Frame::new()
            .fill(glass(100))
            .stroke(Stroke::new(1.0, accent_alpha(45)))
            .corner_radius(CornerRadius::same(14))
            .inner_margin(Margin::symmetric(18, 12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
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
                    ui.add_space(6.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Protection automatique").size(13.5).strong().color(TEXT));
                        ui.label(RichText::new("Répare Flocord au démarrage de Windows quand Discord l'a effacé.").size(11.5).color(MUTED));
                    });

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if link_button(ui, "Journal").clicked() {
                            open(&logger::log_file().to_string_lossy());
                        }
                        if link_button(ui, "GitHub").clicked() {
                            open("https://github.com/Code-Flocord/Flocord");
                        }
                        if link_button(ui, "Serveur support").clicked() {
                            open(cli::SUPPORT_URL);
                        }
                    });
                });
            });
    }

    fn job_overlay(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, full: Rect) {
        ui.painter().rect_filled(full, CornerRadius::same(RADIUS), Color32::from_rgba_unmultiplied(10, 6, 18, 170));

        let panel = Rect::from_center_size(full.center(), Vec2::new(540.0, 320.0));
        let mut close = false;
        let mut relaunch: Option<DiscordClient> = None;

        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(panel).layout(Layout::top_down(Align::Min)));
        egui::Frame::new()
            .fill(glass(235))
            .stroke(Stroke::new(1.0, accent_alpha(110)))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(Margin::same(22))
            .shadow(Shadow { offset: [0, 16], blur: 48, spread: 0, color: Color32::from_rgba_unmultiplied(60, 20, 120, 120) })
            .show(&mut child, |ui| {
                ui.set_width(panel.width() - 44.0);
                ui.set_min_height(panel.height() - 44.0);

                match self.job.as_ref().unwrap() {
                    Job::Running { title, lines, progress, started, .. } => {
                        ui.horizontal(|ui| {
                            spinner(ui, started.elapsed().as_secs_f32());
                            ui.label(RichText::new(title).size(17.0).strong().color(TEXT));
                        });
                        ui.add_space(10.0);
                        log_lines(ui, lines, 7);
                        if let Some((fraction, label)) = progress {
                            ui.add_space(10.0);
                            ui.label(RichText::new(label).size(11.5).color(MUTED));
                            progress_bar(ui, *fraction);
                        }
                        ctx.request_repaint_after(Duration::from_millis(60));
                    }
                    Job::Finished { title, lines, ok, offer_relaunch } => {
                        ui.horizontal(|ui| {
                            result_icon(ui, *ok);
                            ui.label(RichText::new(title).size(17.0).strong().color(TEXT));
                        });
                        ui.add_space(10.0);
                        log_lines(ui, lines, 7);
                        if !*ok {
                            ui.add_space(6.0);
                            ui.label(RichText::new("Le détail est dans le journal (bouton Journal en bas).").size(11.5).color(WARN));
                        }
                        ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
                            ui.horizontal(|ui| {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if let Some(client) = offer_relaunch {
                                        if pill_button(ui, &format!("Relancer {}", client.name), ACCENT, true).clicked() {
                                            relaunch = Some(client.clone());
                                        }
                                    }
                                    if pill_button(ui, "Fermer", MUTED, false).clicked() {
                                        close = true;
                                    }
                                });
                            });
                        });
                    }
                }
            });

        if let Some(client) = relaunch {
            process::launch_discord(&client.path, &client.executable);
            self.toast(format!("{} relancé", client.name));
            close = true;
        }
        if close {
            self.job = None;
        }
    }

    fn dialog_overlay(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, full: Rect) {
        ui.painter().rect_filled(full, CornerRadius::same(RADIUS), Color32::from_rgba_unmultiplied(10, 6, 18, 150));
        let panel = Rect::from_center_size(full.center(), Vec2::new(440.0, 170.0));
        let mut decision: Option<bool> = None;

        let (client_name, ) = match self.dialog.as_ref().unwrap() {
            Dialog::CloseDiscord { client, .. } => (client.name.clone(),),
        };

        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(panel).layout(Layout::top_down(Align::Min)));
        egui::Frame::new()
            .fill(glass(240))
            .stroke(Stroke::new(1.0, accent_alpha(110)))
            .corner_radius(CornerRadius::same(16))
            .inner_margin(Margin::same(22))
            .show(&mut child, |ui| {
                ui.set_width(panel.width() - 44.0);
                ui.set_min_height(panel.height() - 44.0);
                ui.label(RichText::new(format!("{} est ouvert", client_name)).size(17.0).strong().color(TEXT));
                ui.add_space(6.0);
                ui.label(RichText::new("Il doit être fermé pour continuer. Il sera relancé à la fin si tu le souhaites.").size(12.5).color(MUTED));
                ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if pill_button(ui, "Fermer Discord et continuer", ACCENT, true).clicked() {
                                decision = Some(true);
                            }
                            if pill_button(ui, "Annuler", MUTED, false).clicked() {
                                decision = Some(false);
                            }
                        });
                    });
                });
            });

        if let Some(go) = decision {
            if let Some(Dialog::CloseDiscord { action, client }) = self.dialog.take() {
                if go {
                    self.start(ctx, action, client);
                }
            }
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
        let rect = Rect::from_center_size(Pos2::new(full.center().x, full.bottom() - 34.0), size);
        painter.rect(rect, CornerRadius::same(20), glass(230), Stroke::new(1.0, accent_alpha(120)), StrokeKind::Inside);
        painter.galley(rect.min + Vec2::new(14.0, 8.0), galley, TEXT);
        ui.ctx().request_repaint_after(Duration::from_millis(200));
    }
}

// ---------- Widgets ----------
fn paint_logo(painter: &egui::Painter, center: Pos2, radius: f32) {
    painter.circle_filled(center, radius, ACCENT);
    // Le F du logo : trois barres
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
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(12.5), TEXT);
    let size = galley.size() + Vec2::new(26.0, 14.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hovered = response.hovered();

    let (fill, stroke, fg) = if primary {
        let fill = if hovered { ACCENT_LIGHT.linear_multiply(0.9) } else { color };
        (fill, Stroke::new(1.0, ACCENT_LIGHT.gamma_multiply(0.6)), Color32::WHITE)
    } else {
        let fill = with_alpha(color, if hovered { 60 } else { 28 });
        (fill, Stroke::new(1.0, with_alpha(color, 140)), if hovered { TEXT } else { color.lerp_to_gamma(TEXT, 0.35) })
    };

    if primary {
        ui.painter().rect_filled(rect.translate(Vec2::new(0.0, 4.0)), CornerRadius::same(20), accent_alpha(if hovered { 90 } else { 60 }));
        ui.painter().rect(rect, CornerRadius::same(20), fill, stroke, StrokeKind::Inside);
        ui.painter().rect_filled(Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.center().y)), CornerRadius { nw: 20, ne: 20, sw: 0, se: 0 }, Color32::from_rgba_unmultiplied(255, 255, 255, 18));
        let _ = ACCENT_DARK;
    } else {
        ui.painter().rect(rect, CornerRadius::same(20), fill, stroke, StrokeKind::Inside);
    }
    ui.painter().galley(rect.min + Vec2::new(13.0, 7.0), galley, fg);
    ui.add_space(8.0);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn link_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(12.0), MUTED);
    let (rect, response) = ui.allocate_exact_size(galley.size() + Vec2::new(16.0, 10.0), Sense::click());
    let color = if response.hovered() { ACCENT_LIGHT } else { MUTED };
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(8), accent_alpha(28));
    }
    ui.painter().galley(rect.min + Vec2::new(8.0, 5.0), galley, color);
    ui.add_space(4.0);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
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
    egui::Frame::new().fill(Color32::from_rgba_unmultiplied(8, 5, 14, 140)).corner_radius(CornerRadius::same(10)).inner_margin(Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(140.0);
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
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
    let c = rect.center();
    let color = if ok { OK } else { DANGER };
    ui.painter().circle_filled(c, 11.0, with_alpha(color, 40));
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
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::hover());
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
    let _ = std::process::Command::new("cmd").args(["/C", "start", "", target]).spawn();
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

    let _ = Id::NULL;
    if let Err(error) = eframe::run_native("Flocord Installer", options, Box::new(|cc| Ok(Box::new(App::new(cc))))) {
        logger::write(&format!("Interface graphique impossible : {}", error));
        // Sans interface graphique (pilote ou session distante), retour au menu console
        cli::interactive();
    }
}
