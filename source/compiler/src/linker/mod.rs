//! Native Windows x64 linking through the installed MSVC toolchain.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static TEMPORARY_OBJECT_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub struct LinkOptions {
    pub output_name: String,
    pub target: String,
}

pub fn link_object_file(object: &Path, output: &Path) -> Result<(), String> {
    link_object_file_with_optional_runtime(object, output, None, &[])
}

pub fn link_object_file_with_runtime(object: &Path, output: &Path) -> Result<(), String> {
    let runtime = discover_runtime_library()?;
    link_object_file_with_optional_runtime(object, output, Some(runtime.as_path()), &[])
}

fn link_object_file_with_optional_runtime(
    object: &Path,
    output: &Path,
    runtime: Option<&Path>,
    extra_args: &[String],
) -> Result<(), String> {
    #[cfg(not(windows))]
    {
        let _ = (object, output, runtime, extra_args);
        Err("Der MSVC-Linker ist nur auf Windows verfügbar.".to_owned())
    }

    #[cfg(windows)]
    {
        let toolchain = discover_msvc()?;
        let environment = msvc_environment(&toolchain)?;
        let mut command = Command::new(&toolchain.linker);
        command
            .envs(environment)
            .arg(object)
            .arg(format!("/OUT:{}", output.display()))
            .arg("/NOLOGO")
            .arg("/SUBSYSTEM:CONSOLE");
        if let Some(runtime) = runtime {
            command.arg(runtime);
        }
        command
            .arg("/DEFAULTLIB:libcmt.lib")
            .arg("/DEFAULTLIB:libvcruntime.lib")
            .arg("/DEFAULTLIB:libucrt.lib")
            .arg("/DEFAULTLIB:ws2_32.lib")
            .arg("/DEFAULTLIB:userenv.lib")
            .arg("/DEFAULTLIB:ntdll.lib")
            .arg("/DEFAULTLIB:kernel32.lib")
            .args(extra_args);

        let result = command
            .output()
            .map_err(|error| format!("MSVC-Linker konnte nicht gestartet werden: {error}"))?;
        if result.status.success() {
            Ok(())
        } else {
            let stdout = String::from_utf8_lossy(&result.stdout);
            let stderr = String::from_utf8_lossy(&result.stderr);
            Err(format!(
                "MSVC-Linker ist mit Exitcode {:?} fehlgeschlagen.\n{}{}",
                result.status.code(),
                stdout,
                stderr
            ))
        }
    }
}

pub fn link_object_bytes(object: &[u8], output: &Path) -> Result<(), String> {
    link_object_bytes_with_optional_runtime(object, output, None, &[])
}

pub fn link_object_bytes_with_runtime(object: &[u8], output: &Path) -> Result<(), String> {
    let runtime = discover_runtime_library()?;
    link_object_bytes_with_optional_runtime(object, output, Some(runtime.as_path()), &[])
}

/// Linkt mit optionaler Runtime und zusätzlichen Linker-Argumenten (z. B. aus der Projektdatei).
pub fn link_object_bytes_with_options(
    object: &[u8],
    output: &Path,
    with_runtime: bool,
    extra_args: &[String],
) -> Result<(), String> {
    let runtime = if with_runtime {
        Some(discover_runtime_library()?)
    } else {
        None
    };
    link_object_bytes_with_optional_runtime(object, output, runtime.as_deref(), extra_args)
}

fn link_object_bytes_with_optional_runtime(
    object: &[u8],
    output: &Path,
    runtime: Option<&Path>,
    extra_args: &[String],
) -> Result<(), String> {
    let path = create_temporary_object(object)?;
    let result = link_object_file_with_optional_runtime(&path, output, runtime, extra_args);
    let cleanup = std::fs::remove_file(&path).map_err(|error| {
        format!(
            "Temporäre COFF-Datei '{}' konnte nicht entfernt werden: {error}",
            path.display()
        )
    });
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
        (Err(error), Err(cleanup_error)) => Err(format!("{error}\n{cleanup_error}")),
    }
}

/// Workspace root of the source checkout; only known in debug/test builds so
/// release binaries do not embed the developer's local build path.
#[cfg(debug_assertions)]
pub(crate) fn dev_workspace_root() -> Option<PathBuf> {
    Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".."))
}

#[cfg(not(debug_assertions))]
pub(crate) fn dev_workspace_root() -> Option<PathBuf> {
    None
}

