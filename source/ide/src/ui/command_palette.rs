//! Command palette: filterable command list opened with Ctrl+Shift+P or F1.

use crate::i18n::tr;
use eframe::egui::{self, Align2, CornerRadius, Key, Modifiers, Order, Sense, Stroke, vec2};

use super::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    BuildAndRun,
    ToggleTheme,
    ClearConsole,
    ToggleZenMode,
    OpenFile,
    SaveFile,
    SplitColumns,
    SplitRows,
    SplitGrid,
    SplitSingle,
    ClosePane,
    ToggleMinimap,
}

impl Command {
    pub const ALL: [Command; 12] = [
        Command::BuildAndRun,
        Command::ToggleTheme,
        Command::ClearConsole,
        Command::ToggleZenMode,
        Command::OpenFile,
        Command::SaveFile,
        Command::SplitColumns,
        Command::SplitRows,
        Command::SplitGrid,
        Command::SplitSingle,
        Command::ClosePane,
        Command::ToggleMinimap,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Command::BuildAndRun => tr("Build & Run"),
            Command::ToggleTheme => tr("Toggle Theme"),
            Command::ClearConsole => tr("Clear Console"),
            Command::ToggleZenMode => tr("Toggle Zen Mode"),
            Command::OpenFile => tr("Open File"),
            Command::SaveFile => tr("Save File"),
            Command::SplitColumns => tr("Split Editor Columns"),
            Command::SplitRows => tr("Split Editor Rows"),
            Command::SplitGrid => tr("Split Editor 2x2 Grid"),
            Command::SplitSingle => tr("Editor Single Layout"),
            Command::ClosePane => tr("Close Editor Pane"),
            Command::ToggleMinimap => tr("View: Toggle Minimap"),
        }
    }

    pub fn shortcut(self) -> &'static str {
        match self {
            Command::BuildAndRun => "Ctrl+F5",
            Command::SaveFile => "Ctrl+S",
            Command::SplitColumns => "Ctrl+\\",
            Command::SplitRows => "Ctrl+Shift+\\",
            Command::SplitGrid => "Ctrl+Shift+4",
            Command::ClosePane => "Ctrl+Shift+W",
            Command::ToggleMinimap => "Ctrl+Alt+M",
            _ => "",
        }
    }
}

/// Case-insensitive match score; `None` if `query` is neither a substring nor a subsequence.
pub fn match_score(query: &str, title: &str) -> Option<i32> {
    let query: Vec<char> = query.trim().to_lowercase().chars().collect();
    if query.is_empty() {
        return Some(0);
    }
    let title: Vec<char> = title.to_lowercase().chars().collect();
    if let Some(start) = title.windows(query.len()).position(|w| w == query) {
        return Some(1000 - start as i32);
    }
    let mut next = 0;
    let mut gaps = 0;
    let mut last: Option<usize> = None;
    for (index, ch) in title.iter().enumerate() {
        if next < query.len() && *ch == query[next] {
            if let Some(previous) = last {
                gaps += (index - previous - 1) as i32;
            }
            last = Some(index);
            next += 1;
        }
    }
    (next == query.len()).then_some(500 - gaps)
}

/// Commands matching `query`, best match first; ties keep registry order.
pub fn filter_commands(query: &str) -> Vec<Command> {
    let mut scored: Vec<(i32, usize, Command)> = Command::ALL
        .iter()
        .enumerate()
        .filter_map(|(order, command)| {
            match_score(query, command.title()).map(|score| (score, order, *command))
        })
        .collect();
    scored.sort_by_key(|(score, order, _)| (-*score, *order));
    scored.into_iter().map(|(_, _, command)| command).collect()
}

#[derive(Debug, Default)]
pub struct CommandPalette {
    open: bool,
    query: String,
    selected: usize,
}

