use nortex_compiler::driver::check_source;
use nortex_compiler::type_system::{PrimitiveType, TargetArchitecture};

#[test]
fn primitive_names_are_case_sensitive() {
    assert_eq!(PrimitiveType::parse("int"), Some(PrimitiveType::Int32));
    assert_eq!(PrimitiveType::parse("Int32"), None);
    assert_eq!(PrimitiveType::parse("INT"), None);
}

#[test]
fn all_required_integer_types_are_registered() {
    let names = [
        "sbyte", "short", "int", "long", "i128", "byte", "ushort", "uint", "ulong", "u128", "nint",
        "nuint", "isize", "usize",
    ];
    for name in names {
        assert!(
            PrimitiveType::parse(name).is_some(),
            "missing primitive: {name}"
        );
    }
}

#[test]
fn target_dependent_integer_types_are_64_bit_on_windows_targets() {
    for target in [
        TargetArchitecture::WindowsX64,
        TargetArchitecture::WindowsArm64,
    ] {
        assert_eq!(PrimitiveType::NativeInt.bit_width(target), Some(64));
        assert_eq!(PrimitiveType::NativeUInt.bit_width(target), Some(64));
    }
}

#[test]
fn no_implicit_numeric_conversion_is_allowed_between_distinct_primitives() {
    assert!(!PrimitiveType::Int32.can_implicitly_convert_to(PrimitiveType::Int64));
    assert!(!PrimitiveType::Float32.can_implicitly_convert_to(PrimitiveType::Float64));
    assert!(PrimitiveType::Int32.can_implicitly_convert_to(PrimitiveType::Int32));
}

#[test]
fn bool_has_logical_width_one_but_byte_storage() {
    let target = TargetArchitecture::WindowsX64;
    assert_eq!(PrimitiveType::Bool.bit_width(target), Some(1));
    assert_eq!(PrimitiveType::Bool.layout(target).unwrap().size_bytes, 1);
}

#[test]
fn char_is_a_32_bit_unicode_scalar_value() {
    let target = TargetArchitecture::WindowsX64;
    assert_eq!(PrimitiveType::Char.bit_width(target), Some(32));
    assert_eq!(PrimitiveType::Char.layout(target).unwrap().size_bytes, 4);
}

#[test]
fn unqualified_typed_bindings_are_immutable_by_default() {
    let result = check_source(
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
        result
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "NX1200"),
        "expected immutable assignment diagnostic, got: {:#?}",
        result.diagnostics
    );
}

#[test]
fn explicitly_mut_bindings_allow_assignment() {
    let result = check_source(
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
        !result.has_errors(),
        "diagnostics: {:#?}",
        result.diagnostics
    );
}

#[test]
fn const_typed_bindings_are_immutable() {
    let result = check_source(
        r#"
        public class MutabilityTest
        {
            public void run()
            {
                const int SCORE = 10;
                SCORE = 20;
            }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("unveränderliche Wert"))
    );
}

#[test]
fn null_literal_is_rejected_in_safe_nortex_code() {
    let result = check_source(
        r#"
        public class NullTest
        {
            public void run()
            {
                object value = null;
            }
        }
    "#,
    );
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.message.contains("null-Literal"))
    );
}
