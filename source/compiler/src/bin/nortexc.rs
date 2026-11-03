use std::env;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use nortex_compiler::codegen::{CodeGenerator, Target};
use nortex_compiler::diagnostics::Diagnostic;
use nortex_compiler::diagnostics::locale::{Locale, set_locale};
use nortex_compiler::diagnostics::renderer::DiagnosticRenderer;
use nortex_compiler::driver::{check_file, compile_file_to_mir};
use nortex_compiler::linker::link_object_bytes_with_options;
use nortex_compiler::manifest::{NortexManifest, find_manifest};

fn main() -> ExitCode {
    let mut arguments = extract_language(env::args().skip(1).collect()).into_iter();
    let command = arguments.next();

    match command.as_deref() {
        Some("--version") | Some("-V") => {
            println!("NORTEX Compiler {}", nortex_compiler::VERSION);
            ExitCode::SUCCESS
        }
        Some("--help") | Some("-h") => {
            print_usage();
            ExitCode::SUCCESS
        }
        Some("build") => build_command(arguments),
        Some("check") => {
            let Some(path) = arguments.next() else {
                eprintln!("Verwendung: nortexc check <datei.nx>");
                return ExitCode::from(2);
            };
            let path = match load_manifest_build(&path) {
                Ok(Some(manifest)) => manifest.main,
                Ok(None) => path,
                Err(error) => {
                    eprintln!("{error}");
                    return ExitCode::from(2);
                }
            };
            match check_file(&path) {
                Ok(result) => {
                    print_diagnostics(&result.diagnostics, &path);
                    if result.has_errors() {
                        eprintln!(
                            "NORTEX: Prüfung fehlgeschlagen ({} Tokens).",
                            result.token_count
                        );
                        ExitCode::FAILURE
                    } else {
                        println!(
                            "NORTEX: Syntax und grundlegende Semantik geprüft ({} Tokens).",
                            result.token_count
                        );
                        ExitCode::SUCCESS
                    }
                }
                Err(error) => {
                    eprintln!("Quelldatei konnte nicht gelesen werden: {error}");
                    ExitCode::from(2)
                }
            }
        }
        Some("mir") => {
            let Some(path) = arguments.next() else {
                eprintln!("Verwendung: nortexc mir <datei.nx>");
                return ExitCode::from(2);
            };
            match compile_file_to_mir(&path) {
                Ok(result) => {
                    print_diagnostics(&result.diagnostics, &path);
                    if result.has_errors() {
                        return ExitCode::FAILURE;
                    }
                    for function in &result.functions {
                        println!("MIR {}:\n{:#?}", function.name, function.body);
                    }
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("Quelldatei konnte nicht gelesen werden: {error}");
                    ExitCode::from(2)
                }
            }
        }
        Some(path) => build_command(std::iter::once(path.to_owned()).chain(arguments)),
        None => {
            print_usage();
            ExitCode::from(2)
        }
    }
}

/// Entfernt `--lang <de|en>` aus den Argumenten und setzt die Meldungssprache.
fn extract_language(arguments: Vec<String>) -> Vec<String> {
    let mut rest = Vec::with_capacity(arguments.len());
    let mut iter = arguments.into_iter();
    while let Some(argument) = iter.next() {
        if argument == "--lang" {
            if let Some(locale) = iter.next().and_then(|value| Locale::parse(&value)) {
                set_locale(locale);
            }
        } else {
            rest.push(argument);
        }
    }
    rest
}

struct ManifestBuild {
    main: String,
    output: PathBuf,
    overflow_checks: Option<bool>,
    link_args: Vec<String>,
}

/// `Ok(None)`, wenn das Argument weder ein Projektordner noch eine `nortex.toml` ist.
fn load_manifest_build(argument: &str) -> Result<Option<ManifestBuild>, String> {
    let Some(path) = find_manifest(Path::new(argument)) else {
        return Ok(None);
    };
    let manifest = NortexManifest::load_from_file(&path).map_err(|error| error.to_string())?;
    Ok(Some(ManifestBuild {
        main: manifest.main_path(&path).display().to_string(),
        output: manifest.default_output(&path),
        overflow_checks: manifest.build.overflow_checks,
        link_args: manifest.linker_args(&path),
    }))
}

