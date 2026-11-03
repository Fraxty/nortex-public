//! Persistent appearance settings stored as `settings.json`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::account::SyncPreferences;

use super::theme::spacing::Density;
use super::theme::typography::{Zoom, clamp_zoom};
use super::theme::{Theme, ThemeKind};

pub const MAX_RECENT_FILES: usize = 8;
pub const DEFAULT_PARALLEL_EXTENSION_RUNS: u8 = 2;
pub const MAX_PARALLEL_EXTENSION_RUNS: u8 = 4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_mode: ThemeKind,
    pub density: Density,
    pub ui_zoom: f32,
    pub editor_zoom: f32,
    pub recent_files: Vec<String>,
    #[serde(rename = "app.language")]
    pub language: crate::i18n::Language,
    #[serde(rename = "editor.minimap.enabled")]
    pub minimap_enabled: bool,
    #[serde(rename = "editor.line_numbers.enabled")]
    pub line_numbers_enabled: bool,
    /// Speichert geänderte Dokumente mit Dateipfad in festen Abständen.
    #[serde(rename = "editor.auto_save.enabled")]
    pub auto_save_enabled: bool,
    #[serde(rename = "window.status_bar.visible")]
    pub status_bar_visible: bool,
    #[serde(rename = "build.overflow_checks")]
    pub overflow_checks: bool,
    /// Upper bound for sandboxed extension runs that execute at the same time.
    #[serde(rename = "extensions.max_parallel_runs")]
    pub max_parallel_extension_runs: u8,
    /// Vorgemerkt für den späteren Kontodienst; wirkt erst nach einer Anmeldung.
    #[serde(rename = "account.sync")]
    pub sync: SyncPreferences,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeKind::default(),
            density: Density::default(),
            ui_zoom: 1.0,
            editor_zoom: 1.0,
            recent_files: Vec::new(),
            language: crate::i18n::Language::default(),
            minimap_enabled: false,
            line_numbers_enabled: true,
            auto_save_enabled: false,
            status_bar_visible: true,
            overflow_checks: cfg!(debug_assertions),
            max_parallel_extension_runs: DEFAULT_PARALLEL_EXTENSION_RUNS,
            sync: SyncPreferences::default(),
        }
    }
}

