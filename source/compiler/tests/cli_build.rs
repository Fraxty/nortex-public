use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

const OVERFLOW_SOURCE: &str = "public int add(int a, int b) { return a + b; } public int main() { if (add(2147483647, 1) < 0) { return 7; } return 9; }";
const CONSOLE_SOURCE: &str =
    "using System; public int main() { Console.writeLine(\"hi\"); return 0; }";
const ILLEGAL_INSTRUCTION: i32 = 0xC000_001Du32 as i32;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Unique per-test directory that is removed on drop, even when an assertion fails.
struct TestDir(PathBuf);

impl TestDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "nortex-cli-build-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let path = path.join(name);
        std::fs::create_dir_all(&path).expect("test directory should be created");
        Self(path)
    }

    fn write(&self, name: &str, contents: impl AsRef<[u8]>) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, contents).expect("test file should be written");
        path
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        if let Some(root) = self.0.parent() {
            let _ = std::fs::remove_dir_all(root);
        }
    }
}

fn nortexc() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nortexc"))
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_clean_failure(output: &Output) {
    let text = stderr(output);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected a regular build failure, got {:?}: {text}",
        output.status
    );
    assert!(!text.contains("panicked"), "compiler panicked: {text}");
}

fn real_runtime_lib() -> Option<PathBuf> {
    let exe_dir = Path::new(env!("CARGO_BIN_EXE_nortexc")).parent()?;
    [exe_dir, exe_dir.parent()?]
        .into_iter()
        .map(|dir| dir.join("nortex_runtime.lib"))
        .find(|path| path.is_file())
}

#[test]
fn overflow_checks_with_unknown_value_is_a_clean_usage_error() {
    let dir = TestDir::new("flag");
    let source = dir.write("main.nx", OVERFLOW_SOURCE);
    let output = nortexc()
        .arg("build")
        .arg(&source)
        .arg("--overflow-checks=maybe")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let text = stderr(&output);
    assert!(text.contains("--overflow-checks=maybe"), "{text}");
    assert!(!text.contains("panicked"), "{text}");
    assert!(!dir.0.join("main.exe").exists());
}

#[test]
fn failed_frontend_build_returns_nonzero_and_writes_no_output() {
    let dir = TestDir::new("frontend-error");
    let source = dir.write("bad.nx", "public int main( { return 0; }");
    let output = nortexc().arg("build").arg(&source).output().unwrap();
    assert_clean_failure(&output);
    assert!(!dir.0.join("bad.exe").exists());
}

#[test]
fn missing_source_file_returns_usage_exit_code_without_panic() {
    let dir = TestDir::new("missing-source");
    let output = nortexc()
        .arg("build")
        .arg(dir.0.join("nope.nx"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!stderr(&output).contains("panicked"));
}

#[test]
fn object_only_build_succeeds_with_overflow_flag_values() {
    let dir = TestDir::new("obj");
    let source = dir.write("main.nx", OVERFLOW_SOURCE);
    for flag in [
        "--overflow-checks",
        "--overflow-checks=true",
        "--overflow-checks=false",
    ] {
        let output = nortexc()
            .arg("build")
            .arg(&source)
            .arg("--emit-obj")
            .arg(flag)
            .output()
            .unwrap();
        assert!(output.status.success(), "{flag}: {}", stderr(&output));
        assert!(dir.0.join("main.obj").is_file());
        std::fs::remove_file(dir.0.join("main.obj")).unwrap();
    }
}

#[cfg(windows)]
mod windows {
    use super::*;

    fn build_and_run(dir: &TestDir, source: &str, flags: &[&str]) -> Option<i32> {
        let source = dir.write("main.nx", source);
        let exe = dir.0.join("main.exe");
        let build = nortexc()
            .arg("build")
            .arg(&source)
            .args(flags)
            .output()
            .unwrap();
        assert!(build.status.success(), "build failed: {}", stderr(&build));
        Command::new(&exe).status().unwrap().code()
    }

    #[test]
    fn overflow_checks_true_traps_with_illegal_instruction() {
        for flags in [&["--overflow-checks"][..], &["--overflow-checks=true"]] {
            let dir = TestDir::new("trap");
            assert_eq!(
                build_and_run(&dir, OVERFLOW_SOURCE, flags),
                Some(ILLEGAL_INSTRUCTION)
            );
        }
    }

    #[test]
    fn overflow_checks_false_wraps_around() {
        let dir = TestDir::new("wrap");
        assert_eq!(
            build_and_run(&dir, OVERFLOW_SOURCE, &["--overflow-checks=false"]),
            Some(7)
        );
    }

    #[test]
    fn overflow_checks_absent_follows_build_profile_default() {
        let dir = TestDir::new("default");
        let expected = if cfg!(debug_assertions) {
            ILLEGAL_INSTRUCTION
        } else {
            7
        };
        assert_eq!(build_and_run(&dir, OVERFLOW_SOURCE, &[]), Some(expected));
    }

    #[test]
    fn program_exit_code_does_not_make_the_build_fail() {
        let dir = TestDir::new("exit55");
        assert_eq!(
            build_and_run(&dir, "public int main() { return 55; }", &[]),
            Some(55)
        );
    }

    #[test]
    fn missing_runtime_library_override_is_a_clean_error() {
        let dir = TestDir::new("missing-lib");
        let source = dir.write("main.nx", CONSOLE_SOURCE);
        let output = nortexc()
            .arg("build")
            .arg(&source)
            .env("NORTEX_RUNTIME_LIB", dir.0.join("absent.lib"))
            .output()
            .unwrap();
        assert_clean_failure(&output);
        assert!(stderr(&output).contains("NORTEX_RUNTIME_LIB"));
        assert!(!dir.0.join("main.exe").exists());
    }

    #[test]
    fn empty_or_corrupt_runtime_library_fails_cleanly() {
        for (name, contents) in [
            ("empty.lib", &b""[..]),
            ("garbage.lib", &b"this is not a COFF archive"[..]),
        ] {
            let dir = TestDir::new("bad-lib");
            let source = dir.write("main.nx", CONSOLE_SOURCE);
            let lib = dir.write(name, contents);
            let output = nortexc()
                .arg("build")
                .arg(&source)
                .env("NORTEX_RUNTIME_LIB", &lib)
                .output()
                .unwrap();
            assert_clean_failure(&output);
            assert!(stderr(&output).contains("Linken"), "{}", stderr(&output));
            assert!(!dir.0.join("main.exe").exists());
        }
    }

    #[test]
    fn build_works_in_paths_with_spaces_and_umlauts() {
        let Some(real_lib) = real_runtime_lib() else {
            eprintln!("skipped: nortex_runtime.lib not built");
            return;
        };
        let dir = TestDir::new("Prüfung ÄÖÜ mit Leerzeichen");
        let lib = dir.0.join("nortex runtime ß.lib");
        std::fs::copy(&real_lib, &lib).unwrap();
        let source = dir.write("quelle ü.nx", CONSOLE_SOURCE);
        let exe = dir.0.join("ausgabe ö.exe");
        let build = nortexc()
            .arg("build")
            .arg(&source)
            .arg("-o")
            .arg(&exe)
            .env("NORTEX_RUNTIME_LIB", &lib)
            .output()
            .unwrap();
        assert!(build.status.success(), "build failed: {}", stderr(&build));
        let run = Command::new(&exe).output().unwrap();
        assert_eq!(run.status.code(), Some(0));
        assert_eq!(run.stdout, b"hi\n");
    }
}
