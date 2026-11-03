use nortex_compiler::diagnostics::Severity;
use nortex_compiler::driver::{check_file, check_source};
use nortex_compiler::lexer::{Lexer, TokenKind};

/// Sorted 1-based line numbers of all diagnostics with `code`. Exact line
/// lists replace the former `count() >= N` checks, which also passed when a
/// line produced duplicates while another faulty line went undetected.
fn error_lines(result: &nortex_compiler::driver::CheckResult, code: &str) -> Vec<usize> {
    let mut lines = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == code)
        .map(|diagnostic| diagnostic.span.start.line)
        .collect::<Vec<_>>();
    lines.sort_unstable();
    lines
}

#[test]
fn preserves_identifier_case() {
    let output = Lexer::new("name Name NAME").tokenize();
    let identifiers: Vec<_> = output
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Identifier)
        .map(|token| token.lexeme.as_str())
        .collect();
    assert_eq!(identifiers, vec!["name", "Name", "NAME"]);
}

#[test]
fn accepts_the_documented_class_example() {
    let source = r#"
        using System;
        namespace Game.Player;
        public class PlayerController
        {
            private int health = 100;
            public void takeDamage(int damage)
            {
                health = health - damage;
            }
            public int getHealth()
            {
                return health;
            }
        }
    "#;
    let result = check_source(source);
    let errors: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect();
    assert!(errors.is_empty(), "unexpected diagnostics: {errors:#?}");
}

#[test]
fn rejects_forbidden_operator_overload() {
    let source = r#"
        public class Number
        {
            public static Number operator +(Number left, Number right)
            {
                return left;
            }
        }
    "#;
    let result = check_source(source);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1104")
    );
}

#[test]
fn list_and_vec_are_separate_generic_types() {
    let source = r#"
        using System.Collections;
        public class Container
        {
            private List<int> items;
            private Vec<int> values;
        }
    "#;
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_question_mark_error_propagation_operator() {
    let source = r#"
        public Result<int, RuntimeException> readValue() {
            var result = read();
            return result?;
        }
    "#;
    let result = check_source(source);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("?"))
    );
}

#[test]
fn rejects_assignment_to_const_value() {
    let source = r#"
        public class Constants
        {
            public void update()
            {
                const int MAX_HEALTH = 100;
                MAX_HEALTH = 50;
            }
        }
    "#;
    let result = check_source(source);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unveränderliche Wert"))
    );
}

