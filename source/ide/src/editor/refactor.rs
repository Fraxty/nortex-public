//! Symbol umbenennen (F2): token-basiert, damit Kommentare, Strings und Schlüsselwörter unberührt bleiben.

use super::highlighting::{HighlightKind, highlight_nortex};
use crate::i18n::tr_fmt;

/// Zustand des schwebenden Eingabefelds.
#[derive(Debug, Default, Clone)]
pub struct RenameState {
    pub is_active: bool,
    pub original_name: String,
    pub new_name: String,
    pub cursor_pos: Option<(f32, f32)>,
    /// Im ersten Frame: Fokus setzen und den ganzen Namen vorauswählen.
    pub fresh: bool,
}

impl RenameState {
    pub fn open(&mut self, current_name: String, pos: (f32, f32)) {
        self.is_active = true;
        self.original_name.clone_from(&current_name);
        self.new_name = current_name;
        self.cursor_pos = Some(pos);
        self.fresh = true;
    }

    pub fn close(&mut self) {
        self.is_active = false;
        self.fresh = false;
    }
}

fn byte_of_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map_or(text.len(), |(byte, _)| byte)
}

/// Bezeichner unter dem Cursor (Zeichenindex). Der Cursor darf direkt hinter dem Wort stehen.
pub fn identifier_at(text: &str, cursor_char: usize) -> Option<String> {
    let cursor = byte_of_char(text, cursor_char);
    highlight_nortex(text)
        .into_iter()
        .find(|span| {
            span.kind == HighlightKind::Identifier && span.start <= cursor && cursor <= span.end
        })
        .map(|span| text[span.start..span.end].to_owned())
}

pub fn is_valid_identifier(name: &str) -> bool {
    let spans = highlight_nortex(name);
    spans.len() == 1
        && spans[0].kind == HighlightKind::Identifier
        && spans[0].start == 0
        && spans[0].end == name.len()
}

/// Ersetzt alle Bezeichner-Token `old` durch `new`; liefert den neuen Text und die Trefferzahl.
pub fn rename_identifier(text: &str, old: &str, new: &str) -> Result<(String, usize), String> {
    if !is_valid_identifier(new) {
        return Err(tr_fmt(
            "„{}“ ist kein gültiger Bezeichner.",
            &[new.to_string()],
        ));
    }
    let hits: Vec<(usize, usize)> = highlight_nortex(text)
        .into_iter()
        .filter(|span| span.kind == HighlightKind::Identifier && text[span.start..span.end] == *old)
        .map(|span| (span.start, span.end))
        .collect();
    if hits.is_empty() {
        return Err(tr_fmt(
            "Keine Vorkommen von „{}“ gefunden.",
            &[old.to_string()],
        ));
    }
    let mut result = String::with_capacity(text.len());
    let mut offset = 0;
    for &(start, end) in &hits {
        result.push_str(&text[offset..start]);
        result.push_str(new);
        offset = end;
    }
    result.push_str(&text[offset..]);
    Ok((result, hits.len()))
}
