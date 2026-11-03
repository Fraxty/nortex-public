//! Katalog aller Einstellungen: Kategorie, Ebene, Beschreibung, Suchbegriffe und Umsetzungsstand.
//!
//! Einträge mit [`Availability::Active`] sind mit der IDE verdrahtet und liegen in
//! [`super::settings::Settings`]. Einträge mit [`Availability::Planned`] sind vorbereitet,
//! haben aber noch keine Wirkung; die Oberfläche zeigt sie deaktiviert an.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Appearance,
    Editor,
    Shortcuts,
    Language,
    BuildDebug,
    Account,
    System,
}

impl Category {
    pub const ALL: [Category; 7] = [
        Category::Appearance,
        Category::Editor,
        Category::Shortcuts,
        Category::Language,
        Category::BuildDebug,
        Category::Account,
        Category::System,
    ];

    pub fn title(self) -> &'static str {
        crate::i18n::tr(self.german_title())
    }

    fn german_title(self) -> &'static str {
        match self {
            Self::Appearance => "Darstellung",
            Self::Editor => "Editor",
            Self::Shortcuts => "Tastenkürzel",
            Self::Language => "Sprache und Code-Hilfe",
            Self::BuildDebug => "Build und Debug",
            Self::Account => "Konto und Synchronisierung",
            Self::System => "System, Sicherheit und Updates",
        }
    }
}

/// Ebene einer Einstellung: persönliche Vorgabe, projektspezifisch oder für Fortgeschrittene.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingLevel {
    User,
    Project,
    Advanced,
}

impl SettingLevel {
    pub const ALL: [SettingLevel; 3] = [Self::User, Self::Project, Self::Advanced];

