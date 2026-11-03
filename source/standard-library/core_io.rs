//! NORTEX Standard Library: `core::io`
//!
//! Production-grade IO Subsystem:
//! - Core Traits: `Read`, `Write`, `Seek`, `BufRead`
//! - Buffered Streams: `BufReader<R>` & `BufWriter<W>`
//! - In-Memory Stream: `Cursor<T>`
//! - Windows Console Standard Streams (`Stdin`, `Stdout`, `Stderr`) via Win32 File Handles

use std::fmt;
use std::mem;
use std::ops::Deref;

// ----------------------------------------------------------------------------
// 1. IO-FEHLER & RESULT-TYPEN
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    BrokenPipe,
    AlreadyExists,
    WouldBlock,
    InvalidInput,
    InvalidData,
    TimedOut,
    WriteZero,
    UnexpectedEof,
    Other,
}

#[derive(Debug)]
pub struct IoError {
    kind: ErrorKind,
    message: String,
}

impl IoError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for IoError {}

pub type IoResult<T> = Result<T, IoError>;

// ----------------------------------------------------------------------------
// 2. KERN-TRAITS: READ, WRITE, SEEK, BUFREAD
// ----------------------------------------------------------------------------

pub trait Read {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize>;

    fn read_exact(&mut self, mut buf: &mut [u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.read(buf) {
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e),
            }
        }
        if !buf.is_empty() {
            Err(IoError::new(ErrorKind::UnexpectedEof, "Unerwartetes Stream-Ende in read_exact"))
        } else {
            Ok(())
        }
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> IoResult<usize> {
        let start_len = buf.len();
        let mut probe = [0u8; 8192];
        loop {
            match self.read(&mut probe) {
                Ok(0) => break,
                Ok(n) => buf.extend_from_slice(&probe[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(buf.len() - start_len)
    }
}

pub trait Write {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize>;
    fn flush(&mut self) -> IoResult<()>;

    fn write_all(&mut self, mut buf: &[u8]) -> IoResult<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => return Err(IoError::new(ErrorKind::WriteZero, "Konnte keine Bytes schreiben")),
                Ok(n) => buf = &buf[n..],
                Err(e) if e.kind() == ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    Start(u64),
    End(i64),
    Current(i64),
}

pub trait Seek {
    fn seek(&mut self, pos: SeekFrom) -> IoResult<u64>;
}

pub trait BufRead: Read {
    fn fill_buf(&mut self) -> IoResult<&[u8]>;
    fn consume(&mut self, amt: usize);

    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> IoResult<usize> {
        let mut read = 0;
        loop {
            let (done, used) = {
                let available = self.fill_buf()?;
                if available.is_empty() {
                    return Ok(read);
                }
                match available.iter().position(|&b| b == byte) {
                    Some(i) => {
                        buf.extend_from_slice(&available[..=i]);
                        (true, i + 1)
                    }
                    None => {
                        buf.extend_from_slice(available);
                        (false, available.len())
                    }
                }
            };
            self.consume(used);
            read += used;
            if done {
                return Ok(read);
            }
        }
    }

    fn read_line(&mut self, buf: &mut String) -> IoResult<usize> {
        let mut bytes = Vec::new();
        let count = self.read_until(b'\n', &mut bytes)?;
        let chunk_str = std::str::from_utf8(&bytes)
            .map_err(|e| IoError::new(ErrorKind::InvalidData, format!("Ungültiges UTF-8: {}", e)))?;
        buf.push_str(chunk_str);
        Ok(count)
    }
}

// ----------------------------------------------------------------------------
// 3. BUFREADER & BUFWRITER
// ----------------------------------------------------------------------------

pub const DEFAULT_BUF_SIZE: usize = 8192;

pub struct BufReader<R> {
    inner: R,
    buf: Box<[u8]>,
    pos: usize,
    cap: usize,
}

impl<R: Read> BufReader<R> {
    pub fn new(inner: R) -> Self {
        Self::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    pub fn with_capacity(capacity: usize, inner: R) -> Self {
        let buffer = vec![0u8; capacity.max(64)].into_boxed_slice();
        Self {
            inner,
            buf: buffer,
            pos: 0,
            cap: 0,
        }
    }

    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl<R: Read> Read for BufReader<R> {
    fn read(&mut self, out: &mut [u8]) -> IoResult<usize> {
        // Falls der Puffer leer ist und die Anfrage größer als Puffergröße ist: Direkt lesen
        if self.pos >= self.cap && out.len() >= self.buf.len() {
            self.discard_buffer();
            return self.inner.read(out);
        }

        let nread = {
            let mut available = self.fill_buf()?;
            let to_copy = available.len().min(out.len());
            out[..to_copy].copy_from_slice(&available[..to_copy]);
            to_copy
        };
        self.consume(nread);
        Ok(nread)
    }
}

impl<R: Read> BufRead for BufReader<R> {
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        if self.pos >= self.cap {
            self.cap = self.inner.read(&mut self.buf)?;
            self.pos = 0;
        }
        Ok(&self.buf[self.pos..self.cap])
    }

    fn consume(&mut self, amt: usize) {
        self.pos = (self.pos + amt).min(self.cap);
    }
}

impl<R> BufReader<R> {
    fn discard_buffer(&mut self) {
        self.pos = 0;
        self.cap = 0;
    }
}

pub struct BufWriter<W: Write> {
    inner: Option<W>,
    buf: Vec<u8>,
    panicked: bool,
}

impl<W: Write> BufWriter<W> {
    pub fn new(inner: W) -> Self {
        Self::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    pub fn with_capacity(capacity: usize, inner: W) -> Self {
        Self {
            inner: Some(inner),
            buf: Vec::with_capacity(capacity.max(64)),
            panicked: false,
        }
    }

    fn flush_buf(&mut self) -> IoResult<()> {
        let mut written = 0;
        let len = self.buf.len();
        let mut ret = Ok(());
        while written < len {
            self.panicked = true;
            let r = self.inner.as_mut().unwrap().write(&self.buf[written..]);
            self.panicked = false;
            match r {
                Ok(0) => {
                    ret = Err(IoError::new(ErrorKind::WriteZero, "Konnte gepufferte Bytes nicht schreiben"));
                    break;
                }
                Ok(n) => written += n,
                Err(e) => {
                    ret = Err(e);
                    break;
                }
            }
        }
        if written > 0 {
            self.buf.drain(..written);
        }
        ret
    }
}

impl<W: Write> Write for BufWriter<W> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        if self.buf.len() + buf.len() > self.buf.capacity() {
            self.flush_buf()?;
        }
        if buf.len() >= self.buf.capacity() {
            self.panicked = true;
            let r = self.inner.as_mut().unwrap().write(buf);
            self.panicked = false;
            r
        } else {
            self.buf.extend_from_slice(buf);
            Ok(buf.len())
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        self.flush_buf()?;
        self.inner.as_mut().unwrap().flush()
    }
}

impl<W: Write> Drop for BufWriter<W> {
    fn drop(&mut self) {
        if self.inner.is_some() && !self.panicked {
            let _ = self.flush_buf();
        }
    }
}

// ----------------------------------------------------------------------------
// 4. IN-MEMORY STREAM: CURSOR
// ----------------------------------------------------------------------------

pub struct Cursor<T> {
    inner: T,
    pos: u64,
}

impl<T> Cursor<T> {
    pub fn new(inner: T) -> Self {
        Self { inner, pos: 0 }
    }

    pub fn position(&self) -> u64 {
        self.pos
    }

    pub fn set_position(&mut self, pos: u64) {
        self.pos = pos;
    }

    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: AsRef<[u8]>> Read for Cursor<T> {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        let slice = self.inner.as_ref();
        if self.pos >= slice.len() as u64 {
            return Ok(0);
        }
        let available = &slice[self.pos as usize..];
        let n = available.len().min(buf.len());
        buf[..n].copy_from_slice(&available[..n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl<T: AsRef<[u8]>> BufRead for Cursor<T> {
    fn fill_buf(&mut self) -> IoResult<&[u8]> {
        let slice = self.inner.as_ref();
        if self.pos >= slice.len() as u64 {
            return Ok(&[]);
        }
        Ok(&slice[self.pos as usize..])
    }

    fn consume(&mut self, amt: usize) {
        self.pos = (self.pos + amt as u64).min(self.inner.as_ref().len() as u64);
    }
}

impl Write for Cursor<Vec<u8>> {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        let pos = self.pos as usize;
        let len = self.inner.len();
        if pos + buf.len() > len {
            self.inner.resize(pos + buf.len(), 0);
        }
        self.inner[pos..pos + buf.len()].copy_from_slice(buf);
        self.pos += buf.len() as u64;
        Ok(buf.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

// ----------------------------------------------------------------------------
// 5. STANDARD STREAMS (STDIN, STDOUT, STDERR FÜR WINDOWS & POSIX)
// ----------------------------------------------------------------------------

#[cfg(windows)]
extern "system" {
    fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
    fn ReadFile(
        hFile: *mut std::ffi::c_void,
        lpBuffer: *mut u8,
        nNumberOfBytesToRead: u32,
        lpNumberOfBytesRead: *mut u32,
        lpOverlapped: *mut std::ffi::c_void,
    ) -> i32;
    fn WriteFile(
        hFile: *mut std::ffi::c_void,
        lpBuffer: *const u8,
        nNumberOfBytesToWrite: u32,
        lpNumberOfBytesWritten: *mut u32,
        lpOverlapped: *mut std::ffi::c_void,
    ) -> i32;
}

const STD_INPUT_HANDLE: u32 = 0xFFFFFFF6;
const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5;
const STD_ERROR_HANDLE: u32 = 0xFFFFFFF4;

pub struct Stdout;
pub struct Stderr;
pub struct Stdin;

impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        #[cfg(windows)]
        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle.is_null() || handle as isize == -1 {
                return Err(IoError::new(ErrorKind::Other, "Ungültiger Standard-Output-Handle"));
            }
            let mut written: u32 = 0;
            let len = buf.len().min(u32::MAX as usize) as u32;
            let success = WriteFile(handle, buf.as_ptr(), len, &mut written, std::ptr::null_mut());
            if success == 0 {
                return Err(IoError::new(ErrorKind::Other, "WriteFile schlug auf Stdout fehl"));
            }
            Ok(written as usize)
        }
        #[cfg(not(windows))]
        {
            use std::io::Write as _;
            std::io::stdout()
                .write(buf)
                .map_err(|e| IoError::new(ErrorKind::Other, e.to_string()))
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        #[cfg(windows)]
        unsafe {
            let handle = GetStdHandle(STD_ERROR_HANDLE);
            if handle.is_null() || handle as isize == -1 {
                return Err(IoError::new(ErrorKind::Other, "Ungültiger Standard-Error-Handle"));
            }
            let mut written: u32 = 0;
            let len = buf.len().min(u32::MAX as usize) as u32;
            let success = WriteFile(handle, buf.as_ptr(), len, &mut written, std::ptr::null_mut());
            if success == 0 {
                return Err(IoError::new(ErrorKind::Other, "WriteFile schlug auf Stderr fehl"));
            }
            Ok(written as usize)
        }
        #[cfg(not(windows))]
        {
            use std::io::Write as _;
            std::io::stderr()
                .write(buf)
                .map_err(|e| IoError::new(ErrorKind::Other, e.to_string()))
        }
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        #[cfg(windows)]
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            if handle.is_null() || handle as isize == -1 {
                return Err(IoError::new(ErrorKind::Other, "Ungültiger Standard-Input-Handle"));
            }
            let mut read_bytes: u32 = 0;
            let len = buf.len().min(u32::MAX as usize) as u32;
            let success = ReadFile(handle, buf.as_mut_ptr(), len, &mut read_bytes, std::ptr::null_mut());
            if success == 0 {
                return Err(IoError::new(ErrorKind::Other, "ReadFile schlug auf Stdin fehl"));
            }
            Ok(read_bytes as usize)
        }
        #[cfg(not(windows))]
        {
            use std::io::Read as _;
            std::io::stdin()
                .read(buf)
                .map_err(|e| IoError::new(ErrorKind::Other, e.to_string()))
        }
    }
}
