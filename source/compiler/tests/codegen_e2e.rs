#![cfg(windows)]

use std::process::Command;

use nortex_compiler::codegen::{CodeGenerator, Target};
use nortex_compiler::driver::compile_source_to_mir;
use nortex_compiler::linker::{link_object_bytes, link_object_bytes_with_runtime};

#[test]
fn links_and_runs_a_windows_x64_executable() {
    let compiled = compile_source_to_mir("public int main() { return 42; }");
    assert!(
        !compiled.has_errors(),
        "frontend diagnostics: {:#?}",
        compiled.diagnostics
    );

    let object = CodeGenerator::new(Target::X86_64Windows)
        .emit_object(&compiled.functions)
        .expect("supported scalar MIR should generate COFF");
    let output =
        std::env::temp_dir().join(format!("nortex-codegen-e2e-{}.exe", std::process::id()));
    link_object_bytes(&object, &output).expect("MSVC should link the generated COFF object");

    let status = Command::new(&output)
        .status()
        .expect("generated executable should start");
    let exit_code = status.code();
    let cleanup = std::fs::remove_file(&output);
    cleanup.expect("generated executable should be removed");
    assert_eq!(exit_code, Some(42));
}

#[test]
fn links_and_runs_with_read_only_and_mutable_globals() {
    let compiled = compile_source_to_mir(
        "public const int BASE = 40; mut int counter = 1; public int bump() { counter += BASE + 1; return counter; } public int main() { return bump(); }",
    );
    assert!(
        !compiled.has_errors(),
        "global frontend diagnostics: {:#?}",
        compiled.diagnostics
    );

    let object = CodeGenerator::new(Target::X86_64Windows)
        .emit_object(&compiled.functions)
        .expect("global data and accesses should generate COFF");
    let output =
        std::env::temp_dir().join(format!("nortex-globals-e2e-{}.exe", std::process::id()));
    link_object_bytes(&object, &output).expect("MSVC should link the generated COFF object");

    let status = Command::new(&output)
        .status()
        .expect("generated executable should start");
    std::fs::remove_file(&output).expect("temporary executable should be removed");
    assert_eq!(status.code(), Some(42));
}

#[test]
fn links_and_runs_control_flow_with_an_internal_call() {
    let compiled = compile_source_to_mir(
        "public int choose(int value) { if (value > 20) { return 42; } else { return 0; } } public int main() { mut int count = 0; while (count < 1) { count += 1; } return choose(21); }",
    );
    assert!(
        !compiled.has_errors(),
        "frontend diagnostics: {:#?}",
        compiled.diagnostics
    );

    assert!(
        compiled.functions.iter().any(|function| {
            function.body.basic_blocks.iter().any(|block| {
                matches!(
                    block.terminator.as_ref().map(|terminator| &terminator.kind),
                    Some(nortex_compiler::mir::TerminatorKind::Call { .. })
                )
            })
        }),
        "MIR should retain the internal function call"
    );
    assert!(
        compiled.functions.iter().any(|function| {
            function.body.basic_blocks.iter().any(|block| {
                matches!(
                    block.terminator.as_ref().map(|terminator| &terminator.kind),
                    Some(nortex_compiler::mir::TerminatorKind::SwitchInt { .. })
                )
            })
        }),
        "MIR should represent the conditional branch"
    );

    let object = CodeGenerator::new(Target::X86_64Windows)
        .emit_object(&compiled.functions)
        .expect("calls and conditional branches should lower to COFF");
    let output = std::env::temp_dir().join(format!(
        "nortex-control-flow-e2e-{}.exe",
        std::process::id()
    ));
    link_object_bytes(&object, &output).expect("MSVC should link the generated COFF object");

    let status = Command::new(&output)
        .status()
        .expect("generated executable should start");
    let exit_code = status.code();
    std::fs::remove_file(&output).expect("generated executable should be removed");
    assert_eq!(exit_code, Some(42));
}

#[test]
fn example_native_sum_builds_and_returns_its_result() {
    let compiled = compile_source_to_mir(include_str!("../../examples/native-sum/main.nx"));
    assert!(
        !compiled.has_errors(),
        "example frontend diagnostics: {:#?}",
        compiled.diagnostics
    );

    let object = CodeGenerator::new(Target::X86_64Windows)
        .emit_object(&compiled.functions)
        .expect("native-sum example should generate COFF");
    let output =
        std::env::temp_dir().join(format!("nortex-native-sum-e2e-{}.exe", std::process::id()));
    link_object_bytes_with_runtime(&object, &output)
        .expect("MSVC should link the example executable with Nortex Runtime");

    let result = Command::new(&output)
        .output()
        .expect("native-sum executable should start");
    std::fs::remove_file(&output).expect("temporary example executable should be removed");
    assert_eq!(result.status.code(), Some(55));
    assert_eq!(result.stdout, b"The sum from 1 through 10 is 55.\n");
}