impl CommandPalette {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn toggle(&mut self) {
        if self.open {
            self.close();
        } else {
            self.open = true;
            self.query.clear();
            self.selected = 0;
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Handles the open shortcut and draws the overlay; returns the command to run, if any.
    pub fn show(&mut self, context: &egui::Context, theme: &Theme) -> Option<Command> {
        let open_shortcut = context.input_mut(|input| {
            input.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::P)
                | input.consume_key(Modifiers::NONE, Key::F1)
        });
        if open_shortcut {
            self.toggle();
        }
        if !self.open {
            return None;
        }

        let (escape, up, down, enter) = context.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::Escape),
                input.consume_key(Modifiers::NONE, Key::ArrowUp),
                input.consume_key(Modifiers::NONE, Key::ArrowDown),
                input.consume_key(Modifiers::NONE, Key::Enter),
            )
        });
        if escape {
            self.close();
            return None;
        }

        let matches = filter_commands(&self.query);
        let last = matches.len().saturating_sub(1);
        self.selected = self.selected.min(last);
        let moved = up || down;
        if up {
            self.selected = self.selected.saturating_sub(1);
        }
        if down {
            self.selected = (self.selected + 1).min(last);
        }
        if enter && let Some(command) = matches.get(self.selected).copied() {
            self.close();
            return Some(command);
        }

        let palette = theme.palette();
        let mut chosen = None;
        let area = egui::Area::new(egui::Id::new("command-palette"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_TOP, vec2(0.0, 90.0))
            .show(context, |ui| {
                egui::Frame::new()
                    .fill(super::find_bar::BAR_BG)
                    .stroke(Stroke::new(1.0, super::find_bar::BAR_BORDER))
                    .corner_radius(CornerRadius::ZERO)
                    .inner_margin(egui::Margin::symmetric(10, 6))
                    .show(ui, |ui| {
                        ui.set_width(460.0);
                        let input = ui.add(
                            egui::TextEdit::singleline(&mut self.query)
                                .hint_text(tr("Befehl eingeben …"))
                                .desired_width(f32::INFINITY),
                        );
                        input.request_focus();
                        if input.changed() {
                            self.selected = 0;
                        }
                        let bar = ui.cursor().min;
                        ui.painter().rect_filled(
                            egui::Rect::from_min_size(bar, vec2(460.0, 2.0)),
                            CornerRadius::ZERO,
                            palette.amber,
                        );
                        ui.add_space(6.0);

                        let matches = filter_commands(&self.query);
                        if matches.is_empty() {
                            ui.colored_label(palette.text_muted, tr("Keine passenden Befehle"));
                        }
                        egui::ScrollArea::vertical()
                            .id_salt("command-palette-list")
                            .max_height(260.0)
                            .show(ui, |ui| {
                                for (index, command) in matches.iter().enumerate() {
                                    let (rect, response) =
                                        ui.allocate_exact_size(vec2(460.0, 26.0), Sense::click());
                                    let selected = index == self.selected;
                                    if selected {
                                        ui.painter().rect_filled(
                                            rect,
                                            CornerRadius::ZERO,
                                            palette.interactive,
                                        );
                                        ui.painter().rect_filled(
                                            egui::Rect::from_min_size(rect.min, vec2(3.0, 26.0)),
                                            CornerRadius::ZERO,
                                            palette.amber,
                                        );
                                        if moved {
                                            response.scroll_to_me(None);
                                        }
                                    } else if response.hovered() {
                                        ui.painter().rect_filled(
                                            rect,
                                            CornerRadius::ZERO,
                                            palette.surface,
                                        );
                                    }
                                    let font = theme.terminal_font();
                                    ui.painter().text(
                                        rect.left_center() + vec2(12.0, 0.0),
                                        Align2::LEFT_CENTER,
                                        command.title(),
                                        font.clone(),
                                        palette.text_primary,
                                    );
                                    ui.painter().text(
                                        rect.right_center() - vec2(10.0, 0.0),
                                        Align2::RIGHT_CENTER,
                                        command.shortcut(),
                                        font,
                                        palette.text_muted,
                                    );
                                    if response.clicked() {
                                        chosen = Some(*command);
                                    }
                                }
                            });
                    });
            });

        let clicked_outside = context.input(|input| {
            input.pointer.any_pressed()
                && input
                    .pointer
                    .interact_pos()
                    .is_some_and(|pos| !area.response.rect.contains(pos))
        });
        if chosen.is_some() || clicked_outside {
            self.close();
        }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_lists_every_command_in_registry_order() {
        assert_eq!(filter_commands(""), Command::ALL.to_vec());
    }

    #[test]
    fn substring_match_is_case_insensitive() {
        assert_eq!(filter_commands("CONSOLE"), vec![Command::ClearConsole]);
    }

    #[test]
    fn fuzzy_subsequence_matches_and_rejects() {
        assert!(filter_commands("tgth").contains(&Command::ToggleTheme));
        assert!(filter_commands("xyz").is_empty());
    }

    #[test]
    fn substring_ranks_before_subsequence() {
        let ranked = filter_commands("save");
        assert_eq!(ranked.first(), Some(&Command::SaveFile));
    }

    #[test]
    fn prefix_ranks_before_later_substring() {
        let ranked = filter_commands("to");
        assert_eq!(ranked[0], Command::ToggleTheme);
        assert_eq!(ranked[1], Command::ToggleZenMode);
    }

    fn frame(
        context: &egui::Context,
        palette: &mut CommandPalette,
        key: Option<(Key, Modifiers)>,
    ) -> Option<Command> {
        let mut input = egui::RawInput::default();
        if let Some((key, modifiers)) = key {
            input.modifiers = modifiers;
            input.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            });
        }
        let mut result = None;
        let _ = context.run(input, |context| {
            result = palette.show(context, &Theme::default());
        });
        result
    }

    #[test]
    fn keyboard_flow_opens_navigates_confirms_and_cancels() {
        let context = egui::Context::default();
        let mut palette = CommandPalette::default();

        frame(&context, &mut palette, Some((Key::F1, Modifiers::NONE)));
        assert!(palette.is_open());
        frame(&context, &mut palette, Some((Key::Escape, Modifiers::NONE)));
        assert!(!palette.is_open());

        let shortcut = Modifiers::COMMAND | Modifiers::SHIFT;
        frame(&context, &mut palette, Some((Key::P, shortcut)));
        assert!(palette.is_open());

        frame(
            &context,
            &mut palette,
            Some((Key::ArrowDown, Modifiers::NONE)),
        );
        let chosen = frame(&context, &mut palette, Some((Key::Enter, Modifiers::NONE)));
        assert_eq!(chosen, Some(Command::ToggleTheme));
        assert!(!palette.is_open());
    }

    fn open(context: &egui::Context, palette: &mut CommandPalette) {
        let shortcut = Modifiers::COMMAND | Modifiers::SHIFT;
        frame(context, palette, Some((Key::P, shortcut)));
        assert!(palette.is_open());
    }

    #[test]
    fn arrow_keys_stop_at_both_ends_of_the_list() {
        let context = egui::Context::default();
        let mut palette = CommandPalette::default();
        open(&context, &mut palette);
        for _ in 0..3 {
            frame(
                &context,
                &mut palette,
                Some((Key::ArrowUp, Modifiers::NONE)),
            );
        }
        let first = frame(&context, &mut palette, Some((Key::Enter, Modifiers::NONE)));
        assert_eq!(first, filter_commands("").first().copied());

        open(&context, &mut palette);
        for _ in 0..50 {
            frame(
                &context,
                &mut palette,
                Some((Key::ArrowDown, Modifiers::NONE)),
            );
        }
        let last = frame(&context, &mut palette, Some((Key::Enter, Modifiers::NONE)));
        assert_eq!(last, filter_commands("").last().copied());
    }

    #[test]
    fn typed_filter_selects_theme_toggle_and_no_match_returns_nothing() {
        let context = egui::Context::default();
        let mut palette = CommandPalette::default();
        open(&context, &mut palette);
        type_text(&context, &mut palette, "thme");
        let chosen = frame(&context, &mut palette, Some((Key::Enter, Modifiers::NONE)));
        assert_eq!(chosen, Some(Command::ToggleTheme));

        open(&context, &mut palette);
        type_text(&context, &mut palette, "zzzzqqq");
        let chosen = frame(&context, &mut palette, Some((Key::Enter, Modifiers::NONE)));
        assert_eq!(chosen, None);
    }

    fn type_text(context: &egui::Context, palette: &mut CommandPalette, text: &str) {
        for _ in 0..2 {
            let input = egui::RawInput {
                events: vec![egui::Event::Text(text.to_string())],
                ..Default::default()
            };
            let _ = context.run(input, |context| {
                palette.show(context, &Theme::default());
            });
            if palette.query == text {
                return;
            }
        }
    }

    #[test]
    fn filter_with_no_match_is_empty_and_whitespace_query_lists_all() {
        assert!(filter_commands("zzzzqqq").is_empty());
        assert_eq!(filter_commands("   ").len(), filter_commands("").len());
    }
}
