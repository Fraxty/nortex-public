//! Einstellungsansicht: Suche, Ebenenfilter, Kategorien, Zurücksetzen, Import und Export.

use crate::*;
use nortex_ide::i18n::tr_fmt;
use nortex_ide::ui::keymap::SHORTCUTS;
use nortex_ide::ui::settings_catalog::{self as catalog, Availability, Category, Entry};

const THEME_KEYS: [&str; 4] = ["theme_mode", "density", "ui_zoom", "editor_zoom"];

impl NortexCodeApp {
    pub(crate) fn draw_settings_view(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        panel_header(
            ui,
            &self.theme,
            5,
            tr("Einstellungen"),
            self.theme.palette().blue,
        );
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.settings_query)
                    .hint_text(tr("Einstellungen suchen"))
                    .desired_width(220.0),
            );
            let selected = self
                .settings_level
                .map_or("Alle Ebenen", SettingLevel::title);
            egui::ComboBox::from_id_salt("settings-level")
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.settings_level, None, tr("Alle Ebenen"));
                    for level in SettingLevel::ALL {
                        ui.selectable_value(&mut self.settings_level, Some(level), level.title());
                    }
                });
        });
        ui.label(
            egui::RichText::new(
                tr("Benutzer: persönliche Vorgaben · Projekt: pro Projekt (vorbereitet) · Erweitert: Spezialoptionen"),
            )
            .small()
            .color(self.theme.palette().text_muted),
        );
        ui.add_space(6.0);

        let matches = catalog::search(&self.settings_query, self.settings_level);
        let mut theme_block_drawn = false;
        let mut reset_request: Option<Vec<&'static str>> = None;
        egui::ScrollArea::vertical()
            .id_salt("settings-scroll")
            .show(ui, |ui| {
                for category in Category::ALL {
                    let entries: Vec<&Entry> = matches
                        .iter()
                        .copied()
                        .filter(|entry| entry.category == category)
                        .collect();
                    if entries.is_empty() {
                        continue;
                    }
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.heading(category.title());
                        if catalog::keys_in(category).next().is_some()
                            && ui
                                .small_button(tr("Kategorie zurücksetzen"))
                                .on_hover_text(tr(
                                    "Setzt alle aktiven Einstellungen dieser Kategorie zurück",
                                ))
                                .clicked()
                        {
                            reset_request = Some(catalog::keys_in(category).collect());
                        }
                    });
                    for entry in entries {
                        if let Some(keys) =
                            self.draw_setting_entry(ui, context, entry, &mut theme_block_drawn)
                        {
                            reset_request = Some(keys);
                        }
                    }
                    if category == Category::Account
                        && ui.button(tr("Nortex-Konto öffnen…")).clicked()
                    {
                        self.account_open = true;
                    }
                }
                self.draw_shortcut_table(ui);
                ui.add_space(14.0);
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button(tr("Alle zurücksetzen")).clicked() {
                        reset_request = Some(
                            Category::ALL
                                .into_iter()
                                .flat_map(catalog::keys_in)
                                .collect(),
                        );
                    }
                    if ui.button(tr("Exportieren…")).clicked() {
                        self.export_settings();
                    }
                    if ui.button(tr("Importieren…")).clicked() {
                        self.import_settings(context);
                    }
                });
                ui.label(
                    egui::RichText::new(tr_fmt(
                        "Einstellungsdatei: {}",
                        &[self.settings_path.display().to_string()],
                    ))
                    .small()
                    .color(self.theme.palette().text_muted),
                );
            });

        if let Some(keys) = reset_request {
            self.settings.reset_keys(keys.iter().copied());
            self.apply_settings(context);
            self.status = tr("Einstellungen zurückgesetzt").into();
        }
    }

    /// Zeichnet einen Katalogeintrag und liefert die zurückzusetzenden Schlüssel, falls ↺ geklickt wurde.
    fn draw_setting_entry(
        &mut self,
        ui: &mut egui::Ui,
        context: &egui::Context,
        entry: &Entry,
        theme_block_drawn: &mut bool,
    ) -> Option<Vec<&'static str>> {
        let palette = self.theme.palette();
        let mut reset = None;
        let planned = matches!(entry.availability, Availability::Planned(_));
        ui.add_space(4.0);
        if THEME_KEYS.contains(&entry.key) {
            if !*theme_block_drawn {
                *theme_block_drawn = true;
                self.theme_controls(ui, context);
            }
            if !self.settings.is_default(entry.key) {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        palette.amber,
                        format!("• {} {}", tr(entry.label), tr("geändert")),
                    );
                    if ui
                        .small_button("↺")
                        .on_hover_text(tr("Standardwert"))
                        .clicked()
                    {
                        reset = Some(vec![entry.key]);
                    }
                });
            }
            return reset;
        }
        ui.horizontal(|ui| {
            let mut changed = false;
            ui.add_enabled_ui(!planned, |ui| match entry.key {
                "editor.line_numbers.enabled" => {
                    changed = ui
                        .checkbox(&mut self.settings.line_numbers_enabled, tr(entry.label))
                        .changed();
                }
                "editor.minimap.enabled" => {
                    let mut value = self.settings.minimap_enabled;
                    if ui.checkbox(&mut value, tr(entry.label)).changed() {
                        self.toggle_minimap();
                    }
                }
                "editor.auto_save.enabled" => {
                    changed = ui
                        .checkbox(&mut self.settings.auto_save_enabled, tr(entry.label))
                        .changed();
                }
                "window.status_bar.visible" => {
                    changed = ui
                        .checkbox(&mut self.settings.status_bar_visible, tr(entry.label))
                        .changed();
                }
                "build.overflow_checks" => {
                    changed = ui
                        .checkbox(&mut self.settings.overflow_checks, tr(entry.label))
                        .changed();
                }
                "extensions.max_parallel_runs" => {
                    changed = ui
                        .add(
                            egui::DragValue::new(&mut self.settings.max_parallel_extension_runs)
                                .range(1..=nortex_ide::ui::settings::MAX_PARALLEL_EXTENSION_RUNS),
                        )
                        .changed();
                    ui.label(tr(entry.label));
                }
                _ => {
                    ui.label(tr(entry.label));
                }
            });
            if changed {
                self.persist_settings();
            }
            if !planned && !self.settings.is_default(entry.key) {
                ui.colored_label(palette.amber, "•")
                    .on_hover_text(tr("Vom Standard abweichend"));
                if ui
                    .small_button("↺")
                    .on_hover_text(tr("Standardwert"))
                    .clicked()
                {
                    reset = Some(vec![entry.key]);
                }
            }
            if let Availability::Planned(missing) = entry.availability {
                ui.label(
                    egui::RichText::new(tr("geplant"))
                        .small()
                        .color(palette.text_muted),
                )
                .on_hover_text(tr(missing));
            }
        });
        ui.label(
            egui::RichText::new(tr(entry.description))
                .small()
                .color(palette.text_muted),
        );
        reset
    }

    fn draw_shortcut_table(&self, ui: &mut egui::Ui) {
        let words: Vec<String> = self
            .settings_query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let rows: Vec<&(&str, &str)> = SHORTCUTS
            .iter()
            .filter(|(keys, label)| {
                let text = format!("{keys} {label} tastenkürzel shortcut").to_lowercase();
                words.iter().all(|word| text.contains(word))
            })
            .collect();
        if rows.is_empty() || self.settings_level.is_some_and(|l| l != SettingLevel::User) {
            return;
        }
        ui.add_space(10.0);
        ui.heading(Category::Shortcuts.title());
        ui.label(
            egui::RichText::new(tr(
                "Anzeige der aktuellen Belegung; Anpassen ist noch nicht möglich.",
            ))
            .small()
            .color(self.theme.palette().text_muted),
        );
        egui::Grid::new("shortcut-grid")
            .num_columns(2)
            .show(ui, |ui| {
                for (keys, label) in rows {
                    ui.monospace(nortex_ide::i18n::keys(keys));
                    ui.label(tr(label));
                    ui.end_row();
                }
            });
    }

    /// Überträgt `self.settings` auf den laufenden Zustand und speichert.
    pub(crate) fn apply_settings(&mut self, context: &egui::Context) {
        let theme = self.settings.theme();
        self.change_theme(context, theme);
    }

    fn export_settings(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(tr("Einstellungen exportieren"))
            .add_filter(tr("JSON"), &["json"])
            .set_file_name("nortex-code-settings.json")
            .save_file()
        else {
            return;
        };
        self.status = match fs::write(&path, self.settings.to_json_pretty()) {
            Ok(()) => tr_fmt(
                "Einstellungen exportiert: {}",
                &[path.display().to_string()],
            ),
            Err(error) => tr_fmt("Export fehlgeschlagen: {}", &[error.to_string()]),
        };
    }

    fn import_settings(&mut self, context: &egui::Context) {
        let Some(path) = rfd::FileDialog::new()
            .set_title(tr("Einstellungen importieren"))
            .add_filter(tr("JSON"), &["json"])
            .pick_file()
        else {
            return;
        };
        let parsed = fs::read(&path).ok().and_then(|bytes| {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            Settings::from_json_lenient(text.trim_start_matches('\u{feff}'))
        });
        match parsed {
            Some(mut imported) => {
                imported.recent_files = self.settings.recent_files.clone();
                self.settings = imported;
                self.apply_settings(context);
                self.status = tr_fmt(
                    "Einstellungen importiert: {}",
                    &[path.display().to_string()],
                );
            }
            None => {
                self.status = tr("Import fehlgeschlagen: keine gültige Einstellungsdatei.").into()
            }
        }
    }
}
