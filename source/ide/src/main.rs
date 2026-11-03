#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod marketplace;
mod shell;

use shell::*;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{self, FontId};
use marketplace::{
    MarketplaceClient, MarketplaceExtension, MarketplaceSubmission, MarketplaceUpdate,
};
use nortex_compiler::manifest::{self, NortexManifest};
use nortex_ide::account::{AccountService, LocalOnlyService};
use nortex_ide::app::Application;
use nortex_ide::debugger::{Debugger, LaunchConfig};
use nortex_ide::diagnostics::{IdeDiagnostic, collect};
use nortex_ide::editor::EditorBuffer;
use nortex_ide::editor::completion::Completion;
use nortex_ide::editor::completion_worker::{
    CompletionOutcome, CompletionRequest, CompletionWorker,
};
use nortex_ide::editor::diagnostics as inline;
use nortex_ide::editor::find_replace::{self, FindState};
use nortex_ide::editor::gutter::{gutter_text_marked, line_count};
use nortex_ide::editor::highlighting::{HighlightSpan, highlight_nortex};
use nortex_ide::editor::lines;
use nortex_ide::editor::refactor::{self, RenameState};
use nortex_ide::editor::scope;
use nortex_ide::editor::split::{self, DropZone, PaneGrid, SplitMode, ViewState};
use nortex_ide::git::{self, GitStatus};
use nortex_ide::i18n::tr;
use nortex_ide::plugins::PluginRegistry;
use nortex_ide::search::{self, Hit};
use nortex_ide::terminal::Terminal;
use nortex_ide::toolchain::{self, RunGate, RunOutcome, ToolchainStatus};
use nortex_ide::ui::activity_bar::{self, ActivityAction, SideView, ViewMode, Workspace};
use nortex_ide::ui::branding::{self, BrandTextures, FileKind};
use nortex_ide::ui::command_palette::{Command as PaletteCommand, CommandPalette};
use nortex_ide::ui::components::{ButtonKind, FileTabState, IndustrialButton, file_tab};
use nortex_ide::ui::dashboard::{self, DashboardAction, DashboardData};
use nortex_ide::ui::find_bar::{self, FindEvent};
use nortex_ide::ui::keymap::{self, Action};
use nortex_ide::ui::layout;
use nortex_ide::ui::settings::{self, Settings};
use nortex_ide::ui::settings_catalog::SettingLevel;
use nortex_ide::ui::theme::editor::EditorPalette;
use nortex_ide::ui::theme::spacing::Density;
use nortex_ide::ui::theme::{Theme, ThemeKind, panel_header, typography};
use nortex_ide::ui::{breadcrumbs, minimap};

const MAX_PROJECT_FILES: usize = 500;
const MAX_PROJECT_DEPTH: usize = 5;
const USER_MANUAL: [&str; 13] = [
    include_str!("../../docs/user-manual/01-welcome.md"),
    include_str!("../../docs/user-manual/02-installation.md"),
    include_str!("../../docs/user-manual/03-interface-overview.md"),
    include_str!("../../docs/user-manual/04-projects-and-files.md"),
    include_str!("../../docs/user-manual/05-editor.md"),
    include_str!("../../docs/user-manual/06-build-and-run.md"),
    include_str!("../../docs/user-manual/07-terminal-and-consoles.md"),
    include_str!("../../docs/user-manual/08-debugging.md"),
    include_str!("../../docs/user-manual/09-settings-and-customization.md"),
    include_str!("../../docs/user-manual/10-keyboard-shortcuts.md"),
    include_str!("../../docs/user-manual/11-first-project.md"),
    include_str!("../../docs/user-manual/12-efficient-workflows.md"),
    include_str!("../../docs/user-manual/13-faq-and-troubleshooting.md"),
];

/// Release builds have no console, so startup failures are written to `ide_crash.log`
/// next to the executable (falling back to the OS temp directory).
fn write_crash_log(message: &str) {
    use std::io::Write;

    let candidates = [
        std::env::current_exe().ok().and_then(|path| {
            path.parent()
                .map(|directory| directory.join("ide_crash.log"))
        }),
        Some(std::env::temp_dir().join("ide_crash.log")),
    ];
    for path in candidates.into_iter().flatten() {
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(file, "{message}");
            return;
        }
    }
}

