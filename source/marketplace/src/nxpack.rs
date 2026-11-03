//! ZIP-based `.nxpack` archives. Unpacking returns validated file paths and bytes
//! in memory; callers are responsible for any later filesystem writes.

use std::collections::HashSet;
use std::io::{Cursor, Read};

use zip::write::SimpleFileOptions;

use crate::safe_archive_path;

pub const MAX_NXPACK_ENTRIES: usize = 5_000;
pub const MAX_NXPACK_UNCOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_NXPACK_ARCHIVE_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NxpackEntry {
    pub path: String,
    pub data: Vec<u8>,
}

/// Creates a standard Deflate-compressed ZIP archive suitable for a `.nxpack` file.
pub fn pack_nxpack(entries: &[NxpackEntry]) -> Result<Vec<u8>, String> {
    pack_nxpack_with_limit(entries, MAX_NXPACK_UNCOMPRESSED_BYTES)
}

fn pack_nxpack_with_limit(
    entries: &[NxpackEntry],
    max_uncompressed_bytes: u64,
) -> Result<Vec<u8>, String> {
    if entries.len() > MAX_NXPACK_ENTRIES {
        return Err(format!(
            "nxpack may contain at most {MAX_NXPACK_ENTRIES} files"
        ));
    }

    let mut total = 0u64;
    let mut names = HashSet::with_capacity(entries.len());
    for entry in entries {
        if !safe_archive_path(&entry.path) || entry.path.ends_with('/') {
            return Err(format!("unsafe nxpack path: {:?}", entry.path));
        }
        if !names.insert(entry.path.as_str()) {
            return Err("duplicate nxpack path".into());
        }
        total = total
            .checked_add(entry.data.len() as u64)
            .ok_or_else(|| "nxpack uncompressed size overflow".to_string())?;
        if total > max_uncompressed_bytes {
            return Err("nxpack exceeds the uncompressed size limit".into());
        }
    }

    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for entry in entries {
        writer
            .start_file(&entry.path, options)
            .map_err(|_| "cannot create nxpack entry".to_string())?;
        std::io::Write::write_all(&mut writer, &entry.data)
            .map_err(|_| "cannot write nxpack entry".to_string())?;
    }
    let archive = writer
        .finish()
        .map_err(|_| "cannot finish nxpack archive".to_string())?
        .into_inner();
    if archive.len() > MAX_NXPACK_ARCHIVE_BYTES {
        return Err("nxpack exceeds the compressed archive size limit".into());
    }
    Ok(archive)
}

/// Reads a `.nxpack` ZIP archive after validating paths, entry types and expanded size.
pub fn unpack_nxpack(data: &[u8]) -> Result<Vec<NxpackEntry>, String> {
    unpack_nxpack_with_limits(
        data,
        MAX_NXPACK_ENTRIES,
        MAX_NXPACK_UNCOMPRESSED_BYTES,
        MAX_NXPACK_ARCHIVE_BYTES,
    )
}

