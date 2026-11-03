//! Zeichenhilfen für Editor-Overlays.

use crate::*;
use nortex_ide::i18n::tr_fmt;

/// Dezente Amber-Flächen für alle Treffer, kräftig für den aktuellen; scrollt bei Bedarf dorthin.
pub(crate) fn paint_find_matches(
    ui: &mut egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    matches: &[find_replace::Match],
    current: Option<usize>,
    scroll: bool,
) {
    let clip = ui.clip_rect();
    let origin = output.galley_pos;
    for (index, found) in matches.iter().enumerate() {
        let first = output
            .galley
            .pos_from_ccursor(egui::text::CCursor::new(found.char_start));
        let last = output
            .galley
            .pos_from_ccursor(egui::text::CCursor::new(found.char_end));
        let (top, bottom) = (origin.y + first.top(), origin.y + first.bottom());
        let is_current = current == Some(index);
        let x1 = if (last.top() - first.top()).abs() < 1.0 {
            origin.x + last.left()
        } else {
            origin.x + output.galley.rect.right()
        };
        let rect = egui::Rect::from_min_max(
            egui::pos2(origin.x + first.left(), top),
            egui::pos2(x1.max(origin.x + first.left() + 2.0), bottom),
        );
        if is_current && scroll {
            ui.scroll_to_rect(
                rect.expand2(egui::vec2(24.0, 24.0)),
                Some(egui::Align::Center),
            );
        }
        if bottom < clip.top() || top > clip.bottom() {
            continue;
        }
        let alpha = if is_current { 0.55 } else { 0.18 };
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(1),
            find_bar::AMBER.gamma_multiply(alpha),
        );
        if is_current {
            ui.painter().rect_stroke(
                rect,
                egui::CornerRadius::same(1),
                egui::Stroke::new(1.0, find_bar::AMBER),
                egui::StrokeKind::Outside,
            );
        }
    }
}

/// Amber-Linie an der Kante (Edge-Split) bzw. dezente Fläche im Zentrum (Tab einfügen).
pub(crate) fn paint_drop_feedback(context: &egui::Context, rect: egui::Rect, zone: DropZone) {
    let painter = context.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("tab-drop-feedback"),
    ));
    let stroke = egui::Stroke::new(2.0, AMBER);
    match zone {
        DropZone::Center => {
            let _ = painter.rect_filled(rect, egui::CornerRadius::ZERO, AMBER.gamma_multiply(0.14));
        }
        DropZone::Left => {
            let _ = painter.vline(rect.left() + 1.0, rect.y_range(), stroke);
        }
        DropZone::Right => {
            let _ = painter.vline(rect.right() - 1.0, rect.y_range(), stroke);
        }
        DropZone::Top => {
            let _ = painter.hline(rect.x_range(), rect.top() + 1.0, stroke);
        }
        DropZone::Bottom => {
            let _ = painter.hline(rect.x_range(), rect.bottom() - 1.0, stroke);
        }
    }
}

/// Wellenlinien unter Fehler-/Warn-Spans samt Hover-Tooltip mit der Meldung.
pub(crate) fn paint_squiggles(
    ui: &mut egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    text: &str,
    squiggles: &[inline::InlineDiagnostic],
) {
    let clip = ui.clip_rect();
    for (index, item) in squiggles.iter().enumerate() {
        let (Some(start), Some(end)) = (
            inline::char_index(text, item.line, item.col_start),
            inline::char_index(text, item.line, item.col_end),
        ) else {
            continue;
        };
        let first = output
            .galley
            .pos_from_ccursor(egui::text::CCursor::new(start));
        let last = output
            .galley
            .pos_from_ccursor(egui::text::CCursor::new(end));
        let origin = output.galley_pos;
        let (x0, x1) = (origin.x + first.left(), origin.x + last.left());
        let baseline = origin.y + first.bottom() - 1.0;
        if baseline < clip.top() - 4.0 || baseline > clip.bottom() + 4.0 {
            continue;
        }
        let [r, g, b] = match item.severity {
            inline::DiagnosticSeverity::Error => inline::ERROR_RGB,
            inline::DiagnosticSeverity::Warning => inline::WARNING_RGB,
        };
        let points = inline::squiggle_points(x0, x1, baseline)
            .into_iter()
            .map(|(x, y)| egui::pos2(x, y))
            .collect();
        ui.painter().add(egui::Shape::line(
            points,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(r, g, b)),
        ));
        let area = egui::Rect::from_min_max(
            egui::pos2(x0, origin.y + first.top()),
            egui::pos2(x1.max(x0 + 4.0), origin.y + first.bottom()),
        );
        ui.interact(
            area,
            egui::Id::new(("squiggle", index)),
            egui::Sense::hover(),
        )
        .on_hover_text(&item.message);
    }
}

