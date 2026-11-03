//! Editoraktionen: Auswahl, Zeilenoperationen, Suchen/Ersetzen und Umbenennen.

use crate::*;
use nortex_ide::i18n::tr_fmt;

impl NortexCodeApp {
    pub(crate) fn toggle_minimap(&mut self) {
        self.settings.minimap_enabled = !self.settings.minimap_enabled;
        self.persist_settings();
        self.status = if self.settings.minimap_enabled {
            tr("Minimap eingeblendet").into()
        } else {
            tr("Minimap ausgeblendet").into()
        };
    }

    pub(crate) fn breadcrumb_file_label(&self) -> String {
        self.documents
            .active_document()
            .and_then(|document| document.path())
            .map(|path| {
                path.strip_prefix(&self.project_root)
                    .unwrap_or(path)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .unwrap_or_else(|| tr("Unbenannt").into())
    }

    pub(crate) fn breadcrumb_path(&self) -> Vec<scope::Scope> {
        let line = scope::line_of_char(&self.editor_text, self.live_cursor);
        scope::scope_path(&self.editor_text, line)
    }

    /// Zeichnet das F2-Eingabefeld; liefert den neuen Text, wenn Enter eine Umbenennung ausgeführt hat.
    pub(crate) fn show_rename_widget(&mut self, ctx: &egui::Context) -> Option<String> {
        let (x, y) = self.rename.cursor_pos.unwrap_or((40.0, 40.0));
        let mut result = None;
        let mut close = false;
        let id = egui::Id::new("rename-input");
        egui::Area::new(egui::Id::new("rename-area"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(x, y))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        let output = egui::TextEdit::singleline(&mut self.rename.new_name)
                            .id(id)
                            .desired_width(180.0)
                            .show(ui);
                        if self.rename.fresh {
                            self.rename.fresh = false;
                            output.response.request_focus();
                            let mut state = output.state.clone();
                            let len = self.rename.new_name.chars().count();
                            state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(0),
                                    egui::text::CCursor::new(len),
                                )));
                            state.store(ui.ctx(), id);
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            close = true;
                        } else if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            match refactor::rename_identifier(
                                &self.editor_text,
                                &self.rename.original_name,
                                self.rename.new_name.trim(),
                            ) {
                                Ok((text, count)) => {
                                    self.status =
                                        tr_fmt("{} Vorkommen umbenannt", &[count.to_string()]);
                                    result = Some(text);
                                    close = true;
                                }
                                Err(error) => {
                                    self.status = tr_fmt("Umbenennen: {}", &[error.to_string()])
                                }
                            }
                        }
                    });
            });
        if close {
            self.rename.close();
        }
        result
    }

    /// Springt in die angegebene Zeile (nullbasiert) und zentriert sie grob.
    pub(crate) fn jump_to_line(&mut self, line: usize) {
        let cursor = scope::char_index_of_line(&self.editor_text, line);
        let line_height = self.theme.editor_line_height();
        self.pending_restore = Some(ViewState {
            cursor,
            scroll: (line as f32 * line_height - 3.0 * line_height).max(0.0),
        });
    }

    pub(crate) fn edit_history(&mut self, redo: bool) {
        if let Some(document) = self.documents.active_document_mut() {
            let buffer = document.buffer_mut();
            if if redo { buffer.redo() } else { buffer.undo() } {
                self.editor_text = buffer.materialize();
                self.synced_text.clone_from(&self.editor_text);
                self.highlight_spans = highlight_nortex(&self.editor_text);
                self.schedule_analysis();
            }
        }
    }

    pub(crate) fn editor_selection_range(&self, context: &egui::Context) -> Option<(usize, usize)> {
        let range = egui::TextEdit::load_state(context, egui::Id::new("editor-text"))?
            .cursor
            .char_range()?;
        let (a, b) = (range.primary.index, range.secondary.index);
        (a != b).then_some((a.min(b), a.max(b)))
    }

    pub(crate) fn editor_selection(&self, context: &egui::Context) -> Option<String> {
        let (start, end) = self.editor_selection_range(context)?;
        Some(
            self.editor_text
                .chars()
                .skip(start)
                .take(end - start)
                .collect(),
        )
    }

    pub(crate) fn select_in_editor(&self, context: &egui::Context, start: usize, end: usize) {
        let id = egui::Id::new("editor-text");
        let mut state = egui::TextEdit::load_state(context, id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(start),
                egui::text::CCursor::new(end),
            )));
        state.store(context, id);
    }

    pub(crate) fn select_all_in_editor(&self, context: &egui::Context) {
        self.select_in_editor(context, 0, self.editor_text.chars().count());
    }

    pub(crate) fn copy_selection(&mut self, context: &egui::Context, cut: bool) {
        let Some((start, end)) = self.editor_selection_range(context) else {
            self.status = tr("Keine Auswahl vorhanden.").into();
            return;
        };
        let selected: String = self
            .editor_text
            .chars()
            .skip(start)
            .take(end - start)
            .collect();
        context.copy_text(selected);
        if cut {
            let remaining: String = self
                .editor_text
                .chars()
                .take(start)
                .chain(self.editor_text.chars().skip(end))
                .collect();
            self.editor_text = remaining;
            self.sync_active_buffer();
            self.highlight_spans = highlight_nortex(&self.editor_text);
            self.schedule_analysis();
            self.select_in_editor(context, start, start);
        }
    }

    pub(crate) fn select_current_match(&self, context: &egui::Context) {
        if let Some(found) = self.find.current_match() {
            self.select_in_editor(context, found.char_start, found.char_end);
        }
    }

    pub(crate) fn apply_find_event(&mut self, context: &egui::Context, event: FindEvent) {
        match event {
            FindEvent::None => {}
            FindEvent::QueryChanged => {
                self.find.refresh(&self.editor_text);
                self.find.select_from(self.find.anchor);
                self.select_current_match(context);
            }
            FindEvent::Next | FindEvent::Previous => {
                self.find.step(event == FindEvent::Next, self.live_cursor);
                self.select_current_match(context);
            }
            FindEvent::Replace => {
                let Some(found) = self.find.current_match() else {
                    return;
                };
                let result = find_replace::replace_match(
                    &self.editor_text,
                    &found,
                    &self.find.query,
                    &self.find.replacement,
                    self.find.options,
                );
                if let Ok((text, length)) = result {
                    self.set_editor_text(text);
                    self.find.refresh(&self.editor_text);
                    self.find.select_from(found.char_start + length);
                    self.select_current_match(context);
                }
            }
            FindEvent::ReplaceAll => {
                let result = find_replace::replace_all(
                    &self.editor_text,
                    &self.find.query,
                    &self.find.replacement,
                    self.find.options,
                );
                if let Ok((text, count)) = result
                    && count > 0
                {
                    self.set_editor_text(text);
                    self.find.refresh(&self.editor_text);
                    self.find.current = None;
                    self.status = tr_fmt("{} Vorkommen ersetzt.", &[count.to_string()]);
                }
            }
            FindEvent::Close => {
                self.find.close();
                context.memory_mut(|memory| memory.request_focus(egui::Id::new("editor-text")));
            }
        }
    }

    pub(crate) fn line_action(&mut self, context: &egui::Context, action: Action) {
        if self.workspace.view_mode != ViewMode::Editor || self.grid.active_doc().is_none() {
            return;
        }
        let id = egui::Id::new("editor-text");
        let (a, b) = egui::TextEdit::load_state(context, id)
            .and_then(|state| state.cursor.char_range())
            .map(|range| (range.secondary.index, range.primary.index))
            .unwrap_or((self.live_cursor, self.live_cursor));
        let edit =
            match action {
                Action::ToggleComment => Some(lines::toggle_comment(&self.editor_text, a, b)),
                Action::MoveLineUp => lines::move_lines(&self.editor_text, a, b, true),
                Action::MoveLineDown => lines::move_lines(&self.editor_text, a, b, false),
                Action::DuplicateLine => Some(lines::duplicate_lines(&self.editor_text, a, b)),
                Action::SelectNextOccurrence => lines::next_occurrence(&self.editor_text, a, b)
                    .map(|(start, end)| lines::LineEdit {
                        text: self.editor_text.clone(),
                        start,
                        end,
                    }),
                _ => None,
            };
        let Some(edit) = edit else { return };
        if edit.text != self.editor_text {
            self.editor_text = edit.text;
            self.sync_active_buffer();
            self.highlight_spans = highlight_nortex(&self.editor_text);
            self.schedule_analysis();
        }
        self.completion = None;
        let mut state = egui::TextEdit::load_state(context, id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::two(
                egui::text::CCursor::new(edit.start),
                egui::text::CCursor::new(edit.end),
            )));
        state.store(context, id);
    }

    const MAIN_SNIPPET: &'static str = "public int main() {\n    return 0;\n}\n";

    pub(crate) fn insert_main_snippet(&mut self, context: &egui::Context, strip_trigger: bool) {
        if self.workspace.view_mode != ViewMode::Editor || self.grid.active_doc().is_none() {
            return;
        }
        let id = egui::Id::new("editor-text");
        let chars: Vec<char> = self.editor_text.chars().collect();
        let cursor = egui::TextEdit::load_state(context, id)
            .and_then(|state| state.cursor.char_range())
            .map_or(self.live_cursor, |range| range.primary.index)
            .min(chars.len());
        let start = if strip_trigger {
            cursor.saturating_sub(4)
        } else {
            cursor
        };
        let mut text: String = chars[..start].iter().collect();
        text.push_str(Self::MAIN_SNIPPET);
        text.extend(&chars[cursor..]);
        let end = start + Self::MAIN_SNIPPET.chars().count();
        self.editor_text = text;
        self.sync_active_buffer();
        self.highlight_spans = highlight_nortex(&self.editor_text);
        self.schedule_analysis();
        self.completion = None;
        let mut state = egui::TextEdit::load_state(context, id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(egui::text::CCursorRange::one(
                egui::text::CCursor::new(end),
            )));
        state.store(context, id);
    }

    /// "main" + Tab expands to the boilerplate; skipped while the completion popup owns Tab.
    pub(crate) fn expand_main_trigger(&mut self, context: &egui::Context) {
        let id = egui::Id::new("editor-text");
        if self.completion.is_some() || !context.memory(|memory| memory.has_focus(id)) {
            return;
        }
        let cursor = egui::TextEdit::load_state(context, id)
            .and_then(|state| state.cursor.char_range())
            .map_or(self.live_cursor, |range| range.primary.index);
        let before: Vec<char> = self.editor_text.chars().take(cursor).collect();
        let n = before.len();
        let is_trigger = n >= 4
            && before[n - 4..] == ['m', 'a', 'i', 'n']
            && (n == 4 || !(before[n - 5].is_alphanumeric() || before[n - 5] == '_'));
        if is_trigger
            && context.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Tab))
        {
            self.insert_main_snippet(context, true);
        }
    }
}
