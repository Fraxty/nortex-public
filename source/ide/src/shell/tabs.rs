//! Tab-, Split- und Panelverwaltung des Editorbereichs.

use crate::*;
use nortex_ide::i18n::tr_fmt;

impl NortexCodeApp {
    /// A hidden-by-width sidecar is first revealed (taking the explorer's space); otherwise toggled.
    pub(crate) fn toggle_sidecar(&mut self) {
        if self.sidecar_visible && self.sidecar_auto_collapsed {
            self.sidecar_forced = true;
        } else {
            self.sidecar_visible = !self.sidecar_visible;
            self.sidecar_forced = self.sidecar_visible;
        }
    }

    pub(crate) fn after_tab_switch(&mut self) {
        self.completion = None;
        self.pending_restore = self
            .grid
            .active_doc()
            .map(|doc| self.grid.view(self.grid.active_pane_index(), doc));
    }

    /// Fokuswechsel zwischen Panes: aktives Dokument, Cursor und Scroll werden pro Pane wiederhergestellt.
    pub(crate) fn focus_pane(&mut self, index: usize) {
        if index == self.grid.active_pane_index() || index >= self.grid.len() {
            return;
        }
        self.sync_active_buffer();
        self.grid.focus(index);
        if let Some(doc) = self.grid.active_doc() {
            self.activate_document(doc);
        }
        self.after_tab_switch();
    }

    pub(crate) fn activate_tab(&mut self, pane: usize, doc: usize) {
        if pane == self.grid.active_pane_index() && self.grid.active_doc() == Some(doc) {
            return;
        }
        self.sync_active_buffer();
        self.grid.open_in(pane, doc);
        self.activate_document(doc);
        self.after_tab_switch();
    }

    /// Schließt den Tab nur in diesem Pane, solange ein anderes Pane das Dokument noch zeigt.
    pub(crate) fn close_tab(&mut self, pane: usize, doc: usize) {
        if self.grid.holders(doc) > 1 {
            self.sync_active_buffer();
            self.grid.remove_tab(pane, doc);
            if let Some(active) = self.grid.active_doc() {
                self.activate_document(active);
            }
            self.after_tab_switch();
        } else {
            self.close_document(pane, doc, false);
        }
    }

    pub(crate) fn drop_tab(&mut self, doc: usize, from: usize, target: usize, zone: DropZone) {
        self.sync_active_buffer();
        if self.grid.drop_tab(doc, from, target, zone) {
            if let Some(active) = self.grid.active_doc() {
                self.activate_document(active);
            }
            self.after_tab_switch();
            self.status = tr_fmt(
                "Tab verschoben ({})",
                &[self.grid.mode().label().to_string()],
            );
        }
    }

    pub(crate) fn apply_tab_events(&mut self, pane: usize, events: TabEvents) {
        if events.new_doc {
            self.focus_pane(pane);
            self.new_document();
        }
        if let Some((doc, index)) = events.reorder {
            self.grid.reorder_tab(pane, doc, index);
        }
        if let Some(doc) = events.close {
            self.close_tab(pane, doc);
        } else if let Some(doc) = events.activate {
            self.activate_tab(pane, doc);
        }
    }

    pub(crate) fn set_split(&mut self, mode: SplitMode) {
        self.sync_active_buffer();
        self.grid.set_mode(mode);
        if let Some(doc) = self.grid.active_doc() {
            self.activate_document(doc);
        }
        self.after_tab_switch();
        self.workspace.view_mode = ViewMode::Editor;
        self.status = tr_fmt("Editor-Layout: {}", &[mode.label().to_string()]);
    }

    pub(crate) fn close_active_split_pane(&mut self) {
        if self.grid.len() <= 1 {
            self.status = tr("Kein Split-Pane zu schließen.").into();
            return;
        }
        self.sync_active_buffer();
        self.grid.close_active_pane();
        if let Some(doc) = self.grid.active_doc() {
            self.activate_document(doc);
        }
        self.after_tab_switch();
        self.status = tr_fmt("Editor-Layout: {}", &[self.grid.mode().label().to_string()]);
    }

    pub(crate) fn show_side_view(&mut self, view: SideView) {
        self.workspace.side_view = view;
        self.workspace.side_open = true;
        self.workspace.view_mode = ViewMode::Editor;
        if view == SideView::Git {
            self.request_git_refresh();
        }
    }

