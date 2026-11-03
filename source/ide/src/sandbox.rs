//! Windows-only process sandbox for extensions (AppContainer + Job Object).
//!
//! Fails closed: unknown permissions, missing approval, or any setup error
//! aborts the run; the process is never started unsandboxed. On non-Windows
//! platforms every run returns [`SandboxError::Unsupported`].

use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

pub const PERM_NETWORK_CLIENT: &str = "network.client";
pub const PERM_NETWORK_PRIVATE: &str = "network.private";
pub const PERM_FS_PACKAGE_READ: &str = "filesystem.package.read";
pub const PERM_FS_PACKAGE_WRITE: &str = "filesystem.package.write";

const MAX_ID_LEN: usize = 48;
#[cfg(windows)]
const MAX_COMMAND_LINE_UTF16: usize = 32_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    NetworkClient,
    NetworkPrivate,
    FilesystemPackageRead,
    FilesystemPackageWrite,
}

impl Permission {
    pub fn parse(name: &str) -> Result<Self, SandboxError> {
        match name {
            PERM_NETWORK_CLIENT => Ok(Self::NetworkClient),
            PERM_NETWORK_PRIVATE => Ok(Self::NetworkPrivate),
            PERM_FS_PACKAGE_READ => Ok(Self::FilesystemPackageRead),
            PERM_FS_PACKAGE_WRITE => Ok(Self::FilesystemPackageWrite),
            other => Err(SandboxError::UnknownPermission(other.to_string())),
        }
    }
}

/// Parses manifest permission names; any unknown name rejects the whole set.
pub fn parse_permissions(names: &[String]) -> Result<Vec<Permission>, SandboxError> {
    let mut out = Vec::new();
    for name in names {
        let perm = Permission::parse(name)?;
        if !out.contains(&perm) {
            out.push(perm);
        }
    }
    Ok(out)
}

/// Normalizes an extension ID to a stable AppContainer name component.
/// Only `[A-Za-z0-9._-]` is accepted (lowercased); anything else is rejected
/// rather than mapped, so distinct IDs can never collide on one profile.
pub fn normalize_extension_id(id: &str) -> Result<String, SandboxError> {
    let norm = id.trim().to_ascii_lowercase();
    let valid = !norm.is_empty()
        && norm.len() <= MAX_ID_LEN
        && norm
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if valid {
        Ok(norm)
    } else {
        Err(SandboxError::InvalidExtensionId(id.to_string()))
    }
}

/// Quotes one argument following the CommandLineToArgvW rules.
pub fn quote_arg(arg: &str) -> String {
    if !arg.is_empty()
        && !arg
            .chars()
            .any(|c| matches!(c, ' ' | '\t' | '\n' | '\x0b' | '"'))
    {
        return arg.to_string();
    }
    let mut out = String::from("\"");
    let mut backslashes = 0usize;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

pub fn build_command_line(exe: &str, args: &[String]) -> String {
    let mut line = quote_arg(exe);
    for a in args {
        line.push(' ');
        line.push_str(&quote_arg(a));
    }
    line
}

#[derive(Debug, Clone)]
pub struct SandboxLimits {
    /// Wall-clock limit; the whole job is terminated when exceeded.
    pub timeout: Duration,
    /// CPU (user) time limit for the whole job.
    pub cpu_time: Duration,
    /// Committed memory limit for the whole job.
    pub memory_bytes: usize,
    /// Per-stream capture cap; excess output is drained and discarded.
    pub max_output_bytes: usize,
}

impl Default for SandboxLimits {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            cpu_time: Duration::from_secs(20),
            memory_bytes: 256 * 1024 * 1024,
            max_output_bytes: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SandboxRequest {
    pub executable: PathBuf,
    pub args: Vec<String>,
    pub working_dir: PathBuf,
    pub extension_id: String,
    pub installed_dir: PathBuf,
    /// Manifest-declared permissions the user approved for this run.
    pub permissions: Vec<String>,
    /// Must be true: set only after explicit per-run user approval.
    pub run_approved: bool,
    /// SHA-256 (hex) the executable must have. The file is held open without
    /// write/delete sharing from the check until the process has exited.
    pub expected_sha256: Option<String>,
    pub limits: SandboxLimits,
}

#[derive(Debug, Clone)]
pub struct SandboxOutput {
    pub exit_code: u32,
    pub stdout: String,
    pub stderr: String,
    pub output_truncated: bool,
    pub timed_out: bool,
    pub cancelled: bool,
}

#[derive(Debug)]
pub enum SandboxError {
    Unsupported,
    NotApproved,
    UnknownPermission(String),
    InvalidExtensionId(String),
    InvalidPath(String),
    InvalidLimits(String),
    InvalidArgument(String),
    IntegrityMismatch,
    Os { context: &'static str, code: u32 },
}

impl fmt::Display for SandboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => write!(f, "process sandbox is only supported on Windows"),
            Self::NotApproved => write!(f, "run was not explicitly approved by the user"),
            Self::UnknownPermission(p) => write!(f, "unknown permission: {p}"),
            Self::InvalidExtensionId(i) => write!(f, "invalid extension id: {i}"),
            Self::InvalidPath(m) => write!(f, "invalid path: {m}"),
            Self::InvalidLimits(m) => write!(f, "invalid limits: {m}"),
            Self::InvalidArgument(m) => write!(f, "invalid argument: {m}"),
            Self::IntegrityMismatch => write!(f, "executable does not match its expected SHA-256"),
            Self::Os { context, code } => write!(f, "{context} failed (0x{code:08X})"),
        }
    }
}