/// Cursor-anchored completion popup with a bounded, scrollable result list.
pub(crate) fn paint_completion(
    ui: &egui::Ui,
    output: &egui::text_edit::TextEditOutput,
    popup: &Completion,
    palette: &EditorPalette,
    font: &FontId,
) {
    let caret = output
        .galley
        .pos_from_ccursor(egui::text::CCursor::new(popup.anchor));
    let screen = ui.ctx().screen_rect();
    let row_height = font.size + 8.0;
    let content_height = popup.items.len() as f32 * row_height + 12.0;
    let height = content_height.min(300.0);
    let below = output.galley_pos + egui::vec2(caret.left(), caret.bottom() + 2.0);
    let above = output.galley_pos + egui::vec2(caret.left(), caret.top() - height - 2.0);
    let mut position = if below.y + height <= screen.bottom() - 8.0 {
        below
    } else {
        above
    };
    position.x = position
        .x
        .clamp(screen.left() + 8.0, screen.right() - 328.0);
    position.y = position
        .y
        .clamp(screen.top() + 8.0, screen.bottom() - height - 8.0);
    egui::Area::new(egui::Id::new("completion-popup"))
        .order(egui::Order::Foreground)
        .fixed_pos(position)
        .constrain(true)
        .show(ui.ctx(), |ui| {
            egui::Frame::new()
                .fill(palette.background)
                .stroke(egui::Stroke::new(1.0, palette.line_number))
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.set_min_width(320.0);
                    if let Some(help) = &popup.signature_help {
                        ui.label(
                            egui::RichText::new(&help.signature)
                                .small()
                                .color(palette.keyword),
                        );
                        if let Some((index, parameter)) = &help.active_parameter {
                            ui.label(
                                egui::RichText::new(tr_fmt(
                                    "Parameter {}: {}",
                                    &[index.to_string(), parameter.clone()],
                                ))
                                .small()
                                .color(palette.text),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new(tr("Kein passender Parameter"))
                                    .small()
                                    .color(palette.line_number),
                            );
                        }
                        ui.separator();
                    }
                    egui::ScrollArea::vertical()
                        .max_height(280.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for (index, item) in popup.items.iter().enumerate() {
                                let selected = index == popup.selected;
                                let detail = popup.details.get(index);
                                let fill = if selected {
                                    palette.keyword.gamma_multiply(0.9)
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                egui::Frame::new().fill(fill).show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(tr(
                                                detail.map_or("Symbol", |value| value.kind.label())
                                            ))
                                            .small()
                                            .color(palette.line_number),
                                        );
                                        ui.label(
                                            egui::RichText::new(item)
                                                .font(font.clone())
                                                .color(palette.text),
                                        );
                                        if let Some(detail) = detail
                                            && !detail.detail.is_empty()
                                        {
                                            ui.label(
                                                egui::RichText::new(completion_detail(
                                                    &detail.detail,
                                                ))
                                                .small()
                                                .color(palette.line_number),
                                            );
                                        }
                                    });
                                });
                            }
                        });
                });
        });
}

fn completion_detail(detail: &str) -> String {
    for (prefix, template) in [
        ("Feld: ", "Feld: {}"),
        ("Parameter: ", "Parameter: {}"),
        ("Variable: ", "Variable: {}"),
        ("-Variante", "{}-Variante"),
    ] {
        if let Some(value) = detail.strip_prefix(prefix) {
            return tr_fmt(template, &[value.to_owned()]);
        }
        if prefix == "-Variante"
            && let Some(value) = detail.strip_suffix(prefix)
        {
            return tr_fmt(template, &[value.to_owned()]);
        }
    }
    match detail {
        "NORTEX-Schlüsselwort" => tr("NORTEX-Schlüsselwort").to_owned(),
        "Projektquelldatei" => tr("Projektquelldatei").to_owned(),
        "Bezeichner in Datei" => tr("Bezeichner in Datei").to_owned(),
        "Importiertes Modul" => tr("Importiertes Modul").to_owned(),
        "Pattern-Bindung" => tr("Pattern-Bindung").to_owned(),
        "Struktur" => tr("Struktur").to_owned(),
        "Record" => tr("Record").to_owned(),
        "Schnittstelle" => tr("Schnittstelle").to_owned(),
        "Trait" => tr("Trait").to_owned(),
        "Aufzählung" => tr("Aufzählung").to_owned(),
        _ => detail.to_owned(),
    }
}

pub(crate) fn syntax_layout(
    source: &str,
    spans: &[HighlightSpan],
    palette: &EditorPalette,
    font: &FontId,
    line_height: f32,
) -> LayoutJob {
    let format = |color| {
        let mut format = TextFormat::simple(font.clone(), color);
        format.line_height = Some(line_height);
        format
    };
    let classes = nortex_ide::editor::highlighting::classify(source, spans);
    let mut job = LayoutJob::default();
    let mut offset = 0;
    for (span, class) in spans.iter().zip(classes) {
        if span.start < offset || span.end > source.len() {
            continue;
        }
        if offset < span.start {
            job.append(&source[offset..span.start], 0.0, format(palette.text));
        }
        job.append(
            &source[span.start..span.end],
            0.0,
            format(palette.color_for_class(class)),
        );
        offset = span.end;
    }
    if offset < source.len() {
        job.append(&source[offset..], 0.0, format(palette.text));
    }
    // No soft wrapping: visual rows must match the line-number gutter.
    job.wrap.max_width = f32::INFINITY;
    job
}
