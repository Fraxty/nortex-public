//! Dateisystemzugriff für Projektscan und Manifest.

use crate::*;
use nortex_ide::i18n::tr_fmt;

/// `canonicalize` yields `\\?\C:\...` on Windows; the prefix is noise in the UI.
pub(crate) fn simplify_windows_path(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
        _ => path,
    }
}

pub(crate) fn collect_source_files(
    root: &Path,
    depth: usize,
    files: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if depth > MAX_PROJECT_DEPTH || files.len() >= MAX_PROJECT_FILES {
        return Ok(());
    }
    let mut entries = fs::read_dir(root)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if files.len() >= MAX_PROJECT_FILES {
            break;
        }
        let file_type = entry.file_type()?;
        let name = entry.file_name();
        if file_type.is_dir() {
            if !matches!(
                name.to_str(),
                Some(".git" | "target" | "node_modules" | "vendor" | ".idea")
            ) {
                collect_source_files(&entry.path(), depth + 1, files)?;
            }
        } else if file_type.is_file()
            && matches!(
                entry
                    .path()
                    .extension()
                    .and_then(|extension| extension.to_str()),
                Some("nx" | "nortex")
            )
        {
            files.push(entry.path());
        }
    }
    Ok(())
}

pub(crate) fn read_project_snapshot(root: &Path) -> io::Result<ProjectSnapshot> {
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            tr_fmt(
                "Projektordner existiert nicht: {}",
                &[root.display().to_string()],
            ),
        ));
    }
    let project_root = simplify_windows_path(fs::canonicalize(root)?);
    let mut source_files = Vec::new();
    collect_source_files(&project_root, 0, &mut source_files)?;
    source_files.sort();
    let manifest = load_project_manifest(&project_root);
    Ok(ProjectSnapshot {
        project_root,
        source_files,
        manifest,
    })
}

/// Liest `nortex.toml` im Projektordner; ein fehlerhaftes Manifest wird ignoriert
/// (der Compiler meldet den Fehler beim Build).
pub(crate) fn load_project_manifest(root: &Path) -> Option<(PathBuf, NortexManifest)> {
    let path = manifest::find_manifest(root)?;
    NortexManifest::load_from_file(&path)
        .ok()
        .map(|manifest| (path, manifest))
}
