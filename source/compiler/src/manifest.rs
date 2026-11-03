//! Projekt-Manifest `nortex.toml`: Paketdaten, Build-Einstellungen und Abhängigkeiten.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const MANIFEST_FILE: &str = "nortex.toml";
pub const SUPPORTED_TARGET: &str = "x86_64-pc-windows-msvc";

#[derive(Debug)]
pub enum ManifestError {
    Io { path: PathBuf, message: String },
    Parse { path: PathBuf, message: String },
    Invalid { path: PathBuf, message: String },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(f, "Manifest '{}' nicht lesbar: {message}", path.display())
            }
            Self::Parse { path, message } => {
                write!(f, "Manifest '{}' ist beschädigt: {message}", path.display())
            }
            Self::Invalid { path, message } => {
                write!(f, "Manifest '{}' ungültig: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for ManifestError {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Package {
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub authors: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Build {
    pub main: String,
    pub output_name: Option<String>,
    pub target: String,
    pub overflow_checks: Option<bool>,
    pub opt_level: u8,
    pub linker_flags: Vec<String>,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            main: "src/main.nx".into(),
            output_name: None,
            target: SUPPORTED_TARGET.into(),
            overflow_checks: None,
            opt_level: 0,
            linker_flags: Vec::new(),
        }
    }
}

/// Lokale Bibliothek (`path`) oder System-Bibliothek (`lib`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Dependency {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub lib: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct NortexManifest {
    pub package: Package,
    #[serde(default)]
    pub build: Build,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
}

fn default_version() -> String {
    "0.1.0".into()
}

/// Findet das Manifest zu einem Argument: Verzeichnis mit `nortex.toml` oder die Datei selbst.
pub fn find_manifest(argument: &Path) -> Option<PathBuf> {
    if argument.is_dir() {
        let candidate = argument.join(MANIFEST_FILE);
        return candidate.is_file().then_some(candidate);
    }
    (argument
        .file_name()
        .is_some_and(|name| name == MANIFEST_FILE)
        && argument.is_file())
    .then(|| argument.to_path_buf())
}

impl NortexManifest {
    pub fn load_from_file(path: &Path) -> Result<Self, ManifestError> {
        let text = std::fs::read_to_string(path).map_err(|error| ManifestError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        Self::parse(&text, path)
    }

    pub fn parse(text: &str, path: &Path) -> Result<Self, ManifestError> {
        let manifest: Self = toml::from_str(text).map_err(|error| ManifestError::Parse {
            path: path.to_path_buf(),
            message: error.message().to_owned(),
        })?;
        manifest.validate(path)?;
        Ok(manifest)
    }

    fn validate(&self, path: &Path) -> Result<(), ManifestError> {
        let invalid = |message: String| ManifestError::Invalid {
            path: path.to_path_buf(),
            message,
        };
        if self.package.name.trim().is_empty() {
            return Err(invalid("[package].name darf nicht leer sein".into()));
        }
        if self.build.main.trim().is_empty() {
            return Err(invalid("[build].main darf nicht leer sein".into()));
        }
        if self.build.target != SUPPORTED_TARGET {
            return Err(invalid(format!(
                "[build].target '{}' wird nicht unterstützt (erwartet: {SUPPORTED_TARGET})",
                self.build.target
            )));
        }
        if self.build.opt_level > 3 {
            return Err(invalid(
                "[build].opt_level muss zwischen 0 und 3 liegen".into(),
            ));
        }
        let output_name = self
            .build
            .output_name
            .as_deref()
            .unwrap_or(&self.package.name);
        if output_name.is_empty()
            || output_name == "."
            || output_name == ".."
            || output_name.contains(['/', '\\', ':', '\0'])
        {
            return Err(invalid(format!(
                "Ausgabename '{output_name}' muss ein einfacher Dateiname ohne Pfadteile sein"
            )));
        }
        for (name, dependency) in &self.dependencies {
            if dependency.path.is_none() == dependency.lib.is_none() {
                return Err(invalid(format!(
                    "Abhängigkeit '{name}' braucht genau eines von `path` oder `lib`"
                )));
            }
        }
        Ok(())
    }

    /// Dateiname der Ziel-Executable (`output_name` oder `[package].name` + `.exe`).
    pub fn output_file_name(&self) -> String {
        let name = self
            .build
            .output_name
            .clone()
            .unwrap_or_else(|| self.package.name.clone());
        if name.to_ascii_lowercase().ends_with(".exe") {
            name
        } else {
            format!("{name}.exe")
        }
    }

    /// Einstiegspunkt relativ zum Verzeichnis des Manifests.
    pub fn main_path(&self, manifest_path: &Path) -> PathBuf {
        manifest_dir(manifest_path).join(&self.build.main)
    }

    /// Standard-Ausgabepfad: `<Manifest-Ordner>/build/<output_name>`.
    pub fn default_output(&self, manifest_path: &Path) -> PathBuf {
        manifest_dir(manifest_path)
            .join("build")
            .join(self.output_file_name())
    }

    /// Zusätzliche Linker-Argumente: `/LIBPATH:` je Pfad-Abhängigkeit, Systembibliotheken, dann `linker_flags`.
    pub fn linker_args(&self, manifest_path: &Path) -> Vec<String> {
        let base = manifest_dir(manifest_path);
        let mut args = Vec::new();
        for dependency in self.dependencies.values() {
            if let Some(path) = &dependency.path {
                args.push(format!("/LIBPATH:{}", base.join(path).display()));
            }
        }
        for dependency in self.dependencies.values() {
            if let Some(lib) = &dependency.lib {
                args.push(lib.clone());
            }
        }
        args.extend(self.build.linker_flags.iter().cloned());
        args
    }

    /// Alle `.nx`-Quellen unterhalb des Ordners der Hauptdatei (Vorbereitung für Modulauflösung).
    pub fn source_files(&self, manifest_path: &Path) -> Vec<PathBuf> {
        let main = self.main_path(manifest_path);
        let root = main
            .parent()
            .map_or_else(|| manifest_dir(manifest_path), Path::to_path_buf);
        let mut files = Vec::new();
        collect_sources(&root, &mut files, 0);
        files.sort();
        files
    }
}

fn manifest_dir(manifest_path: &Path) -> PathBuf {
    manifest_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn collect_sources(directory: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_sources(&path, files, depth + 1);
        } else if path.extension().is_some_and(|ext| ext == "nx") {
            files.push(path);
        }
    }
}

#[cfg(test)]
mod output_name_tests {
    use super::*;

    fn parse(text: &str) -> Result<NortexManifest, ManifestError> {
        NortexManifest::parse(text, Path::new("nortex.toml"))
    }

    #[test]
    fn rejects_output_names_that_escape_the_build_directory() {
        for name in ["..\\\\evil", "../evil", "C:\\\\evil", "a/b", ".."] {
            let text = format!("[package]\nname = \"demo\"\n[build]\noutput_name = \"{name}\"\n");
            assert!(parse(&text).is_err(), "{name} muss abgelehnt werden");
        }
        assert!(parse("[package]\nname = \"..\\\\x\"\n").is_err());
    }

    #[test]
    fn accepts_plain_output_name() {
        let manifest = parse("[package]\nname = \"demo\"\n[build]\noutput_name = \"app\"\n")
            .expect("gültiges Manifest");
        assert_eq!(manifest.output_file_name(), "app.exe");
    }
}
