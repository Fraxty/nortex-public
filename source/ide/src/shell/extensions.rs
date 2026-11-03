//! Extensions screen backed by the local approval registry.
#![allow(dead_code)] // Online-Marketplace-Anbindung ist vorhanden, aber nicht in der Oberfläche.

use crate::*;
use nortex_ide::confine::{self, ConfineLimits};
use nortex_ide::editor::document::EditorDocument;
use nortex_ide::i18n::{tr, tr_fmt};
use nortex_ide::plugins::PluginApproval;
use nortex_ide::plugins::archive::ArchiveSummary;
use nortex_ide::plugins::host::{
    ArchiveRequest, DiagnosticInfo, DocumentInfo, IdeServices, PluginState, launch_sandboxed,
};
use nortex_ide::plugins::pdf::PdfSummary;
use nortex_ide::plugins::protocol::PluginManifest;
use nortex_ide::sandbox::{self, SandboxLimits, SandboxRequest};
use sha2::{Digest, Sha256};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ExtensionSection {
    #[default]
    Marketplace,
    Installed,
    Publishing,
    Updates,
    Favorites,
}

const COMPILE_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_SNAPSHOT_FILES: usize = 20_000;
const MAX_SNAPSHOT_BYTES: u64 = 128 * 1024 * 1024;

/// Copies the package into a private directory so the compiler works on the
/// exact bytes that were hashed. Links and oversized packages are refused.
fn snapshot_package(from: &Path, to: &Path, files: &mut usize, bytes: &mut u64) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        if name.to_string_lossy().starts_with(".nortex-") {
            continue;
        }
        let kind = entry.file_type()?;
        let target = to.join(&name);
        if kind.is_dir() {
            snapshot_package(&entry.path(), &target, files, bytes)?;
        } else if kind.is_file() {
            *files += 1;
            *bytes += entry.metadata()?.len();
            if *files > MAX_SNAPSHOT_FILES || *bytes > MAX_SNAPSHOT_BYTES {
                return Err(io::Error::other(
                    "extension package is too large to compile",
                ));
            }
            fs::copy(entry.path(), target)?;
        } else {
            return Err(io::Error::other("extension package contains a link"));
        }
    }
    Ok(())
}

/// Compiles the confirmed `.nx` entry and returns the built executable inside
/// the install directory together with its SHA-256. The compiler is *not*
/// sandboxed; only the program it produces is.
fn compile_confirmed_entry(confirmed: &ExtensionRunRequest) -> Result<(PathBuf, String), String> {
    let current = sandbox::file_sha256(&confirmed.executable).map_err(|error| error.to_string())?;
    if current != confirmed.entry_sha256 {
        return Err("The extension entry point changed after it was confirmed.".into());
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let build_dir =
        std::env::temp_dir().join(format!("nortex-build-{}-{nonce}", std::process::id()));
    let outcome = build_in_snapshot(confirmed, &build_dir, nonce);
    let _ = fs::remove_dir_all(&build_dir);
    outcome
}

fn build_in_snapshot(
    confirmed: &ExtensionRunRequest,
    build_dir: &Path,
    nonce: u128,
) -> Result<(PathBuf, String), String> {
    let (mut files, mut bytes) = (0, 0);
    confine::create_workspace(build_dir)
        .map_err(|error| format!("Could not prepare the compiler workspace: {error}"))?;
    snapshot_package(&confirmed.installed_dir, build_dir, &mut files, &mut bytes)
        .map_err(|error| format!("Could not snapshot the extension for compilation: {error}"))?;
    let entry = build_dir.join(&confirmed.entry);
    if sandbox::file_sha256(&entry).map_err(|error| error.to_string())? != confirmed.entry_sha256 {
        return Err("The extension entry point changed while it was being copied.".into());
    }

    let compiler = nortex_ide::toolchain::compiler_path(
        std::env::current_exe()
            .ok()
            .as_deref()
            .and_then(Path::parent),
    );
    let built = build_dir.join(".nortex-out.exe");
    let mut command = Command::new(&compiler);
    command
        .current_dir(build_dir)
        .env("TEMP", build_dir)
        .env("TMP", build_dir)
        .arg("build")
        .arg(&entry)
        .arg("--output")
        .arg(&built)
        .arg("--lang")
        .arg(nortex_ide::i18n::language().code());
    let limits = ConfineLimits {
        timeout: COMPILE_TIMEOUT,
        ..ConfineLimits::default()
    };
    let outcome = confine::run_confined(&mut command, &limits).map_err(|error| {
        format!(
            "NORTEX compiler '{}' could not be started under resource limits: {error}",
            compiler.display()
        )
    })?;
    if outcome.timed_out {
        return Err(
            "NORTEX compilation timed out; the compiler and its child processes were terminated."
                .into(),
        );
    }
    if outcome.exit_code != Some(0) {
        return Err(format!(
            "NORTEX source compilation failed (exit {:?}).\n{}",
            outcome.exit_code, outcome.output
        ));
    }

    let metadata = fs::symlink_metadata(&built)
        .map_err(|_| "NORTEX compiler succeeded but produced no executable".to_string())?;
    if !metadata.is_file() || metadata.len() < 64 {
        return Err("NORTEX compiler output is not a valid executable".into());
    }
    let mut header = [0u8; 2];
    fs::File::open(&built)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(|error| error.to_string())?;
    if &header != b"MZ" {
        return Err("NORTEX compiler output is not a Windows executable".into());
    }

    // The digest is taken from the private build directory; the copy that is
    // started must match it, so nothing can be swapped in between.
    let digest = sandbox::file_sha256(&built).map_err(|error| error.to_string())?;
    let destination = confirmed
        .installed_dir
        .join(format!(".nortex-run-{}-{nonce}.exe", std::process::id()));
    fs::copy(&built, &destination).map_err(|error| error.to_string())?;
    Ok((destination, digest))
}

/// Builds the sandbox request for a confirmed run. A `.nx` entry is compiled first;
/// the compiled executable is a temporary file whose path is returned for cleanup.
fn prepare_sandbox_request(
    confirmed: ExtensionRunRequest,
) -> Result<(Box<SandboxRequest>, Option<PathBuf>), String> {
    if !cfg!(windows) {
        return Err("Sandboxed extension execution is currently supported only on Windows".into());
    }
    let is_source = confirmed
        .executable
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("nx"));
    let (executable, expected_sha256) = if is_source {
        compile_confirmed_entry(&confirmed)?
    } else {
        (confirmed.executable.clone(), confirmed.entry_sha256.clone())
    };
    let temporary = (executable != confirmed.executable).then(|| executable.clone());

    let request = SandboxRequest {
        executable,
        args: Vec::new(),
        working_dir: confirmed.installed_dir.clone(),
        extension_id: confirmed.id,
        installed_dir: confirmed.installed_dir,
        permissions: confirmed.permissions,
        run_approved: true,
        expected_sha256: Some(expected_sha256),
        limits: SandboxLimits::default(),
    };
    Ok((Box::new(request), temporary))
}

fn run_extension_request(
    confirmed: ExtensionRunRequest,
    cancel: &AtomicBool,
) -> Result<nortex_ide::sandbox::SandboxOutput, String> {
    let (request, temporary) = prepare_sandbox_request(confirmed)?;
    let result =
        sandbox::run_sandboxed_cancellable(&request, cancel).map_err(|error| error.to_string());
    if let Some(path) = temporary
        && let Err(error) = fs::remove_file(&path)
    {
        return match result {
            Ok(_) => Err(format!(
                "Sandbox process completed, but temporary executable cleanup failed: {error}"
            )),
            Err(run_error) => Err(format!(
                "{run_error}; temporary executable cleanup failed: {error}"
            )),
        };
    }
    result
}

