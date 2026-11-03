//! Inkrementelles Lexing: Nach einer Textänderung werden nur die Token im
//! beschädigten Bereich neu gelesen. Der Lexer ist an Tokengrenzen zustandslos,
//! daher gilt: Beginnt ein neu gelesenes Token im unveränderten Textende exakt
//! dort, wo auch im alten Text ein Token begann, stimmen alle folgenden Token
//! mit den alten überein (bis auf die verschobenen Positionen).

use super::*;
use crate::diagnostics::PositionShift;

/// Token, die weniger als diesen Abstand vor der Änderung enden, werden neu
/// gelesen. Der Lexer schaut höchstens drei Zeichen voraus.
const LOOKAHEAD_MARGIN: usize = 8;

/// Beschreibt, wie sich die neue Tokenliste aus der alten zusammensetzt:
/// `old[..prefix_tokens]`, dann `fresh_tokens` neue Token, dann
/// `old[old_suffix_start..]` mit `shift` verschoben.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenChange {
    pub prefix_tokens: usize,
    pub fresh_tokens: usize,
    pub old_suffix_start: usize,
    pub shift: Option<PositionShift>,
}

impl TokenChange {
    pub fn new_suffix_start(&self) -> usize {
        self.prefix_tokens + self.fresh_tokens
    }
}

#[derive(Debug, Clone)]
pub struct Relexed {
    pub output: LexOutput,
    pub change: TokenChange,
}

impl<'a> Lexer<'a> {
    fn resume(source: &'a str, from: SourcePosition) -> Self {
        Self {
            source,
            position: from.offset,
            line: from.line,
            column: from.column,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
            comments: Vec::new(),
        }
    }
}

/// Längster gemeinsamer Anfang und gemeinsames Ende (in Bytes, auf
/// Zeichengrenzen); beide überlappen sich nicht.
pub fn common_affixes(old: &str, new: &str) -> (usize, usize) {
    let (a, b) = (old.as_bytes(), new.as_bytes());
    let mut prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    while !(old.is_char_boundary(prefix) && new.is_char_boundary(prefix)) {
        prefix -= 1;
    }
    let room = a.len().min(b.len()) - prefix;
    let mut suffix = a
        .iter()
        .rev()
        .zip(b.iter().rev())
        .take(room)
        .take_while(|(x, y)| x == y)
        .count();
    while suffix > 0
        && !(old.is_char_boundary(a.len() - suffix) && new.is_char_boundary(b.len() - suffix))
    {
        suffix -= 1;
    }
    (prefix, suffix)
}

