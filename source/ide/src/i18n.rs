//! Zweisprachige Oberfläche (Deutsch / English).
//!
//! Die deutschen Texte sind zugleich die Schlüssel. `tr` liefert bei Englisch die Übersetzung aus
//! `translations.rs`; fehlt ein Eintrag, bleibt der deutsche Text stehen.

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

mod translations;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Language {
    #[default]
    #[serde(rename = "de")]
    German,
    #[serde(rename = "en")]
    English,
}

impl Language {
    pub const ALL: [Language; 2] = [Language::German, Language::English];

    /// Name der Sprache in der Sprache selbst.
    pub fn native_name(self) -> &'static str {
        match self {
            Self::German => "Deutsch",
            Self::English => "English",
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::German => "de",
            Self::English => "en",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_language(language: Language) {
    CURRENT.store(language as u8, Ordering::Relaxed);
    nortex_compiler::diagnostics::locale::set_locale(match language {
        Language::German => nortex_compiler::diagnostics::locale::Locale::German,
        Language::English => nortex_compiler::diagnostics::locale::Locale::English,
    });
}

pub fn language() -> Language {
    if CURRENT.load(Ordering::Relaxed) == Language::English as u8 {
        Language::English
    } else {
        Language::German
    }
}

/// Übersetzt einen deutschen Oberflächentext in die aktuelle Sprache.
pub fn tr(german: &'static str) -> &'static str {
    translate(language(), german)
}

pub fn translate(language: Language, german: &'static str) -> &'static str {
    match language {
        Language::German => german,
        Language::English => translations::english(german).unwrap_or(german),
    }
}

/// Übersetzt eine Vorlage mit `{}`-Platzhaltern und setzt die Werte der Reihe nach ein.
pub fn tr_fmt(template: &'static str, args: &[String]) -> String {
    fill(translate(language(), template), args)
}

fn fill(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut values = args.iter();
    let mut rest = template;
    while let Some(index) = rest.find("{}") {
        out.push_str(&rest[..index]);
        if let Some(value) = values.next() {
            out.push_str(value);
        }
        rest = &rest[index + 2..];
    }
    out.push_str(rest);
    out
}
/// Tastenkürzel in der Schreibweise der aktuellen Sprache (Strg+Umschalt+P → Ctrl+Shift+P).
pub fn keys(german: &str) -> String {
    keys_for(language(), german)
}

pub fn keys_for(language: Language, german: &str) -> String {
    if language == Language::German {
        return german.to_owned();
    }
    german
        .replace("Strg", "Ctrl")
        .replace("Umschalt", "Shift")
        .replace("Leertaste", "Space")
        .replace("Auf", "Up")
        .replace("Ab", "Down")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_is_identity_and_unknown_text_falls_back() {
        assert_eq!(translate(Language::German, "Datei"), "Datei");
        assert_eq!(
            translate(Language::English, "kein Eintrag xyz"),
            "kein Eintrag xyz"
        );
    }

    #[test]
    fn templates_are_filled_in_order() {
        assert_eq!(fill("A {} B {}", &["1".into(), "2".into()]), "A 1 B 2");
        assert_eq!(translate(Language::English, "Offline: {}"), "Offline: {}");
    }

    #[test]
    fn shortcuts_follow_the_language() {
        assert_eq!(
            keys_for(Language::English, "Strg+Umschalt+P / F1"),
            "Ctrl+Shift+P / F1"
        );
        assert_eq!(keys_for(Language::English, "Strg+Leertaste"), "Ctrl+Space");
        assert_eq!(keys_for(Language::German, "Strg+S"), "Strg+S");
    }

    #[test]
    fn english_translates_known_entries() {
        assert_eq!(translate(Language::English, "Datei"), "File");
    }

    #[test]
    fn extension_screen_strings_have_english_entries() {
        let keys = [
            "Erweiterungen",
            "Marketplace",
            "Installiert",
            "Veröffentlichungen",
            "Updates",
            "Favoriten",
            "Marketplace nicht verfügbar",
            "Es ist kein Marketplace-Server konfiguriert. Es werden keine Einträge oder Statistiken angezeigt.",
            "Suche, Filter, Bewertungen, Downloads und Aktualisierungen benötigen einen vertrauenswürdigen HTTPS-Dienst.",
            "Veröffentlichungsdienst nicht verfügbar",
            "Uploads, Publisher-Konten und Prüfentscheidungen benötigen ein Marketplace-Backend.",
            "Das lokale Konto ist nicht angemeldet; es werden keine Pakete übertragen.",
            "Updateinformationen sind nicht verfügbar, solange kein Marketplace-Server eingerichtet ist.",
            "Favoriten werden erst mit einem angemeldeten Marketplace-Dienst synchronisiert.",
            "Plugin-Ausführung ist nicht implementiert",
            "Freigaben werden lokal gespeichert. Diese IDE lädt oder führt derzeit keinen Plugin-Code aus.",
            "Es sind keine Plugins registriert.",
            "Freigabe ausstehend",
            "Freigeben",
            "Ablehnen",
            "Freigegeben",
            "Deaktivieren",
            "Aktivieren",
            "Abgelehnt",
            "Seitenleisteneintrag",
            "Erweiterungsstatus gespeichert.",
            "Extension-Zustand konnte nicht geladen werden: {}",
            "Erweiterungsstatus konnte nicht gespeichert werden: {}",
        ];
        for key in keys {
            assert!(
                translations::english(key).is_some(),
                "Missing English translation for {key}"
            );
        }
    }

    #[test]
    fn table_has_no_duplicate_keys() {
        let mut keys: Vec<&str> = translations::all_entries().map(|(de, _)| de).collect();
        let total = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), total);
    }

    #[test]
    fn entries_are_not_empty_and_keep_their_placeholders() {
        for (german, english) in translations::all_entries() {
            assert!(!german.trim().is_empty(), "empty key");
            assert!(
                !english.trim().is_empty(),
                "empty English text for {german}"
            );
            assert_eq!(
                german.matches("{}").count(),
                english.matches("{}").count(),
                "placeholder mismatch for {german}"
            );
        }
    }

    fn rust_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                rust_files(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push(path);
            }
        }
    }

    /// Alle String-Literale, die direkt als erstes Argument von `tr(` oder `tr_fmt(` stehen.
    fn translated_literals(source: &str) -> Vec<String> {
        let mut found = Vec::new();
        for call in ["tr(\"", "tr_fmt(\""] {
            for (at, _) in source.match_indices(call) {
                let before = source[..at].chars().next_back();
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                let mut text = String::new();
                let mut chars = source[at + call.len()..].chars();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => {
                            found.push(text);
                            break;
                        }
                        '\\' => match chars.next() {
                            Some('n') => text.push('\n'),
                            Some(other) => text.push(other),
                            None => break,
                        },
                        other => text.push(other),
                    }
                }
            }
        }
        found
    }

    #[test]
    fn every_literal_passed_to_tr_has_an_english_entry() {
        let mut files = Vec::new();
        rust_files(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut files,
        );
        let mut missing = Vec::new();
        for file in files
            .iter()
            .filter(|file| !file.components().any(|c| c.as_os_str() == "i18n"))
        {
            let source = std::fs::read_to_string(file).unwrap();
            for literal in translated_literals(&source) {
                if translations::english(&literal).is_none() {
                    missing.push(format!("{}: {literal}", file.display()));
                }
            }
        }
        assert!(
            missing.is_empty(),
            "missing English entries:\n{}",
            missing.join("\n")
        );
    }
}