fn describe_run_result(
    id: &str,
    result: Result<nortex_ide::sandbox::SandboxOutput, String>,
) -> String {
    match result {
        Ok(output) => format!(
            "Extension {id} process finished.\nExit code: {}.\nTimed out: {}.\nCancelled: {}.\nOutput truncated: {}.\n\nstdout:\n{}\n\nstderr:\n{}",
            output.exit_code,
            if output.timed_out { "yes" } else { "no" },
            if output.cancelled { "yes" } else { "no" },
            if output.output_truncated { "yes" } else { "no" },
            if output.stdout.is_empty() {
                "(empty)"
            } else {
                &output.stdout
            },
            if output.stderr.is_empty() {
                "(empty)"
            } else {
                &output.stderr
            }
        ),
        Err(error) => format!("Extension {id} sandbox/process error:\n{error}"),
    }
}

/// The IDE state a hosted plugin may see. Paths are relative to the project root;
/// absolute paths never leave the IDE.
struct ActiveIde<'a> {
    document: Option<&'a EditorDocument>,
    project_root: &'a Path,
    source_files: &'a [PathBuf],
    dark: bool,
    replacement: Option<String>,
    notices: Vec<String>,
}

const MAX_PLUGIN_FILE_BYTES: u64 = 1024 * 1024;

impl ActiveIde<'_> {
    fn document_name(document: &EditorDocument) -> String {
        document.path().and_then(Path::file_name).map_or_else(
            || "untitled".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
    }

    /// Resolves a project-relative path and rejects anything that leaves the project,
    /// including through symlinks. The file itself need not exist yet.
    fn project_path(&self, relative: &str) -> Result<PathBuf, &'static str> {
        let relative = Path::new(relative);
        if relative.as_os_str().is_empty()
            || !relative
                .components()
                .all(|part| matches!(part, std::path::Component::Normal(_)))
        {
            return Err("path must be relative and stay inside the project");
        }
        let root = std::fs::canonicalize(self.project_root).map_err(|_| "project unavailable")?;
        let target = root.join(relative);
        let parent = target.parent().unwrap_or(&root);
        let parent = std::fs::canonicalize(parent).map_err(|_| "directory does not exist")?;
        if !parent.starts_with(&root) {
            return Err("path must be relative and stay inside the project");
        }
        Ok(parent.join(target.file_name().ok_or("invalid file name")?))
    }
}

impl IdeServices for ActiveIde<'_> {
    fn replace_document(&mut self, text: String) -> Result<(), &'static str> {
        if self.document.is_none() {
            return Err("no active document");
        }
        self.replacement = Some(text);
        Ok(())
    }

    fn read_file(&mut self, path: &str) -> Result<String, &'static str> {
        let path = self.project_path(path)?;
        let file = std::fs::File::open(&path).map_err(|_| "file cannot be opened")?;
        let mut text = String::new();
        file.take(MAX_PLUGIN_FILE_BYTES + 1)
            .read_to_string(&mut text)
            .map_err(|_| "file is not readable UTF-8 text")?;
        if text.len() as u64 > MAX_PLUGIN_FILE_BYTES {
            return Err("file is larger than 1 MiB");
        }
        Ok(text)
    }

    fn write_file(&mut self, path: &str, content: &str) -> Result<(), &'static str> {
        let path = self.project_path(path)?;
        if std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_symlink()) {
            return Err("refusing to write through a symlink");
        }
        std::fs::write(path, content).map_err(|_| "file cannot be written")
    }

    fn notify(&mut self, message: String) {
        self.notices.push(message);
    }

    fn search_text(&mut self, query: &str, limit: usize) -> Vec<(String, usize, String)> {
        let mut matches = Vec::new();
        for path in self.source_files {
            let Ok(relative) = path.strip_prefix(self.project_root) else {
                continue;
            };
            let Ok(meta) = std::fs::metadata(path) else {
                continue;
            };
            if meta.len() > MAX_PLUGIN_FILE_BYTES {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            for (index, line) in text.lines().enumerate() {
                if line.contains(query) {
                    matches.push((
                        relative.to_string_lossy().replace('\\', "/"),
                        index + 1,
                        line.chars().take(200).collect(),
                    ));
                    if matches.len() == limit {
                        return matches;
                    }
                }
            }
        }
        matches
    }

    fn git_status(&mut self) -> Option<(String, Vec<(String, String)>)> {
        match nortex_ide::git::query(self.project_root) {
            nortex_ide::git::GitStatus::Repository { branch, changes } => Some((
                branch,
                changes
                    .into_iter()
                    .map(|change| (change.code, change.path))
                    .collect(),
            )),
            _ => None,
        }
    }

    fn dark_theme(&mut self) -> bool {
        self.dark
    }

    fn create_archive(&mut self, request: &ArchiveRequest) -> Result<ArchiveSummary, &'static str> {
        let extra = if request.include_active_document {
            let document = self.document.ok_or("no active document")?;
            let name = document
                .path()
                .and_then(|path| path.strip_prefix(self.project_root).ok())
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|| Self::document_name(document));
            Some((name, document.buffer().as_str().into_bytes()))
        } else {
            None
        };
        nortex_ide::plugins::archive::create(
            self.project_root,
            &request.output,
            &request.files,
            extra,
        )
    }

    fn export_document_pdf(&mut self, output: &str) -> Result<PdfSummary, &'static str> {
        let document = self.document.ok_or("no active document")?;
        nortex_ide::plugins::pdf::export(self.project_root, output, &document.buffer().as_str())
    }

    fn active_document(&mut self) -> Option<DocumentInfo> {
        let document = self.document?;
        let is_nortex = document
            .path()
            .and_then(Path::extension)
            .is_some_and(|extension| extension.eq_ignore_ascii_case("nx"));
        Some(DocumentInfo {
            name: Self::document_name(document),
            language: if is_nortex { "nortex" } else { "text" }.into(),
            text: document.buffer().as_str(),
        })
    }

    fn project_files(&mut self) -> Vec<String> {
        self.source_files
            .iter()
            .filter_map(|path| path.strip_prefix(self.project_root).ok())
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .collect()
    }

    fn diagnostics(&mut self) -> Vec<DiagnosticInfo> {
        let Some(document) = self.document else {
            return Vec::new();
        };
        let text = document.buffer().as_str();
        nortex_ide::diagnostics::analyze_source(Self::document_name(document), &text)
            .into_iter()
            .map(|diagnostic| DiagnosticInfo {
                code: diagnostic.code,
                severity: if diagnostic.is_error {
                    "error"
                } else {
                    "warning"
                }
                .into(),
                message: diagnostic.message,
                line: diagnostic.line,
                column: diagnostic.column,
            })
            .collect()
    }
}

enum MarketplaceAction {
    Favorite,
    Rate(u8),
}

fn format_bytes(bytes: i64) -> String {
    let bytes = bytes.max(0) as f64;
    if bytes >= 1024.0 * 1024.0 {
        format!("{:.1} MiB", bytes / (1024.0 * 1024.0))
    } else {
        format!("{:.0} KiB", bytes / 1024.0)
    }
}

