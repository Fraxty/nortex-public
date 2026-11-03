use nortex_compiler::ast::{Declaration, MemberDeclaration};
use nortex_compiler::driver::{check_source, compile_source_to_mir};
use nortex_compiler::mir::{Rvalue, StatementKind};

const HELLO_WORLD: &str = include_str!("../../tests/integration/hello-world.nx");

#[test]
fn hello_world_frontend_resolves_entrypoint_and_console_call() {
    let result = check_source(HELLO_WORLD);
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );

    let program_class = result.program.declarations.iter().find_map(|declaration| {
        let Declaration::Class(class) = declaration else {
            return None;
        };
        (class.name == "Program").then_some(class)
    });
    let program_class = program_class.expect("Program class should be parsed");
    assert!(program_class.members.iter().any(|member| {
        matches!(
            member,
            MemberDeclaration::Method(method)
                if method.name == "main" && method.modifiers.iter().any(|m| m == "static")
        )
    }));
}

#[test]
fn parser_recovers_at_a_semicolon_and_keeps_following_declarations() {
    let result = check_source(
        r#"
        public class Broken
        {
            public void run()
            {
                int broken = ;
                int later = 1;
            }
        }
        public class AfterError
        {
            public int value() { return 1; }
        }
        "#,
    );

    assert!(result.has_errors(), "the malformed initializer must fail");
    assert!(
        result.program.declarations.iter().any(
            |declaration| matches!(declaration, Declaration::Class(class) if class.name == "AfterError")
        ),
        "parser should recover and retain later declarations: {:#?}",
        result.program
    );
}

#[test]
fn mutation_requires_the_explicit_mut_modifier() {
    let immutable =
        check_source("public class C { public void run() { int value = 1; value = 2; } }");
    assert!(
        immutable
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1200")
    );

    let mutable =
        check_source("public class C { public void run() { mut int value = 1; value = 2; } }");
    assert!(!mutable.has_errors(), "{:#?}", mutable.diagnostics);
}

#[test]
fn source_pipeline_lowers_and_optimizes_scalar_functions_to_mir() {
    let result = compile_source_to_mir("public int calculate() { int unused = 9; return 2 + 3; }");
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
    assert_eq!(result.functions.len(), 1);
    let body = &result.functions[0].body;
    body.validate().expect("optimized MIR should remain valid");
    let statements = &body.basic_blocks[0].statements;
    assert!(
        statements.iter().all(|statement| !matches!(
            statement.kind,
            StatementKind::Assign(_, Rvalue::BinaryOp(..))
        )),
        "constant binary expression should be folded: {statements:#?}"
    );
    assert!(
        statements.iter().all(|statement| !matches!(
            &statement.kind,
            StatementKind::Assign(place, _)
                if body.local_decls[place.local.0].name.as_deref() == Some("unused")
        )),
        "unused local assignment should be eliminated: {statements:#?}"
    );
}

#[test]
fn source_pipeline_preserves_parameter_bindings_in_mir() {
    let result = compile_source_to_mir("public int identity(int value) { return value; }");
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
    let body = &result.functions[0].body;
    body.validate().expect("parameter MIR should be valid");
    assert_eq!(body.arg_count, 1);
    assert_eq!(body.local_decls[1].name.as_deref(), Some("value"));
}

#[test]
fn source_pipeline_lowers_global_storage_and_mutable_access() {
    let result = compile_source_to_mir(
        "public const int BASE = 40; mut int counter = 1; public int main() { counter += BASE + 1; return counter; }",
    );
    assert!(
        !result.has_errors(),
        "global declarations should lower to MIR: {:#?}",
        result.diagnostics
    );
    let body = &result.functions[0].body;
    body.validate().expect("global MIR should be valid");
    assert_eq!(body.globals.len(), 2);
    assert!(
        body.globals
            .iter()
            .any(|global| global.name == "BASE" && !global.mutable)
    );
    assert!(
        body.globals
            .iter()
            .any(|global| global.name == "counter" && global.mutable)
    );
    assert!(
        body.local_decls
            .iter()
            .any(|local| local.global_symbol.as_deref() == Some("counter"))
    );
}

#[test]
fn source_pipeline_lowers_mutable_local_assignments_and_compound_updates() {
    let result = compile_source_to_mir(
        "public int calculate() { mut int value = 1; value += 2; return value; }",
    );
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
    let body = &result.functions[0].body;
    body.validate().expect("assignment MIR should be valid");
    assert!(
        body.basic_blocks[0]
            .statements
            .iter()
            .any(|statement| matches!(
                &statement.kind,
                StatementKind::Assign(place, Rvalue::BinaryOp(
                    nortex_compiler::mir::BinaryOp::Add,
                    _,
                    nortex_compiler::mir::Operand::ConstantInt(2),
                )) if body.local_decls[place.local.0].name.as_deref() == Some("value")
            ))
    );

    let immutable =
        compile_source_to_mir("public int calculate() { int value = 1; value = 2; return value; }");
    assert!(immutable.has_errors());
}

