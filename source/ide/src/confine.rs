//! Runs trusted tooling (the NORTEX compiler) with a restricted token, under
//! resource limits.
//!
//! What this enforces on Windows:
//! * The process is created suspended, put into a job object and only then
//!   resumed, so it never runs outside the job. If the assignment fails the
//!   process is terminated without having executed an instruction.
//! * The job limits the process count, committed memory and CPU time, lowers
//!   the priority, and kills every member on timeout, overrun or IDE exit.
//! * The token has no privileges and a Low integrity level. Low integrity
//!   cannot write to objects at Medium integrity (the user's files); it can
//!   only write below directories prepared with [`create_workspace`].
//!
//! What it does not enforce: reading is unrestricted (the compiler must read
//! the MSVC toolchain) and so is network access. Neither a job object nor an
//! integrity level blocks sockets.

use std::io;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ConfineLimits {
    pub timeout: Duration,
    pub cpu_time: Duration,
    pub memory_bytes: usize,
    /// The compiler plus the linker and the helpers the linker starts.
    pub max_processes: u32,
    pub max_output_bytes: usize,
}

impl Default for ConfineLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            cpu_time: Duration::from_secs(120),
            memory_bytes: 2 * 1024 * 1024 * 1024,
            max_processes: 16,
            max_output_bytes: 64 * 1024,
        }
    }
}

#[derive(Debug)]
pub struct ConfinedOutput {
    pub exit_code: Option<i32>,
    pub output: String,
    pub timed_out: bool,
}

#[cfg(not(windows))]
pub fn run_confined(_command: &mut Command, _limits: &ConfineLimits) -> io::Result<ConfinedOutput> {
    Err(unsupported())
}

#[cfg(not(windows))]
pub fn create_workspace(_dir: &Path) -> io::Result<()> {
    Err(unsupported())
}

#[cfg(not(windows))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "confined process execution is only implemented on Windows",
    )
}

/// Creates `dir` as the only place a confined process may write to: it is
/// labelled Low integrity, and files created below it inherit the label.
#[cfg(windows)]
pub fn create_workspace(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    win::label_low_integrity(dir)
}

#[cfg(windows)]
pub fn run_confined(command: &mut Command, limits: &ConfineLimits) -> io::Result<ConfinedOutput> {
    run_confined_with(command, limits, win::Job::assign)
}

/// `assign` puts the still suspended process into the job. It is a parameter
/// so tests can make it fail or observe the suspended state.
#[cfg(windows)]
fn run_confined_with(
    command: &mut Command,
    limits: &ConfineLimits,
    assign: impl FnOnce(&win::Job, win::RawHandle) -> io::Result<()>,
) -> io::Result<ConfinedOutput> {
    use std::io::Read;
    use std::time::Instant;

    let job = win::Job::new(limits)?;
    let mut child = win::spawn_suspended(command)?;
    if let Err(error) = assign(&job, child.raw()).and_then(|()| child.resume()) {
        child.kill();
        return Err(error);
    }

    let cap = limits.max_output_bytes as u64;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let stdout_reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.by_ref().take(cap).read_to_string(&mut text);
        let _ = io::copy(&mut stdout, &mut io::sink());
        text
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.by_ref().take(cap).read_to_string(&mut text);
        let _ = io::copy(&mut stderr, &mut io::sink());
        text
    });

    let deadline = Instant::now() + limits.timeout;
    let mut timed_out = false;
    while !child.wait(Duration::from_millis(10)) {
        if !timed_out && Instant::now() >= deadline {
            timed_out = true;
            job.terminate();
        }
    }
    // Helpers that outlive the main process would keep the pipes open.
    job.terminate();

    let mut output = stdout_reader.join().unwrap_or_default();
    output.push_str(&stderr_reader.join().unwrap_or_default());
    Ok(ConfinedOutput {
        exit_code: child.exit_code().map(|code| code as i32),
        output,
        timed_out,
    })
}

