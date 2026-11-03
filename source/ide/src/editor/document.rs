//! Datei-Dokumente für NORTEX Code. UTF-8 und ein vorhandener UTF-8-BOM
//! werden beim Speichern erhalten; Zeilenenden bleiben unverändert.

use super::EditorBuffer;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_FILE_ID: AtomicU64 = AtomicU64::new(1);
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

#[derive(Debug)]
pub enum DocumentError {
    Io(io::Error),
    InvalidUtf8(std::string::FromUtf8Error),
    BinaryFile,
    MissingPath,
    LossySaveNotConfirmed,
}

impl std::fmt::Display for DocumentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "Dateifehler: {error}"),
            Self::InvalidUtf8(error) => write!(formatter, "Datei ist kein gültiges UTF-8: {error}"),
            Self::BinaryFile => write!(
                formatter,
                "Die Datei enthält Nullbytes und wird als Binärdatei behandelt."
            ),
            Self::MissingPath => write!(formatter, "Das Dokument besitzt noch keinen Dateipfad."),
            Self::LossySaveNotConfirmed => write!(
                formatter,
                "Die Datei wurde verlustbehaftet geöffnet. Bestätige vor dem Speichern ausdrücklich die Umwandlung nach UTF-8."
            ),
        }
    }
}

impl std::error::Error for DocumentError {}

impl From<io::Error> for DocumentError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
    Cr,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }
}

pub struct EditorDocument {
    path: Option<PathBuf>,
    buffer: EditorBuffer,
    has_utf8_bom: bool,
    line_ending: LineEnding,
    saved_state_id: u64,
    lossy_decode: bool,
    lossy_save_confirmed: bool,
}

