//! Kosten der Completion-Anfrage je Phase (Lexen, Parsen, Importe, Symbole/Typen).
//!
//! Aufruf (Release-Build, ein Thread):
//! `cargo test -p nortex-ide --release --test completion_phases -- --ignored --nocapture --test-threads=1`
//!
//! Die Phasenzähler sind prozessweit; der Test enthält deshalb nur eine Messfunktion.

use std::time::{Duration, Instant};

use nortex_ide::editor::analysis::{self, Counter, Phase};
use nortex_ide::editor::completion::Completion;

const RUNS: usize = 15;

fn large_source(lines: usize) -> String {
    let mut source = String::new();
    for index in 0..lines / 4 {
        source.push_str(&format!(
            "public class Shape{index} {{\n    public int width;\n    public int area(int scale) {{ return scale; }}\n}}\n"
        ));
    }
    source
}

fn request(text: &str) -> usize {
    Completion::open(text, text.chars().count()).map_or(0, |completion| completion.items.len())
}

#[derive(Default)]
struct Sample {
    wall: Duration,
    lex_full: u64,
    lex_incremental: u64,
    parse_full: u64,
    parse_incremental: u64,
    symbols: u64,
    request: u64,
    cache_hits: u64,
    cache_misses: u64,
    fallbacks: u64,
}

fn median<T: Ord + Copy>(mut values: Vec<T>) -> T {
    values.sort();
    values[values.len() / 2]
}

/// `texts` sind aufeinanderfolgende Anfragen; vor jedem Durchlauf wird der
/// Analyse-Cache mit einem fremden Text überschrieben.
fn scenario(name: &str, texts: &[String], warm_up_with_first: bool) {
    let mut samples = Vec::new();
    for _ in 0..RUNS {
        request("int unrelated = 1;\nint other = unrelated;\nun");
        let start = usize::from(warm_up_with_first);
        if warm_up_with_first {
            request(&texts[0]);
        }
        analysis::reset();
        let started = Instant::now();
        for text in &texts[start..] {
            std::hint::black_box(request(text));
        }
        let wall = started.elapsed();
        let stats = analysis::snapshot();
        samples.push(Sample {
            wall,
            lex_full: stats.phase(Phase::LexFull).nanos,
            lex_incremental: stats.phase(Phase::LexIncremental).nanos,
            parse_full: stats.phase(Phase::ParseFull).nanos,
            parse_incremental: stats.phase(Phase::ParseIncremental).nanos,
            symbols: stats.phase(Phase::SymbolsAndTypes).nanos,
            request: stats.phase(Phase::Request).nanos,
            cache_hits: stats.counter(Counter::ActiveCacheHit),
            cache_misses: stats.counter(Counter::ActiveCacheMiss),
            fallbacks: stats.counter(Counter::IncrementalFallback),
        });
    }
    let requests = (texts.len() - usize::from(warm_up_with_first)) as f64;
    let ms = |nanos: u64| nanos as f64 / 1e6 / requests;
    let pick = |select: fn(&Sample) -> u64| ms(median(samples.iter().map(select).collect()));
    println!(
        "| {name} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {} / {} / {} |",
        median(samples.iter().map(|s| s.wall).collect()).as_secs_f64() * 1e3 / requests,
        pick(|s| s.lex_full + s.lex_incremental),
        pick(|s| s.parse_full + s.parse_incremental),
        pick(|s| s.symbols),
        pick(|s| s.request),
        pick(|s| s.lex_full),
        pick(|s| s.parse_full),
        median(samples.iter().map(|s| s.cache_hits).collect()),
        median(samples.iter().map(|s| s.cache_misses).collect()),
        median(samples.iter().map(|s| s.fallbacks).collect()),
    );
}

#[test]
#[ignore = "Messung; siehe Kopfkommentar"]
fn completion_phase_costs() {
    println!("\nMedian aus {RUNS} Läufen, Zeiten in ms je Anfrage.\n");
    println!(
        "| Szenario | Wand | Lexen | Parsen | Symbole/Typen | Request (innen) | davon Lex voll | davon Parse voll | Cache Treffer / Fehlgriffe / Fallbacks (je Lauf) |"
    );
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|");

    for lines in [1_000, 10_000, 50_000] {
        let base = large_source(lines);
        let tail = "public void run() {\n    int amount = 1;\n    am";
        let first = format!("{base}{tail}");

        scenario(
            &format!("{lines} Z.: erste Anfrage (Cache fremd)"),
            std::slice::from_ref(&first),
            false,
        );
        scenario(
            &format!("{lines} Z.: unveränderter Text, wiederholt"),
            &vec![first.clone(); 6],
            true,
        );
        let mut typing = Vec::new();
        let mut text = first.clone();
        typing.push(text.clone());
        for letter in "ountxyzuv".chars() {
            text.push(letter);
            typing.push(text.clone());
        }
        scenario(
            &format!("{lines} Z.: 9 Tastendrücke am Dateiende"),
            &typing,
            true,
        );
        let mut middle = Vec::new();
        let mut text = first.clone();
        middle.push(text.clone());
        for _ in 0..9 {
            text.insert(base.len() / 2, 'x');
            middle.push(text.clone());
        }
        scenario(
            &format!("{lines} Z.: 9 Tastendrücke in der Dateimitte"),
            &middle,
            true,
        );
    }
}
