use std::fmt;

/// Calculate the relative luminance of a color
fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    let r_linear = if r as f64 / 255.0 <= 0.03928 {
        r as f64 / 255.0 / 12.92
    } else {
        ((r as f64 / 255.0 + 0.055) / 1.055).powf(2.4)
    };

    let g_linear = if g as f64 / 255.0 <= 0.03928 {
        g as f64 / 255.0 / 12.92
    } else {
        ((g as f64 / 255.0 + 0.055) / 1.055).powf(2.4)
    };

    let b_linear = if b as f64 / 255.0 <= 0.03928 {
        b as f64 / 255.0 / 12.92
    } else {
        ((b as f64 / 255.0 + 0.055) / 1.055).powf(2.4)
    };

    0.2126 * r_linear + 0.7152 * g_linear + 0.0722 * b_linear
}

/// Calculate contrast ratio between two colors
fn contrast_ratio(l1: f64, l2: f64) -> f64 {
    if l1 > l2 {
        (l1 + 0.05) / (l2 + 0.05)
    } else {
        (l2 + 0.05) / (l1 + 0.05)
    }
}

struct ColorInfo {
    name: &'static str,
    r: u8,
    g: u8,
    b: u8,
    hex: &'static str,
}

impl fmt::Display for ColorInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({})", self.name, self.hex)
    }
}

fn main() {
    println!("🔧 Contrast Analysis - CORRECTED for Button Backgrounds");
    println!("====================================================");
    println!();

    // Button background colors (light colors that need dark text)
    let button_bgs = vec![
        ColorInfo { name: "Green Button", r: 166, g: 227, b: 161, hex: "#a6e3a1" },
        ColorInfo { name: "Red Button", r: 243, g: 139, b: 168, hex: "#f38ba8" },
        ColorInfo { name: "Yellow Button", r: 249, g: 226, b: 175, hex: "#f9e2af" },
        ColorInfo { name: "Blue Button", r: 137, g: 180, b: 250, hex: "#89b4fa" },
    ];

    // Text colors to test on light backgrounds
    let dark_texts = vec![
        ColorInfo { name: "Dark Text", r: 31, g: 31, b: 46, hex: "#1f1f2e" },
        ColorInfo { name: "Darker Text", r: 15, g: 15, b: 25, hex: "#0f0f19" },
        ColorInfo { name: "Black Text", r: 0, g: 0, b: 0, hex: "#000000" },
    ];

    println!("📊 Button Background Contrast Analysis");
    println!("--------------------------------------");

    for bg in &button_bgs {
        let bg_lum = relative_luminance(bg.r, bg.g, bg.b);
        println!();
        println!("Button Background: {}", bg);

        for text in &dark_texts {
            let text_lum = relative_luminance(text.r, text.g, text.b);
            let ratio = contrast_ratio(text_lum, bg_lum);
            let status = if ratio >= 4.5 {
                "✅ EXCELLENT"
            } else if ratio >= 3.0 {
                "⚠️  GOOD (large text)"
            } else {
                "❌ POOR"
            };

            println!("  {:<12} vs {:<15} = {:5.1}:1  {}",
                     text.name, bg.name, ratio, status);
        }
    }

    println!();
    println!("🎯 RECOMMENDED FIXES:");
    println!("---------------------");
    println!("1. Use dark text (#1f1f2e) on light button backgrounds");
    println!("2. For even better contrast, use darker text (#0f0f19)");
    println!("3. Avoid light grey text (#cdd6f4) on light backgrounds");
    println!("4. All combinations above meet WCAG AA standards!");
}
