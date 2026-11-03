//! C-compatible struct layout (Windows x64/ARM64 MSVC rules): natural field
//! alignment, inter-field padding and tail padding to the struct alignment.
//! Not yet wired into MIR/codegen; aggregates are still rejected there.

use std::collections::HashMap;

use super::{PrimitiveType, TargetArchitecture};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldType {
    Primitive(PrimitiveType),
    Struct(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLayout {
    pub name: String,
    pub offset: u64,
    pub size: u64,
    pub alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructLayout {
    pub name: String,
    pub fields: Vec<FieldLayout>,
    pub size: u64,
    pub alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    UnknownStruct(String),
    FieldNotFound { struct_name: String, field: String },
    UnsizedField { struct_name: String, field: String },
    RecursiveStruct(String),
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownStruct(name) => write!(f, "Unbekannter Struct '{name}'."),
            Self::FieldNotFound { struct_name, field } => {
                write!(f, "Struct '{struct_name}' hat kein Feld '{field}'.")
            }
            Self::UnsizedField { struct_name, field } => write!(
                f,
                "Feld '{field}' in '{struct_name}' hat keinen festen Speicherplatz."
            ),
            Self::RecursiveStruct(name) => {
                write!(f, "Struct '{name}' enthält sich selbst by-value.")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

pub struct LayoutCalculator<'a> {
    definitions: &'a HashMap<String, Vec<(String, FieldType)>>,
    target: TargetArchitecture,
}

fn align_up(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

impl<'a> LayoutCalculator<'a> {
    pub fn new(
        definitions: &'a HashMap<String, Vec<(String, FieldType)>>,
        target: TargetArchitecture,
    ) -> Self {
        Self {
            definitions,
            target,
        }
    }

    pub fn layout_of(&self, name: &str) -> Result<StructLayout, LayoutError> {
        self.compute(name, &mut Vec::new())
    }

    pub fn field_offset(&self, struct_name: &str, field: &str) -> Result<u64, LayoutError> {
        self.layout_of(struct_name)?
            .fields
            .iter()
            .find(|candidate| candidate.name == field)
            .map(|candidate| candidate.offset)
            .ok_or_else(|| LayoutError::FieldNotFound {
                struct_name: struct_name.to_owned(),
                field: field.to_owned(),
            })
    }

    fn compute(&self, name: &str, stack: &mut Vec<String>) -> Result<StructLayout, LayoutError> {
        if stack.iter().any(|entry| entry == name) {
            return Err(LayoutError::RecursiveStruct(name.to_owned()));
        }
        let definition = self
            .definitions
            .get(name)
            .ok_or_else(|| LayoutError::UnknownStruct(name.to_owned()))?;
        stack.push(name.to_owned());

        let mut fields = Vec::with_capacity(definition.len());
        let mut offset = 0;
        let mut struct_alignment = 1;
        for (field_name, field_type) in definition {
            let (size, alignment) = match field_type {
                FieldType::Primitive(primitive) => {
                    let layout =
                        primitive
                            .layout(self.target)
                            .ok_or_else(|| LayoutError::UnsizedField {
                                struct_name: name.to_owned(),
                                field: field_name.clone(),
                            })?;
                    (
                        u64::from(layout.size_bytes),
                        u64::from(layout.alignment_bytes),
                    )
                }
                FieldType::Struct(inner) => {
                    let layout = self.compute(inner, stack)?;
                    (layout.size, layout.alignment)
                }
            };
            offset = align_up(offset, alignment);
            struct_alignment = struct_alignment.max(alignment);
            fields.push(FieldLayout {
                name: field_name.clone(),
                offset,
                size,
                alignment,
            });
            offset += size;
        }
        stack.pop();

        Ok(StructLayout {
            name: name.to_owned(),
            fields,
            size: align_up(offset, struct_alignment),
            alignment: struct_alignment,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Definitions = HashMap<String, Vec<(String, FieldType)>>;

    fn field(name: &str, primitive: PrimitiveType) -> (String, FieldType) {
        (name.to_owned(), FieldType::Primitive(primitive))
    }

    fn calculator(definitions: &Definitions) -> LayoutCalculator<'_> {
        LayoutCalculator::new(definitions, TargetArchitecture::WindowsX64)
    }

    #[test]
    fn pads_between_fields_and_at_the_tail() {
        let mut definitions = Definitions::new();
        definitions.insert(
            "Point".to_owned(),
            vec![
                field("a", PrimitiveType::SByte),
                field("b", PrimitiveType::Int32),
                field("c", PrimitiveType::SByte),
            ],
        );
        let layout = calculator(&definitions).layout_of("Point").unwrap();
        let offsets: Vec<_> = layout.fields.iter().map(|f| f.offset).collect();
        assert_eq!(offsets, [0, 4, 8]);
        assert_eq!((layout.size, layout.alignment), (12, 4));
    }

    #[test]
    fn nested_structs_use_inner_alignment() {
        let mut definitions = Definitions::new();
        definitions.insert(
            "Inner".to_owned(),
            vec![
                field("x", PrimitiveType::Int32),
                field("y", PrimitiveType::Int64),
            ],
        );
        definitions.insert(
            "Outer".to_owned(),
            vec![
                field("flag", PrimitiveType::Bool),
                ("inner".to_owned(), FieldType::Struct("Inner".to_owned())),
            ],
        );
        let calc = calculator(&definitions);
        let inner = calc.layout_of("Inner").unwrap();
        assert_eq!((inner.size, inner.alignment), (16, 8));
        let outer = calc.layout_of("Outer").unwrap();
        assert_eq!((outer.size, outer.alignment), (24, 8));
        assert_eq!(calc.field_offset("Outer", "inner"), Ok(8));
    }

    #[test]
    fn reports_missing_fields_unknown_and_recursive_structs() {
        let mut definitions = Definitions::new();
        definitions.insert("S".to_owned(), vec![field("a", PrimitiveType::Int32)]);
        definitions.insert(
            "R".to_owned(),
            vec![("me".to_owned(), FieldType::Struct("R".to_owned()))],
        );
        definitions.insert("V".to_owned(), vec![field("v", PrimitiveType::Void)]);
        let calc = calculator(&definitions);
        assert!(matches!(
            calc.field_offset("S", "zzz"),
            Err(LayoutError::FieldNotFound { .. })
        ));
        assert_eq!(
            calc.layout_of("Nope"),
            Err(LayoutError::UnknownStruct("Nope".to_owned()))
        );
        assert_eq!(
            calc.layout_of("R"),
            Err(LayoutError::RecursiveStruct("R".to_owned()))
        );
        assert!(matches!(
            calc.layout_of("V"),
            Err(LayoutError::UnsizedField { .. })
        ));
    }

    #[test]
    fn empty_struct_has_size_zero_and_alignment_one() {
        let mut definitions = Definitions::new();
        definitions.insert("E".to_owned(), Vec::new());
        let layout = calculator(&definitions).layout_of("E").unwrap();
        assert_eq!((layout.size, layout.alignment), (0, 1));
    }
}
