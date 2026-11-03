//! Toolchain discovery and build-state rules shared by the IDE shell and its tests.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Succeeded,
    Failed,
}

/// Only the build decides the outcome. The exit code of the compiled program is data
/// (e.g. `exit(55)`) and never turns a successful build red.
pub fn classify_build(build_succeeded: bool, output_exists: bool) -> RunOutcome {
    if build_succeeded && output_exists {
        RunOutcome::Succeeded
    } else {
        RunOutcome::Failed
    }
}

/// Guards against concurrent builds; they would race on the shared `.obj`/`.exe` files.
#[derive(Debug, Default)]
pub struct RunGate {
    running: bool,
}

impl RunGate {
    pub fn try_begin(&mut self) -> bool {
        !std::mem::replace(&mut self.running, true)
    }

    pub fn finish(&mut self) {
        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }
}

/// Search order relative to the IDE executable: its folder, the parent folder,
/// then `<parent>/target/release` and `<parent>/target/debug`.
fn search_dirs(executable_dir: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![executable_dir.to_path_buf()];
    if let Some(parent) = executable_dir.parent() {
        dirs.push(parent.to_path_buf());
        dirs.push(parent.join("target").join("release"));
        dirs.push(parent.join("target").join("debug"));
    }
    dirs
}

fn find_in_search_dirs(executable_dir: Option<&Path>, file: &str) -> Option<PathBuf> {
    search_dirs(executable_dir?)
        .into_iter()
        .map(|directory| directory.join(file))
        .find(|candidate| candidate.is_file())
}

/// `nortexc.exe` relative to the IDE executable; falls back to `nortexc` on `PATH`.
pub fn compiler_path(executable_dir: Option<&Path>) -> PathBuf {
    find_in_search_dirs(executable_dir, "nortexc.exe").unwrap_or_else(|| PathBuf::from("nortexc"))
}

/// Mirrors the linker lookup: explicit override, then the executable directory and its parent.
pub fn runtime_library_path(
    executable_dir: Option<&Path>,
    override_path: Option<OsString>,
) -> Option<PathBuf> {
    if let Some(path) = override_path.map(PathBuf::from) {
        return path.is_file().then_some(path);
    }
    find_in_search_dirs(executable_dir, "nortex_runtime.lib")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolchainStatus {
    pub compiler: Option<PathBuf>,
    pub runtime_library: Option<PathBuf>,
}

impl ToolchainStatus {
    pub fn detect(executable_dir: Option<&Path>, runtime_override: Option<OsString>) -> Self {
        let compiler = compiler_path(executable_dir);
        Self {
            compiler: compiler.is_file().then_some(compiler),
            runtime_library: runtime_library_path(executable_dir, runtime_override),
        }
    }

    pub fn detect_current() -> Self {
        let executable = std::env::current_exe().ok();
        Self::detect(
            executable.as_deref().and_then(Path::parent),
            std::env::var_os("NORTEX_RUNTIME_LIB"),
        )
    }

    pub fn is_ready(&self) -> bool {
        self.compiler.is_some() && self.runtime_library.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("nortex-tc-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn program_exit_code_never_fails_the_build() {
        assert_eq!(classify_build(true, true), RunOutcome::Succeeded);
        assert_eq!(classify_build(false, true), RunOutcome::Failed);
        assert_eq!(classify_build(true, false), RunOutcome::Failed);
    }

    #[test]
    fn gate_rejects_second_build_until_finished() {
        let mut gate = RunGate::default();
        assert!(gate.try_begin());
        assert!(!gate.try_begin());
        assert!(!gate.try_begin());
        assert!(gate.is_running());
        gate.finish();
        assert!(gate.try_begin());
    }

    #[test]
    fn compiler_is_resolved_relative_to_the_executable_directory() {
        let dir = TempDir::new("compiler");
        assert_eq!(compiler_path(Some(&dir.0)), PathBuf::from("nortexc"));
        fs::write(dir.0.join("nortexc.exe"), b"x").unwrap();
        assert_eq!(compiler_path(Some(&dir.0)), dir.0.join("nortexc.exe"));
        assert_eq!(compiler_path(None), PathBuf::from("nortexc"));
    }

    #[test]
    fn runtime_library_is_found_next_to_or_above_the_executable() {
        let dir = TempDir::new("runtime");
        let bin = dir.0.join("bin");
        fs::create_dir_all(&bin).unwrap();
        assert_eq!(runtime_library_path(Some(&bin), None), None);
        fs::write(dir.0.join("nortex_runtime.lib"), b"x").unwrap();
        assert_eq!(
            runtime_library_path(Some(&bin), None),
            Some(dir.0.join("nortex_runtime.lib"))
        );
        fs::write(bin.join("nortex_runtime.lib"), b"x").unwrap();
        assert_eq!(
            runtime_library_path(Some(&bin), None),
            Some(bin.join("nortex_runtime.lib"))
        );
    }

    #[test]
    fn toolchain_is_found_in_parent_target_release_and_debug() {
        let dir = TempDir::new("target");
        let bin = dir.0.join("bin");
        let release = dir.0.join("target").join("release");
        let debug = dir.0.join("target").join("debug");
        for folder in [&bin, &release, &debug] {
            fs::create_dir_all(folder).unwrap();
        }
        fs::write(debug.join("nortexc.exe"), b"x").unwrap();
        fs::write(debug.join("nortex_runtime.lib"), b"x").unwrap();
        assert_eq!(compiler_path(Some(&bin)), debug.join("nortexc.exe"));
        fs::write(release.join("nortexc.exe"), b"x").unwrap();
        fs::write(release.join("nortex_runtime.lib"), b"x").unwrap();
        assert_eq!(compiler_path(Some(&bin)), release.join("nortexc.exe"));
        assert_eq!(
            runtime_library_path(Some(&bin), None),
            Some(release.join("nortex_runtime.lib"))
        );
    }

    #[test]
    fn invalid_override_does_not_fall_back_silently() {
        let dir = TempDir::new("override");
        fs::write(dir.0.join("nortex_runtime.lib"), b"x").unwrap();
        let missing = dir.0.join("missing.lib");
        assert_eq!(
            runtime_library_path(Some(&dir.0), Some(missing.into_os_string())),
            None
        );
    }

    #[test]
    fn status_is_ready_only_with_compiler_and_runtime() {
        let dir = TempDir::new("status");
        assert!(!ToolchainStatus::detect(Some(&dir.0), None).is_ready());
        fs::write(dir.0.join("nortexc.exe"), b"x").unwrap();
        fs::write(dir.0.join("nortex_runtime.lib"), b"x").unwrap();
        let status = ToolchainStatus::detect(Some(&dir.0), None);
        assert!(status.is_ready());
        assert_eq!(status.compiler, Some(dir.0.join("nortexc.exe")));
    }
}