fn discover_runtime_library() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| {
        format!("Pfad des laufenden Compilers konnte nicht ermittelt werden: {error}")
    });
    let dev_workspace = dev_workspace_root();
    let workspace_target = dev_workspace
        .as_ref()
        .map(|root| root.join("target"))
        .unwrap_or_default();
    let fallback: Vec<PathBuf> = if dev_workspace.is_some() {
        vec![
            workspace_target.join("debug"),
            workspace_target.join("release"),
        ]
    } else {
        Vec::new()
    };
    let lookup = || {
        resolve_runtime_library(
            std::env::var_os("NORTEX_RUNTIME_LIB"),
            executable.as_ref().ok().and_then(|path| path.parent()),
            &fallback,
            executable.as_ref().err().cloned(),
        )
    };
    if std::env::var_os("NORTEX_RUNTIME_LIB").is_some() {
        return lookup();
    }
    let Some(workspace) = dev_workspace else {
        return lookup();
    };
    if !workspace.join("runtime").join("Cargo.toml").is_file() {
        return lookup();
    }
    match lookup() {
        Ok(library) => {
            if runtime_is_stale(&library, &workspace) {
                rebuild_runtime(&workspace, &library);
            }
            Ok(library)
        }
        Err(error) => {
            rebuild_runtime(&workspace, &workspace_target.join("debug"));
            lookup().map_err(|_| error)
        }
    }
}

fn newest_modification(path: &Path) -> Option<std::time::SystemTime> {
    let metadata = std::fs::metadata(path).ok()?;
    let mut newest = metadata.modified().ok();
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path).ok()?.flatten() {
            newest = newest.max(newest_modification(&entry.path()));
        }
    }
    newest
}

/// The library is stale when any runtime source is newer than the built artifact.
fn runtime_is_stale(library: &Path, workspace: &Path) -> bool {
    let Some(built) = std::fs::metadata(library)
        .ok()
        .and_then(|metadata| metadata.modified().ok())
    else {
        return false;
    };
    let runtime = workspace.join("runtime");
    [runtime.join("src"), runtime.join("Cargo.toml")]
        .iter()
        .filter_map(|path| newest_modification(path))
        .any(|modified| modified > built)
}

/// Best effort: a failed rebuild keeps the existing library, linking reports real problems.
fn rebuild_runtime(workspace: &Path, library: &Path) {
    let release = library
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == "release")
        || library.file_name().is_some_and(|name| name == "release");
    let mut command =
        std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command
        .args(["build", "-p", "nortex-runtime"])
        .current_dir(workspace)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if release {
        command.arg("--release");
    }
    let _ = command.status();
}

/// Pure lookup logic: override first, then the executable directory, its parent,
/// and finally the fallback directories.
fn resolve_runtime_library(
    override_path: Option<std::ffi::OsString>,
    executable_dir: Option<&Path>,
    fallback_dirs: &[PathBuf],
    executable_error: Option<String>,
) -> Result<PathBuf, String> {
    if let Some(path) = override_path {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!(
            "NORTEX_RUNTIME_LIB zeigt nicht auf eine Datei: '{}'.",
            path.display()
        ));
    }

    if let Some(error) = executable_error {
        return Err(error);
    }
    let mut directories = Vec::new();
    if let Some(parent) = executable_dir {
        directories.push(parent.to_path_buf());
        if let Some(workspace_target_dir) = parent.parent() {
            directories.push(workspace_target_dir.to_path_buf());
            directories.push(workspace_target_dir.join("target").join("release"));
            directories.push(workspace_target_dir.join("target").join("debug"));
        }
    }
    directories.extend(fallback_dirs.iter().cloned());
    directories.dedup();
    for directory in directories {
        let candidate = directory.join("nortex_runtime.lib");
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(
        "NORTEX Runtime-Import benötigt 'nortex_runtime.lib'. Baue zuerst `cargo build -p nortex-runtime` oder setze NORTEX_RUNTIME_LIB auf die Runtime-Bibliothek."
            .to_owned(),
    )
}

fn create_temporary_object(object: &[u8]) -> Result<PathBuf, String> {
    use std::io::Write;

    let directory = std::env::temp_dir();
    for _ in 0..100 {
        let id = TEMPORARY_OBJECT_ID.fetch_add(1, Ordering::Relaxed);
        let path = directory.join(format!("nortex-{}-{id}.obj", std::process::id()));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path);
        match file {
            Ok(mut file) => {
                if let Err(error) = file.write_all(object) {
                    let cleanup = std::fs::remove_file(&path);
                    return Err(match cleanup {
                        Ok(()) => {
                            format!("Temporäre COFF-Datei konnte nicht geschrieben werden: {error}")
                        }
                        Err(cleanup_error) => format!(
                            "Temporäre COFF-Datei konnte nicht geschrieben werden: {error}; die Teildatei '{}' konnte nicht entfernt werden: {cleanup_error}",
                            path.display()
                        ),
                    });
                }
                return Ok(path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(format!(
                    "Temporäre COFF-Datei '{}' konnte nicht angelegt werden: {error}",
                    path.display()
                ));
            }
        }
    }
    Err("Kein eindeutiger temporärer Name für die COFF-Datei verfügbar.".to_owned())
}

