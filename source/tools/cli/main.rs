//! NORTEX Toolchain CLI.
//!
//! Subcommands:
//!   build, check, run, test, clean, fmt, lint, doc, package, install, publish, version, help
//!
//! Compliant with Rust >= 1.88 / 2024 edition, supporting Microsoft x64 PE/COFF target.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

pub const VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubCommand {
    Build(BuildOptions),
    Check(CheckOptions),
    Run(RunOptions),
    Test(TestOptions),
    Clean(CleanOptions),
    Fmt(FmtOptions),
    Lint(LintOptions),
    Doc(DocOptions),
    Package(PackageOptions),
    Install(InstallOptions),
    Publish(PublishOptions),
    Version,
    Help(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationLevel {
    None,       // -O0 / Debug
    Size,       // -Os
    Speed,      // -O2 / Release default
    Aggressive, // -O3
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetProfile {
    pub os: String,          // "windows", "linux"
    pub arch: String,        // "x86_64", "aarch64"
    pub abi: String,         // "msvc", "gnu"
    pub output_kind: String, // "exe", "dylib", "staticlib"
    pub opt_level: OptimizationLevel,
    pub emit_mir: bool,
    pub emit_asm: bool,
    pub debug_symbols: bool,
}

impl Default for TargetProfile {
    fn default() -> Self {
        Self {
            os: "windows".to_string(),
            arch: "x86_64".to_string(),
            abi: "msvc".to_string(),
            output_kind: "exe".to_string(),
            opt_level: OptimizationLevel::None,
            emit_mir: false,
            emit_asm: false,
            debug_symbols: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuildOptions {
    pub input_path: Option<PathBuf>,
    pub output_path: Option<PathBuf>,
    pub release: bool,
    pub target: Option<String>,
    pub jobs: Option<usize>,
    pub emit_mir: bool,
    pub emit_asm: bool,
    pub verbose: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CheckOptions {
    pub input_path: Option<PathBuf>,
    pub strict_ownership: bool,
    pub all_targets: bool,
    pub verbose: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunOptions {
    pub input_path: Option<PathBuf>,
    pub release: bool,
    pub extra_args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TestOptions {
    pub filter: Option<String>,
    pub nocapture: bool,
    pub ignored: bool,
    pub jobs: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CleanOptions {
    pub doc_only: bool,
    pub target_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FmtOptions {
    pub check: bool,
    pub files: Vec<PathBuf>,
    pub config_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LintOptions {
    pub deny_warnings: bool,
    pub allow_dead_code: bool,
    pub strict_borrow_rules: bool,
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DocOptions {
    pub open_browser: bool,
    pub output_dir: Option<PathBuf>,
    pub include_private: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PackageOptions {
    pub output_archive: Option<PathBuf>,
    pub dry_run: bool,
    pub bundle_runtime: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InstallOptions {
    pub package_name: Option<String>,
    pub path: Option<PathBuf>,
    pub root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PublishOptions {
    pub registry: Option<String>,
    pub token: Option<String>,
    pub dry_run: bool,
}

pub struct CliParser {
    args: Vec<String>,
    cursor: usize,
}

impl CliParser {
    pub fn new(args: impl IntoIterator<Item = String>) -> Self {
        Self {
            args: args.into_iter().collect(),
            cursor: 0,
        }
    }

    fn next(&mut self) -> Option<String> {
        if self.cursor < self.args.len() {
            let item = self.args[self.cursor].clone();
            self.cursor += 1;
            Some(item)
        } else {
            None
        }
    }

    fn next_value(&mut self, flag_name: &str) -> Result<String, String> {
        self.next()
            .ok_or_else(|| format!("Flag '{flag_name}' erfordert ein Argument."))
    }

    pub fn parse(mut self) -> Result<SubCommand, String> {
        let first = match self.next() {
            Some(arg) => arg,
            None => return Ok(SubCommand::Help(None)),
        };

        match first.as_str() {
            "-V" | "--version" | "version" => Ok(SubCommand::Version),
            "-h" | "--help" | "help" => {
                let topic = self.next();
                Ok(SubCommand::Help(topic))
            }
            "build" => self.parse_build(),
            "check" => self.parse_check(),
            "run" => self.parse_run(),
            "test" => self.parse_test(),
            "clean" => self.parse_clean(),
            "fmt" | "format" => self.parse_fmt(),
            "lint" => self.parse_lint(),
            "doc" => self.parse_doc(),
            "package" | "pkg" => self.parse_package(),
            "install" => self.parse_install(),
            "publish" => self.parse_publish(),
            unknown => Err(format!(
                "Unbekannter Befehl: '{unknown}'. Führe 'nortex help' aus für eine Übersicht."
            )),
        }
    }

    fn parse_build(&mut self) -> Result<SubCommand, String> {
        let mut opts = BuildOptions::default();
        while let Some(arg) = self.next() {
            match arg.as_str() {
                "--release" | "-r" => opts.release = true,
                "--target" => opts.target = Some(self.next_value("--target")?),
                "-o" | "--output" => {
                    opts.output_path = Some(PathBuf::from(self.next_value("--output")?))
                }
                "-j" | "--jobs" => {
                    let val = self.next_value("--jobs")?;
                    opts.jobs = Some(
                        val.parse::<usize>()
                            .map_err(|_| "Ungültige Threadanzahl für --jobs")?,
                    );
                }
                "--emit-mir" => opts.emit_mir = true,
                "--emit-asm" => opts.emit_asm = true,
                "-v" | "--verbose" => opts.verbose = true,
                flag if flag.starts_with('-') => {
                    return Err(format!("Unbekannte Option für build: {flag}"));
                }
                positional => {
                    if opts.input_path.is_none() {
                        opts.input_path = Some(PathBuf::from(positional));
                    } else {
                        return Err(format!(
                            "Unerwartetes Argument nach Eingabepfad: {positional}"
                        ));
                    }
                }
            }
        }
        Ok(SubCommand::Build(opts))
    }

    fn parse_check(&mut self) -> Result<SubCommand, String> {
        let mut opts = CheckOptions::default();
        while let Some(arg) = self.next() {
            match arg.as_str() {
                "--strict-ownership" => opts.strict_ownership = true,
                "--all-targets" => opts.all_targets = true,
                "-v" | "--verbose" => opts.verbose = true,
                flag if flag.starts_with('-') => {
                    return Err(format!("Unbekannte Option für check: {flag}"));
                }
                positional => {
                    if opts.input_path.is_none() {
                        opts.input_path = Some(PathBuf::from(positional));
                    } else {
                        return Err(format!("Unerwartetes Argument für check: {positional}"));
                    }
                }
            }
        }
        Ok(SubCommand::Check(opts))
    }

    fn parse_run(&mut self) -> Result<SubCommand, String> {
        let mut opts = RunOptions::default();
        let mut past_separator = false;
        while let Some(arg) = self.next() {
            if past_separator {
                opts.extra_args.push(arg);
                continue;
            }
            match arg.as_str() {
                "--" => past_separator = true,
                "--release" | "-r" => opts.release = true,
                flag if flag.starts_with('-') => {
                    return Err(format!("Unbekannte Option für run: {flag}"));
                }
                positional => {
                    if opts.input_path.is_none() {
                        opts.input_path = Some(PathBuf::from(positional));
                    } else {
                        opts.extra_args.push(positional.to_owned());
                    }
                }
            }
        }
        Ok(SubCommand::Run(opts))
    }

    fn parse_test(&mut self) -> Result<SubCommand, String> {
        let mut opts = TestOptions::default();
        while let Some(arg) = self.next() {
            match arg.as_str() {
                "--nocapture" => opts.nocapture = true,
                "--ignored" => opts.ignored = true,
                "-j" | "--jobs" => {
                    let val = self.next_value("--jobs")?;
                    opts.jobs = Some(
                        val.parse::<usize>()
                            .map_err(|_| "Ungültige Anzahl für --jobs")?,
                    );
                }
                flag if flag.starts_with('-') => {
                    return Err(format!("Unbekannte Option für test: {flag}"));
                }
                filter_pattern => opts.filter = Some(filter_pattern.to_owned()),
            }
        }
        Ok(SubCommand::Test(opts))
    }

    fn parse_clean(&mut self) -> Result<SubCommand, String> {
        let mut opts = CleanOptions::default();
        while let Some(arg) = self.next() {
            match arg.as_str() {
                "--doc" => opts.doc_only = true,
                "--target-dir" => {
                    opts.target_dir = Some(PathBuf::from(self.next_value("--target-dir")?))
                }
                flag => return Err(format!("Unbekannte Option für clean: {flag}")),
            }
        }
        Ok(SubCommand::Clean(opts))
    }

    fn parse_fmt(&mut self) -> Result<SubCommand, String> {
        let mut opts = FmtOptions::default();
        while let Some(arg) = self.next() {
            match arg.as_str() {
                "--check" => opts.check = true,
                "--config" => opts.config_path = Some(PathBuf::from(self.next_value("--config")?)),
                flag if flag.starts_with('-') => {
                    return Err(format!("Unbekannte Option für fmt: {flag}"));
                }
                file => opts.files.push(PathBuf::from(file)),
            }
        }
        Ok(SubCommand::Fmt(opts))
    }

    fn parse_lint(&mut self) -> Result<SubCommand, String> {
        let mut opts = LintOptions::default();
        while let Some(arg) = self.next() {
            match arg.as_str() {
                "-D" | "--deny-warnings" => opts.deny_warnings = true,
                "--allow-dead-code" => opts.allow_dead_code = true,
                "--strict-borrow-rules" => opts.strict_borrow_rules = true,
                flag if flag.starts_with('-') => {
                    return Err(format!("Unbekannte Option für lint: {flag}"));
                }
                file => opts.files.push(PathBuf::from(file)),
            }
        }
        Ok(SubCommand::Lint(opts))
    }

    fn parse_doc(&mut self) -> Result<SubCommand, String> {
        let mut opts = DocOptions::default();
        while let Some(arg) = self.next() {
            match arg.as_str() {
                "--open" => opts.open_browser = true,
                "--output-dir" | "-o" => {
                    opts.output_dir = Some(PathBuf::from(self.next_value("--output-dir")?))
                }
                "--private" => opts.include_private = true,
                flag => return Err(format!("Unbekannte Option für doc: {flag}")),
            }
        }
        Ok(SubCommand::Doc(opts))
    }

    fn parse_package(&mut self) -> Result<SubCommand, String> {
        let mut opts = PackageOptions::default();
        while let Some(arg) = self.next() {
            match arg.as_str() {
                "--dry-run" => opts.dry_run = true,
                "--bundle-runtime" => opts.bundle_runtime = true,
                "-o" | "--output" => {
                    opts.output_archive = Some(PathBuf::from(self.next_value("--output")?))
                }
                flag => return Err(format!("Unbekannte Option für package: {flag}")),
            }
        }
        Ok(SubCommand::Package(opts))
    }

    fn parse_install(&mut self) -> Result<SubCommand, String> {
        let mut opts = InstallOptions::default();
        while let Some(arg) = self.next() {
            match arg.as_str() {
                "--path" => opts.path = Some(PathBuf::from(self.next_value("--path")?)),
                "--root" => opts.root = Some(PathBuf::from(self.next_value("--root")?)),
                flag if flag.starts_with('-') => {
                    return Err(format!("Unbekannte Option für install: {flag}"));
                }
                name => opts.package_name = Some(name.to_owned()),
            }
        }
        Ok(SubCommand::Install(opts))
    }

    fn parse_publish(&mut self) -> Result<SubCommand, String> {
        let mut opts = PublishOptions::default();
        while let Some(arg) = self.next() {
            match arg.as_str() {
                "--dry-run" => opts.dry_run = true,
                "--registry" => opts.registry = Some(self.next_value("--registry")?),
                "--token" => opts.token = Some(self.next_value("--token")?),
                flag => return Err(format!("Unbekannte Option für publish: {flag}")),
            }
        }
        Ok(SubCommand::Publish(opts))
    }
}

// ----------------------------------------------------------------------------
// Ausführungsroutinen & Manifest-Handling
// ----------------------------------------------------------------------------

fn find_manifest_root() -> Option<PathBuf> {
    let mut current = env::current_dir().ok()?;
    loop {
        if current.join("Nortrex.toml").is_file() || current.join(".nortex").is_file() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

pub fn execute_command(cmd: SubCommand) -> ExitCode {
    match cmd {
        SubCommand::Version => {
            println!("NORTEX Toolchain Version {VERSION}");
            println!("Target-Default: x86_64-pc-windows-msvc (PE/COFF SEH)");
            ExitCode::SUCCESS
        }
        SubCommand::Help(topic) => {
            print_help(topic.as_deref());
            ExitCode::SUCCESS
        }
        SubCommand::Check(opts) => execute_check(opts),
        SubCommand::Build(opts) => execute_build(opts),
        SubCommand::Run(opts) => execute_run(opts),
        SubCommand::Test(opts) => execute_test(opts),
        SubCommand::Clean(opts) => execute_clean(opts),
        SubCommand::Fmt(opts) => execute_fmt(opts),
        SubCommand::Lint(opts) => execute_lint(opts),
        SubCommand::Doc(opts) => execute_doc(opts),
        SubCommand::Package(opts) => execute_package(opts),
        SubCommand::Install(opts) => execute_install(opts),
        SubCommand::Publish(opts) => execute_publish(opts),
    }
}

fn execute_check(opts: CheckOptions) -> ExitCode {
    let target_file = match opts.input_path {
        Some(path) => path,
        None => {
            if let Some(root) = find_manifest_root() {
                let candidate = root.join("src").join("main.nx");
                if candidate.is_file() {
                    candidate
                } else {
                    eprintln!("Fehler: Kein Quelldatei-Einstiegspunkt src/main.nx gefunden.");
                    return ExitCode::FAILURE;
                }
            } else {
                eprintln!(
                    "Fehler: Kein Eingabepfad spezifiziert und kein Nortrex.toml im Verzeichnisbaum."
                );
                return ExitCode::from(2);
            }
        }
    };

    println!("[NORTEX] Prüfe Quelldatei: {}", target_file.display());
    if opts.strict_ownership {
        println!("  - Modus: Strict Borrow Checker & Invariantenprüfung aktiv");
    }

    // Delegation an Compiler-Kern
    match nortex_compiler::driver::check_file(&target_file) {
        Ok(result) => {
            let mut err_count = 0;
            for diagnostic in &result.diagnostics {
                let severity_str = match diagnostic.severity {
                    nortex_compiler::diagnostics::Severity::Error => {
                        err_count += 1;
                        "\x1b[31m[FEHLER]\x1b[0m"
                    }
                    nortex_compiler::diagnostics::Severity::Warning => "\x1b[33m[WARNUNG]\x1b[0m",
                };
                let file = diagnostic.file.as_deref().unwrap_or("<unbekannt>");
                eprintln!(
                    "{} [NX{:04}]: {} --> {}:{}:{}",
                    severity_str,
                    diagnostic.code,
                    diagnostic.message,
                    file,
                    diagnostic.line(),
                    diagnostic.column()
                );
                if let Some(help) = &diagnostic.help {
                    eprintln!("       \x1b[36mHilfe:\x1b[0m {help}");
                }
            }

            if err_count > 0 {
                eprintln!("\x1b[31mAbbruch:\x1b[0m {err_count} Semantik-/Syntaxfehler gefunden.");
                ExitCode::FAILURE
            } else {
                println!(
                    "\x1b[32mErfolg:\x1b[0m Keine Fehler gefunden ({} Tokens analysiert).",
                    result.token_count
                );
                ExitCode::SUCCESS
            }
        }
        Err(e) => {
            eprintln!(
                "I/O-Fehler beim Öffnen von {}: {}",
                target_file.display(),
                e
            );
            ExitCode::from(2)
        }
    }
}

fn execute_build(opts: BuildOptions) -> ExitCode {
    println!(
        "[NORTEX BUILD] Modus: {}",
        if opts.release {
            "Release (optimiert, -O3)"
        } else {
            "Debug (-O0, SEH-Symbole)"
        }
    );
    let target = opts
        .target
        .unwrap_or_else(|| "x86_64-pc-windows-msvc".to_string());
    println!("  Target: {target}");
    if opts.emit_mir {
        println!("  Emit: Middle Intermediate Representation (.mir)");
    }
    if opts.emit_asm {
        println!("  Emit: Microsoft x64 Assembly (.asm)");
    }
    eprintln!(
        "NORTEX build ist noch nicht verfügbar: AST/HIR/MIR-Lowering, Codegenerierung und Linker sind nicht verbunden."
    );
    ExitCode::FAILURE
}

fn execute_run(opts: RunOptions) -> ExitCode {
    let _ = opts;
    eprintln!(
        "NORTEX run ist noch nicht verfügbar, weil `nortex build` keine ausführbare Datei erzeugt."
    );
    ExitCode::FAILURE
}

fn execute_test(_opts: TestOptions) -> ExitCode {
    command_not_implemented("test")
}

fn execute_clean(opts: CleanOptions) -> ExitCode {
    let target_dir = opts.target_dir.unwrap_or_else(|| PathBuf::from("target"));
    println!(
        "[NORTEX CLEAN] Entferne Artefakte in {}",
        target_dir.display()
    );
    if target_dir.exists() {
        if let Err(e) = fs::remove_dir_all(&target_dir) {
            eprintln!("Fehler beim Löschen von {}: {}", target_dir.display(), e);
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}

fn execute_fmt(_opts: FmtOptions) -> ExitCode {
    command_not_implemented("fmt")
}

fn execute_lint(_opts: LintOptions) -> ExitCode {
    command_not_implemented("lint")
}

fn execute_doc(_opts: DocOptions) -> ExitCode {
    command_not_implemented("doc")
}

fn execute_package(_opts: PackageOptions) -> ExitCode {
    command_not_implemented("package")
}

fn execute_install(_opts: InstallOptions) -> ExitCode {
    command_not_implemented("install")
}

fn execute_publish(_opts: PublishOptions) -> ExitCode {
    command_not_implemented("publish")
}

fn command_not_implemented(command: &str) -> ExitCode {
    eprintln!("nortex {command} ist noch nicht verfügbar.");
    ExitCode::FAILURE
}

fn print_help(topic: Option<&str>) {
    match topic {
        Some("build") => {
            println!("Verwendung: nortex build [OPTIONEN] [PFAD]");
            println!("Kompiliert das NORTEX-Projekt oder die angegebene Datei.");
            println!("Optionen:");
            println!("  -r, --release      Erstellt optimierte Binärdateien");
            println!("      --target <T>   Ziel-Target-Triple (z.B. x86_64-pc-windows-msvc)");
            println!("  -o, --output <P>   Ausgabepfad für PE/COFF-Binärdatei");
            println!("      --emit-mir     Gibt das MIR-Zwischenformat aus");
            println!("      --emit-asm     Gibt nativen x64-Assemblercode aus");
            println!("  -v, --verbose      Ausführliche Compiler-Logs");
        }
        Some("check") => {
            println!("Verwendung: nortex check [OPTIONEN] [PFAD]");
            println!("Prüft Typen, Syntax und Ownership-Invarianten ohne Codegenerierung.");
        }
        _ => {
            println!("NORTEX Toolchain CLI v{VERSION}");
            println!("Verwendung: nortex <SUBCOMMAND> [OPTIONEN]");
            println!("\nBefehle:");
            println!("  build       Kompiliert die Quellcodedateien in native PE/COFF Executables");
            println!("  check       Führt vollständige Typ-, Scope- und Borrow-Prüfungen durch");
            println!("  run         Kompiliert und startet die Projekt-Hauptfunktion");
            println!("  test        Führt Unit- und Integrationstests aus");
            println!("  clean       Entfernt Build-Artefakte und temporäre Caches");
            println!("  fmt         Formatiert Quelldateien nach dem NORTEX-Styleguide");
            println!("  lint        Statische Codeanalyse für tote Symbole & Mutabilität");
            println!("  doc         Extrahiert Dokumentation in HTML/Markdown");
            println!("  package     Bündelt Projekt in portable .ntx Container");
            println!("  install     Installiert ein NORTEX-Paket in die lokale Toolchain");
            println!("  publish     Veröffentlicht ein Paket in der Paket-Registry");
            println!("  version     Zeigt Versions- und Targetinformationen an");
            println!("  help        Zeigt Hilfe zu Befehlen an");
        }
    }
}

fn main() -> ExitCode {
    let parser = CliParser::new(env::args().skip(1));
    match parser.parse() {
        Ok(cmd) => execute_command(cmd),
        Err(err) => {
            eprintln!("\x1b[31mCLI-Fehler:\x1b[0m {err}");
            eprintln!("Führe 'nortex help' aus, um gültige Optionen einzusehen.");
            ExitCode::from(2)
        }
    }
}
