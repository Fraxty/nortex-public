//! Laden und Validieren von NORTEX-Projektmanifesten.
//!
//! Der Loader deckt das dokumentierte Kernschema ab. Abhängigkeiten und
//! Metadaten werden derzeit eingelesen, aber noch nicht aufgelöst.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectTarget {
    pub platform: String,
    pub architecture: String,
    pub output: String,
    pub optimization: String,
    pub debug_symbols: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    pub root: PathBuf,
    pub manifest: PathBuf,
    pub name: String,
    pub version: String,
    pub language: String,
    pub entry: PathBuf,
    pub target: ProjectTarget,
    pub dependencies: BTreeMap<String, String>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug)]
pub enum ProjectError {
    InvalidManifestExtension,
    MissingManifest,
    InvalidUtf8,
    Io(io::Error),
    InvalidToml { line: usize, message: String },
    MissingField(&'static str),
    InvalidField { field: &'static str, value: String },
    EntryEscapesProject,
    MissingEntry(PathBuf),
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidManifestExtension => {
                write!(formatter, "Projektdatei muss auf .nortex enden.")
            }
            Self::MissingManifest => write!(formatter, "Projektmanifest wurde nicht gefunden."),
            Self::InvalidUtf8 => write!(formatter, "Projektmanifest ist kein gültiges UTF-8."),
            Self::Io(error) => write!(
                formatter,
                "Projektdatei konnte nicht gelesen werden: {error}"
            ),
            Self::InvalidToml { line, message } => {
                write!(formatter, "Ungültiges Manifest in Zeile {line}: {message}")
            }
            Self::MissingField(field) => {
                write!(formatter, "Pflichtfeld '{field}' fehlt im Projektmanifest.")
            }
            Self::InvalidField { field, value } => {
                write!(formatter, "Ungültiger Wert '{value}' für '{field}'.")
            }
            Self::EntryEscapesProject => write!(
                formatter,
                "Die Einstiegsdatei darf das Projektverzeichnis nicht verlassen."
            ),
            Self::MissingEntry(path) => write!(
                formatter,
                "Einstiegsdatei wurde nicht gefunden: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ProjectError {}

impl Project {
    pub fn open(manifest: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let manifest = manifest.as_ref();
        if manifest.extension().and_then(|value| value.to_str()) != Some("nortex") {
            return Err(ProjectError::InvalidManifestExtension);
        }
        if !manifest.is_file() {
            return Err(ProjectError::MissingManifest);
        }
        let manifest = fs::canonicalize(manifest).map_err(ProjectError::Io)?;
        let source = fs::read_to_string(&manifest).map_err(|error| {
            if error.kind() == io::ErrorKind::InvalidData {
                ProjectError::InvalidUtf8
            } else {
                ProjectError::Io(error)
            }
        })?;
        let values = parse_manifest(&source)?;
        let root = manifest
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();

        let name = required_string(&values, "project.name")?;
        let version = required_string(&values, "project.version")?;
        let language = required_string(&values, "project.language")?;
        let entry_value = required_string(&values, "project.entry")?;
        let platform = required_string(&values, "target.platform")?;
        let architecture = required_string(&values, "target.architecture")?;
        let output = required_string(&values, "target.output")?;
        let optimization = required_string(&values, "build.optimization")?;
        let debug_symbols = required_bool(&values, "build.debug_symbols")?;

        if language != "NORTEX" {
            return Err(ProjectError::InvalidField {
                field: "project.language",
                value: language,
            });
        }
        if !matches!(platform.as_str(), "windows") {
            return Err(ProjectError::InvalidField {
                field: "target.platform",
                value: platform,
            });
        }
        if !matches!(architecture.as_str(), "x86_64" | "aarch64") {
            return Err(ProjectError::InvalidField {
                field: "target.architecture",
                value: architecture,
            });
        }
        if !matches!(
            output.as_str(),
            "executable" | "dynamic-library" | "static-library"
        ) {
            return Err(ProjectError::InvalidField {
                field: "target.output",
                value: output,
            });
        }
        if !matches!(optimization.as_str(), "debug" | "release" | "size" | "none") {
            return Err(ProjectError::InvalidField {
                field: "build.optimization",
                value: optimization,
            });
        }

        let entry_relative = PathBuf::from(entry_value);
        if entry_relative.is_absolute()
            || entry_relative.components().any(|part| {
                matches!(
                    part,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(ProjectError::EntryEscapesProject);
        }
        if entry_relative.extension().and_then(|value| value.to_str()) != Some("nx") {
            return Err(ProjectError::InvalidField {
                field: "project.entry",
                value: entry_relative.display().to_string(),
            });
        }
        let entry = root.join(entry_relative);
        let canonical_entry =
            fs::canonicalize(&entry).map_err(|_| ProjectError::MissingEntry(entry.clone()))?;
        if !canonical_entry.starts_with(&root) {
            return Err(ProjectError::EntryEscapesProject);
        }

        let dependencies = section_values(&values, "dependencies");
        let metadata = section_values(&values, "metadata");
        Ok(Self {
            root,
            manifest,
            name,
            version,
            language,
            entry: canonical_entry,
            target: ProjectTarget {
                platform,
                architecture,
                output,
                optimization,
                debug_symbols,
            },
            dependencies,
            metadata,
        })
    }
}

fn parse_manifest(source: &str) -> Result<BTreeMap<String, String>, ProjectError> {
    let mut section = String::new();
    let mut values = BTreeMap::new();
    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index + 1;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_owned();
            if section.is_empty() || section.chars().any(|ch| ch == '[' || ch == ']') {
                return Err(ProjectError::InvalidToml {
                    line: line_number,
                    message: "ungültiger Abschnittsname".into(),
                });
            }
            continue;
        }
        let Some((key, raw_value)) = line.split_once('=') else {
            return Err(ProjectError::InvalidToml {
                line: line_number,
                message: "erwartet wird key = value".into(),
            });
        };
        let key = key.trim();
        if key.is_empty()
            || !key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            return Err(ProjectError::InvalidToml {
                line: line_number,
                message: "ungültiger Schlüssel".into(),
            });
        }
        let value = raw_value.trim();
        let full_key = if section.is_empty() {
            key.to_owned()
        } else {
            format!("{section}.{key}")
        };
        if values.insert(full_key.clone(), value.to_owned()).is_some() {
            return Err(ProjectError::InvalidToml {
                line: line_number,
                message: format!("Schlüssel '{full_key}' doppelt vorhanden"),
            });
        }
    }
    Ok(values)
}

fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            '#' if !quoted => return &line[..index],
            _ => {}
        }
    }
    line
}