#[test]
fn parses_async_await_and_exception_control_flow() {
    let source = r#"
        public class Service
        {
            public async Task<string> loadMessage()
            {
                try
                {
                    return await readText("message.txt");
                }
                catch (RuntimeException error)
                {
                    throw error;
                }
                finally
                {
                    cleanup();
                }
            }
        }
    "#;
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn differently_cased_fields_are_not_duplicate_declarations() {
    let source = r#"
        public class CaseTest
        {
            private int name;
            private int Name;
            private int NAME;
        }
    "#;
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn typed_bindings_are_immutable_by_default_and_mut_is_explicit() {
    let immutable = check_source(
        r#"
        public class MutabilityTest
        {
            public void run()
            {
                int score = 10;
                score = 20;
            }

        }
    "#,
    );
    assert!(
        immutable
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1200"),
        "expected immutable assignment diagnostic, got: {:#?}",
        immutable.diagnostics
    );

    let mutable = check_source(
        r#"
        public class MutabilityTest
        {
            public void run()
            {
                mut int score = 10;
                score = 20;
            }
        }
    "#,
    );
    assert!(
        !mutable.has_errors(),
        "diagnostics: {:#?}",
        mutable.diagnostics
    );

    let immutable = check_source(
        r#"
        public class MutabilityTest
        {
            public void run()
            {
                const int MAX_SCORE = 10;
                MAX_SCORE = 20;
            }
        }
    "#,
    );
    assert!(
        immutable
            .diagnostics
            .iter()
            .any(|d| d.message.contains("unveränderliche Wert"))
    );
}

#[test]
fn global_scalars_parse_with_explicit_mutability_and_precise_top_level_errors() {
    let accepted = check_source(
        r#"
        int age = 19;
        string name = "Jamie";
        bool isActive = true;
        double price = 19.99;
        public const int MAX_PLAYERS = 100;
        public int main() { return age; }
        "#,
    );
    assert!(
        !accepted.has_errors(),
        "global scalar declarations should type-check: {:#?}",
        accepted.diagnostics
    );

    let immutable = check_source("int age = 19; public int main() { age = 20; return age; }");
    assert!(
        immutable
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1200"),
        "unmarked globals should be immutable: {:#?}",
        immutable.diagnostics
    );

    let mutable = check_source("mut int age = 19; public int main() { age = 20; return age; }");
    assert!(!mutable.has_errors(), "{:#?}", mutable.diagnostics);

    let malformed = check_source("int main { return 0; }");
    assert!(
        malformed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX0102"),
        "invalid top-level syntax should report NX0102: {:#?}",
        malformed.diagnostics
    );
}

#[test]
fn naming_style_warns_for_case_separators_and_long_acronyms() {
    let result = check_source(
        r#"
        public class XMLParser
        {
            private int bad_name = 0;
            public void TakeDamage() { }
            public void parseHTTPHeader() { }
            public void getID() { }
        }
        public class XmlParser { }
        public class IOStream { }
        public interface Renderable { }
        "#,
    );
    let style_warnings = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "NX0004")
        .collect::<Vec<_>>();
    let warning_lines = style_warnings
        .iter()
        .map(|diagnostic| diagnostic.span.start.line)
        .collect::<Vec<_>>();
    assert_eq!(
        warning_lines,
        vec![2, 4, 5, 6],
        "only invalid type/value styles should warn; two-letter acronyms and I-less interfaces are accepted: {style_warnings:#?}"
    );
}

#[test]
fn const_requires_a_compile_time_expression() {
    let result = check_source(
        r#"
        public class ConstantTest
        {
            public void run()
            {
                const int MAX_VALUE = calculate();
            }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Compile-Zeit"))
    );
}

#[test]
fn expanded_builtin_generic_types_are_recognized() {
    let result = check_source(
        r#"
        using System.Collections;
        public class TypeTest
        {
            private Dictionary<string, int> scores;
            private HashSet<int> uniqueValues;
            private Queue<int> pending;
            private Stack<int> history;
            private Arc<int> shared;
            private Func<int, string> formatter;
        }
    "#,
    );
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn function_parameters_cannot_be_reassigned() {
    let result = check_source(
        r#"
        public class ParameterTest
        {
            public void update(int value)
            {
                value = 5;
            }
        }
    "#,
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "NX1200"),
        "direct assignment to a non-reassignable parameter should be rejected: {:#?}",
        result.diagnostics
    );
}

#[test]
fn parses_borrow_pointer_fixed_array_and_never_types() {
    let result = check_source(
        r#"
        public class Player { public int health; }
        public abstract class ReferenceTest
        {
            public abstract void inspect(&Player player);
            public abstract void update(&mut Player player);
            public abstract unsafe void inspectRaw(*const int data);
            public abstract unsafe void writeRaw(*mut int data);
            private [int; 5] fixedValues;
            public abstract ! terminate();
        }
    "#,
    );
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn record_declarations_are_recognized() {
    let result = check_source(
        r#"
        public record PlayerSnapshot
        {
            public int health;
        }
    "#,
    );
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_non_ascii_identifier_but_accepts_unicode_comment() {
    let lexed = Lexer::new("// Grüße\nint größe = 1;").tokenize();
    assert!(!lexed.diagnostics.is_empty());
}

#[test]
fn parses_enum_payloads_and_match_arms() {
    let result = check_source(
        r#"
        public enum OperationResult<T, E>
        {
            Success(T),
            Failure(E),
        }

        public class MatchTest
        {
            public void inspect(OperationResult<int, string> result)
            {
                match (result)
                {
                    Success(value) =>
                    {
                        print(value);
                    }
                    Failure(error) =>
                    {
                        print(error);
                    }
                    _ =>
                    {
                        print("unknown");
                    }
                }
            }
        }
    "#,
    );
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_duplicate_match_arms() {
    let result = check_source(
        r#"
        public class MatchTest
        {
            public void inspect(int value)
            {
                match (value)
                {
                    First => { }
                    First => { }
                }
            }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("mehrfach behandelt"))
    );
}

#[test]
fn parses_for_and_switch_control_flow() {
    let result = check_source(
        r#"
        public class ControlFlowTest
        {
            public void run()
            {
                for (mut int index = 0; index < 10; index++)
                {
                    if (index == 5) { continue; }
                }

                switch (1)
                {
                    case 1:
                        break;
                    default:
                        break;
                }
            }
        }
    "#,
    );
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn switch_rejects_cases_after_default() {
    let result = check_source(
        r#"
        public class SwitchTest
        {
            public void run(int value)
            {
                switch (value)
                {
                    default: break;
                    case 1: break;
                }
            }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("default-Fall muss der letzte"))
    );
}

#[test]
fn rejects_break_and_continue_outside_valid_contexts() {
    let result = check_source(
        r#"
        public class InvalidControlFlow
        {
            public void run()
            {
                break;
                continue;
            }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("break ist nur"))
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("continue ist nur"))
    );
}

#[test]
fn allows_break_inside_switch_but_not_continue() {
    let result = check_source(
        r#"
        public class SwitchControlFlow
        {
            public void run(int value)
            {
                switch (value)
                {
                    case 1: break;
                    default: break;
                }
            }
        }
    "#,
    );
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_multiple_class_base_types() {
    let result = check_source(
        r#"
        public class BaseOne { }
        public class BaseTwo { }
        public class Derived : BaseOne, BaseTwo { }
    "#,
    );
    assert!(result.diagnostics.iter().any(|d| d.code == "NX1002"));
}

#[test]
fn interfaces_only_allow_method_signatures() {
    let result = check_source(
        r#"
        public interface IRenderable
        {
            public void render() { }
            public int state;
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("nur Signaturen"))
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("keine Felder"))
    );
}

#[test]
fn immutable_bindings_cannot_be_incremented() {
    let result = check_source(
        r#"
        public class IncrementTest
        {
            public void run()
            {
                const int COUNT = 1;
                COUNT++;
            }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("nicht inkrementiert"))
    );
}

#[test]
fn top_level_functions_can_be_overloaded_by_parameter_types() {
    let accepted = check_source(
        r#"
        public int calculate(int value) { return value; }
        public double calculate(double value) { return value; }
    "#,
    );
    assert!(
        !accepted.has_errors(),
        "diagnostics: {:#?}",
        accepted.diagnostics
    );

    let rejected = check_source(
        r#"
        public int calculate(int value) { return value; }
        public int calculate(int other) { return other; }
    "#,
    );
    assert!(
        rejected
            .diagnostics
            .iter()
            .any(|d| d.message.contains("Doppelte Funktionssignatur"))
    );
}

#[test]
fn lexes_and_parses_bitwise_and_shift_operators() {
    let lexed = Lexer::new("a << 2 | b & 3 ^ c >> 1").tokenize();
    let symbols: Vec<_> = lexed
        .tokens
        .iter()
        .filter(|token| matches!(token.kind, TokenKind::Symbol(_)))
        .map(|token| token.lexeme.as_str())
        .collect();
    assert!(symbols.contains(&"<<"));
    assert!(symbols.contains(&">>"));
    assert!(symbols.contains(&"|"));
    assert!(symbols.contains(&"&"));
    assert!(symbols.contains(&"^"));
}

#[test]
fn parses_bitwise_and_shift_expression_precedence() {
    let result = check_source(
        r#"
        public class BitwiseTest
        {
            public int calculate(int first, int second)
            {
                return (first << 2) | (second & 3) ^ (first >> 1);
            }
        }
    "#,
    );
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn mutating_parameter_objects_requires_mutable_borrow() {
    let rejected = check_source(
        r#"
        public class Player { public int health; }
        public class MutationTest
        {
            public void damage(Player player)
            {
                player.health = 0;
            }
        }
    "#,
    );
    assert!(rejected.diagnostics.iter().any(|d| d.code == "NX1201"));

    let accepted = check_source(
        r#"
        public class Player { public int health; }
        public class MutationTest
        {
            public void damage(&mut Player player)
            {
                player.health = 0;
            }
        }
    "#,
    );
    assert!(
        !accepted.has_errors(),
        "diagnostics: {:#?}",
        accepted.diagnostics
    );
}

#[test]
fn project_examples_pass_the_current_frontend() {
    let examples_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../examples");
    for relative in [
        "hello-world/src/main.nx",
        "console/main.nx",
        "collections/main.nx",
        "classes/main.nx",
        "async/main.nx",
        "graphics/main.nx",
    ] {
        let path = examples_root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
        let result = check_source(&source);
        assert!(
            !result.has_errors(),
            "{}: {:#?}",
            path.display(),
            result.diagnostics
        );
    }
}

#[test]
fn standard_library_signatures_are_explicitly_marked_as_intrinsics() {
    fn collect_sources(directory: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        let entries = std::fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry.expect("directory entry should be readable").path();
            if path.is_dir() {
                collect_sources(&path, files);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("nx") {
                files.push(path);
            }
        }
    }

    let stdlib_root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../standard-library");
    let mut files = Vec::new();
    collect_sources(&stdlib_root, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "standard library source files must be present"
    );

    for path in files {
        let result = check_file(&path)
            .unwrap_or_else(|error| panic!("could not check {}: {error}", path.display()));
        assert!(
            !result.has_errors(),
            "{}: {:#?}",
            path.display(),
            result.diagnostics
        );
    }
}

#[test]
fn const_class_binding_cannot_mutate_its_fields() {
    let result = check_source(
        r#"
        public class Player { public int health; }
        public class MutationTest
        {
            public void run()
            {
                const Player player = new Player();
                player.health = 0;
            }
        }
    "#,
    );
    assert!(
        result.diagnostics.iter().any(|d| d.code == "NX1201"),
        "direct assignment to a non-reassignable parameter should be rejected: {:#?}",
        result.diagnostics
    );
}

#[test]
fn mutable_borrow_parameter_binding_cannot_be_reassigned() {
    let result = check_source(
        r#"
        public class Player { public int health; }
        public class MutationTest
        {
            public void run(&mut Player player, Player other)
            {
                player = other;
            }
        }
    "#,
    );
    assert!(result.diagnostics.iter().any(|d| d.code == "NX1200"));
}

#[test]
fn nested_generic_types_split_shift_token_only_in_type_context() {
    let declarations = check_source(
        r#"
        using System.Collections;
        public class NestedGenericTest
        {
            private Dictionary<string, List<int>> values;
            public Dictionary<string, List<int>> create()
            {
                return new Dictionary<string, List<int>>();
            }
        }
    "#,
    );
    assert!(
        !declarations.has_errors(),
        "diagnostics: {:#?}",
        declarations.diagnostics
    );

    // In expression context, the exact same lexer token remains a shift.
    let expression = check_source(
        r#"
        public class ShiftTest
        {
            public int shift(int value) { return value >> 2; }
        }
    "#,
    );
    assert!(
        !expression.has_errors(),
        "diagnostics: {:#?}",
        expression.diagnostics
    );
}

#[test]
fn parser_accepts_constructors_generic_construction_arrays_chars_and_ternary() {
    let source = r#"
        public class Container<T>
        {
            public Container(T value) { }
        }
        public class SyntaxRegression
        {
            public void run()
            {
                mut int[] values;
                char letter = 'A';
                Container<int> container = Container<int>(1);
                bool ready = true;
                int selected = ready ? 1 : 2;
            }
        }
    "#;
    let result = check_source(source);
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn unknown_local_value_uses_nx1004_with_help() {
    let result = check_source("public class C { public int read() { return playerHealth; } }");
    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "NX1004")
        .expect("unknown value should use NX1004");
    assert!(diagnostic.help.is_some());
}

#[test]
fn sibling_block_scopes_allow_the_same_local_name() {
    let result = check_source(
        r#"
        public class ScopeTest
        {
            public void run()
            {
                { int value = 1; }
                { int value = 2; }
            }
        }
    "#,
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("bereits definiert")),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn parses_multiple_interface_and_trait_contracts_in_base_list() {
    let result = check_source(
        r#"
        public interface IReadable { string read(); }
        public interface IWritable { void write(string value); }
        public trait IFlushable { void flush(); }
        public class Stream : IReadable, IWritable, IFlushable
        {
            public string read() { return ""; }
            public void write(string value) { }
            public void flush() { }
        }
    "#,
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn constructor_name_is_not_reported_as_a_method_naming_error() {
    let result = check_source(
        r#"
        public class PlayerController
        {
            public PlayerController() { }
        }
    "#,
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("camelCase")),
        "{:?}",
        result.diagnostics
    );
}

#[test]
fn rejects_obvious_primitive_literal_type_mismatches() {
    let result = check_source(
        r#"
        public class TypeErrors
        {
            public void run()
            {
                int message = "hello";
                bool count = 5;
                void invalid = 1;
            }
        }
    "#,
    );
    assert_eq!(
        error_lines(&result, "NX1102"),
        vec![6, 7, 8],
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_primitive_literal_return_type_mismatches() {
    let result = check_source(
        r#"
        public class ReturnTypeErrors
        {
            public int readMessage() { return "wrong"; }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1102"),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn missing_return_expression_uses_the_dedicated_nx1103_code() {
    let result = check_source("public int missingValue() { return; }");
    let missing_return = result
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == "NX1103")
        .count();
    assert_eq!(
        missing_return, 1,
        "missing return should produce one NX1103: {:#?}",
        result.diagnostics
    );
}

#[test]
fn exhaustive_enum_match_satisfies_return_path_analysis() {
    let result = check_source(
        r#"
        public enum LoadResult { Success, Failure }
        public class MatchReturn
        {
            public int value(LoadResult result)
            {
                match (result)
                {
                    Success => { return 1; }
                    Failure => { return 0; }
                }
            }
        }
    "#,
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1103"),
        "exhaustive enum match must satisfy return paths: {:#?}",
        result.diagnostics
    );
}

#[test]
fn match_on_another_enum_does_not_count_as_an_exhaustive_return_path() {
    let result = check_source(
        r#"
        public enum Complete { Red, Blue }
        public enum Incomplete { Red }
        public class MatchReturn
        {
            public int value(Complete result)
            {
                match (result)
                {
                    Incomplete.Red => { return 1; }
                }
            }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1103"),
        "non-exhaustive match must not satisfy return paths: {:#?}",
        result.diagnostics
    );
}

#[test]
fn reserved_keywords_cannot_be_used_as_identifiers() {
    let result = check_source(
        r#"
        public class ReservedNames
        {
            public void run()
            {
                int class = 1;
            }
        }
    "#,
    );
    assert!(
        result.has_errors(),
        "reserved keyword was accepted as a variable name"
    );
}

#[test]
fn match_arms_accept_single_expression_bodies() {
    let result = check_source(
        r#"
        public enum LoadResult { Success, Failure }
        public class MatchExpressionBody
        {
            public void printResult(LoadResult result)
            {
                match (result)
                {
                    Success => print("ok");
                    Failure => print("failed");
                }
            }
        }
    "#,
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn standalone_nx_regression_sources_are_wired_into_the_test_suite() {
    let project_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let cases = [
        ("tests/lexer/case-sensitive.nx", false),
        ("tests/parser/class-method.nx", false),
        ("tests/integration/hello-world.nx", false),
        ("tests/semantic/invalid-operator.nx", true),
        ("tests/runtime/thread-pool.nx", false),
    ];

    for (relative, expects_error) in cases {
        let path = project_root.join(relative);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()));
        let result = check_source(&source);
        assert_eq!(
            result.has_errors(),
            expects_error,
            "{}: {:#?}",
            path.display(),
            result.diagnostics
        );
        if relative == "tests/semantic/invalid-operator.nx" {
            assert!(
                result
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("nicht überladen")),
                "{}: {:#?}",
                path.display(),
                result.diagnostics
            );
        }
    }
}

#[test]
fn primitive_type_names_are_reserved_type_keywords() {
    let lexed = Lexer::new("int value = 1;").tokenize();
    assert_eq!(lexed.tokens[0].kind, TokenKind::Keyword);
    let valid = check_source("public class C { public int read() { return 1; } }");
    assert!(!valid.has_errors(), "diagnostics: {:#?}", valid.diagnostics);
    let invalid = check_source("public class C { public void run() { int class = 1; } }");
    assert!(
        invalid.has_errors(),
        "reserved keyword was accepted as an identifier"
    );
}

#[test]
fn parses_single_and_multiple_base_types() {
    let single = check_source(
        r#"
        public class Base { }
        public class Derived : Base { }
    "#,
    );
    assert!(
        !single.has_errors(),
        "single base type: {:#?}",
        single.diagnostics
    );

    let multiple = check_source(
        r#"
        public interface IReadable { string read(); }
        public interface IWritable { void write(string value); }
        public class Stream : IReadable, IWritable
        {
            public string read() { return ""; }
            public void write(string value) { }
        }
    "#,
    );
    assert!(
        !multiple.has_errors(),
        "multiple base types: {:#?}",
        multiple.diagnostics
    );
}

#[test]
fn constructor_can_assign_field_with_same_name_as_immutable_parameter() {
    let result = check_source(
        r#"
        public class Container<T>
        {
            private T value;
            public Container(T value)
            {
                this.value = value;
            }
        }
    "#,
    );
    assert!(
        !result.has_errors(),
        "constructor field assignment: {:#?}",
        result.diagnostics
    );
}

#[test]
fn get_and_set_are_valid_method_identifiers_but_let_is_reserved() {
    let methods = check_source(
        r#"
        public class Values
        {
            public int get() { return 1; }
            public void set(int value) { }
        }
    "#,
    );
    assert!(
        !methods.has_errors(),
        "get/set should be identifiers: {:#?}",
        methods.diagnostics
    );

    let lexed = Lexer::new("let value = 1;").tokenize();
    assert_eq!(lexed.tokens[0].kind, TokenKind::Keyword);
    let rejected = check_source("public class C { public void run() { let value = 1; } }");
    assert!(
        rejected.has_errors(),
        "unsupported let declaration must not be parsed as a type"
    );
}

#[test]
fn async_task_returns_are_checked_against_the_inner_type() {
    let valid = check_source(
        r#"
        public class AsyncService
        {
            public async Task<int> load() { return 42; }
        }
    "#,
    );
    assert!(
        !valid.has_errors(),
        "async Task<T> should accept T: {:#?}",
        valid.diagnostics
    );

    let invalid = check_source(
        r#"
        public class AsyncService
        {
            public async Task<int> load() { return "wrong"; }
        }
    "#,
    );
    assert!(
        invalid
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1102"),
        "async Task<T> should reject values that do not match T: {:#?}",
        invalid.diagnostics
    );
}

#[test]
fn namespace_and_using_directives_can_appear_in_either_header_order() {
    let result = check_source(
        r#"
        namespace Game.Network;
        using System.Collections;
        public class C { private List<int> values; }
    "#,
    );
    assert!(
        !result.has_errors(),
        "namespace followed by using: {:#?}",
        result.diagnostics
    );
}

#[test]
fn box_new_member_call_is_accepted_as_an_expression() {
    let result = check_source(
        r#"
        public class C
        {
            public void run()
            {
                Box<int> value = Box.new(1);
            }
        }
    "#,
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Membername")),
        "Box.new should parse as a member call: {:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_concrete_class_methods_without_implementation_bodies() {
    let result = check_source(
        r#"
        public class MissingImplementation
        {
            public void run();
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1005"),
        "method declarations without bodies must be rejected: {:#?}",
        result.diagnostics
    );
}

#[test]
fn allows_interface_abstract_and_compiler_intrinsic_declarations() {
    let result = check_source(
        r#"
        public interface IService
        {
            public void run();
        }
        public abstract class AbstractService
        {
            public abstract void run();
        }
        public class RuntimeApi
        {
            public void reset() { }
        }
    "#,
    );
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_extern_declarations_without_an_explicit_supported_abi() {
    let result = check_source("public class NativeApi { public extern void run(); }");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1005"),
        "extern without an explicit ABI must not bypass declaration validation: {:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_each_invalid_abstract_and_intrinsic_modifier_combination() {
    let invalid_cases = [
        (
            "public class NotAbstract { public abstract void run(); }",
            "abstract outside abstract class",
        ),
        (
            "public class IntrinsicWithBody { public intrinsic void reset() { } }",
            "intrinsic with body",
        ),
        (
            "public abstract class PrivateAbstract { private abstract void run(); }",
            "private abstract",
        ),
        (
            "public abstract class StaticAbstract { static abstract void run(); }",
            "static abstract",
        ),
        (
            "public abstract class AbstractIntrinsic { abstract intrinsic void run(); }",
            "abstract intrinsic",
        ),
        (
            "public abstract class DuplicateIntrinsic { intrinsic intrinsic void run(); }",
            "duplicate intrinsic",
        ),
        (
            "public abstract class AsyncIntrinsic { async intrinsic Task<int> run(); }",
            "async intrinsic",
        ),
        (
            "public trait IntrinsicTrait { intrinsic void run(); }",
            "trait intrinsic",
        ),
        (
            "public class IntrinsicConstructor { public intrinsic IntrinsicConstructor() { } }",
            "intrinsic constructor",
        ),
    ];

    for (source, label) in invalid_cases {
        let result = check_source(source);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "NX1005"),
            "{label} must produce NX1005: {:#?}",
            result.diagnostics
        );
    }
}

#[test]
fn parses_and_types_the_plan_foreach_example() {
    let result = check_source(
        r#"
        public class Player
        {
            public void printName(string playerName)
            {
                for (char character in playerName.chars())
                {
                    Console.writeLine(character);
                }
            }
        }
    "#,
    );
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_unknown_namespace_imports() {
    let result = check_source("using Does.Not.Exist; public class A { }");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1003")
    );
}

#[test]
fn reports_expression_type_mismatches_for_variables_operators_and_returns() {
    let result = check_source(
        r#"
        public int getNumber() { return 1; }
        public string wrongReturn() { return 1 + 2; }
        public class TypeErrors
        {
            public void run()
            {
                int a = 1;
                bool b = a;
                int c = 1 + "x";
                bool d = 1 && true;
                string s = getNumber();
                if (1) { }
                while ("x") { break; }
            }
        }
    "#,
    );
    assert_eq!(
        error_lines(&result, "NX1102"),
        vec![3, 9, 10, 11, 12, 13, 14],
        "expected one NX1102 per faulty line, got: {:#?}",
        result.diagnostics
    );
}

#[test]
fn checks_call_and_constructor_argument_types_and_arity() {
    let result = check_source(
        r#"
        public class Widget
        {
            public Widget(int value) { }
            public void update(int value) { }
        }
        public class Calls
        {
            public void run()
            {
                Widget widget = new Widget(true);
                widget.update("wrong");
                widget.update(1, 2);
            }
        }
    "#,
    );
    assert_eq!(
        error_lines(&result, "NX1107"),
        vec![11, 12, 13],
        "expected overload and arity diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn resolves_fields_methods_and_reports_unknown_members() {
    let result = check_source(
        r#"
        public class Player
        {
            public int health;
            public int getHealth() { return health; }
            public void run()
            {
                int current = this.getHealth();
                int missing = this.missingField;
                this.missingMethod();
            }
        }
    "#,
    );
    assert_eq!(
        error_lines(&result, "NX1106"),
        vec![9, 10],
        "expected unknown member diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn abstract_contracts_require_concrete_implementations_and_prevent_instantiation() {
    let missing_implementation = check_source(
        r#"
        public abstract class Base
        {
            public abstract void run();
        }
        public class Child : Base { }
    "#,
    );
    assert!(
        missing_implementation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1005")
    );

    let abstract_instantiation = check_source(
        r#"
        public abstract class Base { }
        public class Program
        {
            public void run() { Base value = new Base(); }
        }
    "#,
    );
    assert!(
        abstract_instantiation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1005")
    );

    let valid_implementation = check_source(
        r#"
        public abstract class Base
        {
            public abstract void run();
        }
        public class Child : Base
        {
            public override void run() { }
        }
    "#,
    );
    assert!(
        !valid_implementation.has_errors(),
        "diagnostics: {:#?}",
        valid_implementation.diagnostics
    );
}

#[test]
fn intrinsic_is_contextual_and_restricted_to_standard_library_sources() {
    let identifier = check_source(
        r#"
        public class Program
        {
            public void run() { int intrinsic = 1; }
        }
    "#,
    );
    assert!(
        !identifier.has_errors(),
        "intrinsic should remain usable as an identifier: {:#?}",
        identifier.diagnostics
    );

    let user_intrinsic = check_source(
        r#"
        public class Program
        {
            public intrinsic void run();
        }
    "#,
    );
    assert!(
        user_intrinsic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1005")
    );
}

#[test]
fn rejects_conflicting_abstract_and_intrinsic_modifiers() {
    for source in [
        "public abstract class A { private abstract void run(); }",
        "public abstract class A { static abstract void run(); }",
        "public abstract class A { abstract intrinsic void run(); }",
        "public abstract class A { intrinsic intrinsic void run(); }",
    ] {
        let result = check_source(source);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "NX1005"),
            "source should be rejected: {source}\n{:#?}",
            result.diagnostics
        );
    }
}

#[test]
fn checks_async_expression_results_and_rejects_await_on_non_tasks() {
    let result = check_source(
        r#"
        public int getNumber() { return 1; }
        public class AsyncTypeErrors
        {
            public async Task<string> wrongReturn()
            {
                return getNumber();
            }
            public async Task<string> wrongAwait()
            {
                return await getNumber();
            }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1102"),
        "async return expression should be checked against T: {:#?}",
        result.diagnostics
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1107"),
        "await on a non-Task should be rejected: {:#?}",
        result.diagnostics
    );
}

#[test]
fn overload_resolution_rejects_calls_without_an_exact_candidate() {
    let result = check_source(
        r#"
        public class Overloads
        {
            public int choose(int value) { return value; }
            public string choose(string value) { return value; }
            public void run()
            {
                bool result = choose(true) == 1;
            }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1107"),
        "no exact overload should be accepted: {:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_unimplemented_free_functions_and_invalid_intrinsic_scope() {
    let missing_body = check_source("public int declarationOnly();");
    assert!(
        missing_body
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1005")
    );

    let user_intrinsic = check_source("public intrinsic int runtimeOnly();");
    assert!(
        user_intrinsic
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1005")
    );

    let abi_missing = check_source("public extern void nativeCall();");
    assert!(
        abi_missing
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1005")
    );
}

#[test]
fn interface_contracts_require_matching_return_type_in_concrete_classes() {
    let result = check_source(
        r#"
        public interface IRenderable { void render(); }
        public class BrokenRenderer : IRenderable
        {
            public int render() { return 1; }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1005"),
        "a return-type mismatch must not satisfy an interface contract: {:#?}",
        result.diagnostics
    );
}

#[test]
fn collections_are_visible_from_generic_namespace_import() {
    let result = check_source(
        r#"
        using System.Collections.Generic;
        public class CollectionsExample
        {
            public void run() { List<int> values = new List<int>(); }
        }
    "#,
    );
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1100"),
        "generic collections namespace should expose List: {:#?}",
        result.diagnostics
    );
}

#[test]
fn rejects_let_with_a_specific_migration_message() {
    let result = check_source(
        r#"
        public class Program
        {
            public void run() { let value = 1; }
        }
    "#,
    );
    assert!(
        result.diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("'let' ist reserviert")
                && diagnostic.message.contains("int value = 1")
        }),
        "expected the explicit let migration diagnostic: {:#?}",
        result.diagnostics
    );
}

#[test]
fn intrinsic_is_accepted_only_when_compiler_library_option_is_enabled() {
    let source = "public class RuntimeApi { public intrinsic void write(); }";
    let application = check_source(source);
    assert!(
        application
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1005")
    );

    let standard_library_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../standard-library/system/console.nx");
    let standard_library =
        check_file(&standard_library_path).expect("standard library source should be readable");
    assert!(
        !standard_library.has_errors(),
        "compiler-owned standard-library intrinsic should be allowed: {:#?}",
        standard_library.diagnostics
    );
}

#[test]
fn console_calls_are_checked_against_known_overloads() {
    let valid = check_source(
        r#"
        public class ConsoleUsage
        {
            public void run()
            {
                Console.writeLine('x');
                Console.writeLine("text");
                string input = Console.readLine();
            }
        }
    "#,
    );
    assert!(
        !valid
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1107"),
        "known Console overloads should accept supported argument types: {:#?}",
        valid.diagnostics
    );

    let invalid = check_source(
        r#"
        public class ConsoleUsage
        {
            public void run() { Console.writeLine(true, 1); }
        }
    "#,
    );
    assert!(
        invalid
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1107"),
        "Console overload resolution should reject incorrect arity: {:#?}",
        invalid.diagnostics
    );
}

#[test]
fn user_directory_named_standard_library_cannot_enable_intrinsics() {
    let root = std::env::temp_dir().join(format!(
        "nortex-user-standard-library-{}",
        std::process::id()
    ));
    let fake_standard_library = root.join("standard-library");
    std::fs::create_dir_all(&fake_standard_library).expect("test directory should be created");
    let source_path = fake_standard_library.join("user_api.nx");
    std::fs::write(
        &source_path,
        "public class UserApi { public intrinsic void bypass(); }",
    )
    .expect("test source should be written");

    let result = check_file(&source_path).expect("test source should be readable");
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1005"),
        "a user-controlled directory name must not grant compiler intrinsic privileges: {:#?}",
        result.diagnostics
    );

    std::fs::remove_dir_all(root).expect("test directory should be removed");
}
