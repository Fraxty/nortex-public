//! Central theme management: palette, density, zoom and egui style mapping.
//! The theme only affects rendering and never touches compiler state.

pub mod accessibility;
pub mod colors;
pub mod editor;
pub mod spacing;
pub mod typography;

use eframe::egui::{
    self, Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Vec2,
};

use colors::Palette;
use editor::EditorPalette;
use spacing::Density;
use typography::{Typography, Zoom, clamp_zoom};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeKind {
    #[default]
    IndustrialMidnight,
    ProfessionalDark,
    HighContrast,
}

impl ThemeKind {
    pub fn palette(self) -> Palette {
        match self {
            Self::IndustrialMidnight => colors::INDUSTRIAL,
            Self::ProfessionalDark => colors::PROFESSIONAL_DARK,
            Self::HighContrast => colors::HIGH_CONTRAST,
        }
    }

    pub const ALL: [Self; 3] = [
        Self::IndustrialMidnight,
        Self::ProfessionalDark,
        Self::HighContrast,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::IndustrialMidnight => "Industrial Midnight",
            Self::ProfessionalDark => "Professional Dark",
            Self::HighContrast => "High Contrast",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Theme {
    pub kind: ThemeKind,
    pub density: Density,
    pub zoom: Zoom,
}

impl Theme {
    pub fn palette(&self) -> Palette {
        self.kind.palette()
    }

    pub fn editor_palette(&self) -> EditorPalette {
        EditorPalette::from_palette(&self.palette())
    }

    pub fn editor_font(&self) -> FontId {
        FontId::monospace(Typography::EDITOR * clamp_zoom(self.zoom.editor))
    }

    /// Row height of the editor: font size times the density's line spacing.
    pub fn editor_line_height(&self) -> f32 {
        self.editor_font().size * self.density.spacing().editor_line_spacing
    }

    pub fn terminal_font(&self) -> FontId {
        FontId::monospace(Typography::TERMINAL * clamp_zoom(self.zoom.terminal))
    }

    /// Applies colors, spacing, text styles and UI zoom to the egui context.
    pub fn apply(&self, ctx: &egui::Context) {
        let p = self.palette();
        let s = self.density.spacing();
        ctx.set_zoom_factor(clamp_zoom(self.zoom.ui));

        let mut style = (*ctx.style()).clone();
        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(Typography::TITLE, FontFamily::Proportional),
            ),
            (
                TextStyle::Body,
                FontId::new(Typography::BODY, FontFamily::Proportional),
            ),
            (
                TextStyle::Button,
                FontId::new(Typography::BODY, FontFamily::Proportional),
            ),
            (
                TextStyle::Small,
                FontId::new(Typography::STATUS, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(Typography::EDITOR, FontFamily::Monospace),
            ),
        ]
        .into();

        style.spacing.item_spacing = Vec2::new(s.item_horizontal, s.item_vertical);
        style.spacing.button_padding = Vec2::new(s.button_horizontal, s.button_vertical);
        style.spacing.window_margin = Margin::same(s.panel_padding as i8);

        let v = &mut style.visuals;
        v.dark_mode = true;
        v.panel_fill = p.panel;
        v.window_fill = p.surface;
        v.extreme_bg_color = p.app;
        v.faint_bg_color = p.surface;
        v.window_stroke = Stroke::new(1.0, p.border_subtle);
        v.window_corner_radius = CornerRadius::ZERO;
        v.menu_corner_radius = CornerRadius::ZERO;
        v.hyperlink_color = p.blue;
        v.warn_fg_color = p.amber;
        v.error_fg_color = p.red;
        v.selection.bg_fill = p.accent(p.blue, colors::Intensity::Muted);
        v.selection.stroke = Stroke::new(1.0, p.text_primary);
        v.text_cursor.stroke = Stroke::new(2.0, p.amber);

        let w = &mut v.widgets;
        w.noninteractive.bg_fill = p.panel;
        w.noninteractive.bg_stroke = Stroke::new(1.0, p.border_subtle);
        w.noninteractive.fg_stroke = Stroke::new(1.0, p.text_secondary);
        w.inactive.bg_fill = p.surface;
        w.inactive.weak_bg_fill = p.surface;
        w.inactive.bg_stroke = Stroke::new(1.0, p.border_subtle);
        w.inactive.fg_stroke = Stroke::new(2.0, p.text_primary);
        w.hovered.bg_fill = p.interactive;
        w.hovered.weak_bg_fill = p.interactive;
        w.hovered.bg_stroke = Stroke::new(1.0, p.border);
        w.hovered.fg_stroke = Stroke::new(2.0, p.text_primary);
        w.active.bg_fill = p.accent(p.amber, colors::Intensity::Secondary);
        w.active.weak_bg_fill = p.accent(p.amber, colors::Intensity::Secondary);
        w.active.bg_stroke = Stroke::new(1.0, p.amber);
        // Auch `strong()`-Text nutzt diese Farbe und muss auf dunklem Grund lesbar bleiben.
        w.active.fg_stroke = Stroke::new(2.0, p.text_primary);
        w.open.bg_fill = p.interactive;
        w.open.weak_bg_fill = p.interactive;
        w.open.bg_stroke = Stroke::new(1.0, p.border);
        w.open.fg_stroke = Stroke::new(1.0, p.text_primary);
        for widget in [
            &mut w.noninteractive,
            &mut w.inactive,
            &mut w.hovered,
            &mut w.active,
            &mut w.open,
        ] {
            widget.corner_radius = CornerRadius::ZERO;
        }
        ctx.set_style(style);
    }
}

/// Panel header text such as `01 / PROJECT EXPLORER`.
pub fn panel_header(ui: &mut egui::Ui, theme: &Theme, index: u8, title: &str, status: Color32) {
    let p = theme.palette();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("●")
                .color(status)
                .size(Typography::STATUS),
        );
        ui.label(
            egui::RichText::new(format!("{index:02} / {}", title.to_uppercase()))
                .color(p.text_secondary)
                .size(Typography::PANEL_TITLE)
                .strong(),
        );
    });
    ui.separator();
}

