//! Analyse des aktiven Dokuments: inkrementelles Lexen/Parsen mit Ein-Eintrag-
//! Cache sowie Zähler und Zeitmessungen für die IntelliSense-Phasen.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use nortex_compiler::ast::Program;
use nortex_compiler::diagnostics::SourceSpan;
use nortex_compiler::lexer::incremental::{common_affixes, relex};
use nortex_compiler::lexer::{LexOutput, Lexer, Token};
use nortex_compiler::parser::incremental::reparse;
use nortex_compiler::parser::{ParsedFile, Parser};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    LexFull,
    LexIncremental,
    ParseFull,
    ParseIncremental,
    ProjectIndex,
    ImportResolution,
    SymbolsAndTypes,
    Request,
}

const PHASES: [Phase; 8] = [
    Phase::LexFull,
    Phase::LexIncremental,
    Phase::ParseFull,
    Phase::ParseIncremental,
    Phase::ProjectIndex,
    Phase::ImportResolution,
    Phase::SymbolsAndTypes,
    Phase::Request,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counter {
    ActiveCacheHit,
    ActiveCacheMiss,
    IncrementalFallback,
    ItemsReusedBefore,
    ItemsReparsed,
    ItemsReusedAfter,
    ProjectCacheHit,
    ProjectCacheMiss,
}

const COUNTERS: usize = 8;

struct Stats {
    nanos: [AtomicU64; PHASES.len()],
    calls: [AtomicU64; PHASES.len()],
    counters: [AtomicU64; COUNTERS],
}

#[allow(clippy::declare_interior_mutable_const)]
const ZERO: AtomicU64 = AtomicU64::new(0);

static STATS: Stats = Stats {
    nanos: [ZERO; PHASES.len()],
    calls: [ZERO; PHASES.len()],
    counters: [ZERO; COUNTERS],
};

/// Messwerte seit dem letzten `reset`. Die Zähler sind prozessweit; parallele
/// Tests sehen daher auch fremde Aufrufe.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PhaseStats {
    pub calls: u64,
    pub nanos: u64,
}

#[derive(Debug, Clone, Default)]
pub struct StatsSnapshot {
    phases: [PhaseStats; PHASES.len()],
    counters: [u64; COUNTERS],
}

impl StatsSnapshot {
    pub fn phase(&self, phase: Phase) -> PhaseStats {
        self.phases[phase as usize]
    }

    pub fn counter(&self, counter: Counter) -> u64 {
        self.counters[counter as usize]
    }
}

pub fn snapshot() -> StatsSnapshot {
    let mut snapshot = StatsSnapshot::default();
    for index in 0..PHASES.len() {
        snapshot.phases[index] = PhaseStats {
            calls: STATS.calls[index].load(Ordering::Relaxed),
            nanos: STATS.nanos[index].load(Ordering::Relaxed),
        };
    }
    for (index, slot) in snapshot.counters.iter_mut().enumerate() {
        *slot = STATS.counters[index].load(Ordering::Relaxed);
    }
    snapshot
}

pub fn reset() {
    for index in 0..PHASES.len() {
        STATS.calls[index].store(0, Ordering::Relaxed);
        STATS.nanos[index].store(0, Ordering::Relaxed);
    }
    for counter in &STATS.counters {
        counter.store(0, Ordering::Relaxed);
    }
}

pub(super) fn count(counter: Counter, amount: usize) {
    STATS.counters[counter as usize].fetch_add(amount as u64, Ordering::Relaxed);
}

pub(super) fn timed<T>(phase: Phase, work: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let value = work();
    let elapsed = u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX);
    STATS.nanos[phase as usize].fetch_add(elapsed, Ordering::Relaxed);
    STATS.calls[phase as usize].fetch_add(1, Ordering::Relaxed);
    value
}

pub(super) struct ActiveDocument {
    lexed: LexOutput,
    parsed: ParsedFile,
}

impl ActiveDocument {
    pub(super) fn tokens(&self) -> &[Token] {
        &self.lexed.tokens
    }

    pub(super) fn comments(&self) -> &[SourceSpan] {
        &self.lexed.comments
    }

    pub(super) fn program(&self) -> &Program {
        &self.parsed.program
    }

    fn analyze_full(source: &str) -> Self {
        let lexed = timed(Phase::LexFull, || Lexer::new(source).tokenize());
        let parsed = timed(Phase::ParseFull, || {
            Parser::new(lexed.tokens.clone()).parse_traced()
        });
        Self { lexed, parsed }
    }

    fn analyze_incremental(&self, old_source: &str, source: &str) -> Self {
        let relexed = timed(Phase::LexIncremental, || {
            relex(&self.lexed, old_source, source)
        });
        let (parsed, stats) = timed(Phase::ParseIncremental, || {
            reparse(relexed.output.tokens.clone(), &self.parsed, &relexed.change)
        });
        count(Counter::ItemsReusedBefore, stats.reused_before);
        count(Counter::ItemsReparsed, stats.reparsed);
        count(Counter::ItemsReusedAfter, stats.reused_after);
        Self {
            lexed: relexed.output,
            parsed,
        }
    }
}

