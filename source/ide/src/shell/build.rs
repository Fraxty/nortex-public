//! Build- und Ausführungspipeline über `nortexc`.

use crate::*;
use nortex_ide::i18n::tr_fmt;

/// Builds with `nortexc` and runs the result. Compiler output goes to the compiler
/// log, program output to the runtime tab. A non-zero program exit code is data
/// (programs return values as exit codes), so only compile failures are `Failed`.
pub(crate) fn execute_build_and_run(
    project_root: &Path,
    source: &Path,
    overflow_checks: bool,
    manifest: Option<(PathBuf, PathBuf, bool)>,
    sender: &mpsc::Sender<RunEvent>,
) -> RunOutcome {
    let compiler = |message: String| {
        let _ = sender.send(RunEvent::Compiler(message));
    };
    let executable = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    let executable = toolchain::compiler_path(executable.as_deref());
    let mut command = Command::new(&executable);
    command
        .current_dir(project_root)
        .arg("build")
        .arg(source)
        .arg("--lang")
        .arg(nortex_ide::i18n::language().code());
    // Mit Manifest bestimmt nortex.toml Ausgabe (und ggf. Overflow-Checks).
    let output_path = match &manifest {
        Some((_, output, manifest_sets_overflow)) => {
            if !manifest_sets_overflow {
                command.arg(format!("--overflow-checks={overflow_checks}"));
            }
            output.clone()
        }
        None => {
            let output = source.with_extension("exe");
            command
                .arg("--output")
                .arg(&output)
                .arg(format!("--overflow-checks={overflow_checks}"));
            output
        }
    };
    let build = {
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
        command.output()
    };
    let output = match build {
        Ok(output) => output,
        Err(error) => {
            compiler(format!(
                "Compiler '{}' konnte nicht gestartet werden: {error}",
                executable.display()
            ));
            return RunOutcome::Failed;
        }
    };
    send_process_output(sender, "build", &output.stdout);
    send_process_output(sender, "build stderr", &output.stderr);
    if !output.status.success() {
        compiler(format!(
            "Build fehlgeschlagen (Exitcode {:?}).",
            output.status.code()
        ));
        return RunOutcome::Failed;
    }
    if !output_path.is_file() {
        compiler("Build-Befehl meldete Erfolg, hat aber keine EXE erzeugt.".into());
        return RunOutcome::Failed;
    }
    compiler(tr_fmt(
        "Build erfolgreich: {}",
        &[output_path.display().to_string()],
    ));

    let runtime = |message: String| {
        let _ = sender.send(RunEvent::Runtime(message));
    };
    let mut program = Command::new(&output_path);
    program.current_dir(project_root);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut program, 0x0800_0000);
    match program.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !stdout.is_empty() {
                runtime(stdout.trim_end().to_string());
            }
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.is_empty() {
                runtime(format!("stderr:\n{}", stderr.trim_end()));
            }
            runtime(format!(
                "Programm beendet (Exitcode {:?}).",
                output.status.code()
            ));
        }
        Err(error) => {
            runtime(tr_fmt(
                "Programm konnte nicht gestartet werden: {}",
                &[error.to_string()],
            ));
        }
    }
    RunOutcome::Succeeded
}

pub(crate) fn send_process_output(sender: &mpsc::Sender<RunEvent>, label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let output = String::from_utf8_lossy(bytes);
    let _ = sender.send(RunEvent::Compiler(format!("{label}:\n{output}")));
}
