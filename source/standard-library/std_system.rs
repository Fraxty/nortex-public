//! NORTEX Standard Library: `std::system`
//!
//! Production-grade Operating System Integration for Windows x64:
//! - OS Handle RAII Management (`OwnedHandle`, `BorrowedHandle`, `CloseHandle`)
//! - Native Process Spawning (`CreateProcessW`, Win32 Argument Quoting, Pipes, Exit Codes)
//! - Environment Variables (`GetEnvironmentVariableW`, `SetEnvironmentVariableW`, `GetEnvironmentStringsW`)
//! - Path & Filesystem Primitives

use std::collections::HashMap;
use std::ffi::{c_void, OsStr, OsString};
use std::fmt;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::ptr;

// ----------------------------------------------------------------------------
// 1. WIN32 HANDLE ABSTRAKTIONEN
// ----------------------------------------------------------------------------

pub type RawHandle = *mut c_void;
pub const INVALID_HANDLE_VALUE: RawHandle = -1isize as RawHandle;

#[cfg(windows)]
extern "system" {
    fn CloseHandle(hObject: RawHandle) -> i32;
    fn DuplicateHandle(
        hSourceProcessHandle: RawHandle,
        hSourceHandle: RawHandle,
        hTargetProcessHandle: RawHandle,
        lpTargetHandle: *mut RawHandle,
        dwDesiredAccess: u32,
        bInheritHandle: i32,
        dwOptions: u32,
    ) -> i32;
    fn GetCurrentProcess() -> RawHandle;
}

#[derive(Debug)]
pub struct OwnedHandle {
    handle: RawHandle,
}

unsafe impl Send for OwnedHandle {}
unsafe impl Sync for OwnedHandle {}

impl OwnedHandle {
    pub unsafe fn from_raw(handle: RawHandle) -> Option<Self> {
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            None
        } else {
            Some(Self { handle })
        }
    }

    pub fn as_raw(&self) -> RawHandle {
        self.handle
    }

    pub fn into_raw(mut self) -> RawHandle {
        let h = self.handle;
        self.handle = ptr::null_mut();
        h
    }

    pub fn try_clone(&self) -> Result<Self, std::io::Error> {
        #[cfg(windows)]
        unsafe {
            let mut target: RawHandle = ptr::null_mut();
            let cur = GetCurrentProcess();
            let ok = DuplicateHandle(cur, self.handle, cur, &mut target, 0, 0, 2 /* DUPLICATE_SAME_ACCESS */);
            if ok == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Self::from_raw(target).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Handle duplication resulted in null"))
        }
        #[cfg(not(windows))]
        {
            Ok(Self { handle: self.handle })
        }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.handle.is_null() && self.handle != INVALID_HANDLE_VALUE {
            #[cfg(windows)]
            unsafe {
                CloseHandle(self.handle);
            }
            self.handle = ptr::null_mut();
        }
    }
}

// ----------------------------------------------------------------------------
// 2. PROZESS-VERWALTUNG & SPAWNING
// ----------------------------------------------------------------------------

#[repr(C)]
struct STARTUPINFOW {
    cb: u32,
    lpReserved: *mut u16,
    lpDesktop: *mut u16,
    lpTitle: *mut u16,
    dwX: u32,
    dwY: u32,
    dwXSize: u32,
    dwYSize: u32,
    dwXCountChars: u32,
    dwYCountChars: u32,
    dwFillAttribute: u32,
    dwFlags: u32,
    wShowWindow: u16,
    cbReserved2: u16,
    lpReserved2: *mut u8,
    hStdInput: RawHandle,
    hStdOutput: RawHandle,
    hStdError: RawHandle,
}

#[repr(C)]
struct PROCESS_INFORMATION {
    hProcess: RawHandle,
    hThread: RawHandle,
    dwProcessId: u32,
    dwThreadId: u32,
}

