//! Desktop studio laid out like the JSplacement program:
//! preview and actions on the left, mode controls on the right.

use std::sync::mpsc;
use std::time::Duration;

use displacement_gen::classic;
use displacement_gen::dots::{self, Pattern, Shape};
use displacement_gen::map::{Map, Palette};
use displacement_gen::panels::{self, Pack};
use displacement_gen::velvet::{self, Symmetry};
use displacement_gen::wyre::{self, Directions, Ramp};
use eframe::egui;

const BG: egui::Color32 = egui::Color32::from_rgb(18, 18, 18);
const GREEN: egui::Color32 = egui::Color32::from_rgb(61, 220, 74);
const PINK: egui::Color32 = egui::Color32::from_rgb(255, 90, 170);
const PURPLE: egui::Color32 = egui::Color32::from_rgb(168, 92, 255);
const DARK_RED: egui::Color32 = egui::Color32::from_rgb(160, 42, 48);
const ORANGE: egui::Color32 = egui::Color32::from_rgb(232, 140, 42);
const BLUE: egui::Color32 = egui::Color32::from_rgb(64, 140, 255);
const CYAN: egui::Color32 = egui::Color32::from_rgb(40, 210, 220);
const DIM: egui::Color32 = egui::Color32::from_rgb(150, 150, 150);
const UNDO_LIMIT: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Classic,
    Panels,
    Velvet,
    Wyre,
    Dots,
    Guide,
    Settings,
    About,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Classic,
    Panels,
    Velvet,
    Wyre,
    Dots,
}

struct MenuEntry {
    label: &'static str,
    page: Page,
    mode: Option<Mode>,
}

const MENU: &[MenuEntry] = &[
    MenuEntry {
        label: "JSplacement: Classic",
        page: Page::Classic,
        mode: Some(Mode::Classic),
    },
    MenuEntry {
        label: "JSplacement: 2",
        page: Page::Panels,
        mode: Some(Mode::Panels),
    },
    MenuEntry {
        label: "JSplacement: Velvet",
        page: Page::Velvet,
        mode: Some(Mode::Velvet),
    },
    MenuEntry {
        label: "JSplacement: Wyre",
        page: Page::Wyre,
        mode: Some(Mode::Wyre),
    },
    MenuEntry {
        label: "JSplacement: dotGrid",
        page: Page::Dots,
        mode: Some(Mode::Dots),
    },
    MenuEntry {
        label: "Digital Guide",
        page: Page::Guide,
        mode: None,
    },
    MenuEntry {
        label: "Settings",
        page: Page::Settings,
        mode: None,
    },
    MenuEntry {
        label: "About",
        page: Page::About,
        mode: None,
    },
];

impl Page {
    fn label(self) -> &'static str {
        MENU.iter()
            .find(|entry| entry.page == self)
            .map(|entry| entry.label)
            .unwrap_or("Sci-Fi Displacement")
    }
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Classic => "Classic",
            Mode::Panels => "JSplacement 2",
            Mode::Velvet => "Velvet",
            Mode::Wyre => "Wyre",
            Mode::Dots => "dotGrid",
        }
    }
}

enum Job {
    Classic(classic::Params),
    Panels(panels::Params),
    Velvet(velvet::Params),
    Wyre(wyre::Params),
    Dots(dots::Params),
}

fn run_job(job: Job, size: u32, seed: u64) -> Map {
    match job {
        Job::Classic(params) => classic::generate(&params, size, size, seed),
        Job::Panels(params) => panels::generate(&params, size, size, seed),
        Job::Velvet(params) => velvet::generate(&params, size, size, seed),
        Job::Wyre(params) => wyre::generate(&params, size, size, seed),
        Job::Dots(params) => dots::generate(&params, size, size, seed),
    }
}

pub struct Studio {
    page: Page,
    mode: Mode,
    menu_open: bool,
    size: u32,
    normal_strength: f32,
    palette: Palette,
    colorize: bool,
    lock_seed: bool,
    seed: u64,
    seed_text: String,
    classic: classic::Params,
    panels: panels::Params,
    velvet: velvet::Params,
    wyre: wyre::Params,
    dots: dots::Params,
    map: Option<Map>,
    undo: Vec<Map>,
    texture: Option<egui::TextureHandle>,
    pending: Option<mpsc::Receiver<Map>>,
    preview_dirty: bool,
    status: String,
}