#[test]
fn source_pipeline_tracks_reborrows_and_dereference_writes() {
    let mutable = compile_source_to_mir(
        "public void update(&mut int value) { &mut int child = &mut *value; *child = 7; }",
    );
    assert!(
        !mutable.has_errors(),
        "mutable reborrow should lower and validate: {:#?}",
        mutable.diagnostics
    );
    let body = &mutable.functions[0].body;
    body.validate().expect("reborrow MIR should be valid");
    assert!(
        body.basic_blocks
            .iter()
            .flat_map(|block| &block.statements)
            .any(|statement| matches!(
                &statement.kind,
                StatementKind::Assign(place, _)
                    if place.projection.contains(&nortex_compiler::mir::ProjectionElem::Deref)
            ))
    );

    let shared = compile_source_to_mir("public void update(&int value) { *value = 7; }");
    assert!(
        shared.has_errors(),
        "writing through a shared reference must be rejected"
    );
}

#[test]
fn source_pipeline_lowers_branching_and_optimizes_constant_loop_edges() {
    let branch = compile_source_to_mir(
        "public int choose(bool condition) { if (condition) { return 1; } else { return 0; } }",
    );
    assert!(
        !branch.has_errors(),
        "diagnostics: {:#?}",
        branch.diagnostics
    );
    branch.functions[0]
        .body
        .validate()
        .expect("branch MIR should be valid after optimization");
    assert!(
        branch.functions[0]
            .body
            .basic_blocks
            .iter()
            .any(|block| matches!(
                block.terminator.as_ref().map(|terminator| &terminator.kind),
                Some(nortex_compiler::mir::TerminatorKind::SwitchInt { .. })
            )),
        "condition should lower to a MIR switch"
    );

    let loop_result = compile_source_to_mir("public void spin() { while (false) { } }");
    assert!(
        !loop_result.has_errors(),
        "diagnostics: {:#?}",
        loop_result.diagnostics
    );
    let loop_body = &loop_result.functions[0].body;
    loop_body
        .validate()
        .expect("optimized loop MIR should be valid");
    assert!(
        loop_body.basic_blocks.iter().all(|block| !matches!(
            block.terminator.as_ref().map(|terminator| &terminator.kind),
            Some(nortex_compiler::mir::TerminatorKind::SwitchInt { .. })
        )),
        "constant false loop edge should be folded"
    );

    let infinite = compile_source_to_mir("public int endless() { while (true) { return 1; } }");
    assert!(
        !infinite.has_errors(),
        "non-fallthrough loop should satisfy MIR lowering: {:#?}",
        infinite.diagnostics
    );
    infinite.functions[0]
        .body
        .validate()
        .expect("infinite-loop MIR should be valid");

    let break_loop = compile_source_to_mir("public void stop() { while (true) { break; } }");
    assert!(
        !break_loop.has_errors(),
        "break must make an infinite loop's exit reachable: {:#?}",
        break_loop.diagnostics
    );
    break_loop.functions[0]
        .body
        .validate()
        .expect("break-loop MIR should be valid");

    let unreachable_break = compile_source_to_mir(
        "public int endless() { while (true) { if (false) { break; } return 1; } }",
    );
    assert!(
        !unreachable_break.has_errors(),
        "a syntactically present but unreachable break must not create a fallthrough path: {:#?}",
        unreachable_break.diagnostics
    );
}

#[test]
fn source_pipeline_lowers_for_loops_with_continue_and_mutable_updates() {
    let result = compile_source_to_mir(
        "public int calculate() { mut int total = 0; for (mut int i = 0; i < 3; i += 1) { if (i == 1) { continue; } total += i; } return total; }",
    );
    assert!(
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
    result.functions[0]
        .body
        .validate()
        .expect("for-loop MIR should be valid");
}

#[test]
fn source_pipeline_checks_definite_assignment_across_branches() {
    let both_branches = compile_source_to_mir(
        "public int choose(bool condition) { mut int value; if (condition) { value = 1; } else { value = 2; } return value; }",
    );
    assert!(
        !both_branches.has_errors(),
        "assignments on every branch should initialize the local: {:#?}",
        both_branches.diagnostics
    );

    let constant_branch = compile_source_to_mir(
        "public int choose() { mut int value; if (true) { value = 1; } return value; }",
    );
    assert!(
        !constant_branch.has_errors(),
        "the unreachable branch must not weaken definite assignment: {:#?}",
        constant_branch.diagnostics
    );

    let missing_branch = compile_source_to_mir(
        "public int choose(bool condition) { mut int value; if (condition) { value = 1; } return value; }",
    );
    assert!(missing_branch.has_errors());
    assert!(
        missing_branch.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "NX2003" && diagnostic.message.contains("may be uninitialized")
        }),
        "a read not initialized on every incoming path must be rejected: {:#?}",
        missing_branch.diagnostics
    );
}

