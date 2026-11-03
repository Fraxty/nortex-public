//! Dashboard view: six status cards. All values come from real IDE state; nothing is simulated.

use crate::i18n::{tr, tr_fmt};
use std::path::{Path, PathBuf};

use eframe::egui;

use super::theme::Theme;
use crate::git::GitStatus;
use crate::toolchain::{RunOutcome, ToolchainStatus};

pub struct DashboardData<'a> {
    pub project_root: &'a Path,
    pub source_count: usize,
    pub open_documents: usize,
    pub recent_files: &'a [String],
    pub toolchain: &'a ToolchainStatus,
    pub last_run: Option<RunOutcome>,
    pub run_in_progress: bool,
    pub overflow_checks: bool,
    pub git: &'a GitStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DashboardAction {
    OpenRecent(PathBuf),
    NewFile,
    OpenFile,
    BuildAndRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Good,
    Bad,
    Busy,
    Neutral,
}

pub fn build_status(last_run: Option<RunOutcome>, running: bool) -> (&'static str, Tone) {
    if running {
        return ("Build läuft", Tone::Busy);
    }
    match last_run {
        Some(RunOutcome::Succeeded) => ("Letzter Build erfolgreich", Tone::Good),
        Some(RunOutcome::Failed) => ("Letzter Build fehlgeschlagen", Tone::Bad),
        None => ("Noch kein Build in dieser Sitzung", Tone::Neutral),
    }
}

/// Recent entries with a flag telling whether the file still exists.
pub fn recent_entries(recent: &[String]) -> Vec<(PathBuf, bool)> {
    recent
        .iter()
        .map(|entry| {
            let path = PathBuf::from(entry);
            let exists = path.is_file();
            (path, exists)
        })
        .collect()
}

fn card(
    ui: &mut egui::Ui,
    theme: &Theme,
    title: &str,
    width: f32,
    body: impl FnOnce(&mut egui::Ui),
) {
    let palette = theme.palette();
    egui::Frame::new()
        .fill(palette.panel)
        .stroke(egui::Stroke::new(1.0, palette.border_subtle))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_width((width - 26.0).max(120.0));
                ui.set_min_height(120.0);
                ui.label(
                    egui::RichText::new(title)
                        .strong()
                        .color(palette.text_primary),
                );
                ui.add_space(4.0);
                body(ui);
            });
        });
}

fn tone_color(theme: &Theme, tone: Tone) -> egui::Color32 {
    let palette = theme.palette();
    match tone {
        Tone::Good => palette.green,
        Tone::Bad => palette.red,
        Tone::Busy => palette.amber,
        Tone::Neutral => palette.text_secondary,
    }
}

fn yes_no(present: bool) -> (&'static str, Tone) {
    if present {
        ("gefunden", Tone::Good)
    } else {
        ("fehlt", Tone::Bad)
    }
}

