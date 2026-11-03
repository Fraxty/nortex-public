//! `.nxpack` creation for hosted plugins: a standard ZIP archive with Deflate
//! compression, built in memory under hard limits and written atomically.

use std::collections::HashSet;
use std::io::{Cursor, Write};
use std::path::Path;

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::project_io::{read_project_file, validate_relative, write_project_file};

pub const EXTENSION: &str = "nxpack";
pub const MAX_ARCHIVE_ENTRIES: usize = 200;
pub const MAX_ENTRY_BYTES: u64 = 4 * 1024 * 1024;
/// Sum of all uncompressed entries.
pub const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_ARCHIVE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveSummary {
    pub path: String,
    pub entries: usize,
    pub uncompressed_bytes: u64,
    pub archive_bytes: u64,
}

/// Compresses `entries` (`/`-separated names, unique, already validated) into a ZIP.
pub fn build_zip(entries: &[(String, Vec<u8>)]) -> Result<Vec<u8>, &'static str> {
    if entries.is_empty() {
        return Err("archive has no files");
    }
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in entries {
        writer
            .start_file(name.as_str(), options)
            .map_err(|_| "archive cannot be built")?;
        writer
            .write_all(data)
            .map_err(|_| "archive cannot be built")?;
    }
    let bytes = writer
        .finish()
        .map_err(|_| "archive cannot be built")?
        .into_inner();
    if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
        return Err("archive exceeds the size limit");
    }
    Ok(bytes)
}

struct EntrySet<'a> {
    output: &'a str,
    entries: Vec<(String, Vec<u8>)>,
    seen: HashSet<String>,
    total: u64,
}

impl EntrySet<'_> {
    fn admit(&mut self, name: String, data: Vec<u8>) -> Result<(), &'static str> {
        validate_relative(&name)?;
        if name.eq_ignore_ascii_case(self.output) {
            return Err("archive cannot contain itself");
        }
        if !self.seen.insert(name.to_lowercase()) {
            return Err("duplicate file in archive");
        }
        self.total += data.len() as u64;
        if self.total > MAX_TOTAL_BYTES {
            return Err("files exceed the total size limit");
        }
        self.entries.push((name, data));
        Ok(())
    }
}

