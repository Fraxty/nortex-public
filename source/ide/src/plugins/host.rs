//! Plugin host: starts plugin processes, speaks the IPC protocol with them and
//! serves the versioned IDE API.
//!
//! The host never blocks the caller on a plugin: a reader thread and a writer thread
//! per session own the pipes, [`PluginHost::pump`] only looks at queued frames.
//! Process isolation comes from the launcher. [`launch_sandboxed`] uses the Windows
//! AppContainer sandbox; there is intentionally no launcher that starts a plugin
//! without a sandbox.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};

use super::archive::{ArchiveSummary, MAX_ARCHIVE_ENTRIES};
use super::pdf::PdfSummary;
use super::project_io::MAX_PATH_CHARS;
use super::protocol::{
    self, ApiMethod, CompatibilityError, Frame, IDE_API_VERSION, Kind, MAX_ACTION_ID_CHARS,
    MAX_ACTION_TITLE_CHARS, MAX_ACTIONS, MAX_FRAME_BYTES, METHOD_ACTION_INVOKE, METHOD_INITIALIZE,
    METHOD_SHUTDOWN, PluginManifest, code,
};
use crate::sandbox::{SandboxProcess, SandboxRequest, spawn_sandboxed};

const MAX_LOG_ENTRIES: usize = 200;
const MAX_LOG_MESSAGE_CHARS: usize = 2000;
const MAX_STATUS_CHARS: usize = 120;
const MAX_DOCUMENT_BYTES: usize = 24 * 1024;
const MAX_LISTED_FILES: usize = 500;
const MAX_LISTED_DIAGNOSTICS: usize = 200;
const MAX_WRITE_BYTES: usize = 32 * 1024;
const MAX_SEARCH_MATCHES: usize = 100;
const OUTBOX_FRAMES: usize = 64;
const INBOX_FRAMES: usize = 64;
const MAX_ACTION_EVENTS: usize = 16;

#[derive(Debug, Clone)]
pub struct HostLimits {
    pub handshake_timeout: Duration,
    pub shutdown_grace: Duration,
    pub max_requests_per_second: u32,
    pub max_frames_per_pump: usize,
    /// How long a plugin may take to answer one `action.invoke`; it is stopped after.
    pub action_timeout: Duration,
}

impl Default for HostLimits {
    fn default() -> Self {
        Self {
            handshake_timeout: Duration::from_secs(5),
            shutdown_grace: Duration::from_secs(2),
            max_requests_per_second: 100,
            max_frames_per_pump: 32,
            action_timeout: Duration::from_secs(10),
        }
    }
}

/// The running plugin as seen by the host.
pub trait PluginProcess: Send {
    fn exit_code(&self) -> Option<u32>;
    fn kill(&self);
}

impl PluginProcess for SandboxProcess {
    fn exit_code(&self) -> Option<u32> {
        SandboxProcess::exit_code(self)
    }

    fn kill(&self) {
        SandboxProcess::kill(self);
    }
}

pub struct PluginIo {
    pub process: Box<dyn PluginProcess>,
    pub input: Box<dyn Write + Send>,
    pub output: Box<dyn Read + Send>,
}

