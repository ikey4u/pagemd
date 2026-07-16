use std::path::{Path, PathBuf};

use crate::core::ConvertOptions;

/// Default favicon label from free text (first two alphanumeric chars, uppercase).
pub fn default_icon_label_from_text(text: &str) -> String {
    let chars: Vec<char> = text
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(2)
        .collect();
    match chars.len() {
        0 => "PG".to_string(),
        1 => {
            let c = chars[0].to_ascii_uppercase();
            format!("{c}{c}")
        }
        _ => chars.into_iter().map(|c| c.to_ascii_uppercase()).collect(),
    }
}

/// Default favicon label from the input path (first two alphanumeric chars of the stem, uppercase).
pub fn default_icon_label_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    default_icon_label_from_text(stem)
}

pub fn resolve_icon_label(opts: &ConvertOptions, resolved_inputs: &[PathBuf]) -> String {
    if let Some(icon) = &opts.icon {
        return icon.clone();
    }
    resolved_inputs
        .first()
        .map(|p| default_icon_label_from_path(p))
        .unwrap_or_else(|| "PG".to_string())
}

fn hash_icon_label(label: &str) -> u32 {
    let mut hash: u32 = 5381;
    for byte in label.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u32::from(byte));
    }
    hash
}

/// Deterministic saturated background from icon text (HSL: hue from hash, fixed S/L).
pub fn icon_background_rgb(label: &str) -> (u8, u8, u8) {
    let hue = f64::from(hash_icon_label(label) % 360);
    hsl_to_rgb(hue, 0.62, 0.48)
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let h = (h / 360.0).fract();
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h * 6.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = match (h * 6.0).floor() as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((g + m) * 255.0).round().clamp(0.0, 255.0) as u8,
        ((b + m) * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn srgb_channel(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG 2.x relative luminance.
pub fn relative_luminance(rgb: (u8, u8, u8)) -> f64 {
    let r = srgb_channel(rgb.0);
    let g = srgb_channel(rgb.1);
    let b = srgb_channel(rgb.2);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

pub fn contrast_ratio(lighter: f64, darker: f64) -> f64 {
    (lighter + 0.05) / (darker + 0.05)
}

/// Pick black or white text for maximum contrast on the given background.
fn icon_foreground_rgb(background: (u8, u8, u8)) -> (u8, u8, u8) {
    let bg_l = relative_luminance(background);
    let white = 1.0;
    let black = 0.0;
    let on_white = contrast_ratio(white, bg_l);
    let on_black = contrast_ratio(bg_l, black);
    if on_white >= on_black {
        (255, 255, 255)
    } else {
        (17, 17, 17)
    }
}

pub fn icon_colors(label: &str) -> ((u8, u8, u8), (u8, u8, u8)) {
    let bg = icon_background_rgb(label);
    let fg = icon_foreground_rgb(bg);
    (bg, fg)
}

fn encode_svg_for_data_uri(svg: &str) -> String {
    svg.chars()
        .map(|c| match c {
            '#' => "%23".to_string(),
            '%' => "%25".to_string(),
            '<' => "%3C".to_string(),
            '>' => "%3E".to_string(),
            '"' => "%22".to_string(),
            '\'' => "%27".to_string(),
            '&' => "%26".to_string(),
            '+' => "%2B".to_string(),
            ' ' => "%20".to_string(),
            _ if c.is_ascii() => c.to_string(),
            c => {
                let mut buf = [0u8; 4];
                let encoded = c.encode_utf8(&mut buf);
                encoded.bytes().map(|b| format!("%{b:02X}")).collect()
            }
        })
        .collect()
}

pub fn favicon_link_tag(label: &str) -> String {
    let label = label.to_ascii_uppercase();
    let ((br, bg, bb), (fr, fg, fb)) = icon_colors(&label);
    const ICON_RX: u32 = 7;
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32' viewBox='0 0 32 32'><rect width='32' height='32' rx='{ICON_RX}' ry='{ICON_RX}' fill='#{br:02x}{bg:02x}{bb:02x}'/><text x='16' y='16' text-anchor='middle' dominant-baseline='central' font-family='system-ui,-apple-system,sans-serif' font-size='18' font-weight='700' letter-spacing='1.5' fill='#{fr:02x}{fg:02x}{fb:02x}'>{label}</text></svg>"
    );
    format!(
        "<link rel=\"icon\" href=\"data:image/svg+xml,{}\">\n",
        encode_svg_for_data_uri(&svg)
    )
}
