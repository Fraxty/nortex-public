//! Reproduzierbare IntelliSense-Messungen (nur Standardbibliothek).
//!
//! Aufruf (Release-Build, ein Thread, damit sich Messungen nicht stören):
//! `cargo test -p nortex-ide --release --test intellisense_bench -- --ignored --nocapture --test-threads=1`
//! Die Anzahl der Durchläufe steuert `NORTEX_BENCH_RUNS` (Standard 15).

use std::alloc::{GlobalAlloc, Layout, System};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use nortex_compiler::lexer::Lexer;
use nortex_compiler::parser::Parser;
use nortex_ide::editor::completion::Completion;

struct CountingAllocator;

static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            let live = LIVE_BYTES.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK_BYTES.fetch_max(live, Ordering::Relaxed);
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let moved = unsafe { System.realloc(pointer, layout, new_size) };
        if !moved.is_null() {
            if new_size >= layout.size() {
                let live = LIVE_BYTES.fetch_add(new_size - layout.size(), Ordering::Relaxed)
                    + new_size
                    - layout.size();
                PEAK_BYTES.fetch_max(live, Ordering::Relaxed);
            } else {
                LIVE_BYTES.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        moved
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

struct Measurement {
    name: String,
    input: String,
    times: Vec<Duration>,
    peak_extra_bytes: usize,
}

fn runs() -> usize {
    std::env::var("NORTEX_BENCH_RUNS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|runs| *runs > 0)
        .unwrap_or(15)
}

/// Führt `prepare` (nicht gemessen) und danach `work` (gemessen) je Durchlauf aus.
fn measure<S, R>(
    name: &str,
    input: &str,
    mut prepare: impl FnMut() -> S,
    mut work: impl FnMut(S) -> R,
) -> Measurement {
    let mut times = Vec::new();
    let mut peak_extra_bytes = 0;
    for _ in 0..runs() {
        let state = prepare();
        let baseline = LIVE_BYTES.load(Ordering::Relaxed);
        PEAK_BYTES.store(baseline, Ordering::Relaxed);
        let started = Instant::now();
        let output = work(state);
        times.push(started.elapsed());
        peak_extra_bytes = peak_extra_bytes.max(PEAK_BYTES.load(Ordering::Relaxed) - baseline);
        drop(output);
    }
    times.sort();
    Measurement {
        name: name.to_owned(),
        input: input.to_owned(),
        times,
        peak_extra_bytes,
    }
}

fn print_header() {
    println!("| Messung | Eingabe | Läufe | Min | Median | Mittel | Max | Peak-Heap |");
    println!("|---|---|---:|---:|---:|---:|---:|---:|");
}

fn print_row(measurement: &Measurement) {
    let times = &measurement.times;
    let mean = times.iter().sum::<Duration>() / times.len() as u32;
    println!(
        "| {} | {} | {} | {} | {} | {} | {} | {} |",
        measurement.name,
        measurement.input,
        times.len(),
        format_duration(times[0]),
        format_duration(times[times.len() / 2]),
        format_duration(mean),
        format_duration(times[times.len() - 1]),
        format_bytes(measurement.peak_extra_bytes),
    );
}

fn format_duration(duration: Duration) -> String {
    let micros = duration.as_secs_f64() * 1e6;
    if micros >= 1000.0 {
        format!("{:.2} ms", micros / 1000.0)
    } else {
        format!("{micros:.1} µs")
    }
}

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1 << 20 {
        format!("{:.2} MiB", bytes as f64 / f64::from(1 << 20))
    } else {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    }
}

fn module_source(index: usize, imports: &[usize]) -> String {
    let mut source = format!("namespace Bench.M{index};\n");
    for import in imports {
        source.push_str(&format!("using m{import};\n"));
    }
    source.push_str(&format!(
        "public class Item{index} {{\n    public int value;\n    public string label;\n    private int hidden;\n    public int total(int extra) {{ return extra; }}\n}}\npublic Item{index} make{index}(int seed) {{\n    Item{index} item = new Item{index}();\n    return item;\n}}\n"
    ));
    source
}

/// Legt `count` Module in einem frischen Ordner an. `chain`: Modul i importiert i-1,
/// sonst importiert die Hauptdatei alle direkt.
fn write_project(label: &str, count: usize, chain: bool) -> (PathBuf, String) {
    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "nx_bench_{label}_{}_{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).unwrap();
    for index in 0..count {
        let imports: Vec<usize> = if chain && index > 0 {
            vec![index - 1]
        } else {
            Vec::new()
        };
        fs::write(
            dir.join(format!("m{index}.nx")),
            module_source(index, &imports),
        )
        .unwrap();
    }
    let last = count - 1;
    let mut main = String::new();
    if chain {
        main.push_str(&format!("using m{last};\n"));
    } else {
        for index in 0..count {
            main.push_str(&format!("using m{index};\n"));
        }
    }
    main.push_str(&format!(
        "public void run() {{\n    var item = make{last}(1);\n    item."
    ));
    fs::write(dir.join("main.nx"), &main).unwrap();
    (dir, main)
}

fn complete_at_end(dir: &Path, source: &str) -> usize {
    let path = dir.join("main.nx");
    let items = Completion::manual_for_file(source, source.chars().count(), Some(&path), &[])
        .map_or(0, |completion| completion.items.len());
    assert_eq!(items, 3, "erwartet: value, label, total");
    items
}

fn large_source(lines: usize) -> String {
    let mut source = String::new();
    for index in 0..lines / 4 {
        source.push_str(&format!(
            "public class Shape{index} {{\n    public int width;\n    public int area(int scale) {{ return scale; }}\n}}\n"
        ));
    }
    source
}

#[test]
#[ignore = "Messung; siehe Kopfkommentar"]
fn intellisense_benchmarks() {
    let mut rows = Vec::new();

    for lines in [1_000, 10_000, 50_000] {
        let source = large_source(lines);
        let input = format!("{lines} Zeilen, {} KiB Quelltext", source.len() / 1024);
        rows.push(measure(
            "Lexer (voll)",
            &input,
            || (),
            |()| Lexer::new(&source).tokenize().tokens.len(),
        ));
        let tokens = Lexer::new(&source).tokenize().tokens;
        rows.push(measure(
            "Parser (voll, ohne Lexen)",
            &input,
            || tokens.clone(),
            |tokens| Parser::new(tokens).parse().program.declarations.len(),
        ));
    }

    let broken: String = (0..2_000)
        .map(|line| format!("int value{line} = ;\nclass {{ public int\n"))
        .collect();
    rows.push(measure(
        "Lexer + Parser, viele Syntaxfehler",
        "4000 Zeilen, je zwei defekte Deklarationen",
        || (),
        |()| {
            let output = Lexer::new(&broken).tokenize();
            Parser::new(output.tokens).parse().diagnostics.len()
        },
    ));

    let local =
        "int counter = 1;\nint total = counter;\npublic void run() {\n    int amount = 2;\n    am";
    rows.push(measure(
        "Completion, lokale Symbole ohne Datei",
        "5 Zeilen",
        || (),
        |()| {
            Completion::open(local, local.chars().count())
                .map_or(0, |completion| completion.items.len())
        },
    ));

    let large = format!(
        "{}public void run() {{\n    Shape10 shape = new Shape10();\n    shape.",
        large_source(10_000)
    );
    rows.push(measure(
        "Completion Member nach Punkt, aktive Datei",
        "10000 Zeilen, 2500 Klassen, ohne Imports",
        || (),
        |()| {
            Completion::manual(&large, large.chars().count())
                .map_or(0, |completion| completion.items.len())
        },
    ));

    for (count, chain, label) in [
        (1, true, "1 Modul direkt"),
        (10, true, "10 Module transitiv (Kette)"),
        (10, false, "10 Module direkt"),
        (100, true, "100 Module transitiv (Kette)"),
        (100, false, "100 Module direkt"),
        (400, true, "400 Module transitiv (Kette)"),
    ] {
        let input = format!("{label}, je Modul 1 Klasse + 1 Funktion");
        rows.push(measure(
            "Completion Member, Imports kalt",
            &input,
            || write_project("cold", count, chain),
            |(dir, main)| {
                let items = complete_at_end(&dir, &main);
                let _ = fs::remove_dir_all(&dir);
                items
            },
        ));

        let (dir, main) = write_project("warm", count, chain);
        complete_at_end(&dir, &main);
        rows.push(measure(
            "Completion Member, Imports warm (Cache)",
            &input,
            || (),
            |()| complete_at_end(&dir, &main),
        ));
        let _ = fs::remove_dir_all(&dir);
    }

    // Tippen in einer großen Datei: jede Messung hängt ein Zeichen an,
    // wodurch Lexer und Parser die inkrementellen Pfade nutzen können.
    let base = large_source(10_000);
    let mut typed = base.clone();
    typed.push_str("public void run() {\n    int amount = 1;\n    am");
    rows.push(measure(
        "Completion pro Tastendruck, aktive Datei",
        "10000 Zeilen, Zeichen am Dateiende anhängen",
        || {
            typed.push('o');
            typed.clone()
        },
        |text| {
            Completion::open(&text, text.chars().count())
                .map_or(0, |completion| completion.items.len())
        },
    ));

    print_header();
    for row in &rows {
        print_row(row);
    }
}