/// Starts the plugin in the process sandbox. Fails (and starts nothing) when the
/// sandbox is unavailable, for example on non-Windows platforms. Whatever the plugin
/// writes to stderr is discarded; plugins report through the `log` API method.
pub fn launch_sandboxed(request: &SandboxRequest) -> Result<PluginIo, String> {
    let mut process = spawn_sandboxed(request).map_err(|error| error.to_string())?;
    let (stdin, stdout, stderr) = process
        .take_io()
        .ok_or_else(|| "sandbox pipes are unavailable".to_string())?;
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut BufReader::new(stderr), &mut std::io::sink());
    });
    Ok(PluginIo {
        process: Box::new(process),
        input: Box::new(stdin),
        output: Box::new(stdout),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentInfo {
    pub name: String,
    pub language: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiagnosticInfo {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub line: usize,
    pub column: usize,
}

/// What the IDE offers to plugins; the host calls it only for permitted methods.
pub trait IdeServices {
    fn active_document(&mut self) -> Option<DocumentInfo>;
    /// Project-relative paths with `/` separators.
    fn project_files(&mut self) -> Vec<String>;
    fn diagnostics(&mut self) -> Vec<DiagnosticInfo>;

    /// Queues a replacement of the active document's text (undoable in the editor).
    fn replace_document(&mut self, _text: String) -> Result<(), &'static str> {
        Err("not supported")
    }
    /// Project-relative path; the implementation must keep it inside the project.
    fn read_file(&mut self, _path: &str) -> Result<String, &'static str> {
        Err("not supported")
    }
    fn write_file(&mut self, _path: &str, _content: &str) -> Result<(), &'static str> {
        Err("not supported")
    }
    fn notify(&mut self, _message: String) {}
    /// `(path, line, text)` of the first matches in project files.
    fn search_text(&mut self, _query: &str, _limit: usize) -> Vec<(String, usize, String)> {
        Vec::new()
    }
    /// `(branch, [(status code, path)])`, `None` outside a repository.
    fn git_status(&mut self) -> Option<(String, Vec<(String, String)>)> {
        None
    }
    fn dark_theme(&mut self) -> bool {
        true
    }
    /// Writes a `.nxpack` (Deflate ZIP) of project files and, optionally, the active
    /// document's current text. Paths and limits are enforced by the implementation.
    fn create_archive(
        &mut self,
        _request: &ArchiveRequest,
    ) -> Result<ArchiveSummary, &'static str> {
        Err("not supported")
    }
    /// Writes the full active document as a PDF to a project-relative `.pdf` path.
    fn export_document_pdf(&mut self, _output: &str) -> Result<PdfSummary, &'static str> {
        Err("not supported")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveRequest {
    pub output: String,
    pub files: Vec<String>,
    pub include_active_document: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginFailure {
    Incompatible(String),
    HandshakeTimeout,
    ProtocolViolation(String),
    MessageTooLarge,
    RateLimited,
    ActionTimeout(String),
    ProcessExited(Option<u32>),
    Io(String),
}

impl fmt::Display for PluginFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incompatible(reason) => write!(f, "incompatible: {reason}"),
            Self::HandshakeTimeout => write!(f, "plugin did not answer the handshake in time"),
            Self::ProtocolViolation(reason) => write!(f, "protocol violation: {reason}"),
            Self::MessageTooLarge => write!(f, "plugin sent a message above the size limit"),
            Self::RateLimited => write!(f, "plugin exceeded the request rate limit"),
            Self::ActionTimeout(action) => write!(f, "action {action} did not finish in time"),
            Self::ProcessExited(Some(code)) => write!(f, "plugin process exited ({code})"),
            Self::ProcessExited(None) => write!(f, "plugin process ended unexpectedly"),
            Self::Io(reason) => write!(f, "I/O error: {reason}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginState {
    Starting,
    Active,
    Stopping,
    Stopped,
    Failed(PluginFailure),
}

impl PluginState {
    pub fn is_final(&self) -> bool {
        matches!(self, Self::Stopped | Self::Failed(_))
    }
}

#[derive(Debug)]
pub enum StartError {
    AlreadyRunning,
    Incompatible(CompatibilityError),
    Permissions(String),
    Launch(String),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyRunning => write!(f, "plugin is already running"),
            Self::Incompatible(error) => write!(f, "{error}"),
            Self::Permissions(reason) => write!(f, "{reason}"),
            Self::Launch(reason) => write!(f, "plugin could not be started: {reason}"),
        }
    }
}

/// A user-invokable action a plugin declared in its `initialize` result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginAction {
    pub id: String,
    pub title: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ActionError {
    UnknownPlugin,
    NotActive,
    UnknownAction,
    /// One action runs at a time per plugin.
    Busy,
}

impl fmt::Display for ActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPlugin => write!(f, "plugin is not running"),
            Self::NotActive => write!(f, "plugin is not active"),
            Self::UnknownAction => write!(f, "plugin does not offer this action"),
            Self::Busy => write!(f, "plugin is still running another action"),
        }
    }
}

/// How an invoked action ended; `Ok` carries the optional message the plugin returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionEvent {
    pub plugin: String,
    pub action: String,
    pub outcome: Result<Option<String>, String>,
}

