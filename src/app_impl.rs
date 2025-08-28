use crate::models::*;
use eframe::egui;

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

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            crate::ui_top::render_top_panel(self, ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            crate::ui_chat::render_chat_panel(self, ui);
        });
    }
}
