use super::*;

pub(super) fn function_type(
    function: &FunctionDeclaration,
    declaring_type: Option<&str>,
) -> FunctionType {
    FunctionType {
        parameters: function
            .parameters
            .iter()
            .map(|parameter| type_name(&parameter.type_ref))
            .collect(),
        result: type_name(&function.return_type),
        generic_parameters: function.generic_parameters.clone(),
        declaring_type: declaring_type.map(str::to_string),
        is_static: function
            .modifiers
            .iter()
            .any(|modifier| modifier == "static"),
        is_private: function
            .modifiers
            .iter()
            .any(|modifier| modifier == "private"),
    }
}

pub(super) fn type_name(ty: &TypeReference) -> String {
    let mut name = ty.name.clone();
    if !ty.generic_arguments.is_empty() {
        name.push('<');
        name.push_str(
            &ty.generic_arguments
                .iter()
                .map(type_name)
                .collect::<Vec<_>>()
                .join(", "),
        );
        name.push('>');
    }
    if let Some(length) = ty.fixed_array_length {
        name = format!("[{name}; {length}]");
    } else if ty.array_depth > 0 {
        name.push_str(&"[]".repeat(ty.array_depth));
    }
    match ty.reference_kind {
        ReferenceKind::Value => name,
        ReferenceKind::SharedBorrow => format!("&{name}"),
        ReferenceKind::MutableBorrow => format!("&mut {name}"),
        ReferenceKind::RawConstPointer => format!("*const {name}"),
        ReferenceKind::RawMutablePointer => format!("*mut {name}"),
    }
}

pub(super) fn integer_literal_type(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if lower.ends_with("ul") || lower.ends_with("lu") {
        "ulong".to_string()
    } else if lower.ends_with('u') {
        "uint".to_string()
    } else if lower.ends_with('l') {
        "long".to_string()
    } else {
        "int".to_string()
    }
}

pub(super) fn is_numeric_type(ty: &str) -> bool {
    matches!(
        ty,
        "sbyte"
            | "short"
            | "int"
            | "long"
            | "i128"
            | "byte"
            | "ushort"
            | "uint"
            | "ulong"
            | "u128"
            | "nint"
            | "nuint"
            | "isize"
            | "usize"
            | "float"
            | "double"
            | "decimal"
    )
}

pub(super) fn is_builtin_nominal_type(name: &str) -> bool {
    matches!(
        name,
        "List"
            | "Vec"
            | "Dictionary"
            | "HashSet"
            | "Queue"
            | "Stack"
            | "Option"
            | "Result"
            | "Task"
            | "Future"
            | "Box"
            | "Rc"
            | "Arc"
            | "Cell"
            | "RefCell"
            | "Mutex"
            | "RwLock"
            | "Span"
            | "Memory"
            | "Iterator"
    )
}

pub(super) fn nominal_type_name(ty: &str) -> &str {
    ty.split('<')
        .next()
        .unwrap_or(ty)
        .trim_start_matches('&')
        .trim_start_matches("mut ")
}

pub(super) fn task_result_type(ty: &str) -> Option<&str> {
    ty.strip_prefix("Task<")?.strip_suffix('>')
}

pub(super) fn iterator_element_type(ty: &str) -> Option<&str> {
    if let Some(element) = ty
        .strip_prefix("Iterator<")
        .and_then(|value| value.strip_suffix('>'))
    {
        return Some(element);
    }
    for collection in ["List<", "Vec<", "HashSet<", "Queue<", "Stack<"] {
        if let Some(element) = ty
            .strip_prefix(collection)
            .and_then(|value| value.strip_suffix('>'))
        {
            return Some(element);
        }
    }
    if let Some(element) = ty.strip_suffix("[]") {
        return Some(element);
    }
    if ty.starts_with("[") && ty.ends_with(']') {
        return ty
            .split(';')
            .next()
            .map(str::trim_start)
            .map(|value| value.trim_start_matches('['));
    }
    None
}

/// Integer literal type marker used for call arguments before overload
/// resolution; it is displayed and defaulted as `int`.
pub(super) fn literal_default(ty: &str) -> String {
    if ty == "{integer}" {
        "int".to_string()
    } else {
        ty.to_string()
    }
}

pub(super) fn is_string_concatenable(ty: &str) -> bool {
    ty == "string" || ty == "bool" || ty == "char" || is_numeric_type(ty)
}

