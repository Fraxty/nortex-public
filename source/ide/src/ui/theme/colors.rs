//! Semantic color roles. Colors keep one meaning each: violet is AI only,
//! red is errors only, green is success only.

use eframe::egui::Color32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub app: Color32,
    pub panel: Color32,
    pub surface: Color32,
    pub interactive: Color32,
    pub border: Color32,
    pub border_subtle: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub text_muted: Color32,
    pub amber: Color32,
    pub blue: Color32,
    pub green: Color32,
    pub red: Color32,
    pub violet: Color32,
    pub grey: Color32,
}

/// Industrial Midnight, the default NORTEX palette.
pub const INDUSTRIAL: Palette = Palette {
    app: Color32::from_rgb(0x15, 0x19, 0x1E),
    panel: Color32::from_rgb(0x1D, 0x23, 0x2A),
    surface: Color32::from_rgb(0x25, 0x2D, 0x36),
    interactive: Color32::from_rgb(0x30, 0x3B, 0x47),
    border: Color32::from_rgb(0x46, 0x53, 0x60),
    border_subtle: Color32::from_rgb(0x32, 0x3C, 0x46),
    text_primary: Color32::from_rgb(0xE6, 0xE9, 0xED),
    text_secondary: Color32::from_rgb(0xA6, 0xB0, 0xBB),
    text_muted: Color32::from_rgb(0x82, 0x8D, 0x99),
    amber: Color32::from_rgb(0xD6, 0xA6, 0x4B),
    blue: Color32::from_rgb(0x72, 0x98, 0xB8),
    green: Color32::from_rgb(0x70, 0xA8, 0x89),
    red: Color32::from_rgb(0xD6, 0x6C, 0x6C),
    violet: Color32::from_rgb(0x9B, 0x87, 0xC8),
    grey: Color32::from_rgb(0x92, 0x9B, 0xA5),
};

/// Calmer, slightly lighter alternative.
pub const PROFESSIONAL_DARK: Palette = Palette {
    app: Color32::from_rgb(0x1E, 0x1E, 0x1E),
    panel: Color32::from_rgb(0x25, 0x25, 0x26),
    surface: Color32::from_rgb(0x2D, 0x2D, 0x30),
    interactive: Color32::from_rgb(0x3A, 0x3D, 0x41),
    border: Color32::from_rgb(0x55, 0x55, 0x58),
    border_subtle: Color32::from_rgb(0x3C, 0x3C, 0x3F),
    text_primary: Color32::from_rgb(0xD4, 0xD4, 0xD4),
    text_secondary: Color32::from_rgb(0xA8, 0xA8, 0xA8),
    text_muted: Color32::from_rgb(0x95, 0x95, 0x95),
    amber: Color32::from_rgb(0xD7, 0xBA, 0x7D),
    blue: Color32::from_rgb(0x56, 0x9C, 0xD6),
    green: Color32::from_rgb(0x6A, 0xA9, 0x4F),
    red: Color32::from_rgb(0xF4, 0x71, 0x71),
    violet: Color32::from_rgb(0xB1, 0x8C, 0xE0),
    grey: Color32::from_rgb(0x9D, 0x9D, 0x9D),
};

/// Maximum separation between surfaces, text and borders.
pub const HIGH_CONTRAST: Palette = Palette {
    app: Color32::from_rgb(0x00, 0x00, 0x00),
    panel: Color32::from_rgb(0x0A, 0x0A, 0x0A),
    surface: Color32::from_rgb(0x1A, 0x1A, 0x1A),
    interactive: Color32::from_rgb(0x2E, 0x2E, 0x2E),
    border: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    border_subtle: Color32::from_rgb(0x8C, 0x8C, 0x8C),
    text_primary: Color32::from_rgb(0xFF, 0xFF, 0xFF),
    text_secondary: Color32::from_rgb(0xE0, 0xE0, 0xE0),
    text_muted: Color32::from_rgb(0xBF, 0xBF, 0xBF),
    amber: Color32::from_rgb(0xFF, 0xC8, 0x57),
    blue: Color32::from_rgb(0x6F, 0xB8, 0xFF),
    green: Color32::from_rgb(0x5F, 0xE0, 0x8A),
    red: Color32::from_rgb(0xFF, 0x80, 0x80),
    violet: Color32::from_rgb(0xC0, 0xA8, 0xFF),
    grey: Color32::from_rgb(0xD0, 0xD0, 0xD0),
};

/// How strongly an accent is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intensity {
    Primary,
    Secondary,
    Muted,
}

/// Blends `color` over `background`; `alpha` is 0..=255.
pub fn blend(color: Color32, background: Color32, alpha: u8) -> Color32 {
    let mix = |foreground: u8, base: u8| {
        let alpha = u16::from(alpha);
        ((u16::from(foreground) * alpha + u16::from(base) * (255 - alpha)) / 255) as u8
    };
    Color32::from_rgb(
        mix(color.r(), background.r()),
        mix(color.g(), background.g()),
        mix(color.b(), background.b()),
    )
}

impl Palette {
    pub fn accent(&self, color: Color32, intensity: Intensity) -> Color32 {
        match intensity {
            Intensity::Primary => color,
            Intensity::Secondary => blend(color, self.panel, 190),
            Intensity::Muted => blend(color, self.panel, 70),
        }
    }
}