impl std::error::Error for SandboxError {}

pub(crate) struct Validated {
    ext_name: String,
    perms: Vec<Permission>,
    installed_dir: PathBuf,
    working_dir: PathBuf,
    executable: PathBuf,
}

fn validate(req: &SandboxRequest) -> Result<Validated, SandboxError> {
    if !req.run_approved {
        return Err(SandboxError::NotApproved);
    }
    let perms = parse_permissions(&req.permissions)?;
    let ext_name = normalize_extension_id(&req.extension_id)?;
    let l = &req.limits;
    if l.timeout.is_zero() || l.cpu_time.is_zero() || l.memory_bytes == 0 || l.max_output_bytes == 0
    {
        return Err(SandboxError::InvalidLimits(
            "all limits must be non-zero".into(),
        ));
    }
    if req.executable.as_os_str().is_empty() {
        return Err(SandboxError::InvalidPath("empty executable".into()));
    }
    if req.args.iter().any(|a| a.contains('\0')) || req.executable.to_string_lossy().contains('\0')
    {
        return Err(SandboxError::InvalidArgument("NUL character".into()));
    }
    let installed_dir = std::fs::canonicalize(&req.installed_dir)
        .map_err(|e| SandboxError::InvalidPath(format!("installed dir: {e}")))?;
    if !installed_dir.is_dir() || installed_dir.parent().is_none() {
        return Err(SandboxError::InvalidPath(
            "installed dir must be a non-root directory".into(),
        ));
    }
    let working_dir = std::fs::canonicalize(&req.working_dir)
        .map_err(|e| SandboxError::InvalidPath(format!("working dir: {e}")))?;
    if !working_dir.is_dir() || !working_dir.starts_with(&installed_dir) {
        return Err(SandboxError::InvalidPath(
            "working dir must be inside installed dir".into(),
        ));
    }
    let executable = std::fs::canonicalize(&req.executable)
        .map_err(|e| SandboxError::InvalidPath(format!("executable: {e}")))?;
    if !executable.is_file() || !executable.starts_with(&installed_dir) {
        return Err(SandboxError::InvalidPath(
            "executable must be a regular file inside installed dir".into(),
        ));
    }
    Ok(Validated {
        ext_name,
        perms,
        installed_dir,
        working_dir,
        executable,
    })
}

/// Runs the executable inside an AppContainer + Job Object sandbox.
pub fn run_sandboxed(req: &SandboxRequest) -> Result<SandboxOutput, SandboxError> {
    run_sandboxed_cancellable(req, &AtomicBool::new(false))
}

/// Like [`run_sandboxed`], but terminates the whole job as soon as `cancel` is set.
pub fn run_sandboxed_cancellable(
    req: &SandboxRequest,
    cancel: &AtomicBool,
) -> Result<SandboxOutput, SandboxError> {
    let v = validate(req)?;
    let _pinned = match &req.expected_sha256 {
        Some(expected) => Some(pin_executable(&v.executable, expected)?),
        None => None,
    };
    run_platform(req, &v, cancel)
}

/// A sandboxed process with live pipes, used for plugin IPC. The same checks as
/// [`run_sandboxed`] apply; the executable stays pinned and the process is killed
/// when this value is dropped.
pub struct SandboxProcess {
    stdin: Option<std::fs::File>,
    stdout: Option<std::fs::File>,
    stderr: Option<std::fs::File>,
    child: platform::Child,
    _pinned: Option<std::fs::File>,
}

impl SandboxProcess {
    /// Hands out (stdin, stdout, stderr) once; later calls return `None`.
    pub fn take_io(&mut self) -> Option<(std::fs::File, std::fs::File, std::fs::File)> {
        Some((self.stdin.take()?, self.stdout.take()?, self.stderr.take()?))
    }

    pub fn exit_code(&self) -> Option<u32> {
        self.child.exit_code()
    }

    pub fn kill(&self) {
        self.child.kill();
    }
}

pub fn spawn_sandboxed(req: &SandboxRequest) -> Result<SandboxProcess, SandboxError> {
    let v = validate(req)?;
    let pinned = match &req.expected_sha256 {
        Some(expected) => Some(pin_executable(&v.executable, expected)?),
        None => None,
    };
    let launched = platform::launch(req, &v, true)?;
    Ok(SandboxProcess {
        stdin: launched.stdin,
        stdout: Some(launched.stdout),
        stderr: Some(launched.stderr),
        child: launched.child,
        _pinned: pinned,
    })
}

