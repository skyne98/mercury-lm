mod models;
mod download;
mod unzip;
mod spawn;
mod model_download;
mod runtime;
mod scan;
mod stream;
mod server;
mod hf;
mod ui;
mod ui_top;
mod ui_models;
mod ui_chat;
mod ui_settings;
mod app;
mod app_impl;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let native_opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Mercury LM - AI Chat Interface"),
        ..Default::default()
    };
    eframe::run_native(
        "Mercury LM",
        native_opts,
        Box::new(|cc| {
            ui::setup_style(&cc.egui_ctx);
            Ok(Box::new(app::App::default()))
        }),
    )
}
