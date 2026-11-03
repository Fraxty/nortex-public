//! Inkrementelles Parsen auf Ebene der Top-Level-Elemente.
//!
//! Elemente vor der Änderung werden übernommen, sofern der Parser nie über die
//! unveränderten Token hinausgeschaut hat und kein Fehler auftrat. Ab dem
//! ersten Elementanfang im unveränderten Textende ist das Parsen deterministisch
//! identisch zum alten Lauf; alle restlichen Elemente werden nur verschoben.

use super::*;
use crate::diagnostics::PositionShift;
use crate::lexer::incremental::TokenChange;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReparseStats {
    pub reused_before: usize,
    pub reparsed: usize,
    pub reused_after: usize,
}

/// `tokens` ist die vollständige neue Tokenliste (aus `relex`), `previous` das
/// Ergebnis für die alte Tokenliste. Das Ergebnis ist identisch zu
/// `Parser::new(tokens).parse_traced()`.
pub fn reparse(
    tokens: Vec<Token>,
    previous: &ParsedFile,
    change: &TokenChange,
) -> (ParsedFile, ReparseStats) {
    let mut parser = Parser::new(tokens);
    let mut program = Program::empty();
    parser.parse_header(&mut program);
    let header_diagnostics = std::mem::take(&mut parser.diagnostics);

    let mut stats = ReparseStats::default();
    let mut items: Vec<TopLevelItem> = Vec::new();
    for old in &previous.items {
        let untouched = old.start == parser.current
            && old.end < change.prefix_tokens
            && old.diagnostics.is_empty();
        if !untouched {
            break;
        }
        items.push(carry_over(old, previous, &mut program, None));
        parser.current = old.end;
        stats.reused_before += 1;
    }

    let new_suffix_start = change.new_suffix_start();
    while !parser.is_at_end() {
        if let Some(shift) = &change.shift
            && parser.current >= new_suffix_start
        {
            let old_start = change.old_suffix_start + (parser.current - new_suffix_start);
            let first = previous
                .items
                .partition_point(|item| item.start < old_start);
            if previous
                .items
                .get(first)
                .is_some_and(|item| item.start == old_start)
            {
                let token_delta = new_suffix_start as isize - change.old_suffix_start as isize;
                for old in &previous.items[first..] {
                    items.push(carry_over(
                        old,
                        previous,
                        &mut program,
                        Some((shift, token_delta)),
                    ));
                    stats.reused_after += 1;
                }
                break;
            }
        }
        items.push(parser.parse_top_level_item(&mut program));
        stats.reparsed += 1;
    }

    (
        ParsedFile::assemble(program, header_diagnostics, items),
        stats,
    )
}