impl Studio {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        Self {
            page: Page::Classic,
            mode: Mode::Classic,
            menu_open: false,
            size: 1024,
            normal_strength: 4.0,
            palette: Palette::Metal,
            colorize: false,
            lock_seed: false,
            seed: 1,
            seed_text: "1".into(),
            classic: classic::Params::default(),
            panels: panels::Params::default(),
            velvet: velvet::Params::default(),
            wyre: wyre::Params::default(),
            dots: dots::Params::default(),
            map: None,
            undo: Vec::new(),
            texture: None,
            pending: None,
            preview_dirty: false,
            status: "Click Generate or press R".into(),
        }
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        let typing = ctx.egui_wants_keyboard_input();
        let (pressed_r, pressed_undo, pressed_escape) = ctx.input(|input| {
            (
                input.key_pressed(egui::Key::R),
                input.modifiers.command && input.key_pressed(egui::Key::Z),
                input.key_pressed(egui::Key::Escape),
            )
        });
        if self.menu_open && pressed_escape {
            self.menu_open = false;
        }
        if !typing && !self.menu_open && pressed_r {
            self.start_generate();
        }
        if !typing && pressed_undo {
            self.undo();
        }
    }

    fn sync_preview(&mut self, ctx: &egui::Context) {
        self.poll(ctx);
        if self.preview_dirty {
            self.upload(ctx);
        }
    }

    fn poll(&mut self, ctx: &egui::Context) {
        let finished = match &self.pending {
            Some(rx) => match rx.try_recv() {
                Ok(map) => Some(Ok(map)),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(Err(())),
            },
            None => None,
        };
        match finished {
            Some(Ok(map)) => {
                let width = map.width();
                let height = map.height();
                self.push_history();
                self.map = Some(map);
                self.preview_dirty = true;
                self.pending = None;
                self.status = format!(
                    "{} · {width}×{height} · seed {}",
                    self.mode.label(),
                    self.seed
                );
            }
            Some(Err(())) => {
                self.pending = None;
                self.status = "Generation failed".into();
            }
            None => {
                if self.pending.is_some() {
                    ctx.request_repaint_after(Duration::from_millis(50));
                }
            }
        }
    }

    fn upload(&mut self, ctx: &egui::Context) {
        self.preview_dirty = false;
        let Some(map) = &self.map else {
            return;
        };
        if map.width() == 0 || map.height() == 0 {
            return;
        }
        let rgba = map.preview_rgba(self.colorize.then_some(self.palette));
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [map.width() as usize, map.height() as usize],
            &rgba,
        );
        let options = egui::TextureOptions::NEAREST;
        if let Some(texture) = &mut self.texture {
            texture.set(image, options);
        } else {
            self.texture = Some(ctx.load_texture("preview", image, options));
        }
    }

    fn layout(&mut self, ui: &mut egui::Ui) {
        let left_width = (ui.available_width() * 0.46).clamp(420.0, 680.0);
        egui::Panel::top("header")
            .exact_size(72.0)
            .show_separator_line(false)
            .frame(panel_frame())
            .show(ui, |ui| self.header(ui));
        egui::Panel::left("preview_side")
            .exact_size(left_width)
            .resizable(false)
            .frame(panel_frame())
            .show(ui, |ui| self.preview_column(ui));
        egui::CentralPanel::default()
            .frame(panel_frame())
            .show(ui, |ui| self.show_controls(ui));
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            if menu_button(ui).clicked() {
                self.menu_open = !self.menu_open;
            }
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(self.page.label())
                    .size(20.0)
                    .strong()
                    .color(egui::Color32::WHITE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("R generate    Ctrl+Z undo")
                        .size(12.5)
                        .color(DIM),
                );
            });
        });
    }

    fn preview_column(&mut self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(format!("Generator · {}", self.mode.label()))
                .size(13.0)
                .color(GREEN),
        );
        ui.add_space(6.0);
        let chrome = 36.0 * 3.0 + 8.0 * 4.0 + 48.0;
        let side = ui
            .available_width()
            .min((ui.available_height() - chrome).max(160.0));
        let extra = (ui.available_width() - side).max(0.0) * 0.5;
        ui.horizontal(|ui| {
            ui.add_space(extra);
            self.preview_square(ui, side);
        });
        ui.add_space(8.0);
        self.actions(ui);
        if self.colorize {
            ui.add_space(6.0);
            self.palette_chips(ui);
        }
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new(&self.status)
                .size(12.5)
                .color(status_color(&self.status)),
        );
    }

    fn preview_square(&mut self, ui: &mut egui::Ui, side: f32) {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click());
        ui.painter()
            .rect_filled(rect, 0u8, egui::Color32::from_rgb(8, 8, 8));
        if let Some(texture) = &self.texture {
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Click to generate\nOr press R",
                egui::FontId::proportional(18.0),
                DIM,
            );
        }
        ui.painter().rect_stroke(
            rect,
            0u8,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(42, 42, 42)),
            egui::StrokeKind::Inside,
        );
        if self.pending.is_some() {
            let badge = egui::Rect::from_min_size(
                rect.right_top() + egui::vec2(-108.0, 12.0),
                egui::vec2(96.0, 26.0),
            );
            ui.painter()
                .rect_filled(badge, 2u8, egui::Color32::from_rgb(196, 28, 36));
            ui.painter().text(
                badge.center(),
                egui::Align2::CENTER_CENTER,
                "WORKING",
                egui::FontId::proportional(13.0),
                egui::Color32::WHITE,
            );
        }
        if response.clicked() {
            self.start_generate();
        }
    }

    fn actions(&mut self, ui: &mut egui::Ui) {
        let working = self.pending.is_some();
        let has_map = self.map.is_some();
        let can_undo = !self.undo.is_empty();
        ui.horizontal(|ui| {
            let spacing = ui.spacing().item_spacing.x;
            let width = (ui.available_width() - spacing) / 2.0;
            if accent_button(ui, "Generate", width, GREEN, !working).clicked() {
                self.start_generate();
            }
            if accent_button(ui, "Save Height", width, PURPLE, has_map).clicked() {
                self.save_height();
            }
        });
        ui.horizontal(|ui| {
            let spacing = ui.spacing().item_spacing.x;
            let width = (ui.available_width() - spacing * 2.0) / 3.0;
            if accent_button(ui, "Undo", width, DARK_RED, can_undo).clicked() {
                self.undo();
            }
            if accent_button(ui, "Invert", width, ORANGE, has_map).clicked() {
                self.invert_map();
            }
            if accent_button(ui, "Save Normal", width, BLUE, has_map).clicked() {
                self.save_normal();
            }
        });
        ui.horizontal(|ui| {
            let spacing = ui.spacing().item_spacing.x;
            let width = (ui.available_width() - spacing) / 2.0;
            if accent_button(ui, "Toggle colorizer", width, PINK, has_map).clicked() {
                self.colorize = !self.colorize;
                self.preview_dirty = true;
            }
            if accent_button(ui, "Save Color", width, CYAN, has_map).clicked() {
                self.save_color();
            }
        });
    }

    fn show_controls(&mut self, ui: &mut egui::Ui) {
        let height = ui.available_height().min(ui.clip_rect().height()).max(80.0);
        egui::ScrollArea::vertical()
            .id_salt("controls")
            .max_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                match self.page {
                    Page::Classic => self.classic_controls(ui),
                    Page::Panels => self.panels_controls(ui),
                    Page::Velvet => self.velvet_controls(ui),
                    Page::Wyre => self.wyre_controls(ui),
                    Page::Dots => self.dots_controls(ui),
                    Page::Guide => guide(ui),
                    Page::Settings => self.settings(ui),
                    Page::About => about(ui),
                }
                ui.add_space(28.0);
            });
    }

    fn classic_controls(&mut self, ui: &mut egui::Ui) {
        section(ui, "Passes");
        slider_u32(ui, "Iterations", &mut self.classic.iterations, 1, 5000);
        slider_u8(ui, "Background", &mut self.classic.background, 0, 255);
        let max_brightness = self.classic.max_brightness;
        slider_u8(
            ui,
            "Min brightness",
            &mut self.classic.min_brightness,
            0,
            max_brightness,
        );
        let min_brightness = self.classic.min_brightness;
        slider_u8(
            ui,
            "Max brightness",
            &mut self.classic.max_brightness,
            min_brightness,
            255,
        );
        section(ui, "Shape");
        slider_f32(ui, "Scale", &mut self.classic.scale, 1.0..=200.0, "%");
        slider_percent(ui, "Ghost alpha", &mut self.classic.ghost_alpha);
        slider_f32(
            ui,
            "Ghost hole chance",
            &mut self.classic.ghost_hole,
            0.0..=100.0,
            "%",
        );
        slider_f32(ui, "Spacing", &mut self.classic.spacing, 0.0..=300.0, "%");
        slider_u32(ui, "Max amount", &mut self.classic.max_amount, 1, 16);
        slider_u32(ui, "Line width", &mut self.classic.line_width, 1, 64);
        section(ui, "Layers");
        ui.horizontal_wrapped(|ui| {
            pill(ui, "Squares", &mut self.classic.squares);
            pill(ui, "Ghosts", &mut self.classic.ghosts);
            pill(ui, "Grids", &mut self.classic.grids);
            pill(ui, "V bars", &mut self.classic.vbars);
            pill(ui, "H bars", &mut self.classic.hbars);
            pill(ui, "Crosses", &mut self.classic.crosses);
        });
    }

    fn panels_controls(&mut self, ui: &mut egui::Ui) {
        section(ui, "Pack");
        self.pack_row(ui, "Classic Set", Pack::Classic);
        self.pack_row(ui, "Crap Pack", Pack::Crap);
        self.pack_row(ui, "BIG:DATA", Pack::BigData);
        self.pack_row(ui, "Aggromaxx", Pack::Aggro);
        self.pack_row(ui, "Custom Sprites", Pack::Custom);
        ui.add_space(6.0);
        if accent_button(
            ui,
            "Load PNG or JPEG sprites",
            ui.available_width(),
            CYAN,
            true,
        )
        .clicked()
        {
            self.load_sprites();
        }
        if !self.panels.custom.is_empty() {
            ui.label(format!(
                "{} sprites ready for the custom pack",
                self.panels.custom.len()
            ));
        }
        section(ui, "Stamps");
        slider_u8(ui, "Background", &mut self.panels.background, 0, 255);
        slider_u32(
            ui,
            "Background iterations",
            &mut self.panels.background_iterations,
            0,
            32,
        );
        slider_u32(ui, "Iterations", &mut self.panels.iterations, 1, 500);
        slider_f32(
            ui,
            "Sprite scaling",
            &mut self.panels.scale,
            1.0..=200.0,
            "%",
        );
        section(ui, "Stamp options");
        ui.horizontal_wrapped(|ui| {
            pill(ui, "Seamless", &mut self.panels.seamless);
            pill(ui, "Rotate", &mut self.panels.rotate);
            pill(ui, "Flip", &mut self.panels.flip);
            pill(ui, "Blend", &mut self.panels.blend);
        });
    }

    fn pack_row(&mut self, ui: &mut egui::Ui, label: &str, pack: Pack) {
        let selected = self.panels.pack == pack;
        let accent = if selected {
            GREEN
        } else {
            egui::Color32::from_rgb(170, 48, 58)
        };
        if accent_button(ui, label, ui.available_width(), accent, true).clicked() {
            self.panels.pack = pack;
        }
    }

    fn velvet_controls(&mut self, ui: &mut egui::Ui) {
        section(ui, "Splits");
        slider_u32(ui, "Iterations", &mut self.velvet.iterations, 1, 400);
        slider_u32(
            ui,
            "Sub iterations",
            &mut self.velvet.sub_iterations,
            1,
            200,
        );
        slider_u32(
            ui,
            "Splitter iterations",
            &mut self.velvet.splitter_iterations,
            0,
            200,
        );
        slider_u8(ui, "Background", &mut self.velvet.background, 0, 255);
        section(ui, "Symmetry");
        ui.label(
            egui::RichText::new("X mirrors top to bottom. Y mirrors left to right.")
                .size(12.5)
                .color(DIM),
        );
        ui.horizontal_wrapped(|ui| {
            choice(ui, "Both", Symmetry::Both, &mut self.velvet.symmetry);
            choice(ui, "X", Symmetry::X, &mut self.velvet.symmetry);
            choice(ui, "Y", Symmetry::Y, &mut self.velvet.symmetry);
        });
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            pill(ui, "Splitter", &mut self.velvet.use_splitter);
            pill(ui, "Splitter blend", &mut self.velvet.splitter_blend);
        });
    }

    fn wyre_controls(&mut self, ui: &mut egui::Ui) {
        section(ui, "Wires");
        slider_u32(ui, "Layers", &mut self.wyre.layers, 1, 80);
        slider_u32(ui, "Wires per layer", &mut self.wyre.wires_per_layer, 1, 12);
        slider_u32(ui, "Max width", &mut self.wyre.max_width, 1, 128);
        slider_u32(ui, "Origin spread", &mut self.wyre.origin_spread, 0, 512);
        let max_step = self.wyre.max_step.max(1);
        slider_u32_log(ui, "Min step", &mut self.wyre.min_step, 1, max_step);
        let min_step = self.wyre.min_step.max(1);
        slider_u32_log(ui, "Max step", &mut self.wyre.max_step, min_step, 8192);
        section(ui, "Color");
        ui.horizontal_wrapped(|ui| {
            for (label, ramp) in ramp_choices() {
                choice(ui, label, ramp, &mut self.wyre.color);
            }
        });
        section(ui, "Width");
        ui.horizontal_wrapped(|ui| {
            for (label, ramp) in ramp_choices() {
                choice(ui, label, ramp, &mut self.wyre.width);
            }
        });
        section(ui, "Directions");
        ui.horizontal_wrapped(|ui| {
            choice(ui, "Any", Directions::Any, &mut self.wyre.directions);
            choice(
                ui,
                "Cardinal",
                Directions::Cardinal,
                &mut self.wyre.directions,
            );
            choice(
                ui,
                "Diagonal",
                Directions::Diagonal,
                &mut self.wyre.directions,
            );
            choice(
                ui,
                "Conical",
                Directions::Conical,
                &mut self.wyre.directions,
            );
        });
    }

    fn dots_controls(&mut self, ui: &mut egui::Ui) {
        section(ui, "Grid");
        slider_u32(ui, "Scale", &mut self.dots.scale, 2, 64);
        slider_u8(ui, "Min brightness", &mut self.dots.min_brightness, 0, 255);
        slider_f32(ui, "Chance", &mut self.dots.chance, 0.0..=100.0, "%");
        slider_u32(ui, "Max pattern length", &mut self.dots.max_length, 1, 16);
        section(ui, "Shapes");
        ui.horizontal_wrapped(|ui| {
            for shape in [
                Shape::Square,
                Shape::Box,
                Shape::Circle,
                Shape::Ring,
                Shape::Diamond,
                Shape::Gem,
            ] {
                let on = self.dots.shapes.contains(&shape);
                if icon_toggle(ui, on, |painter, rect| draw_shape(painter, rect, shape)).clicked() {
                    toggle_item(&mut self.dots.shapes, shape);
                }
            }
        });
        section(ui, "Patterns");
        ui.horizontal_wrapped(|ui| {
            for (label, pattern) in [
                ("Row", Pattern::Row),
                ("Col", Pattern::Col),
                ("Rake", Pattern::Rake),
                ("Tee", Pattern::Tee),
                ("Tri", Pattern::Tri),
                ("Diag", Pattern::Diag),
                ("Grid", Pattern::Grid),
            ] {
                let mut on = self.dots.patterns.contains(&pattern);
                if pill(ui, label, &mut on).clicked() {
                    toggle_item(&mut self.dots.patterns, pattern);
                }
            }
        });
        ui.add_space(8.0);
        pill(ui, "Break the code", &mut self.dots.break_code);
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        section(ui, "Resolution");
        ui.label(
            egui::RichText::new("Square maps. The new size is used on the next generate.")
                .size(12.5)
                .color(DIM),
        );
        ui.horizontal_wrapped(|ui| {
            for size in [512u32, 768, 1024, 2048] {
                choice(ui, &format!("{size}"), size, &mut self.size);
            }
        });
        section(ui, "Normal map");
        slider_f32(ui, "Strength", &mut self.normal_strength, 0.25..=16.0, "");
        ui.label(
            egui::RichText::new("Strength is applied when you save a normal map.")
                .size(12.5)
                .color(DIM),
        );
        section(ui, "Colorizer");
        self.palette_chips(ui);
        section(ui, "Seed");
        ui.horizontal_wrapped(|ui| {
            pill(ui, "Lock seed", &mut self.lock_seed);
        });
        ui.label("Seed");
        let edit = ui.text_edit_singleline(&mut self.seed_text);
        if edit.changed() {
            if let Ok(value) = self.seed_text.trim().parse::<u64>() {
                self.seed = value;
                self.lock_seed = true;
            }
        }
        ui.label(
            egui::RichText::new("Unlocked generates pick a new seed. Typing a seed locks it.")
                .size(12.5)
                .color(DIM),
        );
    }

    fn palette_chips(&mut self, ui: &mut egui::Ui) {
        let current = self.palette;
        let colorize = self.colorize;
        ui.horizontal_wrapped(|ui| {
            for palette in [
                Palette::Metal,
                Palette::Cyan,
                Palette::Copper,
                Palette::Ember,
            ] {
                if palette_chip(ui, palette, current == palette).clicked() {
                    self.palette = palette;
                    if colorize {
                        self.preview_dirty = true;
                    }
                }
            }
        });
    }

    fn show_menu(&mut self, ctx: &egui::Context) {
        if !self.menu_open {
            return;
        }
        let screen = ctx.content_rect();
        let panel = egui::Rect::from_min_size(screen.min, egui::vec2(340.0, screen.height()));
        egui::Area::new(egui::Id::unique("menu_layer"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .movable(false)
            .fade_in(false)
            .show(ctx, |ui| {
                let scrim = ui.allocate_response(screen.size(), egui::Sense::click());
                ui.painter()
                    .rect_filled(screen, 0u8, egui::Color32::from_black_alpha(150));
                ui.painter()
                    .rect_filled(panel, 0u8, egui::Color32::from_rgb(14, 14, 14));
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(panel.shrink(16.0))
                        .id_salt("menu_items"),
                    |ui| {
                        ui.label(egui::RichText::new("Modes").size(13.0).color(DIM).strong());
                        ui.add_space(8.0);
                        for (index, entry) in MENU.iter().enumerate() {
                            if index == 5 {
                                ui.add_space(14.0);
                            }
                            if menu_row(ui, entry.label, self.page == entry.page).clicked() {
                                self.page = entry.page;
                                if let Some(mode) = entry.mode {
                                    self.mode = mode;
                                }
                                self.menu_open = false;
                            }
                        }
                    },
                );
                if scrim.clicked() {
                    self.menu_open = false;
                }
            });
    }

    fn start_generate(&mut self) {
        if self.pending.is_some() {
            return;
        }
        if self.mode == Mode::Panels
            && self.panels.pack == Pack::Custom
            && self.panels.custom.is_empty()
        {
            self.status = "Load sprite images for the custom pack".into();
            return;
        }
        let seed = self.take_seed();
        let size = self.size;
        let job = self.job();
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        self.status = format!("Working · {} · {size}×{size}", self.mode.label());
        let spawned = std::thread::Builder::new()
            .name("generate".into())
            .spawn(move || {
                let map = run_job(job, size, seed);
                let _ = tx.send(map);
            });
        if spawned.is_err() {
            self.pending = None;
            self.status = "Could not start generation".into();
        }
    }

    fn job(&self) -> Job {
        match self.mode {
            Mode::Classic => Job::Classic(self.classic.clone()),
            Mode::Panels => Job::Panels(self.panels.clone()),
            Mode::Velvet => Job::Velvet(self.velvet.clone()),
            Mode::Wyre => Job::Wyre(self.wyre.clone()),
            Mode::Dots => Job::Dots(self.dots.clone()),
        }
    }

    fn take_seed(&mut self) -> u64 {
        if self.lock_seed {
            if let Ok(value) = self.seed_text.trim().parse::<u64>() {
                self.seed = value;
            } else {
                self.seed_text = self.seed.to_string();
            }
            return self.seed;
        }
        let seed = fresh_seed();
        self.seed = seed;
        self.seed_text = seed.to_string();
        seed
    }

    fn push_history(&mut self) {
        let Some(map) = self.map.clone() else {
            return;
        };
        self.undo.push(map);
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
    }

    fn undo(&mut self) {
        let Some(previous) = self.undo.pop() else {
            return;
        };
        self.map = Some(previous);
        self.preview_dirty = true;
        self.status = "Undone".into();
    }

    fn invert_map(&mut self) {
        if self.map.is_none() {
            return;
        }
        self.push_history();
        if let Some(map) = &mut self.map {
            map.invert();
        }
        self.preview_dirty = true;
        self.status = "Height inverted".into();
    }

    fn save_height(&mut self) {
        let Some(map) = &self.map else {
            return;
        };
        let Some(path) = save_dialog("Save height", "height.png") else {
            return;
        };
        self.status = match map.save_height(&path) {
            Ok(()) => format!("Saved height {}", path.display()),
            Err(err) => format!("Could not save height: {err}"),
        };
    }

    fn save_normal(&mut self) {
        let Some(map) = &self.map else {
            return;
        };
        let Some(path) = save_dialog("Save normal", "normal.png") else {
            return;
        };
        let wrap = self.mode == Mode::Panels && self.panels.seamless;
        self.status = match map.save_normal(&path, self.normal_strength, wrap) {
            Ok(()) => format!("Saved normal {}", path.display()),
            Err(err) => format!("Could not save normal: {err}"),
        };
    }

    fn save_color(&mut self) {
        let Some(map) = &self.map else {
            return;
        };
        let Some(path) = save_dialog("Save color", "color.png") else {
            return;
        };
        self.status = match map.save_color(&path, self.palette) {
            Ok(()) => format!("Saved color {}", path.display()),
            Err(err) => format!("Could not save color: {err}"),
        };
    }

    fn load_sprites(&mut self) {
        let Some(paths) = rfd::FileDialog::new()
            .set_title("Custom sprites")
            .add_filter("Images", &["png", "jpg", "jpeg"])
            .pick_files()
        else {
            return;
        };
        let mut loaded = Vec::new();
        let mut failed = 0;
        for path in paths {
            match image::open(&path) {
                Ok(image) => loaded.push(panels::Sprite::from_rgba(&image.to_rgba8())),
                Err(_) => failed += 1,
            }
        }
        if loaded.is_empty() {
            self.status = "No sprites could be read".into();
            return;
        }
        let count = loaded.len();
        self.panels.custom = loaded;
        self.panels.pack = Pack::Custom;
        self.page = Page::Panels;
        self.mode = Mode::Panels;
        self.status = if failed == 0 {
            format!("Loaded {count} sprites")
        } else {
            format!("Loaded {count} sprites, {failed} failed")
        };
    }
}

