//! IPC tests for the plugin host against a real child process
//! (`examples/plugin_stub.rs`). The child is started with `std::process::Command`
//! here only; production code launches plugins exclusively through the sandbox.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nortex_ide::plugins::host::{
    ActionError, ArchiveRequest, DiagnosticInfo, DocumentInfo, HostLimits, IdeServices,
    PluginFailure, PluginHost, PluginIo, PluginProcess, PluginState, StartError,
};
use nortex_ide::plugins::protocol::PluginManifest;

struct StubProcess(Arc<Mutex<Child>>);

impl PluginProcess for StubProcess {
    fn exit_code(&self) -> Option<u32> {
        let status = self.0.lock().unwrap().try_wait().ok().flatten()?;
        Some(status.code().unwrap_or(-1) as u32)
    }

    fn kill(&self) {
        let _ = self.0.lock().unwrap().kill();
    }
}

fn stub_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let profile_dir = exe.parent().unwrap().parent().unwrap();
    profile_dir
        .join("examples")
        .join(format!("plugin_stub{}", std::env::consts::EXE_SUFFIX))
}

fn launch_stub(mode: &'static str) -> impl FnOnce() -> Result<PluginIo, String> {
    move || {
        let mut child = Command::new(stub_path())
            .arg(mode)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| error.to_string())?;
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        Ok(PluginIo {
            process: Box::new(StubProcess(Arc::new(Mutex::new(child)))),
            input: Box::new(input),
            output: Box::new(output),
        })
    }
}

struct FakeIde;

impl IdeServices for FakeIde {
    fn active_document(&mut self) -> Option<DocumentInfo> {
        Some(DocumentInfo {
            name: "main.nx".into(),
            language: "nortex".into(),
            text: "namespace app;".into(),
        })
    }

    fn project_files(&mut self) -> Vec<String> {
        vec!["main.nx".into()]
    }

    fn diagnostics(&mut self) -> Vec<DiagnosticInfo> {
        Vec::new()
    }
}

fn manifest(permissions: &[&str]) -> PluginManifest {
    PluginManifest {
        id: "acme.stub".into(),
        ide_api: ">=1.0.0, <2.0.0".into(),
        ide_permissions: permissions.iter().map(|name| name.to_string()).collect(),
    }
}

const ALL: [&str; 5] = [
    "ide.log",
    "ide.document.read",
    "ide.project.read",
    "ide.diagnostics.read",
    "ide.status.write",
];