fn print_usage() {
    eprintln!("NORTEX Compiler {}", nortex_compiler::VERSION);
    eprintln!(
        "Verwendung: nortexc build <datei.nx> [-o|--output <datei.exe|datei.obj>] [--emit-obj] [--overflow-checks[=true|false]] | nortexc <datei.nx> [-o|--output <datei.exe|datei.obj>] | check <datei.nx> | mir <datei.nx> | --version"
    );
}

fn build_command(mut arguments: impl Iterator<Item = String>) -> ExitCode {
    let Some(input) = arguments.next() else {
        eprintln!(
            "Verwendung: nortexc build <datei.nx> [-o|--output <datei.exe|datei.obj>] [--emit-obj] [--overflow-checks[=true|false]]"
        );
        return ExitCode::from(2);
    };

    let mut arguments: Vec<String> = arguments.collect();
    let manifest = match load_manifest_build(&input) {
        Ok(manifest) => manifest,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let input = manifest
        .as_ref()
        .map_or(input, |manifest| manifest.main.clone());
    let link_args = manifest
        .as_ref()
        .map(|manifest| manifest.link_args.clone())
        .unwrap_or_default();
    if let Some(manifest) = &manifest {
        if !arguments.iter().any(|a| a == "-o" || a == "--output") {
            arguments.push("-o".into());
            arguments.push(manifest.output.display().to_string());
        }
        if let Some(value) = manifest.overflow_checks
            && !arguments.iter().any(|a| a.starts_with("--overflow-checks"))
        {
            arguments.push(format!("--overflow-checks={value}"));
        }
    }

    let (output, emit_obj_only, overflow_checks) =
        match parse_build_options(&input, arguments.into_iter()) {
            Ok(options) => options,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::from(2);
            }
        };
    let result = match compile_file_to_mir(&input) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("Quelldatei konnte nicht gelesen werden: {error}");
            return ExitCode::from(2);
        }
    };
    print_diagnostics(&result.diagnostics, &input);
    if result.has_errors() {
        return ExitCode::FAILURE;
    }

    let object = match CodeGenerator::new(Target::X86_64Windows)
        .with_overflow_checks(overflow_checks.unwrap_or(cfg!(debug_assertions)))
        .emit_object(&result.functions)
    {
        Ok(object) => object,
        Err(error) => {
            eprintln!("Codegenerierung fehlgeschlagen: {error}");
            return ExitCode::FAILURE;
        }
    };

    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        eprintln!(
            "Ausgabeverzeichnis '{}' konnte nicht angelegt werden: {error}",
            parent.display()
        );
        return ExitCode::FAILURE;
    }

    match output.extension().and_then(|extension| extension.to_str()) {
        Some(extension)
            if emit_obj_only
                || extension.eq_ignore_ascii_case("obj")
                || extension.eq_ignore_ascii_case("o") =>
        {
            if let Err(error) = std::fs::write(&output, object) {
                eprintln!(
                    "COFF-Objekt '{}' konnte nicht geschrieben werden: {error}",
                    output.display()
                );
                return ExitCode::FAILURE;
            }
        }
        Some(extension) if extension.eq_ignore_ascii_case("exe") => {
            let link_result = link_object_bytes_with_options(
                &object,
                &output,
                CodeGenerator::requires_runtime(&result.functions),
                &link_args,
            );
            if let Err(error) = link_result {
                eprintln!("Linken nach '{}' fehlgeschlagen: {error}", output.display());
                return ExitCode::FAILURE;
            }
        }
        _ => {
            eprintln!(
                "Nicht unterstütztes Ausgabeformat für '{}'; erwartet wird .obj oder .exe.",
                output.display()
            );
            return ExitCode::from(2);
        }
    }

    println!("NORTEX: Ausgabe geschrieben: {}", output.display());
    ExitCode::SUCCESS
}

