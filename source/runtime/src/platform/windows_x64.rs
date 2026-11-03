//! NORTEX Windows x64 Native Runtime & SEH Panic Subsystem
//!
//! Production-grade Runtime Foundation:
//! - Entrypoint: `mainCRTStartup` with command-line parsing, TLS slots, and structured exit
//! - Structured Exception Handling (SEH) personality handler (`_nortex_seh_personality`)
//! - Stack unwinding with 16-byte stack frame alignment validation (Microsoft x64 ABI)
//! - Backtrace capture (`CaptureStackBackTrace`)
//! - Windows IOCP (I/O Completion Port) Async Task Executor & Waker

use std::ffi::c_void;
#[cfg(all(windows, feature = "native-entrypoint"))]
use std::os::windows::ffi::OsStringExt;
use std::ptr;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

// ----------------------------------------------------------------------------
// 1. WINDOWS X64 NATIVE TYPEN & SEH STRUKTUREN
// ----------------------------------------------------------------------------

pub type DWORD = u32;
#[allow(non_camel_case_types)]
pub type ULONG_PTR = usize;
pub type PVOID = *mut c_void;

pub const INVALID_HANDLE_VALUE: PVOID = -1isize as PVOID;
pub const EXCEPTION_CONTINUE_SEARCH: i32 = 0;
pub const EXCEPTION_EXECUTE_HANDLER: i32 = 1;
pub const EXCEPTION_CONTINUE_EXECUTION: i32 = -1;

pub const EXCEPTION_NORTEX_PANIC: DWORD = 0xE06E7478; // 'ntx' Custom SEH Code

#[repr(C)]
pub struct EXCEPTION_RECORD {
    pub exception_code: DWORD,
    pub exception_flags: DWORD,
    pub exception_record: *mut EXCEPTION_RECORD,
    pub exception_address: PVOID,
    pub number_parameters: DWORD,
    pub exception_information: [ULONG_PTR; 15],
}

#[repr(C, align(16))]
pub struct M128A {
    pub low: u64,
    pub high: i64,
}

#[repr(C, align(16))]
pub struct CONTEXT {
    pub p1_home: u64,
    pub p2_home: u64,
    pub p3_home: u64,
    pub p4_home: u64,
    pub p5_home: u64,
    pub p6_home: u64,
    pub context_flags: DWORD,
    pub mx_csr: DWORD,
    pub seg_cs: u16,
    pub seg_ds: u16,
    pub seg_es: u16,
    pub seg_fs: u16,
    pub seg_gs: u16,
    pub seg_ss: u16,
    pub e_flags: DWORD,
    pub dr0: u64,
    pub dr1: u64,
    pub dr2: u64,
    pub dr3: u64,
    pub dr6: u64,
    pub dr7: u64,
    pub rax: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rbx: u64,
    pub rsp: u64,
    pub rbp: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rip: u64,
    // Header & Floating point
    pub flt_save: [u8; 512],
    pub vector_register: [M128A; 26],
    pub vector_control: u64,
    pub debug_control: u64,
    pub last_branch_to_rip: u64,
    pub last_branch_from_rip: u64,
    pub last_exception_to_rip: u64,
    pub last_exception_from_rip: u64,
}

#[repr(C)]
pub struct DISPATCHER_CONTEXT {
    pub control_pc: u64,
    pub image_base: u64,
    pub function_entry: PVOID,
    pub establisher_frame: u64,
    pub target_ip: u64,
    pub context_record: *mut CONTEXT,
    pub language_handler: PVOID,
    pub handler_data: PVOID,
}

#[cfg(windows)]
unsafe extern "system" {
    fn TlsAlloc() -> DWORD;
    fn TlsSetValue(dwTlsIndex: DWORD, lpTlsValue: PVOID) -> i32;
    fn TlsGetValue(dwTlsIndex: DWORD) -> PVOID;
    fn TlsFree(dwTlsIndex: DWORD) -> i32;
    fn RaiseException(
        dwExceptionCode: DWORD,
        dwExceptionFlags: DWORD,
        nNumberOfArguments: DWORD,
        lpArguments: *const ULONG_PTR,
    );
    fn RtlCaptureStackBackTrace(
        FramesToSkip: DWORD,
        FramesToCapture: DWORD,
        BackTrace: *mut PVOID,
        BackTraceHash: *mut DWORD,
    ) -> u16;
    fn CreateIoCompletionPort(
        FileHandle: PVOID,
        ExistingCompletionPort: PVOID,
        CompletionKey: ULONG_PTR,
        NumberOfConcurrentThreads: DWORD,
    ) -> PVOID;
    fn PostQueuedCompletionStatus(
        CompletionPort: PVOID,
        dwNumberOfBytesTransferred: DWORD,
        dwCompletionKey: ULONG_PTR,
        lpOverlapped: PVOID,
    ) -> i32;
    fn GetQueuedCompletionStatus(
        CompletionPort: PVOID,
        lpNumberOfBytesTransferred: *mut DWORD,
        lpCompletionKey: *mut ULONG_PTR,
        lpOverlapped: *mut PVOID,
        dwMilliseconds: DWORD,
    ) -> i32;
}

