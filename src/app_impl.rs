use crate::models::*;
use eframe::egui::{self, Color32};

impl eframe::App for crate::app::App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // streaming updates
        if let Some(rx0) = self.rx.take() {
            let mut rx = rx0;
            let mut done = false;
            while let Ok(ev) = rx.try_recv() {
                match ev {
                    StreamEvent::Token(t) => {
                        if let Some(last) = self.msgs.last_mut() {
                            if last.role == "assistant" {
                                last.content.push_str(&t);
                            }
                        }
                    }
                    StreamEvent::Error(e) => {
                        self.status = format!("Chat err: {e}");
                    }
                    StreamEvent::Done => {
                        done = true;
                    }
                }
                ctx.request_repaint();
            }
            if !done {
                self.rx = Some(rx);
            } else {
                self.status = "Idle".into();
            }
        }

        // drain download events
        if let Some(rx) = self.dl_rx.take() {
            let mut rx = rx;
            let mut events = Vec::new();
            while let Ok(ev) = rx.try_recv() {
                events.push(ev);
            }
            self.dl_rx = Some(rx);
            for ev in events {
                match ev {
                    DownloadEvent::Done {
                        kind: DownloadKind::Model,
                        dest,
                    } => {
                        if let Some(p) = dest {
                            self.model_path = Some(p);
                        }
                        self.model_progress = None;
                        self.status = "Model ready".into();
                        crate::scan::scan_downloaded_models(self);
                    }
                    DownloadEvent::Done {
                        kind: DownloadKind::Runtime,
                        ..
                    } => {
                        let bin_dir = self.runtime_dir.join("llama-bin");
                        self.server_bin = crate::server::find_server_bin(&bin_dir);
                        self.runtime_progress = None;
                        self.status = "Runtime ready".into();
                    }
                    DownloadEvent::Progress {
                        kind: DownloadKind::Runtime,
                        current,
                        total,
                        stage,
                    } => {
                        self.runtime_progress = Some((current, total, stage.to_string()));
                        self.status = format!(
                            "Runtime {stage}: {} / {}",
                            crate::download::human_size(current),
                            total.map(crate::download::human_size).unwrap_or_else(|| "?".into())
                        );
                    }
                    DownloadEvent::Progress {
                        kind: DownloadKind::Model,
                        current,
                        total,
                        stage,
                    } => {
                        self.model_progress = Some((current, total, stage.to_string()));
                        self.status = format!(
                            "Model {stage}: {} / {}",
                            crate::download::human_size(current),
                            total.map(crate::download::human_size).unwrap_or_else(|| "?".into())
                        );
                    }
                    DownloadEvent::Error {
                        kind: DownloadKind::Runtime,
                        err,
                    } => {
                        self.runtime_progress = None;
                        self.status = format!("Runtime err: {err}");
                    }
                    DownloadEvent::Error {
                        kind: DownloadKind::Model,
                        err,
                    } => {
                        self.model_progress = None;
                        self.status = format!("Model err: {err}");
                    }
                }
                ctx.request_repaint();
            }
        }

        // Drain server logs
        if let Some(lrx) = &self.log_rx {
            while let Ok(line) = lrx.try_recv() {
                if line.starts_with("[READY]") {
                    self.server_ready = true;
                    self.server_status = ServerStatus::Running;
                    self.status = "Server ready".into();
                }
                if let Some(rest) = line.strip_prefix("[MODEL] ") {
                    self.served_model_id = Some(rest.to_string());
                }
                self.server_log.push(line);
                if self.server_log.len() > 2000 {
                    let drop = self.server_log.len() - 2000;
                    self.server_log.drain(0..drop);
                }
                ctx.request_repaint();
            }
        }

        // Automatic server management
        self.check_server_timeout();

        // Auto-start server when user is active and has messages
        if !self.msgs.is_empty() && !self.input.is_empty() {
            self.mark_activity();
            self.ensure_server_running();
        }

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Status and controls on the left
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        // Runtime status
                        if let Some(runtime) = &self.current_runtime {
                            ui.colored_label(Color32::from_rgb(166, 227, 161),
                                format!("🖥️ {}", runtime.name));
                        } else {
                            ui.colored_label(Color32::from_rgb(243, 139, 168), "❌ No Runtime");
                        }

                        ui.separator();

                        // Server status
                        match &self.server_status {
                            ServerStatus::Running => {
                                ui.colored_label(Color32::from_rgb(166, 227, 161), "🟢 Server Ready");
                            }
                            ServerStatus::Starting => {
                                ui.colored_label(Color32::from_rgb(249, 226, 175), "⏳ Server Starting...");
                            }
                            _ => {
                                ui.colored_label(Color32::from_rgb(243, 139, 168), "🛑 Server Stopped");
                            }
                        }

                        ui.separator();

                        // Current status
                        ui.label(&self.status);
                    });

                    // Quick actions
                    ui.horizontal(|ui| {
                        if ui.add(crate::ui::light_button("⚙️ Settings", Color32::from_rgb(137, 180, 250))).clicked() {
                            self.show_settings = !self.show_settings;
                        }

                        if self.server_ready {
                            if ui.add(crate::ui::light_button("💬 New Chat", Color32::from_rgb(166, 227, 161))).clicked() {
                                self.msgs.clear();
                                self.input.clear();
                                self.editing = None;
                                self.status = "New chat started".into();
                            }
                        }
                    });
                });

                ui.separator();

                // Model management on the right
                ui.vertical(|ui| {
                    crate::ui_models::render_downloaded_models(self, ui);
                });
            });
        });

        // Settings panel (shown when toggled)
        if self.show_settings {
            egui::SidePanel::right("settings")
                .default_width(300.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        crate::ui_settings::render_settings_panel(self, ui);
                    });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            crate::ui_chat::render_chat_panel(self, ui);
        });
    }
}