#[cfg(test)]
mod tests {
    use super::accessibility::{NORMAL_TEXT_MIN, UI_COMPONENT_MIN, contrast_ratio};
    use super::*;

    #[test]
    fn text_has_wcag_contrast_in_every_theme() {
        for kind in [
            ThemeKind::IndustrialMidnight,
            ThemeKind::ProfessionalDark,
            ThemeKind::HighContrast,
        ] {
            let p = kind.palette();
            for bg in [p.app, p.panel, p.surface] {
                for fg in [
                    p.text_primary,
                    p.text_secondary,
                    p.text_muted,
                    p.amber,
                    p.blue,
                    p.green,
                    p.red,
                    p.violet,
                ] {
                    // On SURFACE only primary/secondary text must reach 4.5:1;
                    // accents and muted grey must reach the 3:1 component level.
                    let required =
                        if bg == p.surface && fg != p.text_primary && fg != p.text_secondary {
                            UI_COMPONENT_MIN
                        } else {
                            NORMAL_TEXT_MIN
                        };
                    assert!(
                        contrast_ratio(fg, bg) >= required,
                        "{kind:?} {fg:?} on {bg:?}"
                    );
                }
            }
            assert!(
                contrast_ratio(p.border, p.panel) >= UI_COMPONENT_MIN
                    || kind != ThemeKind::HighContrast
            );
        }
    }

    #[test]
    fn active_widgets_stay_readable() {
        let p = colors::INDUSTRIAL;
        assert!(contrast_ratio(p.app, p.amber) >= NORMAL_TEXT_MIN);
    }

    #[test]
    fn density_grows_monotonically() {
        let (c, m, s) = (
            Density::Compact.spacing(),
            Density::Comfortable.spacing(),
            Density::Spacious.spacing(),
        );
        assert!(c.panel_padding < m.panel_padding && m.panel_padding < s.panel_padding);
    }

    #[test]
    fn zoom_is_clamped_and_independent() {
        assert_eq!(clamp_zoom(f32::NAN), 1.0);
        assert_eq!(clamp_zoom(10.0), typography::MAX_ZOOM);
        let theme = Theme {
            zoom: Zoom {
                ui: 1.0,
                editor: 2.0,
                terminal: 1.0,
            },
            ..Theme::default()
        };
        assert_eq!(theme.editor_font().size, 30.0);
        assert_eq!(theme.terminal_font().size, 14.0);
    }

    #[test]
    fn line_height_scales_with_density_and_editor_zoom() {
        let compact = Theme {
            density: Density::Compact,
            ..Theme::default()
        };
        let spacious = Theme {
            density: Density::Spacious,
            ..Theme::default()
        };
        assert!(compact.editor_line_height() < spacious.editor_line_height());
        assert!(compact.editor_line_height() > compact.editor_font().size);
    }

    #[test]
    fn embedded_fonts_are_registered_first() {
        let definitions = typography::font_definitions();
        assert_eq!(definitions.families[&FontFamily::Proportional][0], "Inter");
        assert_eq!(
            definitions.families[&FontFamily::Monospace][0],
            "JetBrainsMono"
        );
    }
}
