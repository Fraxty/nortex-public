//! Prozessstart, Argumentübernahme und Panic-Behandlung der NORTEX-Runtime.
//!
//! Der optionale Windows-Einstiegspunkt `mainCRTStartup` ruft den kompilierten
//! Programmeinstieg mit `argc` und `argv` auf. Die Runtime stellt außerdem
//! `nortex_runtime_start(argc, argv, user_main)` als C-ABI-Funktion bereit.
//! Rust-Panics dürfen nie über Windows-x64-Frames des generierten Codes hinweg
//! ungeordnet unwinden: Sie werden hier abgefangen und in einen Exit-Code umgesetzt.
//! Das Unwinding *durch* generierte Frames (SEH-Tabellen, `.pdata`/`.xdata`)
//! ist Aufgabe des Backends und noch nicht Teil dieser Datei.

use std::backtrace::Backtrace;
use std::ffi::{CStr, c_char, c_int};
use std::io::Write;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Once, OnceLock};

/// Exit-Code nach einem nicht abgefangenen NORTEX-Panic (wie Rust).
pub const EXIT_PANIC: i32 = 101;

static ARGUMENTS: OnceLock<Vec<String>> = OnceLock::new();
static PANIC_HOOK: Once = Once::new();
static PROCESS_INIT: Once = Once::new();

/// Panic-Nutzlast für von NORTEX-Code ausgelöste Panics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NortexPanic {
    pub message: String,
}

#[cfg(windows)]
mod os {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn SetErrorMode(mode: u32) -> u32;
        pub fn SetConsoleOutputCP(code_page: u32) -> i32;
    }
}

/// Einmalige Prozessinitialisierung: Panic-Hook, keine Fehlerdialoge, UTF-8-Konsole.
pub fn initialize_process() {
    PROCESS_INIT.call_once(|| {
        install_panic_hook();
        #[cfg(windows)]
        {
            const SEM_FAILCRITICALERRORS: u32 = 0x0001;
            const SEM_NOGPFAULTERRORBOX: u32 = 0x0002;
            const CP_UTF8: u32 = 65001;
            // SAFETY: Reine Prozesseinstellungen ohne Zeigerargumente.
            unsafe {
                os::SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
                os::SetConsoleOutputCP(CP_UTF8);
            }
        }
    });
}

fn payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(panic) = payload.downcast_ref::<NortexPanic>() {
        panic.message.clone()
    } else if let Some(text) = payload.downcast_ref::<&str>() {
        (*text).to_owned()
    } else if let Some(text) = payload.downcast_ref::<String>() {
        text.clone()
    } else {
        "Panic mit unbekannter Nutzlast".to_owned()
    }
}

/// Backtrace-Steuerung: `NORTEX_BACKTRACE=1` (oder `full`) aktiviert sie.
fn backtrace_requested() -> bool {
    std::env::var("NORTEX_BACKTRACE").is_ok_and(|value| !value.is_empty() && value != "0")
}

pub fn install_panic_hook() {
    PANIC_HOOK.call_once(|| {
        panic::set_hook(Box::new(|info| {
            let message = payload_message(info.payload());
            let location = info
                .location()
                .map(|location| {
                    format!(
                        "{}:{}:{}",
                        location.file(),
                        location.line(),
                        location.column()
                    )
                })
                .unwrap_or_else(|| "<unbekannt>".to_owned());
            let mut report = format!("NORTEX-Panic: {message}\n  bei {location}\n");
            if backtrace_requested() {
                report.push_str(&format!("{}\n", Backtrace::force_capture()));
            } else {
                report.push_str("  (NORTEX_BACKTRACE=1 setzen für einen Backtrace)\n");
            }
            let mut stderr = std::io::stderr().lock();
            let _ = stderr.write_all(report.as_bytes());
            let _ = stderr.flush();
        }));
    });
}

/// Übernimmt `argc`/`argv` des C-Einstiegs. Ungültiges UTF-8 wird ersetzt.
///
/// # Safety
/// `argv` muss null sein oder auf mindestens `argc` Zeiger zeigen, die jeweils
/// null oder auf nullterminierte Strings zeigen.
pub unsafe fn collect_arguments(argc: c_int, argv: *const *const c_char) -> Vec<String> {
    if argc <= 0 || argv.is_null() {
        return Vec::new();
    }
    let mut arguments = Vec::with_capacity(argc as usize);
    for index in 0..argc as usize {
        // SAFETY: Nach Vertrag liegen `argc` Zeiger ab `argv`.
        let pointer = unsafe { *argv.add(index) };
        if pointer.is_null() {
            break;
        }
        // SAFETY: Nach Vertrag nullterminiert.
        arguments.push(
            unsafe { CStr::from_ptr(pointer) }
                .to_string_lossy()
                .into_owned(),
        );
    }
    arguments
}