fn parse_build_options(
    input: &str,
    mut arguments: impl Iterator<Item = String>,
) -> Result<(PathBuf, bool, Option<bool>), String> {
    let mut output = None;
    let mut emit_obj_only = false;
    let mut overflow_checks = None;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-o" | "--output" => {
                if output.is_some() {
                    return Err("Die Option -o/--output darf nur einmal angegeben werden.".into());
                }
                let Some(path) = arguments.next() else {
                    return Err("Nach -o/--output muss ein Ausgabepfad stehen.".into());
                };
                output = Some(PathBuf::from(path));
            }
            "--emit-obj" => {
                if emit_obj_only {
                    return Err("Die Option --emit-obj darf nur einmal angegeben werden.".into());
                }
                emit_obj_only = true;
            }
            "--overflow-checks" | "--overflow-checks=true" => overflow_checks = Some(true),
            "--overflow-checks=false" => overflow_checks = Some(false),
            _ => return Err(format!("Unbekanntes Build-Argument: {argument}")),
        }
    }

    let output = output.unwrap_or_else(|| {
        Path::new(input).with_extension(if emit_obj_only { "obj" } else { "exe" })
    });
    if emit_obj_only
        && !matches!(
            output.extension().and_then(|extension| extension.to_str()),
            Some(extension)
                if extension.eq_ignore_ascii_case("obj") || extension.eq_ignore_ascii_case("o")
        )
    {
        return Err("--emit-obj erfordert einen Ausgabepfad mit .obj oder .o.".into());
    }
    Ok((output, emit_obj_only, overflow_checks))
}

fn print_diagnostics(diagnostics: &[Diagnostic], source_path: &str) {
    if diagnostics.is_empty() {
        return;
    }

    let source = match std::fs::read_to_string(source_path) {
        Ok(source) => Some(source),
        Err(error) => {
            eprintln!("Quelldatei für Diagnose-Snippets konnte nicht gelesen werden: {error}");
            None
        }
    };
    let color = std::io::stderr().is_terminal() && env::var_os("NO_COLOR").is_none();
    let renderer = DiagnosticRenderer::new(color);
    for diagnostic in diagnostics {
        eprint!("{}", renderer.render(diagnostic, source.as_deref()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> std::vec::IntoIter<String> {
        values
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn accepts_output_alias() {
        let (output, emit_obj_only, _) =
            parse_build_options("main.nx", arguments(&["--output", "app.exe"])).unwrap();

        assert_eq!(output, PathBuf::from("app.exe"));
        assert!(!emit_obj_only);
    }

    #[test]
    fn emits_object_by_default_with_emit_obj() {
        let (output, emit_obj_only, _) =
            parse_build_options("main.nx", arguments(&["--emit-obj"])).unwrap();

        assert_eq!(output, PathBuf::from("main.obj"));
        assert!(emit_obj_only);
    }

    #[test]
    fn rejects_executable_path_with_emit_obj() {
        let result = parse_build_options("main.nx", arguments(&["--emit-obj", "-o", "app.exe"]));

        assert!(result.is_err());
    }

    #[test]
    fn parses_overflow_check_flag() {
        let parse = |flag: &[&str]| parse_build_options("main.nx", arguments(flag)).unwrap().2;
        assert_eq!(parse(&[]), None);
        assert_eq!(parse(&["--overflow-checks"]), Some(true));
        assert_eq!(parse(&["--overflow-checks=true"]), Some(true));
        assert_eq!(parse(&["--overflow-checks=false"]), Some(false));
        assert!(parse_build_options("main.nx", arguments(&["--overflow-checks=x"])).is_err());
    }

    #[test]
    fn rejects_duplicate_output_options() {
        let result = parse_build_options(
            "main.nx",
            arguments(&["-o", "first.obj", "--output", "second.obj"]),
        );

        assert!(result.is_err());
    }
}
