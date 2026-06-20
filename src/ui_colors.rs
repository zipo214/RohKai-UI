//! Shared RohKai UI color helpers.

use eframe::egui;

/// A darker emerald surface that keeps the mint identity while giving text
/// enough contrast when the accent is used as a filled selection/chip color.
pub(crate) fn accent_fill_for_text(accent: egui::Color32) -> egui::Color32 {
    let [r, g, b, _] = accent.to_array();
    egui::Color32::from_rgb(
        (r as u16 * 3 / 10).min(72) as u8,
        (g as u16 * 11 / 20).min(124) as u8,
        (b as u16 / 2).min(108) as u8,
    )
}

/// Foreground for text drawn on [`accent_fill_for_text`].
pub(crate) fn text_on_accent_fill() -> egui::Color32 {
    egui::Color32::from_rgb(236, 253, 245)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn relative_luminance(c: egui::Color32) -> f32 {
        fn channel(v: u8) -> f32 {
            let v = f32::from(v) / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }

        0.2126 * channel(c.r()) + 0.7152 * channel(c.g()) + 0.0722 * channel(c.b())
    }

    fn contrast_ratio(a: egui::Color32, b: egui::Color32) -> f32 {
        let a = relative_luminance(a);
        let b = relative_luminance(b);
        let (lighter, darker) = if a > b { (a, b) } else { (b, a) };
        (lighter + 0.05) / (darker + 0.05)
    }

    #[test]
    fn accent_fill_keeps_text_legible() {
        let contrast = contrast_ratio(
            accent_fill_for_text(egui::Color32::from_rgb(52, 211, 153)),
            text_on_accent_fill(),
        );
        assert!(
            contrast >= 4.5,
            "accent fill text contrast should meet normal text threshold, got {contrast:.2}"
        );
    }
}
