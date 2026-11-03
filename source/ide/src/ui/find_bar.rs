//! Schwebendes Suchen-/Ersetzen-Overlay oben rechts im aktiven Editor-Pane.

use crate::i18n::tr;
use eframe::egui::{self, Color32, CornerRadius, Key, Modifiers, Stroke, StrokeKind};

use crate::editor::find_replace::FindState;

pub const BAR_BG: Color32 = Color32::from_rgb(0x12, 0x14, 0x18);
pub const BAR_BORDER: Color32 = Color32::from_rgb(0x34, 0x3B, 0x43);
pub const AMBER: Color32 = Color32::from_rgb(0xD6, 0xA6, 0x4B);
pub const RED_ERROR: Color32 = Color32::from_rgb(0xD6, 0x6C, 0x6C);
const MAX_WIDTH: f32 = 420.0;

pub fn query_id() -> egui::Id {
    egui::Id::new("find-query")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindEvent {
    None,
    QueryChanged,
    Next,
    Previous,
    Replace,
    ReplaceAll,
    Close,
}

fn toggle(ui: &mut egui::Ui, on: &mut bool, label: &str, tip: &str) -> bool {
    let response = ui.selectable_label(*on, label).on_hover_text(tip);
    if response.clicked() {
        *on = !*on;
    }
    response.clicked()
}

/// Zeichnet das Overlay mit der rechten oberen Ecke bei `top_right`.
pub fn show(ctx: &egui::Context, state: &mut FindState, top_right: egui::Pos2) -> FindEvent {
    let mut event = FindEvent::None;
    egui::Area::new(egui::Id::new("find-bar"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::RIGHT_TOP)
        .fixed_pos(top_right)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(BAR_BG)
                .stroke(Stroke::new(1.0, BAR_BORDER))
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_max_width(MAX_WIDTH);
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                    ui.horizontal(|ui| {
                        let arrow = if state.show_replace { "v" } else { ">" };
                        if ui
                            .small_button(arrow)
                            .on_hover_text(tr("Ersetzen ein-/ausblenden (Strg+H)"))
                            .clicked()
                        {
                            state.show_replace = !state.show_replace;
                        }
                        let query = egui::TextEdit::singleline(&mut state.query)
                            .id(query_id())
                            .hint_text(tr("Suchen..."))
                            .desired_width(112.0)
                            .show(ui);
                        if state.focus_query {
                            state.focus_query = false;
                            query.response.request_focus();
                            let mut text_state = query.state.clone();
                            text_state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(0),
                                    egui::text::CCursor::new(state.query.chars().count()),
                                )));
                            text_state.store(ctx, query_id());
                        }
                        let frame_color = if state.error.is_some() {
                            RED_ERROR
                        } else {
                            BAR_BORDER
                        };
                        ui.painter().rect_stroke(
                            query.response.rect,
                            CornerRadius::ZERO,
                            Stroke::new(1.0, frame_color),
                            StrokeKind::Outside,
                        );
                        let mut options_changed = false;
                        if query.response.changed() {
                            event = FindEvent::QueryChanged;
                        }
                        let counter = state.counter();
                        let color = if state.error.is_some() {
                            RED_ERROR
                        } else {
                            Color32::GRAY
                        };
                        let label = ui.add_sized(
                            [84.0, 18.0],
                            egui::Label::new(egui::RichText::new(&counter).color(color).small())
                                .truncate(),
                        );
                        if state.error.is_some() {
                            label.on_hover_text(tr("Ungültige RegEx"));
                        }
                        options_changed |= toggle(
                            ui,
                            &mut state.options.case_sensitive,
                            "Aa",
                            "Groß-/Kleinschreibung",
                        );
                        options_changed |=
                            toggle(ui, &mut state.options.whole_word, "W", tr("Ganzes Wort"));
                        options_changed |=
                            toggle(ui, &mut state.options.regex, ".*", tr("Reguläre Ausdrücke"));
                        if options_changed {
                            event = FindEvent::QueryChanged;
                        }
                        if ui
                            .small_button("▲")
                            .on_hover_text(tr("Vorheriges (Umschalt+Enter)"))
                            .clicked()
                        {
                            event = FindEvent::Previous;
                        }
                        if ui
                            .small_button("▼")
                            .on_hover_text(tr("Nächstes (Enter)"))
                            .clicked()
                        {
                            event = FindEvent::Next;
                        }
                        if ui
                            .small_button("×")
                            .on_hover_text(tr("Schließen (Esc)"))
                            .clicked()
                        {
                            event = FindEvent::Close;
                        }
                        if query.response.has_focus() || query.response.lost_focus() {
                            let (enter, shift, down, up) = ctx.input_mut(|input| {
                                let shift = input.modifiers.shift;
                                (
                                    input.consume_key(Modifiers::NONE, Key::Enter)
                                        || input.consume_key(Modifiers::SHIFT, Key::Enter),
                                    shift,
                                    input.consume_key(Modifiers::NONE, Key::ArrowDown),
                                    input.consume_key(Modifiers::NONE, Key::ArrowUp),
                                )
                            });
                            if enter || down || up {
                                event = if up || (enter && shift) {
                                    FindEvent::Previous
                                } else {
                                    FindEvent::Next
                                };
                                query.response.request_focus();
                            }
                        }
                    });
                    if state.show_replace {
                        ui.horizontal(|ui| {
                            ui.add_space(24.0);
                            let replace = egui::TextEdit::singleline(&mut state.replacement)
                                .hint_text(tr("Ersetzen durch..."))
                                .desired_width(112.0)
                                .show(ui);
                            if replace.response.lost_focus()
                                && ui.input(|input| input.key_pressed(Key::Enter))
                            {
                                event = FindEvent::Replace;
                                replace.response.request_focus();
                            }
                            if ui.button(tr("Ersetzen")).clicked() {
                                event = FindEvent::Replace;
                            }
                            if ui
                                .button(tr("Alle"))
                                .on_hover_text(tr("Alle ersetzen"))
                                .clicked()
                            {
                                event = FindEvent::ReplaceAll;
                            }
                        });
                    }
                });
        });
    if ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
        event = FindEvent::Close;
    }
    event
}
