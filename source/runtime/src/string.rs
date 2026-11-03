//! Heap-backed string operations for generated NORTEX code.

#[cfg(windows)]
use std::ffi::c_void;

/// In-memory layout of a NORTEX `string`: pointer plus byte length.
#[repr(C)]
pub struct StringSlice {
    pub data: *const u8,
    pub len: usize,
}

#[cfg(windows)]
unsafe extern "system" {
    fn GetProcessHeap() -> *mut c_void;
    fn HeapAlloc(heap: *mut c_void, flags: u32, bytes: usize) -> *mut c_void;
    fn HeapFree(heap: *mut c_void, flags: u32, block: *mut c_void) -> i32;
}

#[cfg(windows)]
fn heap_allocate(bytes: usize) -> *mut u8 {
    // SAFETY: The process heap is valid for the whole process lifetime.
    unsafe { HeapAlloc(GetProcessHeap(), 0, bytes) as *mut u8 }
}

#[cfg(windows)]
unsafe fn heap_release(block: *mut u8, _bytes: usize) {
    // SAFETY: `block` was returned by `heap_allocate` and is released once.
    unsafe { HeapFree(GetProcessHeap(), 0, block as *mut c_void) };
}

#[cfg(not(windows))]
fn heap_allocate(bytes: usize) -> *mut u8 {
    let layout = match std::alloc::Layout::from_size_align(bytes, 16) {
        Ok(layout) => layout,
        Err(_) => return std::ptr::null_mut(),
    };
    // SAFETY: `bytes` is nonzero because a header is always reserved.
    unsafe { std::alloc::alloc(layout) }
}

#[cfg(not(windows))]
unsafe fn heap_release(block: *mut u8, bytes: usize) {
    if let Ok(layout) = std::alloc::Layout::from_size_align(bytes, 16) {
        // SAFETY: `block` was allocated with this layout and is released once.
        unsafe { std::alloc::dealloc(block, layout) };
    }
}

/// Heap block layout: `[magic u64][block size u64][StringSlice][bytes][NUL]`.
/// Static literal descriptors emitted by the compiler carry 16 zero bytes in
/// front of the descriptor, so the magic distinguishes heap from static data.
const PREFIX: usize = 16;
const HEADER: usize = PREFIX + std::mem::size_of::<StringSlice>();
const HEAP_MAGIC: u64 = 0x4E52_5458_5354_5231;

static LIVE_STRINGS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Number of heap strings that were allocated and not yet freed (diagnostics).
#[unsafe(no_mangle)]
pub extern "C" fn nortex_runtime_live_strings() -> usize {
    LIVE_STRINGS.load(std::sync::atomic::Ordering::SeqCst)
}

/// Concatenates two UTF-8 slices into one heap block.
///
/// The block holds a header, a 16-byte `StringSlice` descriptor and the bytes
/// plus a trailing NUL (not counted in `len`), so the data is a valid `char*`
/// for C. Returns a pointer to the descriptor, or null on invalid input,
/// overflow or allocation failure. Release it with `nortex_string_free`.
///
/// # Safety
/// Each input pointer must be readable for its length when that length is
/// nonzero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_string_concat(
    data1: *const u8,
    len1: usize,
    data2: *const u8,
    len2: usize,
) -> *mut StringSlice {
    if (len1 != 0 && data1.is_null()) || (len2 != 0 && data2.is_null()) {
        return std::ptr::null_mut();
    }
    let Some(total) = len1.checked_add(len2) else {
        return std::ptr::null_mut();
    };
    let Some(bytes) = total
        .checked_add(1)
        .and_then(|value| value.checked_add(HEADER))
    else {
        return std::ptr::null_mut();
    };
    let block = heap_allocate(bytes);
    if block.is_null() {
        return std::ptr::null_mut();
    }
    // SAFETY: `block` holds `HEADER + total + 1` bytes; inputs are readable per
    // contract. The allocation is 16-byte aligned.
    unsafe {
        (block as *mut u64).write(HEAP_MAGIC);
        (block as *mut u64).add(1).write(bytes as u64);
        let buffer = block.add(HEADER);
        if len1 != 0 {
            std::ptr::copy_nonoverlapping(data1, buffer, len1);
        }
        if len2 != 0 {
            std::ptr::copy_nonoverlapping(data2, buffer.add(len1), len2);
        }
        *buffer.add(total) = 0;
        let descriptor = block.add(PREFIX) as *mut StringSlice;
        descriptor.write(StringSlice {
            data: buffer,
            len: total,
        });
        LIVE_STRINGS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        descriptor
    }
}

