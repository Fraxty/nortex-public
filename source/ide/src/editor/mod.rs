//! UI-unabhängiger Piece-Table-Textpuffer für NORTEX Code.
//!
//! Der Puffer materialisiert den gesamten Text nicht nach jeder Änderung.
//! `materialize()` ist für Speichern, Tests und APIs gedacht, die einen
//! zusammenhängenden String benötigen. Für die Darstellung einzelner Zeilen
//! sollten `line()` und `slice()` verwendet werden.

use std::io::{self, Write};
use std::ops::Range;

pub mod analysis;
pub mod completion;
pub mod completion_worker;
pub mod diagnostics;
pub mod document;
pub mod find_replace;
pub mod gutter;
pub mod highlighting;
pub mod imports;
pub mod lines;
pub mod refactor;
pub mod scope;
pub mod split;
pub mod types;

const DEFAULT_HISTORY_LIMIT: usize = 1_024;
const DEFAULT_HISTORY_BYTES_LIMIT: usize = 32 * 1024 * 1024;
const MAX_PIECES_BEFORE_COMPACT: usize = 2_048;
const ADDITIONS_COMPACT_FACTOR: usize = 4;
const GRAPHEME_WINDOW_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceBuffer {
    Original,
    Additions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Piece {
    source: SourceBuffer,
    start: usize,
    length: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Edit {
    start: usize,
    removed: String,
    inserted: String,
    cursor_before: usize,
    cursor_after: usize,
    state_before: u64,
    state_after: u64,
}

#[derive(Debug, Clone)]
pub struct EditorBuffer {
    original: String,
    additions: String,
    pieces: Vec<Piece>,
    piece_offsets: Vec<usize>,
    length: usize,
    line_starts: Vec<usize>,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    history_limit: usize,
    history_bytes_limit: usize,
    history_bytes: usize,
    state_id: u64,
    next_state_id: u64,
    cursor: usize,
    selection: Option<Range<usize>>,
}

impl EditorBuffer {
    pub fn new() -> Self {
        Self::from_text(String::new())
    }

    pub fn from_text(text: impl Into<String>) -> Self {
        let original = text.into();
        let pieces = if original.is_empty() {
            Vec::new()
        } else {
            vec![Piece {
                source: SourceBuffer::Original,
                start: 0,
                length: original.len(),
            }]
        };
        let line_starts = build_line_starts(&original);
        let piece_offsets = build_piece_offsets(&pieces);
        let length = original.len();
        let cursor = length;
        Self {
            original,
            additions: String::new(),
            pieces,
            piece_offsets,
            length,
            line_starts,
            undo: Vec::new(),
            redo: Vec::new(),
            history_limit: DEFAULT_HISTORY_LIMIT,
            history_bytes_limit: DEFAULT_HISTORY_BYTES_LIMIT,
            history_bytes: 0,
            state_id: 0,
            next_state_id: 1,
            cursor,
            selection: None,
        }
    }

    /// Fügt Text am Cursor oder ersetzt die aktuelle Auswahl.
    /// Aufeinanderfolgende Einfügungen am Cursor werden zu einem Undo-Schritt gruppiert.
    pub fn insert(&mut self, value: &str) {
        if value.is_empty() && self.selection.is_none() {
            return;
        }
        let before = self.cursor;
        let range = self.selection.clone().unwrap_or(self.cursor..self.cursor);
        let range = self.normalize_range(range);
        let removed = self.slice(range.clone());
        if removed.is_empty() && value.is_empty() {
            self.selection = None;
            return;
        }
        self.replace_raw(range.start..range.end, value);
        let edit = Edit {
            start: range.start,
            removed,
            inserted: value.to_owned(),
            cursor_before: before,
            cursor_after: range.start + value.len(),
            state_before: self.state_id,
            state_after: 0,
        };
        self.record_edit(edit, true);
        self.cursor = self.normalize_offset(range.start + value.len());
        self.selection = None;
    }

    /// Löscht einen Bytebereich. Der Anfang wird auf die vorherige, das Ende
    /// auf die nächste UTF-8-Grenze erweitert. So werden keine halben Zeichen gelöscht.
    pub fn delete(&mut self, range: Range<usize>) {
        let range = self.normalize_range(range);
        if range.start >= range.end {
            return;
        }
        let removed = self.slice(range.clone());
        let before = self.cursor;
        self.replace_raw(range.clone(), "");
        let removed_len = range.end - range.start;
        self.cursor = if before >= range.end {
            before - removed_len
        } else if before > range.start {
            range.start
        } else {
            before
        };
        self.cursor = self.normalize_offset(self.cursor);
        self.selection = None;
        self.record_edit(
            Edit {
                start: range.start,
                removed,
                inserted: String::new(),
                cursor_before: before,
                cursor_after: self.cursor,
                state_before: self.state_id,
                state_after: 0,
            },
            false,
        );
    }

    /// Replaces one UTF-8-safe byte range as a single undoable edit.
    pub fn replace_range(&mut self, range: Range<usize>, value: &str) {
        let range = self.normalize_range(range);
        let removed = self.slice(range.clone());
        if removed == value {
            return;
        }
        let cursor_before = self.cursor;
        self.replace_raw(range.clone(), value);
        let cursor_after = self.normalize_offset(range.start + value.len());
        self.record_edit(
            Edit {
                start: range.start,
                removed,
                inserted: value.to_owned(),
                cursor_before,
                cursor_after,
                state_before: self.state_id,
                state_after: 0,
            },
            false,
        );
        self.cursor = cursor_after;
        self.selection = None;
    }

    /// Löscht die aktuelle Auswahl.
    pub fn delete_selection(&mut self) -> bool {
        let Some(range) = self.selection.take() else {
            return false;
        };
        let start = range.start;
        self.delete(range);
        self.cursor = self.normalize_offset(start.min(self.len_bytes()));
        true
    }

    pub fn undo(&mut self) -> bool {
        let Some(edit) = self.undo.pop() else {
            return false;
        };
        self.replace_raw(edit.start..edit.start + edit.inserted.len(), &edit.removed);
        self.cursor = self.normalize_offset(edit.cursor_before.min(self.len_bytes()));
        self.state_id = edit.state_before;
        self.selection = None;
        self.redo.push(edit);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(edit) = self.redo.pop() else {
            return false;
        };
        self.replace_raw(edit.start..edit.start + edit.removed.len(), &edit.inserted);
        self.cursor = self.normalize_offset(edit.cursor_after.min(self.len_bytes()));
        self.state_id = edit.state_after;
        self.selection = None;
        self.undo.push(edit);
        self.trim_history();
        true
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn selection(&self) -> Option<Range<usize>> {
        self.selection.clone()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn set_history_limit(&mut self, limit: usize) {
        self.history_limit = limit.max(1);
        self.trim_history();
    }

    /// Begrenzt den zusätzlichen Speicher der Undo-/Redo-Historie.
    pub fn set_history_byte_limit(&mut self, limit: usize) {
        self.history_bytes_limit = limit.max(1);
        self.trim_history();
    }

    pub fn history_bytes(&self) -> usize {
        self.history_bytes
    }

    pub fn move_cursor(&mut self, byte_offset: usize) {
        self.cursor = self.normalize_offset(byte_offset);
        self.selection = None;
    }

    pub fn move_cursor_left(&mut self) {
        let line = self
            .line_starts
            .partition_point(|start| *start <= self.cursor)
            .saturating_sub(1);
        let line_start = self.line_starts[line];
        if self.cursor == line_start && line > 0 {
            let is_crlf = self.cursor >= 2
                && self.byte_at(self.cursor - 1) == Some(b'\n')
                && self.byte_at(self.cursor - 2) == Some(b'\r');
            self.cursor = self.cursor.saturating_sub(if is_crlf { 2 } else { 1 });
        } else {
            self.cursor = self.previous_grapheme_in_line(line_start, self.cursor);
        }
        self.selection = None;
    }

    pub fn move_cursor_right(&mut self) {
        let line = self
            .line_starts
            .partition_point(|start| *start <= self.cursor)
            .saturating_sub(1);
        let line_end = self
            .line_range(line)
            .map(|range| range.end)
            .unwrap_or(self.len_bytes());
        if self.byte_at(self.cursor) == Some(b'\r')
            && self.byte_at(self.cursor.saturating_add(1)) == Some(b'\n')
        {
            self.cursor = (self.cursor + 2).min(self.len_bytes());
        } else if self.byte_at(self.cursor) == Some(b'\n')
            || self.byte_at(self.cursor) == Some(b'\r')
        {
            self.cursor = (self.cursor + 1).min(self.len_bytes());
        } else {
            self.cursor = self.next_grapheme_in_line(self.cursor, line_end);
        }
        self.selection = None;
    }

    pub fn select(&mut self, range: Range<usize>) {
        let range = self.normalize_range(range);
        self.cursor = self.normalize_offset(range.end);
        self.selection = (range.start < range.end).then_some(range);
    }

    pub fn select_all(&mut self) {
        if self.len_bytes() == 0 {
            self.selection = None;
            self.cursor = 0;
        } else {
            self.selection = Some(0..self.len_bytes());
            self.cursor = self.len_bytes();
        }
    }

    pub fn clear_selection(&mut self) {
        self.selection = None;
    }

    pub fn selected_text(&self) -> Option<String> {
        self.selection.clone().map(|range| self.slice(range))
    }

    pub fn as_str(&self) -> String {
        self.materialize()
    }

    pub fn materialize(&self) -> String {
        let mut text = String::with_capacity(self.len_bytes());
        for piece in &self.pieces {
            let source = self.source(piece.source);
            let end = piece.start + piece.length;
            let part = source.get(piece.start..end).expect(
                "Piece-Table-Invariante verletzt: Piece zeigt auf ungültigen UTF-8-Bereich",
            );
            text.push_str(part);
        }
        text
    }

    /// Schreibt das Dokument stückweise, ohne einen zusätzlichen Gesamt-String anzulegen.
    pub(crate) fn write_to(&self, writer: &mut impl Write) -> io::Result<()> {
        for piece in &self.pieces {
            let source = self.source(piece.source);
            let end = piece.start.checked_add(piece.length).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Piece-Längenüberlauf")
            })?;
            let part = source.get(piece.start..end).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Ungültiger Piece-Table-Bereich")
            })?;
            writer.write_all(part.as_bytes())?;
        }
        Ok(())
    }

    pub fn slice(&self, range: Range<usize>) -> String {
        let range = self.normalize_range(range);
        if range.start >= range.end {
            return String::new();
        }
        let mut output = String::with_capacity(range.end - range.start);
        self.for_each_chunk(range, |chunk| output.push_str(chunk));
        output
    }

    /// Besucht die UTF-8-sicheren Stücke eines Bereichs ohne einen Zwischen-String
    /// anzulegen. Geeignet für Viewport-Rendering, Suche und Streaming-Ausgabe.
    pub fn for_each_chunk(&self, range: Range<usize>, mut visit: impl FnMut(&str)) {
        let range = self.normalize_range(range);
        if range.start >= range.end {
            return;
        }
        let first_piece = self
            .piece_offsets
            .partition_point(|offset| *offset <= range.start)
            .saturating_sub(1);
        for index in first_piece..self.pieces.len() {
            let piece = self.pieces[index];
            let logical_start = self.piece_offsets[index];
            let logical_end = self.piece_offsets[index + 1];
            if logical_start >= range.end {
                break;
            }
            let overlap_start = range.start.max(logical_start);
            let overlap_end = range.end.min(logical_end);
            if overlap_start < overlap_end {
                let source_start = piece.start + (overlap_start - logical_start);
                let source_end = source_start + (overlap_end - overlap_start);
                let part = self
                    .source(piece.source)
                    .get(source_start..source_end)
                    .expect("Piece-Table-Invariante verletzt: Chunk liegt nicht auf UTF-8-Grenzen");
                visit(part);
            }
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }

    /// Liefert eine Zeile ohne abschließendes LF-, CRLF- oder CR-Zeilenende.
    pub fn line(&self, line_index: usize) -> Option<String> {
        let range = self.line_content_range(line_index)?;
        Some(self.slice(range))
    }

    pub fn line_range(&self, line_index: usize) -> Option<Range<usize>> {
        let start = *self.line_starts.get(line_index)?;
        let end = self
            .line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or_else(|| self.len_bytes());
        Some(start..end)
    }

    /// Gibt eine nullbasierte Zeile und eine nullbasierte Unicode-Skalarspalte zurück.
    /// Der Text wird stückweise gezählt und nicht als vollständige Zeile materialisiert.
    pub fn offset_to_line_column(&self, offset: usize) -> (usize, usize) {
        let offset = self.normalize_offset(offset);
        let line = self
            .line_starts
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line];
        let column = self.count_chars(line_start..offset);
        (line, column)
    }

    /// Konvertiert nullbasierte Zeile/Spalte in einen UTF-8-sicheren Byteoffset.
    /// Die Suche läuft direkt über Pieces und legt keinen temporären Zeilen-String an.
    pub fn line_column_to_offset(&self, line: usize, column: usize) -> Option<usize> {
        let range = self.line_content_range(line)?;
        let mut remaining = column;
        let mut consumed_bytes = 0usize;
        let mut found = None;
        self.for_each_chunk(range.clone(), |chunk| {
            if found.is_some() {
                return;
            }
            for (byte_index, _) in chunk.char_indices() {
                if remaining == 0 {
                    found = Some(range.start + consumed_bytes + byte_index);
                    return;
                }
                remaining -= 1;
            }
            consumed_bytes += chunk.len();
        });
        Some(found.unwrap_or(range.end))
    }

    fn line_content_range(&self, line_index: usize) -> Option<Range<usize>> {
        let start = *self.line_starts.get(line_index)?;
        let mut end = self
            .line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(self.len_bytes());
        if end > start && self.byte_at(end - 1) == Some(b'\n') {
            end -= 1;
            if end > start && self.byte_at(end - 1) == Some(b'\r') {
                end -= 1;
            }
        } else if end > start && self.byte_at(end - 1) == Some(b'\r') {
            end -= 1;
        }
        Some(start..end)
    }

    fn count_chars(&self, range: Range<usize>) -> usize {
        let mut count = 0usize;
        self.for_each_chunk(range, |chunk| {
            count = count.saturating_add(chunk.chars().count())
        });
        count
    }

    pub fn len_bytes(&self) -> usize {
        self.length
    }

    pub fn is_empty(&self) -> bool {
        self.len_bytes() == 0
    }

    pub fn state_id(&self) -> u64 {
        self.state_id
    }

    fn source(&self, source: SourceBuffer) -> &str {
        match source {
            SourceBuffer::Original => &self.original,
            SourceBuffer::Additions => &self.additions,
        }
    }

    fn byte_at(&self, offset: usize) -> Option<u8> {
        if offset >= self.len_bytes() {
            return None;
        }
        let index = self
            .piece_offsets
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        let piece = self.pieces.get(index)?;
        let source_offset = piece.start + offset - self.piece_offsets[index];
        self.source(piece.source)
            .as_bytes()
            .get(source_offset)
            .copied()
    }

    fn is_char_boundary(&self, offset: usize) -> bool {
        if offset == self.len_bytes() {
            return true;
        }
        if offset > self.len_bytes() {
            return false;
        }
        let index = self
            .piece_offsets
            .partition_point(|start| *start <= offset)
            .saturating_sub(1);
        let Some(piece) = self.pieces.get(index) else {
            return offset == 0;
        };
        let source_offset = piece.start + offset - self.piece_offsets[index];
        self.source(piece.source).is_char_boundary(source_offset)
    }

    fn normalize_offset(&self, offset: usize) -> usize {
        let mut normalized = offset.min(self.len_bytes());
        while !self.is_char_boundary(normalized) {
            normalized -= 1;
        }
        if self.is_between_crlf(normalized) {
            normalized -= 1;
        }
        normalized
    }

    fn normalize_end_offset(&self, offset: usize) -> usize {
        let mut normalized = offset.min(self.len_bytes());
        while !self.is_char_boundary(normalized) && normalized < self.len_bytes() {
            normalized += 1;
        }
        if self.is_between_crlf(normalized) {
            normalized += 1;
        }
        normalized.min(self.len_bytes())
    }

    fn is_between_crlf(&self, offset: usize) -> bool {
        offset > 0
            && offset < self.len_bytes()
            && self.byte_at(offset - 1) == Some(b'\r')
            && self.byte_at(offset) == Some(b'\n')
    }

    fn normalize_range(&self, range: Range<usize>) -> Range<usize> {
        if range.start >= range.end {
            let point = self.normalize_offset(range.start);
            return point..point;
        }
        let start = self.normalize_offset(range.start);
        let end = self.normalize_end_offset(range.end.max(range.start));
        if start > end {
            start..start
        } else {
            start..end
        }
    }

    fn previous_grapheme_in_line(&self, line_start: usize, cursor: usize) -> usize {
        let mut window = GRAPHEME_WINDOW_BYTES
            .min(cursor.saturating_sub(line_start))
            .max(1);
        loop {
            let mut start = cursor.saturating_sub(window).max(line_start);
            while !self.is_char_boundary(start) || self.is_between_crlf(start) {
                start = start.saturating_sub(1);
            }
            let text = self.slice(start..cursor);
            let boundary = previous_grapheme_boundary(&text, text.len());
            let previous = self.char_before(start);
            let first = text.chars().next();
            let split_cluster = match (previous, first) {
                (Some('\u{200D}'), _) => true,
                (Some(_), Some(first)) if is_grapheme_extend(first) || first == '\u{200D}' => true,
                (Some(ch), Some(first))
                    if is_regional_indicator(ch as u32) && is_regional_indicator(first as u32) =>
                {
                    true
                }
                _ => false,
            };
            if start == line_start || (boundary > 0 && !split_cluster) {
                return start + boundary;
            }
            window = window.saturating_mul(2).min(cursor - line_start);
        }
    }

    fn next_grapheme_in_line(&self, cursor: usize, line_end: usize) -> usize {
        let mut window = GRAPHEME_WINDOW_BYTES
            .min(line_end.saturating_sub(cursor))
            .max(1);
        loop {
            let end = cursor.saturating_add(window).min(line_end);
            let text = self.slice(cursor..end);
            let boundary = next_grapheme_boundary(&text, 0);
            if boundary < text.len() || end == line_end {
                return cursor + boundary;
            }
            window = window.saturating_mul(2).min(line_end - cursor);
        }
    }

    fn char_before(&self, offset: usize) -> Option<char> {
        if offset == 0 {
            return None;
        }
        let mut start = offset - 1;
        while !self.is_char_boundary(start) {
            start -= 1;
        }
        self.slice(start..offset).chars().next()
    }

    fn replace_raw(&mut self, range: Range<usize>, inserted: &str) {
        // Alle Aufrufer liefern bereits UTF-8-sichere Offsets. Interne Undo-/Redo-
        // Operationen dürfen CRLF-Paare bytegenau teilen, weil zwei getrennte
        // Benutzereingaben ("\r", danach "\n") getrennt rückgängig sein müssen.
        let range = range.start.min(self.len_bytes())..range.end.min(self.len_bytes());
        debug_assert!(range.start <= range.end);
        debug_assert!(self.is_char_boundary(range.start));
        debug_assert!(self.is_char_boundary(range.end));
        let (left, tail) = split_pieces(&self.pieces, range.start);
        let (_, right_after_delete) = split_pieces(&tail, range.end - range.start);
        let insertion_start = self.additions.len();
        self.additions.push_str(inserted);
        let mut new_pieces = left;
        if !inserted.is_empty() {
            new_pieces.push(Piece {
                source: SourceBuffer::Additions,
                start: insertion_start,
                length: inserted.len(),
            });
        }
        new_pieces.extend(right_after_delete);
        self.pieces = new_pieces;
        self.merge_adjacent();
        self.length = self.length - (range.end - range.start) + inserted.len();
        self.piece_offsets = build_piece_offsets(&self.pieces);
        self.update_line_index(range.start, range.end, inserted);
        self.maybe_compact();
        self.cursor = range.start + inserted.len();
    }

    fn maybe_compact(&mut self) {
        let backing_bytes = self.original.len().saturating_add(self.additions.len());
        let live_storage_limit = self.length.max(1).saturating_mul(ADDITIONS_COMPACT_FACTOR);
        if backing_bytes <= live_storage_limit && self.pieces.len() <= MAX_PIECES_BEFORE_COMPACT {
            return;
        }
        let compacted = self.materialize();
        self.original = compacted;
        self.additions = String::new();
        self.pieces = if self.original.is_empty() {
            Vec::new()
        } else {
            vec![Piece {
                source: SourceBuffer::Original,
                start: 0,
                length: self.original.len(),
            }]
        };
        self.piece_offsets = build_piece_offsets(&self.pieces);
    }

    fn update_line_index(&mut self, start: usize, end: usize, inserted: &str) {
        if self.line_starts.is_empty() {
            self.line_starts.push(0);
            return;
        }
        let removed_length = end - start;
        let inserted_length = inserted.len();
        let old_length = self
            .length
            .saturating_add(removed_length)
            .saturating_sub(inserted_length);

        let start_line = self
            .line_starts
            .partition_point(|line_start| *line_start <= start)
            .saturating_sub(1);
        let end_line = self
            .line_starts
            .partition_point(|line_start| *line_start <= end)
            .saturating_sub(1);
        // Eine Zeile vor und nach dem Edit mit einbeziehen. Dadurch werden
        // CRLF-Paare korrekt erkannt, wenn CR oder LF getrennt eingefügt bzw.
        // gelöscht wird und sich die beiden Bytes über eine Edit-Grenze treffen.
        let first_line = start_line.saturating_sub(1);
        let last_line = end_line.saturating_add(1).min(self.line_starts.len() - 1);
        let region_start = self.line_starts[first_line];
        let old_region_end = self
            .line_starts
            .get(last_line + 1)
            .copied()
            .unwrap_or(old_length);
        let new_region_end = old_region_end
            .saturating_add(inserted_length)
            .saturating_sub(removed_length);

        let tail_index = self
            .line_starts
            .partition_point(|line_start| *line_start <= old_region_end);
        let tail = self.line_starts[tail_index..].to_vec();
        let region_text = self.slice(region_start..new_region_end);

        let mut rebuilt = self.line_starts[..first_line].to_vec();
        rebuilt.push(region_start);
        rebuilt.extend(
            line_break_starts(&region_text)
                .into_iter()
                .map(|offset| region_start + offset),
        );

        if inserted_length >= removed_length {
            let delta = inserted_length - removed_length;
            rebuilt.extend(tail.into_iter().map(|offset| offset.saturating_add(delta)));
        } else {
            let delta = removed_length - inserted_length;
            rebuilt.extend(tail.into_iter().map(|offset| offset.saturating_sub(delta)));
        }
        self.line_starts = rebuilt;
    }

    fn record_edit(&mut self, edit: Edit, coalesce_insert: bool) {
        self.history_bytes = self
            .history_bytes
            .saturating_sub(self.redo.iter().map(edit_size).sum::<usize>());
        self.redo.clear();
        let state_after = self.next_state_id;
        self.next_state_id = self.next_state_id.wrapping_add(1).max(1);
        let mut edit = edit;
        edit.state_before = self.state_id;
        edit.state_after = state_after;
        self.state_id = state_after;
        let added_history_bytes = edit_size(&edit);
        if coalesce_insert
            && edit.removed.is_empty()
            && !edit.inserted.is_empty()
            && let Some(previous) = self.undo.last_mut()
            && previous.removed.is_empty()
            && !(previous.inserted.ends_with('\r') && edit.inserted.starts_with('\n'))
            && previous.start + previous.inserted.len() == edit.start
            && previous.cursor_after == edit.cursor_before
        {
            previous.inserted.push_str(&edit.inserted);
            previous.cursor_after = edit.cursor_after;
            previous.state_after = edit.state_after;
        } else {
            self.undo.push(edit);
        }
        self.history_bytes = self.history_bytes.saturating_add(added_history_bytes);
        self.trim_history();
    }

    fn trim_history(&mut self) {
        while self.undo.len() > self.history_limit {
            let removed = self.undo.remove(0);
            self.history_bytes = self.history_bytes.saturating_sub(edit_size(&removed));
        }
        while self.redo.len() > self.history_limit {
            let removed = self.redo.remove(0);
            self.history_bytes = self.history_bytes.saturating_sub(edit_size(&removed));
        }
        while self.history_bytes > self.history_bytes_limit {
            let removed = if !self.undo.is_empty() {
                self.undo.remove(0)
            } else if !self.redo.is_empty() {
                self.redo.remove(0)
            } else {
                break;
            };
            self.history_bytes = self.history_bytes.saturating_sub(edit_size(&removed));
        }
    }

    fn merge_adjacent(&mut self) {
        let mut merged: Vec<Piece> = Vec::with_capacity(self.pieces.len());
        for piece in self.pieces.drain(..) {
            if piece.length == 0 {
                continue;
            }
            if let Some(previous) = merged.last_mut()
                && previous.source == piece.source
                && previous.start + previous.length == piece.start
            {
                previous.length += piece.length;
                continue;
            }
            merged.push(piece);
        }
        self.pieces = merged;
    }
}

