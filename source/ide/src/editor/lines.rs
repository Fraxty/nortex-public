//! Zeilenoperationen auf Zeichenindizes (Kommentar, Verschieben, Duplizieren, Auswahl).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineEdit {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

fn split_lines(text: &str) -> Vec<Vec<char>> {
    text.split('\n')
        .map(|line| line.chars().collect())
        .collect()
}

fn join(lines: &[Vec<char>]) -> String {
    lines
        .iter()
        .map(|line| line.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn locate(lines: &[Vec<char>], index: usize) -> (usize, usize) {
    let mut remaining = index;
    for (number, line) in lines.iter().enumerate() {
        if remaining <= line.len() {
            return (number, remaining);
        }
        remaining -= line.len() + 1;
    }
    let last = lines.len().saturating_sub(1);
    (last, lines.get(last).map_or(0, Vec::len))
}

fn offset(lines: &[Vec<char>], line: usize, column: usize) -> usize {
    lines.iter().take(line).map(|l| l.len() + 1).sum::<usize>() + column
}

/// Betroffene Zeilen; endet die Auswahl in Spalte 0 einer späteren Zeile, zählt diese nicht mit.
fn line_range(lines: &[Vec<char>], start: usize, end: usize) -> (usize, usize) {
    let (first, _) = locate(lines, start.min(end));
    let (mut last, column) = locate(lines, start.max(end));
    if last > first && column == 0 {
        last -= 1;
    }
    (first, last)
}

fn indent_of(line: &[char]) -> usize {
    line.iter()
        .take_while(|c| **c == ' ' || **c == '\t')
        .count()
}

pub fn toggle_comment(text: &str, start: usize, end: usize) -> LineEdit {
    let mut lines = split_lines(text);
    let (first, last) = line_range(&lines, start, end);
    let body: Vec<usize> = (first..=last)
        .filter(|n| lines[*n].iter().any(|c| !c.is_whitespace()))
        .collect();
    let commented = !body.is_empty()
        && body.iter().all(|n| {
            let line = &lines[*n];
            line[indent_of(line)..].starts_with(&['/', '/'])
        });
    let column = body
        .iter()
        .map(|n| indent_of(&lines[*n]))
        .min()
        .unwrap_or(0);
    let mut deltas = vec![0isize; lines.len()];
    for number in body {
        let line = &mut lines[number];
        if commented {
            let at = indent_of(line);
            let remove = if line.get(at + 2) == Some(&' ') { 3 } else { 2 };
            line.drain(at..at + remove);
            deltas[number] = -(remove as isize);
        } else {
            line.splice(column..column, "// ".chars());
            deltas[number] = 3;
        }
    }
    let shift = |index: usize, lines_after: &[Vec<char>]| -> usize {
        let (number, col) = locate(&split_lines(text), index);
        let delta = deltas[number];
        let new_col = if delta >= 0 {
            if col >= column {
                col + delta as usize
            } else {
                col
            }
        } else {
            col.saturating_sub((-delta) as usize)
        };
        offset(lines_after, number, new_col.min(lines_after[number].len()))
    };
    let new_start = shift(start, &lines);
    let new_end = shift(end, &lines);
    LineEdit {
        text: join(&lines),
        start: new_start,
        end: new_end,
    }
}

pub fn move_lines(text: &str, start: usize, end: usize, up: bool) -> Option<LineEdit> {
    let mut lines = split_lines(text);
    let (first, last) = line_range(&lines, start, end);
    if (up && first == 0) || (!up && last + 1 >= lines.len()) {
        return None;
    }
    let block: Vec<Vec<char>> = lines.drain(first..=last).collect();
    let (insert_at, shift) = if up {
        (first - 1, -((lines[first - 1].len() + 1) as isize))
    } else {
        (first + 1, (lines[first].len() + 1) as isize)
    };
    for (position, line) in block.into_iter().enumerate() {
        lines.insert(insert_at + position, line);
    }
    Some(LineEdit {
        text: join(&lines),
        start: (start as isize + shift) as usize,
        end: (end as isize + shift) as usize,
    })
}

pub fn duplicate_lines(text: &str, start: usize, end: usize) -> LineEdit {
    let mut lines = split_lines(text);
    let (first, last) = line_range(&lines, start, end);
    let block: Vec<Vec<char>> = lines[first..=last].to_vec();
    let block_len: usize = block.iter().map(|l| l.len() + 1).sum();
    for (position, line) in block.into_iter().enumerate() {
        lines.insert(last + 1 + position, line);
    }
    LineEdit {
        text: join(&lines),
        start: start + block_len,
        end: end + block_len,
    }
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Ohne Auswahl: Wort am Cursor. Mit Auswahl: nächstes Vorkommen (zyklisch).
pub fn next_occurrence(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let (a, b) = (
        start.min(end).min(chars.len()),
        start.max(end).min(chars.len()),
    );
    if a == b {
        let mut from = a;
        while from > 0 && is_word(chars[from - 1]) {
            from -= 1;
        }
        let mut to = a;
        while to < chars.len() && is_word(chars[to]) {
            to += 1;
        }
        return (from < to).then_some((from, to));
    }
    let needle = &chars[a..b];
    let length = needle.len();
    let matches_at = |at: usize| chars[at..at + length] == *needle;
    let limit = chars.len() - length;
    (b..=limit)
        .chain(0..a.min(limit + 1))
        .find(|at| matches_at(*at))
        .map(|at| (at, at + length))
}