impl ExtensionSection {
    const ALL: [(Self, &'static str); 5] = [
        (Self::Marketplace, "Marketplace"),
        (Self::Installed, "Installiert"),
        (Self::Publishing, "Veröffentlichungen"),
        (Self::Updates, "Updates"),
        (Self::Favorites, "Favoriten"),
    ];
}

impl NortexCodeApp {
    pub(crate) fn draw_extensions_view(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Erweiterungen"));
        ui.horizontal_wrapped(|ui| {
            for (section, label) in ExtensionSection::ALL {
                ui.selectable_value(&mut self.extensions_section, section, tr(label));
            }
        });
        ui.separator();

        match self.extensions_section {
            ExtensionSection::Marketplace => self.draw_marketplace(ui),
            ExtensionSection::Installed => self.draw_installed_plugins(ui),
            ExtensionSection::Publishing => self.draw_publishing(ui),
            ExtensionSection::Updates => self.draw_updates(ui),
            ExtensionSection::Favorites => self.draw_favorites(ui),
        }
    }

    fn draw_marketplace(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Marketplace"));
        if self.marketplace_busy {
            ui.label(tr("Marketplace-Anfrage läuft …"));
        }
        if let Some(message) = &self.marketplace_status {
            ui.label(message);
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.marketplace_query)
                .desired_width(ui.available_width())
                .hint_text(tr("Erweiterungen suchen")),
        );
        egui::ScrollArea::vertical()
            .id_salt("marketplace-catalog")
            .show(ui, |ui| self.draw_bundled_extensions(ui, false));
    }