impl Settings {
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            theme_mode: theme.kind,
            density: theme.density,
            ui_zoom: theme.zoom.ui,
            editor_zoom: theme.zoom.editor,
            ..Self::default()
        }
    }

    /// `true`, wenn der Wert unter `key` (JSON-Name) dem Standardwert entspricht.
    pub fn is_default(&self, key: &str) -> bool {
        json_field(self, key) == json_field(&Self::default(), key)
    }

    /// Setzt genau einen Schlüssel auf den Standardwert zurück. Unbekannte Schlüssel werden ignoriert.
    pub fn reset_key(&mut self, key: &str) {
        let Some(default) = json_field(&Self::default(), key) else {
            return;
        };
        let Ok(serde_json::Value::Object(mut object)) = serde_json::to_value(&*self) else {
            return;
        };
        object.insert(key.to_owned(), default);
        if let Some(updated) =
            Self::from_json_lenient(&serde_json::Value::Object(object).to_string())
        {
            *self = updated;
        }
    }

    /// Setzt die Schlüssel zurück, behält aber die zuletzt geöffneten Dateien.
    pub fn reset_keys<'a>(&mut self, keys: impl IntoIterator<Item = &'a str>) {
        for key in keys {
            self.reset_key(key);
        }
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Updates the appearance fields and keeps everything else (e.g. recent files).
    pub fn with_theme(mut self, theme: &Theme) -> Self {
        let appearance = Self::from_theme(theme);
        self.theme_mode = appearance.theme_mode;
        self.density = appearance.density;
        self.ui_zoom = appearance.ui_zoom;
        self.editor_zoom = appearance.editor_zoom;
        self
    }

    /// Most recent first, no duplicates, capped at [`MAX_RECENT_FILES`].
    pub fn push_recent(&mut self, path: &Path) {
        let entry = normalize_entry(&path.display().to_string());
        self.recent_files.retain(|existing| *existing != entry);
        self.recent_files.insert(0, entry);
        self.recent_files.truncate(MAX_RECENT_FILES);
    }

    /// Field-wise tolerant parsing: a wrong type in one field only resets that field.
    pub fn from_json_lenient(text: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let object = value.as_object()?;
        let defaults = Self::default();
        let field = |name: &str| object.get(name).cloned();
        let flag = |name: &str, fallback: bool| {
            field(name)
                .and_then(|value| value.as_bool())
                .unwrap_or(fallback)
        };
        let zoom = |name: &str, fallback: f32| {
            field(name)
                .and_then(|value| value.as_f64())
                .filter(|value| value.is_finite())
                .map_or(fallback, |value| value as f32)
        };
        Some(Self {
            theme_mode: field("theme_mode")
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or(defaults.theme_mode),
            density: field("density")
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or(defaults.density),
            ui_zoom: zoom("ui_zoom", defaults.ui_zoom),
            editor_zoom: zoom("editor_zoom", defaults.editor_zoom),
            language: field("app.language")
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or(defaults.language),
            minimap_enabled: field("editor.minimap.enabled")
                .and_then(|value| value.as_bool())
                .unwrap_or(defaults.minimap_enabled),
            line_numbers_enabled: flag(
                "editor.line_numbers.enabled",
                defaults.line_numbers_enabled,
            ),
            auto_save_enabled: flag("editor.auto_save.enabled", defaults.auto_save_enabled),
            status_bar_visible: flag("window.status_bar.visible", defaults.status_bar_visible),
            overflow_checks: flag("build.overflow_checks", defaults.overflow_checks),
            max_parallel_extension_runs: field("extensions.max_parallel_runs")
                .and_then(|value| value.as_u64())
                .map_or(defaults.max_parallel_extension_runs, |count| {
                    count.clamp(1, u64::from(MAX_PARALLEL_EXTENSION_RUNS)) as u8
                }),
            sync: field("account.sync")
                .and_then(|value| serde_json::from_value(value).ok())
                .unwrap_or(defaults.sync),
            recent_files: {
                let mut seen = Vec::<String>::new();
                for entry in field("recent_files")
                    .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
                    .unwrap_or_default()
                {
                    let entry = normalize_entry(&entry);
                    if !seen.contains(&entry) {
                        seen.push(entry);
                    }
                }
                seen.truncate(MAX_RECENT_FILES);
                seen
            },
        })
    }

    pub fn theme(&self) -> Theme {
        Theme {
            kind: self.theme_mode,
            density: self.density,
            zoom: Zoom {
                ui: clamp_zoom(self.ui_zoom),
                editor: clamp_zoom(self.editor_zoom),
                ..Zoom::default()
            },
        }
    }

    /// Missing or unreadable files yield the defaults; settings never block startup.
    pub fn load(path: &Path) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| {
                let text = String::from_utf8_lossy(&bytes);
                Self::from_json_lenient(text.trim_start_matches('\u{feff}'))
            })
            .unwrap_or_default()
    }

    /// Like [`Settings::load`], but rewrites the file when it is missing, corrupt or incomplete.
    pub fn load_and_repair(path: &Path) -> Self {
        let settings = Self::load(path);
        let canonical = serde_json::to_string_pretty(&settings).ok();
        let on_disk = fs::read_to_string(path).ok();
        if on_disk.as_deref() != canonical.as_deref() {
            let _ = settings.save(path);
        }
        settings
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, text)?;
        fs::rename(&temporary, path)
    }
}

fn json_field(settings: &Settings, key: &str) -> Option<serde_json::Value> {
    serde_json::to_value(settings).ok()?.get(key).cloned()
}

/// Drops the Windows verbatim prefix (`\\?\C:\...`) so one file has one recent entry.
fn normalize_entry(entry: &str) -> String {
    match entry.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => rest.to_string(),
        _ => entry.to_string(),
    }
}