fn edit_size(edit: &Edit) -> usize {
    edit.removed.len().saturating_add(edit.inserted.len())
}

impl Default for EditorBuffer {
    fn default() -> Self {
        Self::new()
    }
}

fn split_pieces(pieces: &[Piece], position: usize) -> (Vec<Piece>, Vec<Piece>) {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut logical_offset = 0usize;
    let mut split_done = false;
    for piece in pieces {
        let piece_end = logical_offset + piece.length;
        if split_done {
            right.push(*piece);
        } else if position >= piece_end {
            left.push(*piece);
        } else if position <= logical_offset {
            right.push(*piece);
            split_done = true;
        } else {
            let left_length = position - logical_offset;
            left.push(Piece {
                length: left_length,
                ..*piece
            });
            right.push(Piece {
                start: piece.start + left_length,
                length: piece.length - left_length,
                ..*piece
            });
            split_done = true;
        }
        logical_offset = piece_end;
    }
    (left, right)
}

fn build_line_starts(text: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(line_break_starts(text));
    starts
}

/// Gibt die Bytepositionen direkt nach LF, CRLF und CR zurück.
fn line_break_starts(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut starts = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                index += 2;
                starts.push(index);
            }
            b'\r' | b'\n' => {
                index += 1;
                starts.push(index);
            }
            _ => index += 1,
        }
    }
    starts
}