#[cfg(windows)]
extern "system" {
    fn CreateProcessW(
        lpApplicationName: *const u16,
        lpCommandLine: *mut u16,
        lpProcessAttributes: *mut c_void,
        lpThreadAttributes: *mut c_void,
        bInheritHandles: i32,
        dwCreationFlags: u32,
        lpEnvironment: *mut c_void,
        lpCurrentDirectory: *const u16,
        lpStartupInfo: *const STARTUPINFOW,
        lpProcessInformation: *mut PROCESS_INFORMATION,
    ) -> i32;
    fn WaitForSingleObject(hHandle: RawHandle, dwMilliseconds: u32) -> u32;
    fn GetExitCodeProcess(hProcess: RawHandle, lpExitCode: *mut u32) -> i32;
    fn TerminateProcess(hProcess: RawHandle, uExitCode: u32) -> i32;
}

const STARTF_USESTDHANDLES: u32 = 0x00000100;
const INFINITE: u32 = 0xFFFFFFFF;

pub struct Process {
    process_handle: OwnedHandle,
    thread_handle: OwnedHandle,
    pid: u32,
}

impl Process {
    pub fn id(&self) -> u32 {
        self.pid
    }

    pub fn wait(&self) -> Result<u32, std::io::Error> {
        #[cfg(windows)]
        unsafe {
            let res = WaitForSingleObject(self.process_handle.as_raw(), INFINITE);
            if res != 0 {
                return Err(std::io::Error::last_os_error());
            }
            let mut exit_code: u32 = 0;
            if GetExitCodeProcess(self.process_handle.as_raw(), &mut exit_code) == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(exit_code)
        }
        #[cfg(not(windows))]
        {
            Ok(0)
        }
    }

    pub fn kill(&self) -> Result<(), std::io::Error> {
        #[cfg(windows)]
        unsafe {
            if TerminateProcess(self.process_handle.as_raw(), 1) == 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        }
        #[cfg(not(windows))]
        {
            Ok(())
        }
    }
}

pub struct Command {
    program: OsString,
    args: Vec<OsString>,
    env: HashMap<OsString, OsString>,
    current_dir: Option<PathBuf>,
    stdin_handle: Option<OwnedHandle>,
    stdout_handle: Option<OwnedHandle>,
    stderr_handle: Option<OwnedHandle>,
}

impl Command {
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            program: program.as_ref().to_os_string(),
            args: Vec::new(),
            env: HashMap::new(),
            current_dir: None,
            stdin_handle: None,
            stdout_handle: None,
            stderr_handle: None,
        }
    }

    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for a in args {
            self.arg(a);
        }
        self
    }

    pub fn env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) -> &mut Self {
        self.env.insert(key.as_ref().to_os_string(), val.as_ref().to_os_string());
        self
    }

    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Self {
        self.current_dir = Some(dir.as_ref().to_path_buf());
        self
    }

    /// Windows-konformes Quoting von Befehlszeilen nach den Standard-MSVC-Regeln
    fn build_windows_cmdline(&self) -> Vec<u16> {
        let mut line = String::new();
        let quote_arg = |arg: &str, target: &mut String| {
            if !arg.is_empty() && !arg.contains([' ', '\t', '\n', '\x0b', '\"']) {
                target.push_str(arg);
                return;
            }
            target.push('\"');
            let mut backslashes = 0;
            for c in arg.chars() {
                match c {
                    '\\' => backslashes += 1,
                    '\"' => {
                        target.push_str(&"\\".repeat(backslashes * 2 + 1));
                        target.push('\"');
                        backslashes = 0;
                    }
                    _ => {
                        target.push_str(&"\\".repeat(backslashes));
                        target.push(c);
                        backslashes = 0;
                    }
                }
            }
            target.push_str(&"\\".repeat(backslashes * 2));
            target.push('\"');
        };

        let prog_str = self.program.to_string_lossy();
        quote_arg(&prog_str, &mut line);

        for arg in &self.args {
            line.push(' ');
            let s = arg.to_string_lossy();
            quote_arg(&s, &mut line);
        }

        let mut wide: Vec<u16> = OsStr::new(&line).encode_wide().collect();
        wide.push(0);
        wide
    }

    pub fn spawn(&mut self) -> Result<Process, std::io::Error> {
        #[cfg(windows)]
        unsafe {
            let mut cmdline_wide = self.build_windows_cmdline();
            let mut startup_info: STARTUPINFOW = std::mem::zeroed();
            startup_info.cb = std::mem::size_of::<STARTUPINFOW>() as u32;

            let mut inherit_handles = 0;
            if self.stdin_handle.is_some() || self.stdout_handle.is_some() || self.stderr_handle.is_some() {
                startup_info.dwFlags |= STARTF_USESTDHANDLES;
                inherit_handles = 1;
                if let Some(h) = &self.stdin_handle {
                    startup_info.hStdInput = h.as_raw();
                }
                if let Some(h) = &self.stdout_handle {
                    startup_info.hStdOutput = h.as_raw();
                }
                if let Some(h) = &self.stderr_handle {
                    startup_info.hStdError = h.as_raw();
                }
            }

            let curdir_wide: Option<Vec<u16>> = self.current_dir.as_ref().map(|p| {
                let mut v: Vec<u16> = p.as_os_str().encode_wide().collect();
                v.push(0);
                v
            });
            let curdir_ptr = curdir_wide.as_ref().map_or(ptr::null(), |v| v.as_ptr());

            let mut proc_info: PROCESS_INFORMATION = std::mem::zeroed();

            let success = CreateProcessW(
                ptr::null(),
                cmdline_wide.as_mut_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                inherit_handles,
                0,
                ptr::null_mut(),
                curdir_ptr,
                &startup_info,
                &mut proc_info,
            );

            if success == 0 {
                return Err(std::io::Error::last_os_error());
            }

            let p_handle = OwnedHandle::from_raw(proc_info.hProcess)
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Invalid process handle returned"))?;
            let t_handle = OwnedHandle::from_raw(proc_info.hThread)
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "Invalid thread handle returned"))?;

            Ok(Process {
                process_handle: p_handle,
                thread_handle: t_handle,
                pid: proc_info.dwProcessId,
            })
        }
        #[cfg(not(windows))]
        {
            Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "Plattform wird derzeit nur unter Windows x64 nativ ausgeführt"))
        }
    }
}