    fn draw_marketplace_connection(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(tr("Server"));
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.marketplace_url)
                    .desired_width(ui.available_width()),
            );
            if response.changed() {
                self.marketplace_token = None;
                self.marketplace_user_role = None;
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr("Vertrauenswürdiger Signaturschlüssel"));
            ui.add(
                egui::TextEdit::singleline(&mut self.marketplace_signing_key_hex)
                    .desired_width(ui.available_width())
                    .hint_text(tr("64 Hex-Zeichen, unabhängig vom Server erhalten")),
            );
        });
        ui.horizontal(|ui| {
            ui.label(tr("Konto"));
            ui.add(
                egui::TextEdit::singleline(&mut self.marketplace_username)
                    .desired_width(140.0)
                    .hint_text(tr("Benutzername")),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.marketplace_password)
                    .password(true)
                    .desired_width(140.0)
                    .hint_text(tr("Passwort")),
            );
            let can_auth = !self.marketplace_busy
                && !self.marketplace_username.is_empty()
                && !self.marketplace_password.is_empty();
            if ui
                .add_enabled(can_auth, egui::Button::new(tr("Anmelden")))
                .clicked()
            {
                self.start_marketplace_auth(false);
            }
            if ui
                .add_enabled(can_auth, egui::Button::new(tr("Registrieren")))
                .clicked()
            {
                self.start_marketplace_auth(true);
            }
            if let Some(role) = &self.marketplace_user_role {
                ui.label(tr_fmt("Angemeldet als {}", &[role.clone()]));
                if ui
                    .add_enabled(!self.marketplace_busy, egui::Button::new(tr("Abmelden")))
                    .clicked()
                {
                    self.start_marketplace_logout();
                }
            }
        });
    }

    fn draw_marketplace_results(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("marketplace-results")
            .show(ui, |ui| {
                for extension in self.marketplace_results.clone() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(&extension.name);
                            ui.label(egui::RichText::new(&extension.id).weak());
                            if !extension.latest_version.is_empty() {
                                ui.label(format!("v{}", extension.latest_version));
                            }
                            ui.label(format!(
                                "{:.1} / 5 ({} {})",
                                extension.rating_average,
                                extension.rating_count,
                                tr("Bewertungen")
                            ));
                            if ui
                                .add_enabled(
                                    !self.marketplace_busy
                                        && !self.marketplace_signing_key_hex.trim().is_empty()
                                        && !extension.latest_version.is_empty(),
                                    egui::Button::new(tr("Installieren")),
                                )
                                .clicked()
                            {
                                self.start_marketplace_install(
                                    extension.id.clone(),
                                    extension.latest_version.clone(),
                                );
                            }
                        });
                        ui.label(&extension.description);
                        ui.label(tr(
                            "Fehlende Abhängigkeiten werden signaturgeprüft mit installiert; neu installierte Pakete bleiben deaktiviert und benötigen eine eigene Freigabe.",
                        ));
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    !self.marketplace_busy && self.marketplace_token.is_some(),
                                    egui::Button::new(tr("Favorisieren")),
                                )
                                .clicked()
                            {
                                self.start_marketplace_action(
                                    extension.id.clone(),
                                    MarketplaceAction::Favorite,
                                );
                            }
                            for stars in 1..=5 {
                                if ui
                                    .add_enabled(
                                        !self.marketplace_busy && self.marketplace_token.is_some(),
                                        egui::Button::new(format!("{stars}★")),
                                    )
                                    .clicked()
                                {
                                    self.start_marketplace_action(
                                        extension.id.clone(),
                                        MarketplaceAction::Rate(stars),
                                    );
                                }
                            }
                        });
                    });
                }
            });
    }

    fn draw_publishing(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Veröffentlichungen"));
        ui.label(tr(
            "Publisher-Uploads benötigen ein Marketplace-Konto mit Publisher- oder Admin-Rolle.",
        ));
        if ui
            .add_enabled(
                !self.marketplace_busy && self.marketplace_token.is_some(),
                egui::Button::new(tr("Paket hochladen")),
            )
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("NORTEX Extension package", &["zip"])
                .pick_file()
            {
                self.start_marketplace_upload(path);
            }
        }
        if let Some(message) = &self.marketplace_status {
            ui.label(message);
        }
        if self.marketplace_user_role.as_deref() == Some("admin") {
            ui.separator();
            ui.heading(tr("Admin-Prüfung"));
            if ui
                .add_enabled(
                    !self.marketplace_busy,
                    egui::Button::new(tr("Einreichungen laden")),
                )
                .clicked()
            {
                self.start_marketplace_submissions();
            }
            for submission in self.marketplace_submissions.clone() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.strong(&submission.id);
                        ui.label(format!("v{}", submission.version));
                        ui.label(format!("{}: {}", tr("Publisher"), submission.publisher));
                        ui.label(format!(
                            "{}: {}",
                            tr("Größe"),
                            format_bytes(submission.size)
                        ));
                    });
                    ui.label(format!("SHA-256: {}", submission.sha256));
                    if let Ok(manifest) =
                        serde_json::from_str::<serde_json::Value>(&submission.manifest)
                    {
                        if let Some(nortex) =
                            manifest.get("nortex").and_then(|value| value.as_str())
                        {
                            ui.label(format!("NORTEX-Kompatibilität: {nortex}"));
                        }
                        if let Some(permissions) = manifest
                            .get("permissions")
                            .and_then(|value| value.as_array())
                        {
                            let permissions = permissions
                                .iter()
                                .filter_map(serde_json::Value::as_str)
                                .collect::<Vec<_>>()
                                .join(", ");
                            ui.label(format!("Berechtigungen: {permissions}"));
                        }
                        if let Some(dependencies) = manifest
                            .get("dependencies")
                            .and_then(|value| value.as_object())
                        {
                            for (id, range) in dependencies {
                                if let Some(range) = range.as_str() {
                                    ui.label(format!("Abhängigkeit: {id} {range}"));
                                }
                            }
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(!self.marketplace_busy, egui::Button::new(tr("Freigeben")))
                            .clicked()
                        {
                            self.start_marketplace_review(submission.submission_id, true);
                        }
                        if ui
                            .add_enabled(!self.marketplace_busy, egui::Button::new(tr("Ablehnen")))
                            .clicked()
                        {
                            self.start_marketplace_review(submission.submission_id, false);
                        }
                    });
                });
            }
            ui.separator();
            ui.heading(tr("Publisher-Rolle vergeben"));
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.marketplace_role_username);
                egui::ComboBox::from_id_salt("marketplace-role")
                    .selected_text(&self.marketplace_role)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.marketplace_role, "user".into(), "user");
                        ui.selectable_value(
                            &mut self.marketplace_role,
                            "publisher".into(),
                            "publisher",
                        );
                        ui.selectable_value(&mut self.marketplace_role, "admin".into(), "admin");
                    });
                if ui
                    .add_enabled(
                        !self.marketplace_busy && !self.marketplace_role_username.is_empty(),
                        egui::Button::new(tr("Rolle setzen")),
                    )
                    .clicked()
                {
                    self.start_marketplace_set_role(
                        self.marketplace_role_username.clone(),
                        self.marketplace_role.clone(),
                    );
                }
            });
        }
    }

    fn draw_updates(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Updates"));
        if ui
            .add_enabled(
                !self.marketplace_busy,
                egui::Button::new(tr("Updates suchen")),
            )
            .clicked()
        {
            self.start_marketplace_update_check();
        }
        if let Some(message) = &self.marketplace_status {
            ui.label(message);
        }
        egui::ScrollArea::vertical()
            .id_salt("marketplace-updates")
            .show(ui, |ui| {
                for update in self.marketplace_updates.clone() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(&update.id);
                            ui.label(format!(
                                "{} → {}",
                                update.installed_version, update.latest_version
                            ));
                            if ui
                                .add_enabled(
                                    !self.marketplace_busy
                                        && !self.marketplace_signing_key_hex.trim().is_empty(),
                                    egui::Button::new(tr("Aktualisieren")),
                                )
                                .clicked()
                            {
                                self.start_marketplace_install(
                                    update.id.clone(),
                                    update.latest_version.clone(),
                                );
                            }
                        });
                    });
                }
            });
    }

    fn draw_favorites(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Favoriten"));
        if self.marketplace_token.is_none() {
            ui.label(tr("Melde dich am Marketplace an, um Favoriten zu laden."));
            return;
        }
        if ui
            .add_enabled(
                !self.marketplace_busy,
                egui::Button::new(tr("Aktualisieren")),
            )
            .clicked()
        {
            self.start_marketplace_favorites();
        }
        if let Some(message) = &self.marketplace_status {
            ui.label(message);
        }
        egui::ScrollArea::vertical()
            .id_salt("marketplace-favorites")
            .show(ui, |ui| {
                for extension in &self.marketplace_favorites {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(&extension.name);
                            ui.label(egui::RichText::new(&extension.id).weak());
                            if !extension.latest_version.is_empty() {
                                ui.label(format!("v{}", extension.latest_version));
                            }
                        });
                        ui.label(&extension.description);
                    });
                }
            });
    }

    fn start_marketplace_search(&mut self) {
        let url = self.marketplace_url.clone();
        let query = self.marketplace_query.clone();
        self.spawn_marketplace_job(move || {
            let result = MarketplaceClient::new(&url).and_then(|client| client.search(&query));
            MarketplaceEvent::Search(result)
        });
    }

    fn start_marketplace_auth(&mut self, register: bool) {
        let url = self.marketplace_url.clone();
        let username = self.marketplace_username.clone();
        let password = self.marketplace_password.clone();
        self.spawn_marketplace_job(move || {
            let result = MarketplaceClient::new(&url).and_then(|client| {
                if register {
                    client.register(&username, &password)?;
                }
                client.login(&username, &password)
            });
            MarketplaceEvent::Auth(result)
        });
    }

    fn start_marketplace_favorites(&mut self) {
        let url = self.marketplace_url.clone();
        let token = self.marketplace_token.clone().unwrap_or_default();
        self.spawn_marketplace_job(move || {
            let result = MarketplaceClient::new(&url).and_then(|client| client.favorites(&token));
            MarketplaceEvent::Favorites(result)
        });
    }

    fn start_marketplace_logout(&mut self) {
        let url = self.marketplace_url.clone();
        let token = self.marketplace_token.clone().unwrap_or_default();
        self.spawn_marketplace_job(move || {
            let result = MarketplaceClient::new(&url).and_then(|client| client.logout(&token));
            MarketplaceEvent::Logout(result)
        });
    }

    fn start_marketplace_submissions(&mut self) {
        let url = self.marketplace_url.clone();
        let token = self.marketplace_token.clone().unwrap_or_default();
        self.spawn_marketplace_job(move || {
            let result = MarketplaceClient::new(&url).and_then(|client| client.submissions(&token));
            MarketplaceEvent::Submissions(result)
        });
    }

    fn start_marketplace_review(&mut self, submission_id: i64, approve: bool) {
        let url = self.marketplace_url.clone();
        let token = self.marketplace_token.clone().unwrap_or_default();
        self.spawn_marketplace_job(move || {
            let result = MarketplaceClient::new(&url)
                .and_then(|client| client.review_submission(&token, submission_id, approve));
            MarketplaceEvent::Action(result.map(|()| {
                format!(
                    "{} {submission_id}",
                    tr(if approve {
                        "Einreichung freigegeben:"
                    } else {
                        "Einreichung abgelehnt:"
                    })
                )
            }))
        });
    }

    fn start_marketplace_set_role(&mut self, username: String, role: String) {
        let url = self.marketplace_url.clone();
        let token = self.marketplace_token.clone().unwrap_or_default();
        self.spawn_marketplace_job(move || {
            let result = MarketplaceClient::new(&url)
                .and_then(|client| client.set_role(&token, &username, &role));
            MarketplaceEvent::Action(
                result.map(|()| format!("{}: {username} = {role}", tr("Rolle aktualisiert"))),
            )
        });
    }

    fn start_marketplace_update_check(&mut self) {
        let url = self.marketplace_url.clone();
        let install_root = self
            .extensions_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("extensions")
            .join("installed");
        self.spawn_marketplace_job(move || {
            let result = marketplace::installed_versions(&install_root)
                .and_then(|installed| MarketplaceClient::new(&url)?.check_updates(installed));
            MarketplaceEvent::Updates(result)
        });
    }

    fn start_marketplace_action(&mut self, id: String, action: MarketplaceAction) {
        let url = self.marketplace_url.clone();
        let token = self.marketplace_token.clone().unwrap_or_default();
        self.spawn_marketplace_job(move || {
            let result = MarketplaceClient::new(&url).and_then(|client| match action {
                MarketplaceAction::Favorite => client
                    .favorite(&token, &id)
                    .map(|()| format!("{} {}", tr("Favorisiert:"), id)),
                MarketplaceAction::Rate(stars) => client
                    .rate(&token, &id, stars)
                    .map(|()| format!("{}: {stars}", tr("Bewertung gespeichert"))),
            });
            MarketplaceEvent::Action(result)
        });
    }

    fn start_marketplace_upload(&mut self, path: std::path::PathBuf) {
        let url = self.marketplace_url.clone();
        let token = self.marketplace_token.clone().unwrap_or_default();
        self.spawn_marketplace_job(move || {
            let result = MarketplaceClient::new(&url).and_then(|client| {
                let file = std::fs::File::open(path).map_err(|error| error.to_string())?;
                let mut package = Vec::new();
                file.take(10 * 1024 * 1024 + 1)
                    .read_to_end(&mut package)
                    .map_err(|error| error.to_string())?;
                if package.len() > 10 * 1024 * 1024 {
                    return Err("Package exceeds the 10 MiB client upload limit".into());
                }
                client.upload(&token, package)
            });
            MarketplaceEvent::Upload(result)
        });
    }

    fn start_marketplace_install(&mut self, id: String, version: String) {
        let url = self.marketplace_url.clone();
        let trusted_key = self.marketplace_signing_key_hex.clone();
        let install_root = self
            .extensions_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("extensions")
            .join("installed");
        self.spawn_marketplace_job(move || {
            let result =
                marketplace::download_and_install(&url, &id, &version, &trusted_key, &install_root);
            MarketplaceEvent::Installed(result)
        });
    }

    fn spawn_marketplace_job(&mut self, job: impl FnOnce() -> MarketplaceEvent + Send + 'static) {
        self.marketplace_busy = true;
        let sender = self.marketplace_sender.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("nortex-marketplace".into())
            .spawn(move || {
                let _ = sender.send(job());
            })
        {
            self.marketplace_busy = false;
            self.marketplace_status = Some(format!(
                "{}: {error}",
                tr("Marketplace-Anfrage konnte nicht gestartet werden")
            ));
        }
    }

    pub(crate) fn poll_marketplace_events(&mut self) {
        while let Ok(event) = self.extension_run_results_rx.try_recv() {
            match event {
                ExtensionRunEvent::Finished { id, result } => {
                    self.extension_runs.remove(&id);
                    let message = describe_run_result(&id, result);
                    self.extension_run_status.insert(id, message);
                }
                ExtensionRunEvent::HostedReady { manifest, result } => {
                    self.extension_runs.remove(&manifest.id);
                    self.start_hosted_plugin(manifest, result);
                }
            }
        }

        while let Ok(event) = self.marketplace_results_rx.try_recv() {
            self.marketplace_busy = false;
            match event {
                MarketplaceEvent::Search(Ok(results)) => {
                    self.marketplace_status =
                        Some(format!("{} {}", results.len(), tr("Ergebnisse")));
                    self.marketplace_results = results;
                }
                MarketplaceEvent::Favorites(Ok(results)) => {
                    self.marketplace_status =
                        Some(format!("{} {}", results.len(), tr("Ergebnisse")));
                    self.marketplace_favorites = results;
                }
                MarketplaceEvent::Updates(Ok(updates)) => {
                    self.marketplace_status =
                        Some(format!("{} {}", updates.len(), tr("Updates verfügbar")));
                    self.marketplace_updates = updates;
                }
                MarketplaceEvent::Submissions(Ok(submissions)) => {
                    self.marketplace_status = Some(format!(
                        "{} {}",
                        submissions.len(),
                        tr("offene Einreichungen")
                    ));
                    self.marketplace_submissions = submissions;
                }
                MarketplaceEvent::Auth(Ok((token, role))) => {
                    self.marketplace_token = Some(token);
                    self.marketplace_user_role = Some(role);
                    self.marketplace_password.clear();
                    self.marketplace_status = Some(tr("Marketplace-Anmeldung erfolgreich").into());
                }
                MarketplaceEvent::Logout(Ok(())) => {
                    self.marketplace_token = None;
                    self.marketplace_user_role = None;
                    self.marketplace_status = Some(tr("Marketplace-Abmeldung erfolgreich").into());
                }
                MarketplaceEvent::Action(Ok(message)) => {
                    self.marketplace_status = Some(message);
                }
                MarketplaceEvent::Upload(Ok(id)) => {
                    self.marketplace_status = Some(format!("{} {id}", tr("Upload eingereicht:")));
                }
                MarketplaceEvent::Installed(Ok(installed)) => {
                    let mut installed_ids = Vec::new();
                    for (id, name) in installed {
                        let needs_review = self.documents.plugins.get(&id).is_some();
                        self.documents.plugins.register(&id, name);
                        if needs_review {
                            self.documents.plugins.require_review(&id);
                        }
                        installed_ids.push(id);
                    }
                    self.persist_plugin_registry();
                    self.marketplace_status = Some(format!(
                        "{} {}",
                        tr("Installiert; Freigabe erforderlich:"),
                        installed_ids.join(", ")
                    ));
                }
                MarketplaceEvent::Search(Err(error))
                | MarketplaceEvent::Updates(Err(error))
                | MarketplaceEvent::Submissions(Err(error))
                | MarketplaceEvent::Favorites(Err(error))
                | MarketplaceEvent::Auth(Err(error))
                | MarketplaceEvent::Logout(Err(error))
                | MarketplaceEvent::Action(Err(error))
                | MarketplaceEvent::Upload(Err(error))
                | MarketplaceEvent::Installed(Err(error)) => {
                    self.marketplace_status = Some(error);
                }
            }
        }
    }

    fn draw_bundled_extensions(&mut self, ui: &mut egui::Ui, bundled_only: bool) {
        if !self
            .bundled_extensions
            .iter()
            .any(|e| e.bundled == bundled_only)
        {
            return;
        }
        if bundled_only {
            ui.strong(tr("Mitgelieferte Extensions"));
        }
        let root = self.extension_install_root();
        let filter = self.marketplace_query.trim().to_lowercase();
        let mut action: Option<(usize, bool)> = None;
        for (index, bundled) in self.bundled_extensions.iter().enumerate() {
            let matches = bundled_only
                || filter.is_empty()
                || bundled.name.to_lowercase().contains(&filter)
                || bundled.description.to_lowercase().contains(&filter);
            if bundled.bundled != bundled_only || !matches {
                continue;
            }
            let installed = root
                .join(&bundled.id)
                .join("nortex-extension.json")
                .is_file()
                && self.documents.plugins.get(&bundled.id).is_some();
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.strong(&bundled.name);
                    ui.label(egui::RichText::new(&bundled.id).weak());
                    ui.label(if installed {
                        tr("Installiert (Bundle)")
                    } else {
                        tr("Nicht installiert")
                    });
                });
                ui.label(&bundled.description);
                ui.horizontal(|ui| {
                    let label = match (installed, bundled_only) {
                        (true, _) => tr("Reparieren"),
                        (false, true) => tr("Aus Bundle installieren"),
                        (false, false) => tr("Installieren"),
                    };
                    if ui.button(label).clicked() {
                        action = Some((index, true));
                    }
                    if installed && ui.button(tr("Entfernen")).clicked() {
                        action = Some((index, false));
                    }
                });
            });
        }
        if let Some((index, install)) = action {
            let bundled = self.bundled_extensions[index].clone();
            self.plugin_host.stop(&bundled.id);
            let outcome = if install {
                nortex_ide::plugins::bundle::install(&bundled, &root)
            } else {
                nortex_ide::plugins::bundle::remove(&bundled.id, &root)
            };
            self.status = match outcome {
                Ok(()) if install => {
                    self.documents.plugins.register(&bundled.id, &bundled.name);
                    self.documents.plugins.approve(&bundled.id);
                    self.persist_plugin_registry();
                    nortex_ide::i18n::tr_fmt("Extension installiert: {}", &[bundled.name])
                }
                Ok(()) => {
                    self.documents.plugins.remove(&bundled.id);
                    self.persist_plugin_registry();
                    nortex_ide::i18n::tr_fmt("Extension entfernt: {}", &[bundled.name])
                }
                Err(error) => nortex_ide::i18n::tr_fmt("Bundle-Fehler: {}", &[error.to_string()]),
            };
        }
        ui.separator();
    }

    fn draw_installed_plugins(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr("Installiert"));
        self.draw_bundled_extensions(ui, true);
        ui.label(tr(
            "Manifestdeklarierte .nx-Dateien werden nach Bestätigung mit dem installierten NORTEX-Compiler gebaut; .exe-Einstiegspunkte starten direkt. Beide laufen in einer Windows-AppContainer-Sandbox. Dies ist kein allgemeiner Extension-API-Host.",
        ));
        let limit = usize::from(self.settings.max_parallel_extension_runs);
        if !self.extension_runs.is_empty() {
            ui.label(format!(
                "{} {}/{}",
                tr("Laufende Sandbox-Prozesse:"),
                self.extension_runs.len(),
                limit
            ));
        }
        let mut running_ids: Vec<String> = self.extension_runs.keys().cloned().collect();
        running_ids.sort();
        for id in running_ids {
            ui.horizontal(|ui| {
                ui.monospace(&id);
                if ui.button(tr("Abbrechen")).clicked()
                    && let Some(cancel) = self.extension_runs.get(&id)
                {
                    cancel.store(true, Ordering::Relaxed);
                }
            });
        }
        let mut dismissed = None;
        for (id, status) in &self.extension_run_status {
            ui.group(|ui| {
                ui.label(egui::RichText::new(status).monospace());
                if ui.small_button(tr("Schließen")).clicked() {
                    dismissed = Some(id.clone());
                }
            });
        }
        if let Some(id) = dismissed {
            self.extension_run_status.remove(&id);
        }

        let mut hosted_ids: Vec<String> = self.plugin_host.ids().map(str::to_owned).collect();
        hosted_ids.sort();
        for id in hosted_ids {
            ui.group(|ui| {
                let state = self.plugin_host.state(&id).cloned();
                ui.horizontal(|ui| {
                    ui.strong(&id);
                    ui.label(match &state {
                        Some(PluginState::Starting) => "Startet".to_owned(),
                        Some(PluginState::Active) => "Aktiv".to_owned(),
                        Some(PluginState::Stopping) => "Wird beendet".to_owned(),
                        Some(PluginState::Stopped) => "Beendet".to_owned(),
                        Some(PluginState::Failed(reason)) => format!("Fehler: {reason}"),
                        None => String::new(),
                    });
                    if state.as_ref().is_some_and(|state| !state.is_final())
                        && ui.button(tr("Beenden")).clicked()
                    {
                        self.plugin_host.stop(&id);
                    }
                });
                let status = self.plugin_host.status_text(&id);
                if !status.is_empty() {
                    ui.label(status);
                }
                for entry in self
                    .plugin_host
                    .logs(&id)
                    .collect::<Vec<_>>()
                    .iter()
                    .rev()
                    .take(5)
                    .rev()
                {
                    ui.label(
                        egui::RichText::new(format!("[{}] {}", entry.level, entry.message))
                            .monospace(),
                    );
                }
            });
        }

        let plugins: Vec<_> = self
            .documents
            .plugins
            .records()
            .map(|(id, plugin)| (id.to_owned(), plugin.clone()))
            .collect();
        if plugins.is_empty() {
            ui.label(tr("Es sind keine Plugins registriert."));
            return;
        }

        egui::ScrollArea::vertical()
            .id_salt("installed-extensions")
            .show(ui, |ui| {
                for (id, plugin) in plugins {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.strong(&plugin.name);
                            ui.label(egui::RichText::new(&id).weak());
                        });
                        ui.horizontal_wrapped(|ui| {
                            match plugin.approval {
                                PluginApproval::Pending => {
                                    ui.label(tr("Freigabe ausstehend"));
                                    if ui.button(tr("Freigeben")).clicked() {
                                        self.update_plugin_approval(&id, true);
                                    }
                                    if ui.button(tr("Ablehnen")).clicked() {
                                        self.update_plugin_approval(&id, false);
                                    }
                                }
                                PluginApproval::Approved => {
                                    ui.label(tr("Freigegeben"));
                                    let label = if plugin.enabled {
                                        tr("Deaktivieren")
                                    } else {
                                        tr("Aktivieren")
                                    };
                                    if ui.button(label).clicked() {
                                        self.update_plugin_enabled(&id, !plugin.enabled);
                                    }
                                }
                                PluginApproval::Rejected => {
                                    ui.label(tr("Abgelehnt"));
                                }
                            }
                            let mut visible = plugin.sidebar_visible;
                            if ui
                                .checkbox(&mut visible, tr("Seitenleisteneintrag"))
                                .changed()
                            {
                                self.update_plugin_sidebar(&id, visible);
                            }
                        });
                        let can_run = plugin.approval == PluginApproval::Approved
                            && plugin.enabled
                            && !self.extension_runs.contains_key(&id)
                            && self.extension_runs.len()
                                < usize::from(self.settings.max_parallel_extension_runs)
                            && self.pending_extension_run.is_none();
                        if ui
                            .add_enabled(
                                can_run,
                                egui::Button::new(tr("Sandbox-Prozess ausführen")),
                            )
                            .clicked()
                        {
                            match self.load_extension_run_request(&id, &plugin.name) {
                                Ok(request) => {
                                    self.extension_run_status.remove(&id);
                                    self.pending_extension_run = Some(request);
                                }
                                Err(error) => {
                                    self.extension_run_status.insert(
                                        id.clone(),
                                        format!(
                                            "Extension {id} kann nicht gestartet werden:\n{error}"
                                        ),
                                    );
                                }
                            }
                        }
                    });
                }
            });

        self.draw_extension_run_confirmation(ui);
    }

    fn draw_extension_run_confirmation(&mut self, ui: &egui::Ui) {
        let Some(request) = self.pending_extension_run.clone() else {
            return;
        };
        let mut action = None;
        egui::Window::new(tr("Extension-Prozess bestätigen"))
            .id(egui::Id::new("extension-run-confirmation"))
            .collapsible(false)
            .resizable(true)
            .show(ui.ctx(), |ui| {
                ui.strong(format!("{} ({})", request.name, request.id));
                ui.label(format!("Einstieg: {}", request.entry));
                if request
                    .executable
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("nx"))
                {
                    ui.label(tr(
                        "Die NORTEX-Quelldatei wird nach der Bestätigung mit dem lokalen Compiler gebaut.",
                    ));
                }
                ui.label(format!(
                    "Arbeitsverzeichnis: {}",
                    request.installed_dir.display()
                ));
                ui.label(tr(
                    "Berechtigungen für diesen einzelnen Lauf (eine erneute Bestätigung ist bei jedem Lauf erforderlich):",
                ));
                if request.permissions.is_empty() {
                    ui.label(tr("Keine Berechtigungen angefordert."));
                } else {
                    for permission in &request.permissions {
                        ui.label(format!("• {permission}"));
                    }
                }
                if let Some(hosted) = &request.hosted {
                    ui.label(format!(
                        "Gehosteter Plugin-Prozess mit IDE-API {} (IDE bietet {}).",
                        hosted.ide_api,
                        nortex_ide::plugins::protocol::IDE_API_VERSION
                    ));
                    ui.label(tr("IDE-Berechtigungen (Lesezugriff auf IDE-Daten):"));
                    if hosted.ide_permissions.is_empty() {
                        ui.label(tr("Keine IDE-Berechtigungen angefordert."));
                    }
                    for permission in &hosted.ide_permissions {
                        ui.label(format!("• {permission}"));
                    }
                }
                ui.colored_label(
                    self.theme.palette().amber,
                    tr("Nur fortfahren, wenn du diesem Prozess und allen angezeigten Berechtigungen vertraust."),
                );
                ui.horizontal(|ui| {
                    if ui.button(tr("Abbrechen")).clicked() {
                        action = Some(false);
                    }
                    if ui
                        .button(tr("Für diesen Lauf ausdrücklich genehmigen und ausführen"))
                        .clicked()
                    {
                        action = Some(true);
                    }
                });
            });

        match action {
            Some(false) => self.pending_extension_run = None,
            Some(true) => {
                self.pending_extension_run = None;
                self.start_extension_run(request, ui.ctx());
            }
            None => {}
        }
    }

    fn load_extension_run_request(
        &self,
        id: &str,
        name: &str,
    ) -> Result<ExtensionRunRequest, String> {
        if !valid_installed_extension_id(id) {
            return Err("Registry ID is not a safe installed folder name".into());
        }
        let install_root = self.extension_install_root();
        let canonical_root = std::fs::canonicalize(&install_root)
            .map_err(|error| format!("Installed extension directory is unavailable: {error}"))?;
        let installed_dir_path = install_root.join(id);
        let dir_metadata = std::fs::symlink_metadata(&installed_dir_path)
            .map_err(|error| format!("Installed extension directory: {error}"))?;
        if dir_metadata.file_type().is_symlink() || !dir_metadata.is_dir() {
            return Err("Installed extension path is not a real directory".into());
        }
        let installed_dir = std::fs::canonicalize(&installed_dir_path)
            .map_err(|error| format!("Installed extension directory: {error}"))?;
        if installed_dir.parent() != Some(canonical_root.as_path())
            || installed_dir.file_name().and_then(|part| part.to_str()) != Some(id)
        {
            return Err(
                "Installed extension directory does not exactly match its registry folder".into(),
            );
        }

        let manifest_path = installed_dir.join("nortex-extension.json");
        let manifest_metadata = std::fs::symlink_metadata(&manifest_path)
            .map_err(|error| format!("nortex-extension.json: {error}"))?;
        if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
            return Err("nortex-extension.json is not a regular file".into());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(&manifest_path)
            .map_err(|error| error.to_string())?
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() > 64 * 1024 {
            return Err("nortex-extension.json exceeds 64 KiB".into());
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|error| format!("Invalid manifest: {error}"))?;
        let manifest_id = manifest
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| "Manifest ID is missing or is not a string".to_string())?;
        if manifest_id != id {
            return Err(format!(
                "Manifest ID {manifest_id:?} does not match installed folder/registry ID {id:?}"
            ));
        }

        let entry = manifest
            .get("entry")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                "Manifest entry must name a .nx source file or executable .exe file".to_string()
            })?;
        if !safe_plugin_entry(entry) {
            return Err("Manifest entry must be a safe relative .nx or .exe path".into());
        }
        let entry_path = installed_dir.join(entry);
        let executable = fs::canonicalize(&entry_path)
            .map_err(|error| format!("Manifest executable entry: {error}"))?;
        if !executable.starts_with(&installed_dir)
            || !executable.is_file()
            || !executable.extension().is_some_and(|extension| {
                extension.eq_ignore_ascii_case("exe") || extension.eq_ignore_ascii_case("nx")
            })
        {
            return Err(
                "Manifest entry must be a regular .nx or .exe file inside the installed directory"
                    .into(),
            );
        }
        let mut source_file = fs::File::open(&executable).map_err(|error| error.to_string())?;
        let mut digest = Sha256::new();
        let mut chunk = [0; 16 * 1024];
        loop {
            let read = source_file
                .read(&mut chunk)
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            digest.update(&chunk[..read]);
        }

        let permissions = match manifest.get("permissions") {
            None => Vec::new(),
            Some(value) => value
                .as_array()
                .ok_or_else(|| "Manifest permissions must be an array of strings".to_string())?
                .iter()
                .map(|permission| {
                    permission.as_str().map(str::to_owned).ok_or_else(|| {
                        "Manifest permissions must be an array of strings".to_string()
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        };
        if permissions.len() > 16 {
            return Err("Manifest requests too many permissions".into());
        }
        sandbox::parse_permissions(&permissions).map_err(|error| error.to_string())?;
        let hosted = nortex_ide::plugins::protocol::PluginManifest::from_json(&bytes)?;

        Ok(ExtensionRunRequest {
            id: id.to_owned(),
            name: name.to_owned(),
            installed_dir,
            executable,
            entry: entry.to_owned(),
            entry_sha256: hex::encode(digest.finalize()),
            permissions,
            hosted,
        })
    }

    fn extension_install_root(&self) -> PathBuf {
        self.extensions_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("extensions")
            .join("installed")
    }

    fn start_extension_run(&mut self, confirmed: ExtensionRunRequest, context: &egui::Context) {
        let id = confirmed.id.clone();
        let Some(plugin) = self.documents.plugins.get(&id) else {
            self.set_run_status(&id, "Extension is no longer registered.");
            return;
        };
        if plugin.approval != PluginApproval::Approved || !self.documents.plugins.is_enabled(&id) {
            self.set_run_status(
                &id,
                "Extension must remain locally approved and enabled to run.",
            );
            return;
        }
        let current = match self.load_extension_run_request(&id, &plugin.name) {
            Ok(request) => request,
            Err(error) => {
                self.set_run_status(
                    &id,
                    format!("Extension changed or is no longer runnable:\n{error}"),
                );
                return;
            }
        };
        if current != confirmed {
            self.set_run_status(
                &id,
                "Installed manifest or executable changed after the confirmation was shown. Review and confirm again.",
            );
            return;
        }
        if self.extension_runs.contains_key(&id) {
            self.set_run_status(&id, "This extension is already running.");
            return;
        }
        let limit = usize::from(self.settings.max_parallel_extension_runs);
        if self.extension_runs.len() >= limit {
            self.set_run_status(
                &id,
                format!("Limit of {limit} concurrent extension processes reached."),
            );
            return;
        }

        let sender = self.extension_run_sender.clone();
        let context = context.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        self.extension_runs.insert(id.clone(), cancel);
        self.set_run_status(&id, format!("Extension {id}: preparing sandbox process…"));
        let worker_id = id.clone();
        match std::thread::Builder::new()
            .name(format!("nortex-extension-{id}"))
            .spawn(move || {
                let event = match confirmed.hosted.clone() {
                    Some(manifest) => ExtensionRunEvent::HostedReady {
                        manifest,
                        result: prepare_sandbox_request(confirmed),
                    },
                    None => ExtensionRunEvent::Finished {
                        id: worker_id,
                        result: run_extension_request(confirmed, &worker_cancel),
                    },
                };
                let _ = sender.send(event);
                context.request_repaint();
            }) {
            Ok(_) => {}
            Err(error) => {
                self.extension_runs.remove(&id);
                self.set_run_status(
                    &id,
                    format!("Extension process could not be started: {error}"),
                );
            }
        }
    }

    fn set_run_status(&mut self, id: &str, message: impl Into<String>) {
        self.extension_run_status
            .insert(id.to_owned(), message.into());
    }

    fn start_hosted_plugin(
        &mut self,
        manifest: PluginManifest,
        prepared: Result<(Box<SandboxRequest>, Option<PathBuf>), String>,
    ) {
        let id = manifest.id.clone();
        let (request, temporary) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                self.set_run_status(&id, format!("Plugin {id} could not be prepared:\n{error}"));
                return;
            }
        };
        let allowed = self
            .documents
            .plugins
            .get(&id)
            .is_some_and(|plugin| plugin.approval == PluginApproval::Approved)
            && self.documents.plugins.is_enabled(&id);
        let outcome = if allowed {
            let granted = manifest.ide_permissions.clone();
            self.plugin_host
                .start(&manifest, &granted, || launch_sandboxed(&request))
                .map_err(|error| error.to_string())
        } else {
            Err("Extension must remain locally approved and enabled to run.".to_owned())
        };
        match outcome {
            Ok(()) => {
                if let Some(path) = temporary {
                    self.hosted_executables.insert(id.clone(), path);
                }
                self.set_run_status(&id, format!("Plugin {id} started in the sandbox."));
            }
            Err(error) => {
                if let Some(path) = temporary {
                    let _ = fs::remove_file(path);
                }
                self.set_run_status(&id, format!("Plugin {id} was not started:\n{error}"));
            }
        }
    }

    /// Serves queued plugin requests and removes compiled executables of plugins
    /// that have ended.
    pub(crate) fn pump_plugin_host(&mut self, context: &egui::Context) {
        self.report_plugin_actions();
        if !self.plugin_host.has_live_plugins() && self.hosted_executables.is_empty() {
            return;
        }
        let mut services = ActiveIde {
            document: self.documents.active_document(),
            project_root: &self.project_root,
            source_files: &self.source_files,
            dark: context.style().visuals.dark_mode,
            replacement: None,
            notices: Vec::new(),
        };
        self.plugin_host.pump(&mut services);
        let ActiveIde {
            replacement,
            notices,
            ..
        } = services;
        if let Some(text) = replacement {
            self.set_editor_text(text);
        }
        if let Some(notice) = notices.into_iter().last() {
            self.status = notice;
        }
        self.report_plugin_actions();
        let host = &self.plugin_host;
        self.hosted_executables.retain(|id, path| {
            let ended = host.state(id).is_none_or(PluginState::is_final);
            !(ended && (fs::remove_file(&*path).is_ok() || !path.exists()))
        });
        context.request_repaint_after(Duration::from_millis(50));
    }

    /// Menu entry: runs one declared action of an active, approved and enabled plugin.
    pub(crate) fn invoke_plugin_action(&mut self, plugin: &str, action: &str) {
        if !self.documents.plugins.is_enabled(plugin) {
            self.status = format!("Plugin {plugin} is not approved and enabled.");
            return;
        }
        if let Err(error) = self.plugin_host.invoke_action(plugin, action) {
            self.status = format!("Plugin {plugin}: {error}.");
        }
    }

    fn report_plugin_actions(&mut self) {
        for event in self.plugin_host.take_action_events() {
            self.status = match event.outcome {
                Ok(Some(message)) => format!("{}: {message}", event.plugin),
                Ok(None) => format!("{}: {} done.", event.plugin, event.action),
                Err(error) => format!("{}: {} failed: {error}", event.plugin, event.action),
            };
        }
    }

    fn update_plugin_approval(&mut self, id: &str, approve: bool) {
        if approve {
            self.documents.plugins.approve(id);
        } else {
            self.documents.plugins.reject(id);
            self.plugin_host.stop(id);
        }
        self.persist_plugin_registry();
    }

    fn update_plugin_enabled(&mut self, id: &str, enabled: bool) {
        if self.documents.plugins.set_enabled(id, enabled) {
            if !enabled {
                self.plugin_host.stop(id);
            }
            self.persist_plugin_registry();
        }
    }

    fn update_plugin_sidebar(&mut self, id: &str, visible: bool) {
        if self.documents.plugins.set_sidebar_visible(id, visible) {
            self.persist_plugin_registry();
        }
    }

    fn persist_plugin_registry(&mut self) {
        self.status = match self.documents.plugins.save(&self.extensions_path) {
            Ok(()) => tr("Erweiterungsstatus gespeichert.").into(),
            Err(error) => nortex_ide::i18n::tr_fmt(
                "Erweiterungsstatus konnte nicht gespeichert werden: {}",
                &[error.to_string()],
            ),
        };
    }
}

