use eframe::egui::{self, Color32, Stroke, RichText};

/// Create a button with proper contrast for light backgrounds
pub fn light_button(text: &str, bg_color: Color32) -> egui::Button {
    egui::Button::new(RichText::new(text).color(Color32::from_rgb(31, 31, 46)))
        .fill(bg_color)
}

pub fn setup_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.dark_mode = true;

    // Background colors - very dark for maximum contrast
    style.visuals.panel_fill = Color32::from_rgb(15, 15, 25); // #0f0f19 - Even darker
    style.visuals.window_fill = Color32::from_rgb(10, 10, 18); // #0a0a12 - Darkest
    style.visuals.faint_bg_color = Color32::from_rgb(35, 36, 48); // #232430 - Medium dark

    // Widget backgrounds - progressively lighter
    style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(15, 15, 25); // Same as panel
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(35, 36, 48); // Medium
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(55, 57, 73); // Lighter hover
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(75, 78, 98); // Lightest active

    // Text colors - bright for dark backgrounds, will be overridden for light button backgrounds
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(205, 214, 244));
    style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(205, 214, 244));
    style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, Color32::from_rgb(205, 214, 244));
    style.visuals.widgets.active.fg_stroke = Stroke::new(1.0, Color32::from_rgb(205, 214, 244));

    // Button specific styling
    style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(137, 180, 250));
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(137, 180, 250));
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0, Color32::from_rgb(137, 180, 250));

    ctx.set_style(style);
}