struct PendingAction {
    id: u64,
    action: String,
    deadline: Instant,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
}

enum Inbound {
    Frame(Frame),
    Invalid(String),
    TooLarge,
    Closed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PendingKind {
    Initialize,
    Shutdown,
}

struct Session {
    process: Option<Box<dyn PluginProcess>>,
    outbox: SyncSender<Vec<u8>>,
    inbox: Receiver<Inbound>,
    granted: Vec<String>,
    state: PluginState,
    deadline: Option<Instant>,
    pending: Option<(u64, PendingKind)>,
    next_id: u64,
    window_start: Instant,
    window_requests: u32,
    logs: VecDeque<LogEntry>,
    status: String,
    actions: Vec<PluginAction>,
    action_pending: Option<PendingAction>,
    events: VecDeque<ActionEvent>,
    plugin_id: String,
}

#[derive(Default)]
pub struct PluginHost {
    sessions: HashMap<String, Session>,
    limits: HostLimits,
}

impl PluginHost {
    pub fn new(limits: HostLimits) -> Self {
        Self {
            sessions: HashMap::new(),
            limits,
        }
    }

    /// Starts a plugin. `granted` are the IDE permissions the user confirmed; they
    /// must be a subset of what the manifest declares. Nothing is launched when the
    /// plugin is incompatible or the permission set is invalid.
    pub fn start(
        &mut self,
        manifest: &PluginManifest,
        granted: &[String],
        launch: impl FnOnce() -> Result<PluginIo, String>,
    ) -> Result<(), StartError> {
        if self
            .sessions
            .get(&manifest.id)
            .is_some_and(|session| !session.state.is_final())
        {
            return Err(StartError::AlreadyRunning);
        }
        protocol::check_compatibility(&manifest.ide_api).map_err(StartError::Incompatible)?;
        protocol::validate_ide_permissions(granted).map_err(StartError::Permissions)?;
        if let Some(extra) = granted
            .iter()
            .find(|permission| !manifest.ide_permissions.contains(permission))
        {
            return Err(StartError::Permissions(format!(
                "permission {extra} is not declared in the manifest"
            )));
        }

        let io = launch().map_err(StartError::Launch)?;
        let (outbox, outgoing) = mpsc::sync_channel::<Vec<u8>>(OUTBOX_FRAMES);
        let (incoming, inbox) = mpsc::sync_channel::<Inbound>(INBOX_FRAMES);
        let mut input = io.input;
        std::thread::spawn(move || {
            for line in outgoing {
                if input.write_all(&line).and_then(|()| input.flush()).is_err() {
                    break;
                }
            }
        });
        let output = io.output;
        std::thread::spawn(move || read_frames(output, incoming));

        let now = Instant::now();
        let mut session = Session {
            process: Some(io.process),
            outbox,
            inbox,
            granted: granted.to_vec(),
            state: PluginState::Starting,
            deadline: Some(now + self.limits.handshake_timeout),
            pending: None,
            next_id: 1,
            window_start: now,
            window_requests: 0,
            logs: VecDeque::new(),
            status: String::new(),
            actions: Vec::new(),
            action_pending: None,
            events: VecDeque::new(),
            plugin_id: manifest.id.clone(),
        };
        let methods: Vec<&str> = session
            .granted
            .iter()
            .flat_map(|permission| {
                protocol::API_METHODS
                    .iter()
                    .filter(move |method| method.permission == permission)
                    .map(|method| method.name)
            })
            .collect();
        let params = json!({
            "protocol": protocol::PROTOCOL_VERSION,
            "ide_api": IDE_API_VERSION.to_string(),
            "plugin_id": manifest.id,
            "methods": methods,
        });
        session.send_request(METHOD_INITIALIZE, params, PendingKind::Initialize);
        self.sessions.insert(manifest.id.clone(), session);
        Ok(())
    }