#[test]
fn source_pipeline_lowers_nested_conditional_and_short_circuit_expressions() {
    let result = compile_source_to_mir(
        "public int select(bool first, bool second) { return first && second ? (first ? 1 : 2) : 3; }",
    );
    assert!(
        !result.has_errors(),
        "nested conditional expressions should lower through CFG joins: {:#?}",
        result.diagnostics
    );
    result.functions[0]
        .body
        .validate()
        .expect("nested conditional MIR should be valid");

    let result = compile_source_to_mir(
        "public bool select(bool first, bool second) { return first || second; }",
    );
    assert!(
        !result.has_errors(),
        "logical expressions should use short-circuit CFG lowering: {:#?}",
        result.diagnostics
    );
    result.functions[0]
        .body
        .validate()
        .expect("short-circuit MIR should be valid");
}

#[test]
fn source_pipeline_checks_borrows_and_scope_lifetimes() {
    let shared = compile_source_to_mir(
        "public int read() { int value = 4; &int reference = &value; return value; }",
    );
    assert!(
        !shared.has_errors(),
        "shared borrows may coexist with reads: {:#?}",
        shared.diagnostics
    );

    let dereference = compile_source_to_mir(
        "public int read() { int value = 4; &int reference = &value; return *reference; }",
    );
    assert!(
        !dereference.has_errors(),
        "dereferencing a live scalar borrow should lower and validate: {:#?}",
        dereference.diagnostics
    );

    let write_while_shared = compile_source_to_mir(
        "public int write() { mut int value = 4; &int reference = &value; value = 5; return *reference; }",
    );
    assert!(
        write_while_shared.has_errors(),
        "writes must conflict with a live shared borrow"
    );

    let overlapping_mutable = compile_source_to_mir(
        "public void borrow() { mut int value = 4; &int shared = &value; &mut int exclusive = &mut value; int observed = *shared; }",
    );
    assert!(
        overlapping_mutable.has_errors(),
        "mutable and shared borrows of the same place must conflict"
    );

    let immutable_mutable_borrow = compile_source_to_mir(
        "public void borrow() { int value = 4; &mut int exclusive = &mut value; }",
    );
    assert!(
        immutable_mutable_borrow.has_errors(),
        "mutable borrows require a mutable source binding"
    );

    let dangling = compile_source_to_mir(
        "public int dangling() { mut &int reference; { int inner = 1; reference = &inner; } return *reference; }",
    );
    assert!(
        dangling.has_errors(),
        "an outer reference must not outlive an inner local"
    );

    let ended_borrow = compile_source_to_mir(
        "public void write() { mut int value = 4; { &int shared = &value; } value = 5; }",
    );
    assert!(
        !ended_borrow.has_errors(),
        "leaving the borrower's lexical region must release its loan: {:#?}",
        ended_borrow.diagnostics
    );

    let break_releases_borrow = compile_source_to_mir(
        "public void write() { mut int value = 4; while (true) { { &int shared = &value; break; } } value = 5; }",
    );
    assert!(
        !break_releases_borrow.has_errors(),
        "break must end borrows in scopes it exits: {:#?}",
        break_releases_borrow.diagnostics
    );

    let copied_dangling = compile_source_to_mir(
        "public int dangling() { mut &int outer; { int inner = 1; &int nested = &inner; outer = nested; } return *outer; }",
    );
    assert!(
        copied_dangling.has_errors(),
        "copying a reference must preserve its referent region: {:#?}",
        copied_dangling.diagnostics
    );
}

#[test]
fn source_pipeline_rejects_reference_types_outside_the_scalar_subset() {
    let result = compile_source_to_mir("public void inspect(&int[] values) { }");
    assert!(result.has_errors());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX2002"),
        "non-scalar reference parameters must not be silently accepted by MIR: {:#?}",
        result.diagnostics
    );
}

#[test]
fn source_pipeline_reports_constructs_outside_the_current_mir_subset() {
    let result = compile_source_to_mir("public void calculate() { print(\"hello\"); }");
    assert!(result.has_errors());
    assert!(result.functions.is_empty());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX2001"),
        "lowering should identify unsupported source constructs: {:#?}",
        result.diagnostics
    );
}

#[test]
fn console_output_rejects_non_string_values() {
    let result = compile_source_to_mir(
        "using System; public int main() { Console.writeLine(5); return 0; }",
    );
    assert!(
        result.has_errors(),
        "non-string console values must be rejected"
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX2002"),
        "expected a string-type diagnostic: {:#?}",
        result.diagnostics
    );
}

#[test]
fn nortexc_mir_command_prints_optimized_output() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock should be after Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "nortex-mir-smoke-{}-{stamp}.nx",
        std::process::id()
    ));
    std::fs::write(&path, "public int calculate() { return 2 + 3; }")
        .expect("temporary NORTEX source should be written");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_nortexc"))
        .arg("mir")
        .arg(&path)
        .output()
        .expect("nortexc executable should run");
    let cleanup = std::fs::remove_file(&path);
    assert!(
        cleanup.is_ok(),
        "temporary source should be removed: {cleanup:?}"
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("MIR calculate"),
        "stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
}
