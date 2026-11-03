//! Misst, was der UI-Thread pro Tastendruck für eine Completion bezahlt
//! (Text klonen, `submit`, `poll`) und wie lange das Ergebnis braucht.
//!
//! cargo test -p nortex-ide --release --test completion_async -- --ignored --nocapture --test-threads=1

use std::time::{Duration, Instant};

use nortex_ide::editor::completion_worker::{CompletionRequest, CompletionWorker};

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

fn median(mut values: Vec<Duration>) -> Duration {
    values.sort();
    values[values.len() / 2]
}

#[test]
#[ignore = "Messung, mehrere Sekunden"]
fn ui_thread_cost_and_latency() {
    let mut worker = CompletionWorker::spawn(|| {}).expect("worker");
    for lines in [1_000, 10_000, 50_000] {
        let base = large_source(lines);
        let mut ui_cost = Vec::new();
        let mut latency = Vec::new();
        for run in 0..RUNS {
            let text = format!("{base}Shape{run}");
            let cursor = text.chars().count();
            let start = Instant::now();
            let request = CompletionRequest {
                document: 0,
                text: text.clone(),
                cursor,
                path: None,
                project_files: Vec::new(),
                manual: true,
            };
            worker.submit(request);
            let submitted = start.elapsed();
            let polled = Instant::now();
            let _ = worker.poll();
            ui_cost.push(submitted + polled.elapsed());
            while worker.poll().is_none() {
                std::thread::sleep(Duration::from_micros(200));
            }
            latency.push(start.elapsed());
        }
        println!(
            "{lines:>6} Zeilen: UI-Thread {:>8.3} ms, bis Ergebnis {:>8.3} ms",
            median(ui_cost).as_secs_f64() * 1e3,
            median(latency).as_secs_f64() * 1e3,
        );
    }
}
