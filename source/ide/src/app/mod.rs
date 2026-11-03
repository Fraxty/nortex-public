//! Anwendungszustand und tabbasierte, UI-unabhängige Dokumentverwaltung.

use crate::editor::document::{DocumentError, EditorDocument};
use crate::plugins::PluginRegistry;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseDocumentResult {
    Closed,
    NeedsSaveConfirmation,
    NoDocument,
}

pub struct Application {
    pub running: bool,
    pub plugins: PluginRegistry,
    documents: Vec<EditorDocument>,
    active_document: Option<usize>,
}

impl Application {
    pub fn new() -> Self {
        Self {
            running: true,
            plugins: PluginRegistry::default(),
            documents: Vec::new(),
            active_document: None,
        }
    }

    pub fn new_document(&mut self) -> usize {
        self.documents.push(EditorDocument::untitled());
        let index = self.documents.len() - 1;
        self.active_document = Some(index);
        index
    }

    pub fn open_document(&mut self, path: impl AsRef<Path>) -> Result<usize, DocumentError> {
        let path = fs::canonicalize(path.as_ref()).map_err(DocumentError::Io)?;
        if let Some(index) = self
            .documents
            .iter()
            .position(|document| document.path() == Some(path.as_path()))
        {
            self.active_document = Some(index);
            return Ok(index);
        }
        let document = match EditorDocument::open(&path) {
            // Nicht-UTF-8-Textdateien (z. B. ANSI) werden lesbar geöffnet; Speichern verlangt Bestätigung.
            Err(DocumentError::InvalidUtf8(_)) => EditorDocument::open_lossy(&path)?,
            other => other?,
        };
        self.documents.push(document);
        let index = self.documents.len() - 1;
        self.active_document = Some(index);
        Ok(index)
    }

    pub fn active_document(&self) -> Option<&EditorDocument> {
        self.active_document
            .and_then(|index| self.documents.get(index))
    }

    pub fn active_document_mut(&mut self) -> Option<&mut EditorDocument> {
        self.active_document
            .and_then(|index| self.documents.get_mut(index))
    }

    pub fn documents(&self) -> &[EditorDocument] {
        &self.documents
    }

    pub fn documents_mut(&mut self) -> &mut [EditorDocument] {
        &mut self.documents
    }

    pub fn active_index(&self) -> Option<usize> {
        self.active_document
    }

    pub fn activate(&mut self, index: usize) -> bool {
        if index >= self.documents.len() {
            return false;
        }
        self.active_document = Some(index);
        true
    }

    pub fn save_active(&mut self) -> Result<(), DocumentError> {
        self.active_document_mut()
            .ok_or(DocumentError::MissingPath)?
            .save()
    }

    pub fn save_active_as(&mut self, path: impl AsRef<Path>) -> Result<(), DocumentError> {
        self.active_document_mut()
            .ok_or(DocumentError::MissingPath)?
            .save_as(path)
    }

    /// Ohne `discard_changes` wird ein modifiziertes Dokument nicht geschlossen.
    pub fn close_active(&mut self, discard_changes: bool) -> CloseDocumentResult {
        let Some(index) = self.active_document else {
            return CloseDocumentResult::NoDocument;
        };
        if self.documents[index].is_modified() && !discard_changes {
            return CloseDocumentResult::NeedsSaveConfirmation;
        }
        self.documents.remove(index);
        self.active_document = if self.documents.is_empty() {
            None
        } else {
            Some(index.min(self.documents.len() - 1))
        };
        CloseDocumentResult::Closed
    }
}

impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modified_untitled_document_requires_confirmation_before_close() {
        let mut app = Application::new();
        app.new_document();
        app.active_document_mut()
            .unwrap()
            .buffer_mut()
            .insert("unsaved");
        assert_eq!(
            app.close_active(false),
            CloseDocumentResult::NeedsSaveConfirmation
        );
        assert_eq!(app.close_active(true), CloseDocumentResult::Closed);
    }
}