pub fn show(ui: &mut egui::Ui, theme: &Theme, data: &DashboardData<'_>) -> Option<DashboardAction> {
    let mut action = None;
    let secondary = theme.palette().text_secondary;
    let columns = if ui.available_width() >= 640.0 { 2 } else { 1 };
    let width = (ui.available_width() - 12.0 * (columns as f32 - 1.0)) / columns as f32 - 4.0;

    egui::ScrollArea::vertical()
        .id_salt("dashboard")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(8.0);
            egui::Grid::new("dashboard-grid")
                .spacing([12.0, 12.0])
                .show(ui, |ui| {
                    card(ui, theme, tr("Project Overview"), width, |ui| {
                        ui.label(
                            egui::RichText::new(data.project_root.display().to_string())
                                .color(secondary),
                        );
                        ui.label(tr_fmt("{} Quelldateien", &[data.source_count.to_string()]));
                        ui.label(tr_fmt(
                            "{} offene Dokumente",
                            &[data.open_documents.to_string()],
                        ));
                    });
                    if columns == 1 {
                        ui.end_row();
                    }
                    card(ui, theme, tr("Build & Target Status"), width, |ui| {
                        let (label, tone) = build_status(data.last_run, data.run_in_progress);
                        ui.label(egui::RichText::new(label).color(tone_color(theme, tone)));
                        ui.label(tr("Ziel: Windows x64 (MSVC)"));
                        ui.label(tr_fmt(
                            "Overflow-Checks: {}",
                            &[if data.overflow_checks { "an" } else { "aus" }.to_string()],
                        ));
                    });
                    ui.end_row();

                    card(ui, theme, tr("Quick Actions & Recents"), width, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if ui.button(tr("Neu")).clicked() {
                                action = Some(DashboardAction::NewFile);
                            }
                            if ui.button(tr("Datei öffnen")).clicked() {
                                action = Some(DashboardAction::OpenFile);
                            }
                            if ui
                                .add_enabled(
                                    !data.run_in_progress,
                                    egui::Button::new(tr("Build & Run")),
                                )
                                .clicked()
                            {
                                action = Some(DashboardAction::BuildAndRun);
                            }
                        });
                        ui.separator();
                        if data.recent_files.is_empty() {
                            ui.label(
                                egui::RichText::new(tr("Noch keine zuletzt geöffneten Dateien."))
                                    .color(secondary),
                            );
                        }
                        for (path, exists) in recent_entries(data.recent_files) {
                            let name = path.display().to_string();
                            let text = if exists {
                                name
                            } else {
                                tr_fmt("{} (fehlt)", &[name.to_string()])
                            };
                            if ui
                                .add_enabled(exists, egui::Button::new(text).frame(false))
                                .clicked()
                            {
                                action = Some(DashboardAction::OpenRecent(path));
                            }
                        }
                    });
                    if columns == 1 {
                        ui.end_row();
                    }
                    card(ui, theme, tr("Toolchain Diagnostics"), width, |ui| {
                        let (compiler, tone) = yes_no(data.toolchain.compiler.is_some());
                        ui.label(
                            egui::RichText::new(tr_fmt("nortexc.exe: {}", &[compiler.to_string()]))
                                .color(tone_color(theme, tone)),
                        );
                        let (runtime, tone) = yes_no(data.toolchain.runtime_library.is_some());
                        ui.label(
                            egui::RichText::new(tr_fmt(
                                "nortex_runtime.lib: {}",
                                &[runtime.to_string()],
                            ))
                            .color(tone_color(theme, tone)),
                        );
                        if !data.toolchain.is_ready() {
                            ui.label(
                                egui::RichText::new(tr("Bauen: `cargo build --workspace`"))
                                    .color(secondary),
                            );
                        }
                    });
                    ui.end_row();

                    card(ui, theme, tr("Git Status"), width, |ui| match data.git {
                        GitStatus::Unavailable => {
                            ui.label(
                                egui::RichText::new(tr("git ist nicht installiert."))
                                    .color(secondary),
                            );
                        }
                        GitStatus::NotARepository => {
                            ui.label(
                                egui::RichText::new(tr("Kein Git-Repository.")).color(secondary),
                            );
                        }
                        GitStatus::Repository { branch, changes } => {
                            ui.label(tr_fmt("Branch: {}", &[branch.to_string()]));
                            ui.label(tr_fmt("{} Änderungen", &[changes.len().to_string()]));
                        }
                    });
                    if columns == 1 {
                        ui.end_row();
                    }
                    card(ui, theme, tr("AI & Agent Status"), width, |ui| {
                        ui.label(
                            egui::RichText::new(tr("Kein KI-Anbieter konfiguriert."))
                                .color(secondary),
                        );
                        ui.label(
                            egui::RichText::new(tr("Der Sidecar ist noch nicht angebunden."))
                                .color(secondary),
                        );
                    });
                    ui.end_row();
                });
        });
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::activity_bar::{ActivityAction, ViewMode, Workspace};
    use eframe::egui::{Pos2, Rect, vec2};

    #[test]
    fn build_status_reflects_state_priority() {
        assert_eq!(build_status(Some(RunOutcome::Failed), true).1, Tone::Busy);
        assert_eq!(
            build_status(Some(RunOutcome::Succeeded), false).1,
            Tone::Good
        );
        assert_eq!(build_status(Some(RunOutcome::Failed), false).1, Tone::Bad);
        assert_eq!(build_status(None, false).1, Tone::Neutral);
    }

    #[test]
    fn recent_entries_flag_missing_files() {
        let existing =
            std::env::temp_dir().join(format!("nortex-recent-{}.nx", std::process::id()));
        std::fs::write(&existing, "x").unwrap();
        let entries = recent_entries(&[existing.display().to_string(), "Z:\\nope\\gone.nx".into()]);
        assert!(entries[0].1);
        assert!(!entries[1].1);
        let _ = std::fs::remove_file(&existing);
    }

    #[test]
    fn view_mode_switch_round_trip_keeps_workspace_side_state() {
        let mut workspace = Workspace::default();
        assert_eq!(workspace.view_mode, ViewMode::Editor);
        workspace.apply(ActivityAction::Home);
        assert_eq!(workspace.view_mode, ViewMode::Dashboard);
        workspace.apply(ActivityAction::Editor);
        assert_eq!(workspace.view_mode, ViewMode::Editor);
        assert!(workspace.side_open);
    }

    #[test]
    fn dashboard_renders_at_narrow_and_wide_sizes_without_action() {
        let context = egui::Context::default();
        let theme = Theme::default();
        let toolchain = ToolchainStatus {
            compiler: None,
            runtime_library: None,
        };
        let recent = vec!["Z:\\nope.nx".to_string()];
        let git = GitStatus::Unavailable;
        let data = DashboardData {
            project_root: Path::new("."),
            source_count: 3,
            open_documents: 1,
            recent_files: &recent,
            toolchain: &toolchain,
            last_run: Some(RunOutcome::Failed),
            run_in_progress: false,
            overflow_checks: true,
            git: &git,
        };
        for size in [vec2(400.0, 300.0), vec2(1400.0, 900.0)] {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                ..Default::default()
            };
            let mut result = None;
            let _ = context.run(input, |context| {
                egui::CentralPanel::default().show(context, |ui| result = show(ui, &theme, &data));
            });
            assert_eq!(result, None);
        }
    }
}
