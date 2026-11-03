//! Confined file access for hosted-plugin exports.
//!
//! Every path comes from a plugin and is untrusted: it must be a short relative path
//! made of plain names, may not pass through a symlink or junction, and must resolve
//! inside the project root. All checks fail closed.

use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub const MAX_PATH_CHARS: usize = 260;

const OUTSIDE: &str = "path must be relative and stay inside the project";

/// Splits a plugin-supplied path into plain names. Rejects absolute paths, `..`,
/// backslashes, drive or stream separators (`:`), control characters and names that
/// Windows would silently rewrite (trailing dot or space).
pub fn validate_relative(path: &str) -> Result<Vec<&str>, &'static str> {
    if path.is_empty() || path.chars().count() > MAX_PATH_CHARS {
        return Err(OUTSIDE);
    }
    if path.contains(['\\', ':']) || path.chars().any(char::is_control) {
        return Err(OUTSIDE);
    }
    let parts: Vec<&str> = path.split('/').collect();
    for part in &parts {
        let plain = !part.is_empty()
            && *part != "."
            && *part != ".."
            && !part.ends_with(['.', ' '])
            && Path::new(part)
                .components()
                .all(|component| matches!(component, Component::Normal(_)));
        if !plain {
            return Err(OUTSIDE);
        }
    }
    Ok(parts)
}

fn is_link(path: &Path) -> Result<bool, &'static str> {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .map_err(|_| "path does not exist")
}

/// Walks every directory of `parts[..len - 1]` below the canonical project root,
/// requiring each to be a real directory (not a link) inside the root. Returns the
/// directory that contains the final name.
fn confined_parent(root: &Path, parts: &[&str]) -> Result<PathBuf, &'static str> {
    let root = fs::canonicalize(root).map_err(|_| "project unavailable")?;
    let mut current = root.clone();
    for part in &parts[..parts.len() - 1] {
        current.push(part);
        if is_link(&current).map_err(|_| "directory does not exist")? || !current.is_dir() {
            return Err(OUTSIDE);
        }
    }
    let canonical = fs::canonicalize(&current).map_err(|_| "directory does not exist")?;
    if !canonical.starts_with(&root) {
        return Err(OUTSIDE);
    }
    Ok(canonical)
}

/// Reads one existing regular project file of at most `max_bytes`.
pub fn read_project_file(root: &Path, path: &str, max_bytes: u64) -> Result<Vec<u8>, &'static str> {
    let parts = validate_relative(path)?;
    let target = confined_parent(root, &parts)?.join(parts[parts.len() - 1]);
    let meta = fs::symlink_metadata(&target).map_err(|_| "file cannot be opened")?;
    if !meta.file_type().is_file() {
        return Err("only regular files can be archived");
    }
    if meta.len() > max_bytes {
        return Err("file exceeds the size limit");
    }
    let mut data = Vec::new();
    fs::File::open(&target)
        .map_err(|_| "file cannot be opened")?
        .take(max_bytes + 1)
        .read_to_end(&mut data)
        .map_err(|_| "file cannot be read")?;
    if data.len() as u64 > max_bytes {
        return Err("file exceeds the size limit");
    }
    Ok(data)
}