    pub fn title(self) -> &'static str {
        crate::i18n::tr(self.german_title())
    }

    fn german_title(self) -> &'static str {
        match self {
            Self::User => "Benutzer",
            Self::Project => "Projekt",
            Self::Advanced => "Erweitert",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Availability {
    Active,
    /// Vorbereitet; der Text nennt den fehlenden Baustein.
    Planned(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// JSON-Schlüssel in `settings.json`.
    pub key: &'static str,
    pub category: Category,
    pub level: SettingLevel,
    pub label: &'static str,
    pub description: &'static str,
    /// Zusätzliche Suchbegriffe (Synonyme), durch Leerzeichen getrennt.
    pub keywords: &'static str,
    pub availability: Availability,
}

const fn active(
    key: &'static str,
    category: Category,
    level: SettingLevel,
    label: &'static str,
    description: &'static str,
    keywords: &'static str,
) -> Entry {
    Entry {
        key,
        category,
        level,
        label,
        description,
        keywords,
        availability: Availability::Active,
    }
}

const fn planned(
    key: &'static str,
    category: Category,
    level: SettingLevel,
    label: &'static str,
    description: &'static str,
    keywords: &'static str,
    missing: &'static str,
) -> Entry {
    Entry {
        key,
        category,
        level,
        label,
        description,
        keywords,
        availability: Availability::Planned(missing),
    }
}

use Category::*;
use SettingLevel::*;

pub const ENTRIES: &[Entry] = &[
    active(
        "theme_mode",
        Appearance,
        User,
        "Farbschema",
        "Hell, dunkel oder hoher Kontrast.",
        "theme design modus dunkel hell kontrast",
    ),
    active(
        "density",
        Appearance,
        User,
        "Dichte",
        "Abstände und Größe der Bedienelemente.",
        "kompakt abstand padding",
    ),
    active(
        "ui_zoom",
        Appearance,
        User,
        "Oberflächen-Zoom",
        "Skalierung der gesamten Oberfläche (70–200 %).",
        "skalierung größe schrift",
    ),
    active(
        "editor_zoom",
        Appearance,
        User,
        "Editor-Zoom",
        "Skalierung der Editorschrift (70–200 %).",
        "schriftgröße font",
    ),
    active(
        "editor.line_numbers.enabled",
        Editor,
        User,
        "Zeilennummern anzeigen",
        "Blendet die Zeilennummernspalte ein oder aus.",
        "gutter zeilen",
    ),
    active(
        "editor.minimap.enabled",
        Editor,
        User,
        "Minimap anzeigen",
        "Zeigt eine verkleinerte Dateiübersicht neben dem Editor.",
        "übersicht",
    ),
    active(
        "editor.auto_save.enabled",
        Editor,
        User,
        "Automatisch speichern",
        "Speichert geänderte Dateien mit Pfad alle 30 Sekunden.",
        "autosave sichern",
    ),
    active(
        "window.status_bar.visible",
        Appearance,
        User,
        "Statusleiste anzeigen",
        "Blendet die Statusleiste am unteren Fensterrand ein oder aus.",
        "statuszeile fußzeile",
    ),
    active(
        "build.overflow_checks",
        BuildDebug,
        User,
        "Overflow-Checks",
        "Trap bei Ganzzahl-Überlauf; ein Manifest kann den Wert überschreiben.",
        "überlauf integer arithmetik",
    ),
    active(
        "extensions.max_parallel_runs",
        System,
        Advanced,
        "Parallele Erweiterungsläufe",
        "Höchstzahl gleichzeitiger Sandbox-Läufe von Erweiterungen (1 bis 4).",
        "erweiterungen plugins batch parallel",
    ),
    planned(
        "editor.tab_size",
        Editor,
        User,
        "Tabulatorbreite",
        "Anzahl der Leerzeichen pro Einrückungsebene.",
        "tab einrückung indent",
        "Editor-Einrückung noch nicht konfigurierbar",
    ),
    planned(
        "editor.word_wrap",
        Editor,
        User,
        "Zeilenumbruch",
        "Lange Zeilen am Fensterrand umbrechen.",
        "umbruch wrap",
        "Umbruch im Editor-Widget fehlt",
    ),
    planned(
        "editor.whitespace.visible",
        Editor,
        User,
        "Leerzeichen anzeigen",
        "Stellt Leerzeichen und Tabulatoren sichtbar dar.",
        "whitespace unsichtbar",
        "Rendering fehlt",
    ),
    planned(
        "editor.format_on_save",
        Editor,
        User,
        "Beim Speichern formatieren",
        "Wendet den Formatierer vor dem Speichern an.",
        "formatter format",
        "Es gibt noch keinen Formatierer",
    ),
    planned(
        "editor.bracket_matching",
        Editor,
        User,
        "Klammerpaare hervorheben",
        "Markiert zusammengehörige Klammern.",
        "klammern",
        "Hervorhebung fehlt",
    ),
    planned(
        "editor.font_family",
        Editor,
        Advanced,
        "Editorschrift",
        "Schriftfamilie des Editors.",
        "font schriftart",
        "Nur eingebettete Schrift",
    ),
    planned(
        "language.diagnostics.live",
        Language,
        User,
        "Live-Diagnose",
        "Analysiert den Code während des Tippens.",
        "fehler probleme",
        "Ist immer aktiv und noch nicht abschaltbar",
    ),
    planned(
        "language.completion.auto",
        Language,
        User,
        "Automatische Vervollständigung",
        "Schlägt Code beim Tippen vor.",
        "autocomplete intellisense",
        "Ist immer aktiv und noch nicht abschaltbar",
    ),
    planned(
        "language.inlay_hints",
        Language,
        Advanced,
        "Inline-Hinweise",
        "Zeigt Typhinweise im Code.",
        "typen hints",
        "Sprachdienst liefert keine Typhinweise",
    ),
    planned(
        "build.default_profile",
        BuildDebug,
        Project,
        "Standard-Buildprofil",
        "Debug oder Release als Vorgabe.",
        "release debug profil",
        "Projektebene (.nortex) fehlt",
    ),
    planned(
        "build.compiler_args",
        BuildDebug,
        Project,
        "Zusätzliche Compiler-Argumente",
        "Werden an nortexc übergeben.",
        "argumente flags",
        "Projektebene (.nortex) fehlt",
    ),
    planned(
        "build.working_directory",
        BuildDebug,
        Project,
        "Arbeitsverzeichnis",
        "Ordner, in dem das Programm startet.",
        "cwd ordner",
        "Projektebene (.nortex) fehlt",
    ),
    planned(
        "build.process_timeout",
        BuildDebug,
        Advanced,
        "Prozess-Zeitlimit",
        "Beendet laufende Programme nach einer Zeit.",
        "timeout abbruch",
        "Abbrechen von Prozessen ist nicht implementiert",
    ),
    planned(
        "debug.break_on_start",
        BuildDebug,
        User,
        "Beim Start anhalten",
        "Hält die Sitzung in `main` an.",
        "breakpoint haltepunkt",
        "Debug-Backend fehlt",
    ),
    planned(
        "account.sync.settings",
        Account,
        User,
        "Einstellungen synchronisieren",
        "Gilt erst nach Anmeldung beim Nortex-Konto.",
        "sync cloud",
        "Kontodienst fehlt",
    ),
    planned(
        "system.updates.channel",
        System,
        User,
        "Update-Kanal",
        "Stabil oder Vorabversion.",
        "aktualisierung version",
        "Update-Dienst fehlt",
    ),
    planned(
        "system.telemetry",
        System,
        User,
        "Nutzungsdaten",
        "Es werden derzeit keine Daten erhoben.",
        "datenschutz telemetrie privacy",
        "Keine Erhebung vorhanden",
    ),
    planned(
        "security.workspace_trust",
        System,
        Advanced,
        "Arbeitsbereich-Vertrauen",
        "Fragt vor dem Ausführen unbekannter Projekte nach.",
        "trust sicherheit",
        "Vertrauensprüfung fehlt",
    ),
];

/// Sucht in Titel, Beschreibung, Schlüssel und Synonymen (Groß-/Kleinschreibung egal, alle Wörter müssen passen).
pub fn search(query: &str, level: Option<SettingLevel>) -> Vec<&'static Entry> {
    let words: Vec<String> = query
        .split_whitespace()
        .map(|word| word.to_lowercase())
        .collect();
    ENTRIES
        .iter()
        .filter(|entry| level.is_none_or(|level| entry.level == level))
        .filter(|entry| {
            let haystack = format!(
                "{} {} {} {}",
                entry.label, entry.description, entry.key, entry.keywords
            )
            .to_lowercase();
            words.iter().all(|word| haystack.contains(word))
        })
        .collect()
}

pub fn keys_in(category: Category) -> impl Iterator<Item = &'static str> {
    ENTRIES
        .iter()
        .filter(move |entry| {
            entry.category == category && entry.availability == Availability::Active
        })
        .map(|entry| entry.key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::settings::Settings;

    #[test]
    fn every_active_key_exists_in_settings_json() {
        let value = serde_json::to_value(Settings::default()).unwrap();
        for entry in ENTRIES
            .iter()
            .filter(|e| e.availability == Availability::Active)
        {
            assert!(
                value.get(entry.key).is_some(),
                "{} fehlt in Settings",
                entry.key
            );
        }
    }

    #[test]
    fn keys_are_unique() {
        let mut keys: Vec<_> = ENTRIES.iter().map(|entry| entry.key).collect();
        keys.sort_unstable();
        let total = keys.len();
        keys.dedup();
        assert_eq!(keys.len(), total);
    }

    #[test]
    fn search_matches_synonyms_and_filters_by_level() {
        assert!(
            search("autosave", None)
                .iter()
                .any(|e| e.key == "editor.auto_save.enabled")
        );
        assert!(
            search("TAB einrückung", None)
                .iter()
                .any(|e| e.key == "editor.tab_size")
        );
        assert!(search("", None).len() == ENTRIES.len());
        assert!(search("timeout", Some(User)).is_empty());
        assert!(!search("timeout", Some(Advanced)).is_empty());
        assert!(search("xyz-nichts", None).is_empty());
    }

    #[test]
    fn every_category_has_entries() {
        for category in Category::ALL {
            assert!(
                ENTRIES.iter().any(|entry| entry.category == category) || category == Shortcuts
            );
        }
    }
}