fn build_piece_offsets(pieces: &[Piece]) -> Vec<usize> {
    let mut offsets = Vec::with_capacity(pieces.len() + 1);
    let mut total = 0usize;
    offsets.push(0);
    for piece in pieces {
        total = total.saturating_add(piece.length);
        offsets.push(total);
    }
    offsets
}

fn previous_grapheme_boundary(text: &str, offset: usize) -> usize {
    let mut cursor = offset.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    if cursor == 0 {
        return 0;
    }
    let last_start = previous_char_boundary(text, cursor);
    let last_char = text[last_start..cursor].chars().next().unwrap_or('\0');
    if is_regional_indicator(last_char as u32) {
        let mut run_start = last_start;
        let mut count = 1usize;
        while run_start > 0 {
            let before = previous_char_boundary(text, run_start);
            let previous_char = text[before..run_start].chars().next().unwrap_or('\0');
            if !is_regional_indicator(previous_char as u32) {
                break;
            }
            run_start = before;
            count += 1;
        }
        return if count % 2 == 0 {
            previous_char_boundary(text, last_start)
        } else {
            last_start
        };
    }
    let mut previous = last_start;
    loop {
        let Some(ch) = text[previous..cursor].chars().next() else {
            break;
        };
        if is_grapheme_extend(ch) || ch == '\u{200D}' {
            cursor = previous;
            if cursor == 0 {
                break;
            }
            previous = previous_char_boundary(text, cursor);
            continue;
        }
        if previous > 0 {
            let before = previous_char_boundary(text, previous);
            if text[before..previous].starts_with('\u{200D}') {
                cursor = before;
                if cursor == 0 {
                    break;
                }
                previous = previous_char_boundary(text, cursor);
                continue;
            }
            let current = ch as u32;
            let prior = text[before..previous].chars().next().unwrap_or('\0') as u32;
            if is_regional_indicator(current) && is_regional_indicator(prior) {
                cursor = before;
                previous = if cursor > 0 {
                    previous_char_boundary(text, cursor)
                } else {
                    0
                };
                continue;
            }
        }
        break;
    }
    previous
}

