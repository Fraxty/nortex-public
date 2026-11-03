//! UI-freie Suchen-/Ersetzen-Logik (Text, Groß-/Kleinschreibung, ganze Wörter, RegEx mit `$1`/`${name}`).

use std::fmt;

use regex::{Regex, RegexBuilder};

/// Obergrenze, damit sehr breite Muster (z. B. `.`) die UI nicht lahmlegen.
pub const MAX_MATCHES: usize = 20_000;
const REGEX_SIZE_LIMIT: usize = 1 << 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FindOptions {
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub regex: bool,
}

/// Treffer als Byte- und Zeichen-Offsets (egui adressiert Zeichen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub char_start: usize,
    pub char_end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FindError {
    InvalidRegex(String),
}

impl fmt::Display for FindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRegex(_) => f.write_str("Ungültige RegEx"),
        }
    }
}

impl std::error::Error for FindError {}

/// `Ok(None)` bei leerer Suche; ungültige RegEx liefert `Err` statt zu paniken.
pub fn build(query: &str, options: FindOptions) -> Result<Option<Regex>, FindError> {
    if query.is_empty() {
        return Ok(None);
    }
    let mut pattern = if options.regex {
        query.to_owned()
    } else {
        regex::escape(query)
    };
    if options.whole_word {
        pattern = format!(r"\b(?:{pattern})\b");
    }
    RegexBuilder::new(&pattern)
        .case_insensitive(!options.case_sensitive)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map(Some)
        .map_err(|error| FindError::InvalidRegex(error.to_string()))
}

/// Alle nicht-leeren Treffer in Textreihenfolge.
pub fn find_all(text: &str, query: &str, options: FindOptions) -> Result<Vec<Match>, FindError> {
    let Some(re) = build(query, options)? else {
        return Ok(Vec::new());
    };
    let mut matches = Vec::new();
    let (mut last_byte, mut last_char) = (0, 0);
    for found in re.find_iter(text) {
        if found.is_empty() {
            continue;
        }
        let char_start = last_char + text[last_byte..found.start()].chars().count();
        let char_end = char_start + found.as_str().chars().count();
        matches.push(Match {
            start: found.start(),
            end: found.end(),
            char_start,
            char_end,
        });
        (last_byte, last_char) = (found.end(), char_end);
        if matches.len() >= MAX_MATCHES {
            break;
        }
    }
    Ok(matches)
}

fn expand(caps: &regex::Captures<'_>, replacement: &str, options: FindOptions, out: &mut String) {
    if options.regex {
        caps.expand(replacement, out);
    } else {
        out.push_str(replacement);
    }
}

/// Ersetzt alle Treffer; liefert neuen Text und Anzahl.
pub fn replace_all(
    text: &str,
    query: &str,
    replacement: &str,
    options: FindOptions,
) -> Result<(String, usize), FindError> {
    let Some(re) = build(query, options)? else {
        return Ok((text.to_owned(), 0));
    };
    let mut out = String::with_capacity(text.len());
    let (mut last, mut count) = (0, 0);
    for caps in re.captures_iter(text) {
        let Some(whole) = caps.get(0).filter(|m| !m.is_empty()) else {
            continue;
        };
        out.push_str(&text[last..whole.start()]);
        expand(&caps, replacement, options, &mut out);
        last = whole.end();
        count += 1;
    }
    out.push_str(&text[last..]);
    Ok((out, count))
}

/// Ersetzt genau einen Treffer; liefert neuen Text und die Länge der Ersetzung in Zeichen.
pub fn replace_match(
    text: &str,
    found: &Match,
    query: &str,
    replacement: &str,
    options: FindOptions,
) -> Result<(String, usize), FindError> {
    let mut piece = String::new();
    if let Some(re) = build(query, options)?
        && let Some(caps) = re.captures_at(text, found.start)
        && caps
            .get(0)
            .is_some_and(|m| m.start() == found.start && m.end() == found.end)
    {
        expand(&caps, replacement, options, &mut piece);
    } else {
        piece.push_str(replacement);
    }
    let mut out = String::with_capacity(text.len() + piece.len());
    out.push_str(&text[..found.start]);
    out.push_str(&piece);
    out.push_str(&text[found.end..]);
    Ok((out, piece.chars().count()))
}

/// Nächster (vorwärts: `char_start >= pos`) bzw. vorheriger (`char_start < pos`) Treffer mit Umbruch.
pub fn nearest(matches: &[Match], pos: usize, forward: bool) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    if forward {
        Some(
            matches
                .iter()
                .position(|m| m.char_start >= pos)
                .unwrap_or(0),
        )
    } else {
        Some(
            matches
                .iter()
                .rposition(|m| m.char_start < pos)
                .unwrap_or(matches.len() - 1),
        )
    }
}

/// Zustand des Overlays und der aktuellen Trefferliste.
#[derive(Default)]
pub struct FindState {
    pub open: bool,
    pub show_replace: bool,
    pub query: String,
    pub replacement: String,
    pub options: FindOptions,
    pub matches: Vec<Match>,
    pub current: Option<usize>,
    pub error: Option<String>,
    pub focus_query: bool,
    pub scroll_to_current: bool,
    /// Cursorposition beim Öffnen; Ausgangspunkt für die Trefferwahl während des Tippens.
    pub anchor: usize,
}

impl FindState {
    pub fn open_find(&mut self, selection: Option<&str>) {
        self.open = true;
        self.focus_query = true;
        if let Some(text) = selection.filter(|text| !text.is_empty() && !text.contains('\n')) {
            self.query = text.to_owned();
        }
    }

    pub fn open_replace(&mut self, selection: Option<&str>) {
        self.open_find(selection);
        self.show_replace = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.matches.clear();
        self.current = None;
        self.error = None;
    }

    /// Berechnet Treffer neu; `current` bleibt, wird aber auf gültigen Bereich begrenzt.
    pub fn refresh(&mut self, text: &str) {
        match find_all(text, &self.query, self.options) {
            Ok(matches) => {
                self.matches = matches;
                self.error = None;
            }
            Err(error) => {
                self.matches.clear();
                self.error = Some(error.to_string());
            }
        }
        self.current = match self.current {
            _ if self.matches.is_empty() => None,
            Some(index) => Some(index.min(self.matches.len() - 1)),
            None => None,
        };
    }

    /// Wählt den ersten Treffer ab `anchor` (Zeichenindex).
    pub fn select_from(&mut self, anchor: usize) {
        self.current = nearest(&self.matches, anchor, true);
        self.scroll_to_current = self.current.is_some();
    }

    /// Springt zum nächsten/vorherigen Treffer (mit Umbruch).
    pub fn step(&mut self, forward: bool, anchor: usize) -> Option<Match> {
        if self.matches.is_empty() {
            return None;
        }
        let len = self.matches.len();
        let index = match self.current {
            Some(index) if forward => (index + 1) % len,
            Some(index) => (index + len - 1) % len,
            None => nearest(&self.matches, anchor, forward)?,
        };
        self.current = Some(index);
        self.scroll_to_current = true;
        self.matches.get(index).copied()
    }

    pub fn current_match(&self) -> Option<Match> {
        self.current
            .and_then(|index| self.matches.get(index).copied())
    }

    pub fn counter(&self) -> String {
        if let Some(error) = &self.error {
            return error.clone();
        }
        if self.query.is_empty() {
            return String::new();
        }
        match self.current {
            Some(index) => format!("{}/{}", index + 1, self.matches.len()),
            None => format!("0/{}", self.matches.len()),
        }
    }
}
