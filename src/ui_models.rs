use crate::models::DownloadedModel;
use eframe::egui::{self, Color32, RichText};
use std::{fs, process::Command, path::PathBuf};

pub fn render_downloaded_models(app: &mut crate::app::App, ui: &mut egui::Ui) {
    ui.collapsing(RichText::new("📁 Downloaded models").color(Color32::from_rgb(137, 180, 250)), |ui| {
        if app.downloaded.is_empty() {
            ui.label("No models downloaded yet.");
        }
        egui::ScrollArea::vertical()
            .max_height(160.0)
            .show(ui, |ui| {
                let items = app.downloaded.clone();
                for item in items {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            let size_txt = item.size.map(crate::download::human_size).unwrap_or("?".into());
                            ui.label(format!("{}  ({})", item.file_name, size_txt));
                            if ui.add(crate::ui::light_button("✅ Use", Color32::from_rgb(166, 227, 161))).clicked() {
                                app.model_file = item.file_name.clone();
                                app.model_repo = "(local)".into();
                                app.model_path = Some(item.path.clone());
                                app.status = "Selected local model".into();
                            }
                            if ui.add(crate::ui::light_button("🗑️ Delete", Color32::from_rgb(243, 139, 168))).clicked() {
                                let _ = fs::remove_file(&item.path);
                                crate::scan::scan_downloaded_models(app);
                            }
                            if ui.add(crate::ui::light_button("📂 Reveal", Color32::from_rgb(137, 180, 250))).clicked() {
                                let _ = Command::new("explorer").arg(&item.path).spawn();
                            }
                        });
                    });
                }
            });
    });
}
