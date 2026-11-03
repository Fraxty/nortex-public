//! Reusable industrial widgets. They read only theme values, never own colors.

use crate::i18n::tr;
use eframe::egui::{self, CornerRadius, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};

use super::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

/// Sharp-cornered button; hover fades in over 120 ms (instant with reduced motion).
pub struct IndustrialButton<'a> {
    label: &'a str,
    kind: ButtonKind,
    theme: &'a Theme,
    enabled: bool,
}

impl<'a> IndustrialButton<'a> {
    pub fn new(theme: &'a Theme, label: &'a str) -> Self {
        Self {
            label,
            kind: ButtonKind::Secondary,
            theme,
            enabled: true,
        }
    }

    pub fn kind(mut self, kind: ButtonKind) -> Self {
        self.kind = kind;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl Widget for IndustrialButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let p = self.theme.palette();
        let padding = ui.spacing().button_padding;
        let galley = ui.painter().layout_no_wrap(
            self.label.to_owned(),
            egui::TextStyle::Button.resolve(ui.style()),
            p.text_primary,
        );
        let size = galley.size() + padding * 2.0;
        let sense = if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(size, sense);
        let hover =
            ui.ctx()
                .animate_bool_with_time(response.id, self.enabled && response.hovered(), 0.12);
        let pressed = self.enabled && response.is_pointer_button_down_on();

        let (base, accent) = match self.kind {
            ButtonKind::Primary => (p.amber, p.amber),
            ButtonKind::Secondary => (p.surface, p.border),
            ButtonKind::Ghost => (p.panel, p.border_subtle),
            ButtonKind::Danger => (p.surface, p.red),
        };
        let fill = if pressed {
            p.interactive
        } else {
            super::theme::colors::blend(p.interactive, base, (hover * 255.0) as u8)
        };
        let (fill, text) = if self.kind == ButtonKind::Primary && !pressed {
            (
                super::theme::colors::blend(p.text_primary, fill, (hover * 40.0) as u8),
                p.app,
            )
        } else {
            (fill, p.text_primary)
        };
        let text = if self.enabled { text } else { p.text_muted };
        let stroke_color = if response.has_focus() {
            p.amber
        } else if self.enabled {
            super::theme::colors::blend(p.amber, accent, (hover * 255.0) as u8)
        } else {
            p.border_subtle
        };

        let painter = ui.painter();
        painter.rect(
            rect,
            CornerRadius::ZERO,
            fill,
            Stroke::new(1.0, stroke_color),
            StrokeKind::Inside,
        );
        painter.galley(rect.center() - galley.size() / 2.0, galley, text);
        response
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FileTabResponse {
    pub activated: bool,
    pub closed: bool,
}

/// Display state of a single editor tab.
#[derive(Debug, Clone, Copy)]
pub struct FileTabState<'a> {
    pub name: &'a str,
    pub active: bool,
    pub modified: bool,
    pub has_error: bool,
}

/// Editor tab with an amber underline for the active file, a modified dot and a close button.
pub fn file_tab(
    ui: &mut Ui,
    theme: &Theme,
    state: FileTabState<'_>,
    _drag_id: egui::Id,
    drag_payload: impl Send + Sync + 'static,
) -> FileTabResponse {
    let FileTabState {
        name,
        active,
        modified,
        has_error,
    } = state;
    let p = theme.palette();
    let mut result = FileTabResponse::default();
    let fill = if active {
        egui::Color32::from_rgb(0x1E, 0x22, 0x28)
    } else {
        egui::Color32::from_rgb(0x14, 0x17, 0x1B)
    };
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, p.border_subtle))
        .corner_radius(CornerRadius::ZERO)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if has_error {
                    ui.colored_label(p.red, "●");
                }
                let color = if active {
                    p.text_primary
                } else {
                    p.text_secondary
                };
                let label = ui.add(
                    egui::Label::new(egui::RichText::new(name).color(color))
                        .sense(Sense::click_and_drag()),
                );
                if label.drag_started() {
                    label.dnd_set_drag_payload(drag_payload);
                }
                if label.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::Default);
                }
                result.activated = label.clicked();
                if modified {
                    ui.colored_label(p.amber, "•");
                }
                let (close_rect, close) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::click());
                let close = close.on_hover_text(tr("Schließen"));
                if close.hovered() {
                    ui.painter().rect_filled(
                        close_rect,
                        CornerRadius::ZERO,
                        p.amber.gamma_multiply(0.25),
                    );
                    ui.painter().rect_stroke(
                        close_rect,
                        CornerRadius::ZERO,
                        Stroke::new(1.0, p.amber),
                        StrokeKind::Inside,
                    );
                }
                let cross = if close.hovered() {
                    p.text_primary
                } else {
                    p.text_muted
                };
                let c = close_rect.center();
                let stroke = Stroke::new(1.5, cross);
                ui.painter()
                    .line_segment([c + Vec2::new(-4.0, -4.0), c + Vec2::new(4.0, 4.0)], stroke);
                ui.painter()
                    .line_segment([c + Vec2::new(-4.0, 4.0), c + Vec2::new(4.0, -4.0)], stroke);
                result.closed = close.clicked() || label.middle_clicked();
            });
        });
    if active {
        let rect = ui.min_rect();
        let y = rect.bottom();
        ui.painter()
            .hline(ui.min_rect().x_range(), y, Stroke::new(2.0, p.amber));
    }
    result
}