    /// Asks the plugin to shut down; [`pump`](Self::pump) completes the transition.
    pub fn stop(&mut self, id: &str) {
        let Some(session) = self.sessions.get_mut(id) else {
            return;
        };
        match session.state {
            PluginState::Starting | PluginState::Active => {
                session.state = PluginState::Stopping;
                session.deadline = Some(Instant::now() + self.limits.shutdown_grace);
                session.send_request(METHOD_SHUTDOWN, json!({}), PendingKind::Shutdown);
            }
            _ => {}
        }
    }

    /// Replaces a running plugin: the old process is ended immediately (no shutdown
    /// handshake), then the new one is launched. Nothing is stopped when the new
    /// start would be rejected before launch (incompatible or invalid permissions).
    pub fn restart(
        &mut self,
        manifest: &PluginManifest,
        granted: &[String],
        launch: impl FnOnce() -> Result<PluginIo, String>,
    ) -> Result<(), StartError> {
        protocol::check_compatibility(&manifest.ide_api).map_err(StartError::Incompatible)?;
        protocol::validate_ide_permissions(granted).map_err(StartError::Permissions)?;
        if let Some(extra) = granted
            .iter()
            .find(|permission| !manifest.ide_permissions.contains(permission))
        {
            return Err(StartError::Permissions(format!(
                "permission {extra} is not declared in the manifest"
            )));
        }
        self.terminate(&manifest.id);
        self.start(manifest, granted, launch)
    }

    /// Actions offered by the active plugin; empty in every other state.
    pub fn actions(&self, id: &str) -> &[PluginAction] {
        match self.sessions.get(id) {
            Some(session) if session.state == PluginState::Active => &session.actions,
            _ => &[],
        }
    }

    /// `(plugin id, action)` of all active plugins, sorted for stable menus.
    pub fn active_actions(&self) -> Vec<(String, PluginAction)> {
        let mut all: Vec<(String, PluginAction)> = self
            .sessions
            .iter()
            .filter(|(_, session)| session.state == PluginState::Active)
            .flat_map(|(id, session)| session.actions.iter().map(|a| (id.clone(), a.clone())))
            .collect();
        all.sort_by(|a, b| (&a.0, &a.1.id).cmp(&(&b.0, &b.1.id)));
        all
    }

    /// Sends `action.invoke` to the plugin. The outcome arrives through
    /// [`take_action_events`](Self::take_action_events).
    pub fn invoke_action(&mut self, id: &str, action: &str) -> Result<(), ActionError> {
        let timeout = self.limits.action_timeout;
        let session = self
            .sessions
            .get_mut(id)
            .ok_or(ActionError::UnknownPlugin)?;
        if session.state != PluginState::Active {
            return Err(ActionError::NotActive);
        }
        if !session
            .actions
            .iter()
            .any(|candidate| candidate.id == action)
        {
            return Err(ActionError::UnknownAction);
        }
        if session.action_pending.is_some() {
            return Err(ActionError::Busy);
        }
        let request_id = session.next_id;
        session.next_id += 1;
        session.action_pending = Some(PendingAction {
            id: request_id,
            action: action.to_owned(),
            deadline: Instant::now() + timeout,
        });
        session.send(&Frame::request(
            request_id,
            METHOD_ACTION_INVOKE,
            json!({ "action": action }),
        ));
        Ok(())
    }

    /// Drains finished actions of all plugins.
    pub fn take_action_events(&mut self) -> Vec<ActionEvent> {
        self.sessions
            .values_mut()
            .flat_map(|session| session.events.drain(..))
            .collect()
    }

    /// Ends the plugin process immediately.
    pub fn terminate(&mut self, id: &str) {
        if let Some(session) = self.sessions.get_mut(id)
            && !session.state.is_final()
        {
            session.release_process();
            session.state = PluginState::Stopped;
            session.deadline = None;
        }
    }

    pub fn terminate_all(&mut self) {
        let ids: Vec<String> = self.sessions.keys().cloned().collect();
        for id in ids {
            self.terminate(&id);
        }
    }

    pub fn state(&self, id: &str) -> Option<&PluginState> {
        self.sessions.get(id).map(|session| &session.state)
    }

    pub fn logs(&self, id: &str) -> impl Iterator<Item = &LogEntry> {
        self.sessions
            .get(id)
            .into_iter()
            .flat_map(|session| session.logs.iter())
    }