#[cfg(windows)]
struct MsvcToolchain {
    linker: PathBuf,
    vcvars: PathBuf,
}

#[cfg(windows)]
fn discover_msvc() -> Result<MsvcToolchain, String> {
    let vswhere = vswhere_path().ok_or_else(|| {
        "Visual Studio Installer 'vswhere.exe' wurde nicht gefunden. Installiere die MSVC-x64-Buildtools."
            .to_owned()
    })?;
    let installation = Command::new(&vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .map_err(|error| format!("vswhere.exe konnte nicht ausgeführt werden: {error}"))?;
    if !installation.status.success() {
        return Err(format!(
            "vswhere.exe konnte keine MSVC-x64-Installation finden (Exitcode {:?}).",
            installation.status.code()
        ));
    }
    let root = String::from_utf8_lossy(&installation.stdout)
        .trim()
        .to_owned();
    if root.is_empty() {
        return Err(
            "MSVC-x64-Buildtools fehlen. Installiere 'Desktop development with C++'.".to_owned(),
        );
    }

    let root = PathBuf::from(root);
    let vcvars = root
        .join("VC")
        .join("Auxiliary")
        .join("Build")
        .join("vcvars64.bat");
    if !vcvars.is_file() {
        return Err(format!("MSVC-Umgebungsskript fehlt: {}", vcvars.display()));
    }

    let linker_output = Command::new(&vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-find",
            "VC\\Tools\\MSVC\\**\\bin\\Hostx64\\x64\\link.exe",
        ])
        .output()
        .map_err(|error| format!("MSVC-Linkerpfad konnte nicht ermittelt werden: {error}"))?;
    if !linker_output.status.success() {
        return Err(format!(
            "vswhere.exe konnte link.exe nicht finden (Exitcode {:?}).",
            linker_output.status.code()
        ));
    }
    let linker = String::from_utf8_lossy(&linker_output.stdout)
        .lines()
        .map(str::trim)
        .find(|path| !path.is_empty() && Path::new(path).is_file())
        .map(PathBuf::from)
        .ok_or_else(|| {
            "MSVC-x64-Linker fehlt. Installiere die Komponente 'VC.Tools.x86.x64'.".to_owned()
        })?;

    Ok(MsvcToolchain { linker, vcvars })
}

#[cfg(windows)]
fn vswhere_path() -> Option<PathBuf> {
    [
        std::env::var_os("ProgramFiles(x86)")
            .map(|base| PathBuf::from(base).join("Microsoft Visual Studio/Installer/vswhere.exe")),
        std::env::var_os("ProgramFiles")
            .map(|base| PathBuf::from(base).join("Microsoft Visual Studio/Installer/vswhere.exe")),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.is_file())
}

/// Umgebungsvariablen, die `vcvars64.bat` für Compiler und Linker setzt und die sich zwischen
/// Läufen nicht ändern. `PATH` wird getrennt behandelt (nur die ergänzten Einträge).
#[cfg(windows)]
const CACHED_MSVC_VARIABLES: &[&str] = &[
    "INCLUDE",
    "LIB",
    "LIBPATH",
    "UCRTVersion",
    "UniversalCRTSdkDir",
    "WindowsSdkDir",
    "WindowsSDKVersion",
    "WindowsLibPath",
    "VCINSTALLDIR",
    "VCToolsInstallDir",
    "VSINSTALLDIR",
    "Platform",
];

/// Liefert die MSVC-Umgebung. `vcvars64.bat` kostet pro Aufruf rund eine Sekunde; das Ergebnis
/// wird daher je Benutzer zwischengespeichert und über Größe und Änderungszeit von `vcvars64.bat`
/// und `link.exe` ungültig gemacht. Fehler beim Cache führen immer zum normalen Ablauf.
#[cfg(windows)]
fn msvc_environment(toolchain: &MsvcToolchain) -> Result<HashMap<String, String>, String> {
    let cache_file = msvc_cache_path();
    let fingerprint = msvc_fingerprint(toolchain);
    if let (Some(path), Some(fingerprint)) = (&cache_file, &fingerprint)
        && let Some(cached) = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| parse_msvc_cache(&text, fingerprint))
    {
        return Ok(apply_cached_environment(&cached));
    }
    let captured = capture_msvc_environment(&toolchain.vcvars)?;
    if let (Some(path), Some(fingerprint)) = (cache_file, fingerprint) {
        let text = render_msvc_cache(&fingerprint, &captured);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let temporary = path.with_extension("tmp");
        if std::fs::write(&temporary, text).is_ok() && std::fs::rename(&temporary, &path).is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
    }
    Ok(captured)
}

