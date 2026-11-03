//! Modul-AuflÃ¶sung fÃ¼r `using`-Direktiven, die auf `.nx`-Dateien zeigen.
//!
//! Jede Datei wird einzeln geparst; die Deklarationen der importierten Module
//! werden in AbhÃ¤ngigkeitsreihenfolge (Post-Order) vor die des Importeurs
//! gestellt. Ein Modul wird hÃ¶chstens einmal geladen (Diamant-Importe);
//! Zyklen werden Ã¼ber den Zustand `Visiting` erkannt.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};

use crate::ast::{Declaration, Program, UsingDirective};
use crate::diagnostics::Diagnostic;
use crate::lexer::Lexer;
use crate::parser::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModuleState {
    Visiting,
    Loaded,
}

#[derive(Debug)]
pub struct LoadedProgram {
    pub program: Program,
    pub diagnostics: Vec<Diagnostic>,
    pub token_count: usize,
}

#[derive(Debug, Default)]
pub struct ModuleResolver {
    states: HashMap<PathBuf, ModuleState>,
    declarations: Vec<Declaration>,
    usings: Vec<UsingDirective>,
    diagnostics: Vec<Diagnostic>,
    token_count: usize,
}

impl ModuleResolver {
    /// LÃ¤dt `entry` samt aller per `using` erreichbaren Moduldateien.
    pub fn load(entry: &Path) -> io::Result<LoadedProgram> {
        Self::load_with_source(entry, None)
    }

    /// Wie `load`, nutzt für die Einstiegsdatei aber `source` (z. B. einen
    /// ungespeicherten Editorpuffer) statt des Dateiinhalts. Die Datei muss existieren,
    /// damit der Quellwurzelordner feststeht.
    pub fn load_with_source(entry: &Path, source: Option<&str>) -> io::Result<LoadedProgram> {
        let canonical = std::fs::canonicalize(entry)?;
        let root = canonical.parent().unwrap_or(Path::new(".")).to_path_buf();
        let mut resolver = Self::default();
        let mut program = resolver.load_module(entry, &root, source)?;

        let mut declarations = std::mem::take(&mut resolver.declarations);
        declarations.append(&mut program.declarations);
        program.declarations = declarations;

        let mut seen = HashSet::new();
        let mut usings = std::mem::take(&mut resolver.usings);
        usings.append(&mut program.usings);
        usings.retain(|using| seen.insert(using.path.clone()));
        program.usings = usings;

        Ok(LoadedProgram {
            program,
            diagnostics: resolver.diagnostics,
            token_count: resolver.token_count,
        })
    }

    fn load_module(
        &mut self,
        path: &Path,
        root: &Path,
        source_override: Option<&str>,
    ) -> io::Result<Program> {
        let canonical = std::fs::canonicalize(path)?;
        self.states.insert(canonical.clone(), ModuleState::Visiting);
        let source = match source_override {
            Some(source) => source.to_owned(),
            None => std::fs::read_to_string(path)?,
        };
        let display = path.display().to_string();

        let lexed = Lexer::new(&source).tokenize();
        self.token_count += lexed.tokens.len().saturating_sub(1);
        let parsed = Parser::new(lexed.tokens).parse();
        for mut diagnostic in lexed.diagnostics.into_iter().chain(parsed.diagnostics) {
            diagnostic.file = Some(display.clone());
            self.diagnostics.push(diagnostic);
        }

        let mut program = parsed.program;
        let base = canonical.parent().unwrap_or(root).to_path_buf();
        let mut kept = Vec::new();
        for using in std::mem::take(&mut program.usings) {
            if using.path.first().is_some_and(|part| part == "System") {
                kept.push(using);
                continue;
            }
            let Some(target) = find_module(&base, root, &using.path) else {
                kept.push(using);
                continue;
            };
            match self.states.get(&target) {
                Some(ModuleState::Visiting) => self.error(
                    &display,
                    &using,
                    format!(
                        "Zyklischer Import erkannt: '{}' importiert '{}'.",
                        canonical.display(),
                        target.display()
                    ),
                ),
                Some(ModuleState::Loaded) => {}
                None => match self.load_module(&target, root, None) {
                    Ok(module) => {
                        self.declarations.extend(module.declarations);
                        self.usings.extend(module.usings);
                    }
                    Err(error) => self.error(
                        &display,
                        &using,
                        format!("Modul '{}' ist nicht lesbar: {error}", target.display()),
                    ),
                },
            }
        }
        program.usings = kept;
        self.states.insert(canonical, ModuleState::Loaded);
        Ok(program)
    }