fn next_grapheme_boundary(text: &str, offset: usize) -> usize {
    let mut cursor = offset.min(text.len());
    while !text.is_char_boundary(cursor) {
        cursor += 1;
    }
    if cursor >= text.len() {
        return text.len();
    }
    let first_end = next_char_boundary(text, cursor);
    let first_char = text[cursor..first_end].chars().next().unwrap_or('\0');
    if is_regional_indicator(first_char as u32) {
        if first_end < text.len() {
            let second_end = next_char_boundary(text, first_end);
            let second_char = text[first_end..second_end].chars().next().unwrap_or('\0');
            if is_regional_indicator(second_char as u32) {
                return second_end;
            }
        }
        return first_end;
    }
    let mut end = first_end;
    let mut previous = text[cursor..first_end].chars().next().unwrap_or('\0');
    while end < text.len() {
        let next_end = next_char_boundary(text, end);
        let next = text[end..next_end].chars().next().unwrap_or('\0');
        if is_grapheme_extend(next) || previous == '\u{200D}' || next == '\u{200D}' {
            end = next_end;
            previous = next;
            continue;
        }
        if is_regional_indicator(previous as u32) && is_regional_indicator(next as u32) {
            end = next_end;
        }
        break;
    }
    end
}

fn previous_char_boundary(text: &str, offset: usize) -> usize {
    let mut result = offset.saturating_sub(1).min(text.len());
    while !text.is_char_boundary(result) {
        result -= 1;
    }
    result
}