impl eframe::App for Studio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        apply_theme(ui.ctx());
        self.handle_keys(ui.ctx());
        self.sync_preview(ui.ctx());
        self.layout(ui);
        self.show_menu(ui.ctx());
    }
}

fn save_dialog(title: &str, file_name: &str) -> Option<std::path::PathBuf> {
    let path = rfd::FileDialog::new()
        .set_title(title)
        .set_file_name(file_name)
        .add_filter("PNG", &["png"])
        .save_file()?;
    Some(ensure_png(path))
}

fn ensure_png(mut path: std::path::PathBuf) -> std::path::PathBuf {
    if path.extension().is_none() {
        path.set_extension("png");
    }
    path
}

fn fresh_seed() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static TICK: AtomicU64 = AtomicU64::new(1);
    let tick = TICK.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    nanos.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(tick)
}

fn toggle_item<T: PartialEq>(items: &mut Vec<T>, item: T) {
    if let Some(index) = items.iter().position(|it| *it == item) {
        items.remove(index);
    } else {
        items.push(item);
    }
}

fn ramp_choices() -> [(&'static str, Ramp); 5] {
    [
        ("Linear", Ramp::Linear),
        ("Inverted linear", Ramp::InvertedLinear),
        ("Curve", Ramp::Curve),
        ("Inverted curve", Ramp::InvertedCurve),
        ("Random", Ramp::Random),
    ]
}

fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(BG)
        .inner_margin(egui::Margin::same(12))
}

fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = BG;
    visuals.panel_fill = BG;
    visuals.faint_bg_color = egui::Color32::from_rgb(28, 28, 28);
    visuals.extreme_bg_color = egui::Color32::from_rgb(10, 10, 10);
    visuals.override_text_color = Some(egui::Color32::from_rgb(230, 230, 230));
    visuals.selection.bg_fill = egui::Color32::from_rgb(255, 45, 140);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 45, 140));
    visuals.hyperlink_color = GREEN;
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(42, 42, 42));
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(36, 36, 36);
    visuals.widgets.inactive.weak_bg_fill = egui::Color32::from_rgb(28, 28, 28);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(52, 36, 48);
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(48, 32, 44);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(255, 45, 140);
    visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(255, 45, 140);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.slider_trailing_fill = true;
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_visuals_of(egui::Theme::Dark, visuals.clone());
    ctx.set_visuals_of(egui::Theme::Light, visuals);
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 4.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.5));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.5));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.5));
    });
}

fn section(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    ui.label(egui::RichText::new(text).size(13.0).color(DIM).strong());
    ui.add_space(2.0);
}

fn slider_u32(ui: &mut egui::Ui, label: &str, value: &mut u32, min: u32, max: u32) {
    ui.label(label);
    ui.add(egui::Slider::new(value, min..=max).trailing_fill(true));
}

