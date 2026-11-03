use eframe::egui::{Key, Modifiers};
use nortex_ide::editor::lines;
use nortex_ide::editor::split::{
    Direction, DropZone, MIN_PANE_WIDTH, PaneGrid, SplitMode, ViewState, drop_zone,
};
use nortex_ide::ui::keymap::{Action, dispatch};

fn ctrl() -> Modifiers {
    Modifiers::COMMAND
}

fn grid_with(docs: &[usize]) -> PaneGrid {
    let mut grid = PaneGrid::new();
    for doc in docs {
        grid.open(*doc);
    }
    grid
}

#[test]
fn split_modes_create_and_collapse_panes() {
    let mut grid = grid_with(&[0, 1]);
    assert_eq!((grid.mode(), grid.len()), (SplitMode::Single, 1));
    grid.set_mode(SplitMode::TwoColumns);
    assert_eq!(grid.len(), 2);
    assert_eq!(grid.active_pane_index(), 1);
    assert_eq!(grid.panes()[1].tabs, vec![1]);
    grid.set_mode(SplitMode::FourGrid);
    assert_eq!(grid.len(), 4);
    grid.set_mode(SplitMode::TwoRows);
    assert_eq!(grid.len(), 2);
    grid.set_mode(SplitMode::Single);
    assert_eq!(grid.len(), 1);
    assert_eq!(grid.panes()[0].tabs.len(), 2);
}

#[test]
fn horizontal_and_vertical_cells_cover_the_area() {
    let mut grid = grid_with(&[0]);
    grid.set_mode(SplitMode::TwoColumns);
    assert_eq!(
        grid.cells(1200.0),
        vec![(0.0, 0.0, 0.5, 1.0), (0.5, 0.0, 1.0, 1.0)]
    );
    grid.set_mode(SplitMode::TwoRows);
    assert_eq!(
        grid.cells(1200.0),
        vec![(0.0, 0.0, 1.0, 0.5), (0.0, 0.5, 1.0, 1.0)]
    );
    grid.set_mode(SplitMode::FourGrid);
    assert_eq!(grid.cells(1200.0).len(), 4);
}

#[test]
fn narrow_windows_stack_panes_instead_of_squeezing() {
    let mut grid = grid_with(&[0]);
    grid.set_mode(SplitMode::TwoColumns);
    let width = MIN_PANE_WIDTH * 1.5;
    for (x0, _, x1, _) in grid.cells(width) {
        assert!((x1 - x0) * width >= MIN_PANE_WIDTH);
    }
    grid.set_mode(SplitMode::FourGrid);
    for (x0, _, x1, _) in grid.cells(400.0) {
        assert!((x1 - x0) * 400.0 >= MIN_PANE_WIDTH);
    }
}

#[test]
fn ratios_are_clamped_so_no_pane_disappears() {
    let mut grid = PaneGrid::new();
    grid.set_ratios(0.0, 5.0);
    assert_eq!(grid.ratios(), (0.2, 0.8));
}

#[test]
fn focus_by_number_and_direction() {
    let mut grid = grid_with(&[0]);
    grid.set_mode(SplitMode::FourGrid);
    assert!(grid.focus_number(1));
    assert!(!grid.focus_number(5));
    assert!(!grid.focus_number(0));
    assert!(grid.focus_direction(Direction::Right, 1600.0));
    assert_eq!(grid.active_pane_index(), 1);
    assert!(grid.focus_direction(Direction::Down, 1600.0));
    assert_eq!(grid.active_pane_index(), 3);
    assert!(grid.focus_direction(Direction::Left, 1600.0));
    assert_eq!(grid.active_pane_index(), 2);
    assert!(!grid.focus_direction(Direction::Left, 1600.0));
    assert!(grid.focus_direction(Direction::Up, 1600.0));
    assert_eq!(grid.active_pane_index(), 0);
}

#[test]
fn tabs_are_independent_per_pane() {
    let mut grid = grid_with(&[0, 1]);
    grid.set_mode(SplitMode::TwoColumns);
    grid.open_in(0, 2);
    assert_eq!(grid.panes()[0].tabs, vec![0, 1, 2]);
    assert_eq!(grid.panes()[1].tabs, vec![1]);
    assert_eq!(grid.holders(1), 2);
    grid.store_view(
        2,
        ViewState {
            cursor: 7,
            scroll: 30.0,
        },
    );
    assert_eq!(grid.view(0, 2).cursor, 7);
    assert_eq!(grid.view(1, 2), ViewState::default());
    grid.focus(1);
    assert_eq!(grid.active_doc(), Some(1));
    assert_eq!(grid.cycle_tab(1), Some(1));
}

