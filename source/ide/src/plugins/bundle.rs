//! Extensions shipped with the IDE (`extensions/bundled/<id>/`). Installing copies the
//! package into the install root; it never executes anything.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_FILES: usize = 64;

#[derive(Debug, Clone)]
pub struct BundledExtension {
    pub id: String,
    pub name: String,
    pub description: String,
    pub dir: PathBuf,
    /// `true` for `extensions/bundled`, `false` for the marketplace catalog in `extensions/available`.
    pub bundled: bool,
}

#[derive(Deserialize)]
struct BundleManifest {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
}

/// `extensions/<kind>` next to the executable, or in the source tree when the IDE
/// runs from `target/<profile>`. `kind` is `bundled` or `available`.
pub fn bundle_root(kind: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .skip(1)
        .take(5)
        .map(|dir| dir.join("extensions").join(kind))
        .find(|candidate| candidate.is_dir())
}

pub fn list(root: &Path, bundled: bool) -> Vec<BundledExtension> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<BundledExtension> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let dir = entry.path();
            let text = fs::read_to_string(dir.join("nortex-extension.json")).ok()?;
            let manifest: BundleManifest = serde_json::from_str(&text).ok()?;
            let folder = dir.file_name()?.to_str()?;
            (folder == manifest.id).then_some(BundledExtension {
                id: manifest.id,
                name: manifest.name,
                description: manifest.description,
                dir,
                bundled,
            })
        })
        .collect();
    found.sort_by(|a, b| a.id.cmp(&b.id));
    found
}

/// Copies a bundled package to `<install_root>/<id>`. The copy is staged in a
/// temporary folder first, so a failure leaves an existing installation untouched.
pub fn install(bundled: &BundledExtension, install_root: &Path) -> io::Result<()> {
    fs::create_dir_all(install_root)?;
    let target = install_root.join(&bundled.id);
    let staging = install_root.join(format!("{}.staging", bundled.id));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir(&staging)?;
    if let Err(error) = copy_package(&bundled.dir, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let backup = install_root.join(format!("{}.previous", bundled.id));
    let _ = fs::remove_dir_all(&backup);
    let had_previous = target.exists();
    if had_previous {
        fs::rename(&target, &backup)?;
    }
    if let Err(error) = fs::rename(&staging, &target) {
        if had_previous {
            let _ = fs::rename(&backup, &target);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    let _ = fs::remove_dir_all(&backup);
    Ok(())
}

pub fn remove(id: &str, install_root: &Path) -> io::Result<()> {
    let target = install_root.join(id);
    match fs::remove_dir_all(target) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

fn copy_package(from: &Path, to: &Path) -> io::Result<()> {
    let mut count = 0;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if !metadata.is_file() {
            continue;
        }
        count += 1;
        if count > MAX_FILES || metadata.len() > MAX_FILE_BYTES {
            return Err(io::Error::other("bundled package exceeds its size limits"));
        }
        fs::copy(entry.path(), to.join(entry.file_name()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(root: &Path, id: &str) {
        let dir = root.join("bundled").join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("nortex-extension.json"),
            format!(r#"{{"id":"{id}","name":"Demo","description":"d"}}"#),
        )
        .unwrap();
        fs::write(dir.join("main.nx"), "public int main() { return 0; }").unwrap();
    }

    #[test]
    fn installs_repairs_and_removes_a_package() {
        let root = std::env::temp_dir().join(format!("nx_bundle_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        sample(&root, "demo");
        let found = list(&root.join("bundled"), true);
        assert_eq!(found.len(), 1);
        let installed = root.join("installed");

        install(&found[0], &installed).unwrap();
        assert!(installed.join("demo").join("main.nx").is_file());
        fs::write(installed.join("demo").join("main.nx"), "kaputt").unwrap();
        install(&found[0], &installed).unwrap();
        assert!(
            fs::read_to_string(installed.join("demo").join("main.nx"))
                .unwrap()
                .contains("main")
        );
        assert!(!installed.join("demo.staging").exists());

        remove("demo", &installed).unwrap();
        assert!(!installed.join("demo").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn shipped_packages_are_complete_and_valid() {
        let extensions = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("extensions");
        let bundled = list(&extensions.join("bundled"), true);
        let available = list(&extensions.join("available"), false);
        assert_eq!(bundled.len(), 8);
        assert_eq!(available.len(), 17);
        for package in bundled.iter().chain(&available) {
            for file in ["main.nx", "sdk.nx"] {
                assert!(
                    package.dir.join(file).is_file(),
                    "{} fehlt in {}",
                    file,
                    package.id
                );
            }
        }
    }

    #[test]
    fn folder_name_must_match_the_manifest_id() {
        let root = std::env::temp_dir().join(format!("nx_bundle_id_{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        sample(&root, "demo");
        fs::rename(
            root.join("bundled").join("demo"),
            root.join("bundled").join("other"),
        )
        .unwrap();
        assert!(list(&root.join("bundled"), true).is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