fn slider_u32_log(ui: &mut egui::Ui, label: &str, value: &mut u32, min: u32, max: u32) {
    ui.label(label);
    ui.add(
        egui::Slider::new(value, min..=max)
            .logarithmic(true)
            .trailing_fill(true),
    );
}

fn slider_u8(ui: &mut egui::Ui, label: &str, value: &mut u8, min: u8, max: u8) {
    ui.label(label);
    let mut current = *value as u32;
    let response =
        ui.add(egui::Slider::new(&mut current, min as u32..=max as u32).trailing_fill(true));
    if response.changed() {
        *value = current as u8;
    }
}

fn slider_f32(
    ui: &mut egui::Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    suffix: &str,
) {
    ui.label(label);
    let mut slider = egui::Slider::new(value, range)
        .trailing_fill(true)
        .max_decimals(if suffix == "%" { 0 } else { 2 });
    if !suffix.is_empty() {
        slider = slider.suffix(suffix);
    }
    ui.add(slider);
}

fn slider_percent(ui: &mut egui::Ui, label: &str, value: &mut f32) {
    ui.label(label);
    let mut percent = (*value * 100.0).clamp(0.0, 100.0);
    let response = ui.add(
        egui::Slider::new(&mut percent, 0.0..=100.0)
            .trailing_fill(true)
            .suffix("%")
            .max_decimals(0),
    );
    if response.changed() {
        *value = percent / 100.0;
    }
}

