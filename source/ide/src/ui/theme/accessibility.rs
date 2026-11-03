//! WCAG contrast helpers and accessibility switches.

use eframe::egui::Color32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Accessibility {
    pub high_contrast: bool,
    pub reduced_motion: bool,
}

fn channel(value: u8) -> f64 {
    let value = f64::from(value) / 255.0;
    if value <= 0.039_28 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub fn relative_luminance(color: Color32) -> f64 {
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

/// WCAG contrast ratio between two opaque colors (1.0 to 21.0).
pub fn contrast_ratio(a: Color32, b: Color32) -> f64 {
    let (a, b) = (relative_luminance(a), relative_luminance(b));
    let (light, dark) = if a >= b { (a, b) } else { (b, a) };
    (light + 0.05) / (dark + 0.05)
}

pub const NORMAL_TEXT_MIN: f64 = 4.5;
pub const UI_COMPONENT_MIN: f64 = 3.0;
