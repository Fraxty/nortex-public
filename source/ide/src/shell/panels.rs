//! Seitenpanels, Konsole, Dashboard und Einstellungsansicht.

use crate::*;
use nortex_ide::i18n::tr_fmt;

impl NortexCodeApp {
    pub(crate) fn draw_terminal(
        &mut self,
        ui: &mut egui::Ui,
        context: &egui::Context,
        focus: bool,
    ) {
        if self.terminal.is_none() && self.terminal_error.is_none() {
            match Terminal::spawn_default() {
                Ok(terminal) => self.terminal = Some(terminal),
                Err(error) => self.terminal_error = Some(error.to_string()),
            }
        }
        let palette = self.theme.palette();
        let font = self.theme.terminal_font();
        let Some(terminal) = self.terminal.as_mut() else {
            let message = self.terminal_error.clone().unwrap_or_default();
            ui.colored_label(
                palette.red,
                tr_fmt("Shell nicht startbar: {}", &[message.to_string()]),
            );
            return;
        };
        if terminal.poll() || terminal.is_alive() {
            context.request_repaint_after(Duration::from_millis(100));
        }
        let alive = terminal.is_alive();
        let clear_shortcut =
            ui.input_mut(|input| input.consume_key(egui::Modifiers::CTRL, egui::Key::L));
        ui.horizontal(|ui| {
            let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            let color = if alive {
                egui::Color32::from_rgb(0x70, 0xA8, 0x89)
            } else {
                egui::Color32::from_rgb(0xD6, 0x6C, 0x6C)
            };
            ui.painter().circle_filled(dot.center(), 4.0, color);
            ui.label(format!(
                "{} ({})",
                terminal.shell,
                if alive { "aktiv" } else { "beendet" }
            ));
            if ui.button(tr("Clear")).clicked() || clear_shortcut {
                terminal.clear();
            }
        });
        let input_height = 28.0;
        let area = (ui.available_height() - input_height).max(40.0);
        egui::Frame::new()
            .fill(egui::Color32::from_rgb(0x14, 0x17, 0x1B))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("terminal-output")
                    .max_height(area)
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(&terminal.buffer)
                                .font(font)
                                .color(egui::Color32::from_rgb(0xE6, 0xE9, 0xED)),
                        );
                    });
            });
        let response = ui.add_enabled(
            alive,
            egui::TextEdit::singleline(&mut self.terminal_input)
                .id(egui::Id::new("terminal-input"))
                .desired_width(f32::INFINITY)
                .font(self.theme.terminal_font())
                .hint_text(tr("Befehl eingeben, Enter sendet")),
        );
        if focus && alive {
            response.request_focus();
        }
        if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
            let line = std::mem::take(&mut self.terminal_input);
            let _ = terminal.send_line(&line);
            response.request_focus();
        }
    }

    pub(crate) fn draw_dashboard(&mut self, ui: &mut egui::Ui) {
        let data = DashboardData {
            project_root: &self.project_root,
            source_count: self.source_files.len(),
            open_documents: self.documents.documents().len(),
            recent_files: &self.settings.recent_files,
            toolchain: &self.toolchain,
            last_run: self.last_run,
            run_in_progress: self.run_gate.is_running(),
            overflow_checks: self.settings.overflow_checks,
            git: &self.git_status,
        };
        match dashboard::show(ui, &self.theme, &data) {
            Some(DashboardAction::OpenRecent(path)) => self.open_file(path),
            Some(DashboardAction::NewFile) => {
                self.new_document();
                self.workspace.view_mode = ViewMode::Editor;
            }
            Some(DashboardAction::OpenFile) => {
                self.run_palette_command(ui.ctx(), PaletteCommand::OpenFile);
            }
            Some(DashboardAction::BuildAndRun) => {
                self.workspace.view_mode = ViewMode::Editor;
                self.start_build_and_run();
            }
            None => {}
        }
    }

    pub(crate) fn draw_search_panel(&mut self, ui: &mut egui::Ui) {
        let status = self.status_color();
        panel_header(ui, &self.theme, 1, tr("Suche"), status);
        let response = ui.text_edit_singleline(&mut self.search_query);
        let submit = response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if (ui.button(tr("Suchen")).clicked() || submit) && !self.search_query.trim().is_empty() {
            self.search_hits = search::search(&self.source_files, &self.search_query);
        }
        ui.separator();
        if self.search_hits.is_empty() {
            ui.label(tr("Keine Treffer."));
            return;
        }
        let hits = self.search_hits.clone();
        egui::ScrollArea::vertical()
            .id_salt("search-hits")
            .show(ui, |ui| {
                for hit in hits {
                    let name = search::display_name(&hit.path, &self.project_root);
                    if ui
                        .selectable_label(false, format!("{name}:{}  {}", hit.line, hit.text))
                        .clicked()
                    {
                        self.open_file(hit.path);
                    }
                }
            });
    }

    pub(crate) fn draw_git_panel(&mut self, ui: &mut egui::Ui) {
        panel_header(ui, &self.theme, 1, "Git", self.theme.palette().blue);
        let refresh_label = if self.git_refresh_running {
            tr("Git wird aktualisiert …")
        } else {
            tr("Aktualisieren")
        };
        if ui
            .add_enabled(!self.git_refresh_running, egui::Button::new(refresh_label))
            .clicked()
        {
            self.request_git_refresh();
        }
        ui.separator();
        match self.git_status.clone() {
            GitStatus::Unavailable => {
                if self.git_refresh_running {
                    ui.label(tr("Git-Status wird im Hintergrund geladen …"));
                } else {
                    ui.label(tr("git ist nicht installiert oder nicht im PATH."));
                }
            }
            GitStatus::NotARepository => {
                ui.label(tr("Der Projektordner ist kein Git-Repository."));
            }
            GitStatus::Repository { branch, changes } => {
                ui.label(tr_fmt("Branch: {}", &[branch.to_string()]));
                ui.label(tr_fmt("{} Änderungen", &[changes.len().to_string()]));
                ui.separator();
                type GroupFilter = fn(&str) -> bool;
                let groups: [(&str, GroupFilter); 4] = [
                    ("Hinzugefügt", |code| code.starts_with('A')),
                    ("Geändert", |code| code.contains('M') || code.contains('R')),
                    ("Gelöscht", |code| code.contains('D')),
                    ("Nicht verfolgt", |code| code == "??"),
                ];
                egui::ScrollArea::vertical()
                    .id_salt("git-changes")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (title, matches_group) in groups {
                            let items: Vec<&nortex_ide::git::Change> = changes
                                .iter()
                                .filter(|change| matches_group(&change.code))
                                .collect();
                            if items.is_empty() {
                                continue;
                            }
                            egui::CollapsingHeader::new(format!("{title} ({})", items.len()))
                                .id_salt(("git-group", title))
                                .default_open(items.len() <= 20)
                                .show(ui, |ui| {
                                    for change in items.iter().take(200) {
                                        let path = self.project_root.join(&change.path);
                                        let is_source = path.is_file()
                                            && matches!(
                                                path.extension().and_then(|e| e.to_str()),
                                                Some("nx" | "nortex")
                                            );
                                        ui.horizontal(|ui| {
                                            if is_source {
                                                if ui.link(&change.path).clicked() {
                                                    self.open_file(path);
                                                }
                                            } else {
                                                ui.weak(&change.path);
                                            }
                                            self.draw_git_file_actions(
                                                ui,
                                                &change.path,
                                                &change.code,
                                            );
                                        });
                                    }
                                    if items.len() > 200 {
                                        ui.weak(tr_fmt(
                                            "… {} weitere nicht angezeigt",
                                            &[(items.len() - 200).to_string()],
                                        ));
                                    }
                                });
                        }
                        self.draw_git_diff(ui);
                    });
            }
        }
    }

    /// Wendet die erste Schnellkorrektur zur Diagnose auf das aktive Dokument an (rückgängig machbar).
    pub(crate) fn apply_quick_fix(&mut self, diagnostic: &IdeDiagnostic) {
        let fixes = nortex_ide::diagnostics::quick_fixes(&self.editor_text, diagnostic);
        let applied = fixes.first().and_then(|fix| {
            nortex_ide::diagnostics::apply_edits(&self.editor_text, &fix.edits)
                .map(|text| (fix.title.clone(), text))
        });
        match applied {
            Some((title, text)) => {
                self.set_editor_text(text);
                self.status = tr_fmt("Quick Fix angewendet: {}", &[title]);
            }
            None => self.status = tr("Kein Quick Fix verfügbar.").into(),
        }
    }

    pub(crate) fn status_color(&self) -> egui::Color32 {
        let palette = self.theme.palette();
        if self.diagnostics.iter().any(|item| item.is_error) {
            palette.red
        } else if self.run_gate.is_running() {
            palette.amber
        } else {
            palette.green
        }
    }

    pub(crate) fn console_status_color(&self) -> egui::Color32 {
        let palette = self.theme.palette();
        if self.run_gate.is_running() {
            palette.amber
        } else {
            match self.last_run {
                Some(RunOutcome::Succeeded) => palette.green,
                Some(RunOutcome::Failed) => palette.red,
                None => self.status_color(),
            }
        }
    }
}
