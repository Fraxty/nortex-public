//! Interactive end-to-end check of the three archive/PDF extensions: each is compiled
//! with the real `nortexc`, started as a long-lived process under `PluginHost`, and its
//! menu action is invoked after initialization. Skipped when `nortexc` is not built.
//! The child is started with `std::process::Command` here only; production code
//! launches plugins exclusively through the sandbox.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nortex_ide::plugins::host::{
    ArchiveRequest, DiagnosticInfo, DocumentInfo, HostLimits, IdeServices, PluginHost, PluginIo,
    PluginProcess, PluginState,
};
use nortex_ide::plugins::protocol::PluginManifest;

struct Proc(Arc<Mutex<Child>>);

impl PluginProcess for Proc {
    fn exit_code(&self) -> Option<u32> {
        let status = self.0.lock().unwrap().try_wait().ok().flatten()?;
        Some(status.code().unwrap_or(-1) as u32)
    }

    fn kill(&self) {
        let _ = self.0.lock().unwrap().kill();
    }
}

struct Ide {
    root: PathBuf,
}

const DOC: &str = "namespace demo;\nint main() { return 0; }\n";

impl IdeServices for Ide {
    fn active_document(&mut self) -> Option<DocumentInfo> {
        Some(DocumentInfo {
            name: "main.nx".into(),
            language: "nortex".into(),
            text: DOC.into(),
        })
    }

    fn project_files(&mut self) -> Vec<String> {
        vec!["main.nx".into(), "notes.txt".into()]
    }

    fn diagnostics(&mut self) -> Vec<DiagnosticInfo> {
        Vec::new()
    }

    fn create_archive(
        &mut self,
        request: &ArchiveRequest,
    ) -> Result<nortex_ide::plugins::archive::ArchiveSummary, &'static str> {
        let extra = request
            .include_active_document
            .then(|| ("main.nx".to_owned(), DOC.as_bytes().to_vec()));
        nortex_ide::plugins::archive::create(&self.root, &request.output, &request.files, extra)
    }

    fn export_document_pdf(
        &mut self,
        output: &str,
    ) -> Result<nortex_ide::plugins::pdf::PdfSummary, &'static str> {
        nortex_ide::plugins::pdf::export(&self.root, output, DOC)
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn compiler() -> Option<PathBuf> {
    let path = workspace()
        .join("target")
        .join("debug")
        .join(format!("nortexc{}", std::env::consts::EXE_SUFFIX));
    path.is_file().then_some(path)
}

fn compile(compiler: &Path, id: &str, out_dir: &Path) -> PathBuf {
    let entry = workspace()
        .join("extensions")
        .join("available")
        .join(id)
        .join("main.nx");
    let exe = out_dir.join(format!("{id}{}", std::env::consts::EXE_SUFFIX));
    let output = Command::new(compiler)
        .arg("build")
        .arg(&entry)
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{id} failed to compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    exe
}

fn pump_until(host: &mut PluginHost, ide: &mut Ide, done: &mut dyn FnMut(&mut PluginHost) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !done(host) {
        assert!(Instant::now() < deadline, "timed out");
        host.pump(ide);
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Starts the compiled extension, invokes its single action twice, returns both outcomes.
fn drive(
    exe: &Path,
    id: &str,
    permissions: &[&str],
    action: &str,
    root: &Path,
) -> Vec<Result<Option<String>, String>> {
    let mut ide = Ide {
        root: root.to_path_buf(),
    };
    let mut host = PluginHost::new(HostLimits::default());
    let manifest = PluginManifest {
        id: id.into(),
        ide_api: ">=1.2.0, <2.0.0".into(),
        ide_permissions: permissions.iter().map(|p| p.to_string()).collect(),
    };
    let granted: Vec<String> = permissions.iter().map(|p| p.to_string()).collect();
    let exe = exe.to_path_buf();
    host.start(&manifest, &granted, move || {
        let mut child = Command::new(exe)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?;
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        Ok(PluginIo {
            process: Box::new(Proc(Arc::new(Mutex::new(child)))),
            input: Box::new(input),
            output: Box::new(output),
        })
    })
    .unwrap();
    pump_until(&mut host, &mut ide, &mut |host| {
        host.state(id) == Some(&PluginState::Active)
    });
    let declared: Vec<String> = host.actions(id).iter().map(|a| a.id.clone()).collect();
    assert_eq!(declared, [action]);

    let mut outcomes = Vec::new();
    for _ in 0..2 {
        host.invoke_action(id, action).unwrap();
        let mut events = Vec::new();
        pump_until(&mut host, &mut ide, &mut |host| {
            events.extend(host.take_action_events());
            !events.is_empty()
        });
        outcomes.push(events.remove(0).outcome);
        assert_eq!(host.state(id), Some(&PluginState::Active));
    }
    host.terminate_all();
    outcomes
}

fn project(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("nortex-e2e-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("main.nx"), DOC).unwrap();
    std::fs::write(root.join("notes.txt"), "hello ".repeat(500)).unwrap();
    root
}

#[test]
fn archive_and_pdf_extensions_work_interactively_after_initialization() {
    let Some(compiler) = compiler() else {
        eprintln!("skipped: build nortexc first (cargo build -p nortex-compiler)");
        return;
    };
    let out = std::env::temp_dir().join(format!("nortex-e2e-bin-{}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();

    let root = project("archive");
    let exe = compile(&compiler, "archive-tools", &out);
    let outcomes = drive(
        &exe,
        "archive-tools",
        &["ide.project.read", "ide.archive.write", "ide.log"],
        "pack-project",
        &root,
    );
    eprintln!("archive-tools: {outcomes:?}");
    assert!(
        outcomes
            .iter()
            .all(|o| matches!(o, Ok(Some(m)) if m.contains("2 Dateien")))
    );
    let mut zip = zip::ZipArchive::new(std::fs::File::open(root.join("projekt.nxpack")).unwrap())
        .expect("projekt.nxpack is a ZIP");
    assert_eq!(zip.len(), 2);
    assert_eq!(
        zip.by_name("notes.txt").unwrap().compression(),
        zip::CompressionMethod::Deflated
    );
    let _ = std::fs::remove_dir_all(&root);

    let root = project("compression");
    let exe = compile(&compiler, "compression", &out);
    let outcomes = drive(
        &exe,
        "compression",
        &["ide.archive.write", "ide.log"],
        "pack-active",
        &root,
    );
    eprintln!("compression: {outcomes:?}");
    assert!(
        outcomes
            .iter()
            .all(|o| matches!(o, Ok(Some(m)) if m.contains("ZIP")))
    );
    let mut zip =
        zip::ZipArchive::new(std::fs::File::open(root.join("komprimiert.nxpack")).unwrap())
            .expect("komprimiert.nxpack is a ZIP");
    let mut text = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("main.nx").unwrap(), &mut text).unwrap();
    assert_eq!(text, DOC);
    let _ = std::fs::remove_dir_all(&root);

    let root = project("pdf");
    let exe = compile(&compiler, "pdf-tools", &out);
    let outcomes = drive(
        &exe,
        "pdf-tools",
        &["ide.document.read", "ide.document.export", "ide.log"],
        "export-pdf",
        &root,
    );
    eprintln!("pdf-tools: {outcomes:?}");
    assert!(
        outcomes
            .iter()
            .all(|o| matches!(o, Ok(Some(m)) if m.contains("Seite")))
    );
    let pdf = std::fs::read(root.join("main.nx.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-1.4") && pdf.ends_with(b"%%EOF\n"));
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&out);
}