/// Liest `new_source` unter Wiederverwendung von `old` (das Ergebnis für
/// `old_source`). Das Ergebnis ist identisch zu `Lexer::new(new_source).tokenize()`.
pub fn relex(old: &LexOutput, old_source: &str, new_source: &str) -> Relexed {
    let (prefix, suffix) = common_affixes(old_source, new_source);
    let reused = old
        .tokens
        .partition_point(|token| token.span.end.offset + LOOKAHEAD_MARGIN <= prefix);
    let restart = reused.checked_sub(1).map_or(
        SourcePosition {
            offset: 0,
            line: 1,
            column: 1,
        },
        |last| old.tokens[last].span.end,
    );

    let mut tokens: Vec<Token> = old.tokens[..reused].to_vec();
    let mut comments: Vec<SourceSpan> = old.comments[..old
        .comments
        .partition_point(|span| span.end.offset <= restart.offset)]
        .to_vec();
    let mut diagnostics: Vec<Diagnostic> = old
        .diagnostics
        .iter()
        .filter(|item| {
            item.span.start.offset < restart.offset && item.span.end.offset <= restart.offset
        })
        .cloned()
        .collect();

    let new_floor = new_source.len() - suffix;
    let delta = new_source.len() as isize - old_source.len() as isize;
    let mut lexer = Lexer::resume(new_source, restart);
    let sync = loop {
        lexer.skip_whitespace_and_comments();
        if lexer.is_at_end() {
            break None;
        }
        let here = lexer.current_position();
        if here.offset >= new_floor {
            let old_offset = (here.offset as isize - delta) as usize;
            let index = old
                .tokens
                .partition_point(|token| token.span.start.offset < old_offset);
            if old
                .tokens
                .get(index)
                .is_some_and(|token| token.span.start.offset == old_offset)
            {
                break Some((index, here));
            }
        }
        lexer.scan_token();
    };

    match sync {
        None => {
            let fresh = lexer.finish();
            let fresh_tokens = fresh.tokens.len();
            tokens.extend(fresh.tokens);
            comments.extend(fresh.comments);
            diagnostics.extend(fresh.diagnostics);
            Relexed {
                output: LexOutput {
                    tokens,
                    diagnostics,
                    comments,
                },
                change: TokenChange {
                    prefix_tokens: reused,
                    fresh_tokens,
                    old_suffix_start: old.tokens.len(),
                    shift: None,
                },
            }
        }
        Some((index, here)) => {
            let anchor = old.tokens[index].span.start;
            let shift = PositionShift {
                offset: delta,
                line: here.line as isize - anchor.line as isize,
                anchor_line: anchor.line,
                column: here.column as isize - anchor.column as isize,
            };
            let fresh_tokens = lexer.tokens.len();
            tokens.append(&mut lexer.tokens);
            comments.append(&mut lexer.comments);
            diagnostics.append(&mut lexer.diagnostics);

            tokens.extend(old.tokens[index..].iter().cloned().map(|mut token| {
                shift.apply_span(&mut token.span);
                token
            }));
            comments.extend(
                old.comments
                    .iter()
                    .filter(|span| span.start.offset >= anchor.offset)
                    .map(|span| {
                        let mut span = *span;
                        shift.apply_span(&mut span);
                        span
                    }),
            );
            diagnostics.extend(
                old.diagnostics
                    .iter()
                    .filter(|item| item.span.start.offset >= anchor.offset)
                    .cloned()
                    .map(|mut item| {
                        item.shift(&shift);
                        item
                    }),
            );
            Relexed {
                output: LexOutput {
                    tokens,
                    diagnostics,
                    comments,
                },
                change: TokenChange {
                    prefix_tokens: reused,
                    fresh_tokens,
                    old_suffix_start: index,
                    shift: Some(shift),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_same_as_full(old_source: &str, new_source: &str) -> Relexed {
        let old = Lexer::new(old_source).tokenize();
        let incremental = relex(&old, old_source, new_source);
        let full = Lexer::new(new_source).tokenize();
        assert_eq!(incremental.output.tokens, full.tokens, "{new_source:?}");
        assert_eq!(incremental.output.comments, full.comments, "{new_source:?}");
        assert_eq!(
            incremental.output.diagnostics, full.diagnostics,
            "{new_source:?}"
        );
        incremental
    }

    #[test]
    fn typing_inside_a_line_reuses_tokens_on_both_sides() {
        let old = "int a = 1;\nint bbbbbbbbbbbb = 2;\nint c = 3;\n";
        let new = "int a = 1;\nint bbbbbbbbbbbbX = 2;\nint c = 3;\n";
        let relexed = assert_same_as_full(old, new);
        assert!(relexed.change.prefix_tokens >= 4);
        assert!(relexed.change.shift.is_some());
        assert!(relexed.change.fresh_tokens < 8);
    }

    #[test]
    fn multiline_insertions_and_deletions_shift_lines() {
        let old = "int a = 1;\nint b = 2;\n// kommentar\nint c = 3;\n";
        assert_same_as_full(
            old,
            "int a = 1;\nint x = 0;\nint y = 0;\nint b = 2;\n// kommentar\nint c = 3;\n",
        );
        assert_same_as_full(old, "int a = 1;\nint c = 3;\n");
        assert_same_as_full(old, "");
        assert_same_as_full("", old);
    }

    #[test]
    fn edits_that_merge_or_split_tokens_are_handled() {
        assert_same_as_full("int a = b << 2;", "int a = b < 2;");
        assert_same_as_full("x = 1 + 2;", "x = 1 +  = 2;");
        assert_same_as_full("a + b", "a +b++");
        assert_same_as_full("int abc;", "int a bc;");
        assert_same_as_full("int a /* c */ = 1;", "int a /* c = 1;");
        assert_same_as_full("int a = 1; // x\nint b;", "int a = 1; /* x\nint b;");
        assert_same_as_full("string s = \"abc\";", "string s = \"abc;");
    }

    #[test]
    fn unicode_and_columns_survive_changes() {
        assert_same_as_full("string s = \"äöü\"; int a;", "string s = \"äöüß\"; int a;");
        assert_same_as_full("// ü\nint a;", "// ü ü\nint a;");
    }

    #[test]
    fn randomized_edits_match_a_full_lex() {
        let base = "using System.Text;\nnamespace demo.core;\npublic class Point {\n    public int x;\n    public int y;\n    public int sum() { return x + y; /* summe */ }\n}\npublic int main() {\n    string name = \"welt\";\n    int total = 3 << 2;\n    // ende\n    return total;\n}\n";
        let alphabet: Vec<char> = "ab1 \n;{}()+<=/*\"'_.,äx".chars().collect();
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut current = base.to_string();
        for _ in 0..1500 {
            let chars: Vec<char> = current.chars().collect();
            let start = (next() as usize) % (chars.len() + 1);
            let remove = ((next() as usize) % 4).min(chars.len() - start);
            let insert: String = (0..(next() as usize % 4))
                .map(|_| alphabet[(next() as usize) % alphabet.len()])
                .collect();
            let mut edited: String = chars[..start].iter().collect();
            edited.push_str(&insert);
            edited.extend(&chars[start + remove..]);
            assert_same_as_full(&current, &edited);
            current = if current.len() > 600 {
                base.to_string()
            } else {
                edited
            };
        }
    }
}
