//! NORTEX runtime library (Unified v0.1.0).

pub mod allocation;
#[path = "async/mod.rs"]
pub mod async_runtime;
pub mod exceptions;
pub mod io;
pub mod memory;
pub mod networking;
pub mod platform;
pub mod startup;
pub mod string;
pub mod text;
pub mod threading;

use std::sync::{Mutex, OnceLock};

static OBJECT_ARENA: OnceLock<Mutex<Vec<Box<[u64]>>>> = OnceLock::new();

/// Allocates zero-initialized object storage retained until arena shutdown.
///
/// The current native ABI has no stack-map/GC-root metadata, so reclaiming
/// individual objects safely (especially in the presence of cycles) is not
/// possible yet. Generated entry points shut down the arena on normal return.
#[unsafe(no_mangle)]
pub extern "C" fn nortex_object_alloc(bytes: usize, class_id: usize) -> *mut u8 {
    let Some(total_bytes) = bytes.max(8).checked_add(std::mem::size_of::<usize>()) else {
        return std::ptr::null_mut();
    };
    let words = total_bytes.div_ceil(std::mem::size_of::<u64>());
    let mut storage = Vec::new();
    if storage.try_reserve_exact(words).is_err() {
        return std::ptr::null_mut();
    }
    storage.resize(words, 0_u64);
    let mut allocation = storage.into_boxed_slice();
    allocation[0] = class_id as u64;
    let pointer = unsafe {
        allocation
            .as_mut_ptr()
            .cast::<u8>()
            .add(std::mem::size_of::<usize>())
    };
    let arena = OBJECT_ARENA.get_or_init(|| Mutex::new(Vec::new()));
    match arena.lock() {
        Ok(mut objects) => objects.push(allocation),
        Err(poisoned) => poisoned.into_inner().push(allocation),
    }
    pointer
}

/// Releases all arena allocations after the generated program's entry point returns.
#[unsafe(no_mangle)]
pub extern "C" fn nortex_object_arena_shutdown() {
    let Some(arena) = OBJECT_ARENA.get() else {
        return;
    };
    match arena.lock() {
        Ok(mut objects) => objects.clear(),
        Err(poisoned) => poisoned.into_inner().clear(),
    }
}

/// Returns the runtime name.
pub fn runtime_name() -> &'static str {
    "NORTEX Runtime"
}

/// Stable C ABI marker. The value changes only when the exported runtime ABI
/// contract changes incompatibly.
#[unsafe(no_mangle)]
pub extern "C" fn nortex_runtime_abi_version() -> u32 {
    2
}

/// Initializes process-wide facilities and the panic payload TLS slot.
/// Returns a nonzero exit code if native TLS is unavailable or initialization fails.
#[unsafe(no_mangle)]
pub extern "C" fn nortex_runtime_initialize() -> i32 {
    startup::initialize_process();
    #[cfg(windows)]
    if platform::windows_x64::init_runtime_tls() {
        0
    } else {
        1
    }
    #[cfg(not(windows))]
    {
        1
    }
}

/// Shuts down process-wide facilities. Outstanding tasks must be joined by
/// the caller before shutdown; the current runtime has no global executor.
#[unsafe(no_mangle)]
pub extern "C" fn nortex_runtime_shutdown() -> i32 {
    #[cfg(windows)]
    if platform::windows_x64::shutdown_runtime_tls() {
        0
    } else {
        1
    }
    #[cfg(not(windows))]
    {
        0
    }
}

#[cfg(test)]
mod object_arena_tests {
    use super::{nortex_object_alloc, nortex_object_arena_shutdown};

    #[test]
    fn object_arena_keeps_zeroed_cyclic_objects_alive() {
        unsafe {
            let left = nortex_object_alloc(8, 1).cast::<usize>();
            let right = nortex_object_alloc(8, 2).cast::<usize>();
            assert!(!left.is_null());
            assert!(!right.is_null());
            assert_eq!(*left, 0);
            assert_eq!(*right, 0);

            *left = right as usize;
            *right = left as usize;
            assert_eq!(*(*left as *mut usize), left as usize);
            assert_eq!(*(*right as *mut usize), right as usize);
            assert_eq!(super::OBJECT_ARENA.get().unwrap().lock().unwrap().len(), 2);
        }
        nortex_object_arena_shutdown();
        assert_eq!(super::OBJECT_ARENA.get().unwrap().lock().unwrap().len(), 0);
    }

    #[test]
    fn object_allocation_reports_overflow_as_null() {
        assert!(nortex_object_alloc(usize::MAX, 1).is_null());
    }
}