/// Eine Completion-Anfrage analysiert den Text und die um fehlende `}` ergänzte Fassung
/// (siehe `close_open_blocks`). Beide brauchen einen eigenen Eintrag, sonst überschreiben
/// sie sich gegenseitig und kein Aufruf trifft je den Cache.
const CACHED_DOCUMENTS: usize = 2;

#[derive(Default)]
struct DocumentCache {
    entries: Vec<(String, Arc<ActiveDocument>)>,
}

impl DocumentCache {
    fn analyze(&mut self, source: &str) -> Arc<ActiveDocument> {
        if let Some(position) = self.entries.iter().position(|(cached, _)| cached == source) {
            count(Counter::ActiveCacheHit, 1);
            let entry = self.entries.remove(position);
            let document = entry.1.clone();
            self.entries.push(entry);
            return document;
        }
        count(Counter::ActiveCacheMiss, 1);

        let best_base = self
            .entries
            .iter()
            .map(|(old_source, old)| {
                let (prefix, suffix) = common_affixes(old_source, source);
                (prefix + suffix, old_source, old)
            })
            .max_by_key(|(shared, ..)| *shared)
            .filter(|(shared, ..)| shared * 2 >= source.len());
        let document = Arc::new(match best_base {
            Some((_, old_source, old)) => old.analyze_incremental(old_source, source),
            None => {
                if !self.entries.is_empty() {
                    count(Counter::IncrementalFallback, 1);
                }
                ActiveDocument::analyze_full(source)
            }
        });
        self.entries.push((source.to_owned(), document.clone()));
        if self.entries.len() > CACHED_DOCUMENTS {
            self.entries.remove(0);
        }
        document
    }
}

static ACTIVE_DOCUMENTS: OnceLock<Mutex<DocumentCache>> = OnceLock::new();

/// Liefert die Analyse für `source`. Unveränderter Text kommt aus dem Cache;
/// bei einer Änderung wird der ähnlichste Eintrag inkrementell fortgeschrieben,
/// solange sich die Texte überhaupt ähneln (z. B. nicht beim Tabwechsel).
pub(super) fn analyze_active_document(source: &str) -> Arc<ActiveDocument> {
    ACTIVE_DOCUMENTS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .analyze(source)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "using System.Text;\npublic class Box {\n    public int width;\n    public int area() { return width * width; }\n}\npublic int main() {\n    Box box = new Box();\n    return box.area();\n}\n";

    #[test]
    fn incremental_updates_equal_a_full_analysis() {
        let edits = [
            SOURCE.replace("width * width", "width * height"),
            SOURCE.replace(
                "public int width;",
                "public int width;\n    public int height;",
            ),
            SOURCE.replace("return box.area();", "return box.ar"),
            SOURCE.replace("public int main() {", "public int main( {"),
            SOURCE.to_string(),
            String::new(),
            SOURCE.to_string(),
        ];
        analyze_active_document(SOURCE);
        for edited in edits {
            let incremental = analyze_active_document(&edited);
            let full = ActiveDocument::analyze_full(&edited);
            assert_eq!(incremental.lexed.tokens, full.lexed.tokens);
            assert_eq!(incremental.lexed.comments, full.lexed.comments);
            assert_eq!(incremental.lexed.diagnostics, full.lexed.diagnostics);
            assert_eq!(incremental.parsed, full.parsed);
        }
    }

    #[test]
    fn unchanged_text_is_served_from_the_cache() {
        let source = format!("{SOURCE}// cache-test {}\n", std::process::id());
        let first = analyze_active_document(&source);
        let second = analyze_active_document(&source);
        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn text_and_its_closed_variant_both_stay_cached() {
        let mut cache = DocumentCache::default();
        let source = format!("{SOURCE}public void open() {{\n");
        let closed = format!("{source}}}");
        let first = (cache.analyze(&source), cache.analyze(&closed));
        let again = (cache.analyze(&source), cache.analyze(&closed));
        assert!(Arc::ptr_eq(&first.0, &again.0));
        assert!(Arc::ptr_eq(&first.1, &again.1));
    }

    #[test]
    fn each_variant_is_continued_from_its_own_entry() {
        let mut cache = DocumentCache::default();
        let mut source = format!("{SOURCE}public void open() {{\n    int am");
        cache.analyze(&source);
        cache.analyze(&format!("{source}\n}}"));
        for letter in ['o', 'u', 'n', 't'] {
            source.push(letter);
            let closed = format!("{source}\n}}");
            let plain = cache.analyze(&source);
            let with_closer = cache.analyze(&closed);
            assert_eq!(plain.parsed, ActiveDocument::analyze_full(&source).parsed);
            assert_eq!(
                with_closer.parsed,
                ActiveDocument::analyze_full(&closed).parsed
            );
        }
        assert_eq!(cache.entries.len(), CACHED_DOCUMENTS);
    }
}