#[cfg(windows)]
fn msvc_cache_path() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(
        PathBuf::from(base)
            .join("NORTEX Code")
            .join("msvc-environment.cache"),
    )
}

#[cfg(windows)]
fn msvc_fingerprint(toolchain: &MsvcToolchain) -> Option<String> {
    let describe = |path: &Path| -> Option<String> {
        let meta = std::fs::metadata(path).ok()?;
        let modified = meta
            .modified()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        Some(format!("{}|{}|{}", path.display(), meta.len(), modified))
    };
    Some(format!(
        "{}|{}",
        describe(&toolchain.vcvars)?,
        describe(&toolchain.linker)?
    ))
}

/// Zwischenspeicher: erste Zeile Fingerabdruck, danach `NAME=Wert`; `PATH+=` enthält die von
/// `vcvars64.bat` vorangestellten Einträge.
#[cfg(windows)]
fn render_msvc_cache(fingerprint: &str, captured: &HashMap<String, String>) -> String {
    let mut text = format!("{fingerprint}\n");
    for name in CACHED_MSVC_VARIABLES {
        if let Some((_, value)) = captured
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
        {
            text.push_str(&format!("{name}={value}\n"));
        }
    }
    let baseline = std::env::var("PATH").unwrap_or_default();
    if let Some((_, path)) = captured
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case("PATH"))
    {
        let added: Vec<&str> = path
            .split(';')
            .filter(|entry| !entry.is_empty() && !baseline.split(';').any(|known| known == *entry))
            .collect();
        text.push_str(&format!("PATH+={}\n", added.join(";")));
    }
    text
}

#[cfg(windows)]
fn parse_msvc_cache(text: &str, fingerprint: &str) -> Option<Vec<(String, String)>> {
    let mut lines = text.lines();
    if lines.next()? != fingerprint {
        return None;
    }
    let mut entries = Vec::new();
    for line in lines {
        let (key, value) = line.split_once('=')?;
        entries.push((key.to_owned(), value.to_owned()));
    }
    // Ohne LIB oder INCLUDE ist der Eintrag unbrauchbar; dann neu erfassen.
    let has = |name: &str| {
        entries
            .iter()
            .any(|(key, value)| key == name && !value.is_empty())
    };
    (has("LIB") && has("INCLUDE")).then_some(entries)
}

#[cfg(windows)]
fn apply_cached_environment(entries: &[(String, String)]) -> HashMap<String, String> {
    let mut environment: HashMap<String, String> = std::env::vars().collect();
    for (key, value) in entries {
        if key == "PATH+" {
            let current = std::env::var("PATH").unwrap_or_default();
            let joined = if value.is_empty() {
                current
            } else {
                format!("{value};{current}")
            };
            environment.retain(|name, _| !name.eq_ignore_ascii_case("PATH"));
            environment.insert("PATH".to_owned(), joined);
        } else {
            environment.insert(key.clone(), value.clone());
        }
    }
    environment
}

#[cfg(windows)]
fn capture_msvc_environment(vcvars: &Path) -> Result<HashMap<String, String>, String> {
    let vcvars = short_path(vcvars)?;
    let script = format!("call {} >nul && set", vcvars.to_string_lossy());
    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into());
    let cmd_exe = Path::new(&system_root).join("System32").join("cmd.exe");
    let output = Command::new(cmd_exe)
        .args(["/d", "/c", &script])
        .output()
        .map_err(|error| format!("MSVC-Umgebung konnte nicht initialisiert werden: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "vcvars64.bat ist mit Exitcode {:?} fehlgeschlagen.\n{}{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            (!key.is_empty() && !key.starts_with('=')).then(|| (key.to_owned(), value.to_owned()))
        })
        .collect())
}