/// Opens the executable so it cannot be modified or replaced while the returned
/// handle lives, and verifies its digest through that same handle.
fn pin_executable(path: &Path, expected: &str) -> Result<std::fs::File, SandboxError> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ);
    }
    let mut file = options
        .open(path)
        .map_err(|e| SandboxError::InvalidPath(format!("executable: {e}")))?;
    let mut digest = Sha256::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let read = file
            .read(&mut chunk)
            .map_err(|e| SandboxError::InvalidPath(format!("executable: {e}")))?;
        if read == 0 {
            break;
        }
        digest.update(&chunk[..read]);
    }
    if !hex::encode(digest.finalize()).eq_ignore_ascii_case(expected) {
        return Err(SandboxError::IntegrityMismatch);
    }
    Ok(file)
}

pub fn file_sha256(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        let read = file.read(&mut chunk)?;
        if read == 0 {
            return Ok(hex::encode(digest.finalize()));
        }
        digest.update(&chunk[..read]);
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;
    use std::fs::File;

    pub struct Child;

    impl Child {
        pub fn exit_code(&self) -> Option<u32> {
            None
        }

        pub fn kill(&self) {}
    }

    pub struct Launched {
        pub child: Child,
        pub stdin: Option<File>,
        pub stdout: File,
        pub stderr: File,
    }

    pub fn launch(
        _req: &SandboxRequest,
        _v: &Validated,
        _interactive: bool,
    ) -> Result<Launched, SandboxError> {
        Err(SandboxError::Unsupported)
    }
}

#[cfg(windows)]
use win as platform;

#[cfg(not(windows))]
fn run_platform(
    _req: &SandboxRequest,
    _v: &Validated,
    _cancel: &AtomicBool,
) -> Result<SandboxOutput, SandboxError> {
    Err(SandboxError::Unsupported)
}

#[cfg(windows)]
fn run_platform(
    req: &SandboxRequest,
    v: &Validated,
    cancel: &AtomicBool,
) -> Result<SandboxOutput, SandboxError> {
    win::run(req, v, cancel)
}

#[cfg(windows)]
pub(crate) mod win {
    use super::*;
    use std::ffi::{OsStr, c_void};
    use std::fs::File;
    use std::io::Read;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::*;
    use windows_sys::Win32::Security::Authorization::*;
    use windows_sys::Win32::Security::Isolation::*;
    use windows_sys::Win32::Security::*;
    use windows_sys::Win32::Storage::FileSystem::*;
    use windows_sys::Win32::System::JobObjects::*;
    use windows_sys::Win32::System::Pipes::CreatePipe;
    use windows_sys::Win32::System::Threading::*;

    pub(crate) const PROC_THREAD_ATTRIBUTE_HANDLE_LIST: usize = 0x0002_0002;
    const PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES: usize = 0x0002_0009;
    const SE_GROUP_ENABLED: u32 = 0x4;
    const HRESULT_ALREADY_EXISTS: i32 = 0x8007_00B7_u32 as i32;
    // Well-known capability SIDs: internetClient, privateNetworkClientServer.
    const SID_INTERNET_CLIENT: &str = "S-1-15-3-1";
    const SID_PRIVATE_NETWORK: &str = "S-1-15-3-3";

    fn os_err(context: &'static str, code: u32) -> SandboxError {
        SandboxError::Os { context, code }
    }

    pub(crate) fn last(context: &'static str) -> SandboxError {
        os_err(context, unsafe { GetLastError() })
    }

    pub(crate) fn wide(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(std::iter::once(0)).collect()
    }

