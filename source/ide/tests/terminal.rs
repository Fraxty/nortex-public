use std::time::{Duration, Instant};

use nortex_ide::terminal::{MAX_BUFFER_CHARS, Terminal};

fn wait_for(terminal: &mut Terminal, needle: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        terminal.poll();
        if terminal.buffer.contains(needle) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[cfg(windows)]
#[test]
fn shell_round_trip_and_exit_detection() {
    let mut terminal = Terminal::spawn("cmd.exe", &["/Q"]).unwrap();
    assert!(terminal.is_alive());
    terminal.send_line("echo nx_marker_42").unwrap();
    assert!(wait_for(&mut terminal, "nx_marker_42"));
    terminal.send_line("exit").unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    while terminal.is_alive() && Instant::now() < deadline {
        terminal.poll();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!terminal.is_alive());
    assert!(terminal.send_line("echo x").is_err());
}

#[cfg(windows)]
#[test]
fn clear_empties_buffer_and_buffer_is_capped() {
    let mut terminal = Terminal::spawn("cmd.exe", &["/Q"]).unwrap();
    terminal.buffer = "x".repeat(MAX_BUFFER_CHARS + 500);
    terminal.poll();
    assert!(terminal.buffer.chars().count() <= MAX_BUFFER_CHARS);
    terminal.clear();
    assert!(terminal.buffer.is_empty());
}

#[test]
fn missing_program_is_an_error_not_a_panic() {
    assert!(Terminal::spawn("nortex-no-such-shell-xyz", &[]).is_err());
}
