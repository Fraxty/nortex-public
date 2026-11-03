//! Read-only Git status via the `git` executable (absent on some machines).

use std::io::Write;
use std::path::{Component, Path};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub code: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitStatus {
    /// `git` could not be started.
    Unavailable,
    NotARepository,
    Repository {
        branch: String,
        changes: Vec<Change>,
    },
}

/// Parses `git status --porcelain=v1 --branch` output.
pub fn parse_porcelain(output: &str) -> (String, Vec<Change>) {
    let mut branch = String::from("(unbekannt)");
    let mut changes = Vec::new();
    for line in output.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            let name = header.split("...").next().unwrap_or(header);
            branch = name
                .strip_prefix("No commits yet on ")
                .unwrap_or(name)
                .to_string();
        } else if line.len() > 3 {
            changes.push(Change {
                code: line[..2].trim().to_string(),
                path: line[3..].to_string(),
            });
        }
    }
    (branch, changes)
}

pub fn query(root: &Path) -> GitStatus {
    let mut command = git_command(root);
    command.args([
        "-c",
        "core.quotepath=false",
        "--no-optional-locks",
        "status",
        "--porcelain=v1",
        "--branch",
    ]);
    let output = match command.output() {
        Ok(output) => output,
        Err(_) => return GitStatus::Unavailable,
    };
    if !output.status.success() {
        return GitStatus::NotARepository;
    }
    let (branch, changes) = parse_porcelain(&String::from_utf8_lossy(&output.stdout));
    GitStatus::Repository { branch, changes }
}

fn git_command(root: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(root);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    command
}

