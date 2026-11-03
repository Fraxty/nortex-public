//! Titelleiste, Menüs, Hilfefenster und Darstellungssteuerung.

use crate::*;
use nortex_ide::i18n::tr_fmt;

impl NortexCodeApp {
    pub(crate) fn draw_titlebar(&mut self, context: &egui::Context) {
        let palette = self.theme.palette();
        let frame = egui::Frame::default()
            .fill(palette.panel)
            .inner_margin(egui::Margin::symmetric(8, 3));
        egui::TopBottomPanel::top("main-menu")
            .frame(frame)
            .show(context, |ui| {
                let maximized = context.input(|input| input.viewport().maximized == Some(true));
                let bar = ui.available_rect_before_wrap();
                let height = 24.0;
                let controls_width = 3.0 * 38.0 + 110.0;
                let controls_rect = egui::Rect::from_min_size(
                    egui::pos2((bar.right() - controls_width).max(bar.left()), bar.top()),
                    egui::vec2(controls_width.min(bar.width()), height),
                );
                let left_rect = egui::Rect::from_min_max(
                    bar.left_top(),
                    egui::pos2(controls_rect.left() - 4.0, bar.top() + height),
                );
                ui.allocate_rect(
                    egui::Rect::from_min_max(bar.left_top(), controls_rect.right_bottom()),
                    egui::Sense::hover(),
                );
                // Zuerst registriert: alle später gezeichneten Widgets liegen darüber,
                // freie Flächen der Leiste dienen als Ziehbereich.
                let drag = ui.interact(
                    left_rect,
                    ui.id().with("titlebar-drag"),
                    egui::Sense::click_and_drag(),
                );
                if drag.double_clicked() {
                    context.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                } else if drag.drag_started() {
                    context.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                let search_width = 220.0_f32.min((left_rect.width() * 0.3).max(0.0));
                let search_rect = egui::Rect::from_min_max(
                    egui::pos2(left_rect.right() - search_width, left_rect.top()),
                    left_rect.right_bottom(),
                );
                let menu_rect = egui::Rect::from_min_max(
                    left_rect.left_top(),
                    egui::pos2(search_rect.left() - 8.0, left_rect.bottom()),
                );
                // Fensterknöpfe getrennt vom Rest, damit sie immer sichtbar und klickbar bleiben.
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(controls_rect)
                        .layout(egui::Layout::right_to_left(egui::Align::Center)),
                    |ui| {
                        if window_button(ui, WindowButton::Close, &palette, tr("Schließen"))
                            .clicked()
                        {
                            context.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        let (kind, tip) = if maximized {
                            (WindowButton::Restore, tr("Fenster wiederherstellen"))
                        } else {
                            (WindowButton::Maximize, tr("Maximieren"))
                        };
                        if window_button(ui, kind, &palette, tip).clicked() {
                            context.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                        }
                        if window_button(ui, WindowButton::Minimize, &palette, tr("Minimieren"))
                            .clicked()
                        {
                            context.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        self.draw_account_menu(ui);
                    },
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(menu_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        if let Some(symbol) = self.textures.symbol() {
                            let home = ui
                                .add(
                                    egui::Image::new(symbol)
                                        .fit_to_exact_size(egui::vec2(18.0, 18.0))
                                        .sense(egui::Sense::click()),
                                )
                                .on_hover_text(tr("Startseite"));
                            if home.clicked() {
                                self.workspace.view_mode = ViewMode::Dashboard;
                            }
                        }
                        egui::menu::bar(ui, |ui| self.draw_menu_buttons(ui, context));
                    },
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(search_rect)
                        .layout(egui::Layout::left_to_right(egui::Align::Center)),
                    |ui| {
                        let search = ui.add(
                            egui::TextEdit::singleline(&mut self.header_search)
                                .hint_text(tr("Suchen (Enter)"))
                                .desired_width(search_width),
                        );
                        if search.lost_focus()
                            && ui.input(|input| input.key_pressed(egui::Key::Enter))
                            && !self.header_search.trim().is_empty()
                        {
                            self.search_query = self.header_search.clone();
                            self.search_hits =
                                search::search(&self.source_files, &self.search_query);
                            self.show_side_view(SideView::Search);
                        }
                    },
                );
            });
    }

    pub(crate) fn draw_help_windows(&mut self, context: &egui::Context) {
        if self.shortcuts_open {
            egui::Window::new(tr("Tastatur-Kürzel"))
                .open(&mut self.shortcuts_open)
                .show(context, |ui| {
                    for (keys, description) in nortex_ide::ui::keymap::SHORTCUTS {
                        ui.label(format!(
                            "{}  {}",
                            nortex_ide::i18n::keys(keys),
                            tr(description)
                        ));
                    }
                });
        }
        if self.license_open {
            let palette = self.theme.palette();
            egui::Window::new(tr("Lizenz"))
                .open(&mut self.license_open)
                .default_width(560.0)
                .vscroll(true)
                .show(context, |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(include_str!("../../../LICENSE"))
                                .color(palette.text_primary),
                        )
                        .wrap(),
                    );
                });
        }
        if self.about_open {
            egui::Window::new(tr("Info"))
                .open(&mut self.about_open)
                .show(context, |ui| {
                    ui.label(format!("NORTEX Code {}", env!("CARGO_PKG_VERSION")));
                    ui.label(tr("Editor für die Programmiersprache NORTEX."));
                });
        }
        if self.manual_open {
            let palette = self.theme.palette();
            egui::Window::new(tr("NORTEX Code – Handbuch"))
                .open(&mut self.manual_open)
                .default_width(760.0)
                .default_height(620.0)
                .vscroll(true)
                .show(context, |ui| {
                    for (index, chapter) in USER_MANUAL.iter().enumerate() {
                        if index > 0 {
                            ui.separator();
                        }
                        render_markdown(ui, chapter, &palette);
                    }
                });
        }
        if let Some((pane, document)) = self.pending_close_document {
            let name = self
                .documents
                .documents()
                .get(document)
                .and_then(|document| document.path())
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| tr("Unbenannt").into());
            let mut open = true;
            let mut decision = None;
            egui::Window::new(tr("Ungespeicherte Änderungen"))
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(context, |ui| {
                    ui.label(tr_fmt("Änderungen an „{}“ speichern?", &[name.to_string()]));
                    ui.horizontal(|ui| {
                        if ui.button(tr("Speichern")).clicked() {
                            decision = Some(ClosePromptAction::Save);
                        }
                        if ui.button(tr("Verwerfen")).clicked() {
                            decision = Some(ClosePromptAction::Discard);
                        }
                        if ui.button(tr("Abbrechen")).clicked() {
                            decision = Some(ClosePromptAction::Cancel);
                        }
                    });
                });
            if !open || decision == Some(ClosePromptAction::Cancel) {
                self.pending_close_document = None;
            }
            match decision {
                Some(ClosePromptAction::Save) => {
                    self.activate_tab(pane, document);
                    if self.save_document() {
                        self.pending_close_document = None;
                        self.close_tab(pane, document);
                    }
                }
                Some(ClosePromptAction::Discard) => {
                    self.pending_close_document = None;
                    if self.grid.holders(document) > 1 {
                        self.close_tab(pane, document);
                    } else {
                        self.close_document(pane, document, true);
                    }
                }
                Some(ClosePromptAction::Cancel) | None => {}
            }
        }
        if self.goto_open {
            let mut jump = None;
            let mut open = true;
            let mut submitted = false;
            egui::Window::new(tr("Zu Zeile springen"))
                .open(&mut open)
                .collapsible(false)
                .show(context, |ui| {
                    let response = ui.text_edit_singleline(&mut self.goto_input);
                    response.request_focus();
                    if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                        jump = self.goto_input.trim().parse::<usize>().ok();
                        if jump.is_none() {
                            self.status = tr("Ungültige Zeilennummer.").into();
                        }
                        submitted = true;
                    }
                });
            if let Some(line) = jump {
                self.jump_to_line(line.saturating_sub(1));
            }
            self.goto_open = open && !submitted;
        }
    }

    pub(crate) fn diagnostic_info(&self) -> String {
        format!(
            "NORTEX Code {}\nBetriebssystem: {} ({})\nProjektordner: {}\nOffene Dateien: {}\nDebugger-Backend: {}\nKonto: {}",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            self.project_root.display(),
            self.documents.documents().len(),
            self.debugger.backend_name(),
            self.account.status().label(),
        )
    }

    pub(crate) fn draw_menu_buttons(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        {
            {
                ui.menu_button(tr("Datei"), |ui| {
                    if ui.button(tr("Neue Datei  Strg+N")).clicked() {
                        self.new_document();
                        ui.close_menu();
                    }
                    if ui.button(tr("Öffnen…")).clicked() {
                        self.run_palette_command(context, PaletteCommand::OpenFile);
                        ui.close_menu();
                    }
                    if ui.button(tr("Ordner öffnen…")).clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_directory(&self.project_root)
                            .pick_folder()
                        {
                            self.project_path_input = path.display().to_string();
                            self.request_project_refresh();
                        }
                        ui.close_menu();
                    }
                    if ui.button(tr("Speichern  Strg+S")).clicked() {
                        self.save_document();
                        ui.close_menu();
                    }
                    if ui.button(tr("Speichern unter…")).clicked() {
                        self.save_document_as();
                        ui.close_menu();
                    }
                    if ui.button(tr("Projektordner schließen")).clicked() {
                        self.workspace.side_open = false;
                        ui.close_menu();
                    }
                    if ui.button(tr("Projekt aktualisieren")).clicked() {
                        self.request_project_refresh();
                        ui.close_menu();
                    }
                    if ui.button(tr("Projektordner im Explorer öffnen")).clicked() {
                        self.status = match Command::new("explorer").arg(&self.project_root).spawn()
                        {
                            Ok(_) => tr("Projektordner geöffnet").into(),
                            Err(error) => tr_fmt(
                                "Projektordner konnte nicht geöffnet werden: {}",
                                &[error.to_string()],
                            ),
                        };
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(tr("Beenden")).clicked() {
                        context.send_viewport_cmd(egui::ViewportCommand::Close);
                        ui.close_menu();
                    }
                });
                ui.menu_button(tr("Bearbeiten"), |ui| {
                    let can_undo = self
                        .documents
                        .active_document()
                        .is_some_and(|document| document.buffer().can_undo());
                    let can_redo = self
                        .documents
                        .active_document()
                        .is_some_and(|document| document.buffer().can_redo());
                    if ui
                        .add_enabled(can_undo, egui::Button::new(tr("Rückgängig")))
                        .clicked()
                    {
                        self.edit_history(false);
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(can_redo, egui::Button::new(tr("Wiederholen")))
                        .clicked()
                    {
                        self.edit_history(true);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(tr("Ausschneiden  Strg+X")).clicked() {
                        self.copy_selection(context, true);
                        ui.close_menu();
                    }
                    if ui.button(tr("Kopieren  Strg+C")).clicked() {
                        self.copy_selection(context, false);
                        ui.close_menu();
                    }
                    ui.add_enabled(false, egui::Button::new(tr("Einfügen  Strg+V")))
                        .on_disabled_hover_text(tr(
                            "Einfügen funktioniert über Strg+V direkt im Editor.",
                        ));
                    if ui.button(tr("Alles auswählen  Strg+A")).clicked() {
                        self.select_all_in_editor(context);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button(tr("Suchen  Strg+F")).clicked() {
                        self.run_action(context, Action::FindInFile);
                        ui.close_menu();
                    }
                    if ui.button(tr("Suchen/Ersetzen  Strg+H")).clicked() {
                        self.run_action(context, Action::Replace);
                        ui.close_menu();
                    }
                    if ui.button(tr("Zeile duplizieren")).clicked() {
                        self.run_action(context, Action::DuplicateLine);
                        ui.close_menu();
                    }
                    if ui.button(tr("Zeile nach oben")).clicked() {
                        self.run_action(context, Action::MoveLineUp);
                        ui.close_menu();
                    }
                    if ui.button(tr("Zeile nach unten")).clicked() {
                        self.run_action(context, Action::MoveLineDown);
                        ui.close_menu();
                    }
                    if ui.button(tr("Nächstes Vorkommen auswählen")).clicked() {
                        self.run_action(context, Action::SelectNextOccurrence);
                        ui.close_menu();
                    }
                    if ui.button(tr("Block-Kommentar  Strg+/")).clicked() {
                        self.run_action(context, Action::ToggleComment);
                        ui.close_menu();
                    }
                    if ui.button(tr("Zu Zeile springen…")).clicked() {
                        self.goto_open = true;
                        self.goto_input.clear();
                        ui.close_menu();
                    }
                });
                ui.menu_button(tr("Ansicht"), |ui| {
                    ui.checkbox(&mut self.zen_mode, tr("Zen-Modus (F11)"));
                    ui.checkbox(&mut self.workspace.side_open, tr("Explorer (Strg+B)"));
                    ui.checkbox(
                        &mut self.problems_visible,
                        tr("Terminal / Diagnosen (Strg+J)"),
                    );
                    if ui
                        .checkbox(
                            &mut self.settings.status_bar_visible,
                            tr("Statusleiste anzeigen"),
                        )
                        .changed()
                    {
                        self.persist_settings();
                    }
                    ui.menu_button(tr("Seitenleiste"), |ui| {
                        for (label, view) in [
                            ("Explorer", SideView::Explorer),
                            ("Suche", SideView::Search),
                            ("Git", SideView::Git),
                            ("Debugger", SideView::Debugger),
                        ] {
                            if ui.button(label).clicked() {
                                self.show_side_view(view);
                                ui.close_menu();
                            }
                        }
                    });
                    if ui.button(tr("Layout zurücksetzen")).clicked() {
                        self.workspace.side_open = true;
                        self.problems_visible = true;
                        self.zen_mode = false;
                        self.set_split(SplitMode::Single);
                        self.settings.status_bar_visible = true;
                        self.persist_settings();
                        ui.close_menu();
                    }
                    if ui
                        .checkbox(
                            &mut self.settings.line_numbers_enabled,
                            tr("Zeilennummern anzeigen"),
                        )
                        .changed()
                    {
                        self.persist_settings();
                    }
                    let mut minimap_on = self.settings.minimap_enabled;
                    if ui
                        .checkbox(&mut minimap_on, tr("Minimap anzeigen (Strg+Alt+M)"))
                        .changed()
                    {
                        self.toggle_minimap();
                    }
                    if ui.button(tr("Zoom zurücksetzen")).clicked() {
                        let mut next = self.theme;
                        next.zoom.ui = 1.0;
                        next.zoom.editor = 1.0;
                        self.change_theme(context, next);
                        ui.close_menu();
                    }
                    if ui
                        .checkbox(&mut self.sidecar_visible, tr("Sidecar (Strg+Umschalt+L)"))
                        .changed()
                    {
                        self.sidecar_forced = self.sidecar_visible;
                    }
                    ui.menu_button(tr("Editor-Layout"), |ui| {
                        let entries = [
                            (SplitMode::Single, tr("Einzeln"), ""),
                            (SplitMode::TwoColumns, tr("Zwei Spalten"), "Strg+\\"),
                            (SplitMode::TwoRows, tr("Zwei Zeilen"), "Strg+Umschalt+\\"),
                            (SplitMode::FourGrid, "2x2 Raster", "Strg+Umschalt+4"),
                        ];
                        for (mode, label, shortcut) in entries {
                            let text = format!("{label}  {shortcut}");
                            if ui
                                .selectable_label(self.grid.mode() == mode, text)
                                .clicked()
                            {
                                self.set_split(mode);
                                ui.close_menu();
                            }
                        }
                        if ui
                            .add_enabled(
                                self.grid.len() > 1,
                                egui::Button::new(tr("Pane schließen  Strg+Umschalt+W")),
                            )
                            .clicked()
                        {
                            self.close_active_split_pane();
                            ui.close_menu();
                        }
                    });
                    ui.separator();
                    self.theme_controls(ui, context);
                });
                let plugin_actions = self.plugin_host.active_actions();
                if !plugin_actions.is_empty() {
                    ui.menu_button(tr("Erweiterungen"), |ui| {
                        for (plugin, action) in &plugin_actions {
                            if ui
                                .button(&action.title)
                                .on_hover_text(plugin.as_str())
                                .clicked()
                            {
                                self.invoke_plugin_action(plugin, &action.id);
                                ui.close_menu();
                            }
                        }
                    });
                }
                ui.menu_button(tr("KI"), |ui| {
                    ui.label(
                        egui::RichText::new(tr(
                            "Noch nicht angebunden – es wird kein Code gesendet.",
                        ))
                        .small(),
                    );
                    ui.separator();
                    for label in [
                        tr("Code erklären"),
                        tr("Refactoring-Vorschlag"),
                        tr("Fehler analysieren"),
                        tr("Tests vorschlagen"),
                        tr("Dokumentation generieren"),
                    ] {
                        ui.add_enabled(false, egui::Button::new(label))
                            .on_disabled_hover_text("Geplant: benötigt einen KI-Anbieter.");
                    }
                });
                ui.menu_button(tr("Hilfe"), |ui| {
                    if ui.button(tr("Erste Schritte")).clicked() {
                        self.manual_open = true;
                        ui.close_menu();
                    }
                    if ui.button(tr("Diagnoseinformationen kopieren")).clicked() {
                        context.copy_text(self.diagnostic_info());
                        self.status =
                            tr("Diagnoseinformationen in die Zwischenablage kopiert.").into();
                        ui.close_menu();
                    }
                    if ui.button(tr("Lizenz")).clicked() {
                        self.license_open = true;
                        ui.close_menu();
                    }
                    if ui.button(tr("Handbuch (PDF) öffnen")).clicked() {
                        let english =
                            nortex_ide::i18n::language() == nortex_ide::i18n::Language::English;
                        let (name, bytes): (&str, &[u8]) = if english {
                            (
                                "NORTEX-Code-Manual.pdf",
                                include_bytes!("../../../docs/user-manual/NORTEX-Code-Manual.pdf"),
                            )
                        } else {
                            (
                                "NORTEX-Code-Handbuch.pdf",
                                include_bytes!(
                                    "../../../docs/user-manual/NORTEX-Code-Handbuch.pdf"
                                ),
                            )
                        };
                        let target = std::env::temp_dir().join("NORTEX Code").join(name);
                        let written = target
                            .parent()
                            .map(std::fs::create_dir_all)
                            .transpose()
                            .and_then(|_| std::fs::write(&target, bytes));
                        self.status = match written
                            .and_then(|_| Command::new("explorer").arg(&target).spawn())
                        {
                            Ok(_) => tr("Handbuch (PDF) geöffnet.").into(),
                            Err(error) => tr_fmt(
                                "Handbuch konnte nicht geöffnet werden: {}",
                                &[error.to_string()],
                            ),
                        };
                        ui.close_menu();
                    }
                    if ui.button(tr("Handbuch im Fenster")).clicked() {
                        self.manual_open = true;
                        ui.close_menu();
                    }
                    if ui.button(tr("Dokumentationsordner öffnen")).clicked() {
                        let target = self.project_root.join("docs");
                        self.status = match Command::new("explorer").arg(&target).spawn() {
                            Ok(_) => tr("Dokumentationsordner geöffnet").into(),
                            Err(error) => {
                                tr_fmt("Dokumentation nicht geöffnet: {}", &[error.to_string()])
                            }
                        };
                        ui.close_menu();
                    }
                    if ui.button(tr("Tastatur-Kürzel")).clicked() {
                        self.shortcuts_open = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    for section in [
                        HelpSection::Report(nortex_ide::feedback::Kind::Bug),
                        HelpSection::Report(nortex_ide::feedback::Kind::Improvement),
                        HelpSection::Report(nortex_ide::feedback::Kind::Documentation),
                        HelpSection::Report(nortex_ide::feedback::Kind::General),
                        HelpSection::Support,
                    ] {
                        let label = match section {
                            HelpSection::Report(kind) => kind.label(),
                            _ => "Support und Kontakt",
                        };
                        if ui.button(tr(label)).clicked() {
                            self.feedback.open_section(section);
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                    if ui.button(tr("Update-Check")).clicked() {
                        self.status = format!(
                            "NORTEX Code {} – Update-Prüfung ist offline nicht verfügbar.",
                            env!("CARGO_PKG_VERSION")
                        );
                        ui.close_menu();
                    }
                    if ui.button(tr("Info")).clicked() {
                        self.about_open = true;
                        ui.close_menu();
                    }
                });
            }
        }
    }

    pub(crate) fn theme_controls(&mut self, ui: &mut egui::Ui, context: &egui::Context) {
        let mut next = self.theme;
        let palette = next.palette();
        // Slider-Knopf: runder Griff mit gelbem Rand.
        {
            let style = ui.style_mut();
            style.visuals.handle_shape = egui::style::HandleShape::Circle;
            style.visuals.slider_trailing_fill = true;
            for widget in [
                &mut style.visuals.widgets.inactive,
                &mut style.visuals.widgets.hovered,
                &mut style.visuals.widgets.active,
            ] {
                widget.fg_stroke = egui::Stroke::new(2.0, palette.amber);
            }
            style.visuals.widgets.inactive.bg_fill = palette.text_secondary;
            style.visuals.widgets.hovered.bg_fill = palette.text_primary;
            style.visuals.widgets.active.bg_fill = palette.text_primary;
            style.visuals.selection.bg_fill = palette.amber;
        }
        egui::Grid::new("appearance-choices")
            .num_columns(2)
            .spacing([12.0, 8.0])
            .show(ui, |ui| {
                ui.label(tr("Theme"));
                egui::ComboBox::from_id_salt("theme-choice")
                    .width(220.0)
                    .selected_text(tr(next.kind.label()))
                    .show_ui(ui, |ui| {
                        for kind in ThemeKind::ALL {
                            ui.selectable_value(&mut next.kind, kind, tr(kind.label()));
                        }
                    });
                ui.end_row();
                ui.label(tr("Dichte"));
                egui::ComboBox::from_id_salt("density-choice")
                    .width(220.0)
                    .selected_text(format!(
                        "{} – {}",
                        tr(next.density.label()),
                        tr(next.density.description())
                    ))
                    .show_ui(ui, |ui| {
                        for density in Density::ALL {
                            ui.selectable_value(
                                &mut next.density,
                                density,
                                format!("{} – {}", tr(density.label()), tr(density.description())),
                            );
                        }
                    });
                ui.end_row();
                ui.label(tr("Sprache"));
                let mut language = nortex_ide::i18n::language();
                egui::ComboBox::from_id_salt("language-choice")
                    .width(220.0)
                    .selected_text(language.native_name())
                    .show_ui(ui, |ui| {
                        for option in nortex_ide::i18n::Language::ALL {
                            ui.selectable_value(&mut language, option, option.native_name());
                        }
                    });
                ui.end_row();
                if language != nortex_ide::i18n::language() {
                    self.set_language(language);
                }
            });
        ui.horizontal(|ui| {
            ui.label(tr("UI-Zoom"));
            if let Some(value) = zoom_slider(ui, "ui", self.theme.zoom.ui) {
                next.zoom.ui = value;
                self.ui_zoom_input = zoom_input(value);
            }
            let input = ui.add(
                egui::TextEdit::singleline(&mut self.ui_zoom_input)
                    .desired_width(58.0)
                    .hint_text("%"),
            );
            if input.lost_focus()
                || (input.has_focus() && ui.input(|state| state.key_pressed(egui::Key::Enter)))
            {
                if let Some(value) = parse_zoom_input(&self.ui_zoom_input) {
                    next.zoom.ui = value;
                    self.ui_zoom_input = zoom_input(value);
                } else {
                    self.ui_zoom_input = zoom_input(self.theme.zoom.ui);
                    self.status =
                        tr("Ungültiger UI-Zoom. Erwartet wird ein Wert von 70% bis 200%.").into();
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr("Editor-Zoom"));
            if let Some(value) = zoom_slider(ui, "editor", self.theme.zoom.editor) {
                next.zoom.editor = value;
                self.editor_zoom_input = zoom_input(value);
            }
            let input = ui.add(
                egui::TextEdit::singleline(&mut self.editor_zoom_input)
                    .desired_width(58.0)
                    .hint_text("%"),
            );
            if input.lost_focus()
                || (input.has_focus() && ui.input(|state| state.key_pressed(egui::Key::Enter)))
            {
                if let Some(value) = parse_zoom_input(&self.editor_zoom_input) {
                    next.zoom.editor = value;
                    self.editor_zoom_input = zoom_input(value);
                } else {
                    self.editor_zoom_input = zoom_input(self.theme.zoom.editor);
                    self.status =
                        tr("Ungültiger Editor-Zoom. Erwartet wird ein Wert von 70% bis 200%.")
                            .into();
                }
            }
        });
        if next != self.theme {
            self.change_theme(context, next);
        }
    }
}

/// Zoom-Regler in 10-%-Schritten. Der Wert wird erst beim Loslassen übernommen,
/// damit sich die Oberfläche nicht unter dem Mauszeiger mitskaliert.
fn zoom_slider(ui: &mut egui::Ui, id: &str, applied: f32) -> Option<f32> {
    let key = egui::Id::new(("zoom-slider-pending", id));
    let stored: Option<f32> = ui.data(|data| data.get_temp(key));
    let mut percent = stored.unwrap_or(applied) * 100.0;
    let response = ui.add(
        egui::Slider::new(&mut percent, 70.0..=200.0)
            .step_by(10.0)
            .show_value(false),
    );
    ui.label(format!("{percent:.0}%"));
    if response.dragged() {
        ui.data_mut(|data| data.insert_temp(key, percent / 100.0));
        return None;
    }
    ui.data_mut(|data| data.remove::<f32>(key));
    let committed = response.drag_stopped() || response.changed();
    committed.then(|| typography::clamp_zoom(percent / 100.0))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowButton {
    Minimize,
    Maximize,
    Restore,
    Close,
}

/// Fensterknopf mit selbst gezeichnetem Symbol (unabhängig von Schrift-Glyphen).
pub(crate) fn window_button(
    ui: &mut egui::Ui,
    kind: WindowButton,
    palette: &nortex_ide::ui::theme::colors::Palette,
    tooltip: &str,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(38.0, 24.0), egui::Sense::click());
    let response = response.on_hover_text(tooltip);
    if response.hovered() {
        let fill = if kind == WindowButton::Close {
            palette.red
        } else {
            palette.interactive
        };
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::ZERO, fill);
    }
    let stroke = egui::Stroke::new(1.5, palette.text_primary);
    let c = rect.center();
    let painter = ui.painter();
    match kind {
        WindowButton::Minimize => {
            painter.line_segment(
                [c + egui::vec2(-5.0, 3.0), c + egui::vec2(5.0, 3.0)],
                stroke,
            );
        }
        WindowButton::Maximize => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c, egui::vec2(10.0, 10.0)),
                egui::CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        WindowButton::Restore => {
            painter.rect_stroke(
                egui::Rect::from_center_size(c + egui::vec2(-1.0, 1.0), egui::vec2(8.0, 8.0)),
                egui::CornerRadius::ZERO,
                stroke,
                egui::StrokeKind::Inside,
            );
            painter.line_segment(
                [c + egui::vec2(-1.0, -5.0), c + egui::vec2(5.0, -5.0)],
                stroke,
            );
            painter.line_segment(
                [c + egui::vec2(5.0, -5.0), c + egui::vec2(5.0, 1.0)],
                stroke,
            );
        }
        WindowButton::Close => {
            painter.line_segment(
                [c + egui::vec2(-5.0, -5.0), c + egui::vec2(5.0, 5.0)],
                stroke,
            );
            painter.line_segment(
                [c + egui::vec2(-5.0, 5.0), c + egui::vec2(5.0, -5.0)],
                stroke,
            );
        }
    }
    response
}

fn plain_inline(text: &str) -> String {
    text.replace("**", "").replace('`', "").replace("__", "")
}

/// Einfache Markdown-Darstellung (Überschriften, Listen, Codeblöcke, Tabellen).
pub(crate) fn render_markdown(
    ui: &mut egui::Ui,
    text: &str,
    palette: &nortex_ide::ui::theme::colors::Palette,
) {
    let mut in_code = false;
    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code || trimmed.trim_start().starts_with('|') {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(trimmed)
                        .monospace()
                        .color(palette.text_secondary),
                )
                .wrap_mode(egui::TextWrapMode::Wrap),
            );
            continue;
        }
        let heading = trimmed.chars().take_while(|c| *c == '#').count();
        if heading > 0 && trimmed[heading..].starts_with(' ') {
            let size = match heading {
                1 => 24.0,
                2 => 19.0,
                3 => 16.0,
                _ => 14.5,
            };
            ui.add_space(if heading <= 2 { 10.0 } else { 6.0 });
            ui.label(
                egui::RichText::new(plain_inline(trimmed[heading..].trim()))
                    .size(size)
                    .strong()
                    .color(if heading <= 2 {
                        palette.amber
                    } else {
                        palette.text_primary
                    }),
            );
            if heading <= 2 {
                ui.separator();
            }
        } else if let Some(item) = trimmed
            .trim_start()
            .strip_prefix("- ")
            .or_else(|| trimmed.trim_start().strip_prefix("* "))
        {
            ui.horizontal_wrapped(|ui| {
                ui.add_space(12.0);
                ui.label(egui::RichText::new("•").color(palette.amber));
                ui.label(egui::RichText::new(plain_inline(item)).color(palette.text_primary));
            });
        } else if trimmed.trim() == "---" {
            ui.separator();
        } else if trimmed.trim().is_empty() {
            ui.add_space(4.0);
        } else {
            ui.add(
                egui::Label::new(
                    egui::RichText::new(plain_inline(trimmed)).color(palette.text_primary),
                )
                .wrap_mode(egui::TextWrapMode::Wrap),
            );
        }
    }
}