    pub fn status_text(&self, id: &str) -> &str {
        self.sessions
            .get(id)
            .map_or("", |session| session.status.as_str())
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.sessions.keys().map(String::as_str)
    }

    pub fn has_live_plugins(&self) -> bool {
        self.sessions
            .values()
            .any(|session| !session.state.is_final())
    }

    /// Processes queued plugin messages, deadlines and process exits.
    pub fn pump(&mut self, services: &mut dyn IdeServices) {
        let limits = self.limits.clone();
        for session in self.sessions.values_mut() {
            session.pump(&limits, services);
        }
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        self.terminate_all();
    }
}

impl Session {
    fn pump(&mut self, limits: &HostLimits, services: &mut dyn IdeServices) {
        if self.state.is_final() {
            return;
        }
        for _ in 0..limits.max_frames_per_pump {
            let Ok(inbound) = self.inbox.try_recv() else {
                break;
            };
            match inbound {
                Inbound::Frame(frame) => self.handle_frame(frame, limits, services),
                Inbound::Invalid(reason) => self.fail(PluginFailure::ProtocolViolation(reason)),
                Inbound::TooLarge => self.fail(PluginFailure::MessageTooLarge),
                Inbound::Closed => self.output_closed(),
            }
            if self.state.is_final() {
                return;
            }
        }
        if let Some(code) = self
            .process
            .as_ref()
            .and_then(|process| process.exit_code())
        {
            if self.state == PluginState::Stopping {
                self.finish_stop();
            } else {
                self.fail(PluginFailure::ProcessExited(Some(code)));
            }
            return;
        }
        if self.state == PluginState::Active
            && let Some(pending) = &self.action_pending
            && Instant::now() >= pending.deadline
        {
            let action = pending.action.clone();
            self.fail(PluginFailure::ActionTimeout(action));
            return;
        }
        if self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            match self.state {
                PluginState::Starting => self.fail(PluginFailure::HandshakeTimeout),
                _ => self.finish_stop(),
            }
        }
    }

    fn output_closed(&mut self) {
        if self.state == PluginState::Stopping {
            self.finish_stop();
        } else {
            let code = self
                .process
                .as_ref()
                .and_then(|process| process.exit_code());
            self.fail(PluginFailure::ProcessExited(code));
        }
    }

    fn fail(&mut self, reason: PluginFailure) {
        self.release_process();
        self.state = PluginState::Failed(reason);
        self.deadline = None;
    }

    fn finish_stop(&mut self) {
        self.release_process();
        self.state = PluginState::Stopped;
        self.deadline = None;
    }

    /// Kills the process and drops it, which also releases the pinned executable.
    fn release_process(&mut self) {
        if let Some(process) = self.process.take() {
            process.kill();
        }
        self.actions.clear();
        if let Some(pending) = self.action_pending.take() {
            self.push_event(
                pending.action,
                Err("plugin ended before the action finished".into()),
            );
        }
    }

    fn push_event(&mut self, action: String, outcome: Result<Option<String>, String>) {
        if self.events.len() == MAX_ACTION_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(ActionEvent {
            plugin: self.plugin_id.clone(),
            action,
            outcome,
        });
    }