impl EditorDocument {
    pub fn untitled() -> Self {
        let buffer = EditorBuffer::new();
        let saved_state_id = buffer.state_id();
        Self {
            path: None,
            buffer,
            has_utf8_bom: false,
            line_ending: default_line_ending(),
            saved_state_id,
            lossy_decode: false,
            lossy_save_confirmed: false,
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, DocumentError> {
        let path = fs::canonicalize(path.as_ref())?;
        let bytes = fs::read(&path)?;
        reject_binary_sample(&bytes)?;
        let has_utf8_bom = bytes.starts_with(UTF8_BOM);
        let content = if has_utf8_bom {
            &bytes[UTF8_BOM.len()..]
        } else {
            &bytes[..]
        };
        let text = String::from_utf8(content.to_vec()).map_err(DocumentError::InvalidUtf8)?;
        Self::from_decoded(path, text, has_utf8_bom, false)
    }

    /// Öffnet beliebige Bytes explizit verlustbehaftet als UTF-8.
    /// Vor dem Speichern muss die Umwandlung ausdrücklich bestätigt werden.
    pub fn open_lossy(path: impl AsRef<Path>) -> Result<Self, DocumentError> {
        let path = fs::canonicalize(path.as_ref())?;
        let bytes = fs::read(&path)?;
        reject_binary_sample(&bytes)?;
        let has_utf8_bom = bytes.starts_with(UTF8_BOM);
        let content = if has_utf8_bom {
            &bytes[UTF8_BOM.len()..]
        } else {
            &bytes[..]
        };
        let lossy_decode = std::str::from_utf8(content).is_err();
        let text = String::from_utf8_lossy(content).into_owned();
        Self::from_decoded(path, text, has_utf8_bom, lossy_decode)
    }

    fn from_decoded(
        path: PathBuf,
        text: String,
        has_utf8_bom: bool,
        lossy_decode: bool,
    ) -> Result<Self, DocumentError> {
        let line_ending = detect_line_ending(&text);
        let buffer = EditorBuffer::from_text(text);
        let saved_state_id = buffer.state_id();
        Ok(Self {
            path: Some(path),
            buffer,
            has_utf8_bom,
            line_ending,
            saved_state_id,
            lossy_decode,
            lossy_save_confirmed: false,
        })
    }

    pub fn confirm_lossy_conversion(&mut self) {
        self.lossy_save_confirmed = true;
    }

    pub fn was_lossy_decoded(&self) -> bool {
        self.lossy_decode
    }

    pub fn save(&mut self) -> Result<(), DocumentError> {
        if self.lossy_decode && !self.lossy_save_confirmed {
            return Err(DocumentError::LossySaveNotConfirmed);
        }
        let path = self.path.clone().ok_or(DocumentError::MissingPath)?;
        self.write_to(&path)
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<(), DocumentError> {
        if self.lossy_decode && !self.lossy_save_confirmed {
            return Err(DocumentError::LossySaveNotConfirmed);
        }
        let path = path.as_ref().to_path_buf();
        self.write_to(&path)?;
        self.path = Some(fs::canonicalize(&path).unwrap_or(path));
        Ok(())
    }

    fn write_to(&mut self, path: &Path) -> Result<(), DocumentError> {
        let has_utf8_bom = self.has_utf8_bom;
        atomic_write(path, |file| {
            if has_utf8_bom {
                file.write_all(UTF8_BOM)?;
            }
            self.buffer.write_to(file)
        })?;
        self.saved_state_id = self.buffer.state_id();
        self.lossy_decode = false;
        self.lossy_save_confirmed = false;
        Ok(())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }
    pub fn newline_text(&self) -> &'static str {
        self.line_ending.as_str()
    }
    pub fn buffer(&self) -> &EditorBuffer {
        &self.buffer
    }
    pub fn buffer_mut(&mut self) -> &mut EditorBuffer {
        &mut self.buffer
    }
    pub fn is_modified(&self) -> bool {
        self.buffer.state_id() != self.saved_state_id
    }
}

fn reject_binary_sample(bytes: &[u8]) -> Result<(), DocumentError> {
    // Nullbytes im ersten Block sind ein starkes Indiz für Binärdaten.
    // UTF-8-Textdateien bleiben davon unberührt.
    if bytes.iter().take(8 * 1024).any(|byte| *byte == 0) {
        return Err(DocumentError::BinaryFile);
    }
    Ok(())
}

fn detect_line_ending(text: &str) -> LineEnding {
    let bytes = text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        match *byte {
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => return LineEnding::CrLf,
            b'\r' => return LineEnding::Cr,
            b'\n' => return LineEnding::Lf,
            _ => {}
        }
    }
    default_line_ending()
}

fn default_line_ending() -> LineEnding {
    if cfg!(windows) {
        LineEnding::CrLf
    } else {
        LineEnding::Lf
    }
}

fn atomic_write<F>(path: &Path, write_contents: F) -> io::Result<()>
where
    F: FnOnce(&mut File) -> io::Result<()>,
{
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("document"))
        .to_string_lossy();
    let (temporary, mut file) = {
        let mut last_collision = None;
        let mut created = None;
        for _ in 0..32 {
            let id = TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed);
            let candidate = parent.join(format!(
                ".{file_name}.nortex-tmp-{}-{id}",
                std::process::id()
            ));
            match create_temp_file(&candidate, path) {
                Ok(file) => {
                    created = Some((candidate, file));
                    break;
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    last_collision = Some(error)
                }
                Err(error) => return Err(error),
            }
        }
        created.ok_or_else(|| {
            last_collision.unwrap_or_else(|| {
                io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Kein eindeutiger temporärer Dateiname verfügbar",
                )
            })
        })?
    };

    let result = (|| {
        write_contents(&mut file)?;
        file.sync_all()?;
        drop(file);
        replace_file(&temporary, path)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(unix)]
fn create_temp_file(temporary: &Path, destination: &Path) -> io::Result<File> {
    use std::os::unix::fs::MetadataExt;
    // Mode beim Anlegen setzen: Eine bestehende private Datei wird nie kurzzeitig
    // mit den großzügigeren Standardrechten des Prozesses angelegt.
    let mode = match fs::metadata(destination) {
        Ok(metadata) => metadata.mode() & 0o777,
        Err(error) if error.kind() == io::ErrorKind::NotFound => 0o666,
        Err(error) => return Err(error),
    };
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(temporary)
}

#[cfg(not(unix))]
fn create_temp_file(temporary: &Path, _destination: &Path) -> io::Result<File> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)
}