/// Writes `data` to a new or existing regular project file. The extension must match
/// `extension` (case-insensitive). The content goes to a sibling temporary file first,
/// so a failed write never leaves a partial result under the final name.
pub fn write_project_file(
    root: &Path,
    path: &str,
    extension: &str,
    data: &[u8],
) -> Result<(), &'static str> {
    let parts = validate_relative(path)?;
    let name = parts[parts.len() - 1];
    let stem_len = name.len().saturating_sub(extension.len() + 1);
    let has_extension = stem_len > 0
        && name.is_char_boundary(stem_len)
        && name[stem_len..]
            .strip_prefix('.')
            .is_some_and(|found| found.eq_ignore_ascii_case(extension));
    if !has_extension {
        return Err("output file has the wrong extension");
    }
    let parent = confined_parent(root, &parts)?;
    let target = parent.join(name);
    if fs::symlink_metadata(&target).is_ok_and(|meta| !meta.file_type().is_file()) {
        return Err("output must be a regular file");
    }
    let temporary = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let result = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .and_then(|mut file| file.write_all(data).and_then(|()| file.sync_all()))
        .and_then(|()| fs::rename(&temporary, &target));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("output file cannot be written");
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;

    pub struct TempProject(pub PathBuf);

    impl TempProject {
        pub fn new(label: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("nortex-project-io-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::TempProject;
    use super::*;

    #[test]
    fn hostile_paths_are_rejected() {
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../x",
            "a//b",
            "./a",
            "a\\b",
            "C:/x",
            "a.txt:stream",
            "trailing.",
            "space ",
            "ctl\u{1}x",
        ] {
            assert!(validate_relative(bad).is_err(), "{bad:?}");
        }
        assert!(validate_relative(&"a".repeat(MAX_PATH_CHARS + 1)).is_err());
        assert_eq!(
            validate_relative("src/main.nx").unwrap(),
            ["src", "main.nx"]
        );
    }

    #[test]
    fn reads_are_bounded_and_regular_files_only() {
        let project = TempProject::new("read");
        fs::write(project.0.join("a.txt"), b"12345").unwrap();
        fs::create_dir(project.0.join("dir")).unwrap();
        assert_eq!(read_project_file(&project.0, "a.txt", 5).unwrap(), b"12345");
        assert!(read_project_file(&project.0, "a.txt", 4).is_err());
        assert!(read_project_file(&project.0, "dir", 10).is_err());
        assert!(read_project_file(&project.0, "missing.txt", 10).is_err());
    }

    #[test]
    fn writes_need_the_extension_and_replace_atomically() {
        let project = TempProject::new("write");
        assert!(write_project_file(&project.0, "x.txt", "pdf", b"a").is_err());
        assert!(write_project_file(&project.0, ".pdf", "pdf", b"a").is_err());
        write_project_file(&project.0, "x.PDF", "pdf", b"one").unwrap();
        write_project_file(&project.0, "x.PDF", "pdf", b"two").unwrap();
        assert_eq!(fs::read(project.0.join("x.PDF")).unwrap(), b"two");
        assert_eq!(fs::read_dir(&project.0).unwrap().count(), 1);
        fs::create_dir(project.0.join("d.pdf")).unwrap();
        assert!(write_project_file(&project.0, "d.pdf", "pdf", b"x").is_err());
        assert!(write_project_file(&project.0, "nodir/x.pdf", "pdf", b"x").is_err());
    }

    fn link(target: &Path, at: &Path) -> bool {
        #[cfg(unix)]
        let result = std::os::unix::fs::symlink(target, at);
        #[cfg(windows)]
        let result = if target.is_dir() {
            std::os::windows::fs::symlink_dir(target, at)
        } else {
            std::os::windows::fs::symlink_file(target, at)
        };
        result.is_ok()
    }

    /// Skips silently where the OS refuses to create symlinks (unprivileged Windows).
    #[test]
    fn symlinks_are_refused_for_files_and_directories() {
        let project = TempProject::new("link");
        let outside = TempProject::new("link-outside");
        fs::write(outside.0.join("secret.txt"), b"s").unwrap();
        if !link(&outside.0.join("secret.txt"), &project.0.join("l.txt")) {
            return;
        }
        assert!(link(&outside.0, &project.0.join("d")));
        assert!(read_project_file(&project.0, "l.txt", 10).is_err());
        assert!(read_project_file(&project.0, "d/secret.txt", 10).is_err());
        assert!(write_project_file(&project.0, "d/x.pdf", "pdf", b"x").is_err());
        fs::write(outside.0.join("t.pdf"), b"o").unwrap();
        assert!(link(&outside.0.join("t.pdf"), &project.0.join("t.pdf")));
        assert!(write_project_file(&project.0, "t.pdf", "pdf", b"x").is_err());
        assert_eq!(fs::read(outside.0.join("t.pdf")).unwrap(), b"o");
    }

    #[cfg(windows)]
    #[test]
    fn junctions_are_refused() {
        let project = TempProject::new("junction");
        let outside = TempProject::new("junction-outside");
        fs::write(outside.0.join("secret.txt"), b"s").unwrap();
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(project.0.join("j"))
            .arg(&outside.0)
            .output()
            .unwrap()
            .status;
        if !status.success() {
            return;
        }
        assert!(read_project_file(&project.0, "j/secret.txt", 10).is_err());
        assert!(write_project_file(&project.0, "j/x.pdf", "pdf", b"x").is_err());
        assert!(!outside.0.join("x.pdf").exists());
    }
}