fn accent_button(
    ui: &mut egui::Ui,
    label: &str,
    width: f32,
    accent: egui::Color32,
    enabled: bool,
) -> egui::Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width.max(1.0), 36.0), sense);
    let fill = if !enabled {
        egui::Color32::from_rgb(32, 32, 32)
    } else if response.hovered() {
        egui::Color32::from_rgb(48, 48, 48)
    } else {
        egui::Color32::from_rgb(36, 36, 36)
    };
    let bar = if enabled {
        accent
    } else {
        egui::Color32::from_rgb(70, 70, 70)
    };
    let text = if enabled {
        egui::Color32::from_rgb(240, 240, 240)
    } else {
        egui::Color32::from_rgb(120, 120, 120)
    };
    ui.painter().rect_filled(rect, 2u8, fill);
    ui.painter().rect_filled(
        egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height())),
        0u8,
        bar,
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(14.0),
        text,
    );
    response
}

fn pill(ui: &mut egui::Ui, label: &str, on: &mut bool) -> egui::Response {
    let font = egui::FontId::proportional(13.5);
    let width = text_width(ui, label, font.clone()) + 22.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 28.0), egui::Sense::click());
    if response.clicked() {
        *on = !*on;
    }
    let fill = if *on {
        GREEN
    } else {
        egui::Color32::from_rgb(48, 48, 48)
    };
    let text = if *on {
        egui::Color32::from_rgb(10, 24, 12)
    } else {
        egui::Color32::from_rgb(220, 220, 220)
    };
    ui.painter().rect_filled(rect, 14u8, fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        text,
    );
    response
}

