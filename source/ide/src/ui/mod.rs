//! UI-independent NORTEX Code brand palette.

pub mod activity_bar;
pub mod branding;
pub mod breadcrumbs;
pub mod command_palette;
pub mod components;
pub mod dashboard;
pub mod find_bar;
pub mod keymap;
pub mod layout;
pub mod minimap;
pub mod settings;
pub mod settings_catalog;
pub mod status_bar;
pub mod theme;

pub const BRAND_RED_RGB: [u8; 3] = [196, 42, 52];
pub const BRAND_BLUE_RGB: [u8; 3] = [44, 112, 190];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiState {
    pub dark_mode: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self { dark_mode: true }
    }
}
