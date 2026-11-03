//! Schmale Code-Miniatur am rechten Rand des Editors. Geometrie ist rein berechnet und testbar.

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};

pub const WIDTH: f32 = 60.0;
const MAX_LINE_PX: f32 = 2.0;

/// Höhe einer Textzeile in der Miniatur; schrumpft bei langen Dateien.
pub fn px_per_line(total_lines: usize, height: f32) -> f32 {
    (height / total_lines.max(1) as f32).clamp(0.0, MAX_LINE_PX)
}

/// (Oberkante, Höhe) des Sichtfenster-Rechtecks relativ zur Miniatur.
pub fn viewport_span(offset: f32, view_h: f32, content_h: f32, map_h: f32) -> (f32, f32) {
    if content_h <= 0.0 || map_h <= 0.0 {
        return (0.0, map_h.max(0.0));
    }
    let height = (view_h / content_h * map_h).clamp(4.0_f32.min(map_h), map_h);
    let top = (offset / content_h * map_h).clamp(0.0, (map_h - height).max(0.0));
    (top, height)
}

/// Scroll-Offset, der die angeklickte Miniatur-Position in der Fenstermitte zeigt.
pub fn scroll_for_click(y: f32, map_h: f32, content_h: f32, view_h: f32) -> f32 {
    if map_h <= 0.0 {
        return 0.0;
    }
    let target = (y / map_h).clamp(0.0, 1.0) * content_h - view_h / 2.0;
    target.clamp(0.0, (content_h - view_h).max(0.0))
}

/// Zeichnet die Miniatur über `area` (rechter Rand des Editors); liefert den neuen Scroll-Offset
/// bei Klick/Ziehen.
pub fn show(
    ui: &mut egui::Ui,
    area: Rect,
    text: &str,
    offset: f32,
    content_h: f32,
    tint: Color32,
    accent: Color32,
) -> Option<f32> {
    let total = text.lines().count().max(1);
    let px = px_per_line(total, area.height());
    let map_h = (px * total as f32).min(area.height());
    let rect = Rect::from_min_size(
        Pos2::new(area.right() - WIDTH, area.top()),
        Vec2::new(WIDTH, area.height()),
    );
    let response = ui.interact(rect, egui::Id::new("minimap"), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgba_unmultiplied(18, 20, 24, 200));

    let step = if px >= 1.0 {
        1
    } else {
        (1.0 / px.max(0.001)).ceil() as usize
    };
    let bar_h = px.max(1.0) - if px >= 2.0 { 0.5 } else { 0.0 };
    for (index, line) in text.lines().enumerate().step_by(step) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let indent = (line.len() - trimmed.len()).min(16) as f32 * 0.8;
        let width = (trimmed.len().min(60) as f32 / 60.0) * (WIDTH - 10.0 - indent);
        let y = rect.top() + index as f32 * px;
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left() + 4.0 + indent, y),
                Vec2::new(width.max(1.0), bar_h),
            ),
            0.0,
            tint,
        );
    }

    let (top, height) = viewport_span(offset, area.height(), content_h, map_h);
    let view = Rect::from_min_size(
        Pos2::new(rect.left(), rect.top() + top),
        Vec2::new(WIDTH, height),
    );
    painter.rect_filled(view, 0.0, accent.gamma_multiply(0.18));
    painter.rect_stroke(
        view,
        0.0,
        egui::Stroke::new(1.0, accent.gamma_multiply(0.5)),
        egui::StrokeKind::Inside,
    );

    if (response.clicked() || response.dragged())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        return Some(scroll_for_click(
            pointer.y - rect.top(),
            map_h,
            content_h,
            area.height(),
        ));
    }
    None
}