/// Übernimmt ein Element samt Deklaration aus dem alten Ergebnis, bei Bedarf
/// mit verschobenen Positionen und Tokenindizes.
fn carry_over(
    old: &TopLevelItem,
    previous: &ParsedFile,
    program: &mut Program,
    moved: Option<(&PositionShift, isize)>,
) -> TopLevelItem {
    let declaration = old.declaration.map(|index| {
        let mut declaration = previous.program.declarations[index].clone();
        if let Some((shift, _)) = moved {
            declaration.shift(shift);
        }
        program.declarations.push(declaration);
        program.declarations.len() - 1
    });
    let mut item = TopLevelItem {
        start: old.start,
        end: old.end,
        declaration,
        diagnostics: old.diagnostics.clone(),
    };
    if let Some((shift, token_delta)) = moved {
        item.start = old.start.wrapping_add_signed(token_delta);
        item.end = old.end.wrapping_add_signed(token_delta);
        for diagnostic in &mut item.diagnostics {
            diagnostic.shift(shift);
        }
    }
    item
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::lexer::incremental::relex;

    const BASE: &str = "using System.Text;\nnamespace demo.core;\npublic int counter = 0;\npublic class Point {\n    public int x;\n    public int y;\n    public int sum() { return x + y; }\n}\npublic enum Color { Red, Green }\npublic int twice(int value) {\n    int doubled = value * 2;\n    if (doubled > 10) { return 10; }\n    return doubled;\n}\npublic int main() {\n    Point p = new Point();\n    string name = \"welt\";\n    return twice(p.sum());\n}\n";

    fn check(old_source: &str, new_source: &str) -> ReparseStats {
        let old_lexed = Lexer::new(old_source).tokenize();
        let old_parsed = Parser::new(old_lexed.tokens.clone()).parse_traced();

        let relexed = relex(&old_lexed, old_source, new_source);
        let (incremental, stats) =
            reparse(relexed.output.tokens.clone(), &old_parsed, &relexed.change);

        let full_lexed = Lexer::new(new_source).tokenize();
        let full = Parser::new(full_lexed.tokens.clone()).parse_traced();
        assert_eq!(relexed.output.tokens, full_lexed.tokens, "{new_source:?}");
        assert_eq!(incremental, full, "{new_source:?}");
        stats
    }

    #[test]
    fn editing_one_function_reuses_the_other_declarations() {
        let new = BASE.replace("int doubled = value * 2;", "int doubled = value * 3 + 1;");
        let stats = check(BASE, &new);
        assert!(stats.reused_before >= 3, "{stats:?}");
        assert!(stats.reused_after >= 1, "{stats:?}");
        assert_eq!(stats.reparsed, 1, "{stats:?}");
    }

    #[test]
    fn line_shifts_and_new_declarations_match_a_full_parse() {
        check(
            BASE,
            &BASE.replace(
                "public int twice",
                "public int added() { return 1; }\npublic int twice",
            ),
        );
        check(
            BASE,
            &BASE.replace("public enum Color { Red, Green }\n", ""),
        );
        check(
            BASE,
            &BASE.replace(
                "using System.Text;\n",
                "using System.Text;\nusing System.Math;\n",
            ),
        );
        check(
            BASE,
            &BASE.replace("namespace demo.core;", "namespace demo.other;"),
        );
        check(BASE, "");
        check("", BASE);
    }

    #[test]
    fn broken_code_matches_a_full_parse() {
        check(BASE, &BASE.replace("return doubled;", "return doubled"));
        check(
            BASE,
            &BASE.replace("public int sum() {", "public int sum( {"),
        );
        check(
            BASE,
            &BASE.replace(
                "if (doubled > 10) { return 10; }",
                "if (doubled > 10 { return 10; }",
            ),
        );
        check(
            BASE,
            &BASE.replace("public class Point {", "public class Point"),
        );
        check(
            BASE,
            &BASE.replace("}\npublic int main", "\npublic int main"),
        );
        check(BASE, &format!("{BASE}\npublic int ("));
    }

    #[test]
    fn undo_and_redo_restore_the_exact_previous_result() {
        let edited = BASE.replace("value * 2", "value * 22");
        check(BASE, &edited);
        check(&edited, BASE);
    }

    #[test]
    fn randomized_edits_match_a_full_parse() {
        let alphabet: Vec<char> = "ab1 \n;{}()+<=/*\"'_.,äxinpublicint".chars().collect();
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut current = BASE.to_string();
        let mut reused = 0;
        for _ in 0..2500 {
            let chars: Vec<char> = current.chars().collect();
            let start = (next() as usize) % (chars.len() + 1);
            let remove = ((next() as usize) % 6).min(chars.len() - start);
            let insert: String = (0..(next() as usize % 5))
                .map(|_| alphabet[(next() as usize) % alphabet.len()])
                .collect();
            let mut edited: String = chars[..start].iter().collect();
            edited.push_str(&insert);
            edited.extend(&chars[start + remove..]);
            let stats = check(&current, &edited);
            reused += stats.reused_before + stats.reused_after;
            current = if edited.len() > 900 || edited.len() < 100 {
                BASE.to_string()
            } else {
                edited
            };
        }
        assert!(reused > 1000, "zu wenig Wiederverwendung: {reused}");
    }
}
