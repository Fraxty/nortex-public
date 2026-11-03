//! Verteilung von Tastenkürzeln und Befehlspalette auf Aktionen.

use crate::*;

impl NortexCodeApp {
    pub(crate) fn change_theme(&mut self, context: &egui::Context, theme: Theme) {
        self.theme = theme;
        self.ui_zoom_input = zoom_input(theme.zoom.ui);
        self.editor_zoom_input = zoom_input(theme.zoom.editor);
        theme.apply(context);
        self.settings = std::mem::take(&mut self.settings).with_theme(&theme);
        self.persist_settings();
    }

    pub(crate) fn set_language(&mut self, language: nortex_ide::i18n::Language) {
        nortex_ide::i18n::set_language(language);
        self.settings.language = language;
        self.persist_settings();
    }

    pub(crate) fn run_palette_command(&mut self, context: &egui::Context, command: PaletteCommand) {
        match command {
            PaletteCommand::BuildAndRun => self.start_build_and_run(),
            PaletteCommand::ToggleTheme => {
                let kinds = ThemeKind::ALL;
                let current = kinds.iter().position(|kind| *kind == self.theme.kind);
                let mut next = self.theme;
                next.kind = kinds[(current.unwrap_or(0) + 1) % kinds.len()];
                self.change_theme(context, next);
            }
            PaletteCommand::ClearConsole => {
                self.compiler_log.clear();
                self.runtime_output.clear();
                self.last_run = None;
            }
            PaletteCommand::ToggleZenMode => self.zen_mode = !self.zen_mode,
            PaletteCommand::OpenFile => {
                let picked = rfd::FileDialog::new()
                    .set_directory(&self.project_root)
                    .add_filter(tr("NORTEX"), &["nx", "nortex"])
                    .pick_file();
                if let Some(path) = picked {
                    self.open_file(path);
                }
            }
            PaletteCommand::SaveFile => {
                self.save_document();
            }
            PaletteCommand::SplitColumns => self.set_split(SplitMode::TwoColumns),
            PaletteCommand::SplitRows => self.set_split(SplitMode::TwoRows),
            PaletteCommand::SplitGrid => self.set_split(SplitMode::FourGrid),
            PaletteCommand::SplitSingle => self.set_split(SplitMode::Single),
            PaletteCommand::ClosePane => self.close_active_split_pane(),
            PaletteCommand::ToggleMinimap => self.toggle_minimap(),
        }
    }

    pub(crate) fn run_action(&mut self, context: &egui::Context, action: Action) {
        match action {
            Action::NewFile => self.new_document(),
            Action::ToggleZen => self.zen_mode = !self.zen_mode,
            Action::InsertMain => self.insert_main_snippet(context, false),
            Action::FocusExplorer => self.show_side_view(SideView::Explorer),
            Action::FocusSearch => self.show_side_view(SideView::Search),
            Action::FocusGit => self.show_side_view(SideView::Git),
            Action::FocusDebugger => self.show_side_view(SideView::Debugger),
            Action::FocusExtensions => self.workspace.apply(ActivityAction::Extensions),
            Action::ToggleSidebar => self.workspace.side_open = !self.workspace.side_open,
            Action::TogglePanel => self.problems_visible = !self.problems_visible,
            Action::ToggleSidecar => self.toggle_sidecar(),
            Action::CommandPalette => self.palette.toggle(),
            Action::QuickOpen => self.run_palette_command(context, PaletteCommand::OpenFile),
            Action::NextTab => {
                self.sync_active_buffer();
                if let Some(doc) = self.grid.cycle_tab(1) {
                    self.activate_document(doc);
                    self.after_tab_switch();
                }
            }
            Action::Save => {
                self.save_document();
            }
            Action::SaveAs => {
                self.save_document_as();
            }
            Action::Undo => self.edit_history(false),
            Action::Redo => self.edit_history(true),
            Action::FindInFile | Action::Replace => {
                if self.workspace.view_mode == ViewMode::Editor && self.grid.active_doc().is_some()
                {
                    let selection = self.editor_selection(context);
                    if action == Action::Replace {
                        self.find.open_replace(selection.as_deref());
                    } else {
                        self.find.open_find(selection.as_deref());
                    }
                    self.find.anchor = self
                        .editor_selection_range(context)
                        .map_or(self.live_cursor, |(start, _)| start);
                    self.find.refresh(&self.editor_text);
                    self.find.select_from(self.find.anchor);
                }
            }
            Action::ToggleComment
            | Action::MoveLineUp
            | Action::MoveLineDown
            | Action::DuplicateLine
            | Action::SelectNextOccurrence => self.line_action(context, action),
            // Der Compiler-Job baut und startet in einem Schritt.
            Action::Build | Action::BuildAndRun => self.start_build_and_run(),
            Action::StartDebugging => self.start_debugging(),
            Action::ToggleBreakpoint => self.toggle_breakpoint_at_cursor(context),
            Action::StopExecution => {
                self.status = tr("Abbrechen laufender Programme ist nicht implementiert.").into();
            }
            Action::ShowProblems => {
                self.problems_visible = true;
                self.console_tab = ConsoleTab::CompilerLog;
            }
            Action::ShowOutput => {
                self.problems_visible = true;
                self.console_tab = ConsoleTab::RuntimeOutput;
            }
            Action::Split(mode) => self.set_split(mode),
            Action::RenameSymbol => {
                if self.workspace.view_mode == ViewMode::Editor && self.grid.active_doc().is_some()
                {
                    self.rename_requested = true;
                }
            }
            Action::ToggleMinimap => self.toggle_minimap(),
            Action::ClosePane => self.close_active_split_pane(),
            Action::FocusPane(index) => self.focus_pane(index),
            Action::FocusDirection(direction) => {
                let mut probe = self.grid.clone();
                if probe.focus_direction(direction, self.editor_width) {
                    self.focus_pane(probe.active_pane_index());
                }
            }
        }
    }
}
