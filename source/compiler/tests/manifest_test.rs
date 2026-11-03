use std::path::Path;

use nortex_compiler::manifest::{ManifestError, NortexManifest, find_manifest};

const FULL: &str = r#"
[package]
name = "MyNortexApp"
version = "0.2.0"
authors = ["Example Author"]
description = "Nortex Application"

[build]
main = "src/app.nx"
output_name = "custom_app.exe"
target = "x86_64-pc-windows-msvc"
overflow_checks = true
opt_level = 2
linker_flags = ["/SUBSYSTEM:CONSOLE"]

[dependencies]
nortex_std = { path = "../standard-library" }
native_sys = { lib = "user32.lib" }
"#;

fn parse(text: &str) -> Result<NortexManifest, ManifestError> {
    NortexManifest::parse(text, Path::new("proj/nortex.toml"))
}

#[test]
fn parses_full_manifest() {
    let manifest = parse(FULL).unwrap();
    assert_eq!(manifest.package.name, "MyNortexApp");
    assert_eq!(manifest.package.authors, ["Example Author"]);
    assert_eq!(manifest.build.opt_level, 2);
    assert_eq!(manifest.build.overflow_checks, Some(true));
    assert_eq!(manifest.output_file_name(), "custom_app.exe");
    assert_eq!(manifest.dependencies.len(), 2);
    let manifest_path = Path::new("proj/nortex.toml");
    assert_eq!(
        manifest.main_path(manifest_path),
        Path::new("proj/src/app.nx")
    );
    assert_eq!(
        manifest.default_output(manifest_path),
        Path::new("proj/build/custom_app.exe")
    );
    let args = manifest.linker_args(manifest_path);
    assert!(args[0].starts_with("/LIBPATH:") && args[0].ends_with("standard-library"));
    assert_eq!(&args[1..], ["user32.lib", "/SUBSYSTEM:CONSOLE"]);
}

#[test]
fn minimal_manifest_uses_defaults() {
    let manifest = parse("[package]\nname = \"Tiny\"\n").unwrap();
    assert_eq!(manifest.package.version, "0.1.0");
    assert_eq!(manifest.build.main, "src/main.nx");
    assert_eq!(manifest.build.target, "x86_64-pc-windows-msvc");
    assert_eq!(manifest.build.opt_level, 0);
    assert_eq!(manifest.build.overflow_checks, None);
    assert!(manifest.build.linker_flags.is_empty());
    assert!(manifest.dependencies.is_empty());
    assert_eq!(manifest.output_file_name(), "Tiny.exe");
}

#[test]
fn broken_toml_reports_parse_error() {
    let error = parse("[package\nname = ").unwrap_err();
    assert!(matches!(error, ManifestError::Parse { .. }));
    assert!(error.to_string().contains("nortex.toml"));
}

#[test]
fn missing_fields_and_invalid_values_are_reported() {
    let missing = parse("[build]\nmain = \"a.nx\"\n").unwrap_err();
    assert!(matches!(missing, ManifestError::Parse { .. }));
    assert!(missing.to_string().contains("package"));
    let no_name = parse("[package]\nversion = \"1\"\n").unwrap_err();
    assert!(no_name.to_string().contains("name"));
    let empty = parse("[package]\nname = \" \"\n").unwrap_err();
    assert!(matches!(empty, ManifestError::Invalid { .. }));
    let target = parse("[package]\nname = \"a\"\n[build]\ntarget = \"arm\"\n").unwrap_err();
    assert!(target.to_string().contains("target"));
    let opt = parse("[package]\nname = \"a\"\n[build]\nopt_level = 9\n").unwrap_err();
    assert!(opt.to_string().contains("opt_level"));
    let dep = parse("[package]\nname = \"a\"\n[dependencies]\nx = {}\n").unwrap_err();
    assert!(dep.to_string().contains("Abhängigkeit 'x'"));
}

#[test]
fn load_from_file_and_discovery() {
    let dir = std::env::temp_dir().join(format!("nortex-manifest-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let manifest_path = dir.join("nortex.toml");
    std::fs::write(&manifest_path, "[package]\nname = \"Disk\"\n").unwrap();
    std::fs::write(dir.join("src/main.nx"), "").unwrap();
    std::fs::write(dir.join("src/util.nx"), "").unwrap();

    let manifest = NortexManifest::load_from_file(&manifest_path).unwrap();
    assert_eq!(manifest.package.name, "Disk");
    assert_eq!(find_manifest(&dir), Some(manifest_path.clone()));
    assert_eq!(find_manifest(&manifest_path), Some(manifest_path.clone()));
    assert_eq!(manifest.source_files(&manifest_path).len(), 2);
    assert!(find_manifest(&dir.join("src")).is_none());

    let missing = NortexManifest::load_from_file(&dir.join("nope.toml")).unwrap_err();
    assert!(matches!(missing, ManifestError::Io { .. }));
    std::fs::remove_dir_all(&dir).unwrap();
}
