//! Windowed displacement studio. Release builds hide the console so a
//! double-click opens the window directly.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod gui;

fn main() {
    if let Err(err) = run() {
        let path = std::env::temp_dir().join("displacement-gen-error.txt");
        let _ = std::fs::write(&path, err.to_string());
    }
}

fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1240.0, 860.0])
            .with_min_inner_size([980.0, 640.0])
            .with_position([48.0, 32.0])
            .with_title("Sci-Fi Displacement")
            .with_app_id("sci-fi-displacement"),
        renderer: eframe::Renderer::Glow,
        centered: false,
        ..Default::default()
    };
    eframe::run_native(
        "Sci-Fi Displacement",
        options,
        Box::new(|cc| Ok(Box::new(gui::Studio::new(cc)))),
    )
}