fn choice<T: Copy + PartialEq>(ui: &mut egui::Ui, label: &str, value: T, current: &mut T) {
    let selected = *current == value;
    let color = if selected {
        GREEN
    } else {
        egui::Color32::from_rgb(190, 190, 190)
    };
    let font = egui::FontId::proportional(14.0);
    let width = text_width(ui, label, font.clone()) + 12.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 26.0), egui::Sense::click());
    ui.painter().text(
        rect.left_center() + egui::vec2(2.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        font,
        color,
    );
    if selected {
        ui.painter().line_segment(
            [
                rect.left_bottom() + egui::vec2(0.0, -2.0),
                rect.right_bottom() + egui::vec2(0.0, -2.0),
            ],
            egui::Stroke::new(2.0, GREEN),
        );
    }
    if response.clicked() {
        *current = value;
    }
}

fn icon_toggle(
    ui: &mut egui::Ui,
    on: bool,
    draw: impl FnOnce(&egui::Painter, egui::Rect),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(42.0, 42.0), egui::Sense::click());
    let fill = if on {
        egui::Color32::from_rgb(20, 42, 24)
    } else {
        egui::Color32::from_rgb(32, 32, 32)
    };
    ui.painter().rect_filled(rect, 4u8, fill);
    if on {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            4u8,
            egui::Stroke::new(2.0, GREEN),
            egui::StrokeKind::Inside,
        );
    }
    draw(ui.painter(), rect.shrink(10.0));
    response
}

fn draw_shape(painter: &egui::Painter, rect: egui::Rect, shape: Shape) {
    let color = egui::Color32::from_rgb(235, 235, 235);
    let center = rect.center();
    let radius = rect.width().min(rect.height()) * 0.5;
    match shape {
        Shape::Square => {
            painter.rect_filled(rect, 1u8, color);
        }
        Shape::Box => {
            painter.rect_stroke(
                rect,
                1u8,
                egui::Stroke::new(2.0, color),
                egui::StrokeKind::Inside,
            );
        }
        Shape::Circle => {
            painter.circle_filled(center, radius, color);
        }
        Shape::Ring => {
            painter.circle_stroke(
                center,
                (radius - 1.0).max(1.0),
                egui::Stroke::new(2.0, color),
            );
        }
        Shape::Diamond => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(center.x, rect.top()),
                    egui::pos2(rect.right(), center.y),
                    egui::pos2(center.x, rect.bottom()),
                    egui::pos2(rect.left(), center.y),
                ],
                color,
                egui::Stroke::NONE,
            ));
        }
        Shape::Gem => {
            let mid_y = rect.top() + rect.height() * 0.38;
            painter.add(egui::Shape::convex_polygon(
                vec![
                    egui::pos2(rect.left() + rect.width() * 0.28, rect.top()),
                    egui::pos2(rect.right() - rect.width() * 0.28, rect.top()),
                    egui::pos2(rect.right(), mid_y),
                    egui::pos2(center.x, rect.bottom()),
                    egui::pos2(rect.left(), mid_y),
                ],
                color,
                egui::Stroke::NONE,
            ));
        }
    }
}

fn menu_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 4u8, egui::Color32::from_rgb(36, 36, 36));
    }
    let cell = 5.5;
    let gap = 3.5;
    let span = cell * 3.0 + gap * 2.0;
    let origin = rect.center() - egui::vec2(span * 0.5, span * 0.5);
    for row in 0..3 {
        for col in 0..3 {
            let pos = origin + egui::vec2(col as f32 * (cell + gap), row as f32 * (cell + gap));
            ui.painter().rect_filled(
                egui::Rect::from_min_size(pos, egui::vec2(cell, cell)),
                1u8,
                PINK,
            );
        }
    }
    response
}

