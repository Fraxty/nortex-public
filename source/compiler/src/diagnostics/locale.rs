//! Sprachumschaltung für Compiler-Meldungen (Deutsch ist die Quellsprache).
//!
//! Meldungen werden zur Anzeige übersetzt: Die Vorlagen in `locale_table` enthalten `{}` als
//! Platzhalter. Die Compiler-Logik, Tests und Diagnosecodes bleiben davon unberührt.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[path = "locale_table.rs"]
mod table;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Locale {
    #[default]
    German,
    English,
}

impl Locale {
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "de" | "german" | "deutsch" => Some(Self::German),
            "en" | "english" => Some(Self::English),
            _ => None,
        }
    }

    pub fn from_env() -> Self {
        std::env::var("NORTEX_LANG")
            .ok()
            .and_then(|value| Self::parse(&value))
            .unwrap_or_default()
    }
}

const UNSET: u8 = 0;
static OVERRIDE: AtomicU8 = AtomicU8::new(UNSET);

/// Setzt die Sprache für diesen Prozess (z. B. durch `--lang`); hat Vorrang vor `NORTEX_LANG`.
pub fn set_locale(locale: Locale) {
    OVERRIDE.store(
        match locale {
            Locale::German => 1,
            Locale::English => 2,
        },
        Ordering::Relaxed,
    );
}

pub fn current() -> Locale {
    match OVERRIDE.load(Ordering::Relaxed) {
        1 => Locale::German,
        2 => Locale::English,
        _ => Locale::from_env(),
    }
}

struct Entry {
    pieces: Vec<&'static str>,
    english: &'static str,
}

struct Catalog {
    exact: std::collections::HashMap<&'static str, &'static str>,
    patterns: Vec<Entry>,
}

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    let mut exact = std::collections::HashMap::new();
    let mut patterns = Vec::new();
    for (german, english) in table::TABLE {
        if german.contains("{}") {
            patterns.push(Entry {
                pieces: german.split("{}").collect(),
                english,
            });
        } else {
            exact.insert(*german, *english);
        }
    }
    // Spezifischere Vorlagen (mehr feste Zeichen) zuerst.
    patterns.sort_by_key(|entry| {
        std::cmp::Reverse(entry.pieces.iter().map(|p| p.len()).sum::<usize>())
    });
    Catalog { exact, patterns }
});

fn capture<'a>(pieces: &[&str], message: &'a str) -> Option<Vec<&'a str>> {
    let first = pieces.first()?;
    let last = pieces.last()?;
    let rest = message.strip_prefix(first)?;
    let mut captures = Vec::new();
    let mut remaining = rest;
    for piece in &pieces[1..pieces.len() - 1] {
        let index = remaining.find(piece)?;
        captures.push(&remaining[..index]);
        remaining = &remaining[index + piece.len()..];
    }
    let end = remaining.strip_suffix(last)?;
    captures.push(end);
    Some(captures)
}

/// Übersetzt eine Compiler-Meldung. Unbekannte Meldungen bleiben unverändert.
pub fn localize(message: &str, locale: Locale) -> String {
    if locale == Locale::German {
        return message.to_owned();
    }
    let catalog = &*CATALOG;
    if let Some(english) = catalog.exact.get(message) {
        return (*english).to_owned();
    }
    for entry in &catalog.patterns {
        if let Some(values) = capture(&entry.pieces, message) {
            let mut values = values.into_iter();
            let mut out = String::with_capacity(entry.english.len() + message.len());
            let mut rest = entry.english;
            while let Some(index) = rest.find("{}") {
                out.push_str(&rest[..index]);
                out.push_str(values.next().unwrap_or_default());
                rest = &rest[index + 2..];
            }
            out.push_str(rest);
            return out;
        }
    }
    message.to_owned()
}

/// Übersetzt in die aktuelle Prozesssprache.
pub fn localize_current(message: &str) -> String {
    localize(message, current())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_is_the_identity() {
        let text = "Unbekannter Bezeichner 'x'. Der Wert wurde weder deklariert noch importiert.";
        assert_eq!(localize(text, Locale::German), text);
    }

    #[test]
    fn placeholders_are_carried_over() {
        assert_eq!(
            localize(
                "Unbekannter Bezeichner 'summee'. Der Wert wurde weder deklariert noch importiert.",
                Locale::English
            ),
            "Unknown identifier 'summee'. The value was neither declared nor imported."
        );
        assert_eq!(
            localize(
                "Methode 'A.f' ist außerhalb ihres deklarierenden Typs nicht zugänglich.",
                Locale::English
            ),
            "Method 'A.f' is not accessible outside its declaring type."
        );
    }

    #[test]
    fn exact_messages_and_unknown_messages() {
        assert_eq!(
            localize("Block ist nicht abgeschlossen.", Locale::English),
            "Block is not terminated."
        );
        assert_eq!(
            localize("völlig unbekannt", Locale::English),
            "völlig unbekannt"
        );
    }

    #[test]
    fn language_codes_are_parsed() {
        assert_eq!(Locale::parse("EN"), Some(Locale::English));
        assert_eq!(Locale::parse("de"), Some(Locale::German));
        assert_eq!(Locale::parse("fr"), None);
    }
}