/// Führt `user_main` aus und setzt Panics in [`EXIT_PANIC`] um.
#[allow(clippy::redundant_closure)]
pub fn run_main(user_main: extern "C-unwind" fn() -> i32) -> i32 {
    install_panic_hook();
    match panic::catch_unwind(AssertUnwindSafe(|| user_main())) {
        Ok(code) => code,
        Err(_) => EXIT_PANIC,
    }
}

pub fn arguments() -> &'static [String] {
    ARGUMENTS.get().map(Vec::as_slice).unwrap_or(&[])
}

/// Einstiegspunkt für den vom Linker erzeugten `mainCRTStartup`-Stub.
///
/// # Safety
/// Siehe [`collect_arguments`]; `user_main` muss gültig sein oder null.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_runtime_start(
    argc: c_int,
    argv: *const *const c_char,
    user_main: Option<extern "C-unwind" fn() -> i32>,
) -> i32 {
    initialize_process();
    // SAFETY: Vertrag wird an den Aufrufer weitergereicht.
    let collected = unsafe { collect_arguments(argc, argv) };
    let _ = ARGUMENTS.set(collected);
    let code = match user_main {
        Some(main) => run_main(main),
        None => {
            eprintln!("NORTEX: Kein Programm-Einstiegspunkt übergeben.");
            EXIT_PANIC
        }
    };
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    code
}

#[unsafe(no_mangle)]
pub extern "C" fn nortex_runtime_argc() -> usize {
    arguments().len()
}

/// Liefert Zeiger und Länge (UTF-8, nicht nullterminiert) des `index`-ten
/// Arguments; gültig bis Prozessende. Bei ungültigem Index: Nullzeiger, Länge 0.
///
/// # Safety
/// `length` muss null oder auf beschreibbaren `usize`-Speicher zeigen.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_runtime_argv(index: usize, length: *mut usize) -> *const u8 {
    let (pointer, len) = match arguments().get(index) {
        Some(argument) => (argument.as_ptr(), argument.len()),
        None => (std::ptr::null(), 0),
    };
    if !length.is_null() {
        // SAFETY: Nach Vertrag beschreibbar.
        unsafe { *length = len };
    }
    pointer
}

/// Löst einen NORTEX-Panic aus. Darf von generiertem Code aufgerufen werden.
///
/// # Safety
/// `message` muss null oder auf `length` gültige Bytes zeigen.
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn nortex_panic(message: *const u8, length: usize) -> ! {
    let text = if message.is_null() || length == 0 {
        String::from("explizit ausgelöster Panic")
    } else {
        // SAFETY: Nach Vertrag `length` lesbare Bytes.
        String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(message, length) }).into_owned()
    };
    panic::panic_any(NortexPanic { message: text })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    extern "C-unwind" fn returns_seven() -> i32 {
        7
    }
    extern "C-unwind" fn panics() -> i32 {
        panic::panic_any(NortexPanic {
            message: "boom".into(),
        })
    }

    #[test]
    fn exit_code_is_forwarded() {
        assert_eq!(run_main(returns_seven), 7);
    }

    #[test]
    fn panic_becomes_exit_code_101() {
        assert_eq!(run_main(panics), EXIT_PANIC);
    }

    #[test]
    fn arguments_are_collected_and_null_terminated_list_is_respected() {
        let first = CString::new("prog.exe").unwrap();
        let second = CString::new("ärger").unwrap();
        let argv = [first.as_ptr(), second.as_ptr(), std::ptr::null()];
        let collected = unsafe { collect_arguments(3, argv.as_ptr()) };
        assert_eq!(collected, vec!["prog.exe".to_owned(), "ärger".to_owned()]);
        assert!(unsafe { collect_arguments(0, argv.as_ptr()) }.is_empty());
        assert!(unsafe { collect_arguments(2, std::ptr::null()) }.is_empty());
    }

    #[test]
    fn payload_messages_are_extracted() {
        assert_eq!(
            payload_message(&NortexPanic {
                message: "a".into()
            }),
            "a"
        );
        assert_eq!(payload_message(&"b"), "b");
        assert_eq!(payload_message(&String::from("c")), "c");
        assert_eq!(payload_message(&42u8), "Panic mit unbekannter Nutzlast");
    }
}
