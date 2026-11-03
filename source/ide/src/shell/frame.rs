//! Frame-Schleife: Eingaben, Layout und Zeichnen aller Bereiche.

use crate::*;
use nortex_ide::i18n::tr_fmt;

impl eframe::App for NortexCodeApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_background_results();
        self.poll_marketplace_events();
        self.pump_plugin_host(context);
        let zoom_step = context.input(|input| {
            let scroll = input.raw_scroll_delta.y;
            if !input.modifiers.ctrl || scroll == 0.0 || !scroll.is_finite() {
                0.0
            } else {
                scroll.signum()
            }
        });
        if zoom_step != 0.0 {
            let mut next = self.theme;
            let stepped = ((next.zoom.ui + zoom_step * 0.1) * 10.0).round() / 10.0;
            next.zoom.ui = typography::clamp_zoom(stepped);
            if next != self.theme {
                self.change_theme(context, next);
            }
        }
        let title = self.manifest.as_ref().map_or_else(
            || "NORTEX Code".to_owned(),
            |(_, manifest)| format!("{} — NORTEX Code", manifest.package.name),
        );
        if title != self.window_title {
            context.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
        if self.run_gate.is_running()
            || self.project_refresh_running
            || self.git_refresh_running
            || self.marketplace_busy
        {
            context.request_repaint_after(Duration::from_millis(100));
        }
        if self.startup_pending {
            self.startup_pending = false;
            self.request_git_refresh();
        }
        if self.zen_mode && context.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.zen_mode = false;
        }
        self.draw_titlebar(context);
        self.draw_help_windows(context);
        self.draw_feedback_window(context);
        self.draw_account_window(context);
        self.run_auto_save(context);
        self.handle_dropped_files(context);
        if let Some(command) = self.palette.show(context, &self.theme) {
            self.run_palette_command(context, command);
        }
        if !self.palette.is_open() {
            self.expand_main_trigger(context);
            for action in context.input_mut(keymap::poll) {
                self.run_action(context, action);
            }
        }

        let size = context.screen_rect().size();
        let plan = layout::compute(layout::LayoutInput {
            window: (size.x, size.y),
            zen: self.zen_mode,
            explorer_open: self.workspace.side_open,
            sidecar_visible: self.sidecar_visible,
            sidecar_forced: self.sidecar_forced,
            console_open: self.problems_visible && self.workspace.view_mode != ViewMode::Settings,
        });
        self.sidecar_auto_collapsed = plan.sidecar_auto_collapsed;

        if plan.activity_bar {
            if let Some(action) = activity_bar::show(context, &self.workspace, &self.textures) {
                self.workspace.apply(action);
                if matches!(action, ActivityAction::Git)
                    || self.workspace.view_mode == ViewMode::Dashboard
                {
                    self.request_git_refresh();
                }
            }
        }

        if let Some(explorer) = plan.explorer {
            egui::SidePanel::left("project-explorer")
                .resizable(true)
                .default_width(explorer.default)
                .width_range(explorer.min..=explorer.max)
                .show(context, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                    let status = self.status_color();
                    match self.workspace.side_view {
                        SideView::Search => return self.draw_search_panel(ui),
                        SideView::Git => return self.draw_git_panel(ui),
                        SideView::Debugger => return self.draw_debugger_panel(ui),
                        SideView::Explorer => {}
                    }
                    panel_header(ui, &self.theme, 1, tr("Explorer"), status);
                    ui.horizontal(|ui| {
                        let field_width = (ui.available_width() - 52.0).max(60.0);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.project_path_input)
                                .desired_width(field_width),
                        );
                        if ui.button(tr("Öffnen")).clicked() {
                            self.request_project_refresh();
                        }
                    });
                    ui.separator();
                    if let Some(path) = self.draw_explorer_tree(ui) {
                        self.open_file(path);
                    }
                });
        }

        if let Some(sidecar) = plan.sidecar {
            egui::SidePanel::right("sidecar")
                .resizable(true)
                .default_width(sidecar.default)
                .width_range(sidecar.min..=sidecar.max)
                .show(context, |ui| {
                    let status = self.theme.palette().violet;
                    panel_header(ui, &self.theme, 4, "Sidecar", status);
                    ui.label(tr("KI-Assistenz"));
                    ui.separator();
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        ui.horizontal(|ui| {
                            let response = ui.text_edit_singleline(&mut self.sidecar_prompt);
                            let send = ui.button(tr("Senden")).clicked()
                                || (response.lost_focus()
                                    && ui.input(|input| input.key_pressed(egui::Key::Enter)));
                            if send && !self.sidecar_prompt.trim().is_empty() {
                                self.status = tr(
                                    "KI-Anfrage nicht gesendet: Es ist kein Anbieter konfiguriert.",
                                )
                                .into();
                            }
                        });
                    });
                    ui.label(tr("Vorschläge und Kontext erscheinen hier."));
                });
        }

        if let Some(console) = plan.console {
            egui::TopBottomPanel::bottom("output-panel")
                .resizable(true)
                .default_height(console.default)
                .height_range(console.min..=console.max)
                .show(context, |ui| {
                    let status = self.console_status_color();
                    panel_header(ui, &self.theme, 3, tr("Console"), status);
                    let palette = self.theme.palette();
                    let mut terminal_focus = false;
                    ui.horizontal(|ui| {
                        let log_label = format!("Compiler Log ({})", self.diagnostics.len());
                        ui.selectable_value(
                            &mut self.console_tab,
                            ConsoleTab::CompilerLog,
                            log_label,
                        );
                        ui.selectable_value(
                            &mut self.console_tab,
                            ConsoleTab::RuntimeOutput,
                            tr("Runtime Output"),
                        );
                        let previous_tab = self.console_tab;
                        ui.selectable_value(
                            &mut self.console_tab,
                            ConsoleTab::Terminal,
                            tr("Terminal"),
                        );
                        if self.console_tab == ConsoleTab::Terminal
                            && previous_tab != ConsoleTab::Terminal
                        {
                            terminal_focus = true;
                        }
                        if self.run_gate.is_running() {
                            ui.spinner();
                        }
                    });
                    ui.separator();
                    let font = self.theme.terminal_font();
                    match self.console_tab {
                        ConsoleTab::CompilerLog => {
                            let mut quick_fix = None;
                            egui::ScrollArea::vertical()
                                .id_salt("compiler-log")
                                .auto_shrink([false, false])
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    if !self.compiler_log.is_empty() {
                                        ui.label(
                                            egui::RichText::new(&self.compiler_log)
                                                .font(font.clone())
                                                .color(palette.text_primary),
                                        );
                                    }
                                    for diagnostic in &self.diagnostics {
                                        let color = if diagnostic.is_error {
                                            palette.red
                                        } else {
                                            palette.amber
                                        };
                                        let mut line = format!(
                                            "{} [{}:{}:{}] {}",
                                            diagnostic.code,
                                            diagnostic.file,
                                            diagnostic.line,
                                            diagnostic.column,
                                            diagnostic.message
                                        );
                                        if let Some(help) = &diagnostic.help {
                                            line.push_str(&tr_fmt(
                                                "\n  Hilfe: {}",
                                                &[help.to_string()],
                                            ));
                                        }
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                egui::RichText::new(line)
                                                    .font(font.clone())
                                                    .color(color),
                                            );
                                            let fixable = !nortex_ide::diagnostics::quick_fixes(
                                                &self.editor_text,
                                                diagnostic,
                                            )
                                            .is_empty();
                                            if fixable && ui.small_button(tr("Quick Fix")).clicked()
                                            {
                                                quick_fix = Some(diagnostic.clone());
                                            }
                                        });
                                    }
                                });
                            if let Some(diagnostic) = quick_fix {
                                self.apply_quick_fix(&diagnostic);
                            }
                        }
                        ConsoleTab::RuntimeOutput => {
                            egui::ScrollArea::vertical()
                                .id_salt("runtime-output")
                                .auto_shrink([false, false])
                                .stick_to_bottom(true)
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new(&self.runtime_output)
                                            .font(font.clone())
                                            .color(palette.text_primary),
                                    );
                                });
                        }
                        ConsoleTab::Terminal => {
                            self.draw_terminal(ui, context, terminal_focus);
                        }
                    }
                });
        }

        let status_bar_shown = !self.zen_mode && self.settings.status_bar_visible;
        egui::TopBottomPanel::bottom("status-bar").show_animated(context, status_bar_shown, |ui| {
            let cursor = egui::TextEdit::load_state(context, egui::Id::new("editor-text"))
                .and_then(|state| state.cursor.char_range())
                .map_or(0, |range| range.primary.index);
            let position = nortex_ide::ui::status_bar::position_label(&self.editor_text, cursor);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(position);
                ui.separator();
                ui.label(tr_fmt(
                    "{} Fehler",
                    &[self
                        .diagnostics
                        .iter()
                        .filter(|item| item.is_error)
                        .count()
                        .to_string()],
                ));
                ui.separator();
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    if let Some((path, manifest)) = &self.manifest {
                        ui.label(format!(
                            "{} → {}",
                            manifest.package.name,
                            manifest.output_file_name()
                        ))
                        .on_hover_text(path.display().to_string());
                        ui.separator();
                    }
                    let build_color = match self.last_run {
                        None => None,
                        Some(_) if self.diagnostics.iter().any(|item| item.is_error) => {
                            Some(egui::Color32::from_rgb(0xD6, 0x6C, 0x6C))
                        }
                        Some(_) => Some(egui::Color32::from_rgb(0x70, 0xA8, 0x89)),
                    };
                    let mut text = egui::RichText::new(&self.status);
                    if let Some(color) = build_color {
                        text = text.color(color);
                    }
                    ui.add(egui::Label::new(text).truncate());
                });
            });
        });

        egui::CentralPanel::default().show(context, |ui| {
            match self.workspace.view_mode {
                ViewMode::Dashboard => return self.draw_dashboard(ui),
                ViewMode::Settings => return self.draw_settings_view(ui, context),
                ViewMode::Extensions => return self.draw_extensions_view(ui),
                ViewMode::Editor => {}
            }
            let has_errors = self.diagnostics.iter().any(|item| item.is_error);
            let theme = self.theme;
            if !self.zen_mode {
            let editor_status = self.status_color();
            panel_header(ui, &self.theme, 2, tr("Editor"), editor_status);
            ui.horizontal(|ui| {
                if ui
                    .add(IndustrialButton::new(&theme, tr("+ Neue Datei  Strg+N")).kind(ButtonKind::Ghost))
                    .clicked()
                {
                    self.new_document();
                }
                // Feste Größe, damit Run-Button und Kompiliere-Anzeige das Layout nicht verschieben.
                let build_running = self.run_gate.is_running();
                let mut start_build = false;
                ui.allocate_ui_with_layout(
                    egui::vec2(RUN_SLOT_WIDTH, RUN_SLOT_HEIGHT),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        if build_running {
                            let amber = theme.palette().amber;
                            egui::Frame::new()
                                .stroke(egui::Stroke::new(1.0, amber))
                                .corner_radius(egui::CornerRadius::ZERO)
                                .inner_margin(egui::Margin::symmetric(10, 4))
                                .show(ui, |ui| {
                                    ui.spinner();
                                    ui.label(egui::RichText::new(tr("Kompiliere...")).color(amber));
                                });
                        } else if ui
                            .add(
                                IndustrialButton::new(&theme, tr("▶ Run  Ctrl+F5"))
                                    .kind(ButtonKind::Primary),
                            )
                            .clicked()
                        {
                            start_build = true;
                        }
                    },
                );
                if start_build {
                    self.start_build_and_run();
                }
                if ui
                    .checkbox(&mut self.settings.overflow_checks, tr("Overflow-Checks"))
                    .changed()
                {
                    self.persist_settings();
                }
                if ui
                    .add(IndustrialButton::new(&theme, tr("Speichern  Strg+S")))
                    .clicked()
                {
                    self.save_document();
                }
                ui.separator();
                let title = self
                    .documents
                    .active_document()
                    .and_then(|document| document.path())
                    .and_then(Path::file_name)
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| tr("Unbenannt").into());
                ui.strong(title);
            });
            ui.separator();
            }

            let area = ui.available_rect_before_wrap();
            self.editor_width = area.width();
            let cells = self.grid.cells(area.width());
            let pane_rects: Vec<egui::Rect> = cells
                .iter()
                .map(|&(x0, y0, x1, y1)| {
                    egui::Rect::from_min_max(
                        egui::pos2(
                            area.left() + x0 * area.width(),
                            area.top() + y0 * area.height(),
                        ),
                        egui::pos2(
                            area.left() + x1 * area.width(),
                            area.top() + y1 * area.height(),
                        ),
                    )
                })
                .collect();
            let pressed = ui.input(|input| {
                input
                    .pointer
                    .primary_pressed()
                    .then(|| input.pointer.interact_pos())
                    .flatten()
            });
            if let Some(position) = pressed
                && let Some(clicked) = pane_rects.iter().position(|rect| rect.contains(position))
            {
                self.focus_pane(clicked);
            }
            let dragged = egui::DragAndDrop::payload::<TabDrag>(ui.ctx());
            let hover = ui.input(|input| input.pointer.hover_pos());
            let released = ui.input(|input| input.pointer.any_released());
            let mut drop_request = None;
            let mut tab_events = Vec::new();
            for (pane_index, rect) in pane_rects.iter().copied().enumerate() {
                let mut pane_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(rect.shrink(1.0))
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                );
                let ui = &mut pane_ui;
                ui.set_clip_rect(rect);
                tab_events.push((pane_index, self.draw_pane_tabs(ui, pane_index, has_errors)));
                if let (Some(payload), Some(position)) = (&dragged, hover)
                    && rect.contains(position)
                    && !(payload.pane == pane_index && position.y < rect.top() + 34.0)
                {
                    let zone = if self.grid.len() > 1 {
                        DropZone::Center
                    } else {
                        split::drop_zone(
                            (rect.width(), rect.height()),
                            (position.x - rect.left(), position.y - rect.top()),
                        )
                    };
                    paint_drop_feedback(ui.ctx(), rect, zone);
                    if released {
                        drop_request = Some((payload.doc, payload.pane, pane_index, zone));
                    }
                }
                if self.grid.active_pane_index() != pane_index
                    || self.grid.panes()[pane_index].active.is_none()
                {
                    self.draw_pane_preview(ui, pane_index);
                    continue;
                }
                let crumb_label = self.breadcrumb_file_label();
                let crumb_path = self.breadcrumb_path();
                if let Some(target) = breadcrumbs::show(ui, &crumb_label, &crumb_path) {
                    self.jump_to_line(target);
                }
                let restore_view = self.pending_restore.take();
                let editor_palette = self.theme.editor_palette();
                let editor_font = self.theme.editor_font();
                let line_height = self.theme.editor_line_height();
                let grey = self.theme.palette().grey;
                let active_file = self.active_file_label();
                let highlight_spans = &self.highlight_spans;
                let mut layouter = |ui: &egui::Ui, text: &str, _wrap_width: f32| {
                    let job = syntax_layout(
                        text,
                        highlight_spans,
                        &editor_palette,
                        &editor_font,
                        line_height,
                    );
                    ui.fonts(|fonts| fonts.layout_job(job))
                };
                let rows = line_count(&self.editor_text);
                if self.find.open {
                    self.find.refresh(&self.editor_text);
                }
                let find_scroll = std::mem::take(&mut self.find.scroll_to_current);
                let find_matches = &self.find.matches;
                let find_current = self.find.current;
                let breakpoint_lines = self
                    .documents
                    .active_document()
                    .and_then(|document| document.path())
                    .map(|path| self.debugger.lines_in(path))
                    .unwrap_or_default();
                let gutter = gutter_text_marked(rows, &breakpoint_lines);
                let visible_rows = (ui.available_height() / line_height).ceil().max(1.0) as usize;
                let mut changed = false;
                let scroll_area = egui::ScrollArea::vertical()
                    .id_salt("editor-scroll")
                    .auto_shrink([false, false]);
                let scroll_area = match &restore_view {
                    Some(view) => scroll_area.vertical_scroll_offset(view.scroll),
                    None => scroll_area,
                };
                let scroll_area = match self.scroll_request.take() {
                    Some(offset) => scroll_area.vertical_scroll_offset(offset),
                    None => scroll_area,
                };
                let mut restore_cursor = restore_view.map(|view| view.cursor);
                let scroll_output = scroll_area.show(ui, |ui| {
                    egui::Frame::new()
                        .fill(editor_palette.background)
                        .inner_margin(egui::Margin::symmetric(10, 8))
                        .show(ui, |ui| {
                            ui.horizontal_top(|ui| {
                                if self.settings.line_numbers_enabled {
                                    let mut gutter_job = LayoutJob::default();
                                    let mut format =
                                        TextFormat::simple(editor_font.clone(), grey);
                                    format.line_height = Some(line_height);
                                    gutter_job.append(&gutter, 0.0, format);
                                    ui.add(egui::Label::new(gutter_job).selectable(false));
                                    ui.add_space(10.0);
                                }
                                egui::ScrollArea::horizontal()
                                    .id_salt("editor-horizontal")
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        let editor_id = egui::Id::new("editor-text");
                                        let mut accepted = false;
                                        let manual_completion = ui.input_mut(|input| {
                                            input.consume_key(
                                                egui::Modifiers::COMMAND,
                                                egui::Key::Space,
                                            )
                                        });
                                        if let Some(popup) = self.completion.as_mut() {
                                            let (down, up, accept, cancel) =
                                                ui.input_mut(|input| {
                                                    let none = egui::Modifiers::NONE;
                                                    let has_items = !popup.items.is_empty();
                                                    (
                                                        has_items
                                                            && input.consume_key(
                                                                none,
                                                                egui::Key::ArrowDown,
                                                            ),
                                                        has_items
                                                            && input.consume_key(
                                                                none,
                                                                egui::Key::ArrowUp,
                                                            ),
                                                        has_items
                                                            && (input.consume_key(
                                                                none,
                                                                egui::Key::Tab,
                                                            ) | input.consume_key(
                                                                none,
                                                                egui::Key::Enter,
                                                            )),
                                                        input.consume_key(none, egui::Key::Escape),
                                                    )
                                                });
                                            if down {
                                                popup.step(1);
                                            }
                                            if up {
                                                popup.step(-1);
                                            }
                                            if accept {
                                                if let Some((text, cursor)) =
                                                    popup.accept(&self.editor_text)
                                                {
                                                    self.editor_text = text;
                                                    let mut state = egui::TextEdit::load_state(
                                                        ui.ctx(),
                                                        editor_id,
                                                    )
                                                    .unwrap_or_default();
                                                    state.cursor.set_char_range(Some(
                                                        egui::text::CCursorRange::one(
                                                            egui::text::CCursor::new(cursor),
                                                        ),
                                                    ));
                                                    state.store(ui.ctx(), editor_id);
                                                    accepted = true;
                                                }
                                            }
                                            if accept || cancel {
                                                self.completion = None;
                                            }
                                        }
                                        if let Some(cursor) = restore_cursor.take() {
                                            let mut state =
                                                egui::TextEdit::load_state(ui.ctx(), editor_id)
                                                    .unwrap_or_default();
                                            let index =
                                                cursor.min(self.editor_text.chars().count());
                                            state.cursor.set_char_range(Some(
                                                egui::text::CCursorRange::one(
                                                    egui::text::CCursor::new(index),
                                                ),
                                            ));
                                            state.store(ui.ctx(), editor_id);
                                            ui.memory_mut(|memory| memory.request_focus(editor_id));
                                        }
                                        let output =
                                            egui::TextEdit::multiline(&mut self.editor_text)
                                                .id(editor_id)
                                                .font(egui::TextStyle::Monospace)
                                                .frame(false)
                                                .margin(egui::Margin::ZERO)
                                                .desired_width(f32::INFINITY)
                                                .desired_rows(visible_rows.max(rows))
                                                .code_editor()
                                                .layouter(&mut layouter)
                                                .show(ui);
                                        changed = output.response.changed() || accepted;
                                        let cursor = output
                                            .cursor_range
                                            .map(|range| range.primary.ccursor.index);
                                        if let Some(index) = cursor {
                                            self.live_cursor = index;
                                        }
                                        if std::mem::take(&mut self.rename_requested) {
                                            match refactor::identifier_at(
                                                &self.editor_text,
                                                self.live_cursor,
                                            ) {
                                                Some(name) => {
                                                    let local = output
                                                        .galley
                                                        .pos_from_ccursor(egui::text::CCursor::new(
                                                            self.live_cursor,
                                                        ))
                                                        .left_bottom();
                                                    let pos = output.galley_pos + local.to_vec2();
                                                    self.rename.open(name, (pos.x, pos.y + 4.0));
                                                }
                                                None => {
                                                    self.status =
                                                        tr("F2: Cursor steht nicht auf einem Bezeichner.")
                                                            .into();
                                                }
                                            }
                                        }
                                        let requested = if output.response.changed() && !accepted {
                                            self.completion = None;
                                            Some(false)
                                        } else if manual_completion {
                                            Some(true)
                                        } else {
                                            if let Some(popup) = &self.completion
                                                && (cursor != Some(popup.anchor)
                                                    || !output.response.has_focus())
                                            {
                                                self.completion = None;
                                            }
                                            None
                                        };
                                        if let (Some(manual), Some(index), Some(worker)) =
                                            (requested, cursor, self.completion_worker.as_mut())
                                        {
                                            NortexCodeApp::request_completion(
                                                worker,
                                                &self.documents,
                                                &self.editor_text,
                                                &self.source_files,
                                                index,
                                                manual,
                                            );
                                        }
                                        let own_diagnostics: Vec<_> = self
                                            .diagnostics
                                            .iter()
                                            .filter(|item| item.file == active_file)
                                            .cloned()
                                            .collect();
                                        let squiggles = if own_diagnostics.is_empty()
                                            && self.build_squiggles.is_empty()
                                        {
                                            Vec::new()
                                        } else {
                                            inline::merge(
                                                &own_diagnostics,
                                                &self.build_squiggles,
                                                &self.editor_text,
                                            )
                                        };
                                        paint_find_matches(
                                            ui,
                                            &output,
                                            find_matches,
                                            find_current,
                                            find_scroll,
                                        );
                                        paint_squiggles(ui, &output, &self.editor_text, &squiggles);
                                        if let Some(popup) = &self.completion {
                                            paint_completion(
                                                ui,
                                                &output,
                                                popup,
                                                &editor_palette,
                                                &editor_font,
                                            );
                                        }
                                    });
                            });
                        });
                });
                if self.settings.minimap_enabled {
                    if let Some(offset) = minimap::show(
                        ui,
                        scroll_output.inner_rect,
                        &self.editor_text,
                        scroll_output.state.offset.y,
                        scroll_output.content_size.y,
                        editor_palette.line_number.gamma_multiply(0.9),
                        editor_palette.keyword,
                    ) {
                        self.scroll_request = Some(offset);
                        ui.ctx().request_repaint();
                    }
                }
                if self.rename.is_active
                    && let Some(text) = self.show_rename_widget(ui.ctx())
                {
                    self.editor_text = text;
                    changed = true;
                }
                if changed {
                    if let Some(document) = self.documents.active_document_mut() {
                        sync_buffer(document.buffer_mut(), &self.synced_text, &self.editor_text);
                    }
                    self.synced_text.clone_from(&self.editor_text);
                    self.highlight_spans = highlight_nortex(&self.editor_text);
                    self.schedule_analysis();
                }
                if self.find.open {
                    let top_right = scroll_output.inner_rect.right_top() + egui::vec2(-18.0, 6.0);
                    let event = find_bar::show(ui.ctx(), &mut self.find, top_right);
                    self.apply_find_event(ui.ctx(), event);
                }
                if let Some(doc) = self.grid.active_doc() {
                    let scroll = scroll_output.state.offset.y;
                    self.grid.store_view(
                        doc,
                        ViewState {
                            cursor: self.live_cursor,
                            scroll,
                        },
                    );
                }
            }

            self.draw_split_chrome(ui, area, &pane_rects);
            if let Some((doc, from, target, zone)) = drop_request {
                egui::DragAndDrop::clear_payload(ui.ctx());
                self.drop_tab(doc, from, target, zone);
            }
            for (pane, events) in tab_events {
                self.apply_tab_events(pane, events);
            }
        });
    }
}
