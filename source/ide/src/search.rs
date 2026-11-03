//! Plain-text search over the project's source files.

use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_HITS: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub path: PathBuf,
    pub line: usize,
    pub text: String,
}

/// Case-insensitive substring search. Oversized or unreadable files are skipped,
/// and at most [`MAX_HITS`] results are returned.
pub fn search(files: &[PathBuf], query: &str) -> Vec<Hit> {
    let needle = query.trim().to_lowercase();
    let mut hits = Vec::new();
    if needle.is_empty() {
        return hits;
    }
    for path in files {
        if !fs::metadata(path).is_ok_and(|meta| meta.len() <= MAX_FILE_BYTES) {
            continue;
        }
        let Ok(bytes) = fs::read(path) else { continue };
        let text = String::from_utf8_lossy(&bytes);
        for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
            if line.to_lowercase().contains(&needle) {
                hits.push(Hit {
                    path: path.clone(),
                    line: index + 1,
                    text: line.trim().chars().take(160).collect(),
                });
                if hits.len() >= MAX_HITS {
                    return hits;
                }
            }
        }
    }
    hits
}

pub fn display_name(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("nortex-search-{}-{name}", std::process::id()))
    }

    #[test]
    fn finds_case_insensitive_matches_with_line_numbers_and_strips_bom() {
        let path = temp("a.nx");
        fs::write(&path, "\u{feff}fn Main() {}\nlet größe = 1;\n").unwrap();
        let hits = search(std::slice::from_ref(&path), "GRÖSSE");
        assert!(hits.is_empty(), "ß is not folded to SS");
        let hits = search(std::slice::from_ref(&path), "main");
        assert_eq!((hits[0].line, hits[0].text.as_str()), (1, "fn Main() {}"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn empty_query_missing_files_and_hit_cap_are_safe() {
        assert!(search(&[temp("missing.nx")], "x").is_empty());
        assert!(search(&[], "   ").is_empty());
        let path = temp("many.nx");
        fs::write(&path, "x\n".repeat(MAX_HITS + 50)).unwrap();
        assert_eq!(search(std::slice::from_ref(&path), "x").len(), MAX_HITS);
        let _ = fs::remove_file(&path);
    }
}