    pub(crate) fn draw_pane_tabs(
        &mut self,
        ui: &mut egui::Ui,
        pane: usize,
        has_errors: bool,
    ) -> TabEvents {
        let mut events = TabEvents::default();
        if self.zen_mode {
            return events;
        }
        let theme = self.theme;
        let focused = self.grid.active_pane_index() == pane;
        let active_doc = self.grid.panes()[pane].active;
        let tabs: Vec<(usize, String, bool)> = self.grid.panes()[pane]
            .tabs
            .iter()
            .filter_map(|&doc| {
                let document = self.documents.documents().get(doc)?;
                let name = document
                    .path()
                    .and_then(Path::file_name)
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| tr("Unbenannt").into());
                Some((doc, name, document.is_modified()))
            })
            .collect();
        let x_range = ui.max_rect().x_range();
        let mut tab_rects: Vec<(usize, egui::Rect)> = Vec::new();
        let bar = ui.horizontal(|ui| {
            for (doc, name, modified) in tabs {
                let active = focused && active_doc == Some(doc);
                let start = ui.cursor().min;
                let tab = file_tab(
                    ui,
                    &theme,
                    FileTabState {
                        name: &name,
                        active,
                        modified,
                        has_error: active && has_errors,
                    },
                    egui::Id::new(("nx-tab", pane, doc)),
                    TabDrag { pane, doc },
                );
                let end = ui.min_rect().max.x;
                tab_rects.push((
                    doc,
                    egui::Rect::from_min_max(start, egui::pos2(end, ui.min_rect().max.y)),
                ));
                if tab.closed {
                    events.close = Some(doc);
                } else if tab.activated {
                    events.activate = Some(doc);
                }
            }
            if ui
                .add(IndustrialButton::new(&theme, "+").kind(ButtonKind::Ghost))
                .clicked()
            {
                events.new_doc = true;
            }
        });
        let bar_rect = bar.response.rect;
        if let (Some(payload), Some(pointer)) = (
            egui::DragAndDrop::payload::<TabDrag>(ui.ctx()),
            ui.input(|input| input.pointer.hover_pos()),
        ) && payload.pane == pane
            && bar_rect.expand2(egui::vec2(0.0, 6.0)).contains(pointer)
        {
            let index = tab_rects
                .iter()
                .position(|(_, rect)| pointer.x < rect.center().x)
                .unwrap_or(tab_rects.len());
            let line_x = match tab_rects.get(index) {
                Some((_, rect)) => rect.left(),
                None => tab_rects.last().map_or(bar_rect.left(), |(_, r)| r.right()),
            };
            ui.painter()
                .vline(line_x, bar_rect.y_range(), egui::Stroke::new(2.0, AMBER));
            if ui.input(|input| input.pointer.any_released()) {
                events.reorder = Some((payload.doc, index));
            }
        }
        let stroke = if focused {
            egui::Stroke::new(2.0, AMBER)
        } else {
            egui::Stroke::new(1.0, SPLIT_LINE)
        };
        ui.painter()
            .hline(x_range, bar.response.rect.bottom() + 1.0, stroke);
        ui.add_space(4.0);
        events
    }

    /// Nicht fokussierte Panes zeigen das geteilte Dokument schreibgeschützt; ein Klick fokussiert.
    pub(crate) fn draw_pane_preview(&mut self, ui: &mut egui::Ui, pane: usize) {
        let Some(doc) = self.grid.panes()[pane].active else {
            ui.weak(tr("Keine Datei: Tab hierher ziehen oder Datei öffnen."));
            return;
        };
        let Some(document) = self.documents.documents().get(doc) else {
            return;
        };
        let text = document.buffer().materialize();
        let spans = highlight_nortex(&text);
        let palette = self.theme.editor_palette();
        let font = self.theme.editor_font();
        let job = syntax_layout(
            &text,
            &spans,
            &palette,
            &font,
            self.theme.editor_line_height(),
        );
        egui::Frame::new()
            .fill(palette.background)
            .inner_margin(egui::Margin::symmetric(10, 8))
            .show(ui, |ui| {
                egui::ScrollArea::both()
                    .id_salt(("pane-preview", pane, doc))
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(job)
                                .selectable(false)
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                    });
            });
    }

    /// 1px-Trennlinien zwischen den Panes plus ziehbare Splitter-Griffe.
    pub(crate) fn draw_split_chrome(
        &mut self,
        ui: &mut egui::Ui,
        area: egui::Rect,
        rects: &[egui::Rect],
    ) {
        if rects.len() < 2 {
            return;
        }
        let line = egui::Stroke::new(1.0, SPLIT_LINE);
        for rect in rects {
            if rect.left() > area.left() + 0.5 {
                ui.painter().vline(rect.left(), rect.y_range(), line);
            }
            if rect.top() > area.top() + 0.5 {
                ui.painter().hline(rect.x_range(), rect.top(), line);
            }
        }
        let side_by_side =
            rects[1].left() > area.left() + 0.5 && (rects[1].top() - area.top()).abs() < 0.5;
        let (col, row) = self.grid.ratios();
        let (mut new_col, mut new_row) = (col, row);
        if side_by_side {
            let x = rects[1].left();
            let span = if rects.len() == 4 {
                area.y_range()
            } else {
                rects[0].y_range()
            };
            let handle = egui::Rect::from_x_y_ranges((x - 3.0)..=(x + 3.0), span);
            let response = ui
                .interact(
                    handle,
                    egui::Id::new("split-col-handle"),
                    egui::Sense::drag(),
                )
                .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
            if let (true, Some(pointer)) = (response.dragged(), response.interact_pointer_pos()) {
                new_col = (pointer.x - area.left()) / area.width().max(1.0);
                ui.painter().vline(x, span, egui::Stroke::new(1.0, AMBER));
            }
        }
        let rows_resizable = self.grid.mode() == SplitMode::TwoRows
            || (self.grid.mode() == SplitMode::FourGrid && side_by_side);
        if rows_resizable && let Some(last) = rects.last() {
            let y = last.top();
            let handle = egui::Rect::from_x_y_ranges(area.x_range(), (y - 3.0)..=(y + 3.0));
            let response = ui
                .interact(
                    handle,
                    egui::Id::new("split-row-handle"),
                    egui::Sense::drag(),
                )
                .on_hover_cursor(egui::CursorIcon::ResizeVertical);
            if let (true, Some(pointer)) = (response.dragged(), response.interact_pointer_pos()) {
                new_row = (pointer.y - area.top()) / area.height().max(1.0);
                ui.painter()
                    .hline(area.x_range(), y, egui::Stroke::new(1.0, AMBER));
            }
        }
        self.grid.set_ratios(new_col, new_row);
    }
}