    pub(crate) struct Handle(pub(crate) HANDLE);
    impl Handle {
        pub(crate) fn valid(h: HANDLE) -> Option<Self> {
            (!h.is_null() && h != INVALID_HANDLE_VALUE).then(|| Self(h))
        }
        pub(crate) fn into_file(mut self) -> File {
            let h = std::mem::replace(&mut self.0, null_mut());
            unsafe { File::from_raw_handle(h as _) }
        }
    }
    impl Drop for Handle {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CloseHandle(self.0) };
            }
        }
    }

    /// SID allocated by the AppContainer APIs (released with FreeSid).
    struct AcSid(PSID);
    impl Drop for AcSid {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { FreeSid(self.0) };
            }
        }
    }

    /// Memory allocated by the system (released with LocalFree).
    struct LocalMem(*mut c_void);
    impl Drop for LocalMem {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { LocalFree(self.0 as _) };
            }
        }
    }

    pub(crate) struct AttrList {
        buf: Vec<usize>,
    }
    impl AttrList {
        pub(crate) fn new(count: u32) -> Result<Self, SandboxError> {
            let mut size = 0usize;
            unsafe { InitializeProcThreadAttributeList(null_mut(), count, 0, &mut size) };
            let mut buf = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
            if unsafe {
                InitializeProcThreadAttributeList(buf.as_mut_ptr() as _, count, 0, &mut size)
            } == 0
            {
                return Err(last("InitializeProcThreadAttributeList"));
            }
            Ok(Self { buf })
        }
        pub(crate) fn ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
            self.buf.as_mut_ptr() as _
        }
        /// Caller guarantees `value` outlives this list's use.
        pub(crate) unsafe fn set(
            &mut self,
            attr: usize,
            value: *const c_void,
            size: usize,
        ) -> Result<(), SandboxError> {
            if unsafe {
                UpdateProcThreadAttribute(self.ptr(), 0, attr, value, size, null_mut(), null())
            } == 0
            {
                return Err(last("UpdateProcThreadAttribute"));
            }
            Ok(())
        }
    }
    impl Drop for AttrList {
        fn drop(&mut self) {
            unsafe { DeleteProcThreadAttributeList(self.ptr()) };
        }
    }

    fn container_sid(name: &str) -> Result<AcSid, SandboxError> {
        let wname = wide(OsStr::new(name));
        let mut sid: PSID = null_mut();
        let hr = unsafe {
            CreateAppContainerProfile(
                wname.as_ptr(),
                wname.as_ptr(),
                wname.as_ptr(),
                null(),
                0,
                &mut sid,
            )
        };
        if hr == 0 {
            return Ok(AcSid(sid));
        }
        if hr != HRESULT_ALREADY_EXISTS {
            return Err(os_err("CreateAppContainerProfile", hr as u32));
        }
        let hr = unsafe { DeriveAppContainerSidFromAppContainerName(wname.as_ptr(), &mut sid) };
        if hr != 0 {
            return Err(os_err(
                "DeriveAppContainerSidFromAppContainerName",
                hr as u32,
            ));
        }
        Ok(AcSid(sid))
    }

    fn capability_sid(s: &str) -> Result<LocalMem, SandboxError> {
        let w = wide(OsStr::new(s));
        let mut sid: PSID = null_mut();
        if unsafe { ConvertStringSidToSidW(w.as_ptr(), &mut sid) } == 0 {
            return Err(last("ConvertStringSidToSidW"));
        }
        Ok(LocalMem(sid))
    }

    /// Grants (or revokes, when `access` is `None`) the AppContainer SID access
    /// to exactly `dir`, inherited by its children. Replaces earlier ACEs for the SID.
    fn apply_dir_acl(dir: &Path, sid: PSID, access: Option<u32>) -> Result<(), SandboxError> {
        let path = wide(dir.as_os_str());
        let mut old_dacl: *mut ACL = null_mut();
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        let rc = unsafe {
            GetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut old_dacl,
                null_mut(),
                &mut sd,
            )
        };
        let _sd = LocalMem(sd);
        if rc != 0 {
            return Err(os_err("GetNamedSecurityInfoW", rc));
        }
        let ea = EXPLICIT_ACCESS_W {
            grfAccessPermissions: access.unwrap_or(0),
            grfAccessMode: if access.is_some() {
                SET_ACCESS
            } else {
                REVOKE_ACCESS
            },
            grfInheritance: SUB_CONTAINERS_AND_OBJECTS_INHERIT,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: null_mut(),
                MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_UNKNOWN,
                ptstrName: sid as *mut u16,
            },
        };
        let mut new_dacl: *mut ACL = null_mut();
        let rc = unsafe { SetEntriesInAclW(1, &ea, old_dacl, &mut new_dacl) };
        let _nd = LocalMem(new_dacl as _);
        if rc != 0 {
            return Err(os_err("SetEntriesInAclW", rc));
        }
        let rc = unsafe {
            SetNamedSecurityInfoW(
                path.as_ptr() as _,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                new_dacl,
                null_mut(),
            )
        };
        if rc != 0 {
            return Err(os_err("SetNamedSecurityInfoW", rc));
        }
        Ok(())
    }

    fn apply_executable_acl(executable: &Path, sid: PSID) -> Result<(), SandboxError> {
        let path = wide(executable.as_os_str());
        let mut old_dacl: *mut ACL = null_mut();
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        let rc = unsafe {
            GetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                &mut old_dacl,
                null_mut(),
                &mut sd,
            )
        };
        let _sd = LocalMem(sd);
        if rc != 0 {
            return Err(os_err("GetNamedSecurityInfoW(executable)", rc));
        }
        let ea = EXPLICIT_ACCESS_W {
            grfAccessPermissions: FILE_GENERIC_READ | FILE_GENERIC_EXECUTE,
            grfAccessMode: SET_ACCESS,
            grfInheritance: NO_INHERITANCE,
            Trustee: TRUSTEE_W {
                pMultipleTrustee: null_mut(),
                MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_UNKNOWN,
                ptstrName: sid as *mut u16,
            },
        };
        let mut new_dacl: *mut ACL = null_mut();
        let rc = unsafe { SetEntriesInAclW(1, &ea, old_dacl, &mut new_dacl) };
        let _nd = LocalMem(new_dacl as _);
        if rc != 0 {
            return Err(os_err("SetEntriesInAclW(executable)", rc));
        }
        let rc = unsafe {
            SetNamedSecurityInfoW(
                path.as_ptr() as _,
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                new_dacl,
                null_mut(),
            )
        };
        if rc != 0 {
            return Err(os_err("SetNamedSecurityInfoW(executable)", rc));
        }
        Ok(())
    }

    fn inheritable_attrs() -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: null_mut(),
            bInheritHandle: 1,
        }
    }

    /// Returns (read end, inheritable write end).
    pub(crate) fn make_pipe() -> Result<(Handle, Handle), SandboxError> {
        let sa = inheritable_attrs();
        let (mut r, mut w): (HANDLE, HANDLE) = (null_mut(), null_mut());
        if unsafe { CreatePipe(&mut r, &mut w, &sa, 0) } == 0 {
            return Err(last("CreatePipe"));
        }
        let (r, w) = (Handle(r), Handle(w));
        // The read end must not be inherited by the child.
        if unsafe { SetHandleInformation(r.0, HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err(last("SetHandleInformation"));
        }
        Ok((r, w))
    }

    /// Returns (inheritable read end for the child, parent's write end).
    fn make_stdin_pipe() -> Result<(Handle, Handle), SandboxError> {
        let sa = inheritable_attrs();
        let (mut r, mut w): (HANDLE, HANDLE) = (null_mut(), null_mut());
        if unsafe { CreatePipe(&mut r, &mut w, &sa, 0) } == 0 {
            return Err(last("CreatePipe(stdin)"));
        }
        let (r, w) = (Handle(r), Handle(w));
        if unsafe { SetHandleInformation(w.0, HANDLE_FLAG_INHERIT, 0) } == 0 {
            return Err(last("SetHandleInformation(stdin)"));
        }
        Ok((r, w))
    }

    pub(crate) fn open_nul() -> Result<Handle, SandboxError> {
        let sa = inheritable_attrs();
        let name = wide(OsStr::new("NUL"));
        let h = unsafe {
            CreateFileW(
                name.as_ptr(),
                GENERIC_READ,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                &sa,
                OPEN_EXISTING,
                0,
                null_mut(),
            )
        };
        Handle::valid(h).ok_or_else(|| last("CreateFileW(NUL)"))
    }

    fn drain(mut f: File, cap: usize) -> (Vec<u8>, bool) {
        let mut data = Vec::new();
        let mut truncated = false;
        let mut buf = [0u8; 8192];
        loop {
            match f.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let room = cap.saturating_sub(data.len());
                    if n > room {
                        truncated = true;
                    }
                    data.extend_from_slice(&buf[..n.min(room)]);
                }
            }
        }
        (data, truncated)
    }

    fn make_job(l: &SandboxLimits) -> Result<Handle, SandboxError> {
        let job = Handle::valid(unsafe { CreateJobObjectW(null(), null()) })
            .ok_or_else(|| last("CreateJobObjectW"))?;
        let mut ext: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        ext.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
            | JOB_OBJECT_LIMIT_JOB_TIME
            | JOB_OBJECT_LIMIT_JOB_MEMORY
            | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        ext.BasicLimitInformation.ActiveProcessLimit = 1;
        ext.BasicLimitInformation.PerJobUserTimeLimit =
            i64::try_from(l.cpu_time.as_nanos() / 100).unwrap_or(i64::MAX);
        ext.JobMemoryLimit = l.memory_bytes;
        let ok = unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &ext as *const _ as *const c_void,
                std::mem::size_of_val(&ext) as u32,
            )
        };
        if ok == 0 {
            return Err(last("SetInformationJobObject(limits)"));
        }
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
            return Err(last("SetInformationJobObject(ui)"));
        }
        Ok(job)
    }

    /// Minimal environment: nothing from the host process leaks into the sandbox.
    /// Profile/temp variables point at the extension dir (AppContainer start needs them).
    fn env_block(ext_name: &str, dir: &Path) -> Vec<u16> {
        let home = dir
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_string();
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        let mut vars = [
            format!("TEMP={home}"),
            format!("TMP={home}"),
            format!("USERPROFILE={home}"),
            format!("LOCALAPPDATA={home}"),
            format!("APPDATA={home}"),
            format!("NORTEX_EXTENSION_ID={ext_name}"),
            format!("PATH={root}\\System32"),
            format!("SystemRoot={root}"),
            format!("windir={root}"),
        ];
        vars.sort_by_key(|v| v.to_ascii_uppercase());
        let mut block = Vec::new();
        for v in vars {
            block.extend(v.encode_utf16());
            block.push(0);
        }
        block.push(0);
        block
    }

    /// Many programs (cmd.exe among them) reject `\\?\C:\...` as a working
    /// directory; drive-letter paths are equivalent without the prefix.
    fn plain_path(path: &Path) -> PathBuf {
        let text = path.to_string_lossy();
        match text.strip_prefix(r"\\?\") {
            Some(rest) if rest.as_bytes().get(1) == Some(&b':') => PathBuf::from(rest),
            _ => path.to_path_buf(),
        }
    }

    /// Handles of a started sandbox process. Closing the job handle kills the
    /// process (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`).
    pub struct Child {
        process: Handle,
        job: Handle,
    }

    // Kernel handles may be used from any thread.
    unsafe impl Send for Child {}
    unsafe impl Sync for Child {}

    impl Child {
        pub fn exit_code(&self) -> Option<u32> {
            if unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_OBJECT_0 {
                return None;
            }
            let mut code = 0u32;
            (unsafe { GetExitCodeProcess(self.process.0, &mut code) } != 0).then_some(code)
        }

        pub fn kill(&self) {
            unsafe { TerminateJobObject(self.job.0, 1) };
        }
    }

    pub struct Launched {
        pub child: Child,
        pub stdin: Option<File>,
        pub stdout: File,
        pub stderr: File,
    }

    /// Starts the process inside the AppContainer and the job object. With
    /// `interactive` the child's stdin is a pipe owned by the caller, otherwise NUL.
    pub fn launch(
        req: &SandboxRequest,
        v: &Validated,
        interactive: bool,
    ) -> Result<Launched, SandboxError> {
        let exe = v.executable.to_string_lossy().into_owned();
        let cmdline = build_command_line(&exe, &req.args);
        let mut cmd_w: Vec<u16> = cmdline.encode_utf16().chain(std::iter::once(0)).collect();
        if cmd_w.len() > MAX_COMMAND_LINE_UTF16 {
            return Err(SandboxError::InvalidArgument(
                "command line too long".into(),
            ));
        }
        let exe_w = wide(v.executable.as_os_str());
        let cwd_w = wide(plain_path(&v.working_dir).as_os_str());
        let env = env_block(&v.ext_name, &v.installed_dir);

        let sid = container_sid(&format!("nortex.ext.{}", v.ext_name))?;

        let read = v.perms.contains(&Permission::FilesystemPackageRead);
        let write = v.perms.contains(&Permission::FilesystemPackageWrite);
        let access = if write {
            Some(FILE_GENERIC_READ | FILE_GENERIC_EXECUTE | FILE_GENERIC_WRITE | DELETE)
        } else if read {
            Some(FILE_GENERIC_READ | FILE_GENERIC_EXECUTE)
        } else {
            None
        };
        apply_dir_acl(&v.installed_dir, sid.0, access)?;
        apply_executable_acl(&v.executable, sid.0)?;

        // Capability SIDs only for explicitly requested network permissions.
        let mut cap_mem = Vec::new();
        let mut caps: Vec<SID_AND_ATTRIBUTES> = Vec::new();
        for (p, s) in [
            (Permission::NetworkClient, SID_INTERNET_CLIENT),
            (Permission::NetworkPrivate, SID_PRIVATE_NETWORK),
        ] {
            if v.perms.contains(&p) {
                let m = capability_sid(s)?;
                caps.push(SID_AND_ATTRIBUTES {
                    Sid: m.0,
                    Attributes: SE_GROUP_ENABLED,
                });
                cap_mem.push(m);
            }
        }
        let sec_caps = SECURITY_CAPABILITIES {
            AppContainerSid: sid.0,
            Capabilities: if caps.is_empty() {
                null_mut()
            } else {
                caps.as_mut_ptr()
            },
            CapabilityCount: caps.len() as u32,
            Reserved: 0,
        };

        let (out_r, out_w) = make_pipe()?;
        let (err_r, err_w) = make_pipe()?;
        let (child_stdin, parent_stdin) = if interactive {
            let (read, write) = make_stdin_pipe()?;
            (read, Some(write))
        } else {
            (open_nul()?, None)
        };
        let inherit: [HANDLE; 3] = [child_stdin.0, out_w.0, err_w.0];

        let mut attrs = AttrList::new(2)?;
        unsafe {
            attrs.set(
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
                &sec_caps as *const _ as *const c_void,
                std::mem::size_of::<SECURITY_CAPABILITIES>(),
            )?;
            attrs.set(
                PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                inherit.as_ptr() as *const c_void,
                std::mem::size_of_val(&inherit),
            )?;
        }

        let mut si: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
        si.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
        si.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        si.StartupInfo.hStdInput = child_stdin.0;
        si.StartupInfo.hStdOutput = out_w.0;
        si.StartupInfo.hStdError = err_w.0;
        si.lpAttributeList = attrs.ptr();

        let job = make_job(&req.limits)?;

        let mut pi: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        let ok = unsafe {
            CreateProcessW(
                exe_w.as_ptr(),
                cmd_w.as_mut_ptr(),
                null(),
                null(),
                1,
                EXTENDED_STARTUPINFO_PRESENT
                    | CREATE_SUSPENDED
                    | CREATE_UNICODE_ENVIRONMENT
                    | CREATE_NO_WINDOW,
                env.as_ptr() as *const c_void,
                cwd_w.as_ptr(),
                &si.StartupInfo,
                &mut pi,
            )
        };
        if ok == 0 {
            return Err(last("CreateProcessW"));
        }
        let process = Handle(pi.hProcess);
        let thread = Handle(pi.hThread);

        // Contain the process before it executes a single instruction.
        if unsafe { AssignProcessToJobObject(job.0, process.0) } == 0 {
            let e = last("AssignProcessToJobObject");
            unsafe { TerminateProcess(process.0, 1) };
            return Err(e);
        }
        if unsafe { ResumeThread(thread.0) } == u32::MAX {
            let e = last("ResumeThread");
            unsafe { TerminateProcess(process.0, 1) };
            return Err(e);
        }
        drop(thread);
        // Close our copies of the child's ends so the readers see EOF.
        drop(out_w);
        drop(err_w);
        drop(child_stdin);
        drop(cap_mem);
        Ok(Launched {
            child: Child { process, job },
            stdin: parent_stdin.map(Handle::into_file),
            stdout: out_r.into_file(),
            stderr: err_r.into_file(),
        })
    }

    pub fn run(
        req: &SandboxRequest,
        v: &Validated,
        cancel: &AtomicBool,
    ) -> Result<SandboxOutput, SandboxError> {
        let Launched {
            child,
            stdout,
            stderr,
            ..
        } = launch(req, v, false)?;
        let Child { process, job } = child;

        let cap = req.limits.max_output_bytes;
        let t_out = std::thread::spawn(move || drain(stdout, cap));
        let t_err = std::thread::spawn(move || drain(stderr, cap));

        let deadline = std::time::Instant::now() + req.limits.timeout;
        let mut timed_out = false;
        let mut cancelled = false;
        loop {
            let wait = unsafe { WaitForSingleObject(process.0, 50) };
            if wait == WAIT_OBJECT_0 {
                break;
            }
            if wait != WAIT_TIMEOUT {
                unsafe { TerminateJobObject(job.0, 1) };
                return Err(last("WaitForSingleObject"));
            }
            cancelled = cancel.load(Ordering::Relaxed);
            timed_out = !cancelled && std::time::Instant::now() >= deadline;
            if cancelled || timed_out {
                unsafe { TerminateJobObject(job.0, 1) };
                unsafe { WaitForSingleObject(process.0, 5000) };
                break;
            }
        }
        let mut code = 0u32;
        if unsafe { GetExitCodeProcess(process.0, &mut code) } == 0 {
            return Err(last("GetExitCodeProcess"));
        }
        // Kill any leftover job members so pipe readers always terminate.
        unsafe { TerminateJobObject(job.0, code) };

        let (o, o_trunc) = t_out.join().unwrap_or_default();
        let (e, e_trunc) = t_err.join().unwrap_or_default();
        Ok(SandboxOutput {
            exit_code: code,
            stdout: String::from_utf8_lossy(&o).into_owned(),
            stderr: String::from_utf8_lossy(&e).into_owned(),
            output_truncated: o_trunc || e_trunc,
            timed_out,
            cancelled,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permissions_allow_list() {
        let ok = parse_permissions(&[
            "network.client".into(),
            "filesystem.package.read".into(),
            "network.client".into(),
        ])
        .unwrap();
        assert_eq!(
            ok,
            vec![Permission::NetworkClient, Permission::FilesystemPackageRead]
        );
        assert!(matches!(
            parse_permissions(&["network.client".into(), "filesystem.all".into()]),
            Err(SandboxError::UnknownPermission(_))
        ));
        assert!(Permission::parse("Network.Client").is_err());
        assert!(Permission::parse("").is_err());
        assert!(parse_permissions(&[]).unwrap().is_empty());
    }

    #[test]
    fn id_normalization() {
        assert_eq!(
            normalize_extension_id(" My.Ext-1_x ").unwrap(),
            "my.ext-1_x"
        );
        for bad in ["", "a/b", "a b", "..\\x", "?", &"a".repeat(MAX_ID_LEN + 1)] {
            assert!(normalize_extension_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn quoting() {
        assert_eq!(quote_arg("abc"), "abc");
        assert_eq!(quote_arg(""), "\"\"");
        assert_eq!(quote_arg("a b"), "\"a b\"");
        assert_eq!(quote_arg("a\"b"), "\"a\\\"b\"");
        assert_eq!(quote_arg("a b\\"), "\"a b\\\\\"");
        assert_eq!(quote_arg("c:\\x\\y"), "c:\\x\\y");
        assert_eq!(
            build_command_line("C:\\p f\\a.exe", &["x y".into()]),
            "\"C:\\p f\\a.exe\" \"x y\""
        );
    }

    fn req(approved: bool, perms: &[&str]) -> SandboxRequest {
        let dir = std::env::temp_dir();
        SandboxRequest {
            executable: "x.exe".into(),
            args: vec![],
            working_dir: dir.clone(),
            extension_id: "ext".into(),
            installed_dir: dir,
            permissions: perms.iter().map(|s| s.to_string()).collect(),
            run_approved: approved,
            expected_sha256: None,
            limits: SandboxLimits::default(),
        }
    }

    #[test]
    fn unapproved_and_unknown_fail_closed() {
        assert!(matches!(
            run_sandboxed(&req(false, &[])),
            Err(SandboxError::NotApproved)
        ));
        assert!(matches!(
            run_sandboxed(&req(true, &["bogus"])),
            Err(SandboxError::UnknownPermission(_))
        ));
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_unsupported() {
        assert!(matches!(
            run_sandboxed(&req(true, &[])),
            Err(SandboxError::Unsupported)
        ));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "needs a real AppContainer-capable Windows host; modifies a temp dir ACL"]
    fn windows_smoke() {
        let dir = smoke_dir("echo");
        let mut r = smoke_request(&dir, &["filesystem.package.read"]);
        r.args = vec!["/c".into(), "echo hello".into()];
        r.expected_sha256 = Some(file_sha256(&r.executable).unwrap());
        let out = run_sandboxed(&r);
        let _ = std::fs::remove_dir_all(&dir);
        let out = out.unwrap();
        assert_eq!(out.exit_code, 0);
        assert!(out.stdout.contains("hello"));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "needs a real AppContainer-capable Windows host; modifies a temp dir ACL"]
    fn windows_smoke_job_timeout_terminates_the_process() {
        let dir = smoke_dir("timeout");
        let mut r = smoke_request(&dir, &[]);
        r.args = vec!["/c".into(), "for /l %i in (1,1,2000000000) do @rem".into()];
        r.limits.timeout = Duration::from_millis(1500);
        let out = run_sandboxed(&r);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.unwrap().timed_out);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "needs a real AppContainer-capable Windows host; modifies a temp dir ACL"]
    fn windows_smoke_cancel_flag_terminates_the_process() {
        let dir = smoke_dir("cancel");
        let mut r = smoke_request(&dir, &[]);
        r.args = vec!["/c".into(), "for /l %i in (1,1,2000000000) do @rem".into()];
        r.limits.timeout = Duration::from_secs(30);
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let setter = std::sync::Arc::clone(&cancel);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(500));
            setter.store(true, Ordering::Relaxed);
        });
        let started = std::time::Instant::now();
        let out = run_sandboxed_cancellable(&r, &cancel);
        let _ = std::fs::remove_dir_all(&dir);
        let out = out.unwrap();
        assert!(out.cancelled && !out.timed_out);
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "needs a real AppContainer-capable Windows host; modifies a temp dir ACL"]
    fn windows_smoke_denies_package_writes_without_permission() {
        let dir = smoke_dir("deny-write");
        let mut r = smoke_request(&dir, &["filesystem.package.read"]);
        r.args = vec!["/c".into(), "copy nul probe.txt".into()];
        let out = run_sandboxed(&r);
        let written = dir.join("probe.txt").exists();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(!written);
        assert_ne!(out.unwrap().exit_code, 0);
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "needs a real AppContainer-capable Windows host; modifies a temp dir ACL"]
    fn windows_smoke_allows_package_writes_with_permission() {
        let dir = smoke_dir("allow-write");
        let mut r = smoke_request(&dir, &["filesystem.package.write"]);
        r.args = vec!["/c".into(), "copy nul probe.txt".into()];
        let out = run_sandboxed(&r);
        let written = dir.join("probe.txt").exists();
        let _ = std::fs::remove_dir_all(&dir);
        let out = out.unwrap();
        assert_eq!(out.exit_code, 0, "{} {}", out.stdout, out.stderr);
        assert!(written);
    }

    #[cfg(windows)]
    fn smoke_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("nortex-sandbox-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // The runner only starts files inside the install directory, so the smoke
    // tests copy cmd.exe there.
    #[cfg(windows)]
    fn smoke_request(dir: &Path, permissions: &[&str]) -> SandboxRequest {
        let root = std::env::var("SystemRoot").unwrap();
        let executable = dir.join("cmd.exe");
        std::fs::copy(format!("{root}\\System32\\cmd.exe"), &executable).unwrap();
        let mut request = req(true, permissions);
        request.executable = executable;
        request.working_dir = dir.to_path_buf();
        request.installed_dir = dir.to_path_buf();
        request.extension_id = "smoke".into();
        request
    }

    #[test]
    fn mismatching_digest_is_refused_before_any_process_starts() {
        let dir =
            std::env::temp_dir().join(format!("nortex-sandbox-digest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let executable = dir.join("tool.exe");
        std::fs::write(&executable, b"not really a program").unwrap();
        let mut r = req(true, &[]);
        r.executable = executable.clone();
        r.working_dir = dir.clone();
        r.installed_dir = dir.clone();
        r.expected_sha256 = Some("0".repeat(64));
        let refused = run_sandboxed(&r);
        let good = file_sha256(&executable).unwrap();
        let pinned = pin_executable(&executable, &good).is_ok();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(matches!(refused, Err(SandboxError::IntegrityMismatch)));
        assert!(pinned);
    }

    #[cfg(windows)]
    #[test]
    fn pinned_executable_cannot_be_overwritten_while_held() {
        let dir = std::env::temp_dir().join(format!("nortex-sandbox-pin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let executable = dir.join("tool.exe");
        std::fs::write(&executable, b"original").unwrap();
        let digest = file_sha256(&executable).unwrap();
        let pin = pin_executable(&executable, &digest).unwrap();
        let overwrite = std::fs::write(&executable, b"tampered");
        drop(pin);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(overwrite.is_err());
    }
}