#[test]
fn moving_last_tab_closes_the_empty_pane() {
    let mut grid = grid_with(&[0, 1]);
    grid.set_mode(SplitMode::TwoColumns);
    grid.remove_tab(0, 0);
    assert_eq!(grid.panes()[0].tabs, vec![1]);
    assert!(grid.move_tab(1, 1, 0));
    assert_eq!((grid.len(), grid.mode()), (1, SplitMode::Single));
    assert_eq!(grid.panes()[0].tabs, vec![1]);
    assert_eq!(grid.active_pane_index(), 0);
}

#[test]
fn moving_tab_between_panes_keeps_both_alive() {
    let mut grid = grid_with(&[0, 1]);
    grid.set_mode(SplitMode::TwoColumns);
    grid.open_in(0, 0);
    assert!(grid.move_tab(0, 0, 1));
    assert_eq!(grid.panes()[0].tabs, vec![1]);
    assert_eq!(grid.panes()[1].tabs, vec![1, 0]);
    assert_eq!(grid.active_pane_index(), 1);
    assert!(!grid.move_tab(0, 1, 1));
}

#[test]
fn reorder_moves_tab_within_pane() {
    let mut grid = grid_with(&[0, 1, 2]);
    assert!(grid.reorder_tab(0, 0, 3));
    assert_eq!(grid.panes()[0].tabs, vec![1, 2, 0]);
    assert!(grid.reorder_tab(0, 2, 0));
    assert_eq!(grid.panes()[0].tabs, vec![2, 1, 0]);
    assert!(!grid.reorder_tab(0, 1, 1));
    assert!(!grid.reorder_tab(0, 1, 2));
    assert!(!grid.reorder_tab(5, 1, 0));
}

#[test]
fn edge_drop_splits_only_with_spare_tabs() {
    let mut grid = grid_with(&[0]);
    assert!(!grid.drop_tab(0, 0, 0, DropZone::Right));
    assert_eq!(grid.len(), 1);
    grid.open(1);
    assert!(grid.drop_tab(1, 0, 0, DropZone::Left));
    assert_eq!((grid.len(), grid.mode()), (2, SplitMode::TwoColumns));
    assert_eq!(grid.panes()[0].tabs, vec![1]);
    assert_eq!(grid.panes()[1].tabs, vec![0]);
    let mut rows = grid_with(&[0, 1]);
    assert!(rows.drop_tab(0, 0, 0, DropZone::Bottom));
    assert_eq!(rows.mode(), SplitMode::TwoRows);
}

#[test]
fn drop_zone_uses_twenty_percent_edges() {
    let size = (1000.0, 500.0);
    assert_eq!(drop_zone(size, (100.0, 250.0)), DropZone::Left);
    assert_eq!(drop_zone(size, (950.0, 250.0)), DropZone::Right);
    assert_eq!(drop_zone(size, (500.0, 40.0)), DropZone::Top);
    assert_eq!(drop_zone(size, (500.0, 480.0)), DropZone::Bottom);
    assert_eq!(drop_zone(size, (500.0, 250.0)), DropZone::Center);
}

#[test]
fn closing_a_pane_keeps_its_open_files() {
    let mut grid = grid_with(&[0]);
    grid.set_mode(SplitMode::TwoColumns);
    grid.open_in(1, 5);
    assert!(grid.close_active_pane());
    assert_eq!(grid.len(), 1);
    assert!(grid.panes()[0].tabs.contains(&5));
    assert_eq!(grid.active_doc(), Some(5));
    assert!(!grid.close_active_pane());
}

#[test]
fn four_grid_closes_down_to_three_then_two_then_single() {
    let mut grid = grid_with(&[0]);
    grid.set_mode(SplitMode::FourGrid);
    assert!(grid.close_pane(3));
    assert_eq!((grid.len(), grid.mode()), (3, SplitMode::FourGrid));
    assert_eq!(grid.cells(1600.0).len(), 3);
    assert!(grid.close_pane(2));
    assert_eq!((grid.len(), grid.mode()), (2, SplitMode::TwoColumns));
    assert!(grid.close_pane(0));
    assert_eq!((grid.len(), grid.mode()), (1, SplitMode::Single));
}