fn main() -> eframe::Result<()> {
    std::panic::set_hook(Box::new(|info| {
        write_crash_log(&format!("NORTEX Code: Panic: {info}"));
        eprintln!("NORTEX Code: Panic: {info}");
    }));
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("NORTEX Code")
        .with_decorations(false)
        .with_inner_size([1280.0, 800.0])
        .with_min_inner_size([400.0, 300.0]);
    if let Some(icon) = branding::window_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    let result = eframe::run_native(
        "NORTEX Code",
        options,
        Box::new(|context| Ok(Box::new(NortexCodeApp::new(context)))),
    );
    if let Err(error) = &result {
        write_crash_log(&format!("NORTEX Code: Start fehlgeschlagen: {error}"));
    }
    result
}

struct AnalysisResult {
    revision: u64,
    diagnostics: Vec<IdeDiagnostic>,
}

struct ProjectSnapshot {
    project_root: PathBuf,
    source_files: Vec<PathBuf>,
    manifest: Option<(PathBuf, NortexManifest)>,
}

struct ProjectLoadResult {
    revision: u64,
    result: io::Result<ProjectSnapshot>,
}

struct GitLoadResult {
    revision: u64,
    project_root: PathBuf,
    status: GitStatus,
}

enum RunEvent {
    Compiler(String),
    Runtime(String),
    Finished(RunOutcome),
}