pub(super) fn requires_move(ty: &str) -> bool {
    if ty.starts_with('&') || ty.starts_with('*') {
        return false;
    }
    !matches!(
        ty,
        "void"
            | "!"
            | "bool"
            | "char"
            | "byte"
            | "sbyte"
            | "short"
            | "ushort"
            | "int"
            | "uint"
            | "long"
            | "ulong"
            | "i128"
            | "u128"
            | "nint"
            | "nuint"
            | "isize"
            | "usize"
            | "float"
            | "double"
            | "decimal"
            | "IntList"
            | "StringList"
    )
}

pub(super) fn move_source_identifier(expression: &Expression) -> Option<&str> {
    match expression {
        Expression::Identifier(name, _) => Some(name),
        Expression::Group(inner, _) => move_source_identifier(inner),
        _ => None,
    }
}

pub(super) fn borrow_argument(
    expression: &Expression,
    parameter_type: &str,
) -> Option<(BorrowPlace, bool)> {
    let (expression, explicit_mutability) = match expression {
        Expression::Group(inner, _) => return borrow_argument(inner, parameter_type),
        Expression::Unary {
            operator, operand, ..
        } if operator == "&" || operator == "&mut" => (operand.as_ref(), Some(operator == "&mut")),
        _ => (expression, None),
    };
    let mutable = explicit_mutability.unwrap_or_else(|| parameter_type.starts_with("&mut "));
    Some((
        borrow_place(expression).unwrap_or(BorrowPlace {
            root: None,
            projection: Vec::new(),
        }),
        mutable,
    ))
}

pub(super) fn borrow_place(expression: &Expression) -> Option<BorrowPlace> {
    match expression {
        Expression::Identifier(name, _) => Some(BorrowPlace {
            root: Some(name.clone()),
            projection: Vec::new(),
        }),
        Expression::Group(inner, _) => borrow_place(inner),
        Expression::Member { object, name, .. } => {
            let mut place = borrow_place(object)?;
            place.projection.push(BorrowProjection::Field(name.clone()));
            Some(place)
        }
        Expression::Index { object, index, .. } => {
            let mut place = borrow_place(object)?;
            let constant_index = match index.as_ref() {
                Expression::Integer(value, _) => {
                    parse_integer_literal(value).map(|index| index.to_string())
                }
                _ => None,
            };
            place
                .projection
                .push(BorrowProjection::Index(constant_index));
            Some(place)
        }
        Expression::Unary {
            operator, operand, ..
        } if operator == "*" => {
            let mut place = borrow_place(operand)?;
            place.root = None;
            place.projection.push(BorrowProjection::Dereference);
            Some(place)
        }
        _ => None,
    }
}

