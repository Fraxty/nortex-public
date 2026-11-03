//! Regression tests for the 0.3.4-dev type checker.
//!
//! Three groups:
//! 1. `accepts_*`: valid NORTEX code that must not produce diagnostics.
//!    Each case was a verified false positive in the 0.3.4-dev delivery.
//! 2. `rejects_*`: invalid code with an exact code, line and column.
//! 3. `gap_*`: regressions that documented earlier semantic gaps. They now run
//!    normally so the implemented checks remain covered.

use nortex_compiler::diagnostics::{Diagnostic, Severity};
use nortex_compiler::driver::check_source;

fn errors(source: &str) -> Vec<Diagnostic> {
    check_source(source)
        .diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .collect()
}

#[track_caller]
fn assert_accepts(source: &str) {
    let errors = errors(source);
    assert!(errors.is_empty(), "expected no errors, got: {errors:#?}");
}

/// Asserts exactly one error with `code` at `line:column` (1-based).
#[track_caller]
fn assert_single_error(source: &str, code: &str, line: usize, column: usize) {
    let errors = errors(source);
    assert_eq!(
        errors.len(),
        1,
        "expected exactly one error, got: {errors:#?}"
    );
    let error = &errors[0];
    assert_eq!(error.code, code, "wrong code: {error:#?}");
    assert_eq!(
        (error.span.start.line, error.span.start.column),
        (line, column),
        "wrong position: {error:#?}"
    );
}

/// Asserts that an error with `code` exists at `line:column`.
#[track_caller]
fn assert_has_error(source: &str, code: &str, line: usize, column: usize) {
    let errors = errors(source);
    assert!(
        errors.iter().any(|error| error.code == code
            && error.span.start.line == line
            && error.span.start.column == column),
        "expected {code} at {line}:{column}, got: {errors:#?}"
    );
}

// ---------------------------------------------------------------------------
// 1. Valid programs (verified false positives in 0.3.4-dev)
// ---------------------------------------------------------------------------

#[test]
fn accepts_hex_and_binary_literals_containing_e_f_or_d() {
    // 0xFF, 0x1E and 0xD were classified as floating-point literals.
    assert_accepts(
        "public class A { public void u() { int a = 0xFF; int b = 0x1E; int c = 0xD; int d = 0b1010; } }",
    );
}

#[test]
fn accepts_unsuffixed_integer_literals_for_every_integer_type() {
    assert_accepts(
        "public class A { public void u() { byte a = 1; sbyte b = -1; short c = 2; ushort d = 3; \
         uint e = 4; long f = 5; ulong g = 6; usize h = 0; isize i = -7; } }",
    );
}

#[test]
fn accepts_integer_literals_for_floating_point_targets_and_float_suffix() {
    assert_accepts(
        "public class A { public void u() { double a = 2; float b = 1.5f; double c = 1.5e3; } }",
    );
}

#[test]
fn accepts_integer_literal_operands_next_to_other_integer_types() {
    assert_accepts(
        "public class A { public void u() { string s = \"ab\"; usize n = s.length(); \
         bool b = n > 0; long l = 1; long m = l + 1; } }",
    );
}

#[test]
fn accepts_ok_and_err_against_a_declared_result_type() {
    // Plan example (phase 4): Result<int, string> divide(...)
    assert_accepts(
        "public class M { public Result<int, string> divide(int first, int second) { \
         if (second == 0) { return Err(\"Division by zero\"); } return Ok(first / second); } }",
    );
    assert_accepts(
        "public class A { public void u() { Result<int, string> r = Ok(1); Option<int> o = Some(1); } }",
    );
}

#[test]
fn accepts_plan_construction_syntax_without_new_for_default_constructor() {
    // Plan example: Box<Player> player = Box.new(Player());
    assert_accepts(
        "public class Player { public void takeDamage(int d) { } } \
         public class G { public void u() { Box<Player> player = Box.new(Player()); player.takeDamage(20); } }",
    );
    assert_accepts(
        "public struct V { public int x; } public class A { public void u() { V v = V(); int a = v.x; } }",
    );
}

#[test]
fn accepts_upcast_to_base_class_and_interface() {
    assert_accepts(
        "public class B { } public class C : B { } public class A { public void u() { B b = new C(); } }",
    );
    assert_accepts(
        "public interface I { void d(); } public class C : I { public void d() { } } \
         public class A { public void u() { I i = new C(); i.d(); } }",
    );
    assert_accepts(
        "public class B { } public class C : B { } \
         public class A { public void take(B b) { } public void u() { take(new C()); } }",
    );
}

#[test]
fn accepts_generic_class_construction_and_substituted_members() {
    assert_accepts(
        "public class Box2<T> { private T v; public Box2(T v) { this.v = v; } public T get() { return v; } } \
         public class A { public void u() { Box2<int> b = new Box2<int>(1); int x = b.get(); \
         Box2<string> s = Box2<string>(\"a\"); string y = s.get(); } }",
    );
}

