//! Test plugin for the IPC host. The first argument selects the behaviour:
//! `good`, `incompatible`, `silent`, `crash`, `garbage`, `oversize`, `flood`,
//! `noperm`, `ignore_shutdown`, `bench`, `actions`, `badactions`, `export`.

use std::io::{BufRead, Write};

use serde_json::{Value, json};

fn send(value: Value) {
    let mut out = std::io::stdout().lock();
    writeln!(out, "{value}").unwrap();
    out.flush().unwrap();
}

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({ "protocol": 1, "id": id, "type": "request", "method": method, "params": params })
}

fn read_frame(lines: &mut impl Iterator<Item = std::io::Result<String>>) -> Value {
    serde_json::from_str(&lines.next().unwrap().unwrap()).unwrap()
}

fn wait_for_shutdown(lines: &mut impl Iterator<Item = std::io::Result<String>>) {
    for line in lines {
        let Ok(line) = line else { return };
        let frame: Value = serde_json::from_str(&line).unwrap();
        if frame["method"] == "shutdown" {
            send(json!({ "protocol": 1, "id": frame["id"], "type": "response", "result": {} }));
            return;
        }
    }
}

/// CPU time (user + kernel, µs) and peak working set (KiB) of this process.
#[cfg(windows)]
fn process_usage() -> Option<(u64, u64)> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::ProcessStatus::{
        K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    let ticks =
        |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    let zero = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
    let mut memory: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    unsafe {
        let process = GetCurrentProcess();
        if GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) == 0 {
            return None;
        }
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        if K32GetProcessMemoryInfo(process, &mut memory, size) == 0 {
            return None;
        }
    }
    // FILETIME counts 100 ns.
    Some((
        (ticks(kernel) + ticks(user)) / 10,
        memory.PeakWorkingSetSize as u64 / 1024,
    ))
}

#[cfg(target_os = "linux")]
fn process_usage() -> Option<(u64, u64)> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // Fields after the parenthesised name; utime and stime are the 12th and 13th.
    let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
    let ticks: u64 = fields.get(11)?.parse::<u64>().ok()? + fields.get(12)?.parse::<u64>().ok()?;
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let peak = status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))?
        .split_whitespace()
        .next()?
        .parse()
        .ok()?;
    // USER_HZ is 100 on every common Linux configuration.
    Some((ticks * 10_000, peak))
}

#[cfg(not(any(windows, target_os = "linux")))]
fn process_usage() -> Option<(u64, u64)> {
    None
}

/// `bench <method> <count> <window> <payload_chars>`: keeps `window` requests in
/// flight until `count` have been answered and reports the round-trip times
/// and the process's own CPU and memory use as one log line.
fn bench(lines: &mut impl Iterator<Item = std::io::Result<String>>) {
    let arg = |position: usize| std::env::args().nth(position).unwrap();
    let method = arg(2);
    let count: u64 = arg(3).parse().unwrap();
    let window: u64 = arg(4).parse().unwrap();
    let payload = "x".repeat(arg(5).parse().unwrap());
    let params = |method: &str| match method {
        "log" => json!({ "level": "info", "message": payload }),
        _ => json!({}),
    };

    let mut sent_at = std::collections::HashMap::new();
    let mut round_trips_us = Vec::with_capacity(count as usize);
    let mut next_id = 1;
    let started = std::time::Instant::now();
    let send_request =
        |id: u64, sent_at: &mut std::collections::HashMap<u64, std::time::Instant>| {
            sent_at.insert(id, std::time::Instant::now());
            send(request(id, &method, params(&method)));
        };
    while next_id <= window.min(count) {
        send_request(next_id, &mut sent_at);
        next_id += 1;
    }
    while (round_trips_us.len() as u64) < count {
        let reply = read_frame(lines);
        let sent = sent_at.remove(&reply["id"].as_u64().unwrap()).unwrap();
        round_trips_us.push(sent.elapsed().as_micros() as u64);
        if next_id <= count {
            send_request(next_id, &mut sent_at);
            next_id += 1;
        }
    }
    let total_us = started.elapsed().as_micros();

    round_trips_us.sort_unstable();
    let at =
        |fraction: f64| round_trips_us[((round_trips_us.len() - 1) as f64 * fraction) as usize];
    let (cpu_us, peak_kib) =
        process_usage().map_or((-1, -1), |(cpu, peak)| (cpu as i64, peak as i64));
    send(request(
        next_id,
        "log",
        json!({ "level": "info", "message": format!(
            "bench count={count} total_us={total_us} min={} p50={} p95={} p99={} max={} cpu_us={cpu_us} peak_kib={peak_kib}",
            at(0.0), at(0.5), at(0.95), at(0.99), at(1.0)
        ) }),
    ));
    wait_for_shutdown(lines);
}