#[test]
fn removed_documents_shift_indices_and_prune_panes() {
    let mut grid = grid_with(&[0, 1, 2]);
    grid.set_mode(SplitMode::TwoColumns);
    grid.remove_tab(1, 1);
    grid.remove_tab(1, 2);
    assert_eq!(grid.len(), 1);
    grid.document_removed(0);
    assert_eq!(grid.panes()[0].tabs, vec![0, 1]);
    assert_eq!(grid.active_doc().map(|d| d <= 1), Some(true));
}

#[test]
fn keymap_dispatches_vs_code_defaults() {
    let shift = Modifiers::COMMAND | Modifiers::SHIFT;
    assert_eq!(dispatch(shift, Key::B), Some(Action::Build));
    assert_eq!(dispatch(ctrl(), Key::B), Some(Action::ToggleSidebar));
    assert_eq!(dispatch(ctrl(), Key::J), Some(Action::TogglePanel));
    assert_eq!(dispatch(ctrl(), Key::F5), Some(Action::BuildAndRun));
    assert_eq!(
        dispatch(Modifiers::NONE, Key::F5),
        Some(Action::StartDebugging)
    );
    assert_eq!(
        dispatch(Modifiers::SHIFT, Key::F5),
        Some(Action::StopExecution)
    );
    assert_eq!(
        dispatch(Modifiers::NONE, Key::F1),
        Some(Action::CommandPalette)
    );
    assert_eq!(dispatch(shift, Key::P), Some(Action::CommandPalette));
    for (key, index) in [
        (Key::Num1, 0),
        (Key::Num2, 1),
        (Key::Num3, 2),
        (Key::Num4, 3),
    ] {
        assert_eq!(dispatch(ctrl(), key), Some(Action::FocusPane(index)));
    }
    assert_eq!(
        dispatch(ctrl(), Key::Backslash),
        Some(Action::Split(SplitMode::TwoColumns))
    );
    assert_eq!(
        dispatch(shift, Key::Backslash),
        Some(Action::Split(SplitMode::TwoRows))
    );
    assert_eq!(
        dispatch(shift, Key::Num4),
        Some(Action::Split(SplitMode::FourGrid))
    );
    assert_eq!(dispatch(shift, Key::W), Some(Action::ClosePane));
    assert_eq!(
        dispatch(Modifiers::ALT, Key::ArrowLeft),
        Some(Action::FocusDirection(Direction::Left))
    );
    assert_eq!(
        dispatch(Modifiers::ALT, Key::ArrowUp),
        Some(Action::MoveLineUp)
    );
    assert_eq!(
        dispatch(Modifiers::COMMAND | Modifiers::ALT, Key::ArrowDown),
        Some(Action::FocusDirection(Direction::Down))
    );
    assert_eq!(
        dispatch(Modifiers::SHIFT | Modifiers::ALT, Key::ArrowDown),
        Some(Action::DuplicateLine)
    );
    assert_eq!(dispatch(ctrl(), Key::A), None);
    assert_eq!(dispatch(Modifiers::NONE, Key::B), None);
}

#[test]
fn comment_toggle_round_trips() {
    let source = "a\n  b\n\nc";
    let on = lines::toggle_comment(source, 0, source.chars().count());
    assert_eq!(on.text, "// a\n//   b\n\n// c");
    let off = lines::toggle_comment(&on.text, 0, on.text.chars().count());
    assert_eq!(off.text, source);
}

#[test]
fn lines_move_and_duplicate() {
    let moved = lines::move_lines("one\ntwo\nthree", 4, 4, true).unwrap();
    assert_eq!(moved.text, "two\none\nthree");
    assert_eq!(moved.start, 0);
    let down = lines::move_lines("one\ntwo\nthree", 0, 0, false).unwrap();
    assert_eq!(down.text, "two\none\nthree");
    assert_eq!(down.start, 4);
    assert!(lines::move_lines("one", 0, 0, true).is_none());
    assert!(lines::move_lines("one\ntwo", 4, 4, false).is_none());
    let dup = lines::duplicate_lines("ab\ncd", 1, 1);
    assert_eq!(dup.text, "ab\nab\ncd");
    assert_eq!(dup.start, 4);
}

#[test]
fn next_occurrence_selects_word_then_cycles() {
    let text = "foo bar foo";
    assert_eq!(lines::next_occurrence(text, 1, 1), Some((0, 3)));
    assert_eq!(lines::next_occurrence(text, 0, 3), Some((8, 11)));
    assert_eq!(lines::next_occurrence(text, 8, 11), Some((0, 3)));
    assert_eq!(lines::next_occurrence(" ", 0, 0), None);
}