fn unpack_nxpack_with_limits(
    data: &[u8],
    max_entries: usize,
    max_uncompressed_bytes: u64,
    max_archive_bytes: usize,
) -> Result<Vec<NxpackEntry>, String> {
    if data.len() > max_archive_bytes {
        return Err("nxpack exceeds the compressed archive size limit".into());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(data))
        .map_err(|_| "nxpack is not a valid zip archive".to_string())?;
    if archive.len() > max_entries {
        return Err(format!("nxpack may contain at most {max_entries} entries"));
    }

    let mut declared_total = 0u64;
    let mut names = HashSet::with_capacity(archive.len());
    for index in 0..archive.len() {
        let file = archive
            .by_index_raw(index)
            .map_err(|_| "unreadable nxpack entry".to_string())?;
        let name = file.name();
        if !safe_archive_path(name) {
            return Err(format!("unsafe nxpack path: {name:?}"));
        }
        if !names.insert(name.to_string()) {
            return Err("duplicate nxpack path".into());
        }
        if matches!(file.unix_mode(), Some(mode) if mode & 0o170000 == 0o120000) {
            return Err("symbolic links are not allowed in nxpack files".into());
        }
        declared_total = declared_total
            .checked_add(file.size())
            .ok_or_else(|| "nxpack uncompressed size overflow".to_string())?;
        if declared_total > max_uncompressed_bytes {
            return Err("nxpack exceeds the uncompressed size limit".into());
        }
        if file.is_dir() && file.size() != 0 {
            return Err("nxpack directory entries must be empty".into());
        }
    }

    let mut entries = Vec::with_capacity(archive.len());
    let mut actual_total = 0u64;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|_| "cannot decompress nxpack entry".to_string())?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_string();
        if name.ends_with('/') {
            return Err(format!("invalid nxpack file path: {name:?}"));
        }
        let remaining = max_uncompressed_bytes - actual_total;
        let mut content = Vec::new();
        (&mut file)
            .take(remaining + 1)
            .read_to_end(&mut content)
            .map_err(|_| "cannot read nxpack entry".to_string())?;
        if content.len() as u64 > remaining {
            return Err("nxpack exceeds the uncompressed size limit".into());
        }
        if content.len() as u64 != file.size() {
            return Err("nxpack entry size does not match its directory record".into());
        }
        actual_total += content.len() as u64;
        entries.push(NxpackEntry {
            path: name,
            data: content,
        });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn pack_is_a_zip_and_round_trips_nested_binary_files() {
        let files = vec![
            NxpackEntry {
                path: "src/main.nx".into(),
                data: b"fn main() {}".to_vec(),
            },
            NxpackEntry {
                path: "assets/icon.bin".into(),
                data: vec![0, 1, 2, 255],
            },
        ];
        let packed = pack_nxpack(&files).unwrap();
        assert_eq!(&packed[..4], b"PK\x03\x04");
        assert_eq!(unpack_nxpack(&packed).unwrap(), files);
    }

    #[test]
    fn packing_rejects_traversal_duplicates_and_oversized_input() {
        for path in ["../escape", "/absolute", "C:/drive", "folder\\file"] {
            let files = [NxpackEntry {
                path: path.into(),
                data: vec![],
            }];
            assert!(pack_nxpack(&files).is_err(), "accepted {path:?}");
        }
        let duplicate = [
            NxpackEntry {
                path: "same.txt".into(),
                data: vec![],
            },
            NxpackEntry {
                path: "same.txt".into(),
                data: vec![],
            },
        ];
        assert!(pack_nxpack(&duplicate).is_err());
        assert!(pack_nxpack_with_limit(
            &[NxpackEntry {
                path: "large.bin".into(),
                data: vec![0; 5],
            }],
            4
        )
        .unwrap_err()
        .contains("size limit"));
    }

    #[test]
    fn unpack_rejects_traversal_and_enforces_entry_and_size_limits() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("../escape", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"bad").unwrap();
        let traversal = writer.finish().unwrap().into_inner();
        assert!(unpack_nxpack(&traversal)
            .unwrap_err()
            .contains("unsafe nxpack path"));

        let files = [
            NxpackEntry {
                path: "one.txt".into(),
                data: b"one".to_vec(),
            },
            NxpackEntry {
                path: "two.txt".into(),
                data: b"two".to_vec(),
            },
        ];
        let packed = pack_nxpack(&files).unwrap();
        assert!(
            unpack_nxpack_with_limits(&packed, 1, 100, MAX_NXPACK_ARCHIVE_BYTES)
                .unwrap_err()
                .contains("at most 1")
        );
        assert!(
            unpack_nxpack_with_limits(&packed, 10, 5, MAX_NXPACK_ARCHIVE_BYTES)
                .unwrap_err()
                .contains("size limit")
        );
        assert!(unpack_nxpack_with_limits(&packed, 10, 100, 1)
            .unwrap_err()
            .contains("archive size limit"));
    }
}