/// Releases a string created by `nortex_string_concat`. Null pointers and
/// static literal descriptors are ignored, so it is safe on any string value.
///
/// # Safety
/// `descriptor` must be null or a NORTEX string descriptor (heap or static)
/// that is not used after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_string_free(descriptor: *mut StringSlice) {
    if descriptor.is_null() {
        return;
    }
    // SAFETY: Every descriptor is preceded by a 16-byte header.
    unsafe {
        let block = (descriptor as *mut u8).sub(PREFIX);
        if (block as *const u64).read() != HEAP_MAGIC {
            return;
        }
        let bytes = (block as *const u64).add(1).read() as usize;
        (block as *mut u64).write(0);
        LIVE_STRINGS.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
        heap_release(block, bytes);
    }
}

/// Compares two byte slices for equality.
///
/// # Safety
/// Each pointer must be readable for its length when that length is nonzero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_string_eq(
    data1: *const u8,
    len1: usize,
    data2: *const u8,
    len2: usize,
) -> bool {
    if len1 != len2 {
        return false;
    }
    if len1 == 0 {
        return true;
    }
    if data1.is_null() || data2.is_null() {
        return false;
    }
    // SAFETY: Both ranges are readable for `len1` bytes per contract.
    unsafe { std::slice::from_raw_parts(data1, len1) == std::slice::from_raw_parts(data2, len2) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concatenates_with_nul_terminator() {
        // SAFETY: All pointers reference live buffers of the given lengths.
        let out = unsafe { nortex_string_concat(b"ab".as_ptr(), 2, b"cd".as_ptr(), 2) };
        assert!(!out.is_null());
        // SAFETY: The descriptor and its `len + 1` bytes were just allocated.
        let bytes = unsafe { std::slice::from_raw_parts((*out).data, (*out).len + 1) };
        assert_eq!(bytes, b"abcd\0");
    }

    #[test]
    fn eq_and_free_handle_static_and_heap_strings() {
        // SAFETY: All pointers reference live buffers of the given lengths.
        unsafe {
            assert!(nortex_string_eq(b"ab".as_ptr(), 2, b"ab".as_ptr(), 2));
            assert!(!nortex_string_eq(b"ab".as_ptr(), 2, b"ac".as_ptr(), 2));
            assert!(!nortex_string_eq(b"a".as_ptr(), 1, b"ab".as_ptr(), 2));
            assert!(nortex_string_eq(std::ptr::null(), 0, std::ptr::null(), 0));
            nortex_string_free(std::ptr::null_mut());
            // A static descriptor carries a zeroed 16-byte header and is ignored.
            let static_block = [0u64, 0, 0, 0];
            nortex_string_free(static_block.as_ptr().add(2) as *mut StringSlice);
            let heap = nortex_string_concat(b"x".as_ptr(), 1, b"y".as_ptr(), 1);
            assert!(!heap.is_null());
            nortex_string_free(heap);
        }
    }

    #[test]
    fn handles_empty_inputs_and_null_rejection() {
        // SAFETY: Zero lengths need no valid input pointers.
        let out = unsafe { nortex_string_concat(std::ptr::null(), 0, std::ptr::null(), 0) };
        assert!(!out.is_null());
        // SAFETY: `out` is a valid descriptor.
        assert_eq!(unsafe { (*out).len }, 0);
        // SAFETY: Null with nonzero length is rejected before use.
        assert!(
            unsafe { nortex_string_concat(std::ptr::null(), 1, std::ptr::null(), 0) }.is_null()
        );
    }
}
