//! Debugger-Anbindung der Oberfläche: Breakpoints, Startversuch und Debugger-Panel.

use crate::*;
use nortex_ide::i18n::tr_fmt;

impl NortexCodeApp {
    pub(crate) fn toggle_breakpoint_at_cursor(&mut self, context: &egui::Context) {
        if self.workspace.view_mode != ViewMode::Editor {
            return;
        }
        let Some(path) = self
            .documents
            .active_document()
            .and_then(|document| document.path().map(Path::to_path_buf))
        else {
            self.status = tr("Breakpoints brauchen eine gespeicherte Datei.").into();
            return;
        };
        let cursor = egui::TextEdit::load_state(context, egui::Id::new("editor-text"))
            .and_then(|state| state.cursor.char_range())
            .map_or(self.live_cursor, |range| range.primary.index);
        let line = self
            .editor_text
            .chars()
            .take(cursor)
            .filter(|&c| c == '\n')
            .count()
            + 1;
        self.status = if self.debugger.toggle_breakpoint(&path, line) {
            tr_fmt("Breakpoint in Zeile {} gesetzt", &[line.to_string()])
        } else {
            tr_fmt("Breakpoint in Zeile {} entfernt", &[line.to_string()])
        };
    }

    pub(crate) fn start_debugging(&mut self) {
        self.show_side_view(SideView::Debugger);
        let Some(source) = self
            .documents
            .active_document()
            .and_then(|document| document.path().map(Path::to_path_buf))
        else {
            self.status = tr("Debugging braucht eine gespeicherte Datei.").into();
            return;
        };
        let config = LaunchConfig {
            source,
            release: !self.settings.overflow_checks,
        };
        self.status = match self.debugger.start(&config) {
            Ok(()) => tr("Debug-Sitzung gestartet").into(),
            Err(error) => tr_fmt(
                "{} Strg+F5 startet das Programm ohne Debugger.",
                &[error.to_string()],
            ),
        };
    }

    pub(crate) fn draw_debugger_panel(&mut self, ui: &mut egui::Ui) {
        panel_header(
            ui,
            &self.theme,
            1,
            tr("Debugger"),
            self.theme.palette().blue,
        );
        ui.label(tr_fmt(
            "Backend: {}",
            &[self.debugger.backend_name().to_string()],
        ));
        if !self.debugger.backend_available() {
            ui.colored_label(
                self.theme.palette().amber,
                tr("Schrittweises Debugging ist vorbereitet, aber noch nicht verfügbar: nortexc erzeugt keine Debug-Informationen."),
            );
        }
        ui.horizontal(|ui| {
            if ui.button(tr("▶ Debuggen  F5")).clicked() {
                self.start_debugging();
            }
            if ui.button(tr("■ Stopp")).clicked() {
                self.debugger.stop();
            }
        });
        ui.separator();
        let count = self.debugger.breakpoint_count();
        ui.horizontal(|ui| {
            ui.strong(tr_fmt("Breakpoints ({})", &[count.to_string()]));
            if count > 0 && ui.small_button(tr("Alle entfernen")).clicked() {
                self.debugger.clear_breakpoints();
            }
        });
        if count == 0 {
            ui.label(tr("Keine. Mit F9 im Editor setzen."));
        }
        let mut remove = None;
        let mut open = None;
        for breakpoint in self.debugger.breakpoints() {
            ui.horizontal(|ui| {
                if ui
                    .small_button("×")
                    .on_hover_text(tr("Entfernen"))
                    .clicked()
                {
                    remove = Some(breakpoint.clone());
                }
                let name = search::display_name(&breakpoint.file, &self.project_root);
                if ui
                    .link(format!("{name}:{}", breakpoint.line))
                    .on_hover_text(tr("Öffnen"))
                    .clicked()
                {
                    open = Some(breakpoint.clone());
                }
            });
        }
        if let Some(breakpoint) = remove {
            self.debugger.remove_breakpoint(&breakpoint);
        }
        if let Some(breakpoint) = open {
            self.open_file(breakpoint.file);
            self.jump_to_line(breakpoint.line.saturating_sub(1));
        }
        ui.separator();
        ui.strong(tr("Aufrufliste"));
        if self.debugger.call_stack.is_empty() {
            ui.label(tr("Keine aktive Sitzung."));
        }
        for frame in &self.debugger.call_stack {
            ui.label(format!(
                "{} ({}:{})",
                frame.function,
                frame.file.display(),
                frame.line
            ));
        }
        ui.strong(tr("Variablen"));
        if self.debugger.variables.is_empty() {
            ui.label(tr("Keine aktive Sitzung."));
        }
        for variable in &self.debugger.variables {
            ui.label(format!(
                "{}: {} = {}",
                variable.name, variable.type_name, variable.value
            ));
        }
    }
}