fn required_string(
    values: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<String, ProjectError> {
    let raw = values.get(key).ok_or(ProjectError::MissingField(key))?;
    parse_string(raw).ok_or_else(|| ProjectError::InvalidField {
        field: key,
        value: raw.clone(),
    })
}

fn parse_string(value: &str) -> Option<String> {
    let value = value.strip_prefix('"')?.strip_suffix('"')?;
    let mut output = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next()? {
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                _ => return None,
            }
        } else {
            output.push(ch);
        }
    }
    Some(output)
}

fn required_bool(
    values: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<bool, ProjectError> {
    match values
        .get(key)
        .ok_or(ProjectError::MissingField(key))?
        .as_str()
    {
        "true" => Ok(true),
        "false" => Ok(false),
        raw => Err(ProjectError::InvalidField {
            field: key,
            value: raw.to_owned(),
        }),
    }
}

fn section_values(values: &BTreeMap<String, String>, section: &str) -> BTreeMap<String, String> {
    let prefix = format!("{section}.");
    values
        .iter()
        .filter_map(|(key, value)| {
            key.strip_prefix(&prefix)
                .map(|name| (name.to_owned(), value.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_project() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("nortex-project-{}-{nonce}", std::process::id()));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.nx"), "public int main() { return 0; }").unwrap();
        fs::write(root.join("Game.nortex"), "[project]\nname = \"Game\"\nversion = \"1.0.0\"\nlanguage = \"NORTEX\"\nentry = \"src/main.nx\"\n\n[target]\nplatform = \"windows\"\narchitecture = \"x86_64\"\noutput = \"executable\"\n\n[build]\noptimization = \"debug\"\ndebug_symbols = true\n\n[dependencies]\n\n[metadata]\n").unwrap();
        root
    }

    #[test]
    fn reads_manifest_name_and_entry() {
        let root = temporary_project();
        let project = Project::open(root.join("Game.nortex")).unwrap();
        assert_eq!(project.name, "Game");
        assert_eq!(project.entry.file_name().unwrap(), "main.nx");
        assert_eq!(project.target.architecture, "x86_64");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn opens_repository_hello_world_manifest() {
        let manifest =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/hello-world/Example.nortex");
        let project = Project::open(manifest).unwrap();
        assert_eq!(project.name, "HelloWorld");
        assert!(project.entry.ends_with("src/main.nx"));
        assert_eq!(project.target.output, "executable");
    }

    #[test]
    fn rejects_entry_traversal() {
        let root = temporary_project();
        let manifest = root.join("Game.nortex");
        let original = fs::read_to_string(&manifest).unwrap();
        fs::write(&manifest, original.replace("src/main.nx", "../outside.nx")).unwrap();
        assert!(matches!(
            Project::open(&manifest),
            Err(ProjectError::EntryEscapesProject)
        ));
        fs::remove_dir_all(root).unwrap();
    }
}
