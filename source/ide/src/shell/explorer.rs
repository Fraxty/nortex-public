//! Datei-Explorer: Baumansicht mit Chevrons, Ordner-Symbolen und Einrückungslinien.

use crate::*;
use nortex_ide::ui::theme::colors::Intensity;
use std::collections::HashSet;

type Entries = Vec<(PathBuf, bool)>;
type Row = (PathBuf, bool, usize);

const ROW_HEIGHT: f32 = 22.0;
const INDENT: f32 = 14.0;
const CACHE_SECONDS: f64 = 3.0;
const MAX_DEPTH: usize = 24;

fn lower_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn read_entries(ui: &egui::Ui, dir: &Path, now: f64, refresh: bool) -> Entries {
    let key = egui::Id::new(("explorer-dir", dir));
    let cached: Option<(f64, Entries)> = ui.data(|data| data.get_temp(key));
    if let Some((time, entries)) = cached
        && !refresh
        && now - time < CACHE_SECONDS
    {
        return entries;
    }
    let mut entries: Entries = fs::read_dir(dir)
        .map(|read| {
            read.filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_str() != Some(".git"))
                .map(|entry| {
                    let is_dir = entry.path().metadata().is_ok_and(|meta| meta.is_dir());
                    (entry.path(), is_dir)
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| lower_name(&a.0).cmp(&lower_name(&b.0)))
    });
    ui.data_mut(|data| data.insert_temp(key, (now, entries.clone())));
    entries
}

fn collect_rows(
    ui: &egui::Ui,
    dir: &Path,
    depth: usize,
    expanded: &HashSet<PathBuf>,
    now: f64,
    refresh: bool,
    out: &mut Vec<Row>,
) {
    if depth > MAX_DEPTH {
        return;
    }
    for (path, is_dir) in read_entries(ui, dir, now, refresh) {
        out.push((path.clone(), is_dir, depth));
        if is_dir && expanded.contains(&path) {
            collect_rows(ui, &path, depth + 1, expanded, now, refresh, out);
        }
    }
}

fn paint_chevron(painter: &egui::Painter, center: egui::Pos2, open: bool, color: egui::Color32) {
    let stroke = egui::Stroke::new(1.5, color);
    let v = egui::vec2;
    let (a, b, c) = if open {
        (
            center + v(-3.5, -1.5),
            center + v(0.0, 2.5),
            center + v(3.5, -1.5),
        )
    } else {
        (
            center + v(-1.5, -3.5),
            center + v(2.5, 0.0),
            center + v(-1.5, 3.5),
        )
    };
    painter.line_segment([a, b], stroke);
    painter.line_segment([b, c], stroke);
}

fn paint_folder(painter: &egui::Painter, left: f32, center_y: f32, color: egui::Color32) {
    let body = egui::Rect::from_min_size(egui::pos2(left, center_y - 5.0), egui::vec2(15.0, 11.0));
    let tab = egui::Rect::from_min_size(body.min + egui::vec2(0.0, -2.0), egui::vec2(6.0, 3.0));
    painter.rect_filled(body, egui::CornerRadius::same(1), color);
    painter.rect_filled(tab, egui::CornerRadius::same(1), color);
}

impl NortexCodeApp {
    /// Zeichnet den Projektbaum; gibt die angeklickte Datei zurück.
    pub(crate) fn draw_explorer_tree(&mut self, ui: &mut egui::Ui) -> Option<PathBuf> {
        let palette = self.theme.palette();
        let expanded_key = egui::Id::new("explorer-expanded");
        let mut expanded: HashSet<PathBuf> = ui
            .data(|data| data.get_temp(expanded_key))
            .unwrap_or_default();
        let root = self.project_root.clone();
        let title = root.file_name().map_or_else(
            || "PROJEKT".into(),
            |name| name.to_string_lossy().to_uppercase(),
        );
        let mut refresh = false;
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(title)
                    .strong()
                    .color(palette.text_primary),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("⟳")
                    .on_hover_text(tr("Aktualisieren"))
                    .clicked()
                {
                    refresh = true;
                }
                if ui
                    .small_button("−")
                    .on_hover_text(tr("Alle Ordner einklappen"))
                    .clicked()
                {
                    expanded.clear();
                }
            });
        });
        ui.add_space(2.0);

        let now = ui.input(|input| input.time);
        let mut rows: Vec<Row> = Vec::new();
        collect_rows(ui, &root, 0, &expanded, now, refresh, &mut rows);
        let active = self
            .documents
            .active_document()
            .and_then(|document| document.path())
            .map(Path::to_path_buf);

        let mut opened = None;
        egui::ScrollArea::both()
            .id_salt("explorer-scroll")
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
                for (path, is_dir, depth) in &rows[range] {
                    let width = ui.available_width().max(180.0);
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(width, ROW_HEIGHT), egui::Sense::click());
                    let selected = !*is_dir && active.as_deref() == Some(path.as_path());
                    if selected {
                        ui.painter().rect_filled(
                            rect,
                            egui::CornerRadius::ZERO,
                            palette.accent(palette.blue, Intensity::Muted),
                        );
                    } else if response.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            egui::CornerRadius::ZERO,
                            palette.interactive,
                        );
                    }
                    for level in 0..*depth {
                        let x = rect.left() + 9.0 + level as f32 * INDENT;
                        ui.painter().line_segment(
                            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                            egui::Stroke::new(1.0, palette.border_subtle),
                        );
                    }
                    let base = rect.left() + 4.0 + *depth as f32 * INDENT;
                    let center_y = rect.center().y;
                    let text_x = if *is_dir {
                        let open = expanded.contains(path);
                        paint_chevron(
                            ui.painter(),
                            egui::pos2(base + 5.0, center_y),
                            open,
                            palette.text_secondary,
                        );
                        paint_folder(ui.painter(), base + 14.0, center_y, palette.amber);
                        base + 34.0
                    } else {
                        if let Some(texture) = self.textures.file_icon(FileKind::of(path)) {
                            let icon = egui::Rect::from_min_size(
                                egui::pos2(base + 18.0, center_y - 8.0),
                                egui::vec2(16.0, 16.0),
                            );
                            egui::Image::new(texture)
                                .tint(nortex_ide::ui::branding::file_tint(path))
                                .paint_at(ui, icon);
                        }
                        base + 40.0
                    };
                    let color = if *is_dir || selected {
                        palette.text_primary
                    } else {
                        palette.text_secondary
                    };
                    ui.painter().text(
                        egui::pos2(text_x, center_y),
                        egui::Align2::LEFT_CENTER,
                        path.file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                        egui::FontId::proportional(13.5),
                        color,
                    );
                    if response.clicked() {
                        if *is_dir {
                            if !expanded.remove(path) {
                                expanded.insert(path.clone());
                            }
                        } else {
                            opened = Some(path.clone());
                        }
                    }
                }
            });
        ui.data_mut(|data| data.insert_temp(expanded_key, expanded));
        opened
    }
}