#[test]
fn runtime_console_output_is_linked_and_written_to_stdout() {
    let compiled = compile_source_to_mir(
        "using System; public int main() { Console.writeLine(\"Grüße aus NORTEX\"); Console.write(\"sum=\"); Console.writeLine(\"55\"); return 0; }",
    );
    assert!(
        !compiled.has_errors(),
        "console output frontend diagnostics: {:#?}",
        compiled.diagnostics
    );
    assert!(
        CodeGenerator::requires_runtime(&compiled.functions),
        "console intrinsics must request the Nortex runtime at link time"
    );

    let object = CodeGenerator::new(Target::X86_64Windows)
        .emit_object(&compiled.functions)
        .expect("console write intrinsics should generate COFF");
    let output =
        std::env::temp_dir().join(format!("nortex-console-e2e-{}.exe", std::process::id()));
    link_object_bytes_with_runtime(&object, &output)
        .expect("MSVC should link the generated COFF object with Nortex Runtime");

    let result = Command::new(&output)
        .output()
        .expect("console output executable should start");
    std::fs::remove_file(&output).expect("temporary console executable should be removed");
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, "Grüße aus NORTEX\nsum=55\n".as_bytes());
    assert!(
        result.stderr.is_empty(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn string_slices_flow_through_variables_parameters_returns_and_console_output() {
    let compiled = compile_source_to_mir(
        "using System; public string echo(string value) { string copy = value; return copy; } public int main() { string greeting = echo(\"Grüße aus NORTEX\"); Console.writeLine(greeting); Console.writeLine(\"\"); Console.writeLine(\"a\\0b\"); return 0; }",
    );
    assert!(
        !compiled.has_errors(),
        "string-slice frontend diagnostics: {:#?}",
        compiled.diagnostics
    );

    let object = CodeGenerator::new(Target::X86_64Windows)
        .emit_object(&compiled.functions)
        .expect("string slices should generate COFF");
    let output = std::env::temp_dir().join(format!(
        "nortex-string-slice-e2e-{}.exe",
        std::process::id()
    ));
    link_object_bytes_with_runtime(&object, &output)
        .expect("MSVC should link the string-slice executable with Nortex Runtime");

    let result = Command::new(&output)
        .output()
        .expect("string-slice executable should start");
    std::fs::remove_file(&output).expect("temporary string-slice executable should be removed");
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, "Grüße aus NORTEX\n\na\0b\n".as_bytes());
    assert!(
        result.stderr.is_empty(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn run_string_program(source: &str, tag: &str) -> (Option<i32>, Vec<u8>) {
    let compiled = compile_source_to_mir(source);
    assert!(
        !compiled.has_errors(),
        "frontend diagnostics: {:#?}",
        compiled.diagnostics
    );
    let object = CodeGenerator::new(Target::X86_64Windows)
        .emit_object(&compiled.functions)
        .expect("string program should generate COFF");
    let output = std::env::temp_dir().join(format!("nortex-{tag}-{}.exe", std::process::id()));
    link_object_bytes_with_runtime(&object, &output).expect("MSVC should link with Nortex Runtime");
    let result = Command::new(&output).output().expect("executable starts");
    std::fs::remove_file(&output).expect("temporary executable should be removed");
    assert!(
        result.stderr.is_empty(),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    (result.status.code(), result.stdout)
}

#[test]
fn string_concatenation_allocates_new_heap_strings() {
    let (code, stdout) = run_string_program(
        "using System; public string join(string a, string b) { return a + b; } public int main() { string both = join(\"Gr\", \"üße\") + \"!\"; Console.writeLine(both); Console.writeLine(\"\" + \"\"); mut string acc = \"\"; mut int i = 0; while (i < 3) { acc = acc + \"ab\"; i += 1; } Console.writeLine(acc); return 0; }",
        "string-concat-e2e",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "Grüße!\n\nababab\n".as_bytes());
}

#[test]
fn string_globals_are_readable_and_mutable_descriptors() {
    let (code, stdout) = run_string_program(
        "using System; const string GREETING = \"Hallo\"; public const string SUFFIX = \"!\" + \"?\"; mut string name = \"Welt\"; public int main() { Console.writeLine(GREETING + \" \" + name); name = name + SUFFIX; Console.writeLine(name); return 0; }",
        "string-globals-e2e",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "Hallo Welt\nWelt!?\n".as_bytes());
}

#[test]
fn strings_are_passed_to_extern_c_as_nul_terminated_pointers() {
    let (code, _) = run_string_program(
        "public extern \"C\" int lstrlenA(string text); public int main() { string joined = \"Nor\" + \"tex\"; return lstrlenA(joined) + lstrlenA(\"ab\"); }",
        "string-extern-e2e",
    );
    assert_eq!(code, Some(8));
}

#[test]
fn string_equality_compares_contents() {
    let (code, stdout) = run_string_program(
        "using System; public int main() { string a = \"ab\" + \"c\"; string b = \"a\" + \"bc\"; mut int r = 0; if (a == b) { r += 1; } if (a != b) { r += 100; } if (a == \"abd\") { r += 100; } if (a != \"abd\") { r += 2; } if (\"\" + \"\" == \"\") { r += 4; } if (\"a\\0b\" == \"a\\0c\") { r += 100; } Console.writeLine(a); return r; }",
        "string-eq-e2e",
    );
    assert_eq!(code, Some(7));
    assert_eq!(stdout, "abc\n".as_bytes());
}

#[test]
fn heap_strings_are_freed_at_scope_exit_unless_moved() {
    let (code, _) = run_string_program(
        "public extern \"C\" int nortex_runtime_live_strings(); public int work(int n) { mut int i = 0; while (i < n) { string t = \"a\" + \"b\"; string u = t + \"c\"; i += 1; } return 0; } public string make() { string s = \"x\" + \"y\"; return s; } public int take(string s) { return 0; } public int main() { work(5); string kept = make(); take(kept + \"z\"); return nortex_runtime_live_strings(); }",
        "string-free-e2e",
    );
    assert_eq!(code, Some(1));
}

#[test]
fn live_heap_string_is_counted_before_scope_exit() {
    let (code, _) = run_string_program(
        "public extern \"C\" int nortex_runtime_live_strings(); public int main() { string held = \"a\" + \"b\"; return nortex_runtime_live_strings(); }",
        "string-live-e2e",
    );
    assert_eq!(code, Some(1));
}

#[test]
fn use_after_move_of_strings_reports_nx1202() {
    for source in [
        "using System; public int main() { string a = \"x\" + \"y\"; string b = a; Console.writeLine(a); return 0; }",
        "public int take(string s) { return 0; } public int main() { string a = \"x\" + \"y\"; take(a); take(a); return 0; }",
    ] {
        let compiled = compile_source_to_mir(source);
        assert!(
            compiled
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "NX1202"),
            "{:#?}",
            compiled.diagnostics
        );
    }
}

#[test]
fn ternary_strings_and_mut_global_copies_do_not_leak_or_fail() {
    let (code, stdout) = run_string_program(
        "using System; public extern \"C\" int nortex_runtime_live_strings(); mut string name = \"a\"; public int main() { name = name + \"b\"; name = name + \"c\"; string copy = name; name = \"\" + \"z\"; bool c = true; string x = \"p\" + \"q\"; string y = \"r\" + \"s\"; string pick = c ? x : y; Console.writeLine(copy); Console.writeLine(pick); Console.writeLine(name); return 0; }",
        "string-ternary-global-e2e",
    );
    assert_eq!(code, Some(0));
    assert_eq!(stdout, "abc\npq\nz\n".as_bytes());
}

fn run_overflow_program(overflow_checks: bool, tag: &str) -> Option<i32> {
    let compiled = compile_source_to_mir(
        "public int add(int a, int b) { return a + b; } public int main() { if (add(2147483647, 1) < 0) { return 7; } return 9; }",
    );
    assert!(
        !compiled.has_errors(),
        "frontend diagnostics: {:#?}",
        compiled.diagnostics
    );
    let object = CodeGenerator::new(Target::X86_64Windows)
        .with_overflow_checks(overflow_checks)
        .emit_object(&compiled.functions)
        .expect("scalar MIR should generate COFF");
    let output =
        std::env::temp_dir().join(format!("nortex-overflow-{tag}-{}.exe", std::process::id()));
    link_object_bytes(&object, &output).expect("MSVC should link the generated COFF object");
    let status = Command::new(&output)
        .status()
        .expect("generated executable should start");
    std::fs::remove_file(&output).expect("generated executable should be removed");
    status.code()
}

#[test]
fn default_codegen_wraps_on_signed_overflow() {
    assert_eq!(run_overflow_program(false, "wrap"), Some(7));
}

#[test]
fn overflow_checks_trap_on_signed_overflow() {
    let code = run_overflow_program(true, "trap");
    assert_eq!(code, Some(0xC000_001Du32 as i32));
}

#[test]
fn text_convert_and_file_builtins_run_natively() {
    let (code, stdout) = run_string_program(
        "using System; public int main() { string path = \"nortex_builtin_e2e.txt\"; File.writeAllText(path, \"  Grüße, Welt  \"); string text = Text.trim(File.readAllText(path)); Console.writeLine(text); Console.writeLine(Text.toUpper(text)); Console.writeLine(Convert.toString(Text.length(text))); Console.writeLine(Text.replace(text, \"Welt\", \"NORTEX\")); if (Text.contains(text, \"Welt\")) { Console.writeLine(\"ja\"); } mut int n = Convert.toInt(\"40\") + 2; File.delete(path); if (File.exists(path)) { return 1; } return n; }",
        "builtins-e2e",
    );
    assert_eq!(code, Some(42));
    assert_eq!(
        stdout,
        "Grüße, Welt\nGRÜSSE, WELT\n13\nGrüße, NORTEX\nja\n".as_bytes()
    );
}

#[test]
fn builtin_calls_check_argument_types() {
    let compiled = compile_source_to_mir("public int main() { return Text.length(5); }");
    assert!(compiled.has_errors());
}

#[test]
fn double_arithmetic_comparisons_and_conversions_run_natively() {
    let (code, stdout) = run_string_program(
        "using System; public double half(double x) { return x / 2.0; } public double hyp(double a, double b) { return Math.sqrt(a * a + b * b); } public int main() { double a = 3.0; double b = 4.0; double h = hyp(a, b); Console.writeLine(Convert.doubleToString(h)); Console.writeLine(Convert.doubleToString(half(5.0) + 0.25)); mut double acc = 0.0; mut int i = 0; while (i < 4) { acc += 0.5; i += 1; } Console.writeLine(Convert.doubleToString(acc)); Console.writeLine(Convert.doubleToString(Math.pow(2.0, 10.0))); Console.writeLine(Convert.doubleToString(Math.floor(-1.5))); Console.writeLine(Convert.doubleToString(Math.abs(-2.5))); Console.writeLine(Convert.doubleToString(-acc)); Console.writeLine(Convert.doubleToString(Convert.toDouble(7) / 2.0)); double p = Convert.parseDouble(\"1.5\"); if (Convert.isDouble(\"abc\")) { return 1; } if (!Convert.isDouble(\" 2.5 \")) { return 2; } if (a < b && h == 5.0 && b >= 4.0 && a != b && !(a > b) && a <= 3.0) { return Convert.doubleToInt(p * 10.0) + 27; } return 3; }",
        "double-e2e",
    );
    assert_eq!(code, Some(42));
    assert_eq!(
        stdout,
        "5.0\n2.75\n2.0\n1024.0\n-2.0\n2.5\n-2.0\n3.5\n".as_bytes()
    );
}

#[test]
fn int_and_string_lists_run_natively_and_do_not_leak_strings() {
    let (code, stdout) = run_string_program(
        "using System; public extern \"C\" int nortex_runtime_live_strings(); public int sum(IntList xs) { mut int total = 0; mut int i = 0; while (i < IntList.length(xs)) { total += IntList.get(xs, i); i += 1; } return total; } public IntList make() { IntList xs = IntList.create(); IntList.add(xs, 1); IntList.add(xs, 2); IntList.add(xs, 3); return xs; } public int work() { StringList parts = Text.split(\"a,b,c\", \",\"); StringList.add(parts, \"d\"); StringList.set(parts, 0, \"A\"); Console.writeLine(Text.join(parts, \"-\")); Console.writeLine(StringList.get(parts, 2)); Console.writeLine(Convert.toString(StringList.length(parts))); mut int i = 0; while (i < 3) { string s = StringList.get(parts, i); i += 1; } StringList.free(parts); return 0; } public int main() { IntList xs = make(); IntList.set(xs, 0, 10); int s = sum(xs); IntList.free(xs); work(); return s + nortex_runtime_live_strings(); }",
        "lists-e2e",
    );
    assert_eq!(stdout, "A-b-c-d\nc\n4\n".as_bytes());
    assert_eq!(code, Some(15));
}
