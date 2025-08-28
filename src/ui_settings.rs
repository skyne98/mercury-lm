use crate::models::{Backend, RuntimeInfo, ServerStatus};
use eframe::egui::{self, Color32, RichText};

pub fn render_settings_panel(app: &mut crate::app::App, ui: &mut egui::Ui) {
    ui.heading(RichText::new("⚙️ Settings").color(Color32::from_rgb(137, 180, 250)));

    ui.separator();

    // Runtime Settings
    ui.collapsing(RichText::new("🖥️ Runtime").color(Color32::from_rgb(166, 227, 161)), |ui| {
        ui.label("Available Runtimes:");
        for runtime in &app.available_runtimes {
            let is_selected = app.current_runtime.as_ref()
                .map(|r| r.name == runtime.name)
                .unwrap_or(false);

            let mut selected = is_selected;
            if ui.checkbox(&mut selected, &runtime.name).changed() {
                if selected {
                    app.current_runtime = Some(runtime.clone());
                    app.server_bin = Some(runtime.path.clone());
                    app.settings.default_runtime = Some(runtime.name.clone());
                    let _ = app.save_settings();
                    app.status = format!("Runtime selected: {}", runtime.name);
                } else {
                    app.current_runtime = None;
                    app.server_bin = None;
                    app.settings.default_runtime = None;
                    let _ = app.save_settings();
                    app.status = "No runtime selected".into();
                }
            }
        }

        if app.available_runtimes.is_empty() {
            ui.colored_label(Color32::from_rgb(249, 226, 175), "No runtimes detected");
            if ui.add(crate::ui::light_button("⬇️ Download Runtime", Color32::from_rgb(166, 227, 161))).clicked() {
                let _ = crate::runtime::ensure_runtime(app);
            }
        }
    });

    ui.separator();

    // Server Settings
    ui.collapsing(RichText::new("🚀 Server").color(Color32::from_rgb(166, 227, 161)), |ui| {
        ui.checkbox(&mut app.settings.auto_start_server, "Auto-start server when needed");
        ui.checkbox(&mut app.settings.auto_stop_server, "Auto-stop server after inactivity");

        ui.horizontal(|ui| {
            ui.label("Server timeout (minutes):");
            let mut timeout = app.settings.server_timeout_minutes as i32;
            if ui.add(egui::DragValue::new(&mut timeout).range(5..=120)).changed() {
                app.settings.server_timeout_minutes = timeout as u32;
                let _ = app.save_settings();
            }
        });

        ui.separator();

        // Server Status
        ui.label(RichText::new("Server Status:").color(Color32::from_rgb(137, 180, 250)));
        match &app.server_status {
            ServerStatus::Stopped => {
                ui.colored_label(Color32::from_rgb(243, 139, 168), "🛑 Stopped");
                if ui.add(crate::ui::light_button("▶️ Start Server", Color32::from_rgb(166, 227, 161))).clicked() {
                    app.ensure_server_running();
                }
            }
            ServerStatus::Starting => {
                ui.colored_label(Color32::from_rgb(249, 226, 175), "⏳ Starting...");
            }
            ServerStatus::Running => {
                ui.colored_label(Color32::from_rgb(166, 227, 161), "🟢 Running");
                if ui.add(crate::ui::light_button("⏹️ Stop Server", Color32::from_rgb(243, 139, 168))).clicked() {
                    if let Some(mut child) = app.server_child.take() {
                        let _ = child.kill();
                        app.server_ready = false;
                        app.server_status = ServerStatus::Stopped;
                        app.status = "Server stopped".into();
                    }
                }
            }
            ServerStatus::Error(err) => {
                ui.colored_label(Color32::from_rgb(243, 139, 168), format!("❌ Error: {}", err));
            }
        }
    });

    ui.separator();

    // Chat Settings
    ui.collapsing(RichText::new("💬 Chat").color(Color32::from_rgb(249, 226, 175)), |ui| {
        ui.horizontal(|ui| {
            ui.label("Max chat history:");
            let mut max_history = app.settings.max_chat_history as i32;
            if ui.add(egui::DragValue::new(&mut max_history).range(100..=10000)).changed() {
                app.settings.max_chat_history = max_history as usize;
                let _ = app.save_settings();
            }
        });
    });

    ui.separator();

    // Actions
    ui.horizontal(|ui| {
        if ui.add(crate::ui::light_button("💾 Save Settings", Color32::from_rgb(166, 227, 161))).clicked() {
            if let Err(e) = app.save_settings() {
                app.status = format!("Save error: {e}");
            } else {
                app.status = "Settings saved".into();
            }
        }

        if ui.add(crate::ui::light_button("🔄 Refresh Runtimes", Color32::from_rgb(137, 180, 250))).clicked() {
            app.detect_runtimes();
            app.status = format!("Found {} runtime(s)", app.available_runtimes.len());
        }
    });
}
