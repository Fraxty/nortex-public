use nortex_compiler::driver::compile_source_to_mir;

const CASES: &[(&str, &str, &str)] = &[
    (
        "use_after_move",
        include_str!("negatives/use_after_move.nx"),
        "NX1202",
    ),
    (
        "write_while_shared_borrowed",
        include_str!("negatives/write_while_shared_borrowed.nx"),
        "NX2003",
    ),
    (
        "shared_mutable_alias",
        include_str!("negatives/shared_mutable_alias.nx"),
        "NX2003",
    ),
    (
        "mutable_borrow_of_immutable",
        include_str!("negatives/mutable_borrow_of_immutable.nx"),
        "NX1201",
    ),
    (
        "borrow_outlives_scope",
        include_str!("negatives/borrow_outlives_scope.nx"),
        "NX2003",
    ),
    (
        "reference_return_without_lifetime",
        include_str!("negatives/reference_return_without_lifetime.nx"),
        "NX2003",
    ),
    (
        "mutable_reborrow_from_shared",
        include_str!("negatives/mutable_reborrow_from_shared.nx"),
        "NX2003",
    ),
    (
        "write_through_shared_reference",
        include_str!("negatives/write_through_shared_reference.nx"),
        "NX1200",
    ),
];

#[test]
fn ownership_and_region_negative_fixtures_are_rejected() {
    for (name, source, expected_code) in CASES {
        let result = compile_source_to_mir(source);
        assert!(result.has_errors(), "{name} unexpectedly compiled");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == *expected_code),
            "{name} must report {expected_code}: {:#?}",
            result.diagnostics
        );
    }
}
