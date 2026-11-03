//! Hintergrundarbeit: Projekt-/Git-Refresh, Analyse, Build und Ergebnisverarbeitung.

use crate::*;
use nortex_ide::i18n::tr_fmt;

impl NortexCodeApp {
    pub(crate) fn refresh_project_files(&mut self) -> io::Result<()> {
        let root = PathBuf::from(&self.project_path_input);
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                tr_fmt(
                    "Projektordner existiert nicht: {}",
                    &[root.display().to_string()],
                ),
            ));
        }
        if self.project_refresh_running {
            self.project_refresh_pending = Some(root);
            return Ok(());
        }
        self.start_project_refresh(root)?;
        Ok(())
    }

    pub(crate) fn request_project_refresh(&mut self) {
        if let Err(error) = self.refresh_project_files() {
            self.status = tr_fmt(
                "Projekt konnte nicht aktualisiert werden: {}",
                &[error.to_string()],
            );
        }
    }

    pub(crate) fn start_project_refresh(&mut self, root: PathBuf) -> io::Result<()> {
        self.project_refresh_revision = self.project_refresh_revision.wrapping_add(1);
        let revision = self.project_refresh_revision;
        let sender = self.project_sender.clone();
        thread::Builder::new()
            .name("nortex-project-refresh".into())
            .spawn(move || {
                let result = read_project_snapshot(&root);
                let _ = sender.send(ProjectLoadResult { revision, result });
            })?;
        self.project_refresh_running = true;
        self.status = tr("Projekt wird aktualisiert …").into();
        Ok(())
    }

    pub(crate) fn active_file_label(&self) -> String {
        self.documents
            .active_document()
            .and_then(|document| document.path())
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "<untitled>".into())
    }

    pub(crate) fn schedule_analysis(&mut self) {
        self.diagnostics.clear();
        let Some(worker) = &self.analysis_worker else {
            return;
        };
        self.analysis_revision = self.analysis_revision.wrapping_add(1);
        let file = self.active_file_label();
        worker.submit(self.analysis_revision, file, self.editor_text.clone());
    }

    pub(crate) fn request_completion(
        worker: &mut CompletionWorker,
        documents: &Application,
        text: &str,
        project_files: &[PathBuf],
        cursor: usize,
        manual: bool,
    ) {
        worker.submit(CompletionRequest {
            document: documents.active_index().unwrap_or_default(),
            text: text.to_owned(),
            cursor,
            path: documents
                .active_document()
                .and_then(|document| document.path())
                .map(Path::to_path_buf),
            project_files: project_files.to_vec(),
            manual,
        });
    }

    fn poll_completion(&mut self) {
        let Some(reply) = self
            .completion_worker
            .as_mut()
            .and_then(CompletionWorker::poll)
        else {
            return;
        };
        let document = self.documents.active_index().unwrap_or_default();
        if !reply.matches(document, &self.editor_text, self.live_cursor) {
            return;
        }
        match reply.outcome {
            CompletionOutcome::Found(completion) => self.completion = Some(completion),
            CompletionOutcome::Empty => self.completion = None,
            CompletionOutcome::Failed => {
                self.completion = None;
                self.status = tr("Autovervollständigung ist fehlgeschlagen.").into();
            }
        }
    }

    pub(crate) fn poll_background_results(&mut self) {
        self.poll_completion();
        while let Ok(result) = self.analysis_results.try_recv() {
            if result.revision == self.analysis_revision {
                self.diagnostics = result.diagnostics;
            }
        }
        while let Ok(result) = self.project_results.try_recv() {
            if result.revision != self.project_refresh_revision {
                continue;
            }
            self.project_refresh_running = false;
            match result.result {
                Ok(snapshot) => {
                    let project_changed = snapshot.project_root != self.project_root;
                    self.project_root = snapshot.project_root;
                    self.project_path_input = self.project_root.display().to_string();
                    self.source_files = snapshot.source_files;
                    self.manifest = snapshot.manifest;
                    self.status = tr_fmt(
                        "Projekt geladen ({} Dateien)",
                        &[self.source_files.len().to_string()],
                    );
                    if project_changed {
                        self.request_git_refresh();
                    }
                }
                Err(error) => {
                    self.status = tr_fmt(
                        "Projekt konnte nicht aktualisiert werden: {}",
                        &[error.to_string()],
                    );
                }
            }
            if let Some(root) = self.project_refresh_pending.take() {
                if let Err(error) = self.start_project_refresh(root) {
                    self.status = tr_fmt(
                        "Projektaktualisierung fehlgeschlagen: {}",
                        &[error.to_string()],
                    );
                }
            }
        }
        while let Ok(result) = self.git_results.try_recv() {
            if result.revision == self.git_refresh_revision {
                self.git_refresh_running = false;
                if result.project_root == self.project_root {
                    self.git_status = result.status;
                } else {
                    self.git_refresh_pending = true;
                }
                if self.git_refresh_pending {
                    self.git_refresh_pending = false;
                    self.request_git_refresh();
                }
            }
        }
        while let Ok(event) = self.run_results.try_recv() {
            match event {
                RunEvent::Finished(outcome) => {
                    self.run_gate.finish();
                    self.last_run = Some(outcome);
                    self.build_squiggles = match outcome {
                        RunOutcome::Succeeded => Vec::new(),
                        RunOutcome::Failed => {
                            let file = self
                                .documents
                                .active_document()
                                .and_then(|document| document.path())
                                .map(|path| path.display().to_string());
                            inline::parse_compiler_log(&self.compiler_log, file.as_deref())
                        }
                    };
                    self.status = match outcome {
                        RunOutcome::Succeeded => tr("Build und Lauf abgeschlossen").into(),
                        RunOutcome::Failed => tr("Build fehlgeschlagen").into(),
                    };
                }
                RunEvent::Compiler(message) => {
                    self.compiler_log.push_str(&message);
                    self.compiler_log.push('\n');
                }
                RunEvent::Runtime(message) => {
                    self.runtime_output.push_str(&message);
                    self.runtime_output.push('\n');
                }
            }
        }
    }

    pub(crate) fn request_git_refresh(&mut self) {
        if self.git_refresh_running {
            self.git_refresh_pending = true;
            return;
        }
        self.git_refresh_revision = self.git_refresh_revision.wrapping_add(1);
        let revision = self.git_refresh_revision;
        let root = self.project_root.clone();
        let sender = self.git_sender.clone();
        match thread::Builder::new()
            .name("nortex-git-status".into())
            .spawn(move || {
                let status = git::query(&root);
                let _ = sender.send(GitLoadResult {
                    revision,
                    project_root: root,
                    status,
                });
            }) {
            Ok(_) => self.git_refresh_running = true,
            Err(error) => {
                self.status = tr_fmt(
                    "Git-Abfrage konnte nicht gestartet werden: {}",
                    &[error.to_string()],
                );
            }
        }
    }

    pub(crate) fn start_build_and_run(&mut self) {
        if self.run_gate.is_running() {
            return;
        }
        if !self.save_document() {
            return;
        }
        let manifest = self.manifest.as_ref().map(|(path, manifest)| {
            (
                path.clone(),
                manifest.default_output(path),
                manifest.build.overflow_checks.is_some(),
            )
        });
        let source = match (
            &manifest,
            self.documents
                .active_document()
                .and_then(|document| document.path()),
        ) {
            (Some((path, _, _)), _) => path.clone(),
            (None, Some(path)) => path.to_path_buf(),
            (None, None) => {
                self.status = tr("Zum Bauen zuerst einen Dateipfad angeben und speichern.").into();
                return;
            }
        };
        let sender = self.run_sender.clone();
        let project_root = self.project_root.clone();
        let overflow_checks = self.settings.overflow_checks;
        self.compiler_log.clear();
        self.runtime_output.clear();
        self.last_run = None;
        self.problems_visible = true;
        if !self.run_gate.try_begin() {
            return;
        }
        self.status = tr("Build läuft im Hintergrund ...").into();
        let spawn_result = thread::Builder::new()
            .name("nortex-build-run".into())
            .spawn(move || {
                let outcome = execute_build_and_run(
                    &project_root,
                    &source,
                    overflow_checks,
                    manifest,
                    &sender,
                );
                let _ = sender.send(RunEvent::Finished(outcome));
            });
        if let Err(error) = spawn_result {
            self.run_gate.finish();
            self.status = tr_fmt(
                "Build-Worker konnte nicht gestartet werden: {}",
                &[error.to_string()],
            );
        }
    }

    pub(crate) fn persist_settings(&mut self) {
        if let Err(error) = self.settings.save(&self.settings_path) {
            self.status = tr_fmt(
                "Einstellungen konnten nicht gespeichert werden: {}",
                &[error.to_string()],
            );
        }
    }

    /// Speichert geänderte Dokumente mit Pfad höchstens alle 30 Sekunden.
    pub(crate) fn run_auto_save(&mut self, context: &egui::Context) {
        const INTERVAL_SECONDS: f64 = 30.0;
        if !self.settings.auto_save_enabled {
            return;
        }
        let now = context.input(|input| input.time);
        if now - self.last_auto_save < INTERVAL_SECONDS {
            return;
        }
        self.last_auto_save = now;
        self.sync_active_buffer();
        let mut saved = 0;
        for document in self.documents.documents_mut() {
            if document.is_modified() && document.path().is_some() && document.save().is_ok() {
                saved += 1;
            }
        }
        if saved > 0 {
            self.status = tr_fmt("Automatisch gespeichert ({})", &[saved.to_string()]);
        }
        context.request_repaint_after(Duration::from_secs(INTERVAL_SECONDS as u64));
    }
}