#[cfg(windows)]
mod win {
    use super::ConfineLimits;
    use crate::sandbox::win::{
        AttrList, Handle, PROC_THREAD_ATTRIBUTE_HANDLE_LIST, make_pipe, open_nul, wide,
    };
    use crate::sandbox::{SandboxError, build_command_line};
    use std::collections::BTreeMap;
    use std::ffi::{OsString, c_void};
    use std::fs::File;
    use std::io;
    use std::os::windows::ffi::OsStrExt;
    use std::path::Path;
    use std::process::Command;
    use std::ptr::{null, null_mut};
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, LocalFree, WAIT_OBJECT_0};
    use windows_sys::Win32::Security::Authorization::*;
    use windows_sys::Win32::Security::*;
    use windows_sys::Win32::System::JobObjects::*;
    use windows_sys::Win32::System::Threading::*;

    pub type RawHandle = HANDLE;

    const SE_GROUP_INTEGRITY: u32 = 0x20;
    const LOW_INTEGRITY_SID: &str = "S-1-16-4096";
    const LOW_INTEGRITY_LABEL: &str = "S:(ML;OICI;NW;;;LW)";

    fn io_error(error: SandboxError) -> io::Error {
        io::Error::other(error.to_string())
    }

    /// Closing the handle kills all members (`KILL_ON_JOB_CLOSE`).
    pub struct Job(HANDLE);

    impl Job {
        pub fn new(limits: &ConfineLimits) -> io::Result<Self> {
            let handle = unsafe { CreateJobObjectW(null(), null()) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = Self(handle);

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_ACTIVE_PROCESS
                | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_JOB_TIME
                | JOB_OBJECT_LIMIT_JOB_MEMORY
                | JOB_OBJECT_LIMIT_PRIORITY_CLASS
                | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
            info.BasicLimitInformation.ActiveProcessLimit = limits.max_processes;
            info.BasicLimitInformation.PerJobUserTimeLimit =
                i64::try_from(limits.cpu_time.as_nanos() / 100).unwrap_or(i64::MAX);
            info.BasicLimitInformation.PriorityClass = BELOW_NORMAL_PRIORITY_CLASS;
            info.JobMemoryLimit = limits.memory_bytes;
            let ok = unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const c_void,
                    std::mem::size_of_val(&info) as u32,
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }

            // No desktop, clipboard, global atom or handle access from the tree.
            let ui = JOBOBJECT_BASIC_UI_RESTRICTIONS {
                UIRestrictionsClass: 0xFF,
            };
            let ok = unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectBasicUIRestrictions,
                    &ui as *const _ as *const c_void,
                    std::mem::size_of_val(&ui) as u32,
                )
            };
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(job)
        }

        pub fn assign(&self, process: RawHandle) -> io::Result<()> {
            if unsafe { AssignProcessToJobObject(self.0, process) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        pub fn terminate(&self) {
            unsafe { TerminateJobObject(self.0, 1) };
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    /// A created but not yet resumed process with the parent's ends of its pipes.
    pub struct Suspended {
        process: Handle,
        thread: Handle,
        pub stdout: Option<File>,
        pub stderr: Option<File>,
    }

    impl Suspended {
        pub fn raw(&self) -> RawHandle {
            self.process.0
        }

        pub fn resume(&self) -> io::Result<()> {
            if unsafe { ResumeThread(self.thread.0) } == u32::MAX {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        }

        /// Ends a process that never got to run, or whose job assignment failed.
        pub fn kill(&self) {
            unsafe {
                TerminateProcess(self.process.0, 1);
                WaitForSingleObject(self.process.0, 5000);
            }
        }

        /// Returns true once the process has exited.
        pub fn wait(&self, timeout: Duration) -> bool {
            let millis = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
            unsafe { WaitForSingleObject(self.process.0, millis) == WAIT_OBJECT_0 }
        }

        pub fn exit_code(&self) -> Option<u32> {
            let mut code = 0u32;
            (unsafe { GetExitCodeProcess(self.process.0, &mut code) } != 0).then_some(code)
        }
    }

    /// Creates the process from `command` with the Low integrity token, suspended.
    /// Only the three standard handles are inherited.
    pub fn spawn_suspended(command: &Command) -> io::Result<Suspended> {
        let token = low_integrity_token()?;
        let args: Vec<String> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let command_line = build_command_line(&command.get_program().to_string_lossy(), &args);
        let mut command_line: Vec<u16> = command_line
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let environment = environment_block(command);
        let current_dir = command.get_current_dir().map(|dir| wide(dir.as_os_str()));

        let (stdout_read, stdout_write) = make_pipe().map_err(io_error)?;
        let (stderr_read, stderr_write) = make_pipe().map_err(io_error)?;
        let stdin = open_nul().map_err(io_error)?;
        let inherited: [HANDLE; 3] = [stdin.0, stdout_write.0, stderr_write.0];
        let mut attributes = AttrList::new(1).map_err(io_error)?;
        unsafe {
            attributes
                .set(
                    PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                    inherited.as_ptr() as *const c_void,
                    std::mem::size_of_val(&inherited),
                )
                .map_err(io_error)?;
        }

        let mut startup: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
        startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = stdin.0;
        startup.StartupInfo.hStdOutput = stdout_write.0;
        startup.StartupInfo.hStdError = stderr_write.0;
        startup.lpAttributeList = attributes.ptr();

        let mut created: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        let ok = unsafe {
            CreateProcessAsUserW(
                token.0,
                null(),
                command_line.as_mut_ptr(),
                null(),
                null(),
                1,
                EXTENDED_STARTUPINFO_PRESENT
                    | CREATE_SUSPENDED
                    | CREATE_UNICODE_ENVIRONMENT
                    | CREATE_NO_WINDOW,
                environment.as_ptr() as *const c_void,
                current_dir.as_ref().map_or(null(), |dir| dir.as_ptr()),
                &startup.StartupInfo,
                &mut created,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        // The child owns its copies; ours must go so the readers see EOF.
        Ok(Suspended {
            process: Handle(created.hProcess),
            thread: Handle(created.hThread),
            stdout: Some(stdout_read.into_file()),
            stderr: Some(stderr_read.into_file()),
        })
    }

    fn environment_block(command: &Command) -> Vec<u16> {
        let mut variables: BTreeMap<String, (OsString, OsString)> = std::env::vars_os()
            .map(|(name, value)| (name.to_string_lossy().to_uppercase(), (name, value)))
            .collect();
        for (name, value) in command.get_envs() {
            let key = name.to_string_lossy().to_uppercase();
            match value {
                Some(value) => variables.insert(key, (name.to_owned(), value.to_owned())),
                None => variables.remove(&key),
            };
        }
        let mut block = Vec::new();
        for (name, value) in variables.values() {
            block.extend(name.encode_wide());
            block.push(u16::from(b'='));
            block.extend(value.encode_wide());
            block.push(0);
        }
        block.push(0);
        block
    }

    /// A copy of the IDE's token without privileges, at Low integrity.
    fn low_integrity_token() -> io::Result<Handle> {
        let mut own: HANDLE = null_mut();
        let ok = unsafe {
            OpenProcessToken(
                GetCurrentProcess(),
                TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY,
                &mut own,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let own = Handle(own);
        let mut restricted: HANDLE = null_mut();
        let ok = unsafe {
            CreateRestrictedToken(
                own.0,
                DISABLE_MAX_PRIVILEGE,
                0,
                null(),
                0,
                null(),
                0,
                null(),
                &mut restricted,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let restricted = Handle(restricted);
        let mut primary: HANDLE = null_mut();
        let ok = unsafe {
            DuplicateTokenEx(
                restricted.0,
                0x0200_0000, // MAXIMUM_ALLOWED
                null(),
                SecurityImpersonation,
                TokenPrimary,
                &mut primary,
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let restricted = Handle(primary);
        let mut sid: *mut c_void = null_mut();
        let name = wide(std::ffi::OsStr::new(LOW_INTEGRITY_SID));
        if unsafe { ConvertStringSidToSidW(name.as_ptr(), &mut sid) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let label = TOKEN_MANDATORY_LABEL {
            Label: SID_AND_ATTRIBUTES {
                Sid: sid,
                Attributes: SE_GROUP_INTEGRITY,
            },
        };
        let size =
            std::mem::size_of::<TOKEN_MANDATORY_LABEL>() as u32 + unsafe { GetLengthSid(sid) };
        let ok = unsafe {
            SetTokenInformation(
                restricted.0,
                TokenIntegrityLevel,
                &label as *const _ as *const c_void,
                size,
            )
        };
        let result = if ok == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(restricted)
        };
        unsafe { LocalFree(sid) };
        result
    }

    pub fn label_low_integrity(dir: &Path) -> io::Result<()> {
        let sddl = wide(std::ffi::OsStr::new(LOW_INTEGRITY_LABEL));
        let mut descriptor: PSECURITY_DESCRIPTOR = null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        let mut present = 0;
        let mut defaulted = 0;
        let mut sacl: *mut ACL = null_mut();
        let result = if unsafe {
            GetSecurityDescriptorSacl(descriptor, &mut present, &mut sacl, &mut defaulted)
        } == 0
        {
            Err(io::Error::last_os_error())
        } else {
            let path = wide(dir.as_os_str());
            let code = unsafe {
                SetNamedSecurityInfoW(
                    path.as_ptr() as _,
                    SE_FILE_OBJECT,
                    LABEL_SECURITY_INFORMATION,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    sacl,
                )
            };
            if code == 0 {
                Ok(())
            } else {
                Err(io::Error::from_raw_os_error(code as i32))
            }
        };
        unsafe { LocalFree(descriptor) };
        result
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::time::Instant;

    fn shell(script: &str) -> Command {
        let mut command = Command::new("cmd");
        command.args(["/C", script]);
        command
    }

    #[test]
    fn captures_output_and_exit_code() {
        let output =
            run_confined(&mut shell("echo hello & exit 3"), &ConfineLimits::default()).unwrap();
        assert_eq!(output.exit_code, Some(3));
        assert!(output.output.contains("hello"));
        assert!(!output.timed_out);
    }

    #[test]
    fn timeout_kills_the_whole_process_tree() {
        let limits = ConfineLimits {
            timeout: Duration::from_millis(500),
            ..ConfineLimits::default()
        };
        let started = Instant::now();
        let output = run_confined(&mut shell("ping -n 30 127.0.0.1 >nul"), &limits).unwrap();
        assert!(output.timed_out);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn process_limit_stops_a_fork_bomb_pattern() {
        let nested = "cmd /C echo nested";
        let unrestricted = run_confined(&mut shell(nested), &ConfineLimits::default()).unwrap();
        assert!(unrestricted.output.contains("nested"));

        let limits = ConfineLimits {
            max_processes: 1,
            ..ConfineLimits::default()
        };
        let output = run_confined(&mut shell(nested), &limits).unwrap();
        assert!(!output.output.contains("nested"));
    }

    #[test]
    fn excess_output_is_capped() {
        let limits = ConfineLimits {
            max_output_bytes: 1024,
            ..ConfineLimits::default()
        };
        let output = run_confined(
            &mut shell("for /L %i in (1,1,5000) do @echo 0123456789abcdef"),
            &limits,
        )
        .unwrap();
        assert!(output.output.len() <= 2048);
    }

    #[test]
    fn missing_program_is_an_error() {
        let mut command = Command::new("nortex-no-such-program.exe");
        assert!(run_confined(&mut command, &ConfineLimits::default()).is_err());
    }

    fn scratch(name: &str) -> std::path::PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "nortex-confine-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    fn writer(workspace: &Path, target: &str) -> Command {
        let mut command = shell(&format!("echo written> {target}"));
        command.current_dir(workspace);
        command
    }

    #[test]
    fn process_does_not_run_before_it_is_in_the_job() {
        let workspace = scratch("suspended");
        create_workspace(&workspace).unwrap();
        let marker = workspace.join("started.txt");

        let output = run_confined_with(
            &mut writer(&workspace, "started.txt"),
            &ConfineLimits::default(),
            |job, process| {
                std::thread::sleep(Duration::from_millis(500));
                assert!(
                    !marker.exists(),
                    "the process ran before the job assignment"
                );
                job.assign(process)
            },
        )
        .unwrap();
        assert_eq!(output.exit_code, Some(0));
        assert!(marker.exists());
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn failed_job_assignment_ends_the_process_unexecuted() {
        let workspace = scratch("assign-failure");
        create_workspace(&workspace).unwrap();

        let result = run_confined_with(
            &mut writer(&workspace, "started.txt"),
            &ConfineLimits::default(),
            |_, _| Err(io::Error::other("assignment refused")),
        );
        assert_eq!(result.unwrap_err().to_string(), "assignment refused");
        std::thread::sleep(Duration::from_millis(500));
        assert!(!workspace.join("started.txt").exists());
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn writes_are_limited_to_the_workspace() {
        let workspace = scratch("write-inside");
        let outside = scratch("write-outside");
        create_workspace(&workspace).unwrap();
        std::fs::create_dir_all(&outside).unwrap();

        let inside =
            run_confined(&mut writer(&workspace, "ok.txt"), &ConfineLimits::default()).unwrap();
        assert_eq!(inside.exit_code, Some(0), "{}", inside.output);
        assert!(workspace.join("ok.txt").exists());

        let target = outside.join("denied.txt");
        let denied = run_confined(
            &mut writer(&workspace, &target.to_string_lossy()),
            &ConfineLimits::default(),
        )
        .unwrap();
        assert_ne!(denied.exit_code, Some(0));
        assert!(!target.exists());

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn files_created_in_the_workspace_stay_writable() {
        let workspace = scratch("inherit");
        create_workspace(&workspace).unwrap();
        std::fs::create_dir(workspace.join("sub")).unwrap();
        std::fs::write(workspace.join("sub").join("copy.txt"), "x").unwrap();

        let output = run_confined(
            &mut writer(&workspace, r"sub\copy.txt"),
            &ConfineLimits::default(),
        )
        .unwrap();
        assert_eq!(output.exit_code, Some(0), "{}", output.output);
        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn reading_outside_the_workspace_is_still_possible() {
        let workspace = scratch("read-inside");
        let outside = scratch("read-outside");
        create_workspace(&workspace).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let source = outside.join("source.txt");
        std::fs::write(&source, "readable").unwrap();

        let mut command = shell(&format!("type {}", source.display()));
        command.current_dir(&workspace);
        let output = run_confined(&mut command, &ConfineLimits::default()).unwrap();
        assert!(output.output.contains("readable"), "{}", output.output);

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_dir_all(&outside);
    }

    #[test]
    fn environment_overrides_reach_the_process() {
        let mut command = shell("echo %NORTEX_CONFINE_PROBE%");
        command.env("NORTEX_CONFINE_PROBE", "probe-value");
        let output = run_confined(&mut command, &ConfineLimits::default()).unwrap();
        assert!(output.output.contains("probe-value"));
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    #[test]
    fn unsupported_platforms_refuse_to_run() {
        let error = run_confined(&mut Command::new("true"), &ConfineLimits::default()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}