#[allow(dead_code)]
enum MarketplaceEvent {
    Search(Result<Vec<MarketplaceExtension>, String>),
    Updates(Result<Vec<MarketplaceUpdate>, String>),
    Favorites(Result<Vec<MarketplaceExtension>, String>),
    Auth(Result<(String, String), String>),
    Logout(Result<(), String>),
    Action(Result<String, String>),
    Upload(Result<i64, String>),
    Installed(Result<Vec<(String, String)>, String>),
    Submissions(Result<Vec<MarketplaceSubmission>, String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExtensionRunRequest {
    id: String,
    name: String,
    installed_dir: PathBuf,
    executable: PathBuf,
    entry: String,
    entry_sha256: String,
    permissions: Vec<String>,
    /// Set when the manifest declares an IDE API requirement; the extension then
    /// runs as a hosted plugin with IPC instead of as a batch process.
    hosted: Option<nortex_ide::plugins::protocol::PluginManifest>,
}

enum ExtensionRunEvent {
    Finished {
        id: String,
        result: Result<nortex_ide::sandbox::SandboxOutput, String>,
    },
    HostedReady {
        manifest: nortex_ide::plugins::protocol::PluginManifest,
        result: Result<(Box<nortex_ide::sandbox::SandboxRequest>, Option<PathBuf>), String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConsoleTab {
    CompilerLog,
    RuntimeOutput,
    Terminal,
}

struct AnalysisState {
    pending: Option<(u64, String, String)>,
    shutting_down: bool,
}

struct AnalysisWorker {
    shared: Arc<(Mutex<AnalysisState>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}

impl AnalysisWorker {
    fn new() -> io::Result<(Self, mpsc::Receiver<AnalysisResult>)> {
        let shared = Arc::new((
            Mutex::new(AnalysisState {
                pending: None,
                shutting_down: false,
            }),
            Condvar::new(),
        ));
        let worker_state = Arc::clone(&shared);
        let (sender, results) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("nortex-diagnostics".into())
            .spawn(move || {
                loop {
                    let job = {
                        let (lock, condition) = &*worker_state;
                        let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
                        while state.pending.is_none() && !state.shutting_down {
                            state = condition
                                .wait(state)
                                .unwrap_or_else(|error| error.into_inner());
                        }
                        if state.shutting_down {
                            break;
                        }
                        state.pending.take()
                    };
                    let Some((revision, file, source)) = job else {
                        continue;
                    };
                    let diagnostics = collect(&file, &source);
                    if sender
                        .send(AnalysisResult {
                            revision,
                            diagnostics,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })?;
        Ok((
            Self {
                shared,
                thread: Some(thread),
            },
            results,
        ))
    }

    fn submit(&self, revision: u64, file: String, source: String) {
        let (lock, condition) = &*self.shared;
        let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
        state.pending = Some((revision, file, source));
        condition.notify_one();
    }
}

impl Drop for AnalysisWorker {
    fn drop(&mut self) {
        let (lock, condition) = &*self.shared;
        let mut state = lock.lock().unwrap_or_else(|error| error.into_inner());
        state.shutting_down = true;
        condition.notify_one();
        drop(state);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct NortexCodeApp {
    documents: Application,
    project_root: PathBuf,
    project_path_input: String,
    source_files: Vec<PathBuf>,
    editor_text: String,
    synced_text: String,
    highlight_spans: Vec<HighlightSpan>,
    diagnostics: Vec<IdeDiagnostic>,
    analysis_revision: u64,
    analysis_worker: Option<AnalysisWorker>,
    analysis_results: mpsc::Receiver<AnalysisResult>,
    project_sender: mpsc::Sender<ProjectLoadResult>,
    project_results: mpsc::Receiver<ProjectLoadResult>,
    project_refresh_revision: u64,
    project_refresh_running: bool,
    project_refresh_pending: Option<PathBuf>,
    run_sender: mpsc::Sender<RunEvent>,
    run_results: mpsc::Receiver<RunEvent>,
    run_gate: RunGate,
    compiler_log: String,
    build_squiggles: Vec<inline::InlineDiagnostic>,
    completion: Option<Completion>,
    completion_worker: Option<CompletionWorker>,
    grid: PaneGrid,
    pending_restore: Option<ViewState>,
    live_cursor: usize,
    editor_width: f32,
    find: FindState,
    manifest: Option<(PathBuf, NortexManifest)>,
    window_title: String,
    runtime_output: String,
    console_tab: ConsoleTab,
    terminal: Option<Terminal>,
    terminal_input: String,
    terminal_error: Option<String>,
    last_run: Option<RunOutcome>,
    sidecar_prompt: String,
    status: String,
    problems_visible: bool,
    sidecar_visible: bool,
    sidecar_forced: bool,
    sidecar_auto_collapsed: bool,
    zen_mode: bool,
    header_search: String,
    shortcuts_open: bool,
    about_open: bool,
    license_open: bool,
    goto_open: bool,
    goto_input: String,
    pending_close_document: Option<(usize, usize)>,
    startup_pending: bool,
    debugger: Debugger,
    account: Box<dyn AccountService>,
    account_open: bool,
    last_auto_save: f64,
    settings_query: String,
    settings_level: Option<SettingLevel>,
    palette: CommandPalette,
    theme: Theme,
    settings_path: PathBuf,
    extensions_path: PathBuf,
    settings: Settings,
    ui_zoom_input: String,
    editor_zoom_input: String,
    workspace: Workspace,
    extensions_section: ExtensionSection,
    marketplace_url: String,
    marketplace_signing_key_hex: String,
    marketplace_query: String,
    #[allow(dead_code)]
    marketplace_username: String,
    marketplace_password: String,
    marketplace_token: Option<String>,
    marketplace_user_role: Option<String>,
    marketplace_results: Vec<MarketplaceExtension>,
    marketplace_favorites: Vec<MarketplaceExtension>,
    marketplace_updates: Vec<MarketplaceUpdate>,
    marketplace_submissions: Vec<MarketplaceSubmission>,
    marketplace_role_username: String,
    marketplace_role: String,
    marketplace_status: Option<String>,
    marketplace_busy: bool,
    marketplace_sender: mpsc::Sender<MarketplaceEvent>,
    marketplace_results_rx: mpsc::Receiver<MarketplaceEvent>,
    extension_run_sender: mpsc::Sender<ExtensionRunEvent>,
    extension_run_results_rx: mpsc::Receiver<ExtensionRunEvent>,
    extension_runs: std::collections::HashMap<String, Arc<AtomicBool>>,
    pending_extension_run: Option<ExtensionRunRequest>,
    extension_run_status: std::collections::BTreeMap<String, String>,
    plugin_host: nortex_ide::plugins::host::PluginHost,
    hosted_executables: std::collections::HashMap<String, PathBuf>,
    bundled_extensions: Vec<nortex_ide::plugins::bundle::BundledExtension>,
    textures: BrandTextures,
    toolchain: ToolchainStatus,
    git_status: GitStatus,
    git_sender: mpsc::Sender<GitLoadResult>,
    git_results: mpsc::Receiver<GitLoadResult>,
    git_refresh_revision: u64,
    git_refresh_running: bool,
    git_refresh_pending: bool,
    manual_open: bool,
    feedback: FeedbackState,
    git_panel: GitPanelState,
    search_query: String,
    search_hits: Vec<Hit>,
    rename: RenameState,
    rename_requested: bool,
    scroll_request: Option<f32>,
}

impl NortexCodeApp {
    fn new(context: &eframe::CreationContext<'_>) -> Self {
        let settings_path = settings::settings_path();
        let extensions_path = settings_path.with_file_name("extensions.json");
        let settings = Settings::load_and_repair(&settings_path);
        nortex_ide::i18n::set_language(settings.language);
        let theme = settings.theme();
        context.egui_ctx.set_fonts(typography::font_definitions());
        theme.apply(&context.egui_ctx);
        let mut textures = BrandTextures::default();
        textures.load(&context.egui_ctx);

        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let (run_sender, run_results) = mpsc::channel();
        let (project_sender, project_results) = mpsc::channel();
        let (git_sender, git_results) = mpsc::channel();
        let (marketplace_sender, marketplace_results_rx) = mpsc::channel();
        let (extension_run_sender, extension_run_results_rx) = mpsc::channel();
        let (analysis_worker, analysis_results, mut status) = match AnalysisWorker::new() {
            Ok((worker, results)) => (Some(worker), results, tr("Bereit").to_string()),
            Err(error) => {
                let (_, results) = mpsc::channel();
                (
                    None,
                    results,
                    nortex_ide::i18n::tr_fmt(
                        "Diagnose-Worker konnte nicht starten: {}",
                        &[error.to_string()],
                    ),
                )
            }
        };

        let repaint_context = context.egui_ctx.clone();
        let completion_worker =
            match CompletionWorker::spawn(move || repaint_context.request_repaint()) {
                Ok(worker) => Some(worker),
                Err(error) => {
                    status = nortex_ide::i18n::tr_fmt(
                        "Completion-Worker konnte nicht starten: {}",
                        &[error.to_string()],
                    );
                    None
                }
            };

        let mut documents = Application::new();
        match PluginRegistry::load(&extensions_path) {
            Ok(registry) => documents.plugins = registry,
            Err(error) => {
                status = nortex_ide::i18n::tr_fmt(
                    "Extension-Zustand konnte nicht geladen werden: {}",
                    &[error.to_string()],
                );
            }
        }

        let mut app = Self {
            documents,
            project_path_input: project_root.display().to_string(),
            project_root,
            source_files: Vec::new(),
            editor_text: String::new(),
            synced_text: String::new(),
            highlight_spans: Vec::new(),
            diagnostics: Vec::new(),
            analysis_revision: 0,
            analysis_worker,
            analysis_results,
            project_sender,
            project_results,
            project_refresh_revision: 0,
            project_refresh_running: false,
            project_refresh_pending: None,
            run_sender,
            run_results,
            run_gate: RunGate::default(),
            compiler_log: String::new(),
            build_squiggles: Vec::new(),
            completion: None,
            completion_worker,
            grid: PaneGrid::new(),
            pending_restore: None,
            live_cursor: 0,
            editor_width: 1200.0,
            find: FindState::default(),
            manifest: None,
            window_title: String::new(),
            runtime_output: String::new(),
            console_tab: ConsoleTab::CompilerLog,
            terminal: None,
            terminal_input: String::new(),
            terminal_error: None,
            last_run: None,
            sidecar_prompt: String::new(),
            status,
            problems_visible: true,
            sidecar_visible: false,
            sidecar_forced: false,
            sidecar_auto_collapsed: false,
            zen_mode: false,
            header_search: String::new(),
            shortcuts_open: false,
            about_open: false,
            license_open: false,
            goto_open: false,
            goto_input: String::new(),
            pending_close_document: None,
            startup_pending: true,
            debugger: Debugger::default(),
            account: Box::new(LocalOnlyService),
            account_open: false,
            last_auto_save: 0.0,
            settings_query: String::new(),
            settings_level: None,
            palette: CommandPalette::default(),
            theme,
            settings_path,
            extensions_path,
            settings,
            ui_zoom_input: zoom_input(theme.zoom.ui),
            editor_zoom_input: zoom_input(theme.zoom.editor),
            workspace: Workspace::default(),
            extensions_section: ExtensionSection::Marketplace,
            marketplace_url: std::env::var("NORTEX_MARKETPLACE_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8787".into()),
            marketplace_signing_key_hex: std::env::var("NORTEX_MARKET_SIGNING_PUBLIC_KEY_HEX")
                .unwrap_or_default(),
            marketplace_query: String::new(),
            marketplace_username: String::new(),
            marketplace_password: String::new(),
            marketplace_token: None,
            marketplace_user_role: None,
            marketplace_results: Vec::new(),
            marketplace_favorites: Vec::new(),
            marketplace_updates: Vec::new(),
            marketplace_submissions: Vec::new(),
            marketplace_role_username: String::new(),
            marketplace_role: "publisher".into(),
            marketplace_status: None,
            marketplace_busy: false,
            marketplace_sender,
            marketplace_results_rx,
            extension_run_sender,
            extension_run_results_rx,
            extension_runs: std::collections::HashMap::new(),
            pending_extension_run: None,
            extension_run_status: std::collections::BTreeMap::new(),
            plugin_host: nortex_ide::plugins::host::PluginHost::default(),
            hosted_executables: std::collections::HashMap::new(),
            bundled_extensions: [("bundled", true), ("available", false)]
                .into_iter()
                .filter_map(|(kind, bundled)| {
                    nortex_ide::plugins::bundle::bundle_root(kind)
                        .map(|root| nortex_ide::plugins::bundle::list(&root, bundled))
                })
                .flatten()
                .collect(),
            textures,
            toolchain: ToolchainStatus::detect_current(),
            git_status: GitStatus::Unavailable,
            git_sender,
            git_results,
            git_refresh_revision: 0,
            git_refresh_running: false,
            git_refresh_pending: false,
            manual_open: false,
            feedback: FeedbackState::default(),
            git_panel: GitPanelState::default(),
            search_query: String::new(),
            search_hits: Vec::new(),
            rename: RenameState::default(),
            rename_requested: false,
            scroll_request: None,
        };

        app.request_project_refresh();
        // Nothing is opened unasked: the dashboard shows recent files and projects.
        app.documents.new_document();
        app.workspace.view_mode = ViewMode::Dashboard;
        if let Some(arg) = std::env::args_os().nth(1).map(PathBuf::from)
            && arg.is_file()
        {
            app.open_file(arg);
        }
        app.schedule_analysis();
        app
    }
}

const RUN_SLOT_WIDTH: f32 = 170.0;
const RUN_SLOT_HEIGHT: f32 = 30.0;
const AMBER: egui::Color32 = egui::Color32::from_rgb(0xD6, 0xA6, 0x4B);
const SPLIT_LINE: egui::Color32 = egui::Color32::from_rgb(0x34, 0x3B, 0x43);

#[derive(Debug, Clone, Copy)]
struct TabDrag {
    pane: usize,
    doc: usize,
}

#[derive(Debug, Default)]
struct TabEvents {
    activate: Option<usize>,
    close: Option<usize>,
    new_doc: bool,
    reorder: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClosePromptAction {
    Save,
    Discard,
    Cancel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_sync_is_unicode_safe_and_one_step_undoable() {
        let old = "NORTEX 😊";
        let new = "NORTEX Code";
        let mut buffer = EditorBuffer::from_text(old);

        sync_buffer(&mut buffer, old, new);

        assert_eq!(buffer.materialize(), new);
        assert!(buffer.undo());
        assert_eq!(buffer.materialize(), old);
    }
}
