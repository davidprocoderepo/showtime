mod audio;
mod error;
mod export;
mod live;
mod markers;
mod project;
mod timecode;
mod ui;

use ui::app::ShowtimeApp;

fn main() -> eframe::Result<()> {
    env_logger::init();
    log::info!("Showtime iniciando...");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Showtime — Marcador de Cues para GrandMA2")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Showtime — GrandMA2 Cue Marker",
        options,
        Box::new(|_cc| Ok(Box::new(ShowtimeApp::new()) as Box<dyn eframe::App>)),
    )
}