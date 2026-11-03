//! Debugger-Grundlage: Breakpoints, Sitzungszustand und austauschbares Backend.
//!
//! Breakpoints lassen sich bereits vollständig verwalten. Schrittweises Ausführen setzt
//! voraus, dass `nortexc` Debug-Informationen (Zeilentabellen) erzeugt; das ist aktuell
//! nicht der Fall. [`NoDebugInfoBackend`] meldet das ausdrücklich, statt eine Sitzung
//! vorzutäuschen. Ein echtes Backend implementiert [`DebugBackend`].

use crate::i18n::tr;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Breakpoint {
    pub file: PathBuf,
    /// 1-basierte Zeilennummer.
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StackFrame {
    pub function: String,
    pub file: PathBuf,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variable {
    pub name: String,
    pub type_name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SessionState {
    #[default]
    Inactive,
    Running,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchConfig {
    pub source: PathBuf,
    pub release: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DebugError {
    /// Der Compiler erzeugt keine Debug-Informationen, daher ist kein Anhalten möglich.
    NoDebugInfo,
}

impl std::fmt::Display for DebugError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDebugInfo => formatter.write_str(
                tr("Schrittweises Debugging ist noch nicht verfügbar: nortexc erzeugt keine Debug-Informationen."),
            ),
        }
    }
}

impl std::error::Error for DebugError {}

pub trait DebugBackend {
    fn name(&self) -> &'static str;
    fn is_available(&self) -> bool;
    fn start(
        &mut self,
        config: &LaunchConfig,
        breakpoints: &[Breakpoint],
    ) -> Result<(), DebugError>;
}

#[derive(Debug, Default)]
pub struct NoDebugInfoBackend;

impl DebugBackend for NoDebugInfoBackend {
    fn name(&self) -> &'static str {
        tr("Kein Backend (keine Debug-Informationen)")
    }

    fn is_available(&self) -> bool {
        false
    }

    fn start(&mut self, _: &LaunchConfig, _: &[Breakpoint]) -> Result<(), DebugError> {
        Err(DebugError::NoDebugInfo)
    }
}

pub struct Debugger {
    breakpoints: BTreeSet<Breakpoint>,
    backend: Box<dyn DebugBackend>,
    pub state: SessionState,
    pub call_stack: Vec<StackFrame>,
    pub variables: Vec<Variable>,
}

impl Default for Debugger {
    fn default() -> Self {
        Self {
            breakpoints: BTreeSet::new(),
            backend: Box::new(NoDebugInfoBackend),
            state: SessionState::Inactive,
            call_stack: Vec::new(),
            variables: Vec::new(),
        }
    }
}

impl Debugger {
    /// Schaltet den Breakpoint um und liefert `true`, wenn er danach gesetzt ist.
    pub fn toggle_breakpoint(&mut self, file: &Path, line: usize) -> bool {
        let breakpoint = Breakpoint {
            file: file.to_path_buf(),
            line,
        };
        if self.breakpoints.remove(&breakpoint) {
            false
        } else {
            self.breakpoints.insert(breakpoint);
            true
        }
    }

    pub fn remove_breakpoint(&mut self, breakpoint: &Breakpoint) {
        self.breakpoints.remove(breakpoint);
    }

    pub fn clear_breakpoints(&mut self) {
        self.breakpoints.clear();
    }

    pub fn breakpoints(&self) -> impl Iterator<Item = &Breakpoint> {
        self.breakpoints.iter()
    }

    pub fn breakpoint_count(&self) -> usize {
        self.breakpoints.len()
    }

    pub fn lines_in(&self, file: &Path) -> BTreeSet<usize> {
        self.breakpoints
            .iter()
            .filter(|breakpoint| breakpoint.file == file)
            .map(|breakpoint| breakpoint.line)
            .collect()
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend.name()
    }

    pub fn backend_available(&self) -> bool {
        self.backend.is_available()
    }

    /// Startet eine Sitzung. Ohne verfügbares Backend bleibt der Zustand `Inactive`.
    pub fn start(&mut self, config: &LaunchConfig) -> Result<(), DebugError> {
        let breakpoints: Vec<Breakpoint> = self.breakpoints.iter().cloned().collect();
        self.backend.start(config, &breakpoints)?;
        self.state = SessionState::Running;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.state = SessionState::Inactive;
        self.call_stack.clear();
        self.variables.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_adds_and_removes_breakpoints_per_file() {
        let mut debugger = Debugger::default();
        let file = Path::new("main.nx");
        assert!(debugger.toggle_breakpoint(file, 3));
        assert!(debugger.toggle_breakpoint(Path::new("other.nx"), 3));
        assert_eq!(debugger.lines_in(file), BTreeSet::from([3]));
        assert!(!debugger.toggle_breakpoint(file, 3));
        assert!(debugger.lines_in(file).is_empty());
        assert_eq!(debugger.breakpoint_count(), 1);
        debugger.clear_breakpoints();
        assert_eq!(debugger.breakpoint_count(), 0);
    }

    #[test]
    fn start_without_backend_reports_the_reason_and_stays_inactive() {
        let mut debugger = Debugger::default();
        let config = LaunchConfig {
            source: PathBuf::from("main.nx"),
            release: false,
        };
        assert_eq!(debugger.start(&config), Err(DebugError::NoDebugInfo));
        assert_eq!(debugger.state, SessionState::Inactive);
        assert!(!debugger.backend_available());
    }
}
