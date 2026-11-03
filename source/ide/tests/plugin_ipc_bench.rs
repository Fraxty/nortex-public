//! Measurements of the plugin IPC path with real child processes
//! (`examples/plugin_stub.rs` in `bench` mode). Ignored by default:
//!
//! ```text
//! cargo build --release -p nortex-ide --example plugin_stub
//! cargo test -p nortex-ide --release --test plugin_ipc_bench -- --ignored --nocapture --test-threads=1
//! ```
//!
//! Every scenario runs once per isolation level: as a plain child process and
//! inside the AppContainer sandbox (`launch_sandboxed`). A level that cannot be
//! started on the machine is reported as not executed, never filled in.
//!
//! Round-trip times, CPU time and peak working set are taken inside the plugin
//! process (request written until the matching response is read). The host side
//! is measured around `pump`. The plugin's own numbers need a platform
//! implementation in the stub; where there is none the columns show n/a.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nortex_ide::plugins::host::{
    DiagnosticInfo, DocumentInfo, HostLimits, IdeServices, PluginHost, PluginIo, PluginProcess,
    PluginState, launch_sandboxed,
};
use nortex_ide::plugins::protocol::{Frame, PluginManifest};
use nortex_ide::sandbox::{SandboxLimits, SandboxRequest, file_sha256};
use serde_json::json;

const RUNS: usize = 7;

#[derive(Clone, Copy, PartialEq)]
enum Isolation {
    Plain,
    AppContainer,
}

impl Isolation {
    fn label(self) -> &'static str {
        match self {
            Isolation::Plain => "ohne AppContainer (normaler Kindprozess)",
            Isolation::AppContainer => "im AppContainer (launch_sandboxed)",
        }
    }
}

struct StubProcess(Arc<Mutex<Child>>);

impl PluginProcess for StubProcess {
    fn exit_code(&self) -> Option<u32> {
        let status = self.0.lock().unwrap().try_wait().ok().flatten()?;
        Some(status.code().unwrap_or(-1) as u32)
    }

    fn kill(&self) {
        let _ = self.0.lock().unwrap().kill();
    }
}

fn stub_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    exe.parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("plugin_stub{}", std::env::consts::EXE_SUFFIX))
}

fn launch_plain(args: Vec<String>) -> Result<PluginIo, String> {
    let mut child = Command::new(stub_path())
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| error.to_string())?;
    let input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    Ok(PluginIo {
        process: Box::new(StubProcess(Arc::new(Mutex::new(child)))),
        input: Box::new(input),
        output: Box::new(output),
    })
}

