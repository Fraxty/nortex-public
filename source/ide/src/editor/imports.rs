//! Import-Auflösung für IntelliSense.
//!
//! Die Dateisuche übernimmt `find_module` des Compilers; die Traversierung folgt
//! `ModuleResolver`: Post-Order, jedes Modul höchstens einmal, `System.*` wird nicht
//! als Datei gesucht. Zyklen und doppelte Imports werden hier nur übersprungen – die
//! Fehlermeldungen dazu liefert derselbe Resolver über `check_file_with_source`.
//!
//! Geparste Moduldateien werden pro Pfad zwischengespeichert und über Änderungszeit
//! und Länge invalidiert. Der Importgraph selbst wird bei jeder Anfrage neu
//! durchlaufen (ein `find_module` pro `using`), damit neu angelegte, gelöschte oder
//! umbenannte Dateien sofort berücksichtigt werden, ohne dass Dateien neu geparst
//! werden müssen.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::SystemTime;

use nortex_compiler::ast::{Program, UsingDirective};
use nortex_compiler::lexer::Lexer;
use nortex_compiler::parser::Parser;
use nortex_compiler::semantic::resolver::find_module;

use super::analysis::{Counter, Phase, count, timed};

const MAX_MODULE_BYTES: u64 = 1024 * 1024;
const MAX_CACHED_MODULES: usize = 500;

#[derive(Debug, Clone)]
pub struct ImportedModule {
    pub path: PathBuf,
    pub program: Arc<Program>,
}

struct StoredModule {
    modified: Option<SystemTime>,
    length: u64,
    program: Arc<Program>,
}

static MODULES: OnceLock<Mutex<HashMap<PathBuf, StoredModule>>> = OnceLock::new();

fn load_module(path: &Path) -> Option<Arc<Program>> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > MAX_MODULE_BYTES {
        return None;
    }
    let modified = metadata.modified().ok();
    let modules = MODULES.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let modules = modules
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(stored) = modules.get(path)
            && stored.modified == modified
            && stored.length == metadata.len()
        {
            count(Counter::ProjectCacheHit, 1);
            return Some(stored.program.clone());
        }
    }

    count(Counter::ProjectCacheMiss, 1);
    let source = fs::read_to_string(path).ok()?;
    let program = Arc::new(timed(Phase::ProjectIndex, || {
        let tokens = Lexer::new(&source).tokenize().tokens;
        Parser::new(tokens).parse().program
    }));
    let mut modules = modules
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if modules.len() >= MAX_CACHED_MODULES {
        modules.clear();
    }
    modules.insert(
        path.to_owned(),
        StoredModule {
            modified,
            length: metadata.len(),
            program: program.clone(),
        },
    );
    Some(program)
}

struct Walk {
    root: PathBuf,
    seen: HashSet<PathBuf>,
    modules: Vec<ImportedModule>,
}

impl Walk {
    fn visit(&mut self, base: &Path, usings: &[UsingDirective]) {
        for using in usings {
            if using.path.first().is_some_and(|part| part == "System") {
                continue;
            }
            let Some(target) = find_module(base, &self.root, &using.path) else {
                continue;
            };
            if !self.seen.insert(target.clone()) {
                continue;
            }
            let Some(program) = load_module(&target) else {
                continue;
            };
            let target_base = target.parent().unwrap_or(&self.root).to_path_buf();
            self.visit(&target_base, &program.usings);
            self.modules.push(ImportedModule {
                path: target,
                program,
            });
        }
    }
}

