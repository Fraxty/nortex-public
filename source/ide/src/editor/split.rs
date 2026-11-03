//! Pane-Grid des Split-Editors: reines Modell ohne UI.
//!
//! Dokumente liegen weiterhin einmalig in `Application`; Panes verweisen per Dokumentindex
//! darauf. Dieselbe Datei in mehreren Panes teilt dadurch automatisch einen Zustand.

use crate::i18n::tr;
use std::collections::HashMap;

pub const MIN_PANE_WIDTH: f32 = 250.0;
pub const MIN_RATIO: f32 = 0.2;
pub const MAX_PANES: usize = 4;
pub const EDGE_FRACTION: f32 = 0.2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitMode {
    Single,
    TwoColumns,
    TwoRows,
    FourGrid,
}

impl SplitMode {
    pub fn pane_count(self) -> usize {
        match self {
            SplitMode::Single => 1,
            SplitMode::TwoColumns | SplitMode::TwoRows => 2,
            SplitMode::FourGrid => 4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SplitMode::Single => tr("Einzeln"),
            SplitMode::TwoColumns => tr("Zwei Spalten"),
            SplitMode::TwoRows => tr("Zwei Zeilen"),
            SplitMode::FourGrid => "2x2 Raster",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropZone {
    Center,
    Left,
    Right,
    Top,
    Bottom,
}

/// Zone unter dem Mauszeiger: näher als 20 % an einer Kante = Edge-Split, sonst Zentrum.
pub fn drop_zone(size: (f32, f32), pos: (f32, f32)) -> DropZone {
    let (width, height) = (size.0.max(1.0), size.1.max(1.0));
    let (x, y) = (pos.0 / width, pos.1 / height);
    let distances = [
        (x, DropZone::Left),
        (1.0 - x, DropZone::Right),
        (y, DropZone::Top),
        (1.0 - y, DropZone::Bottom),
    ];
    let (nearest, zone) = distances
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap_or((1.0, DropZone::Center));
    if nearest < EDGE_FRACTION {
        zone
    } else {
        DropZone::Center
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ViewState {
    pub cursor: usize,
    pub scroll: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Pane {
    pub tabs: Vec<usize>,
    pub active: Option<usize>,
    pub views: HashMap<usize, ViewState>,
}

impl Pane {
    fn with_doc(doc: Option<usize>) -> Self {
        Self {
            tabs: doc.into_iter().collect(),
            active: doc,
            views: HashMap::new(),
        }
    }

    fn add(&mut self, doc: usize) {
        if !self.tabs.contains(&doc) {
            self.tabs.push(doc);
        }
        self.active = Some(doc);
    }

    fn remove(&mut self, doc: usize) {
        let Some(position) = self.tabs.iter().position(|tab| *tab == doc) else {
            return;
        };
        self.tabs.remove(position);
        self.views.remove(&doc);
        if self.active == Some(doc) {
            self.active = self
                .tabs
                .get(position.min(self.tabs.len().saturating_sub(1)))
                .copied();
        }
    }
}

/// Normalisiertes Rechteck (x0, y0, x1, y1) innerhalb der Editorfläche.
pub type Cell = (f32, f32, f32, f32);

#[derive(Debug, Clone)]
pub struct PaneGrid {
    mode: SplitMode,
    panes: Vec<Pane>,
    active_pane_index: usize,
    col_ratio: f32,
    row_ratio: f32,
}

impl Default for PaneGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl PaneGrid {
    pub fn new() -> Self {
        Self {
            mode: SplitMode::Single,
            panes: vec![Pane::default()],
            active_pane_index: 0,
            col_ratio: 0.5,
            row_ratio: 0.5,
        }
    }

    pub fn mode(&self) -> SplitMode {
        self.mode
    }

    pub fn panes(&self) -> &[Pane] {
        &self.panes
    }

    pub fn pane_mut(&mut self, index: usize) -> Option<&mut Pane> {
        self.panes.get_mut(index)
    }

    pub fn len(&self) -> usize {
        self.panes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.panes.is_empty()
    }

    pub fn active_pane_index(&self) -> usize {
        self.active_pane_index
    }

    pub fn active_doc(&self) -> Option<usize> {
        self.panes[self.active_pane_index].active
    }

    pub fn ratios(&self) -> (f32, f32) {
        (self.col_ratio, self.row_ratio)
    }

    pub fn set_ratios(&mut self, col: f32, row: f32) {
        self.col_ratio = col.clamp(MIN_RATIO, 1.0 - MIN_RATIO);
        self.row_ratio = row.clamp(MIN_RATIO, 1.0 - MIN_RATIO);
    }

    pub fn holders(&self, doc: usize) -> usize {
        self.panes
            .iter()
            .filter(|pane| pane.tabs.contains(&doc))
            .count()
    }

    pub fn focus(&mut self, index: usize) -> bool {
        if index >= self.panes.len() {
            return false;
        }
        self.active_pane_index = index;
        true
    }

    /// Öffnet `doc` im aktiven Pane (bzw. aktiviert den vorhandenen Tab dort).
    pub fn open(&mut self, doc: usize) {
        let index = self.active_pane_index;
        self.open_in(index, doc);
    }

    pub fn open_in(&mut self, pane: usize, doc: usize) {
        if let Some(target) = self.panes.get_mut(pane) {
            target.add(doc);
            self.active_pane_index = pane;
        }
    }

    pub fn set_active_tab(&mut self, pane: usize, doc: usize) {
        if let Some(target) = self.panes.get_mut(pane)
            && target.tabs.contains(&doc)
        {
            target.active = Some(doc);
        }
    }

    /// Wechselt zyklisch den Tab im aktiven Pane; liefert das neue Dokument.
    pub fn cycle_tab(&mut self, step: isize) -> Option<usize> {
        let pane = &mut self.panes[self.active_pane_index];
        if pane.tabs.is_empty() {
            return None;
        }
        let count = pane.tabs.len() as isize;
        let current = pane
            .active
            .and_then(|doc| pane.tabs.iter().position(|tab| *tab == doc))
            .unwrap_or(0) as isize;
        let next = (current + step).rem_euclid(count) as usize;
        pane.active = Some(pane.tabs[next]);
        pane.active
    }

    pub fn store_view(&mut self, doc: usize, view: ViewState) {
        self.panes[self.active_pane_index].views.insert(doc, view);
    }

    pub fn view(&self, pane: usize, doc: usize) -> ViewState {
        self.panes
            .get(pane)
            .and_then(|pane| pane.views.get(&doc))
            .copied()
            .unwrap_or_default()
    }

    /// Stellt den Modus her; neue Panes zeigen zunächst das aktive Dokument, überzählige
    /// Panes übergeben ihre Tabs an das letzte verbleibende.
    pub fn set_mode(&mut self, mode: SplitMode) {
        let target = mode.pane_count();
        let seed = self.active_doc();
        let previous_len = self.panes.len();
        while self.panes.len() > target {
            let removed = self.panes.pop().unwrap_or_default();
            if let Some(last) = self.panes.last_mut() {
                for doc in removed.tabs {
                    if !last.tabs.contains(&doc) {
                        last.tabs.push(doc);
                    }
                }
            }
        }
        while self.panes.len() < target {
            self.panes.push(Pane::with_doc(seed));
        }
        self.mode = mode;
        self.active_pane_index = if self.panes.len() > previous_len {
            previous_len
        } else {
            self.active_pane_index.min(self.panes.len() - 1)
        };
    }

    /// Schließt ein Pane; seine Tabs wandern in das Nachbar-Pane, nichts geht verloren.
    pub fn close_pane(&mut self, index: usize) -> bool {
        if self.panes.len() <= 1 || index >= self.panes.len() {
            return false;
        }
        let destination = if index > 0 { index - 1 } else { 1 };
        let removed = self.panes[index].clone();
        let keep_active = removed.active;
        {
            let target = &mut self.panes[destination];
            for doc in removed.tabs {
                if !target.tabs.contains(&doc) {
                    target.tabs.push(doc);
                }
            }
            for (doc, view) in removed.views {
                target.views.entry(doc).or_insert(view);
            }
            if keep_active.is_some() && index == self.active_pane_index {
                target.active = keep_active;
            }
        }
        let was_active = index == self.active_pane_index;
        self.remove_pane_raw(index);
        if was_active {
            self.active_pane_index = if destination > index {
                destination - 1
            } else {
                destination
            };
        }
        true
    }

    pub fn close_active_pane(&mut self) -> bool {
        self.close_pane(self.active_pane_index)
    }

    fn remove_pane_raw(&mut self, index: usize) {
        self.panes.remove(index);
        if self.active_pane_index > index {
            self.active_pane_index -= 1;
        } else if self.active_pane_index >= self.panes.len() {
            self.active_pane_index = self.panes.len() - 1;
        }
        self.mode = match (self.panes.len(), self.mode) {
            (1, _) => SplitMode::Single,
            (2, SplitMode::FourGrid) => SplitMode::TwoColumns,
            (_, mode) => mode,
        };
    }

    fn prune_empty(&mut self) {
        while self.panes.len() > 1 {
            let Some(index) = self.panes.iter().position(|pane| pane.tabs.is_empty()) else {
                break;
            };
            self.remove_pane_raw(index);
        }
    }

    /// Entfernt einen Tab aus einem Pane; ein dadurch leeres Pane schließt sich selbst.
    pub fn remove_tab(&mut self, pane: usize, doc: usize) {
        if let Some(target) = self.panes.get_mut(pane) {
            target.remove(doc);
            self.prune_empty();
        }
    }

    /// Ein Dokument wurde aus `Application` entfernt: Indizes hinter ihm rücken nach.
    pub fn document_removed(&mut self, doc: usize) {
        for pane in &mut self.panes {
            pane.remove(doc);
            for tab in &mut pane.tabs {
                if *tab > doc {
                    *tab -= 1;
                }
            }
            if let Some(active) = pane.active.as_mut()
                && *active > doc
            {
                *active -= 1;
            }
            pane.views = pane
                .views
                .drain()
                .map(|(key, value)| (if key > doc { key - 1 } else { key }, value))
                .collect();
        }
        self.prune_empty();
    }

    /// Verschiebt einen Tab innerhalb seines Panes; `insert_at` ist die Lücke in der ursprünglichen Reihenfolge.
    pub fn reorder_tab(&mut self, pane: usize, doc: usize, insert_at: usize) -> bool {
        let Some(pane) = self.panes.get_mut(pane) else {
            return false;
        };
        let Some(current) = pane.tabs.iter().position(|tab| *tab == doc) else {
            return false;
        };
        let mut target = insert_at.min(pane.tabs.len());
        if current < target {
            target -= 1;
        }
        if current == target {
            return false;
        }
        pane.tabs.remove(current);
        pane.tabs.insert(target, doc);
        true
    }

    pub fn move_tab(&mut self, doc: usize, from: usize, to: usize) -> bool {
        if from == to || from >= self.panes.len() || to >= self.panes.len() {
            return false;
        }
        if !self.panes[from].tabs.contains(&doc) {
            return false;
        }
        let view = self.panes[from].views.get(&doc).copied();
        self.panes[from].remove(doc);
        self.panes[to].add(doc);
        if let Some(view) = view {
            self.panes[to].views.insert(doc, view);
        }
        self.active_pane_index = to;
        if self.panes[from].tabs.is_empty() {
            self.remove_pane_raw(from);
        }
        true
    }

    /// Drop eines Tabs auf ein Pane: Kante teilt (nur aus Single), Zentrum verschiebt.
    pub fn drop_tab(&mut self, doc: usize, from: usize, target: usize, zone: DropZone) -> bool {
        if zone == DropZone::Center || self.panes.len() > 1 {
            return self.move_tab(doc, from, target);
        }
        if from != 0 || target != 0 || self.panes[0].tabs.len() < 2 {
            return false;
        }
        let mode = match zone {
            DropZone::Left | DropZone::Right => SplitMode::TwoColumns,
            _ => SplitMode::TwoRows,
        };
        let view = self.panes[0].views.get(&doc).copied();
        self.panes[0].remove(doc);
        let mut fresh = Pane::with_doc(Some(doc));
        if let Some(view) = view {
            fresh.views.insert(doc, view);
        }
        if matches!(zone, DropZone::Left | DropZone::Top) {
            self.panes.insert(0, fresh);
            self.active_pane_index = 0;
        } else {
            self.panes.push(fresh);
            self.active_pane_index = 1;
        }
        self.mode = mode;
        true
    }

    /// Zellen der Panes; unter `MIN_PANE_WIDTH` pro Spalte werden Panes gestapelt.
    pub fn cells(&self, width: f32) -> Vec<Cell> {
        let count = self.panes.len();
        let (col, row) = (self.col_ratio, self.row_ratio);
        let narrow = width * col.min(1.0 - col) < MIN_PANE_WIDTH;
        let stacked = |count: usize| -> Vec<Cell> {
            (0..count)
                .map(|index| {
                    (
                        0.0,
                        index as f32 / count as f32,
                        1.0,
                        (index + 1) as f32 / count as f32,
                    )
                })
                .collect()
        };
        match (count, self.mode) {
            (1, _) => vec![(0.0, 0.0, 1.0, 1.0)],
            (_, SplitMode::TwoRows) => vec![(0.0, 0.0, 1.0, row), (0.0, row, 1.0, 1.0)],
            (_, SplitMode::TwoColumns) if narrow => stacked(count),
            (_, SplitMode::TwoColumns) => vec![(0.0, 0.0, col, 1.0), (col, 0.0, 1.0, 1.0)],
            (_, _) if narrow => stacked(count),
            (3, _) => vec![
                (0.0, 0.0, col, row),
                (col, 0.0, 1.0, row),
                (0.0, row, 1.0, 1.0),
            ],
            _ => vec![
                (0.0, 0.0, col, row),
                (col, 0.0, 1.0, row),
                (0.0, row, col, 1.0),
                (col, row, 1.0, 1.0),
            ],
        }
    }

    pub fn focus_number(&mut self, number: usize) -> bool {
        number.checked_sub(1).is_some_and(|index| self.focus(index))
    }

    /// Fokus ins räumlich benachbarte Pane (nächste Zelle in Pfeilrichtung mit Überlappung).
    pub fn focus_direction(&mut self, direction: Direction, width: f32) -> bool {
        let cells = self.cells(width);
        let Some(&(x0, y0, x1, y1)) = cells.get(self.active_pane_index) else {
            return false;
        };
        let eps = 1e-4;
        let best = cells
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != self.active_pane_index)
            .filter_map(|(index, &(a0, b0, a1, b1))| {
                let overlap_x = a0 < x1 - eps && a1 > x0 + eps;
                let overlap_y = b0 < y1 - eps && b1 > y0 + eps;
                let gap = match direction {
                    Direction::Left if overlap_y && a1 <= x0 + eps => x0 - a1,
                    Direction::Right if overlap_y && a0 >= x1 - eps => a0 - x1,
                    Direction::Up if overlap_x && b1 <= y0 + eps => y0 - b1,
                    Direction::Down if overlap_x && b0 >= y1 - eps => b0 - y1,
                    _ => return None,
                };
                Some((index, gap))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        match best {
            Some((index, _)) => self.focus(index),
            None => false,
        }
    }
}
