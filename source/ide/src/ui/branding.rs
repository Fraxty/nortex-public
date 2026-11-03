//! Embedded runtime brand assets and file-kind icon mapping.
//!
//! Only the PNGs below are loaded at runtime; the SVG masters under `assets/` are
//! build-time sources for `scripts/generate_brand_assets.py`.

use std::path::Path;

use eframe::egui;

const APP_ICON_PNG: &[u8] = include_bytes!("../../../assets/icons/app/nortex-code-256.png");
const SYMBOL_PNG: &[u8] = include_bytes!("../../../assets/icons/runtime/nortex-symbol-128.png");
const FILE_PNG: &[u8] = include_bytes!("../../../assets/icons/runtime/nortex-file-64.png");
const CODE_FILE_PNG: &[u8] =
    include_bytes!("../../../assets/icons/runtime/nortex-code-file-64.png");

/// Source file categories shown with their own explorer icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// `.nx` source files.
    Code,
    /// Other NORTEX files (`.nortex`, `.ntx`).
    General,
    Other,
}

impl FileKind {
    pub fn of(path: &Path) -> Self {
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase);
        match extension.as_deref() {
            Some("nx") => Self::Code,
            Some("nortex" | "ntx") => Self::General,
            _ => Self::Other,
        }
    }
}

/// Explorer tint by file extension (`.nx` violet, `.toml` amber, `.json` blue, `.md` light gray).
pub fn file_tint(path: &Path) -> egui::Color32 {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("nx" | "nortex" | "ntx") => egui::Color32::from_rgb(0x96, 0x70, 0xE8),
        Some("toml") => egui::Color32::from_rgb(0xD6, 0xA6, 0x4B),
        Some("json") => egui::Color32::from_rgb(0x39, 0x8B, 0xFF),
        Some("md") => egui::Color32::from_rgb(0xC4, 0xC8, 0xCC),
        _ => egui::Color32::from_rgb(0x85, 0x8B, 0x91),
    }
}

fn decode(bytes: &[u8]) -> Option<egui::ColorImage> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        rgba.as_raw(),
    ))
}

/// Window and taskbar icon derived from the application icon.
pub fn window_icon() -> Option<egui::IconData> {
    let image = image::load_from_memory_with_format(APP_ICON_PNG, image::ImageFormat::Png).ok()?;
    let rgba = image.to_rgba8();
    Some(egui::IconData {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

/// Lazily uploaded textures; a decoding failure leaves the slot empty and the UI falls back to text.
#[derive(Default)]
pub struct BrandTextures {
    symbol: Option<egui::TextureHandle>,
    file: Option<egui::TextureHandle>,
    code_file: Option<egui::TextureHandle>,
}

impl BrandTextures {
    pub fn load(&mut self, context: &egui::Context) {
        let upload = |name: &str, bytes: &[u8]| {
            decode(bytes)
                .map(|image| context.load_texture(name, image, egui::TextureOptions::LINEAR))
        };
        self.symbol = upload("nortex-symbol", SYMBOL_PNG);
        self.file = upload("nortex-file", FILE_PNG);
        self.code_file = upload("nortex-code-file", CODE_FILE_PNG);
    }

    pub fn symbol(&self) -> Option<&egui::TextureHandle> {
        self.symbol.as_ref()
    }

    pub fn file_icon(&self, kind: FileKind) -> Option<&egui::TextureHandle> {
        match kind {
            FileKind::Code => self.code_file.as_ref(),
            FileKind::General => self.file.as_ref(),
            FileKind::Other => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_map_to_their_icon_kind_case_insensitively() {
        assert_eq!(FileKind::of(Path::new("a/main.nx")), FileKind::Code);
        assert_eq!(FileKind::of(Path::new("MAIN.NX")), FileKind::Code);
        assert_eq!(FileKind::of(Path::new("x.nortex")), FileKind::General);
        assert_eq!(FileKind::of(Path::new("x.ntx")), FileKind::General);
        assert_eq!(FileKind::of(Path::new("x.txt")), FileKind::Other);
        assert_eq!(FileKind::of(Path::new("noext")), FileKind::Other);
    }

    #[test]
    fn embedded_pngs_decode_and_window_icon_is_256_square() {
        for bytes in [APP_ICON_PNG, SYMBOL_PNG, FILE_PNG, CODE_FILE_PNG] {
            assert!(decode(bytes).is_some());
        }
        let icon = window_icon().expect("icon decodes");
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
    }

    #[test]
    fn textures_load_without_panic_headless() {
        let context = egui::Context::default();
        let mut textures = BrandTextures::default();
        textures.load(&context);
        assert!(textures.symbol().is_some());
        assert!(textures.file_icon(FileKind::Code).is_some());
        assert!(textures.file_icon(FileKind::Other).is_none());
    }
}
