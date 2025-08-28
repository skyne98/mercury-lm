use crate::models::Msg;
use eframe::egui::{self, Align, Layout, Color32, RichText};

pub fn render_chat_panel(app: &mut crate::app::App, ui: &mut egui::Ui) {
    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        let mut pending_truncate: Option<usize> = None;
        for (i, m) in app.msgs.iter_mut().enumerate() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let icon = if m.role == "user" { "👤" } else { "🤖" };
                    ui.label(RichText::new(format!("{} {}", icon, if m.role == "user" { "You" } else { "Assistant" })).color(Color32::from_rgb(137, 180, 250)));
                    if ui.add(crate::ui::light_button("✏️ Edit", Color32::from_rgb(137, 180, 250))).clicked() {
                        app.editing = Some(i);
                    }
                    if ui.add(crate::ui::light_button("🔄 Restart from here", Color32::from_rgb(249, 226, 175))).clicked() {
                        pending_truncate = Some(i + 1);
                    }
                });
                if app.editing == Some(i) {
                    ui.text_edit_multiline(&mut m.content);
                    if ui.add(crate::ui::light_button("✅ Apply", Color32::from_rgb(166, 227, 161))).clicked() {
                        app.editing = None;
                    }
                } else {
                    ui.label(&m.content);
                }
            });
        }
        if let Some(t) = pending_truncate {
            app.msgs.truncate(t);
        }
        ui.separator();
        ui.text_edit_multiline(&mut app.input);
        ui.horizontal(|ui| {
            let sending = app.rx.is_some();
            if ui
                .add_enabled(!sending, crate::ui::light_button("📤 Send", Color32::from_rgb(166, 227, 161)))
                .clicked()
            {
                if !app.server_ready {
                    app.status = "Server not ready yet".into();
                }
                let input_text = app.input.trim().to_string();
                if !input_text.is_empty() {
                    match app.msgs.last_mut() {
                        Some(last) if last.role == "user" => {
                            if !last.content.is_empty() {
                                last.content.push_str("\n\n");
                            }
                            last.content.push_str(&input_text);
                        }
                        _ => {
                            app.msgs.push(Msg {
                                role: "user".into(),
                                content: input_text.clone(),
                            });
                        }
                    }
                    app.msgs.push(Msg {
                        role: "assistant".into(),
                        content: String::new(),
                    });
                    let (tx, rx) = std::sync::mpsc::channel::<crate::models::StreamEvent>();
                    app.rx = Some(rx);
                    let url = app.server_url.clone();
                    let msgs = app.msgs.clone();
                    let model = app
                        .served_model_id
                        .clone()
                        .unwrap_or_else(|| "local".into());
                    crate::stream::stream_chat(&url, model, msgs, tx);
                    app.input.clear();
                }
            }
            if sending {
                if ui.add(crate::ui::light_button("❌ Cancel", Color32::from_rgb(243, 139, 168))).clicked() {
                    app.rx = None;
                    app.status = "Canceled".into();
                }
                ui.label(RichText::new("⚡ Generating…").color(Color32::from_rgb(249, 226, 175)));
            } else {
                ui.label(RichText::new("💬 Streaming; edit any message to branch.").color(Color32::from_rgb(186, 194, 222)));
            }
        });
    });
}