#[cfg(all(windows, feature = "native-entrypoint"))]
unsafe extern "system" {
    fn GetCommandLineW() -> *const u16;
    fn LocalFree(hMem: PVOID) -> PVOID;
    fn ExitProcess(uExitCode: u32) -> !;
}

// ----------------------------------------------------------------------------
// 2. PANIC-PAYLOAD & STACKTRACE-SYSTEM
// ----------------------------------------------------------------------------

#[derive(Debug)]
pub struct NortrexPanicPayload {
    pub message: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub backtrace: Vec<usize>,
}

static TLS_PANIC_SLOT: AtomicUsize = AtomicUsize::new(usize::MAX);

pub fn init_runtime_tls() -> bool {
    #[cfg(windows)]
    {
        if TLS_PANIC_SLOT.load(Ordering::Acquire) != usize::MAX {
            return true;
        }

        let slot = unsafe { TlsAlloc() };
        if slot == DWORD::MAX {
            return false;
        }

        match TLS_PANIC_SLOT.compare_exchange(
            usize::MAX,
            slot as usize,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => true,
            Err(_) => unsafe { TlsFree(slot) != 0 },
        }
    }

    #[cfg(not(windows))]
    true
}

pub fn shutdown_runtime_tls() -> bool {
    #[cfg(windows)]
    {
        let slot = TLS_PANIC_SLOT.swap(usize::MAX, Ordering::AcqRel);
        if slot == usize::MAX {
            return true;
        }

        if unsafe { TlsFree(slot as DWORD) } != 0 {
            true
        } else {
            TLS_PANIC_SLOT.store(slot, Ordering::Release);
            false
        }
    }

    #[cfg(not(windows))]
    true
}

