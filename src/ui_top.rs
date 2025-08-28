use crate::models::{Backend, HFFile};
use crate::download::human_size;
use eframe::egui::{self, Color32, RichText};
use std::{fs, process::Command, path::PathBuf};

pub fn render_top_panel(app: &mut crate::app::App, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("🖥️ Backend:").color(Color32::from_rgb(137, 180, 250)));
        for (b, label) in [
            (Backend::Auto, "Auto"),
            (Backend::Cpu, "CPU"),
            (Backend::Cuda, "CUDA"),
            (Backend::Hip, "HIP"),
            (Backend::Metal, "Metal"),
            (Backend::Vulkan, "Vulkan"),
        ] {
            ui.selectable_value(&mut app.backend, b, label);
        }

        ui.separator();

        ui.label(RichText::new("📦 Model repo:").color(Color32::from_rgb(137, 180, 250)));
        ui.text_edit_singleline(&mut app.model_repo);
        ui.label(RichText::new("📄 File:").color(Color32::from_rgb(137, 180, 250)));
        ui.text_edit_singleline(&mut app.model_file);

        if ui.add(crate::ui::light_button("⬇️ Download model", Color32::from_rgb(166, 227, 161))).clicked() {
            let _ = crate::runtime::start_model_download(app);
        }

        if let Some((cur, tot, stage)) = &app.model_progress {
            let frac = tot.map(|t| *cur as f32 / t as f32).unwrap_or(0.0);
            ui.add(
                egui::ProgressBar::new(frac)
                    .text(format!("{stage} {}", human_size(*cur)))
                    .fill(Color32::from_rgb(249, 226, 175)),
            );
        }
    });

    ui.separator();

    ui.horizontal(|ui| {
        ui.label(RichText::new("🔍 Search HF:").color(Color32::from_rgb(137, 180, 250)));
        ui.text_edit_singleline(&mut app.search_query);
        if ui.add(crate::ui::light_button("🔎 Search", Color32::from_rgb(166, 227, 161))).clicked() {
            match crate::hf::hf_search_models(&app.search_query) {
                Ok(list) => {
                    app.search_results = list;
                    app.search_status = String::new();
                }
                Err(e) => {
                    app.search_status = format!("Search err: {e}");
                    app.search_results.clear();
                }
            }
        }
        if !app.search_status.is_empty() {
            ui.label(RichText::new(&app.search_status).color(Color32::from_rgb(243, 139, 168)));
        }
    });

    if !app.search_results.is_empty() {
        egui::ScrollArea::vertical()
            .max_height(120.0)
            .show(ui, |ui| {
                for id in app.search_results.clone() {
                    ui.horizontal(|ui| {
                        ui.label(&id);
                        if ui.add(crate::ui::light_button("📂 Open", Color32::from_rgb(137, 180, 250))).clicked() {
                            match crate::hf::hf_fetch_files(&id) {
                                Ok(files) => {
                                    app.selected_model = Some(id.clone());
                                    app.files_for_selected = files;
                                }
                                Err(e) => {
                                    app.search_status = format!("Files err: {e}");
                                }
                            }
                        }
                    });
                }
            });
    }

    ui.separator();

    crate::ui_models::render_downloaded_models(app, ui);

    if let Some(model_id) = app.selected_model.clone() {
        let files = app.files_for_selected.clone();
        ui.collapsing(format!("📄 {} files", model_id), |ui| {
            egui::ScrollArea::vertical()
                .max_height(160.0)
                .show(ui, |ui| {
                    for f in files.clone() {
                        let size_txt = f.size.map(human_size).unwrap_or("?".into());
                        ui.horizontal(|ui| {
                            ui.label(format!("{} ({})", f.rfilename, size_txt));
                            if ui.add(crate::ui::light_button("⬇️ Download", Color32::from_rgb(166, 227, 161))).clicked() {
                                app.model_repo = model_id.clone();
                                app.model_file = f.rfilename.clone();
                                let _ = crate::runtime::start_model_download(app);
                            }
                        });
                    }
                });
        });
    }

    if let Some(m) = &app.loaded_model {
        ui.label(format!("📂 Loaded model path: {m}"));
    }
    if let Some(mid) = &app.served_model_id {
        ui.label(format!("🆔 Server model id: {mid}"));
    }
}