fn menu_row(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    let width = ui.available_width().max(280.0);
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 40.0), egui::Sense::click());
    if selected {
        ui.painter()
            .rect_filled(rect, 0u8, egui::Color32::from_rgb(24, 40, 26));
        ui.painter().rect_filled(
            egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height())),
            0u8,
            GREEN,
        );
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 0u8, egui::Color32::from_rgb(32, 32, 32));
    }
    let color = if selected {
        GREEN
    } else {
        egui::Color32::from_rgb(225, 225, 225)
    };
    ui.painter().text(
        rect.left_center() + egui::vec2(16.0, 0.0),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(16.0),
        color,
    );
    response
}

fn palette_chip(ui: &mut egui::Ui, palette: Palette, selected: bool) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(92.0, 28.0), egui::Sense::click());
    let fill = if response.hovered() {
        egui::Color32::from_rgb(42, 42, 42)
    } else {
        egui::Color32::from_rgb(28, 28, 28)
    };
    ui.painter().rect_filled(rect, 4u8, fill);
    if selected {
        ui.painter().rect_stroke(
            rect,
            4u8,
            egui::Stroke::new(2.0, GREEN),
            egui::StrokeKind::Inside,
        );
    }
    let swatch = egui::Rect::from_min_size(rect.min + egui::vec2(8.0, 8.0), egui::vec2(12.0, 12.0));
    ui.painter()
        .rect_filled(swatch, 2u8, palette_swatch(palette));
    ui.painter().text(
        rect.left_center() + egui::vec2(26.0, 0.0),
        egui::Align2::LEFT_CENTER,
        palette_name(palette),
        egui::FontId::proportional(13.0),
        egui::Color32::from_rgb(230, 230, 230),
    );
    response
}

fn palette_name(palette: Palette) -> &'static str {
    match palette {
        Palette::Metal => "Metal",
        Palette::Cyan => "Cyan",
        Palette::Copper => "Copper",
        Palette::Ember => "Ember",
    }
}

fn palette_swatch(palette: Palette) -> egui::Color32 {
    match palette {
        Palette::Metal => egui::Color32::from_rgb(176, 182, 190),
        Palette::Cyan => egui::Color32::from_rgb(80, 196, 206),
        Palette::Copper => egui::Color32::from_rgb(214, 150, 86),
        Palette::Ember => egui::Color32::from_rgb(232, 120, 36),
    }
}

fn text_width(ui: &egui::Ui, text: &str, font: egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, egui::Color32::WHITE)
        .size()
        .x
}

fn status_color(status: &str) -> egui::Color32 {
    if status.starts_with("Could")
        || status.starts_with("Load")
        || status.starts_with("Generation")
        || status.starts_with("No ")
    {
        ORANGE
    } else {
        DIM
    }
}

fn guide(ui: &mut egui::Ui) {
    heading(ui, "Classic");
    body(
        ui,
        "Stacked plates, soft rectangles, grilles, bars, and seams. Large shapes land first. Later passes shrink, so the detail sits on top.",
    );
    heading(ui, "JSplacement 2");
    body(
        ui,
        "Greeble stamps. The packs are original procedural stand-ins for the classic sprite sets. Load your own PNG or JPEG stamps to fill the custom pack. Seamless wraps the tile.",
    );
    heading(ui, "Velvet");
    body(
        ui,
        "Mirrored blocks from a recursive split. X mirrors top to bottom. Y mirrors left to right. Both mirrors the field both ways.",
    );
    heading(ui, "Wyre");
    body(
        ui,
        "Circuit traces. Each layer shares one origin when origin spread is zero. Width is in pixels, so a width of 64 is a thick trace. Linear color makes later wires brighter.",
    );
    heading(ui, "dotGrid");
    body(
        ui,
        "A lattice of shapes and short patterns. Chance is the percent of cells that receive a mark. Break the code jitters the marks.",
    );
    heading(ui, "Blender");
    body(
        ui,
        "Use the height PNG as a displacement texture. Set Color Space to Non-Color and midlevel to 0.5. Brighter pixels are raised. The normal map is tangent space. Invert flips the height before normals and color are saved.",
    );
    heading(ui, "Keys");
    body(
        ui,
        "R generates. Ctrl+Z undoes. Click the preview to generate. Generate, Undo, and Invert keep the last 16 maps.",
    );
}

fn about(ui: &mut egui::Ui) {
    heading(ui, "Sci-Fi Displacement");
    body(
        ui,
        "An original generator for sci-fi height, normal, and color maps. The window follows the JSplacement program by Windmill. This is not that application, and the panel packs are procedural shapes rather than the original sprite artwork.",
    );
    body(
        ui,
        "The same generators are available from the command line as displacement-cli. It writes height, normal, and color PNG files.",
    );
}

fn heading(ui: &mut egui::Ui, text: &str) {
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(text)
            .size(18.0)
            .strong()
            .color(egui::Color32::WHITE),
    );
    ui.add_space(2.0);
}

fn body(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(14.5)
            .color(egui::Color32::from_rgb(210, 210, 210)),
    );
    ui.add_space(4.0);
}