/// Builds `output` (a project-relative `.nxpack` path) from project `files` and an
/// optional extra in-memory entry (the active document). Nothing is written when any
/// path, size or count limit is violated.
pub fn create(
    root: &Path,
    output: &str,
    files: &[String],
    extra: Option<(String, Vec<u8>)>,
) -> Result<ArchiveSummary, &'static str> {
    validate_relative(output)?;
    let total_entries = files.len() + usize::from(extra.is_some());
    if total_entries == 0 {
        return Err("archive has no files");
    }
    if total_entries > MAX_ARCHIVE_ENTRIES {
        return Err("too many files for one archive");
    }

    let mut set = EntrySet {
        output,
        entries: Vec::with_capacity(total_entries),
        seen: HashSet::new(),
        total: 0,
    };
    let mut extra_name = None;
    if let Some((name, data)) = extra {
        if data.len() as u64 > MAX_ENTRY_BYTES {
            return Err("file exceeds the size limit");
        }
        extra_name = Some(name.to_lowercase());
        set.admit(name, data)?;
    }
    for file in files {
        // The extra entry carries the unsaved text and wins over the file on disk.
        if extra_name.as_deref() == Some(file.to_lowercase().as_str()) {
            continue;
        }
        let data = read_project_file(root, file, MAX_ENTRY_BYTES)?;
        set.admit(file.clone(), data)?;
    }
    let EntrySet { entries, total, .. } = set;
    let bytes = build_zip(&entries)?;
    write_project_file(root, output, EXTENSION, &bytes)?;
    Ok(ArchiveSummary {
        path: output.to_owned(),
        entries: entries.len(),
        uncompressed_bytes: total,
        archive_bytes: bytes.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Read;

    use super::super::project_io::test_support::TempProject;
    use super::*;

    fn read_back(path: &Path) -> Vec<(String, Vec<u8>, CompressionMethod)> {
        let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        (0..archive.len())
            .map(|index| {
                let mut file = archive.by_index(index).unwrap();
                let mut data = Vec::new();
                file.read_to_end(&mut data).unwrap();
                (file.name().to_owned(), data, file.compression())
            })
            .collect()
    }

    #[test]
    fn creates_a_real_deflate_zip_that_round_trips() {
        let project = TempProject::new("zip");
        let text = "nortex ".repeat(2000);
        fs::write(project.0.join("a.nx"), &text).unwrap();
        fs::create_dir(project.0.join("src")).unwrap();
        fs::write(project.0.join("src").join("b.bin"), [0u8, 159, 146, 150]).unwrap();

        let summary = create(
            &project.0,
            "out.nxpack",
            &["a.nx".into(), "src/b.bin".into()],
            None,
        )
        .unwrap();
        assert_eq!(summary.entries, 2);
        assert_eq!(summary.uncompressed_bytes, text.len() as u64 + 4);
        assert!(summary.archive_bytes < text.len() as u64);

        let raw = fs::read(project.0.join("out.nxpack")).unwrap();
        assert_eq!(&raw[..4], b"PK\x03\x04");
        let entries = read_back(&project.0.join("out.nxpack"));
        assert_eq!(entries[0].0, "a.nx");
        assert_eq!(entries[0].1, text.as_bytes());
        assert_eq!(entries[0].2, CompressionMethod::Deflated);
        assert_eq!(entries[1].0, "src/b.bin");
        assert_eq!(entries[1].1, [0u8, 159, 146, 150]);
    }

    #[test]
    fn active_document_text_replaces_the_disk_version() {
        let project = TempProject::new("active");
        fs::write(project.0.join("a.nx"), "old").unwrap();
        let summary = create(
            &project.0,
            "doc.nxpack",
            &["a.nx".into()],
            Some(("a.nx".into(), b"unsaved".to_vec())),
        )
        .unwrap();
        assert_eq!(summary.entries, 1);
        let entries = read_back(&project.0.join("doc.nxpack"));
        assert_eq!(entries[0].1, b"unsaved");
    }

    #[test]
    fn violations_fail_closed_and_leave_no_output() {
        let project = TempProject::new("limits");
        fs::write(project.0.join("a.txt"), "a").unwrap();
        let big = vec![0u8; MAX_ENTRY_BYTES as usize + 1];
        fs::write(project.0.join("big.bin"), &big).unwrap();
        let cases: Vec<(&str, Vec<String>)> = vec![
            ("out.nxpack", vec![]),
            ("out.nxpack", vec!["../a.txt".into()]),
            ("out.nxpack", vec!["missing.txt".into()]),
            ("out.nxpack", vec!["big.bin".into()]),
            ("out.nxpack", vec!["a.txt".into(), "A.TXT".into()]),
            ("out.nxpack", vec!["out.nxpack".into()]),
            ("out.zip", vec!["a.txt".into()]),
            ("../out.nxpack", vec!["a.txt".into()]),
            (
                "out.nxpack",
                (0..=MAX_ARCHIVE_ENTRIES).map(|n| format!("f{n}")).collect(),
            ),
        ];
        for (output, files) in cases {
            assert!(
                create(&project.0, output, &files, None).is_err(),
                "{output} {files:?}"
            );
        }
        assert!(!project.0.join("out.nxpack").exists());
        assert!(!project.0.join("out.zip").exists());
    }

    #[test]
    fn total_uncompressed_size_is_capped() {
        let project = TempProject::new("total");
        let chunk = vec![7u8; MAX_ENTRY_BYTES as usize];
        let count = (MAX_TOTAL_BYTES / MAX_ENTRY_BYTES) as usize + 1;
        let names: Vec<String> = (0..count).map(|n| format!("c{n}.bin")).collect();
        for name in &names {
            fs::write(project.0.join(name), &chunk).unwrap();
        }
        assert_eq!(
            create(&project.0, "out.nxpack", &names, None),
            Err("files exceed the total size limit")
        );
        assert!(!project.0.join("out.nxpack").exists());
    }
}