    fn error(&mut self, file: &str, using: &UsingDirective, message: String) {
        let mut diagnostic = Diagnostic::error_with_code("NX1003", message, using.span);
        diagnostic.file = Some(file.to_owned());
        self.diagnostics.push(diagnostic);
    }
}

/// `using a.b;` â†’ `<dir>/a/b.nx` oder `<dir>/a/b/mod.nx`, erst relativ zur
/// importierenden Datei, dann relativ zum Quellwurzel (Ordner der Einstiegsdatei).
pub fn find_module(base: &Path, root: &Path, parts: &[String]) -> Option<PathBuf> {
    let relative: PathBuf = parts.iter().collect();
    let mut directories = vec![base];
    if root != base {
        directories.push(root);
    }
    for directory in directories {
        let candidates = [
            directory.join(&relative).with_extension("nx"),
            directory.join(&relative).join("mod.nx"),
        ];
        for candidate in candidates {
            if candidate.is_file() {
                return std::fs::canonicalize(candidate).ok();
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nx_resolver_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (file, content) in files {
            std::fs::write(dir.join(file), content).unwrap();
        }
        dir
    }

    #[test]
    fn merges_imported_declarations_and_removes_the_using() {
        let dir = temp_project(
            "merge",
            &[
                ("util.nx", "public int twice(int x) { return x * 2; }"),
                (
                    "main.nx",
                    "using util;\npublic int main() { return twice(21); }",
                ),
            ],
        );
        let loaded = ModuleResolver::load(&dir.join("main.nx")).unwrap();
        assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
        assert!(loaded.program.usings.is_empty());
        assert_eq!(loaded.program.declarations.len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn diamond_imports_load_a_module_once() {
        let dir = temp_project(
            "diamond",
            &[
                ("d.nx", "public int shared() { return 1; }"),
                ("b.nx", "using d;\npublic int fromB() { return shared(); }"),
                ("c.nx", "using d;\npublic int fromC() { return shared(); }"),
                (
                    "main.nx",
                    "using b;\nusing c;\npublic int main() { return fromB() + fromC(); }",
                ),
            ],
        );
        let loaded = ModuleResolver::load(&dir.join("main.nx")).unwrap();
        assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
        assert_eq!(loaded.program.declarations.len(), 4);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn import_cycles_are_reported() {
        let dir = temp_project(
            "cycle",
            &[
                ("a.nx", "using b;\npublic int fa() { return 1; }"),
                ("b.nx", "using a;\npublic int fb() { return 2; }"),
                ("main.nx", "using a;\npublic int main() { return fa(); }"),
            ],
        );
        let loaded = ModuleResolver::load(&dir.join("main.nx")).unwrap();
        assert!(
            loaded
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("Zyklischer Import")),
            "{:?}",
            loaded.diagnostics
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn unknown_namespaces_keep_their_using_for_the_semantic_error() {
        let dir = temp_project(
            "unknown",
            &[(
                "main.nx",
                "using nirgends;\npublic int main() { return 0; }",
            )],
        );
        let loaded = ModuleResolver::load(&dir.join("main.nx")).unwrap();
        assert_eq!(loaded.program.usings.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn entry_source_override_replaces_the_file_content_but_not_the_imports() {
        let dir = temp_project(
            "override",
            &[
                ("util.nx", "public int twice(int x) { return x * 2; }"),
                ("main.nx", "public int main() { return 0; }"),
            ],
        );
        let unsaved = "using util;\npublic int main() { return twice(4); }";
        let loaded = ModuleResolver::load_with_source(&dir.join("main.nx"), Some(unsaved)).unwrap();
        assert!(loaded.diagnostics.is_empty(), "{:?}", loaded.diagnostics);
        assert_eq!(loaded.program.declarations.len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }
}
