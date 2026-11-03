//! Breadcrumb-Zeile über der Code-Fläche: `src/main.nx > struct Vector3 > fn normalize()`.

use eframe::egui::{self, Color32, RichText, Sense};

use crate::editor::scope::Scope;

pub const HEIGHT: f32 = 22.0;
const BG: Color32 = Color32::from_rgb(0x14, 0x17, 0x1B);
const TEXT: Color32 = Color32::from_rgb(0x8A, 0x95, 0xA5);
const AMBER: Color32 = Color32::from_rgb(0xD6, 0xA6, 0x4B);

/// Zeichnet die Zeile; liefert die Zeile (nullbasiert), auf die ein angeklicktes Segment zeigt.
pub fn show(ui: &mut egui::Ui, file_label: &str, path: &[Scope]) -> Option<usize> {
    let mut jump = None;
    egui::Frame::new()
        .fill(BG)
        .inner_margin(egui::Margin::symmetric(8, 3))
        .show(ui, |ui| {
            ui.set_min_height(HEIGHT - 6.0);
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(RichText::new(file_label).color(TEXT).size(12.0));
                for (index, scope) in path.iter().enumerate() {
                    ui.label(RichText::new(">").color(TEXT).size(12.0));
                    let last = index + 1 == path.len();
                    let label = RichText::new(scope.label())
                        .color(if last { AMBER } else { TEXT })
                        .size(12.0);
                    if ui
                        .add(egui::Label::new(label).sense(Sense::click()))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        jump = Some(scope.start_line);
                    }
                }
            });
        });
    jump
}