#[cfg(unix)]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x0000_0001;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x0000_0008;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(
            existing_file_name: *const u16,
            new_file_name: *const u16,
            flags: u32,
        ) -> i32;
    }
    let succeeded = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if succeeded == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(source, destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("nortex-{name}-{}-{nonce}.nx", std::process::id()))
    }

    #[test]
    fn open_save_preserves_bom_and_crlf() {
        let path = temp_path("document");
        fs::write(&path, b"\xEF\xBB\xBFline1\r\nline2\r\n").unwrap();
        let mut document = EditorDocument::open(&path).unwrap();
        assert_eq!(document.line_ending(), LineEnding::CrLf);
        assert_eq!(document.newline_text(), "\r\n");
        assert!(!document.is_modified());
        let end = document.buffer().len_bytes();
        document.buffer_mut().move_cursor(end);
        document.buffer_mut().insert("line3");
        assert!(document.is_modified());
        document.save().unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            b"\xEF\xBB\xBFline1\r\nline2\r\nline3"
        );
        assert!(!document.is_modified());
        document.buffer_mut().move_cursor(0);
        document.buffer_mut().insert("X");
        assert!(document.is_modified());
        assert!(document.buffer_mut().undo());
        assert!(!document.is_modified());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn untitled_document_uses_platform_default_and_detects_lf_files() {
        let document = EditorDocument::untitled();
        assert_eq!(
            document.newline_text(),
            if cfg!(windows) { "\r\n" } else { "\n" }
        );
        let path = temp_path("lf-line-ending");
        fs::write(&path, b"first\nsecond\n").unwrap();
        let opened = EditorDocument::open(&path).unwrap();
        assert_eq!(opened.line_ending(), LineEnding::Lf);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn new_lines_follow_the_opened_documents_line_ending() {
        let path = temp_path("newline-style");
        fs::write(&path, b"first\r\nsecond").unwrap();
        let mut document = EditorDocument::open(&path).unwrap();
        let newline = document.newline_text().to_owned();
        let end = document.buffer().len_bytes();
        document.buffer_mut().move_cursor(end);
        document.buffer_mut().insert(&newline);
        document.save().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first\r\nsecond\r\n");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn save_as_assigns_path_only_after_success() {
        let mut document = EditorDocument::untitled();
        document.buffer_mut().insert("hello");
        let path = temp_path("save-as");
        document.save_as(&path).unwrap();
        let canonical_path = fs::canonicalize(&path).unwrap();
        assert_eq!(document.path(), Some(canonical_path.as_path()));
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn detects_and_preserves_cr_only_line_endings() {
        let path = temp_path("cr-only");
        fs::write(&path, b"first\rsecond\rthird").unwrap();
        let mut document = EditorDocument::open(&path).unwrap();
        assert_eq!(document.line_ending(), LineEnding::Cr);
        assert_eq!(document.buffer().line_count(), 3);
        assert_eq!(document.buffer().line(1).as_deref(), Some("second"));
        let end = document.buffer().len_bytes();
        document.buffer_mut().move_cursor(end);
        let newline = document.newline_text().to_owned();
        document.buffer_mut().insert(&newline);
        document.buffer_mut().insert("fourth");
        document.save().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first\rsecond\rthird\rfourth");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn lossy_open_requires_confirmation_before_overwriting_original_bytes() {
        let path = temp_path("lossy");
        fs::write(&path, [0xFF, b'a']).unwrap();
        assert!(matches!(
            EditorDocument::open(&path),
            Err(DocumentError::InvalidUtf8(_))
        ));
        let mut document = EditorDocument::open_lossy(&path).unwrap();
        assert!(document.was_lossy_decoded());
        assert!(matches!(
            document.save(),
            Err(DocumentError::LossySaveNotConfirmed)
        ));
        assert_eq!(fs::read(&path).unwrap(), vec![0xFF, b'a']);
        document.confirm_lossy_conversion();
        document.save().unwrap();
        assert!(!document.was_lossy_decoded());
        assert_eq!(fs::read_to_string(&path).unwrap(), "�a");
        fs::remove_file(path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn atomic_save_preserves_restrictive_unix_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let path = temp_path("permissions");
        fs::write(&path, b"secret").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let mut document = EditorDocument::open(&path).unwrap();
        document.buffer_mut().move_cursor(0);
        document.buffer_mut().insert("updated-");
        document.save().unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(fs::read_to_string(&path).unwrap(), "updated-secret");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn failed_atomic_write_keeps_the_original_file_intact() {
        let path = temp_path("atomic-write-failure");
        fs::write(&path, b"original").unwrap();
        let result = atomic_write(&path, |file| {
            file.write_all(b"partial replacement")?;
            Err(io::Error::other("injected write failure"))
        });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        let prefix = format!(
            ".{}.nortex-tmp-",
            path.file_name().unwrap().to_string_lossy()
        );
        let leftovers = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .any(|entry| entry.file_name().to_string_lossy().starts_with(&prefix));
        assert!(
            !leftovers,
            "temporary file must be removed after a failed write"
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_binary_files_with_null_bytes() {
        let path = temp_path("binary");
        fs::write(&path, [b'M', b'Z', 0, 0, 1, 2]).unwrap();
        assert!(matches!(
            EditorDocument::open(&path),
            Err(DocumentError::BinaryFile)
        ));
        assert!(matches!(
            EditorDocument::open_lossy(&path),
            Err(DocumentError::BinaryFile)
        ));
        fs::remove_file(path).unwrap();
    }
}