/// Führt `git` mit festen Argumenten aus (keine Shell); optional mit Eingabe über stdin.
fn run_git(root: &Path, args: &[&str], stdin: Option<&str>) -> Result<String, String> {
    let mut command = git_command(root);
    command
        .args(["-c", "core.quotepath=false", "--no-optional-locks"])
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(input.as_bytes())
            .map_err(|error| error.to_string())?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

/// Nur relative Pfade ohne `..`; Pfade stehen in den Aufrufen stets hinter `--`.
fn checked_path(rel_path: &str) -> Result<&str, String> {
    let path = Path::new(rel_path);
    let valid = !rel_path.is_empty()
        && !rel_path.contains('\0')
        && path.components().all(|c| matches!(c, Component::Normal(_)));
    if valid {
        Ok(rel_path)
    } else {
        Err(format!("ungültiger Pfad: {rel_path}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Context,
    Add,
    Del,
    /// `\ No newline at end of file`
    Meta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: LineKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    pub header: String,
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<DiffLine>,
}

fn parse_range(text: &str) -> Option<(usize, usize)> {
    match text.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((text.parse().ok()?, 1)),
    }
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize, usize, usize)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(" +")?;
    let (new, _) = rest.split_once(" @@")?;
    let (old_start, old_lines) = parse_range(old)?;
    let (new_start, new_lines) = parse_range(new)?;
    Some((old_start, old_lines, new_start, new_lines))
}

/// Zerlegt ein unified diff einer Datei in Hunks (Dateikopf wird übersprungen).
pub fn parse_diff(diff: &str) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = Vec::new();
    for line in diff.lines() {
        if let Some((old_start, old_lines, new_start, new_lines)) = parse_hunk_header(line) {
            hunks.push(Hunk {
                header: line.to_string(),
                old_start,
                old_lines,
                new_start,
                new_lines,
                lines: Vec::new(),
            });
        } else if let Some(hunk) = hunks.last_mut() {
            let (kind, text) = match line.chars().next() {
                Some('+') => (LineKind::Add, &line[1..]),
                Some('-') => (LineKind::Del, &line[1..]),
                Some('\\') => (LineKind::Meta, line),
                Some(' ') => (LineKind::Context, &line[1..]),
                // Beginn der nächsten Datei oder Leerzeile außerhalb des Formats
                _ => continue,
            };
            hunk.lines.push(DiffLine {
                kind,
                text: text.to_string(),
            });
        }
    }
    hunks
}

/// Hunks einer Datei: Arbeitsbaum gegen Index (`staged == false`) bzw. Index gegen HEAD.
pub fn diff_file(root: &Path, rel_path: &str, staged: bool) -> Result<Vec<Hunk>, String> {
    let path = checked_path(rel_path)?;
    let mut args = vec!["diff", "--no-color", "--no-ext-diff", "--unified=3"];
    if staged {
        args.push("--cached");
    }
    args.extend(["--", path]);
    run_git(root, &args, None).map(|output| parse_diff(&output))
}

pub fn stage_file(root: &Path, rel_path: &str) -> Result<(), String> {
    let path = checked_path(rel_path)?;
    run_git(root, &["add", "--", path], None).map(drop)
}

pub fn unstage_file(root: &Path, rel_path: &str) -> Result<(), String> {
    let path = checked_path(rel_path)?;
    if run_git(root, &["rev-parse", "--verify", "-q", "HEAD"], None).is_ok() {
        run_git(root, &["reset", "-q", "HEAD", "--", path], None).map(drop)
    } else {
        run_git(root, &["rm", "--cached", "-q", "--", path], None).map(drop)
    }
}

fn hunk_patch(rel_path: &str, hunk: &Hunk) -> String {
    let mut patch =
        format!("diff --git a/{rel_path} b/{rel_path}\n--- a/{rel_path}\n+++ b/{rel_path}\n");
    patch.push_str(&hunk.header);
    patch.push('\n');
    for line in &hunk.lines {
        match line.kind {
            LineKind::Add => patch.push('+'),
            LineKind::Del => patch.push('-'),
            LineKind::Context => patch.push(' '),
            LineKind::Meta => {}
        }
        patch.push_str(&line.text);
        patch.push('\n');
    }
    patch
}

fn apply_hunk(root: &Path, rel_path: &str, hunk: &Hunk, reverse: bool) -> Result<(), String> {
    let path = checked_path(rel_path)?;
    let mut args = vec!["apply", "--cached", "--whitespace=nowarn"];
    if reverse {
        args.push("--reverse");
    }
    args.push("-");
    run_git(root, &args, Some(&hunk_patch(path, hunk))).map(drop)
}

/// Stellt einen einzelnen Hunk des Arbeitsbaum-Diffs in den Index.
pub fn stage_hunk(root: &Path, rel_path: &str, hunk: &Hunk) -> Result<(), String> {
    apply_hunk(root, rel_path, hunk, false)
}

/// Nimmt einen Hunk des Index-Diffs (`diff_file(.., true)`) aus dem Index.
pub fn unstage_hunk(root: &Path, rel_path: &str, hunk: &Hunk) -> Result<(), String> {
    apply_hunk(root, rel_path, hunk, true)
}

/// Ein Merge-Konfliktbereich; Werte sind nullbasierte Zeilenindizes der Markerzeilen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictRegion {
    pub start_line: usize,
    pub separator_line: usize,
    pub end_line: usize,
    pub ours_label: String,
    pub theirs_label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    Ours,
    Theirs,
    Both,
}

fn marker<'a>(line: &'a str, symbol: &str) -> Option<&'a str> {
    let line = line.trim_end_matches(['\r', '\n']);
    let rest = line.strip_prefix(symbol)?;
    (rest.is_empty() || rest.starts_with(' ')).then_some(rest.trim())
}