    fn send(&mut self, frame: &Frame) -> bool {
        let line = frame.to_line();
        let line = if line.len() > MAX_FRAME_BYTES {
            Frame::failure(
                frame.id,
                code::UNAVAILABLE,
                "response exceeds the size limit",
            )
            .to_line()
        } else {
            line
        };
        match self.outbox.try_send(line) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                self.fail(PluginFailure::Io("plugin does not read its input".into()));
                false
            }
            Err(TrySendError::Disconnected(_)) => {
                self.fail(PluginFailure::Io("plugin input is closed".into()));
                false
            }
        }
    }

    fn send_request(&mut self, method: &str, params: Value, purpose: PendingKind) {
        let id = self.next_id;
        self.next_id += 1;
        self.pending = Some((id, purpose));
        self.send(&Frame::request(id, method, params));
    }

    fn handle_frame(&mut self, frame: Frame, limits: &HostLimits, services: &mut dyn IdeServices) {
        match frame.kind {
            Kind::Response => self.handle_response(frame),
            Kind::Request => self.handle_request(frame, limits, services),
        }
    }

    fn handle_response(&mut self, frame: Frame) {
        if let Some(action) = self.action_pending.take_if(|p| p.id == frame.id) {
            let outcome = match frame.error {
                Some(error) => Err(error.message.chars().take(MAX_STATUS_CHARS).collect()),
                None => Ok(frame
                    .result
                    .as_ref()
                    .and_then(|result| result.get("message"))
                    .and_then(Value::as_str)
                    .map(|message| message.chars().take(MAX_STATUS_CHARS).collect())),
            };
            self.push_event(action.action, outcome);
            return;
        }
        let Some((id, purpose)) = self.pending.take_if(|(id, _)| *id == frame.id) else {
            self.fail(PluginFailure::ProtocolViolation(format!(
                "response {} does not answer a pending request",
                frame.id
            )));
            return;
        };
        debug_assert_eq!(id, frame.id);
        match purpose {
            PendingKind::Shutdown => self.finish_stop(),
            PendingKind::Initialize => {
                if let Some(error) = frame.error {
                    self.fail(PluginFailure::ProtocolViolation(format!(
                        "plugin refused initialize: {}",
                        error.code
                    )));
                    return;
                }
                let requirement = frame
                    .result
                    .as_ref()
                    .and_then(|result| result.get("ide_api"))
                    .and_then(Value::as_str);
                let actions = match parse_actions(frame.result.as_ref()) {
                    Ok(actions) => actions,
                    Err(reason) => {
                        self.fail(PluginFailure::ProtocolViolation(reason));
                        return;
                    }
                };
                match requirement.map(protocol::check_compatibility) {
                    Some(Ok(())) => {
                        self.actions = actions;
                        self.state = PluginState::Active;
                        self.deadline = None;
                    }
                    Some(Err(error)) => self.fail(PluginFailure::Incompatible(error.to_string())),
                    None => self.fail(PluginFailure::ProtocolViolation(
                        "initialize result lacks ide_api".into(),
                    )),
                }
            }
        }
    }

    fn handle_request(
        &mut self,
        frame: Frame,
        limits: &HostLimits,
        services: &mut dyn IdeServices,
    ) {
        if self.window_start.elapsed() >= Duration::from_secs(1) {
            self.window_start = Instant::now();
            self.window_requests = 0;
        }
        self.window_requests += 1;
        if self.window_requests > limits.max_requests_per_second {
            self.send(&Frame::failure(
                frame.id,
                code::RATE_LIMITED,
                "too many requests",
            ));
            self.fail(PluginFailure::RateLimited);
            return;
        }
        let method = frame.method.as_deref().unwrap_or_default();
        let outcome = match self.state {
            PluginState::Active => self.dispatch(method, frame.params.as_ref(), services),
            PluginState::Starting => Err((code::NOT_INITIALIZED, "initialize has not completed")),
            _ => Err((code::UNAVAILABLE, "plugin is shutting down")),
        };
        let reply = match outcome {
            Ok(result) => Frame::success(frame.id, result),
            Err((error_code, message)) => Frame::failure(frame.id, error_code, message),
        };
        self.send(&reply);
    }

    fn dispatch(
        &mut self,
        method: &str,
        params: Option<&Value>,
        services: &mut dyn IdeServices,
    ) -> Result<Value, (&'static str, &'static str)> {
        let api = ApiMethod::find(method).ok_or((code::UNKNOWN_METHOD, "unknown method"))?;
        if !api.available() {
            return Err((
                code::UNAVAILABLE,
                "method is not part of this IDE API version",
            ));
        }
        if !self
            .granted
            .iter()
            .any(|permission| permission == api.permission)
        {
            return Err((code::PERMISSION_DENIED, "permission was not granted"));
        }
        let invalid = (code::INVALID_PARAMS, "invalid parameters");
        match api.name {
            "log" => {
                let params = params.ok_or(invalid)?;
                let level = params.get("level").and_then(Value::as_str).ok_or(invalid)?;
                let message = params
                    .get("message")
                    .and_then(Value::as_str)
                    .ok_or(invalid)?;
                if !matches!(level, "info" | "warn" | "error") {
                    return Err(invalid);
                }
                if self.logs.len() == MAX_LOG_ENTRIES {
                    self.logs.pop_front();
                }
                self.logs.push_back(LogEntry {
                    level: level.to_owned(),
                    message: message.chars().take(MAX_LOG_MESSAGE_CHARS).collect(),
                });
                Ok(json!({}))
            }
            "status.set" => {
                let text = params
                    .and_then(|params| params.get("text"))
                    .and_then(Value::as_str)
                    .ok_or(invalid)?;
                self.status = text.chars().take(MAX_STATUS_CHARS).collect();
                Ok(json!({}))
            }
            "document.active" => {
                let Some(mut document) = services.active_document() else {
                    return Ok(Value::Null);
                };
                let truncated = document.text.len() > MAX_DOCUMENT_BYTES;
                if truncated {
                    let mut end = MAX_DOCUMENT_BYTES;
                    while !document.text.is_char_boundary(end) {
                        end -= 1;
                    }
                    document.text.truncate(end);
                }
                Ok(json!({
                    "name": document.name,
                    "language": document.language,
                    "text": document.text,
                    "truncated": truncated,
                }))
            }
            "project.files" => {
                let mut files = services.project_files();
                let truncated = files.len() > MAX_LISTED_FILES;
                files.truncate(MAX_LISTED_FILES);
                Ok(json!({ "files": files, "truncated": truncated }))
            }
            "diagnostics.active" => {
                let mut diagnostics = services.diagnostics();
                let truncated = diagnostics.len() > MAX_LISTED_DIAGNOSTICS;
                diagnostics.truncate(MAX_LISTED_DIAGNOSTICS);
                Ok(json!({ "diagnostics": diagnostics, "truncated": truncated }))
            }
            "document.replace" => {
                let text = params
                    .and_then(|params| params.get("text"))
                    .and_then(Value::as_str)
                    .ok_or(invalid)?;
                if text.len() > MAX_WRITE_BYTES {
                    return Err(invalid);
                }
                services
                    .replace_document(text.to_owned())
                    .map_err(|message| (code::UNAVAILABLE, message))?;
                Ok(json!({}))
            }
            "file.read" => {
                let path = string_param(params, "path").ok_or(invalid)?;
                let text = services
                    .read_file(path)
                    .map_err(|message| (code::UNAVAILABLE, message))?;
                Ok(json!({ "text": text }))
            }
            "file.write" => {
                let path = string_param(params, "path").ok_or(invalid)?;
                let text = string_param(params, "text").ok_or(invalid)?;
                if text.len() > MAX_WRITE_BYTES {
                    return Err(invalid);
                }
                services
                    .write_file(path, text)
                    .map_err(|message| (code::UNAVAILABLE, message))?;
                Ok(json!({}))
            }
            "notification.show" => {
                let text = string_param(params, "text").ok_or(invalid)?;
                services.notify(text.chars().take(MAX_STATUS_CHARS).collect());
                Ok(json!({}))
            }
            "search.text" => {
                let query = string_param(params, "query").ok_or(invalid)?;
                if query.is_empty() || query.len() > 200 {
                    return Err(invalid);
                }
                let matches: Vec<Value> = services
                    .search_text(query, MAX_SEARCH_MATCHES)
                    .into_iter()
                    .map(|(file, line, text)| json!({ "file": file, "line": line, "text": text }))
                    .collect();
                Ok(json!({ "matches": matches }))
            }
            "git.status" => Ok(match services.git_status() {
                Some((branch, changes)) => json!({
                    "branch": branch,
                    "changes": changes
                        .into_iter()
                        .take(MAX_LISTED_FILES)
                        .map(|(status, path)| json!({ "status": status, "path": path }))
                        .collect::<Vec<_>>(),
                }),
                None => Value::Null,
            }),
            "archive.create" => {
                let output = string_param(params, "output").ok_or(invalid)?;
                let files = params
                    .and_then(|params| params.get("files"))
                    .and_then(Value::as_array)
                    .ok_or(invalid)?;
                let include_active_document =
                    match params.and_then(|p| p.get("include_active_document")) {
                        None => false,
                        Some(value) => value.as_bool().ok_or(invalid)?,
                    };
                if files.len() > MAX_ARCHIVE_ENTRIES || output.chars().count() > MAX_PATH_CHARS {
                    return Err(invalid);
                }
                let files = files
                    .iter()
                    .map(|file| {
                        file.as_str()
                            .filter(|name| name.chars().count() <= MAX_PATH_CHARS)
                            .map(str::to_owned)
                            .ok_or(invalid)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let summary = services
                    .create_archive(&ArchiveRequest {
                        output: output.to_owned(),
                        files,
                        include_active_document,
                    })
                    .map_err(|message| (code::UNAVAILABLE, message))?;
                Ok(json!({
                    "path": summary.path,
                    "entries": summary.entries,
                    "uncompressed_bytes": summary.uncompressed_bytes,
                    "archive_bytes": summary.archive_bytes,
                }))
            }
            "document.export_pdf" => {
                let output = string_param(params, "output").ok_or(invalid)?;
                if output.chars().count() > MAX_PATH_CHARS {
                    return Err(invalid);
                }
                let summary = services
                    .export_document_pdf(output)
                    .map_err(|message| (code::UNAVAILABLE, message))?;
                Ok(json!({
                    "path": summary.path,
                    "pages": summary.pages,
                    "bytes": summary.bytes,
                }))
            }
            "theme.current" => Ok(json!({
                "name": if services.dark_theme() { "dark" } else { "light" },
            })),
            _ => Err((code::UNKNOWN_METHOD, "unknown method")),
        }
    }
}

/// Reads the optional `actions` of an `initialize` result; any malformed or
/// excessive declaration rejects the handshake.
fn parse_actions(result: Option<&Value>) -> Result<Vec<PluginAction>, String> {
    let Some(list) = result.and_then(|result| result.get("actions")) else {
        return Ok(Vec::new());
    };
    let list = list.as_array().ok_or("actions must be an array")?;
    if list.len() > MAX_ACTIONS {
        return Err(format!("more than {MAX_ACTIONS} actions declared"));
    }
    let mut actions: Vec<PluginAction> = Vec::with_capacity(list.len());
    for entry in list {
        let id = entry.get("id").and_then(Value::as_str).unwrap_or_default();
        let title = entry
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        let id_valid = !id.is_empty()
            && id.chars().count() <= MAX_ACTION_ID_CHARS
            && id
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b));
        let title_valid = !title.is_empty()
            && title.chars().count() <= MAX_ACTION_TITLE_CHARS
            && !title.chars().any(char::is_control);
        if !id_valid || !title_valid {
            return Err("invalid action declaration".into());
        }
        if actions.iter().any(|action| action.id == id) {
            return Err(format!("duplicate action {id}"));
        }
        actions.push(PluginAction {
            id: id.to_owned(),
            title: title.to_owned(),
        });
    }
    Ok(actions)
}

fn string_param<'a>(params: Option<&'a Value>, key: &str) -> Option<&'a str> {
    params?.get(key)?.as_str()
}

/// Reads newline-delimited frames until the stream ends or violates the protocol.
fn read_frames(output: Box<dyn Read + Send>, incoming: SyncSender<Inbound>) {
    let mut reader = BufReader::new(output);
    let mut line = Vec::new();
    loop {
        line.clear();
        let read = reader
            .by_ref()
            .take(MAX_FRAME_BYTES as u64 + 2)
            .read_until(b'\n', &mut line);
        let message = match read {
            Ok(0) | Err(_) => Inbound::Closed,
            Ok(_) if line.last() != Some(&b'\n') && line.len() > MAX_FRAME_BYTES => {
                Inbound::TooLarge
            }
            Ok(_) if line.last() != Some(&b'\n') => Inbound::Closed,
            Ok(_) => {
                line.pop();
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                match Frame::parse(&line) {
                    Ok(frame) => Inbound::Frame(frame),
                    Err(error) => Inbound::Invalid(error.0),
                }
            }
        };
        let last = !matches!(message, Inbound::Frame(_));
        if incoming.send(message).is_err() || last {
            return;
        }
    }
}