fn valid_installed_extension_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 48
        && !id.contains("..")
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
}

fn safe_plugin_entry(entry: &str) -> bool {
    if entry.is_empty()
        || entry.len() > 255
        || entry.starts_with('/')
        || entry.starts_with('\\')
        || entry.contains('\\')
        || entry.contains(':')
        || entry.chars().any(char::is_control)
        || !entry.rsplit('/').all(safe_entry_segment)
        || !entry.rsplit('/').next().is_some_and(|filename| {
            let filename = filename.to_ascii_lowercase();
            filename.ends_with(".exe") || filename.ends_with(".nx")
        })
    {
        return false;
    }
    Path::new(entry)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
}

fn safe_entry_segment(segment: &str) -> bool {
    if segment.is_empty()
        || segment == "."
        || segment == ".."
        || segment.ends_with('.')
        || segment.ends_with(' ')
    {
        return false;
    }
    let basename = segment
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    !matches!(
        basename.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_entry_requires_safe_relative_source_or_executable_path() {
        assert!(safe_plugin_entry("bin/plugin.exe"));
        assert!(safe_plugin_entry("PLUGIN.EXE"));
        assert!(safe_plugin_entry("src/main.nx"));
        assert!(!safe_plugin_entry("../plugin.exe"));
        assert!(!safe_plugin_entry("bin/../plugin.exe"));
        assert!(!safe_plugin_entry("/plugin.exe"));
        assert!(!safe_plugin_entry(r"bin\plugin.exe"));
        assert!(!safe_plugin_entry("CON.exe"));
        assert!(!safe_plugin_entry("plugin.dll"));
    }

    #[test]
    fn installed_extension_ids_cannot_escape_the_install_root() {
        assert!(valid_installed_extension_id("formatter.extra"));
        assert!(!valid_installed_extension_id("../formatter"));
        assert!(!valid_installed_extension_id("Formatter"));
        assert!(!valid_installed_extension_id("formatter..extra"));
    }

    fn source_extension(name: &str, source: &str) -> (PathBuf, ExtensionRunRequest) {
        let dir = std::env::temp_dir().join(format!("nortex-ext-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let entry = dir.join("main.nx");
        fs::write(&entry, source).unwrap();
        let request = ExtensionRunRequest {
            id: name.to_owned(),
            name: name.to_owned(),
            installed_dir: fs::canonicalize(&dir).unwrap(),
            executable: fs::canonicalize(&entry).unwrap(),
            entry: "main.nx".into(),
            entry_sha256: sandbox::file_sha256(&entry).unwrap(),
            permissions: Vec::new(),
            hosted: None,
        };
        (dir, request)
    }

    #[test]
    fn changed_entry_point_is_rejected_before_compilation() {
        let (dir, request) = source_extension("tamper", "public int main() { return 0; }");
        fs::write(dir.join("main.nx"), "public int main() { return 1; }").unwrap();
        let result = compile_confirmed_entry(&request);
        let _ = fs::remove_dir_all(&dir);
        assert!(result.unwrap_err().contains("changed after"));
    }

    #[test]
    fn failing_source_leaves_no_build_artifacts() {
        let (dir, request) = source_extension("broken", "this is not NORTEX");
        let result = compile_confirmed_entry(&request);
        let leftovers = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with(".nortex-"));
        let _ = fs::remove_dir_all(&dir);
        assert!(result.is_err());
        assert!(!leftovers);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "needs nortexc built next to the test binary and an AppContainer-capable host"]
    fn confirmed_source_entry_is_compiled_then_runs_in_the_sandbox() {
        let (dir, request) = source_extension("e2e", "public int main() { return 7; }");
        let output = run_extension_request(request, &AtomicBool::new(false));
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(output.unwrap().exit_code, 7);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "needs nortexc built next to the test binary and an AppContainer-capable host"]
    fn independent_runs_execute_in_parallel_with_separate_results() {
        let runs: Vec<_> = [("par-a", 11), ("par-b", 22), ("par-c", 33)]
            .into_iter()
            .map(|(name, code)| {
                let (dir, request) =
                    source_extension(name, &format!("public int main() {{ return {code}; }}"));
                std::thread::spawn(move || {
                    let output = run_extension_request(request, &AtomicBool::new(false));
                    let _ = fs::remove_dir_all(&dir);
                    (code, output.unwrap().exit_code)
                })
            })
            .collect();
        for run in runs {
            let (expected, actual) = run.join().unwrap();
            assert_eq!(expected, actual);
        }
    }
}