fn next_char_boundary(text: &str, offset: usize) -> usize {
    let mut result = (offset + 1).min(text.len());
    while result < text.len() && !text.is_char_boundary(result) {
        result += 1;
    }
    result
}

fn is_grapheme_extend(ch: char) -> bool {
    let value = ch as u32;
    matches!(value,
        0x0300..=0x036F | 0x0483..=0x0489 | 0x0591..=0x05BD | 0x05BF..=0x05C7 |
        0x0610..=0x061A | 0x064B..=0x065F | 0x0670 | 0x06D6..=0x06ED |
        0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE00..=0xFE0F |
        0xFE20..=0xFE2F | 0x1F3FB..=0x1F3FF | 0xE0100..=0xE01EF
    )
}

fn is_regional_indicator(value: u32) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_undo_redo_round_trip() {
        let mut buffer = EditorBuffer::from_text("Hello world");
        buffer.move_cursor(6);
        buffer.insert("NORTEX ");
        assert_eq!(buffer.as_str(), "Hello NORTEX world");
        assert!(buffer.undo());
        assert_eq!(buffer.as_str(), "Hello world");
        assert!(buffer.redo());
        assert_eq!(buffer.as_str(), "Hello NORTEX world");
    }

    #[test]
    fn replacing_a_range_is_one_undoable_edit() {
        let mut buffer = EditorBuffer::from_text("Hello world");
        buffer.replace_range(6..11, "NORTEX");
        assert_eq!(buffer.as_str(), "Hello NORTEX");
        assert!(buffer.undo());
        assert_eq!(buffer.as_str(), "Hello world");
        assert!(buffer.redo());
        assert_eq!(buffer.as_str(), "Hello NORTEX");
    }

    #[test]
    fn replacing_unicode_text_preserves_utf8_and_undo_state() {
        let mut buffer = EditorBuffer::from_text("NORTEX 😊");
        buffer.replace_range(7..11, "Code");
        assert_eq!(buffer.as_str(), "NORTEX Code");
        assert!(buffer.undo());
        assert_eq!(buffer.as_str(), "NORTEX 😊");
    }

    #[test]
    fn delete_keeps_utf8_boundaries_and_cursor_position() {
        let mut buffer = EditorBuffer::from_text("abcdef");
        buffer.move_cursor(6);
        buffer.delete(0..3);
        assert_eq!(buffer.as_str(), "def");
        assert_eq!(buffer.cursor, 3);

        let mut unicode = EditorBuffer::from_text("A😊B");
        unicode.delete(2..5);
        assert_eq!(unicode.as_str(), "AB");
    }

    #[test]
    fn typing_coalesces_and_history_is_bounded() {
        let mut buffer = EditorBuffer::new();
        for _ in 0..100_000 {
            buffer.insert("x");
        }
        assert_eq!(buffer.len_bytes(), 100_000);
        assert_eq!(buffer.undo.len(), 1);
        assert!(buffer.pieces.len() <= 2);
        assert!(buffer.undo());
        assert!(buffer.is_empty());
        buffer.set_history_limit(3);
        for value in ["a", " ", "b", " ", "c", " ", "d"] {
            buffer.insert(value);
        }
        assert!(buffer.undo.len() <= 3);

        let mut large = EditorBuffer::new();
        large.set_history_byte_limit(128);
        large.insert(&"x".repeat(1_000));
        assert!(large.history_bytes() <= 128);
    }

    #[test]
    fn line_index_and_coordinate_conversion_stay_consistent() {
        let mut buffer = EditorBuffer::from_text("a\nGrüße\nz");
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.line(1).as_deref(), Some("Grüße"));
        let offset = buffer.line_column_to_offset(1, 3).unwrap();
        assert_eq!(buffer.offset_to_line_column(offset), (1, 3));
        buffer.move_cursor(2);
        buffer.insert("NEW\n");
        assert_eq!(buffer.line_count(), 4);
        assert_eq!(buffer.line(1).as_deref(), Some("NEW"));
    }

    #[test]
    fn chunk_iteration_and_coordinate_mapping_match_materialized_text() {
        let mut buffer = EditorBuffer::from_text("ab😊\r\ncd\rxy\nz");
        buffer.move_cursor(1);
        buffer.insert("Q");
        buffer.move_cursor(9);
        buffer.insert("R");
        for line in 0..buffer.line_count() {
            let expected = buffer.line(line).unwrap();
            for column in 0..=expected.chars().count() {
                let offset = buffer.line_column_to_offset(line, column).unwrap();
                assert_eq!(buffer.offset_to_line_column(offset), (line, column));
            }
        }
        let mut chunks = String::new();
        buffer.for_each_chunk(3..7, |chunk| chunks.push_str(chunk));
        assert_eq!(chunks, "😊");
        buffer.move_cursor(buffer.len_bytes());
        assert_eq!(buffer.line_count(), 4);
    }

    #[test]
    fn cursor_treats_crlf_as_one_line_break() {
        let mut buffer = EditorBuffer::from_text("a\r\nb");
        buffer.move_cursor(3);
        buffer.move_cursor_left();
        assert_eq!(buffer.cursor, 1);
        buffer.move_cursor_right();
        assert_eq!(buffer.cursor, 3);
        buffer.move_cursor(2);
        assert_eq!(buffer.cursor, 1, "cursor must not split a CRLF sequence");
        assert_eq!(buffer.offset_to_line_column(2), (0, 1));
    }

    #[test]
    fn selection_insert_replaces_selected_text() {
        let mut buffer = EditorBuffer::from_text("Hello world");
        buffer.select(6..11);
        assert_eq!(buffer.selected_text().as_deref(), Some("world"));
        buffer.insert("NORTEX");
        assert_eq!(buffer.as_str(), "Hello NORTEX");
        assert_eq!(buffer.cursor, 12);
        assert!(buffer.undo());
        assert_eq!(buffer.as_str(), "Hello world");
        assert!(buffer.redo());
        assert_eq!(buffer.as_str(), "Hello NORTEX");
    }

    #[test]
    fn cursor_moves_by_combining_and_emoji_sequences() {
        let text = "a\u{301}👩‍💻🇩🇪🇺x";
        let mut buffer = EditorBuffer::from_text(text);
        buffer.move_cursor(text.len());
        buffer.move_cursor_left();
        assert_eq!(buffer.slice(buffer.cursor..text.len()), "x");
        buffer.move_cursor_left();
        let last_flag_start = buffer.cursor;
        assert_eq!(buffer.slice(buffer.cursor..text.len() - 1), "🇺");
        buffer.move_cursor_left();
        let flag_pair_start = buffer.cursor;
        assert_eq!(buffer.slice(buffer.cursor..last_flag_start), "🇩🇪");
        buffer.move_cursor_left();
        let technologist_start = buffer.cursor;
        assert_eq!(buffer.slice(buffer.cursor..flag_pair_start), "👩‍💻");
        buffer.move_cursor_left();
        assert_eq!(buffer.slice(buffer.cursor..technologist_start), "a\u{301}");
    }

    #[test]
    fn deleting_newline_updates_line_index() {
        let mut buffer = EditorBuffer::from_text("one\ntwo\nthree");
        buffer.delete(3..4);
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.line(0).as_deref(), Some("onetwo"));
        assert_eq!(buffer.line(1).as_deref(), Some("three"));
        assert!(buffer.undo());
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.line(1).as_deref(), Some("two"));
    }

    #[test]
    fn delete_rounds_utf8_end_up_and_never_silently_skips_a_character() {
        let mut buffer = EditorBuffer::from_text("😊x");
        buffer.delete(1..3);
        assert_eq!(buffer.as_str(), "x");
        assert_eq!(buffer.len_bytes(), 1);

        let mut crlf = EditorBuffer::from_text("a\r\nb");
        crlf.delete(2..2);
        assert_eq!(crlf.as_str(), "a\r\nb");
        crlf.delete(2..3);
        assert_eq!(crlf.as_str(), "ab");
    }

    #[test]
    fn additions_buffer_and_piece_count_are_compacted() {
        let mut buffer = EditorBuffer::from_text("x".repeat(16_384));
        for index in 0..6_000 {
            let offset = (index * 7919) % buffer.len_bytes().max(1);
            buffer.move_cursor(offset);
            buffer.insert("z");
        }
        assert!(buffer.pieces.len() <= MAX_PIECES_BEFORE_COMPACT + 2);
        assert!(
            buffer.additions.len()
                <= buffer.len_bytes().saturating_mul(ADDITIONS_COMPACT_FACTOR) + 6_000
        );
        assert_eq!(buffer.materialize().len(), buffer.len_bytes());
    }

    #[test]
    fn separately_inserted_cr_and_lf_are_one_line_break() {
        let mut buffer = EditorBuffer::from_text("ab");
        buffer.move_cursor(1);
        buffer.insert("\r");
        assert_eq!(buffer.line_count(), 2);
        buffer.insert("\n");
        assert_eq!(buffer.as_str(), "a\r\nb");
        assert_eq!(buffer.line_count(), 2);
        assert_eq!(buffer.line(0).as_deref(), Some("a"));
        assert_eq!(buffer.line(1).as_deref(), Some("b"));
        assert!(buffer.undo());
        assert_eq!(buffer.as_str(), "a\rb");
        assert_eq!(buffer.line_count(), 2);
        assert!(buffer.redo());
        assert_eq!(buffer.as_str(), "a\r\nb");

        let mut deleted = EditorBuffer::from_text("a\r\nb");
        deleted.delete(1..2);
        assert_eq!(deleted.as_str(), "ab");
        assert_eq!(deleted.line_count(), 1);
    }

    #[test]
    fn deleting_most_of_original_text_releases_dead_piece_storage() {
        let mut buffer = EditorBuffer::from_text("x".repeat(100_000));
        buffer.delete(0..99_999);
        assert_eq!(buffer.as_str(), "x");
        assert!(buffer.original.len() <= buffer.len_bytes());
        assert_eq!(buffer.redo.len(), 0);
        assert!(buffer.undo());
        assert_eq!(buffer.len_bytes(), 100_000);
    }

    #[test]
    fn cr_only_line_endings_are_indexed_and_navigable() {
        let mut buffer = EditorBuffer::from_text("one\rtwo\rthree");
        assert_eq!(buffer.line_count(), 3);
        assert_eq!(buffer.line(0).as_deref(), Some("one"));
        assert_eq!(buffer.line(1).as_deref(), Some("two"));
        assert_eq!(buffer.line(2).as_deref(), Some("three"));
        buffer.move_cursor(4);
        assert_eq!(buffer.offset_to_line_column(4), (1, 0));
        buffer.move_cursor_left();
        assert_eq!(buffer.cursor(), 3);
    }

    #[test]
    fn deterministic_mixed_line_endings_match_reference_model() {
        fn normalize_model_range(text: &str, start: usize, end: usize) -> Range<usize> {
            if start >= end {
                let mut point = start.min(text.len());
                while !text.is_char_boundary(point) {
                    point -= 1;
                }
                if point > 0
                    && point < text.len()
                    && text.as_bytes()[point - 1] == b'\r'
                    && text.as_bytes()[point] == b'\n'
                {
                    point -= 1;
                }
                return point..point;
            }
            let mut normalized_start = start.min(text.len());
            while !text.is_char_boundary(normalized_start) {
                normalized_start -= 1;
            }
            if normalized_start > 0
                && normalized_start < text.len()
                && text.as_bytes()[normalized_start - 1] == b'\r'
                && text.as_bytes()[normalized_start] == b'\n'
            {
                normalized_start -= 1;
            }
            let mut normalized_end = end.min(text.len());
            while normalized_end < text.len() && !text.is_char_boundary(normalized_end) {
                normalized_end += 1;
            }
            if normalized_end > 0
                && normalized_end < text.len()
                && text.as_bytes()[normalized_end - 1] == b'\r'
                && text.as_bytes()[normalized_end] == b'\n'
            {
                normalized_end += 1;
            }
            normalized_start..normalized_end
        }

        fn expected_lines(text: &str) -> Vec<&str> {
            let bytes = text.as_bytes();
            let mut result = Vec::new();
            let mut line_start = 0usize;
            let mut index = 0usize;
            while index < bytes.len() {
                match bytes[index] {
                    b'\r' if bytes.get(index + 1) == Some(&b'\n') => {
                        result.push(&text[line_start..index]);
                        index += 2;
                        line_start = index;
                    }
                    b'\r' | b'\n' => {
                        result.push(&text[line_start..index]);
                        index += 1;
                        line_start = index;
                    }
                    _ => index += 1,
                }
            }
            result.push(&text[line_start..]);
            result
        }

        let mut buffer = EditorBuffer::new();
        let mut model = String::new();
        let mut seed = 0x00C0_FFEE_u64;
        let insertions = ["a", "😊", "\r", "\n", "\r\n", "e\u{301}"];
        for step in 0..4_000usize {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            if seed & 3 == 0 && !model.is_empty() {
                let a = (seed as usize) % (model.len() + 1);
                let b = ((seed >> 19) as usize) % (model.len() + 1);
                let (start, end) = if a <= b { (a, b) } else { (b, a) };
                let range = normalize_model_range(&model, start, end);
                buffer.delete(start..end);
                model.replace_range(range, "");
            } else {
                let boundaries: Vec<usize> = model
                    .char_indices()
                    .map(|(offset, _)| offset)
                    .chain([model.len()])
                    .collect();
                let at = boundaries[(seed as usize) % boundaries.len()];
                let at = if at > 0
                    && at < model.len()
                    && model.as_bytes()[at - 1] == b'\r'
                    && model.as_bytes()[at] == b'\n'
                {
                    at - 1
                } else {
                    at
                };
                let value = insertions[step % insertions.len()];
                buffer.move_cursor(at);
                buffer.insert(value);
                model.insert_str(at, value);
            }
            assert_eq!(buffer.as_str(), model, "text mismatch at operation {step}");
            let lines = expected_lines(&model);
            assert_eq!(
                buffer.line_count(),
                lines.len(),
                "line count mismatch at operation {step}"
            );
            if step % 97 == 0 {
                for (index, expected) in lines.iter().enumerate() {
                    assert_eq!(
                        buffer.line(index).as_deref(),
                        Some(*expected),
                        "line mismatch at operation {step}, line {index}"
                    );
                }
                let mut chunked = String::new();
                buffer.for_each_chunk(0..buffer.len_bytes(), |chunk| chunked.push_str(chunk));
                assert_eq!(chunked, model);
            }
        }
    }

    #[test]
    fn long_line_navigation_uses_a_local_grapheme_window() {
        let text = format!("{}{}{}", "a".repeat(2_000_000), "👩‍💻", "z");
        let mut buffer = EditorBuffer::from_text(text.clone());
        buffer.move_cursor(text.len());
        buffer.move_cursor_left();
        assert_eq!(buffer.slice(buffer.cursor()..text.len()), "z");
        buffer.move_cursor_left();
        assert_eq!(buffer.slice(buffer.cursor()..text.len() - 1), "👩‍💻");
    }

    #[test]
    #[ignore = "Performance benchmark; run explicitly when measuring editor throughput"]
    fn benchmark_100k_distributed_edits_in_10_mib_single_line_document() {
        use std::time::Instant;

        let mut buffer = EditorBuffer::from_text("x".repeat(10 * 1024 * 1024));
        let started = Instant::now();
        let mut seed = 0x00A1_1CE5_u64;
        let mut expected_len = 10 * 1024 * 1024;
        for index in 0..100_000usize {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let offset = (seed as usize) % buffer.len_bytes().max(1);
            buffer.move_cursor(offset);
            let inserted = if index % 2 == 0 { "a" } else { "😊" };
            buffer.insert(inserted);
            expected_len += inserted.len();
            if index % 3 == 0 {
                let cursor = buffer.cursor();
                if cursor > 0 {
                    let before = buffer.len_bytes();
                    buffer.delete(cursor - 1..cursor);
                    expected_len -= before - buffer.len_bytes();
                }
            }
        }
        assert_eq!(buffer.len_bytes(), expected_len);
        eprintln!(
            "100k distributed edits in 10 MiB single-line document: {:?}; pieces={}; history={} bytes",
            started.elapsed(),
            buffer.pieces.len(),
            buffer.history_bytes()
        );
    }

    #[test]
    #[ignore = "Performance benchmark; run explicitly when measuring editor throughput"]
    fn benchmark_100k_distributed_edits_in_10_mib_multiline_document() {
        use std::time::Instant;

        let line = format!("{}\n", "x".repeat(1_023));
        let source = line.repeat(10_240);
        assert_eq!(source.len(), 10 * 1024 * 1024);
        let mut buffer = EditorBuffer::from_text(source);
        let started = Instant::now();
        let mut seed = 0x0BAD_C0DE_u64;
        for _ in 0..100_000usize {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let offset = (seed as usize) % buffer.len_bytes().max(1);
            buffer.move_cursor(offset);
            buffer.insert("z");
        }
        assert_eq!(buffer.line_count(), 10_241);
        eprintln!(
            "100k distributed edits in 10 MiB / 10,240 lines: {:?}; pieces={}; history={} bytes",
            started.elapsed(),
            buffer.pieces.len(),
            buffer.history_bytes()
        );
    }

    #[test]
    fn deterministic_edits_match_string_model() {
        let mut buffer = EditorBuffer::new();
        let mut model = String::new();
        let mut seed = 0x1234_5678_u64;
        for step in 0..5_000 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            if seed & 3 == 0 && !model.is_empty() {
                let boundaries: Vec<usize> = model
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain([model.len()])
                    .collect();
                let a = boundaries[(seed as usize) % boundaries.len()];
                let b = boundaries[((seed >> 16) as usize) % boundaries.len()];
                let (start, end) = if a <= b { (a, b) } else { (b, a) };
                buffer.delete(start..end);
                model.replace_range(start..end, "");
            } else {
                let boundaries: Vec<usize> = model
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain([model.len()])
                    .collect();
                let at = boundaries[(seed as usize) % boundaries.len()];
                buffer.move_cursor(at);
                let value = if step % 11 == 0 {
                    "\n"
                } else if step % 7 == 0 {
                    "😊"
                } else {
                    "x"
                };
                buffer.insert(value);
                model.insert_str(at, value);
            }
            assert_eq!(buffer.as_str(), model, "diverged at operation {step}");
            assert_eq!(
                buffer.len_bytes(),
                model.len(),
                "length mismatch at operation {step}"
            );
            let expected_line_count = model.split('\n').count();
            assert_eq!(
                buffer.line_count(),
                expected_line_count,
                "line count mismatch at operation {step}"
            );
            if step % 250 == 0 {
                let expected_lines: Vec<&str> = model.split('\n').collect();
                for line_index in [0, expected_lines.len() / 2, expected_lines.len() - 1] {
                    assert_eq!(
                        buffer.line(line_index).as_deref(),
                        Some(expected_lines[line_index]),
                        "line mismatch at operation {step}"
                    );
                }
            }
        }
        let expected_lines: Vec<&str> = model.split('\n').collect();
        assert_eq!(buffer.line_count(), expected_lines.len());
        for (line_index, expected_line) in expected_lines.iter().enumerate() {
            assert_eq!(
                buffer.line(line_index).as_deref(),
                Some(*expected_line),
                "final line mismatch at {line_index}"
            );
        }
    }
}