pub(super) fn borrow_places_overlap(left: &BorrowPlace, right: &BorrowPlace) -> bool {
    if let (Some(left_root), Some(right_root)) = (&left.root, &right.root)
        && left_root != right_root
    {
        return false;
    }

    for (left_projection, right_projection) in left.projection.iter().zip(&right.projection) {
        match (left_projection, right_projection) {
            (BorrowProjection::Field(left), BorrowProjection::Field(right)) if left != right => {
                return false;
            }
            (BorrowProjection::Index(Some(left)), BorrowProjection::Index(Some(right)))
                if left != right =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

pub(super) fn overload_score(
    candidate: &FunctionType,
    arguments: &[Option<String>],
) -> Option<u32> {
    candidate
        .parameters
        .iter()
        .zip(arguments)
        .try_fold(0u32, |score, (expected, actual)| {
            let cost = match actual.as_deref() {
                None => 8,
                Some(actual) if expected == actual => 0,
                Some("{integer}") if integer_range(expected).is_some() => 1,
                Some("{integer}")
                    if matches!(expected.as_str(), "float" | "double" | "decimal") =>
                {
                    2
                }
                Some(actual) if actual == expected => 0,
                Some(_) => 1,
            };
            score.checked_add(cost)
        })
}

/// Returns the literal text and sign if `expression` is an unsuffixed
/// integer literal, optionally negated (`-5`).
pub(super) fn unsuffixed_integer_literal(expression: &Expression) -> Option<(&str, bool)> {
    match expression {
        Expression::Integer(value, _) => {
            let lower = value.to_ascii_lowercase();
            let digits = lower
                .strip_prefix("0x")
                .or_else(|| lower.strip_prefix("0b"))
                .unwrap_or(&lower);
            let radix_prefixed = lower.starts_with("0x") || lower.starts_with("0b");
            let has_suffix = if radix_prefixed {
                digits.ends_with('u') || digits.ends_with('l')
            } else {
                digits.chars().any(|c| c.is_ascii_alphabetic())
            };
            if has_suffix {
                None
            } else {
                Some((value.as_str(), false))
            }
        }
        Expression::Unary {
            operator, operand, ..
        } if operator == "-" => match operand.as_ref() {
            Expression::Integer(..) => {
                unsuffixed_integer_literal(operand).map(|(value, _)| (value, true))
            }
            _ => None,
        },
        Expression::Group(inner, _) => unsuffixed_integer_literal(inner),
        _ => None,
    }
}

pub(super) fn parse_integer_literal(value: &str) -> Option<u128> {
    let cleaned = value.replace('_', "").to_ascii_lowercase();
    if let Some(hex) = cleaned.strip_prefix("0x") {
        return u128::from_str_radix(hex, 16).ok();
    }
    if let Some(binary) = cleaned.strip_prefix("0b") {
        return u128::from_str_radix(binary, 2).ok();
    }
    cleaned.parse::<u128>().ok()
}

/// Value range of an integer type as (min, max) magnitude bounds.
/// `nint`/`nuint`/`isize`/`usize` use the 64-bit Windows x64 target.
pub(super) fn integer_range(ty: &str) -> Option<(i128, u128)> {
    Some(match ty {
        "sbyte" => (i8::MIN as i128, i8::MAX as u128),
        "byte" => (0, u8::MAX as u128),
        "short" => (i16::MIN as i128, i16::MAX as u128),
        "ushort" => (0, u16::MAX as u128),
        "int" => (i32::MIN as i128, i32::MAX as u128),
        "uint" => (0, u32::MAX as u128),
        "long" | "nint" | "isize" => (i64::MIN as i128, i64::MAX as u128),
        "ulong" | "nuint" | "usize" => (0, u64::MAX as u128),
        "i128" => (i128::MIN, i128::MAX as u128),
        "u128" => (0, u128::MAX),
        _ => return None,
    })
}

/// Splits `Name<A, B<C>>` into `Name` and its top-level arguments.
pub(super) fn split_generic(ty: &str) -> (&str, Vec<&str>) {
    let Some(open) = ty.find('<') else {
        return (ty, Vec::new());
    };
    if !ty.ends_with('>') {
        return (ty, Vec::new());
    }
    let inner = &ty[open + 1..ty.len() - 1];
    let mut arguments = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, character) in inner.char_indices() {
        match character {
            '<' | '[' => depth += 1,
            '>' | ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                arguments.push(inner[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    arguments.push(inner[start..].trim());
    (&ty[..open], arguments)
}

/// Replaces whole identifiers in a type string using `map`.
pub(super) fn substitute(ty: &str, map: &HashMap<String, String>) -> String {
    if map.is_empty() {
        return ty.to_string();
    }
    let mut output = String::with_capacity(ty.len());
    let mut identifier = String::new();
    let flush = |identifier: &mut String, output: &mut String| {
        if !identifier.is_empty() {
            output.push_str(
                map.get(identifier.as_str())
                    .map_or(identifier.as_str(), String::as_str),
            );
            identifier.clear();
        }
    };
    for character in ty.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            identifier.push(character);
        } else {
            flush(&mut identifier, &mut output);
            output.push(character);
        }
    }
    flush(&mut identifier, &mut output);
    output
}

pub(super) fn substitute_function(
    function: &FunctionType,
    map: &HashMap<String, String>,
) -> FunctionType {
    let mut substituted = function.clone();
    substituted.parameters = function
        .parameters
        .iter()
        .map(|parameter| substitute(parameter, map))
        .collect();
    substituted.result = substitute(&function.result, map);
    substituted
}

pub(super) fn accessible_methods(
    methods: &[FunctionType],
    accessing_type: Option<&str>,
) -> Vec<FunctionType> {
    methods
        .iter()
        .filter(|method| {
            !method.is_private
                || method
                    .declaring_type
                    .as_deref()
                    .is_some_and(|declaring_type| Some(declaring_type) == accessing_type)
        })
        .cloned()
        .collect()
}