fn strings(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn fast_limits() -> HostLimits {
    HostLimits {
        handshake_timeout: Duration::from_millis(800),
        shutdown_grace: Duration::from_millis(500),
        ..HostLimits::default()
    }
}

fn pump_until(host: &mut PluginHost, done: impl Fn(&PluginHost) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(host) {
        assert!(
            Instant::now() < deadline,
            "timed out; state: {:?}",
            host.state("acme.stub")
        );
        host.pump(&mut FakeIde);
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn run_until_final(mode: &'static str) -> PluginState {
    let mut host = PluginHost::new(fast_limits());
    host.start(&manifest(&ALL), &strings(&ALL), launch_stub(mode))
        .unwrap();
    pump_until(&mut host, |host| {
        host.state("acme.stub").unwrap().is_final()
    });
    host.state("acme.stub").unwrap().clone()
}

#[test]
fn plugin_starts_uses_the_api_and_stops_on_request() {
    let mut host = PluginHost::new(fast_limits());
    host.start(&manifest(&ALL), &strings(&ALL), launch_stub("good"))
        .unwrap();
    pump_until(&mut host, |host| host.logs("acme.stub").count() == 2);

    let messages: Vec<String> = host
        .logs("acme.stub")
        .map(|entry| entry.message.clone())
        .collect();
    assert_eq!(messages[0], "hello");
    assert_eq!(messages[1], "ok,ok,ok,ok,ok,unknown_method");
    assert_eq!(host.state("acme.stub"), Some(&PluginState::Active));
    assert_eq!(host.status_text("acme.stub"), "ready");

    host.stop("acme.stub");
    pump_until(&mut host, |host| {
        host.state("acme.stub") == Some(&PluginState::Stopped)
    });
}

#[test]
fn api_calls_without_a_granted_permission_are_denied() {
    let mut host = PluginHost::new(fast_limits());
    host.start(
        &manifest(&ALL),
        &strings(&["ide.log"]),
        launch_stub("noperm"),
    )
    .unwrap();
    pump_until(&mut host, |host| host.logs("acme.stub").count() == 1);
    let entry = host.logs("acme.stub").next().unwrap();
    assert_eq!(entry.message, "permission_denied");
    assert_eq!(host.state("acme.stub"), Some(&PluginState::Active));
}

#[test]
fn incompatible_manifest_never_launches_the_process() {
    let mut host = PluginHost::new(fast_limits());
    let mut manifest = manifest(&ALL);
    manifest.ide_api = ">=2.0.0".into();
    let mut launched = false;
    let result = host.start(&manifest, &[], || {
        launched = true;
        Err("unreachable".into())
    });
    assert!(matches!(result, Err(StartError::Incompatible(_))));
    assert!(!launched);
}

#[test]
fn undeclared_or_unknown_permissions_are_rejected_before_launch() {
    let mut host = PluginHost::new(fast_limits());
    let declared = manifest(&["ide.log"]);
    let result = host.start(
        &declared,
        &strings(&["ide.document.read"]),
        launch_stub("good"),
    );
    assert!(matches!(result, Err(StartError::Permissions(_))));
    let result = host.start(
        &declared,
        &strings(&["ide.everything"]),
        launch_stub("good"),
    );
    assert!(matches!(result, Err(StartError::Permissions(_))));
    assert!(host.state("acme.stub").is_none());
}

#[test]
fn plugin_that_requires_a_newer_api_is_rejected_after_the_handshake() {
    assert!(matches!(
        run_until_final("incompatible"),
        PluginState::Failed(PluginFailure::Incompatible(_))
    ));
}

#[test]
fn unlaunchable_process_is_reported() {
    let mut host = PluginHost::new(fast_limits());
    let result = host.start(&manifest(&ALL), &[], || Err("no such file".into()));
    assert!(matches!(result, Err(StartError::Launch(_))));
    assert!(host.state("acme.stub").is_none());
}

#[test]
fn silent_plugin_times_out_in_the_handshake() {
    assert_eq!(
        run_until_final("silent"),
        PluginState::Failed(PluginFailure::HandshakeTimeout)
    );
}

#[test]
fn crashing_plugin_is_detected() {
    assert!(matches!(
        run_until_final("crash"),
        PluginState::Failed(PluginFailure::ProcessExited(_))
    ));
}

#[test]
fn invalid_messages_end_the_session() {
    assert!(matches!(
        run_until_final("garbage"),
        PluginState::Failed(PluginFailure::ProtocolViolation(_))
    ));
}

#[test]
fn oversized_messages_end_the_session() {
    assert_eq!(
        run_until_final("oversize"),
        PluginState::Failed(PluginFailure::MessageTooLarge)
    );
}

#[test]
fn request_flood_is_rate_limited() {
    assert_eq!(
        run_until_final("flood"),
        PluginState::Failed(PluginFailure::RateLimited)
    );
}

#[test]
fn plugin_ignoring_shutdown_is_killed_after_the_grace_period() {
    let mut host = PluginHost::new(fast_limits());
    host.start(
        &manifest(&ALL),
        &strings(&ALL),
        launch_stub("ignore_shutdown"),
    )
    .unwrap();
    pump_until(&mut host, |host| {
        host.state("acme.stub") == Some(&PluginState::Active)
    });
    host.stop("acme.stub");
    pump_until(&mut host, |host| {
        host.state("acme.stub") == Some(&PluginState::Stopped)
    });
}

#[cfg(windows)]
#[test]
#[ignore = "needs a real AppContainer-capable Windows host; modifies a temp dir ACL"]
fn plugin_runs_hosted_inside_the_appcontainer_sandbox() {
    use nortex_ide::plugins::host::launch_sandboxed;
    use nortex_ide::sandbox::{SandboxLimits, SandboxRequest, file_sha256};

    let dir = std::env::temp_dir().join(format!("nortex-plugin-smoke-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let executable = dir.join("plugin_stub.exe");
    std::fs::copy(stub_path(), &executable).unwrap();
    let request = SandboxRequest {
        expected_sha256: Some(file_sha256(&executable).unwrap()),
        executable,
        args: vec!["good".into()],
        working_dir: dir.clone(),
        extension_id: "acme.stub".into(),
        installed_dir: dir.clone(),
        permissions: Vec::new(),
        run_approved: true,
        limits: SandboxLimits::default(),
    };

    let mut host = PluginHost::new(fast_limits());
    host.start(&manifest(&ALL), &strings(&ALL), || {
        launch_sandboxed(&request)
    })
    .unwrap();
    pump_until(&mut host, |host| host.logs("acme.stub").count() == 2);
    assert_eq!(host.status_text("acme.stub"), "ready");
    host.stop("acme.stub");
    pump_until(&mut host, |host| {
        host.state("acme.stub") == Some(&PluginState::Stopped)
    });
    drop(host);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn running_plugin_cannot_be_started_twice_but_can_restart_after_stopping() {
    let mut host = PluginHost::new(fast_limits());
    host.start(&manifest(&ALL), &strings(&ALL), launch_stub("good"))
        .unwrap();
    let second = host.start(&manifest(&ALL), &strings(&ALL), launch_stub("good"));
    assert!(matches!(second, Err(StartError::AlreadyRunning)));
    host.terminate("acme.stub");
    assert_eq!(host.state("acme.stub"), Some(&PluginState::Stopped));
    host.start(&manifest(&ALL), &strings(&ALL), launch_stub("good"))
        .unwrap();
    host.terminate_all();
}

fn start_actions(host: &mut PluginHost) {
    host.start(&manifest(&ALL), &strings(&ALL), launch_stub("actions"))
        .unwrap();
    pump_until(host, |host| {
        host.state("acme.stub") == Some(&PluginState::Active)
    });
}

#[test]
fn declared_actions_are_listed_and_invoked_explicitly() {
    let mut host = PluginHost::new(fast_limits());
    start_actions(&mut host);
    let ids: Vec<&str> = host
        .actions("acme.stub")
        .iter()
        .map(|a| a.id.as_str())
        .collect();
    assert_eq!(ids, ["hello", "fail", "slow"]);
    assert_eq!(host.active_actions().len(), 3);

    assert_eq!(
        host.invoke_action("acme.stub", "nope"),
        Err(ActionError::UnknownAction)
    );
    assert_eq!(
        host.invoke_action("other", "hello"),
        Err(ActionError::UnknownPlugin)
    );
    host.invoke_action("acme.stub", "hello").unwrap();
    assert_eq!(
        host.invoke_action("acme.stub", "hello"),
        Err(ActionError::Busy)
    );
    let mut events = Vec::new();
    pump_until(&mut host, |_| true);
    while events.is_empty() {
        host.pump(&mut FakeIde);
        events.extend(host.take_action_events());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(events[0].outcome, Ok(Some("hello from plugin".into())));

    host.invoke_action("acme.stub", "fail").unwrap();
    let mut events = Vec::new();
    while events.is_empty() {
        host.pump(&mut FakeIde);
        events.extend(host.take_action_events());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(events[0].outcome, Err("nope".into()));
    assert_eq!(host.state("acme.stub"), Some(&PluginState::Active));
    host.terminate_all();
}

#[test]
fn unanswered_action_stops_the_plugin_after_the_timeout() {
    let mut host = PluginHost::new(HostLimits {
        action_timeout: Duration::from_millis(200),
        ..fast_limits()
    });
    start_actions(&mut host);
    host.invoke_action("acme.stub", "slow").unwrap();
    pump_until(&mut host, |host| {
        host.state("acme.stub").unwrap().is_final()
    });
    assert!(matches!(
        host.state("acme.stub"),
        Some(PluginState::Failed(PluginFailure::ActionTimeout(_)))
    ));
    assert!(host.actions("acme.stub").is_empty());
    let events = host.take_action_events();
    assert_eq!(events.len(), 1);
    assert!(events[0].outcome.is_err());
    assert_eq!(
        host.invoke_action("acme.stub", "hello"),
        Err(ActionError::NotActive)
    );
}

#[test]
fn malformed_action_declaration_fails_the_handshake() {
    assert!(matches!(
        run_until_final("badactions"),
        PluginState::Failed(PluginFailure::ProtocolViolation(_))
    ));
}

#[test]
fn restart_replaces_the_running_process_and_shutdown_drops_actions() {
    let mut host = PluginHost::new(fast_limits());
    start_actions(&mut host);
    host.restart(&manifest(&ALL), &strings(&ALL), launch_stub("actions"))
        .unwrap();
    assert_eq!(host.state("acme.stub"), Some(&PluginState::Starting));
    pump_until(&mut host, |host| {
        host.state("acme.stub") == Some(&PluginState::Active)
    });
    let rejected = host.restart(
        &manifest(&["ide.log"]),
        &strings(&ALL),
        launch_stub("actions"),
    );
    assert!(matches!(rejected, Err(StartError::Permissions(_))));
    assert_eq!(host.state("acme.stub"), Some(&PluginState::Active));

    host.stop("acme.stub");
    assert!(host.actions("acme.stub").is_empty());
    pump_until(&mut host, |host| {
        host.state("acme.stub") == Some(&PluginState::Stopped)
    });
}

/// Real archive and PDF export confined to a temporary project directory.
struct ExportIde {
    root: PathBuf,
}

impl IdeServices for ExportIde {
    fn active_document(&mut self) -> Option<DocumentInfo> {
        None
    }

    fn project_files(&mut self) -> Vec<String> {
        Vec::new()
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
            .then(|| ("active.txt".to_owned(), b"unsaved text".to_vec()));
        nortex_ide::plugins::archive::create(&self.root, &request.output, &request.files, extra)
    }

    fn export_document_pdf(
        &mut self,
        output: &str,
    ) -> Result<nortex_ide::plugins::pdf::PdfSummary, &'static str> {
        nortex_ide::plugins::pdf::export(&self.root, output, "exported text")
    }
}

const EXPORT_PERMISSIONS: [&str; 3] = ["ide.log", "ide.archive.write", "ide.document.export"];

fn run_export_stub(label: &str, granted: &[&str]) -> (Vec<String>, PathBuf) {
    let root = std::env::temp_dir().join(format!("nortex-ipc-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("main.nx"), "namespace app;\n".repeat(100)).unwrap();
    let mut ide = ExportIde { root: root.clone() };
    let mut host = PluginHost::new(fast_limits());
    host.start(
        &manifest(&EXPORT_PERMISSIONS),
        &strings(granted),
        launch_stub("export"),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while host.logs("acme.stub").count() == 0 {
        assert!(Instant::now() < deadline, "timed out");
        host.pump(&mut ide);
        std::thread::sleep(Duration::from_millis(5));
    }
    let outcome = host.logs("acme.stub").next().unwrap().message.clone();
    host.terminate_all();
    (outcome.split(',').map(str::to_owned).collect(), root)
}

#[test]
fn archive_and_pdf_exports_write_real_files_and_reject_bad_requests() {
    let (outcomes, root) = run_export_stub("granted", &EXPORT_PERMISSIONS);
    assert_eq!(
        outcomes,
        [
            "ok",
            "invalid_params",
            "unavailable",
            "ok",
            "invalid_params"
        ]
    );

    let mut archive = zip::ZipArchive::new(std::fs::File::open(root.join("a.nxpack")).unwrap())
        .expect("a.nxpack is a ZIP archive");
    assert_eq!(archive.len(), 2);
    let names: Vec<String> = (0..2)
        .map(|index| archive.by_index(index).unwrap().name().to_owned())
        .collect();
    assert_eq!(names, ["active.txt", "main.nx"]);
    assert_eq!(
        archive.by_index(0).unwrap().compression(),
        zip::CompressionMethod::Deflated
    );
    assert!(
        std::fs::read(root.join("a.pdf"))
            .unwrap()
            .starts_with(b"%PDF-1.4")
    );
    assert!(!root.join("b.nxpack").exists());
    assert!(!root.parent().unwrap().join("c.nxpack").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn export_methods_need_their_own_permissions() {
    let (outcomes, root) = run_export_stub("denied", &["ide.log"]);
    assert_eq!(outcomes, ["permission_denied"; 5]);
    assert!(!root.join("a.nxpack").exists());
    assert!(!root.join("a.pdf").exists());
    let _ = std::fs::remove_dir_all(root);
}
