use std::io::{self, Write};

fn report_write_error(error: io::Error) -> i32 {
    let mut stderr = io::stderr().lock();
    let _ = writeln!(
        stderr,
        "NORTEX Runtime: Konsolenausgabe fehlgeschlagen: {error}"
    );
    1
}

/// Writes UTF-8 bytes to standard output for generated NORTEX code.
///
/// Returns zero on success and a nonzero value if the byte range is invalid or
/// the operating system reports an output error.
///
/// # Safety
/// When `length` is nonzero, `bytes` must point to `length` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_runtime_console_write(
    bytes: *const u8,
    length: usize,
    newline: u8,
) -> i32 {
    if length != 0 && bytes.is_null() {
        return 1;
    }

    crate::startup::initialize_process();
    let text = if length == 0 {
        &[]
    } else {
        // SAFETY: The caller guarantees that the byte range is readable.
        unsafe { std::slice::from_raw_parts(bytes, length) }
    };
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    if let Err(error) = output.write_all(text) {
        return report_write_error(error);
    }
    if newline != 0
        && let Err(error) = output.write_all(b"\n")
    {
        return report_write_error(error);
    }
    if let Err(error) = output.flush() {
        return report_write_error(error);
    }
    0
}

/// C-ABI: `int32_t nortex_print_i32(int32_t value)`; prints the value and a newline.
#[unsafe(no_mangle)]
pub extern "C" fn nortex_print_i32(value: i32) -> i32 {
    let text = value.to_string();
    // SAFETY: `text` is a live buffer of `text.len()` bytes.
    unsafe { nortex_runtime_console_write(text.as_ptr(), text.len(), 1) }
}

/// C-ABI: `int32_t nortex_print_str(const uint8_t* ptr, size_t len)`.
///
/// Prints exactly `len` bytes (embedded NULs allowed) plus a newline.
///
/// # Safety
/// When `len` is nonzero, `ptr` must point to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_print_str(ptr: *const u8, len: usize) -> i32 {
    // SAFETY: Forwarded under the same contract.
    unsafe { nortex_runtime_console_write(ptr, len, 1) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_functions_succeed_and_reject_null() {
        assert_eq!(nortex_print_i32(-7), 0);
        // SAFETY: Zero length needs no valid pointer.
        assert_eq!(unsafe { nortex_print_str(std::ptr::null(), 0) }, 0);
        // SAFETY: Null with nonzero length is rejected before use.
        assert_ne!(unsafe { nortex_print_str(std::ptr::null(), 3) }, 0);
    }

    #[test]
    fn rejects_null_pointer_for_nonempty_buffer() {
        // SAFETY: A null pointer with nonzero length is rejected before use.
        assert_ne!(
            unsafe { nortex_runtime_console_write(std::ptr::null(), 1, 0) },
            0
        );
    }
}