/// The AppContainer only reaches files it was granted, so every plugin runs
/// from its own copy of the stub in a private directory.
fn launch_contained(id: &str, dir: PathBuf, args: Vec<String>) -> Result<PluginIo, String> {
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let executable = dir.join(format!("plugin_stub{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(stub_path(), &executable).map_err(|error| error.to_string())?;
    let request = SandboxRequest {
        expected_sha256: Some(file_sha256(&executable).map_err(|error| error.to_string())?),
        executable,
        args,
        working_dir: dir.clone(),
        extension_id: id.to_owned(),
        installed_dir: dir,
        permissions: Vec::new(),
        run_approved: true,
        limits: SandboxLimits {
            timeout: Duration::from_secs(600),
            cpu_time: Duration::from_secs(600),
            memory_bytes: 512 * 1024 * 1024,
            ..SandboxLimits::default()
        },
    };
    launch_sandboxed(&request)
}

struct FakeIde {
    document: String,
}

impl IdeServices for FakeIde {
    fn active_document(&mut self) -> Option<DocumentInfo> {
        Some(DocumentInfo {
            name: "main.nx".into(),
            language: "nortex".into(),
            text: self.document.clone(),
        })
    }

    fn project_files(&mut self) -> Vec<String> {
        Vec::new()
    }

    fn diagnostics(&mut self) -> Vec<DiagnosticInfo> {
        Vec::new()
    }
}

const LOG_ONLY: [&str; 1] = ["ide.log"];
const LOG_AND_DOCUMENT: [&str; 2] = ["ide.log", "ide.document.read"];

#[derive(Clone, Copy)]
struct Scenario {
    label: &'static str,
    plugins: usize,
    method: &'static str,
    count: u64,
    window: u64,
    payload_chars: usize,
    document_bytes: usize,
    /// `None` pumps back to back; `Some` emulates a UI that pumps once per frame.
    pump_interval: Option<Duration>,
}

#[derive(Default, Clone)]
struct Outcome {
    requests_per_second: f64,
    min_us: f64,
    p50_us: f64,
    p95_us: f64,
    p99_us: f64,
    max_us: f64,
    pump_max_us: f64,
    pump_busy_percent: f64,
    cpu_us_per_request: f64,
    peak_kib: f64,
}

fn field(line: &str, name: &str) -> f64 {
    line.split_whitespace()
        .find_map(|part| part.strip_prefix(&format!("{name}=")))
        .unwrap_or_else(|| panic!("{name} missing in {line}"))
        .parse()
        .unwrap()
}

fn run_once(scenario: Scenario, isolation: Isolation) -> Result<Outcome, String> {
    let limits = HostLimits {
        max_requests_per_second: 1_000_000,
        handshake_timeout: Duration::from_secs(10),
        ..HostLimits::default()
    };
    let mut host = PluginHost::new(limits);
    let (permissions, granted): (&[&str], Vec<String>) = if scenario.method == "log" {
        (&LOG_ONLY, vec!["ide.log".into()])
    } else {
        (
            &LOG_AND_DOCUMENT,
            LOG_AND_DOCUMENT.map(String::from).to_vec(),
        )
    };
    let ids: Vec<String> = (0..scenario.plugins)
        .map(|index| format!("bench.p{index}"))
        .collect();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        ^ u128::from(std::process::id());
    let mut dirs = Vec::new();
    for id in &ids {
        let manifest = PluginManifest {
            id: id.clone(),
            ide_api: ">=1.0.0, <2.0.0".into(),
            ide_permissions: permissions.iter().map(|name| name.to_string()).collect(),
        };
        let args = vec![
            "bench".to_owned(),
            scenario.method.to_owned(),
            scenario.count.to_string(),
            scenario.window.to_string(),
            scenario.payload_chars.to_string(),
        ];
        let id = id.clone();
        let dir = std::env::temp_dir().join(format!("nortex-ipc-bench-{nonce:x}-{id}"));
        dirs.push(dir.clone());
        host.start(&manifest, &granted, move || match isolation {
            Isolation::Plain => launch_plain(args),
            Isolation::AppContainer => launch_contained(&id, dir, args),
        })
        .map_err(|error| format!("{error:?}"))?;
    }

    let mut ide = FakeIde {
        document: "x".repeat(scenario.document_bytes),
    };
    let mut pump_total = Duration::ZERO;
    let mut pump_max = Duration::ZERO;
    let started = Instant::now();
    let deadline = started + Duration::from_secs(120);
    let outcome = loop {
        let before = Instant::now();
        host.pump(&mut ide);
        let spent = before.elapsed();
        pump_total += spent;
        pump_max = pump_max.max(spent);

        let reports: Vec<String> = ids
            .iter()
            .filter_map(|id| {
                host.logs(id)
                    .find(|entry| entry.message.starts_with("bench "))
                    .map(|entry| entry.message.clone())
            })
            .collect();
        if reports.len() == ids.len() {
            break Ok(reports);
        }
        if let Some(id) = ids
            .iter()
            .find(|id| host.state(id).is_some_and(|state| state.is_final()))
        {
            break Err(format!("plugin {id} ended early: {}", host.status_text(id)));
        }
        if Instant::now() >= deadline {
            break Err("benchmark timed out".to_owned());
        }
        match scenario.pump_interval {
            Some(interval) => std::thread::sleep(interval),
            None => std::thread::yield_now(),
        }
    };
    let elapsed = started.elapsed();

    for id in &ids {
        host.stop(id);
    }
    let stop_deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < stop_deadline
        && ids
            .iter()
            .any(|id| host.state(id).is_some_and(|state| !state.is_final()))
    {
        host.pump(&mut ide);
        std::thread::sleep(Duration::from_millis(2));
    }
    drop(host);
    for dir in dirs {
        let _ = std::fs::remove_dir_all(dir);
    }

    let reports = outcome?;
    let mean = |name: &str| {
        reports.iter().map(|line| field(line, name)).sum::<f64>() / reports.len() as f64
    };
    let max = |name: &str| {
        reports
            .iter()
            .map(|line| field(line, name))
            .fold(0.0, f64::max)
    };
    let measured = |value: f64| if value < 0.0 { f64::NAN } else { value };
    let total_requests = scenario.count as f64 * scenario.plugins as f64;
    Ok(Outcome {
        requests_per_second: total_requests / (max("total_us") / 1e6),
        min_us: mean("min"),
        p50_us: mean("p50"),
        p95_us: mean("p95"),
        p99_us: mean("p99"),
        max_us: max("max"),
        pump_max_us: pump_max.as_secs_f64() * 1e6,
        pump_busy_percent: pump_total.as_secs_f64() / elapsed.as_secs_f64() * 100.0,
        cpu_us_per_request: measured(mean("cpu_us")) / scenario.count as f64,
        peak_kib: measured(max("peak_kib")),
    })
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    values[values.len() / 2]
}

fn cell(value: f64) -> String {
    if value.is_nan() {
        "n/a".to_owned()
    } else {
        format!("{value:.0}")
    }
}

fn run_scenario(scenario: Scenario, isolation: Isolation) -> Result<(), String> {
    let outcomes = (0..RUNS)
        .map(|_| run_once(scenario, isolation))
        .collect::<Result<Vec<_>, _>>()?;
    let pick = |select: fn(&Outcome) -> f64| median(outcomes.iter().map(select).collect());
    println!(
        "| {} | {} | {} | {} | {} | {} | {} | {} | {:.1} | {} | {} |",
        scenario.label,
        cell(pick(|o| o.requests_per_second)),
        cell(pick(|o| o.min_us)),
        cell(pick(|o| o.p50_us)),
        cell(pick(|o| o.p95_us)),
        cell(pick(|o| o.p99_us)),
        cell(pick(|o| o.max_us)),
        cell(pick(|o| o.pump_max_us)),
        pick(|o| o.pump_busy_percent),
        cell(pick(|o| o.cpu_us_per_request)),
        cell(pick(|o| o.peak_kib)),
    );
    Ok(())
}

fn time_per_call(iterations: u32, mut work: impl FnMut()) -> f64 {
    for _ in 0..iterations / 10 {
        work();
    }
    let started = Instant::now();
    for _ in 0..iterations {
        work();
    }
    started.elapsed().as_secs_f64() * 1e9 / f64::from(iterations)
}

fn codec_costs() {
    println!("\n| Frame (JSON) | Kodieren ns | Parsen + Validieren ns | Zeilenlänge Byte |");
    println!("|---|---:|---:|---:|");
    for (label, request) in [
        (
            "log, 16 Zeichen",
            Frame::request(
                1,
                "log",
                json!({ "level": "info", "message": "x".repeat(16) }),
            ),
        ),
        (
            "log, 2000 Zeichen",
            Frame::request(
                1,
                "log",
                json!({ "level": "info", "message": "x".repeat(2000) }),
            ),
        ),
        (
            "Antwort, 24 KiB Dokument",
            Frame::success(
                1,
                json!({ "name": "main.nx", "language": "nortex", "text": "x".repeat(24 * 1024) }),
            ),
        ),
    ] {
        let line = request.to_line();
        let body = &line[..line.len() - 1];
        let encode = time_per_call(20_000, || {
            std::hint::black_box(request.to_line());
        });
        let parse = time_per_call(20_000, || {
            std::hint::black_box(Frame::parse(body).unwrap());
        });
        println!("| {label} | {encode:.0} | {parse:.0} | {} |", line.len());
    }
}

fn timeout_behaviour() {
    println!("\n| Zeitüberschreitung | konfiguriert | gemessen bis Endzustand |");
    println!("|---|---:|---:|");
    let manifest = PluginManifest {
        id: "bench.timeout".into(),
        ide_api: ">=1.0.0, <2.0.0".into(),
        ide_permissions: vec![],
    };
    let mut ide = FakeIde {
        document: String::new(),
    };

    let limits = HostLimits {
        handshake_timeout: Duration::from_millis(300),
        ..HostLimits::default()
    };
    let mut host = PluginHost::new(limits);
    let started = Instant::now();
    host.start(&manifest, &[], || launch_plain(vec!["silent".into()]))
        .unwrap();
    while !host.state("bench.timeout").unwrap().is_final() {
        host.pump(&mut ide);
        std::thread::sleep(Duration::from_millis(1));
    }
    println!(
        "| Handshake (Plugin schweigt) | 300 ms | {} ms |",
        started.elapsed().as_millis()
    );

    let limits = HostLimits {
        shutdown_grace: Duration::from_millis(300),
        ..HostLimits::default()
    };
    let mut host = PluginHost::new(limits);
    host.start(&manifest, &[], || {
        launch_plain(vec!["ignore_shutdown".into()])
    })
    .unwrap();
    while *host.state("bench.timeout").unwrap() != PluginState::Active {
        host.pump(&mut ide);
        std::thread::sleep(Duration::from_millis(1));
    }
    let started = Instant::now();
    host.stop("bench.timeout");
    while !host.state("bench.timeout").unwrap().is_final() {
        host.pump(&mut ide);
        std::thread::sleep(Duration::from_millis(1));
    }
    println!(
        "| Shutdown (Plugin ignoriert) | 300 ms | {} ms |",
        started.elapsed().as_millis()
    );
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!text.is_empty()).then_some(text)
}

fn describe_environment() {
    println!(
        "- Betriebssystem: {} {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let version = if cfg!(windows) {
        command_output("cmd", &["/C", "ver"])
    } else {
        command_output("uname", &["-sr"])
    };
    println!(
        "- Version: {}",
        version.unwrap_or_else(|| "unbekannt".into())
    );
    let cpu = std::env::var("PROCESSOR_IDENTIFIER").ok().or_else(|| {
        std::fs::read_to_string("/proc/cpuinfo")
            .ok()?
            .lines()
            .find_map(|line| line.strip_prefix("model name"))
            .map(|rest| rest.trim_start_matches([' ', '\t', ':']).to_owned())
    });
    println!("- Prozessor: {}", cpu.unwrap_or_else(|| "unbekannt".into()));
    println!(
        "- Logische Prozessoren: {}",
        std::thread::available_parallelism().map_or(0, usize::from)
    );
    println!(
        "- Build: {}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        }
    );
}

#[test]
#[ignore = "measurement; needs the plugin_stub example built in the same profile"]
fn ipc_benchmarks() {
    assert!(
        stub_path().exists(),
        "build it first: cargo build --release -p nortex-ide --example plugin_stub"
    );
    println!("\nMediane aus {RUNS} Läufen; Zeiten in µs.");
    describe_environment();

    let base = Scenario {
        label: "",
        plugins: 1,
        method: "log",
        count: 2000,
        window: 1,
        payload_chars: 16,
        document_bytes: 0,
        pump_interval: None,
    };
    let scenarios = [
        Scenario {
            label: "klein (16 Z.), nacheinander, Pump ohne Pause",
            ..base
        },
        Scenario {
            label: "groß (1900 Z.), nacheinander, Pump ohne Pause",
            payload_chars: 1900,
            ..base
        },
        Scenario {
            label: "Antwort 24 KiB (document.active), nacheinander",
            method: "document.active",
            count: 1000,
            document_bytes: 24 * 1024,
            ..base
        },
        Scenario {
            label: "klein, 16 gleichzeitig offen",
            window: 16,
            count: 5000,
            ..base
        },
        Scenario {
            label: "klein, 4 Plugins parallel, je nacheinander",
            plugins: 4,
            ..base
        },
        Scenario {
            label: "klein, nacheinander, Pump alle 16 ms (UI-Takt)",
            count: 100,
            pump_interval: Some(Duration::from_millis(16)),
            ..base
        },
        Scenario {
            label: "klein, 16 offen, Pump alle 16 ms (UI-Takt)",
            window: 16,
            count: 320,
            pump_interval: Some(Duration::from_millis(16)),
            ..base
        },
    ];

    for isolation in [Isolation::Plain, Isolation::AppContainer] {
        println!("\n### {}\n", isolation.label());
        println!(
            "| Szenario | Anfragen/s | min | p50 | p95 | p99 | max | pump max | pump Auslastung % | Plugin-CPU µs/Anfrage | Plugin-Spitzenspeicher KiB |"
        );
        println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
        for scenario in scenarios {
            if let Err(reason) = run_scenario(scenario, isolation) {
                println!(
                    "\nNicht ausgeführt: {reason}. Die übrigen Szenarien dieser Stufe wurden übersprungen."
                );
                break;
            }
        }
    }
    codec_costs();
    timeout_behaviour();
}
