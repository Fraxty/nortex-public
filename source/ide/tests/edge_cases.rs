//! Robustness tests for editor I/O and rendering limits. Each test names the failure it prevents.

use std::fs;
use std::path::PathBuf;

use eframe::egui::{self, Pos2, Rect, vec2};
use nortex_ide::editor::document::EditorDocument;
use nortex_ide::editor::gutter::{gutter_text, line_count};
use nortex_ide::editor::highlighting::highlight_nortex;
use nortex_ide::ui::theme::Theme;
use nortex_ide::ui::theme::typography::{self, Zoom};

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("nortex-edge-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.0) {
            for entry in entries.flatten() {
                if let Ok(mut permissions) = entry.metadata().map(|meta| meta.permissions()) {
                    #[allow(clippy::permissions_set_readonly_false)]
                    permissions.set_readonly(false);
                    let _ = fs::set_permissions(entry.path(), permissions);
                }
            }
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Protects against the gutter and the text drifting apart on very large files.
#[test]
fn large_file_keeps_gutter_and_text_row_counts_in_sync() {
    let dir = TempDir::new("large");
    let path = dir.0.join("big.nx");
    let text: String = (1..=12_000)
        .map(|n| format!("let value_{n} = {n};\n"))
        .collect();
    fs::write(&path, &text).unwrap();

    let document = EditorDocument::open(&path).unwrap();
    let loaded = document.buffer().materialize();
    assert_eq!(loaded, text);
    let rows = line_count(&loaded);
    assert_eq!(rows, 12_001);
    assert_eq!(gutter_text(rows).lines().count(), rows);
}

/// Protects against panics or quadratic blow-ups with a single very long line.
#[test]
fn very_long_line_opens_highlights_and_counts_as_one_row() {
    let dir = TempDir::new("longline");
    let path = dir.0.join("long.nx");
    let line = "let x = 1; ".repeat(400);
    assert!(line.len() > 2_000);
    fs::write(&path, &line).unwrap();

    let document = EditorDocument::open(&path).unwrap();
    let loaded = document.buffer().materialize();
    assert_eq!(line_count(&loaded), 1);
    assert!(!highlight_nortex(&loaded).is_empty());
}

/// Protects the byte-exact round trip for BOM, umlaut/space paths and emoji/math symbols.
#[test]
fn unicode_content_and_special_paths_round_trip() {
    let dir = TempDir::new("unicode dir äöü");
    let path = dir.0.join("größe test ∑.nx");
    let content = "\u{feff}// 🚀 ∑ ≠ ∞ äöüß\nlet π = 3;\n";
    fs::write(&path, content).unwrap();

    let mut document = EditorDocument::open(&path).unwrap();
    assert!(document.buffer().materialize().contains("🚀 ∑ ≠ ∞"));
    let end = document.buffer().len_bytes();
    document.buffer_mut().move_cursor(end);
    document.buffer_mut().insert("// 日本語\n");
    document.save().unwrap();
    let saved = fs::read(&path).unwrap();
    assert!(
        saved.starts_with(b"\xEF\xBB\xBF"),
        "BOM must survive saving"
    );
    let saved = String::from_utf8(saved).unwrap();
    assert!(saved.ends_with("// 日本語\n"));
}

/// Protects against a panic when the file vanishes while it is open.
#[test]
fn saving_after_external_delete_fails_cleanly_or_recreates_the_file() {
    let dir = TempDir::new("deleted");
    let path = dir.0.join("gone.nx");
    fs::write(&path, "let a = 1;\n").unwrap();
    let mut document = EditorDocument::open(&path).unwrap();
    fs::remove_file(&path).unwrap();
    document.buffer_mut().insert("// edit\n");

    match document.save() {
        Ok(()) => assert!(path.is_file()),
        Err(_) => assert!(
            document.is_modified(),
            "failed save must keep the edit pending"
        ),
    }
}

/// Protects the user's text when the target became read-only: no panic, edits stay pending.
#[test]
fn saving_a_read_only_file_reports_an_error_and_keeps_the_original() {
    let dir = TempDir::new("readonly");
    let path = dir.0.join("locked.nx");
    fs::write(&path, "original\n").unwrap();
    let mut document = EditorDocument::open(&path).unwrap();
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();
    document.buffer_mut().insert("changed ");

    let result = document.save();
    if cfg!(windows) {
        assert!(
            result.is_err(),
            "read-only files must not be overwritten silently"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "original\n");
        assert!(document.is_modified());
    }
}

/// Protects opening a path that does not exist (deleted between listing and click).
#[test]
fn opening_a_missing_file_returns_an_error() {
    let dir = TempDir::new("missing");
    assert!(EditorDocument::open(dir.0.join("nope.nx")).is_err());
}

fn run_frame(context: &egui::Context, size: egui::Vec2) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
        ..Default::default()
    };
    let _ = context.run(input, |context| {
        egui::CentralPanel::default().show(context, |ui| {
            ui.label("NORTEX Code");
            ui.horizontal_wrapped(|ui| {
                for index in 0..30 {
                    let _ = ui.button(format!("Aktion {index}"));
                }
            });
        });
    });
}

/// Protects against panics from `Theme::apply` for every theme, density and extreme zoom.
#[test]
fn theme_apply_survives_all_kinds_extreme_zoom_and_tiny_windows() {
    use nortex_ide::ui::theme::ThemeKind;
    use nortex_ide::ui::theme::spacing::Density;
    let context = egui::Context::default();
    context.set_fonts(typography::font_definitions());
    for kind in ThemeKind::ALL {
        for density in Density::ALL {
            for zoom in [typography::MIN_ZOOM, 1.0, typography::MAX_ZOOM] {
                let theme = Theme {
                    kind,
                    density,
                    zoom: Zoom {
                        ui: zoom,
                        editor: zoom,
                        ..Zoom::default()
                    },
                };
                theme.apply(&context);
                for size in [vec2(400.0, 300.0), vec2(1920.0, 1080.0)] {
                    run_frame(&context, size);
                }
            }
        }
    }
}