/// Alle von `active_path` aus transitiv importierten Module in Abhängigkeitsreihenfolge
/// (Post-Order, ohne die aktive Datei). Die Quellwurzel ist – wie beim Build – der
/// Ordner der Datei, die als Einstieg gedacht ist, hier also der aktiven Datei.
pub fn resolve_imports(active_path: &Path, active: &Program) -> Vec<ImportedModule> {
    timed(Phase::ImportResolution, || {
        let Ok(canonical) = fs::canonicalize(active_path) else {
            return Vec::new();
        };
        let root = canonical.parent().unwrap_or(Path::new(".")).to_path_buf();
        let mut walk = Walk {
            root: root.clone(),
            seen: HashSet::from([canonical]),
            modules: Vec::new(),
        };
        walk.visit(&root, &active.usings);
        walk.modules
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nortex_compiler::ast::Declaration;
    use nortex_compiler::semantic::resolver::ModuleResolver;

    fn project(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nx_ide_imports_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        for (file, content) in files {
            let path = dir.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        dir
    }

    fn parse(source: &str) -> Program {
        Parser::new(Lexer::new(source).tokenize().tokens)
            .parse()
            .program
    }

    fn resolve(dir: &Path, entry: &str) -> Vec<String> {
        let path = dir.join(entry);
        let program = parse(&fs::read_to_string(&path).unwrap());
        resolve_imports(&path, &program)
            .iter()
            .map(|module| {
                module
                    .path
                    .strip_prefix(fs::canonicalize(dir).unwrap())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    fn declaration_names(program: &Program) -> Vec<String> {
        program
            .declarations
            .iter()
            .map(|declaration| match declaration {
                Declaration::Class(class) => class.name.clone(),
                Declaration::Function(function) => function.name.clone(),
                Declaration::GlobalVariable(global) => global.name.clone(),
            })
            .collect()
    }

    #[test]
    fn resolves_relative_and_project_root_imports_in_dependency_order() {
        let dir = project(
            "order",
            &[
                (
                    "main.nx",
                    "using net.http;\nusing util;\npublic int main() { return 0; }",
                ),
                ("util.nx", "public int twice(int x) { return x * 2; }"),
                ("net/http.nx", "using client;\npublic class Http {}"),
                ("net/client.nx", "public class Client {}"),
            ],
        );
        assert_eq!(
            resolve(&dir, "main.nx"),
            ["net/client.nx", "net/http.nx", "util.nx"]
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn root_relative_import_works_from_a_nested_module() {
        let dir = project(
            "root",
            &[
                (
                    "main.nx",
                    "using feature.api;\npublic int main() { return 0; }",
                ),
                ("feature/api.nx", "using shared;\npublic class Api {}"),
                ("shared.nx", "public class Shared {}"),
            ],
        );
        assert_eq!(resolve(&dir, "main.nx"), ["shared.nx", "feature/api.nx"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn mod_nx_directory_modules_system_namespaces_and_missing_modules() {
        let dir = project(
            "mod",
            &[
                (
                    "main.nx",
                    "using System.Text;\nusing pkg;\nusing missing.thing;\npublic int main() { return 0; }",
                ),
                ("pkg/mod.nx", "public class Pkg {}"),
            ],
        );
        assert_eq!(resolve(&dir, "main.nx"), ["pkg/mod.nx"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn duplicate_imports_and_diamonds_load_a_module_once() {
        let dir = project(
            "diamond",
            &[
                (
                    "main.nx",
                    "using b;\nusing c;\nusing b;\npublic int main() { return 0; }",
                ),
                ("b.nx", "using d;\npublic class B {}"),
                ("c.nx", "using d;\npublic class C {}"),
                ("d.nx", "public class D {}"),
            ],
        );
        assert_eq!(resolve(&dir, "main.nx"), ["d.nx", "b.nx", "c.nx"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cycles_terminate_and_do_not_include_the_active_file() {
        let dir = project(
            "cycle",
            &[
                ("main.nx", "using a;\npublic int main() { return 0; }"),
                ("a.nx", "using b;\npublic class A {}"),
                ("b.nx", "using a;\nusing main;\npublic class B {}"),
            ],
        );
        assert_eq!(resolve(&dir, "main.nx"), ["b.nx", "a.nx"]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn imported_file_changes_and_import_path_changes_are_picked_up() {
        let dir = project(
            "changes",
            &[
                ("main.nx", "using one;\npublic int main() { return 0; }"),
                ("one.nx", "public class One {}"),
                ("two.nx", "public class Two {}"),
            ],
        );
        let path = dir.join("main.nx");
        let first = resolve_imports(&path, &parse(&fs::read_to_string(&path).unwrap()));
        assert_eq!(declaration_names(&first[0].program), ["One"]);

        fs::write(
            dir.join("one.nx"),
            "public class One {}\npublic class Uno {}",
        )
        .unwrap();
        let changed = resolve_imports(&path, &parse(&fs::read_to_string(&path).unwrap()));
        assert_eq!(declaration_names(&changed[0].program), ["One", "Uno"]);

        let retargeted = parse("using two;\npublic int main() { return 0; }");
        let after = resolve_imports(&path, &retargeted);
        assert_eq!(declaration_names(&after[0].program), ["Two"]);

        fs::remove_file(dir.join("two.nx")).unwrap();
        assert!(resolve_imports(&path, &retargeted).is_empty());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unchanged_modules_are_served_from_the_cache() {
        let dir = project(
            "cache",
            &[
                ("main.nx", "using one;\npublic int main() { return 0; }"),
                ("one.nx", "public class One {}"),
            ],
        );
        let path = dir.join("main.nx");
        let program = parse(&fs::read_to_string(&path).unwrap());
        let first = resolve_imports(&path, &program);
        let second = resolve_imports(&path, &program);
        assert!(Arc::ptr_eq(&first[0].program, &second[0].program));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn module_order_matches_the_compiler_resolver() {
        let dir = project(
            "equivalence",
            &[
                (
                    "main.nx",
                    "using net.http;\nusing util;\nusing util;\npublic int main() { return 0; }",
                ),
                (
                    "util.nx",
                    "using core;\npublic int twice(int x) { return x * 2; }",
                ),
                ("core.nx", "public class Core {}"),
                (
                    "net/http.nx",
                    "using client;\nusing util;\npublic class Http {}",
                ),
                ("net/client.nx", "public class Client {}"),
            ],
        );
        let path = dir.join("main.nx");
        let program = parse(&fs::read_to_string(&path).unwrap());
        let mut ide_names: Vec<String> = resolve_imports(&path, &program)
            .iter()
            .flat_map(|module| declaration_names(&module.program))
            .collect();
        ide_names.extend(declaration_names(&program));

        let loaded = ModuleResolver::load(&path).unwrap();
        assert_eq!(ide_names, declaration_names(&loaded.program));
        fs::remove_dir_all(dir).unwrap();
    }
}