#[test]
fn accepts_inherited_protected_members_by_simple_name() {
    assert_accepts(
        "public class B { protected int x; protected void f() { } } \
         public class C : B { public int g() { f(); return x; } }",
    );
}

#[test]
fn accepts_foreach_over_arrays_collections_and_chars() {
    assert_accepts(
        "public class A { public void u(int[] a) { for (int x in a) { int y = x + 1; } } }",
    );
    assert_accepts(
        "using System.Collections; public class A { public void u() { Vec<int> v = Vec<int>(); \
         List<int> l = List<int>(); for (int x in v) { } for (int y in l) { } } }",
    );
    assert_accepts(
        "public class A { public void u() { string s = \"ab\"; for (char c in s.chars()) { } } }",
    );
    assert_accepts(
        "public class A { public void u(int[] a) { for (int x in a) { if (x > 1) { break; } continue; } } }",
    );
}

#[test]
fn accepts_console_overloads_with_integer_literals() {
    assert_accepts(
        "using System; public class A { public void u() { Console.writeLine(1); Console.writeLine(\"x\"); } }",
    );
}

// ---------------------------------------------------------------------------
// 2. Invalid programs with exact code and position
// ---------------------------------------------------------------------------

#[test]
fn rejects_out_of_range_integer_literals() {
    assert_single_error(
        "public class A { public void u() { byte b = 300; } }",
        "NX1102",
        1,
        45,
    );
    assert_single_error(
        "public class A { public void u() { int i = 99999999999; } }",
        "NX1102",
        1,
        44,
    );
    assert_single_error(
        "public class A { public void u() { byte c = -1; } }",
        "NX1102",
        1,
        45,
    );
    assert_single_error(
        "public class A { public void u() { int e = 0xFFFFFFFF; } }",
        "NX1102",
        1,
        44,
    );
    assert_accepts("public class A { public void u() { sbyte d = -128; long f = 0xFFFFFFFF; } }");
}

#[test]
fn rejects_wrong_element_type_for_vec_push_and_list_add() {
    assert_single_error(
        "using System.Collections; public class A { public void u() { Vec<int> v = Vec<int>(); v.push(\"x\"); } }",
        "NX1107",
        1,
        87,
    );
    assert_single_error(
        "using System.Collections; public class A { public void u() { List<int> l = List<int>(); l.add(\"x\"); } }",
        "NX1107",
        1,
        89,
    );
}

#[test]
fn rejects_generic_argument_mismatch() {
    assert_single_error(
        "using System.Collections; public class A { public void u() { List<int> l = List<string>(); } }",
        "NX1102",
        1,
        76,
    );
}

#[test]
fn rejects_downcast_without_explicit_conversion() {
    assert_single_error(
        "public class B { } public class C : B { } public class A { public void u() { C c = new B(); } }",
        "NX1102",
        1,
        84,
    );
}

#[test]
fn private_base_members_stay_invisible_in_subclasses() {
    assert_single_error(
        "public class B { private int y; } public class C : B { public int g() { return y; } }",
        "NX1004",
        1,
        80,
    );
}

#[test]
fn rejects_foreach_over_non_iterable_and_wrong_element_type() {
    assert_single_error(
        "public class A { public void u() { int n = 3; for (int x in n) { } } }",
        "NX1107",
        1,
        61,
    );
    assert_single_error(
        "public class A { public void u(int[] a) { for (string x in a) { } } }",
        "NX1102",
        1,
        48,
    );
}

#[test]
fn foreach_variable_is_scoped_to_the_loop_body() {
    assert_single_error(
        "public class A { public void u(int[] a) { for (int x in a) { } int z = x; } }",
        "NX1004",
        1,
        72,
    );
}

#[test]
fn plan_nx1004_example_reports_the_exact_position() {
    // Plan 9.2 example. The plan text says column 9; the identifier begins
    // in column 14 with the indentation used here (see audit reports).
    let source = "public class Player\n{\n    public void update()\n    {\n        float damage = playerHealth * 2;\n    }\n}\n";
    assert_has_error(source, "NX1004", 5, 24);
}

#[test]
fn rejects_constructor_arity_and_overload_mismatch() {
    assert_single_error(
        "public class W { public W(string t) { } } public class A { public void u() { W w = new W(\"a\", 1, 2); } }",
        "NX1107",
        1,
        84,
    );
    assert_single_error(
        "public class Container<T> { public Container(T value) { } } \
         public class A { public void u() { Container<int> c = Container<int>(); } }",
        "NX1107",
        1,
        115,
    );
}

// ---------------------------------------------------------------------------
// 3. Documented gaps (fail today; tracked with --ignored)
// ---------------------------------------------------------------------------

#[test]
fn gap_private_field_access_from_another_class() {
    assert_single_error(
        "public class P { private int x; } public class A { public void u(P p) { int y = p.x; } }",
        "NX1105",
        1,
        81,
    );
}

#[test]
fn gap_private_method_call_from_another_class() {
    let errors = errors(
        "public class P { private void f() { } } public class A { public void u(P p) { p.f(); } }",
    );
    assert_eq!(errors.len(), 1, "{errors:#?}");
}