/// `%APPDATA%\NORTEX Code\settings.json`, falling back to the working directory.
pub fn settings_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(|dir| PathBuf::from(dir).join("NORTEX Code"))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::theme::typography;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nortex-settings-{}-{name}", std::process::id()))
    }

    #[test]
    fn round_trips_through_json() {
        let path = temp_file("roundtrip.json");
        let settings = Settings {
            theme_mode: ThemeKind::HighContrast,
            density: Density::Compact,
            ui_zoom: 1.25,
            editor_zoom: 1.5,
            recent_files: vec!["a.nx".into()],
            language: crate::i18n::Language::English,
            minimap_enabled: false,
            line_numbers_enabled: false,
            auto_save_enabled: true,
            status_bar_visible: false,
            overflow_checks: !cfg!(debug_assertions),
            max_parallel_extension_runs: 3,
            sync: SyncPreferences {
                settings: true,
                ..SyncPreferences::default()
            },
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path), settings);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn single_keys_can_be_checked_and_reset() {
        let mut settings = Settings {
            line_numbers_enabled: false,
            ui_zoom: 1.5,
            ..Settings::default()
        };
        assert!(settings.is_default("editor.minimap.enabled"));
        assert!(!settings.is_default("editor.line_numbers.enabled"));
        settings.reset_key("editor.line_numbers.enabled");
        assert!(settings.line_numbers_enabled);
        assert_eq!(settings.ui_zoom, 1.5);
        settings.reset_keys(["ui_zoom", "does.not.exist"]);
        assert_eq!(settings.ui_zoom, 1.0);
    }

    #[test]
    fn exported_json_can_be_imported_again() {
        let settings = Settings {
            auto_save_enabled: true,
            ..Settings::default()
        };
        let imported = Settings::from_json_lenient(&settings.to_json_pretty()).unwrap();
        assert_eq!(imported, settings);
    }

    #[test]
    fn broken_theme_values_fall_back_silently() {
        let text = r##"{"theme_mode":"#zzzzzz","slider_sensitivity":"fast"}"##;
        let loaded = Settings::from_json_lenient(text).unwrap();
        assert_eq!(loaded.theme_mode, ThemeKind::default());
        assert_eq!(
            Settings::load(Path::new("does/not/exist.json")),
            Settings::default()
        );
    }

    #[test]
    fn corrupt_or_missing_file_falls_back_to_defaults() {
        let path = temp_file("corrupt.json");
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
        let _ = fs::remove_file(&path);
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[test]
    fn partial_file_keeps_defaults_for_missing_fields() {
        let path = temp_file("partial.json");
        fs::write(&path, r#"{"theme_mode":"professional_dark"}"#).unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.theme_mode, ThemeKind::ProfessionalDark);
        assert_eq!(loaded.ui_zoom, 1.0);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn wrong_types_only_reset_the_affected_field() {
        let text = r#"{"theme_mode":"professional_dark","ui_zoom":"big","editor_zoom":1.5,"recent_files":42,"density":null}"#;
        let loaded = Settings::from_json_lenient(text).unwrap();
        assert_eq!(loaded.theme_mode, ThemeKind::ProfessionalDark);
        assert_eq!(loaded.ui_zoom, 1.0);
        assert_eq!(loaded.editor_zoom, 1.5);
        assert!(loaded.recent_files.is_empty());
        assert_eq!(loaded.density, Density::default());
    }

    #[test]
    fn non_object_json_and_bom_are_handled() {
        assert!(Settings::from_json_lenient("[1,2]").is_none());
        assert!(Settings::from_json_lenient("").is_none());
        let path = temp_file("bom.json");
        fs::write(&path, "\u{feff}{\"editor_zoom\":2.0}").unwrap();
        assert_eq!(Settings::load(&path).editor_zoom, 2.0);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_and_repair_rewrites_corrupt_and_incomplete_files() {
        for (name, content) in [
            ("repair-a.json", "{ broken"),
            ("repair-b.json", r#"{"ui_zoom":"x"}"#),
        ] {
            let path = temp_file(name);
            fs::write(&path, content).unwrap();
            let repaired = Settings::load_and_repair(&path);
            assert_eq!(repaired, Settings::default());
            let on_disk: Settings =
                serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(on_disk, Settings::default());
            let _ = fs::remove_file(&path);
        }
    }

    #[test]
    fn recent_files_are_deduplicated_ordered_and_capped() {
        let mut settings = Settings::default();
        for index in 0..20 {
            settings.push_recent(Path::new(&format!("f{index}.nx")));
        }
        settings.push_recent(Path::new("f15.nx"));
        assert_eq!(settings.recent_files.len(), MAX_RECENT_FILES);
        assert_eq!(settings.recent_files[0], "f15.nx");
        assert_eq!(
            settings
                .recent_files
                .iter()
                .filter(|entry| *entry == "f15.nx")
                .count(),
            1
        );
    }

    #[test]
    fn verbatim_prefixed_and_plain_paths_share_one_recent_entry() {
        let loaded = Settings::from_json_lenient(
            r#"{"recent_files":["\\\\?\\C:\\a.nx","C:\\a.nx","D:\\b.nx"]}"#,
        )
        .unwrap();
        assert_eq!(loaded.recent_files, vec!["C:\\a.nx", "D:\\b.nx"]);
        let mut settings = Settings::default();
        settings.push_recent(Path::new(r"\\?\C:\a.nx"));
        settings.push_recent(Path::new(r"C:\a.nx"));
        assert_eq!(settings.recent_files.len(), 1);
    }

    #[test]
    fn with_theme_preserves_recents() {
        let mut settings = Settings::default();
        settings.push_recent(Path::new("a.nx"));
        let theme = Theme {
            kind: ThemeKind::HighContrast,
            ..settings.theme()
        };
        let updated = settings.with_theme(&theme);
        assert_eq!(updated.theme_mode, ThemeKind::HighContrast);
        assert_eq!(updated.recent_files, vec!["a.nx".to_string()]);
    }

    #[test]
    fn out_of_range_zoom_is_clamped_when_building_theme() {
        for (value, expected) in [
            (99.0, typography::MAX_ZOOM),
            (-5.0, typography::MIN_ZOOM),
            (f32::NAN, 1.0),
        ] {
            let settings = Settings {
                ui_zoom: value,
                ..Settings::default()
            };
            let zoom = settings.theme().zoom.ui;
            assert!(
                zoom.is_finite() && (typography::MIN_ZOOM..=typography::MAX_ZOOM).contains(&zoom)
            );
            if !value.is_nan() {
                assert_eq!(zoom, expected);
            }
        }
    }
}
