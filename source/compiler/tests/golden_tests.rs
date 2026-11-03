//! Golden tests: compile `examples/language/*.nx`, run the EXE and compare stdout and exit code.
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("examples")
        .join("language")
}

fn runtime_lib() -> Option<PathBuf> {
    let exe_dir = Path::new(env!("CARGO_BIN_EXE_nortexc")).parent()?;
    [exe_dir, exe_dir.parent()?]
        .into_iter()
        .map(|dir| dir.join("nortex_runtime.lib"))
        .find(|path| path.is_file())
}

fn run_example(name: &str, expected_stdout: &str, expected_exit: i32) {
    let Some(lib) = runtime_lib() else {
        eprintln!("skipped: nortex_runtime.lib not built");
        return;
    };
    let out_dir = std::env::temp_dir().join(format!("nortex-golden-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let exe = out_dir.join("example.exe");
    let build = Command::new(env!("CARGO_BIN_EXE_nortexc"))
        .arg("build")
        .arg(examples_dir().join(format!("{name}.nx")))
        .arg("-o")
        .arg(&exe)
        .env("NORTEX_RUNTIME_LIB", &lib)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "{name}: build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(&exe).output().unwrap();
    let _ = std::fs::remove_dir_all(&out_dir);
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        expected_stdout,
        "{name}: stdout"
    );
    assert_eq!(run.status.code(), Some(expected_exit), "{name}: exit code");
}

fn run_project(dir: &str, expected_exit: i32) {
    let out_dir = std::env::temp_dir().join(format!("nortex-golden-{}-{dir}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let exe = out_dir.join("project.exe");
    let mut command = Command::new(env!("CARGO_BIN_EXE_nortexc"));
    command
        .arg("build")
        .arg(examples_dir().join(dir))
        .arg("-o")
        .arg(&exe);
    if let Some(lib) = runtime_lib() {
        command.env("NORTEX_RUNTIME_LIB", lib);
    }
    let build = command.output().unwrap();
    assert!(
        build.status.success(),
        "{dir}: build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );
    let run = Command::new(&exe).output().unwrap();
    let _ = std::fs::remove_dir_all(&out_dir);
    assert_eq!(run.status.code(), Some(expected_exit), "{dir}: exit code");
}

#[test]
fn multi_file_project_links_imported_module() {
    run_project("03_multi_file_project", 42);
}

#[test]
fn win32_demo_calls_extern_c_functions() {
    run_project("04_win32_demo", 0);
}

#[test]
fn overflow_example_traps() {
    run_project("02_math_and_overflows", 0xC000001Du32 as i32);
}

#[test]
fn hello_prints_greeting() {
    run_example("01_hello", "Hallo, NORTEX!\n", 0);
}

#[test]
fn loops_return_sum_plus_counter() {
    run_example("04_loops", "", 35);
}

#[test]
fn functions_return_gcd_plus_fib() {
    run_example("05_functions", "", 61);
}
