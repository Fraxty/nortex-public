//! Eingebaute Text-, Konvertierungs- und Dateifunktionen.
//!
//! Die Tabelle ist die einzige Quelle für HIR (Auflösung), MIR (Typprüfung) und
//! Codegen (Runtime-Symbol und ABI). Strings werden als (Zeiger, Länge) übergeben.

pub struct Builtin {
    pub name: &'static str,
    pub symbol: &'static str,
    pub params: &'static [&'static str],
    pub returns: &'static str,
}

const fn builtin(
    name: &'static str,
    symbol: &'static str,
    params: &'static [&'static str],
    returns: &'static str,
) -> Builtin {
    Builtin {
        name,
        symbol,
        params,
        returns,
    }
}

pub const BUILTINS: &[Builtin] = &[
    builtin("Text.length", "nortex_text_length", &["string"], "int"),
    builtin(
        "Text.substring",
        "nortex_text_substring",
        &["string", "int", "int"],
        "string",
    ),
    builtin(
        "Text.indexOf",
        "nortex_text_index_of",
        &["string", "string"],
        "int",
    ),
    builtin(
        "Text.contains",
        "nortex_text_contains",
        &["string", "string"],
        "bool",
    ),
    builtin(
        "Text.startsWith",
        "nortex_text_starts_with",
        &["string", "string"],
        "bool",
    ),
    builtin(
        "Text.endsWith",
        "nortex_text_ends_with",
        &["string", "string"],
        "bool",
    ),
    builtin(
        "Text.charAt",
        "nortex_text_char_at",
        &["string", "int"],
        "int",
    ),
    builtin("Text.trim", "nortex_text_trim", &["string"], "string"),
    builtin(
        "Text.toUpper",
        "nortex_text_to_upper",
        &["string"],
        "string",
    ),
    builtin(
        "Text.toLower",
        "nortex_text_to_lower",
        &["string"],
        "string",
    ),
    builtin(
        "Text.replace",
        "nortex_text_replace",
        &["string", "string", "string"],
        "string",
    ),
    builtin(
        "Convert.toString",
        "nortex_convert_int_to_string",
        &["int"],
        "string",
    ),
    builtin(
        "Convert.boolToString",
        "nortex_convert_bool_to_string",
        &["bool"],
        "string",
    ),
    builtin("Convert.toInt", "nortex_convert_to_int", &["string"], "int"),
    builtin(
        "Convert.isInt",
        "nortex_convert_is_int",
        &["string"],
        "bool",
    ),
    builtin(
        "File.readAllText",
        "nortex_file_read_all_text",
        &["string"],
        "string",
    ),
    builtin(
        "File.writeAllText",
        "nortex_file_write_all_text",
        &["string", "string"],
        "bool",
    ),
    builtin(
        "File.appendAllText",
        "nortex_file_append_all_text",
        &["string", "string"],
        "bool",
    ),
    builtin("File.exists", "nortex_file_exists", &["string"], "bool"),
    builtin("File.delete", "nortex_file_delete", &["string"], "bool"),
    builtin(
        "Directory.exists",
        "nortex_directory_exists",
        &["string"],
        "bool",
    ),
    builtin(
        "Directory.create",
        "nortex_directory_create",
        &["string"],
        "bool",
    ),
    builtin(
        "Console.readLine",
        "nortex_console_read_line",
        &[],
        "string",
    ),
    builtin(
        "Convert.toDouble",
        "nortex_convert_to_double",
        &["int"],
        "double",
    ),
    builtin(
        "Convert.doubleToInt",
        "nortex_convert_double_to_int",
        &["double"],
        "int",
    ),
    builtin(
        "Convert.doubleToString",
        "nortex_convert_double_to_string",
        &["double"],
        "string",
    ),
    builtin(
        "Convert.parseDouble",
        "nortex_convert_parse_double",
        &["string"],
        "double",
    ),
    builtin(
        "Convert.isDouble",
        "nortex_convert_is_double",
        &["string"],
        "bool",
    ),
    builtin("Math.sqrt", "nortex_math_sqrt", &["double"], "double"),
    builtin("Math.abs", "nortex_math_abs", &["double"], "double"),
    builtin("Math.floor", "nortex_math_floor", &["double"], "double"),
    builtin(
        "Math.pow",
        "nortex_math_pow",
        &["double", "double"],
        "double",
    ),
    builtin("IntList.create", "nortex_int_list_create", &[], "IntList"),
    builtin(
        "IntList.add",
        "nortex_int_list_add",
        &["IntList", "int"],
        "bool",
    ),
    builtin(
        "IntList.get",
        "nortex_int_list_get",
        &["IntList", "int"],
        "int",
    ),
    builtin(
        "IntList.set",
        "nortex_int_list_set",
        &["IntList", "int", "int"],
        "bool",
    ),
    builtin(
        "IntList.length",
        "nortex_int_list_length",
        &["IntList"],
        "int",
    ),
    builtin("IntList.free", "nortex_int_list_free", &["IntList"], "bool"),
    builtin(
        "StringList.create",
        "nortex_string_list_create",
        &[],
        "StringList",
    ),
    builtin(
        "StringList.add",
        "nortex_string_list_add",
        &["StringList", "string"],
        "bool",
    ),
    builtin(
        "StringList.get",
        "nortex_string_list_get",
        &["StringList", "int"],
        "string",
    ),
    builtin(
        "StringList.set",
        "nortex_string_list_set",
        &["StringList", "int", "string"],
        "bool",
    ),
    builtin(
        "StringList.length",
        "nortex_string_list_length",
        &["StringList"],
        "int",
    ),
    builtin(
        "StringList.free",
        "nortex_string_list_free",
        &["StringList"],
        "bool",
    ),
    builtin(
        "Text.split",
        "nortex_text_split",
        &["string", "string"],
        "StringList",
    ),
    builtin(
        "Text.join",
        "nortex_text_join",
        &["StringList", "string"],
        "string",
    ),
];

pub fn find(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|entry| entry.name == name)
}

/// Namespaces, deren Member-Aufrufe über die Tabelle aufgelöst werden.
pub fn is_namespace(name: &str) -> bool {
    matches!(
        name,
        "Text" | "Convert" | "File" | "Directory" | "Math" | "IntList" | "StringList"
    )
}

/// Opake Listentypen: im Native-Code ein i64-Handle.
pub fn is_list_type(name: &str) -> bool {
    matches!(name, "IntList" | "StringList")
}