#[test]
fn gap_static_call_of_instance_method() {
    let errors = errors(
        "public class P { public void f() { } } public class A { public void u() { P.f(); } }",
    );
    assert_eq!(errors.len(), 1, "{errors:#?}");
}

#[test]
fn gap_missing_return_on_some_path() {
    let errors = errors("public class A { public int f(int a) { if (a > 0) { return 1; } } }");
    assert!(
        errors.iter().any(|error| error.code == "NX1103"),
        "{errors:#?}"
    );
    assert_accepts(
        "public class A { public int f(int a) { if (a > 0) { return 1; } else { return 2; } } }",
    );
    assert_accepts("public class A { public int f(int a) { while (true) { return a; } } }");
}

#[test]
fn gap_ignored_result_value() {
    let errors = errors(
        "public class A { public Result<int, string> load() { return Ok(1); } public void u() { load(); } }",
    );
    assert_eq!(errors.len(), 1, "{errors:#?}");
}

#[test]
fn gap_non_exhaustive_enum_match() {
    let errors = errors(
        "public enum Color { Red, Green } public class A { public void u(Color c) { match (c) { Color.Red => { } } } }",
    );
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert_ne!(
        errors[0].code, "NX0100",
        "the pattern must parse: {errors:#?}"
    );
}

#[test]
fn class_references_can_be_reused_after_passing() {
    assert_accepts(
        "public class P { } public class A { public void t(P p) { } public void u() { P a = new P(); t(a); t(a); } }",
    );
}

#[test]
fn move_state_is_scoped_to_the_binding_when_names_shadow() {
    assert_accepts(
        "public class P { } public class A { public void take(P p) { } public void u() { \
         { P value = P(); take(value); } { P value = P(); take(value); } } }",
    );
}

#[test]
fn move_and_borrow_analysis_handles_borrows_reinitialization_and_conflicts() {
    assert_accepts(
        "public class P { } public class A { public void inspect(&P p) { } \
         public void u() { P value = P(); inspect(&value); inspect(&value); } }",
    );
    assert_accepts(
        "public class P { } public class A { public void take(P p) { } \
         public void u() { mut P value = P(); take(value); value = P(); take(value); } }",
    );
    let conflicting_borrow = errors(
        "public class P { } public class A { public void both(&mut P a, &P b) { } \
         public void u() { mut P value = P(); both(&mut value, &value); } }",
    );
    assert_eq!(conflicting_borrow.len(), 1, "{conflicting_borrow:#?}");
    assert_eq!(conflicting_borrow[0].code, "NX1203");
    let immutable_borrow = errors(
        "public class P { } public class A { public void mutate(&mut P p) { } \
         public void u() { P value = P(); mutate(&mut value); } }",
    );
    assert_eq!(immutable_borrow.len(), 1, "{immutable_borrow:#?}");
    assert_eq!(immutable_borrow[0].code, "NX1201");
}

#[test]
fn borrow_conflicts_use_projected_places_instead_of_only_root_names() {
    assert_accepts(
        "public class Pair { public int left; public int right; } \
         public class A { public void inspect(&mut int a, &int b) { } \
         public void u() { mut Pair pair = Pair(); inspect(&mut pair.left, &pair.right); } }",
    );

    let same_field = errors(
        "public class Pair { public int left; } \
         public class A { public void inspect(&mut int a, &int b) { } \
         public void u() { mut Pair pair = Pair(); inspect(&mut pair.left, &pair.left); } }",
    );
    assert_eq!(same_field.len(), 1, "{same_field:#?}");
    assert_eq!(same_field[0].code, "NX1203");

    let unknown_roots = errors(
        "public class Pair { } public class A { public Pair get() { return Pair(); } \
         public void inspect(&mut Pair a, &Pair b) { } \
        public void u() { inspect(&mut get(), &get()); } }",
    );
    assert_eq!(unknown_roots.len(), 1, "{unknown_roots:#?}");
    assert_eq!(unknown_roots[0].code, "NX1203");
}

#[test]
fn gap_generic_method_call() {
    assert_accepts(
        "public class A { public T id<T>(T v) { return v; } public void u() { int x = id<int>(1); } }",
    );
}

#[test]
fn gap_generic_method_argument_substitution() {
    let errors = errors(
        "public class A { public T id<T>(T v) { return v; } public void u() { string s = id(1); } }",
    );
    assert_eq!(errors.len(), 1, "{errors:#?}");
}

#[test]
fn gap_string_concatenation_with_numbers() {
    assert_accepts("public class A { public void u() { string t = \"n=\" + 1; } }");
}

#[test]
fn gap_literal_overload_ambiguity_is_specified() {
    // f(long) and f(double) with argument 1: today picks neither exact `int`,
    // so the call is reported as ambiguous. The language spec must decide.
    assert_accepts(
        "public class A { public void f(long a) { } public void f(double a) { } public void u() { f(1); } }",
    );
}
