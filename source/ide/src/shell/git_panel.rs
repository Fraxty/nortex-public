//! Git-Seitenleiste: Stage/Unstage je Datei und Hunk, Diff-Ansicht, Konfliktauflösung.

use crate::*;
use nortex_ide::git::{self, ConflictRegion, Hunk, LineKind, Resolution};
use nortex_ide::i18n::tr_fmt;

const MAX_LINES_PER_HUNK: usize = 400;
const MAX_CONFLICT_FILE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Default)]
pub(crate) struct GitPanelState {
    selected: Option<String>,
    staged_view: bool,
    hunks: Vec<Hunk>,
    conflicts: Vec<ConflictRegion>,
    error: Option<String>,
}

enum Action {
    StageFile(String),
    UnstageFile(String),
    ShowDiff(String, bool),
    CloseDiff,
    Hunk(usize),
    Resolve(usize, Resolution),
}

impl NortexCodeApp {
    /// Schaltflächen hinter einem Dateinamen in der Änderungsliste.
    pub(crate) fn draw_git_file_actions(&mut self, ui: &mut egui::Ui, path: &str, code: &str) {
        let mut action = None;
        if ui.small_button(tr("Stagen")).clicked() {
            action = Some(Action::StageFile(path.to_string()));
        }
        if ui
            .add_enabled(code != "??", egui::Button::new(tr("Entstagen")).small())
            .clicked()
        {
            action = Some(Action::UnstageFile(path.to_string()));
        }
        if ui.small_button(tr("Diff")).clicked() {
            action = Some(Action::ShowDiff(path.to_string(), false));
        }
        if code != "??" && ui.small_button(tr("Diff (Index)")).clicked() {
            action = Some(Action::ShowDiff(path.to_string(), true));
        }
        if let Some(action) = action {
            self.run_git_action(action);
        }
    }

    /// Diff-Ansicht der ausgewählten Datei samt Konfliktschaltflächen.
    pub(crate) fn draw_git_diff(&mut self, ui: &mut egui::Ui) {
        let Some(path) = self.git_panel.selected.clone() else {
            return;
        };
        let palette = self.theme.palette();
        let font = self.theme.terminal_font();
        let mut action = None;
        ui.separator();
        ui.horizontal(|ui| {
            ui.strong(tr_fmt("Diff: {}", std::slice::from_ref(&path)));
            if ui.small_button(tr("Diff schließen")).clicked() {
                action = Some(Action::CloseDiff);
            }
        });
        if let Some(error) = &self.git_panel.error {
            ui.colored_label(palette.red, tr_fmt("Git-Fehler: {}", &[error.clone()]));
        }
        for (index, region) in self.git_panel.conflicts.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.colored_label(
                    palette.amber,
                    tr_fmt(
                        "Konflikt {} (Zeilen {}–{})",
                        &[
                            (index + 1).to_string(),
                            (region.start_line + 1).to_string(),
                            (region.end_line + 1).to_string(),
                        ],
                    ),
                );
                for (label, resolution) in [
                    (tr("Eigene übernehmen"), Resolution::Ours),
                    (tr("Fremde übernehmen"), Resolution::Theirs),
                    (tr("Beide übernehmen"), Resolution::Both),
                ] {
                    if ui.small_button(label).clicked() {
                        action = Some(Action::Resolve(index, resolution));
                    }
                }
            });
        }
        if self.git_panel.hunks.is_empty() && self.git_panel.conflicts.is_empty() {
            ui.weak(tr("Keine Änderungen in dieser Datei."));
        }
        let staged = self.git_panel.staged_view;
        for (index, hunk) in self.git_panel.hunks.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&hunk.header)
                        .font(font.clone())
                        .color(palette.blue),
                );
                let label = if staged {
                    tr("Hunk entstagen")
                } else {
                    tr("Hunk stagen")
                };
                if ui.small_button(label).clicked() {
                    action = Some(Action::Hunk(index));
                }
            });
            for line in hunk.lines.iter().take(MAX_LINES_PER_HUNK) {
                let (prefix, color) = match line.kind {
                    LineKind::Add => ("+", palette.green),
                    LineKind::Del => ("-", palette.red),
                    LineKind::Context => (" ", palette.text_primary),
                    LineKind::Meta => ("", palette.text_primary),
                };
                ui.label(
                    egui::RichText::new(format!("{prefix}{}", line.text))
                        .font(font.clone())
                        .color(color),
                );
            }
        }
        if let Some(action) = action {
            self.run_git_action(action);
        }
    }

    fn run_git_action(&mut self, action: Action) {
        let root = self.project_root.clone();
        let result = match action {
            Action::StageFile(path) => git::stage_file(&root, &path),
            Action::UnstageFile(path) => git::unstage_file(&root, &path),
            Action::ShowDiff(path, staged) => {
                self.git_panel.selected = Some(path);
                self.git_panel.staged_view = staged;
                Ok(())
            }
            Action::CloseDiff => {
                self.git_panel = GitPanelState::default();
                return;
            }
            Action::Hunk(index) => match (
                self.git_panel.selected.clone(),
                self.git_panel.hunks.get(index).cloned(),
            ) {
                (Some(path), Some(hunk)) if self.git_panel.staged_view => {
                    git::unstage_hunk(&root, &path, &hunk)
                }
                (Some(path), Some(hunk)) => git::stage_hunk(&root, &path, &hunk),
                _ => Ok(()),
            },
            Action::Resolve(index, resolution) => self.resolve_conflict_on_disk(index, resolution),
        };
        self.git_panel.error = result.err();
        self.reload_git_selection();
        self.request_git_refresh();
    }

    fn resolve_conflict_on_disk(&self, index: usize, resolution: Resolution) -> Result<(), String> {
        let Some(path) = &self.git_panel.selected else {
            return Ok(());
        };
        let full = self.project_root.join(path);
        let text = std::fs::read_to_string(&full).map_err(|error| error.to_string())?;
        let resolved = git::resolve_conflict(&text, index, resolution);
        std::fs::write(&full, resolved).map_err(|error| error.to_string())?;
        Ok(())
    }

    fn reload_git_selection(&mut self) {
        let Some(path) = self.git_panel.selected.clone() else {
            return;
        };
        let staged = self.git_panel.staged_view;
        match git::diff_file(&self.project_root, &path, staged) {
            Ok(hunks) => self.git_panel.hunks = hunks,
            Err(error) => {
                self.git_panel.hunks.clear();
                self.git_panel.error.get_or_insert(error);
            }
        }
        let full = self.project_root.join(&path);
        let small =
            std::fs::metadata(&full).is_ok_and(|meta| meta.len() <= MAX_CONFLICT_FILE_BYTES);
        self.git_panel.conflicts = if small {
            std::fs::read_to_string(&full)
                .map(|text| git::parse_conflicts(&text))
                .unwrap_or_default()
        } else {
            Vec::new()
        };
    }
}
