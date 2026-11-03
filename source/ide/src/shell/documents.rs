//! Dokumente öffnen, speichern, schließen und neu anlegen.

use crate::*;
use nortex_ide::i18n::tr_fmt;

impl NortexCodeApp {
    pub(crate) fn open_file(&mut self, path: PathBuf) {
        match self.documents.open_document(&path) {
            Ok(_) => {
                if let Some(index) = self.documents.active_index() {
                    self.grid.open(index);
                    self.after_tab_switch();
                }
                self.editor_text = self
                    .documents
                    .active_document()
                    .map(|document| document.buffer().materialize())
                    .unwrap_or_default();
                self.synced_text = self.editor_text.clone();
                self.highlight_spans = highlight_nortex(&self.editor_text);
                self.schedule_analysis();
                self.status = tr_fmt("Geöffnet: {}", &[path.display().to_string()]);
                self.workspace.view_mode = ViewMode::Editor;
                self.settings.push_recent(&path);
                self.persist_settings();
            }
            Err(error) => {
                self.status = tr_fmt(
                    "Datei konnte nicht geöffnet werden: {}",
                    &[error.to_string()],
                )
            }
        }
    }

    pub(crate) fn new_document(&mut self) {
        self.documents.new_document();
        if let Some(index) = self.documents.active_index() {
            self.grid.open(index);
            self.after_tab_switch();
        }
        self.editor_text.clear();
        self.synced_text.clear();
        self.highlight_spans.clear();
        self.diagnostics.clear();
        self.status = tr("Neues Dokument").into();
        self.schedule_analysis();
    }

    pub(crate) fn activate_document(&mut self, index: usize) {
        if self.documents.activate(index) {
            self.editor_text = self
                .documents
                .active_document()
                .map(|document| document.buffer().materialize())
                .unwrap_or_default();
            self.synced_text.clone_from(&self.editor_text);
            self.highlight_spans = highlight_nortex(&self.editor_text);
            self.schedule_analysis();
        }
    }

    pub(crate) fn save_document(&mut self) -> bool {
        if self
            .documents
            .active_document()
            .and_then(|document| document.path())
            .is_none()
        {
            return self.save_document_as();
        }
        self.sync_active_buffer();
        let result = self.documents.save_active();
        match result {
            Ok(()) => {
                self.status = tr("Datei gespeichert").into();
                self.request_project_refresh();
                true
            }
            Err(error) => {
                self.status = tr_fmt("Speichern fehlgeschlagen: {}", &[error.to_string()]);
                false
            }
        }
    }

    pub(crate) fn save_document_as(&mut self) -> bool {
        self.sync_active_buffer();
        let mut dialog = rfd::FileDialog::new()
            .set_directory(&self.project_root)
            .add_filter(tr("NORTEX-Dateien"), &["nx", "nortex"]);
        if let Some(path) = self
            .documents
            .active_document()
            .and_then(|document| document.path())
        {
            if let Some(parent) = path.parent() {
                dialog = dialog.set_directory(parent);
            }
            if let Some(name) = path.file_name() {
                dialog = dialog.set_file_name(name.to_string_lossy());
            }
        } else {
            dialog = dialog.set_file_name("main.nx");
        }
        let Some(path) = dialog.save_file() else {
            return false;
        };
        match self.documents.save_active_as(&path) {
            Ok(()) => {
                self.status = tr("Datei gespeichert").into();
                self.request_project_refresh();
                true
            }
            Err(error) => {
                self.status = tr_fmt("Speichern unter fehlgeschlagen: {}", &[error.to_string()]);
                false
            }
        }
    }

    pub(crate) fn sync_active_buffer(&mut self) {
        if let Some(document) = self.documents.active_document_mut() {
            sync_buffer(document.buffer_mut(), &self.synced_text, &self.editor_text);
        }
        self.synced_text.clone_from(&self.editor_text);
    }

    pub(crate) fn close_document(&mut self, pane: usize, index: usize, discard_changes: bool) {
        use nortex_ide::app::CloseDocumentResult;
        if self.pending_close_document.is_some() {
            return;
        }
        let previous = self.documents.active_index();
        if !self.documents.activate(index) {
            return;
        }
        match self.documents.close_active(discard_changes) {
            CloseDocumentResult::Closed => {
                self.grid.document_removed(index);
                if let Some(doc) = self.grid.active_doc() {
                    self.documents.activate(doc);
                }
                self.after_tab_switch();
                self.editor_text = self
                    .documents
                    .active_document()
                    .map(|document| document.buffer().materialize())
                    .unwrap_or_default();
                self.synced_text.clone_from(&self.editor_text);
                self.highlight_spans = highlight_nortex(&self.editor_text);
                self.diagnostics.clear();
                self.schedule_analysis();
            }
            CloseDocumentResult::NeedsSaveConfirmation => {
                if let Some(previous) = previous {
                    self.documents.activate(previous);
                }
                self.pending_close_document = Some((pane, index));
                self.status = tr("Ungespeicherte Änderungen: Speichern oder Verwerfen.").into();
            }
            CloseDocumentResult::NoDocument => {}
        }
    }

    pub(crate) fn handle_dropped_files(&mut self, context: &egui::Context) {
        let dropped = context.input(|input| input.raw.dropped_files.clone());
        for file in dropped {
            let Some(path) = file.path else { continue };
            let is_source = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| matches!(extension, "nx" | "nortex"));
            if is_source {
                self.open_file(path);
            } else {
                self.status = tr_fmt(
                    "Nur .nx-Dateien werden geöffnet: {}",
                    &[path.display().to_string()],
                );
            }
        }
    }

    pub(crate) fn set_editor_text(&mut self, text: String) {
        self.editor_text = text;
        self.sync_active_buffer();
        self.highlight_spans = highlight_nortex(&self.editor_text);
        self.completion = None;
        self.schedule_analysis();
    }
}