#[cfg(windows)]
fn short_path(path: &Path) -> Result<PathBuf, String> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use std::ptr;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetShortPathNameW(
            long_path: *const u16,
            short_path: *mut u16,
            buffer_length: u32,
        ) -> u32;
    }

    let input = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // cmd.exe parses quotes differently from normal Windows process arguments.
    // Use the short path form so the batch-file path needs no shell quoting.
    let required = unsafe { GetShortPathNameW(input.as_ptr(), ptr::null_mut(), 0) };
    if required == 0 {
        return Err(format!(
            "MSVC-Umgebungspfad '{}' konnte nicht für cmd.exe aufbereitet werden: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }

    let mut buffer = vec![0; required as usize];
    let written =
        unsafe { GetShortPathNameW(input.as_ptr(), buffer.as_mut_ptr(), buffer.len() as u32) };
    if written == 0 || written as usize >= buffer.len() {
        return Err(format!(
            "MSVC-Kurzpfad für '{}' konnte nicht ermittelt werden.",
            path.display()
        ));
    }
    buffer.truncate(written as usize);
    let short_path = PathBuf::from(std::ffi::OsString::from_wide(&buffer));
    if short_path
        .to_string_lossy()
        .chars()
        .any(char::is_whitespace)
    {
        return Err(format!(
            "MSVC-Umgebungspfad '{}' enthält Leerzeichen, aber Windows liefert keinen shell-sicheren Kurzpfad.",
            path.display()
        ));
    }
    Ok(short_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn msvc_cache_roundtrip_and_invalidation() {
        let mut captured = HashMap::new();
        captured.insert("LIB".to_owned(), "C:\\lib".to_owned());
        captured.insert("INCLUDE".to_owned(), "C:\\inc".to_owned());
        let text = render_msvc_cache("fp1", &captured);
        let entries = parse_msvc_cache(&text, "fp1").expect("gültiger Cache");
        assert!(entries.iter().any(|(k, v)| k == "LIB" && v == "C:\\lib"));
        assert!(parse_msvc_cache(&text, "fp2").is_none());
        assert!(parse_msvc_cache("fp1\nPATH+=x\n", "fp1").is_none());
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "nortex-linker-{tag}-{}-{}",
                std::process::id(),
                TEMPORARY_OBJECT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).expect("test directory should be created");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn finds_runtime_next_to_executable() {
        let dir = TempDir::new("next-to-exe");
        let lib = dir.0.join("nortex_runtime.lib");
        std::fs::write(&lib, b"x").unwrap();
        assert_eq!(
            resolve_runtime_library(None, Some(&dir.0), &[], None),
            Ok(lib)
        );
    }

    #[test]
    fn finds_runtime_in_parent_of_executable_directory() {
        let dir = TempDir::new("parent");
        let exe_dir = dir.0.join("deps");
        std::fs::create_dir_all(&exe_dir).unwrap();
        let lib = dir.0.join("nortex_runtime.lib");
        std::fs::write(&lib, b"x").unwrap();
        assert_eq!(
            resolve_runtime_library(None, Some(&exe_dir), &[], None),
            Ok(lib)
        );
    }

    #[test]
    fn override_wins_over_executable_directory() {
        let dir = TempDir::new("override");
        std::fs::write(dir.0.join("nortex_runtime.lib"), b"x").unwrap();
        let custom = dir.0.join("custom.lib");
        std::fs::write(&custom, b"y").unwrap();
        assert_eq!(
            resolve_runtime_library(Some(custom.clone().into()), Some(&dir.0), &[], None),
            Ok(custom)
        );
    }

    #[test]
    fn override_pointing_to_non_file_is_a_clean_error_without_fallback() {
        let dir = TempDir::new("override-bad");
        std::fs::write(dir.0.join("nortex_runtime.lib"), b"x").unwrap();
        let error = resolve_runtime_library(Some(dir.0.clone().into()), Some(&dir.0), &[], None)
            .unwrap_err();
        assert!(error.contains("NORTEX_RUNTIME_LIB"), "{error}");
    }

    #[test]
    fn missing_runtime_reports_actionable_message() {
        let dir = TempDir::new("missing");
        let error =
            resolve_runtime_library(None, Some(&dir.0), &[dir.0.join("none")], None).unwrap_err();
        assert!(error.contains("nortex_runtime.lib"), "{error}");
        assert!(error.contains("cargo build -p nortex-runtime"), "{error}");
        assert!(error.contains("NORTEX_RUNTIME_LIB"), "{error}");
    }

    #[test]
    fn fallback_directories_are_searched_last() {
        let dir = TempDir::new("fallback");
        let fallback = dir.0.join("fb");
        std::fs::create_dir_all(&fallback).unwrap();
        let lib = fallback.join("nortex_runtime.lib");
        std::fs::write(&lib, b"x").unwrap();
        assert_eq!(
            resolve_runtime_library(None, None, &[fallback], None),
            Ok(lib)
        );
    }
}