/// Findet vollständige `<<<<<<<` / `=======` / `>>>>>>>`-Blöcke (auch diff3 mit `|||||||`).
pub fn parse_conflicts(text: &str) -> Vec<ConflictRegion> {
    let mut regions = Vec::new();
    let mut open: Option<(usize, String, Option<usize>)> = None;
    for (index, line) in text.split_inclusive('\n').enumerate() {
        if let Some(label) = marker(line, "<<<<<<<") {
            open = Some((index, label.to_string(), None));
        } else if let Some((_, _, separator)) = open.as_mut() {
            if separator.is_none() && marker(line, "=======").is_some() {
                *separator = Some(index);
            } else if let (Some(label), Some(separator_line)) =
                (marker(line, ">>>>>>>"), *separator)
            {
                let (start_line, ours_label, _) = open.take().unwrap_or_default();
                regions.push(ConflictRegion {
                    start_line,
                    separator_line,
                    end_line: index,
                    ours_label,
                    theirs_label: label.to_string(),
                });
            }
        }
    }
    regions
}

/// Löst den Bereich `region_index` auf; bei ungültigem Index bleibt der Text unverändert.
pub fn resolve_conflict(text: &str, region_index: usize, resolution: Resolution) -> String {
    let regions = parse_conflicts(text);
    let Some(region) = regions.get(region_index) else {
        return text.to_string();
    };
    let ours_end = (region.start_line + 1..region.separator_line)
        .find(|&index| {
            text.split_inclusive('\n')
                .nth(index)
                .is_some_and(|line| marker(line, "|||||||").is_some())
        })
        .unwrap_or(region.separator_line);
    let mut out = String::with_capacity(text.len());
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let inside = index >= region.start_line && index <= region.end_line;
        if !inside {
            out.push_str(line);
            continue;
        }
        let in_ours = index > region.start_line && index < ours_end;
        let in_theirs = index > region.separator_line && index < region.end_line;
        let keep = match resolution {
            Resolution::Ours => in_ours,
            Resolution::Theirs => in_theirs,
            Resolution::Both => in_ours || in_theirs,
        };
        if keep {
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_repo(name: &str) -> Option<std::path::PathBuf> {
        let dir = std::env::temp_dir().join(format!("nortex-git-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
        run_git(&dir, &["init", "-q"], None).ok()?;
        Some(dir)
    }

    fn commit(dir: &Path, message: &str) {
        run_git(
            dir,
            &[
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                message,
            ],
            None,
        )
        .unwrap();
    }

    #[test]
    fn diff_stage_and_unstage_single_hunk() {
        let Some(dir) = temp_repo("hunk") else {
            return;
        };
        let original: String = (1..=30).map(|n| format!("line {n}\n")).collect();
        std::fs::write(dir.join("a.txt"), &original).unwrap();
        run_git(&dir, &["add", "--", "a.txt"], None).unwrap();
        commit(&dir, "init");
        let changed = original
            .replace("line 2\n", "line two\n")
            .replace("line 28\n", "line 28\nextra\n");
        std::fs::write(dir.join("a.txt"), &changed).unwrap();

        let hunks = diff_file(&dir, "a.txt", false).unwrap();
        assert_eq!(hunks.len(), 2);
        assert!(
            hunks[0]
                .lines
                .iter()
                .any(|l| l.kind == LineKind::Del && l.text == "line 2")
        );
        assert!(
            hunks[0]
                .lines
                .iter()
                .any(|l| l.kind == LineKind::Add && l.text == "line two")
        );
        assert_eq!(hunks[1].old_start, 26);

        stage_hunk(&dir, "a.txt", &hunks[1]).unwrap();
        let staged = diff_file(&dir, "a.txt", true).unwrap();
        assert_eq!(staged.len(), 1);
        assert!(staged[0].lines.iter().any(|l| l.text == "extra"));
        assert_eq!(diff_file(&dir, "a.txt", false).unwrap().len(), 1);

        unstage_hunk(&dir, "a.txt", &staged[0]).unwrap();
        assert!(diff_file(&dir, "a.txt", true).unwrap().is_empty());
        assert_eq!(diff_file(&dir, "a.txt", false).unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stage_and_unstage_file_including_unborn_head() {
        let Some(dir) = temp_repo("file") else {
            return;
        };
        std::fs::write(dir.join("new.txt"), "x\n").unwrap();
        stage_file(&dir, "new.txt").unwrap();
        let Ok(output) = run_git(&dir, &["status", "--porcelain"], None) else {
            return;
        };
        assert!(output.starts_with("A "));
        unstage_file(&dir, "new.txt").unwrap();
        let output = run_git(&dir, &["status", "--porcelain"], None).unwrap();
        assert!(output.starts_with("??"));
        assert!(stage_file(&dir, "../x").is_err());
        assert!(stage_file(&dir, "").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parses_hunk_headers_and_no_newline_marker() {
        let diff = "diff --git a/f b/f\n--- a/f\n+++ b/f\n@@ -1 +1,2 @@ fn\n-a\n+b\n+c\n\\ No newline at end of file\n";
        let hunks = parse_diff(diff);
        assert_eq!(hunks.len(), 1);
        assert_eq!(
            (hunks[0].old_start, hunks[0].old_lines, hunks[0].new_lines),
            (1, 1, 2)
        );
        assert_eq!(hunks[0].lines.len(), 4);
        assert_eq!(hunks[0].lines[3].kind, LineKind::Meta);
    }

    const CONFLICT: &str = "a\n<<<<<<< HEAD\nours1\nours2\n=======\ntheirs\n>>>>>>> feature\nb\n<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> feature\n";

    #[test]
    fn parses_conflict_regions() {
        let regions = parse_conflicts(CONFLICT);
        assert_eq!(regions.len(), 2);
        assert_eq!(
            (
                regions[0].start_line,
                regions[0].separator_line,
                regions[0].end_line
            ),
            (1, 4, 6)
        );
        assert_eq!(regions[0].ours_label, "HEAD");
        assert_eq!(regions[0].theirs_label, "feature");
        assert!(parse_conflicts("<<<<<<< a\nx\n=======\ny\n").is_empty());
    }

    #[test]
    fn resolves_conflicts() {
        assert_eq!(
            resolve_conflict(CONFLICT, 0, Resolution::Ours),
            "a\nours1\nours2\nb\n<<<<<<< HEAD\nx\n=======\ny\n>>>>>>> feature\n"
        );
        assert_eq!(
            resolve_conflict(CONFLICT, 1, Resolution::Theirs),
            "a\n<<<<<<< HEAD\nours1\nours2\n=======\ntheirs\n>>>>>>> feature\nb\ny\n"
        );
        let both = resolve_conflict(
            &resolve_conflict(CONFLICT, 0, Resolution::Both),
            0,
            Resolution::Both,
        );
        assert_eq!(both, "a\nours1\nours2\ntheirs\nb\nx\ny\n");
        assert_eq!(resolve_conflict(CONFLICT, 9, Resolution::Ours), CONFLICT);
        let diff3 = "<<<<<<< HEAD\nA\n||||||| base\nB\n=======\nC\n>>>>>>> t\n";
        assert_eq!(resolve_conflict(diff3, 0, Resolution::Ours), "A\n");
    }

    #[test]
    fn parses_branch_and_changes() {
        let (branch, changes) =
            parse_porcelain("## main...origin/main\n M src/a.rs\n?? new file.nx\nA  b.nx\n");
        assert_eq!(branch, "main");
        assert_eq!(changes.len(), 3);
        assert_eq!(
            changes[0],
            Change {
                code: "M".into(),
                path: "src/a.rs".into()
            }
        );
        assert_eq!(changes[1].path, "new file.nx");
        assert_eq!(changes[2].code, "A");
    }

    #[test]
    fn handles_fresh_repository_and_empty_output() {
        assert_eq!(parse_porcelain("## No commits yet on master\n").0, "master");
        let (branch, changes) = parse_porcelain("");
        assert_eq!(branch, "(unbekannt)");
        assert!(changes.is_empty());
    }

    #[test]
    fn query_outside_any_repository_never_panics() {
        let status = query(&std::env::temp_dir().join("nortex-no-such-dir-xyz"));
        assert!(matches!(
            status,
            GitStatus::Unavailable | GitStatus::NotARepository
        ));
    }
}
