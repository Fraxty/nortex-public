//! Central font sizes and optional local font loading.

use std::sync::Arc;

use eframe::egui::{FontData, FontDefinitions, FontFamily};

pub struct Typography;

impl Typography {
    pub const TITLE: f32 = 20.0;
    pub const PANEL_TITLE: f32 = 14.0;
    pub const BODY: f32 = 14.0;
    pub const SECONDARY: f32 = 13.0;
    pub const EDITOR: f32 = 15.0;
    pub const TERMINAL: f32 = 14.0;
    pub const STATUS: f32 = 12.0;
}

/// Zoom levels are independent: UI, editor and terminal scale separately.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zoom {
    pub ui: f32,
    pub editor: f32,
    pub terminal: f32,
}

impl Default for Zoom {
    fn default() -> Self {
        Self {
            ui: 1.0,
            editor: 1.0,
            terminal: 1.0,
        }
    }
}

pub const MIN_ZOOM: f32 = 0.7;
pub const MAX_ZOOM: f32 = 2.0;

pub fn clamp_zoom(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(MIN_ZOOM, MAX_ZOOM)
    } else {
        1.0
    }
}

const INTER_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.otf");
const JETBRAINS_MONO_REGULAR: &[u8] =
    include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");

/// Inter for the proportional UI font, JetBrains Mono for code and terminal.
/// Both are embedded at compile time; egui's built-in fonts stay as glyph fallback.
pub fn font_definitions() -> FontDefinitions {
    let mut definitions = FontDefinitions::default();
    for (name, bytes, family) in [
        ("Inter", INTER_REGULAR, FontFamily::Proportional),
        (
            "JetBrainsMono",
            JETBRAINS_MONO_REGULAR,
            FontFamily::Monospace,
        ),
    ] {
        definitions
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
        definitions
            .families
            .entry(family)
            .or_default()
            .insert(0, name.to_owned());
    }
    definitions
}
