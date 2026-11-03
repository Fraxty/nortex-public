//! Canonical primitive types and target-dependent layout facts for NORTEX.
//!
//! This module describes the language's primitive type vocabulary. It does not
//! yet define the final native ABI or emit target-specific machine code.

pub mod struct_layout;

/// Canonical primitive spellings. Compatibility aliases are intentionally not accepted.
pub const BUILTIN_TYPE_NAMES: &[&str] = &[
    "sbyte", "short", "int", "long", "i128", "byte", "ushort", "uint", "ulong", "u128", "nint",
    "nuint", "isize", "usize", "float", "double", "decimal", "bool", "char", "string", "void", "!",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetArchitecture {
    WindowsX64,
    WindowsArm64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    SByte,
    Int16,
    Int32,
    Int64,
    Int128,
    Byte,
    UInt16,
    UInt32,
    UInt64,
    UInt128,
    NativeInt,
    NativeUInt,
    ISized,
    USized,
    Float32,
    Float64,
    Decimal128,
    Bool,
    Char,
    String,
    Void,
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeLayout {
    pub size_bytes: u16,
    pub alignment_bytes: u16,
}

impl PrimitiveType {
    /// Parse a NORTEX primitive spelling. Names are case-sensitive.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "sbyte" => Self::SByte,
            "short" => Self::Int16,
            "int" => Self::Int32,
            "long" => Self::Int64,
            "i128" => Self::Int128,
            "byte" => Self::Byte,
            "ushort" => Self::UInt16,
            "uint" => Self::UInt32,
            "ulong" => Self::UInt64,
            "u128" => Self::UInt128,
            "nint" => Self::NativeInt,
            "nuint" => Self::NativeUInt,
            "isize" => Self::ISized,
            "usize" => Self::USized,
            "float" => Self::Float32,
            "double" => Self::Float64,
            "decimal" => Self::Decimal128,
            "bool" => Self::Bool,
            "char" => Self::Char,
            "string" => Self::String,
            "void" => Self::Void,
            "!" => Self::Never,
            _ => return None,
        })
    }

    pub fn canonical_name(self) -> &'static str {
        match self {
            Self::SByte => "sbyte",
            Self::Int16 => "short",
            Self::Int32 => "int",
            Self::Int64 => "long",
            Self::Int128 => "i128",
            Self::Byte => "byte",
            Self::UInt16 => "ushort",
            Self::UInt32 => "uint",
            Self::UInt64 => "ulong",
            Self::UInt128 => "u128",
            Self::NativeInt => "nint",
            Self::NativeUInt => "nuint",
            Self::ISized => "isize",
            Self::USized => "usize",
            Self::Float32 => "float",
            Self::Float64 => "double",
            Self::Decimal128 => "decimal",
            Self::Bool => "bool",
            Self::Char => "char",
            Self::String => "string",
            Self::Void => "void",
            Self::Never => "!",
        }
    }

    /// Current language layout proposal for the two supported Windows targets.
    /// String is a runtime value with a pointer/length/capacity representation;
    /// this internal representation is not yet a stable public ABI guarantee.
    pub fn layout(self, target: TargetArchitecture) -> Option<TypeLayout> {
        let pointer_size = match target {
            TargetArchitecture::WindowsX64 | TargetArchitecture::WindowsArm64 => 8,
        };
        let layout = match self {
            Self::SByte | Self::Byte | Self::Bool => TypeLayout {
                size_bytes: 1,
                alignment_bytes: 1,
            },
            Self::Int16 | Self::UInt16 => TypeLayout {
                size_bytes: 2,
                alignment_bytes: 2,
            },
            Self::Int32 | Self::UInt32 | Self::Float32 | Self::Char => TypeLayout {
                size_bytes: 4,
                alignment_bytes: 4,
            },
            Self::Int64
            | Self::UInt64
            | Self::NativeInt
            | Self::NativeUInt
            | Self::ISized
            | Self::USized
            | Self::Float64 => TypeLayout {
                size_bytes: 8,
                alignment_bytes: 8,
            },
            Self::Int128 | Self::UInt128 | Self::Decimal128 => TypeLayout {
                size_bytes: 16,
                alignment_bytes: 16,
            },
            Self::String => TypeLayout {
                size_bytes: pointer_size * 3,
                alignment_bytes: pointer_size,
            },
            Self::Void | Self::Never => return None,
        };
        Some(layout)
    }

    pub fn is_integer(self) -> bool {
        matches!(
            self,
            Self::SByte
                | Self::Int16
                | Self::Int32
                | Self::Int64
                | Self::Int128
                | Self::Byte
                | Self::UInt16
                | Self::UInt32
                | Self::UInt64
                | Self::UInt128
                | Self::NativeInt
                | Self::NativeUInt
                | Self::ISized
                | Self::USized
        )
    }

    pub fn is_float(self) -> bool {
        matches!(self, Self::Float32 | Self::Float64)
    }

    pub fn is_signed_integer(self) -> bool {
        matches!(
            self,
            Self::SByte
                | Self::Int16
                | Self::Int32
                | Self::Int64
                | Self::Int128
                | Self::NativeInt
                | Self::ISized
        )
    }

    pub fn is_unsigned_integer(self) -> bool {
        matches!(
            self,
            Self::Byte
                | Self::UInt16
                | Self::UInt32
                | Self::UInt64
                | Self::UInt128
                | Self::NativeUInt
                | Self::USized
        )
    }

    pub fn is_numeric(self) -> bool {
        self.is_integer() || self.is_float() || self == Self::Decimal128
    }

    /// Width of an integer or floating-point representation, if applicable.
    pub fn bit_width(self, target: TargetArchitecture) -> Option<u16> {
        match self {
            Self::SByte | Self::Byte => Some(8),
            Self::Int16 | Self::UInt16 => Some(16),
            Self::Int32 | Self::UInt32 | Self::Float32 => Some(32),
            Self::Int64 | Self::UInt64 | Self::Float64 => Some(64),
            Self::Int128 | Self::UInt128 | Self::Decimal128 => Some(128),
            Self::NativeInt | Self::NativeUInt | Self::ISized | Self::USized => match target {
                TargetArchitecture::WindowsX64 | TargetArchitecture::WindowsArm64 => Some(64),
            },
            Self::Bool => Some(1),
            Self::Char => Some(32),
            Self::String | Self::Void | Self::Never => None,
        }
    }

    /// NORTEX 0.2 allows implicit conversion only when source and target are
    /// identical. Literal-fit conversions will be a separate contextual rule.
    pub fn can_implicitly_convert_to(self, target: Self) -> bool {
        self == target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_canonical_primitive_spellings_are_accepted() {
        assert_eq!(PrimitiveType::parse("int"), Some(PrimitiveType::Int32));
        assert_eq!(PrimitiveType::parse("Int32"), None);
        assert_eq!(PrimitiveType::parse("Unit"), None);
        assert_eq!(PrimitiveType::parse("Never"), None);
        assert_eq!(PrimitiveType::parse("name"), None);
    }

    #[test]
    fn pointer_sized_integers_are_64_bit_on_both_windows_targets() {
        for target in [
            TargetArchitecture::WindowsX64,
            TargetArchitecture::WindowsArm64,
        ] {
            assert_eq!(
                PrimitiveType::NativeInt.layout(target).unwrap().size_bytes,
                8
            );
            assert_eq!(
                PrimitiveType::NativeUInt.layout(target).unwrap().size_bytes,
                8
            );
        }
    }

    #[test]
    fn bool_has_byte_storage_but_logical_boolean_semantics() {
        let layout = PrimitiveType::Bool
            .layout(TargetArchitecture::WindowsX64)
            .unwrap();
        assert_eq!(layout.size_bytes, 1);
    }

    #[test]
    fn numeric_classification_and_implicit_conversion_are_explicit() {
        assert!(PrimitiveType::Int32.is_numeric());
        assert!(PrimitiveType::Decimal128.is_numeric());
        assert!(!PrimitiveType::Bool.is_numeric());
        assert!(PrimitiveType::Int32.can_implicitly_convert_to(PrimitiveType::Int32));
        assert!(!PrimitiveType::Int32.can_implicitly_convert_to(PrimitiveType::Int64));
        assert_eq!(
            PrimitiveType::Char.bit_width(TargetArchitecture::WindowsX64),
            Some(32)
        );
    }
}
