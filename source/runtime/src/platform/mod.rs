//! Platform runtime services.

#[cfg(windows)]
pub mod windows_x64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatingSystem {
    Windows,
    Linux,
    MacOS,
    Unknown,
}

pub fn current_os() -> OperatingSystem {
    if cfg!(target_os = "windows") {
        OperatingSystem::Windows
    } else if cfg!(target_os = "linux") {
        OperatingSystem::Linux
    } else if cfg!(target_os = "macos") {
        OperatingSystem::MacOS
    } else {
        OperatingSystem::Unknown
    }
}