/// Answers `action.invoke` (`hello` succeeds, `fail` errors, `slow` is ignored)
/// until the host asks for shutdown.
fn serve_actions(lines: &mut impl Iterator<Item = std::io::Result<String>>) {
    for line in lines {
        let Ok(line) = line else { return };
        let frame: Value = serde_json::from_str(&line).unwrap();
        let reply = |body: Value| {
            let mut value = json!({ "protocol": 1, "id": frame["id"], "type": "response" });
            value
                .as_object_mut()
                .unwrap()
                .extend(body.as_object().unwrap().clone());
            send(value);
        };
        match frame["method"].as_str() {
            Some("shutdown") => {
                reply(json!({ "result": {} }));
                return;
            }
            Some("action.invoke") => match frame["params"]["action"].as_str() {
                Some("hello") => reply(json!({ "result": { "message": "hello from plugin" } })),
                Some("fail") => {
                    reply(json!({ "error": { "code": "unavailable", "message": "nope" } }))
                }
                _ => {}
            },
            _ => {}
        }
    }
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "good".into());
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();

    let init = read_frame(&mut lines);
    let id = init["id"].as_u64().unwrap();
    let half_minute = std::time::Duration::from_secs(30);

    match mode.as_str() {
        "silent" => {
            std::thread::sleep(half_minute);
            return;
        }
        "crash" => std::process::exit(3),
        "garbage" => {
            println!("this is not json");
            std::thread::sleep(half_minute);
            return;
        }
        "oversize" => {
            let padding = "x".repeat(200_000);
            send(
                json!({ "protocol": 1, "id": id, "type": "response", "result": { "pad": padding } }),
            );
            std::thread::sleep(half_minute);
            return;
        }
        "incompatible" => {
            send(json!({
                "protocol": 1, "id": id, "type": "response",
                "result": { "ide_api": ">=2.0.0", "name": "stub", "version": "1.0.0" }
            }));
            std::thread::sleep(half_minute);
            return;
        }
        "badactions" => {
            send(json!({
                "protocol": 1, "id": id, "type": "response",
                "result": { "ide_api": ">=1.0.0, <2.0.0", "actions": [{ "id": "Bad Id", "title": "x" }] }
            }));
            std::thread::sleep(half_minute);
            return;
        }
        "actions" => {
            send(json!({
                "protocol": 1, "id": id, "type": "response",
                "result": { "ide_api": ">=1.0.0, <2.0.0", "actions": [
                    { "id": "hello", "title": "Say hello" },
                    { "id": "fail", "title": "Always fails" },
                    { "id": "slow", "title": "Never answers" }
                ] }
            }));
            serve_actions(&mut lines);
            return;
        }
        _ => {}
    }

    send(json!({
        "protocol": 1, "id": id, "type": "response",
        "result": { "ide_api": ">=1.0.0, <2.0.0", "name": "stub", "version": "1.0.0" }
    }));

    match mode.as_str() {
        "bench" => bench(&mut lines),
        "flood" => {
            for n in 1..=1000 {
                send(request(
                    n,
                    "log",
                    json!({ "level": "info", "message": "flood" }),
                ));
            }
            std::thread::sleep(half_minute);
        }
        "noperm" => {
            send(request(1, "document.active", json!({})));
            let reply = read_frame(&mut lines);
            send(request(
                2,
                "log",
                json!({ "level": "info", "message": reply["error"]["code"] }),
            ));
            wait_for_shutdown(&mut lines);
        }
        "export" => {
            let calls = [
                (
                    "archive.create",
                    json!({ "output": "a.nxpack", "files": ["main.nx"], "include_active_document": true }),
                ),
                (
                    "archive.create",
                    json!({ "output": "b.nxpack", "files": "main.nx" }),
                ),
                (
                    "archive.create",
                    json!({ "output": "../c.nxpack", "files": ["main.nx"] }),
                ),
                ("document.export_pdf", json!({ "output": "a.pdf" })),
                ("document.export_pdf", json!({})),
            ];
            let mut outcomes = Vec::new();
            for (n, (method, params)) in calls.into_iter().enumerate() {
                send(request(n as u64 + 1, method, params));
                let reply = read_frame(&mut lines);
                outcomes.push(reply["error"]["code"].as_str().unwrap_or("ok").to_owned());
            }
            send(request(
                9,
                "log",
                json!({ "level": "info", "message": outcomes.join(",") }),
            ));
            wait_for_shutdown(&mut lines);
        }
        "ignore_shutdown" => std::thread::sleep(half_minute),
        _ => {
            send(request(
                1,
                "log",
                json!({ "level": "info", "message": "hello" }),
            ));
            send(request(2, "status.set", json!({ "text": "ready" })));
            send(request(3, "document.active", json!({})));
            send(request(4, "project.files", json!({})));
            send(request(5, "diagnostics.active", json!({})));
            send(request(6, "no.such.method", json!({})));
            let outcomes: Vec<String> = (0..6)
                .map(|_| {
                    let reply = read_frame(&mut lines);
                    reply["error"]["code"].as_str().unwrap_or("ok").to_owned()
                })
                .collect();
            send(request(
                7,
                "log",
                json!({ "level": "info", "message": outcomes.join(",") }),
            ));
            wait_for_shutdown(&mut lines);
        }
    }
}
