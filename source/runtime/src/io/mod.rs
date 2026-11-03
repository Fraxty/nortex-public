//! Io runtime services.

pub mod console;

use std::io;
use std::path::Path;

pub fn read_file(path: impl AsRef<Path>) -> io::Result<String> {
    std::fs::read_to_string(path)
}