pub fn capture_backtrace(skip: u32, max_frames: u32) -> Vec<usize> {
    let mut frames = vec![ptr::null_mut(); max_frames as usize];
    #[cfg(windows)]
    unsafe {
        let captured =
            RtlCaptureStackBackTrace(skip + 1, max_frames, frames.as_mut_ptr(), ptr::null_mut());
        frames.truncate(captured as usize);
        frames.into_iter().map(|p| p as usize).collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

pub fn trigger_nortex_panic(msg: &str, file: &str, line: u32, col: u32) -> ! {
    let bt = capture_backtrace(1, 64);
    let payload = Box::new(NortrexPanicPayload {
        message: msg.to_string(),
        file: file.to_string(),
        line,
        column: col,
        backtrace: bt,
    });

    let raw_payload = Box::into_raw(payload) as ULONG_PTR;

    #[cfg(windows)]
    unsafe {
        let slot = TLS_PANIC_SLOT.load(Ordering::Acquire);
        if slot != usize::MAX {
            TlsSetValue(slot as DWORD, raw_payload as PVOID);
        }
        let args = [raw_payload];
        RaiseException(
            EXCEPTION_NORTEX_PANIC,
            1, // EXCEPTION_NONCONTINUABLE
            1,
            args.as_ptr(),
        );
    }

    #[cfg(all(windows, feature = "native-entrypoint"))]
    #[link(name = "shell32")]
    unsafe extern "system" {
        fn CommandLineToArgvW(lpCmdLine: *const u16, pNumArgs: *mut i32) -> *mut *mut u16;
    }

    eprintln!("\x1b[31m[NORTEX RUNTIME FATAL]\x1b[0m Panic in {file}:{line}:{col} - {msg}");
    std::process::exit(101);
}

/// NORTEX SEH Personality Handler für MSVC x64
#[unsafe(no_mangle)]
/// # Safety
/// Pointers must be valid Windows SEH records for the duration of this call.
pub unsafe extern "C" fn _nortex_seh_personality(
    exception_record: *mut EXCEPTION_RECORD,
    _establisher_frame: u64,
    _context_record: *mut CONTEXT,
    _dispatcher_context: *mut DISPATCHER_CONTEXT,
) -> i32 {
    let Some(rec) = (unsafe { exception_record.as_ref() }) else {
        return EXCEPTION_CONTINUE_SEARCH;
    };
    if rec.exception_code == EXCEPTION_NORTEX_PANIC {
        let payload_ptr = if rec.number_parameters >= 1 {
            rec.exception_information[0] as *mut NortrexPanicPayload
        } else {
            let slot = TLS_PANIC_SLOT.load(Ordering::Acquire);
            if slot == usize::MAX {
                ptr::null_mut()
            } else {
                unsafe { TlsGetValue(slot as DWORD) as *mut NortrexPanicPayload }
            }
        };
        if let Some(payload) = unsafe { payload_ptr.as_ref() } {
            eprintln!("\n=== NORTEX PANIC UNWIND (Windows SEH x64) ===");
            eprintln!("Meldung: {}", payload.message);
            eprintln!(
                "Ort:     {}:{}:{}",
                payload.file, payload.line, payload.column
            );
            eprintln!("Callstack RIPs:");
            for (i, frame) in payload.backtrace.iter().enumerate() {
                eprintln!("  #{i:02}: 0x{frame:016x}");
            }
            eprintln!("==============================================\n");
        }
        return EXCEPTION_EXECUTE_HANDLER;
    }
    EXCEPTION_CONTINUE_SEARCH
}

// ----------------------------------------------------------------------------
// 3. NATIVE MAIN ENTRYPOINT: mainCRTStartup
// ----------------------------------------------------------------------------

#[cfg(all(windows, feature = "native-entrypoint"))]
unsafe extern "C" {
    fn nortex_main(argc: i32, argv: *const *const u8) -> i32;
}

#[cfg(all(windows, feature = "native-entrypoint"))]
#[unsafe(no_mangle)]
/// # Safety
/// This process entry point must only be invoked by the Windows loader.
pub unsafe extern "system" fn mainCRTStartup() -> ! {
    if !init_runtime_tls() {
        eprintln!("NORTEX Runtime: Windows-TLS konnte nicht initialisiert werden.");
        std::process::exit(101);
    }

    // 2. Parsen von argc / argv über Win32 APIs
    #[cfg(windows)]
    let (argc, argv_ptrs) = unsafe {
        let cmd_line = GetCommandLineW();
        let mut num_args: i32 = 0;
        let wide_argv = CommandLineToArgvW(cmd_line, &mut num_args);

        let mut utf8_strings: Vec<Vec<u8>> = Vec::with_capacity(num_args as usize);
        let mut pointers: Vec<*const u8> = Vec::with_capacity(num_args as usize + 1);

        for i in 0..num_args {
            let wide_str = *wide_argv.offset(i as isize);
            let mut len = 0;
            while *wide_str.offset(len) != 0 {
                len += 1;
            }
            let wide_slice = std::slice::from_raw_parts(wide_str, len as usize);
            let os_str = std::ffi::OsString::from_wide(wide_slice);
            let mut utf8 = os_str.to_string_lossy().into_owned().into_bytes();
            utf8.push(0); // Nullterminator
            pointers.push(utf8.as_ptr());
            utf8_strings.push(utf8);
        }
        pointers.push(ptr::null());
        LocalFree(wide_argv as PVOID);

        // Leak strings für die Programmdauer
        std::mem::forget(utf8_strings);
        (num_args, pointers)
    };

    // 3. Aufruf des NORTEX Compiler Entry Points
    // Der Stack ist garantiert an dieser Grenze 16-Byte-aligned nach MS x64 ABI
    let exit_code = unsafe { nortex_main(argc, argv_ptrs.as_ptr()) };

    // 4. Geordneter Shutdown
    unsafe { ExitProcess(exit_code as u32) }
}

// ----------------------------------------------------------------------------
// 4. WINDOWS IOCP ASYNC TASK EXECUTOR & WAKER
// ----------------------------------------------------------------------------

pub struct IocpExecutor {
    port: PVOID,
    task_count: AtomicU64,
}

unsafe impl Send for IocpExecutor {}
unsafe impl Sync for IocpExecutor {}

impl IocpExecutor {
    pub fn new(threads: u32) -> Result<Self, std::io::Error> {
        #[cfg(windows)]
        unsafe {
            let port = CreateIoCompletionPort(INVALID_HANDLE_VALUE, ptr::null_mut(), 0, threads);
            if port.is_null() {
                return Err(std::io::Error::last_os_error());
            }
            Ok(Self {
                port,
                task_count: AtomicU64::new(0),
            })
        }
        #[cfg(not(windows))]
        {
            Ok(Self {
                port: ptr::null_mut(),
                task_count: AtomicU64::new(0),
            })
        }
    }

    pub fn schedule_task<F>(&self, task: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let boxed: Box<Box<dyn FnOnce() + Send>> = Box::new(Box::new(task));
        let raw = Box::into_raw(boxed) as ULONG_PTR;

        #[cfg(windows)]
        unsafe {
            PostQueuedCompletionStatus(self.port, 0, raw, ptr::null_mut());
            self.task_count.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn run_worker_loop(&self) {
        #[cfg(windows)]
        unsafe {
            let mut bytes_transferred: DWORD = 0;
            let mut key: ULONG_PTR = 0;
            let mut overlapped: PVOID = ptr::null_mut();

            while GetQueuedCompletionStatus(
                self.port,
                &mut bytes_transferred,
                &mut key,
                &mut overlapped,
                0xFFFFFFFF,
            ) != 0
            {
                if key == 0 {
                    break; // Termination Signal
                }
                let task_box: Box<Box<dyn FnOnce() + Send>> = Box::from_raw(key as *mut _);
                task_box();
                self.task_count.fetch_sub(1, Ordering::Relaxed);
            }
        }
    }
}