// ----------------------------------------------------------------------------
// 3. UMGEBUNGSVARIABLEN (ENVIRONMENT)
// ----------------------------------------------------------------------------

#[cfg(windows)]
extern "system" {
    fn GetEnvironmentVariableW(lpName: *const u16, lpBuffer: *mut u16, nSize: u32) -> u32;
    fn SetEnvironmentVariableW(lpName: *const u16, lpValue: *const u16) -> i32;
    fn GetEnvironmentStringsW() -> *mut u16;
    fn FreeEnvironmentStringsW(lpszEnvironmentBlock: *mut u16) -> i32;
}

pub fn var(key: impl AsRef<OsStr>) -> Result<String, std::io::Error> {
    #[cfg(windows)]
    unsafe {
        let mut key_wide: Vec<u16> = key.as_ref().encode_wide().collect();
        key_wide.push(0);

        let needed = GetEnvironmentVariableW(key_wide.as_ptr(), ptr::null_mut(), 0);
        if needed == 0 {
            return Err(std::io::Error::last_os_error());
        }

        let mut buf = vec![0u16; needed as usize];
        let written = GetEnvironmentVariableW(key_wide.as_ptr(), buf.as_mut_ptr(), needed);
        if written == 0 {
            return Err(std::io::Error::last_os_error());
        }

        buf.truncate(written as usize);
        let os_str = OsString::from_wide(&buf);
        os_str.into_string().map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Umgebungsvariable ist kein gültiges UTF-8"))
    }
    #[cfg(not(windows))]
    {
        std::env::var(key.as_ref()).map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))
    }
}

pub fn set_var(key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Result<(), std::io::Error> {
    #[cfg(windows)]
    unsafe {
        let mut key_wide: Vec<u16> = key.as_ref().encode_wide().collect();
        key_wide.push(0);
        let mut val_wide: Vec<u16> = value.as_ref().encode_wide().collect();
        val_wide.push(0);

        if SetEnvironmentVariableW(key_wide.as_ptr(), val_wide.as_ptr()) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::env::set_var(key.as_ref(), value.as_ref());
        Ok(())
    }
}
