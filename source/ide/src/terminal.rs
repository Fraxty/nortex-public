//! Integrierte Shell: Prozess mit Pipes (kein ConPTY). Reader-Threads blockieren die GUI nie.

use std::io::{self, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;

pub const MAX_BUFFER_CHARS: usize = 200_000;

pub struct Terminal {
    child: Child,
    stdin: Option<ChildStdin>,
    output: Receiver<String>,
    pub buffer: String,
    pub shell: String,
    alive: bool,
}

fn forward(mut source: impl Read + Send + 'static, sink: mpsc::Sender<String>) {
    thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(read) = source.read(&mut chunk) {
            if read == 0
                || sink
                    .send(String::from_utf8_lossy(&chunk[..read]).into_owned())
                    .is_err()
            {
                break;
            }
        }
    });
}

impl Terminal {
    /// `powershell.exe` mit Fallback auf `cmd.exe` (sonst `sh`).
    pub fn spawn_default() -> io::Result<Self> {
        let candidates: &[(&str, &[&str])] = if cfg!(windows) {
            &[
                (
                    "powershell.exe",
                    &["-NoLogo", "-NoProfile", "-Command", "-"],
                ),
                ("cmd.exe", &["/Q"]),
            ]
        } else {
            &[("sh", &[])]
        };
        let mut last = io::Error::other("keine Shell gefunden");
        for (program, arguments) in candidates {
            match Self::spawn(program, arguments) {
                Ok(terminal) => return Ok(terminal),
                Err(error) => last = error,
            }
        }
        Err(last)
    }

    pub fn spawn(program: &str, arguments: &[&str]) -> io::Result<Self> {
        let mut command = Command::new(program);
        command
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
        let mut child = command.spawn()?;
        let (sender, output) = mpsc::channel();
        if let Some(stdout) = child.stdout.take() {
            forward(stdout, sender.clone());
        }
        if let Some(stderr) = child.stderr.take() {
            forward(stderr, sender);
        }
        Ok(Self {
            stdin: child.stdin.take(),
            child,
            output,
            buffer: String::new(),
            shell: program.to_string(),
            alive: true,
        })
    }

    /// Übernimmt neue Ausgabe und prüft, ob der Prozess noch läuft. Blockiert nie.
    pub fn poll(&mut self) -> bool {
        let mut received = false;
        while let Ok(text) = self.output.try_recv() {
            self.buffer.push_str(&text.replace('\r', ""));
            received = true;
        }
        if self.buffer.chars().count() > MAX_BUFFER_CHARS {
            let excess = self.buffer.chars().count() - MAX_BUFFER_CHARS;
            let cut = self
                .buffer
                .char_indices()
                .nth(excess)
                .map_or(self.buffer.len(), |(index, _)| index);
            self.buffer.drain(..cut);
        }
        if self.alive && !matches!(self.child.try_wait(), Ok(None)) {
            self.alive = false;
            self.stdin = None;
        }
        received
    }

    pub fn is_alive(&self) -> bool {
        self.alive
    }

    pub fn send_line(&mut self, line: &str) -> io::Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::BrokenPipe, "Shell beendet"))?;
        self.buffer.push_str(&format!("> {line}\n"));
        stdin.write_all(format!("{line}\r\n").as_bytes())?;
        stdin.flush()
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
