//! Text-, Konvertierungs- und Dateifunktionen für erzeugten NORTEX-Code.
//!
//! Strings kommen als (Zeiger, Länge) an. Längen und Indizes zählen UTF-8-Bytes.
//! Gelieferte Strings sind Heap-Strings wie bei `nortex_string_concat`; Null bedeutet
//! Speichermangel und löst im erzeugten Code einen Trap aus.

use crate::string::{StringSlice, nortex_string_concat};
use std::io::{BufRead, Write};

/// # Safety
/// `data` muss für `len` Bytes lesbar sein, sofern `len` nicht null ist.
unsafe fn view<'a>(data: *const u8, len: usize) -> &'a [u8] {
    if len == 0 || data.is_null() {
        return &[];
    }
    // SAFETY: Vertrag der Funktion.
    unsafe { std::slice::from_raw_parts(data, len) }
}

fn new_string(bytes: &[u8]) -> *mut StringSlice {
    // SAFETY: `bytes` ist ein gültiger Slice; Länge null braucht keinen Zeiger.
    unsafe { nortex_string_concat(bytes.as_ptr(), bytes.len(), std::ptr::null(), 0) }
}

fn clamp_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[derive(Clone, Copy)]
struct Text {
    data: *const u8,
    len: usize,
}

impl Text {
    /// # Safety
    /// Siehe `view`.
    unsafe fn bytes<'a>(self) -> &'a [u8] {
        // SAFETY: Vertrag der Funktion.
        unsafe { view(self.data, self.len) }
    }
}

macro_rules! text_args {
    ($data:expr, $len:expr) => {
        Text {
            data: $data,
            len: $len,
        }
    };
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_length(data: *const u8, len: usize) -> i32 {
    let _ = data;
    clamp_to_i32(len)
}

/// Byte-Teilstring ab `start` mit höchstens `count` Bytes; Werte außerhalb werden begrenzt.
///
/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_substring(
    data: *const u8,
    len: usize,
    start: i32,
    count: i32,
) -> *mut StringSlice {
    // SAFETY: Vertrag der Funktion.
    let bytes = unsafe { text_args!(data, len).bytes() };
    let start = usize::try_from(start).unwrap_or(0).min(bytes.len());
    let count = usize::try_from(count).unwrap_or(0).min(bytes.len() - start);
    new_string(&bytes[start..start + count])
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_index_of(
    data: *const u8,
    len: usize,
    needle: *const u8,
    needle_len: usize,
) -> i32 {
    // SAFETY: Vertrag der Funktion.
    let (haystack, needle) = unsafe { (view(data, len), view(needle, needle_len)) };
    find(haystack, needle).map_or(-1, clamp_to_i32)
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_contains(
    data: *const u8,
    len: usize,
    needle: *const u8,
    needle_len: usize,
) -> bool {
    // SAFETY: Vertrag der Funktion.
    unsafe { nortex_text_index_of(data, len, needle, needle_len) >= 0 }
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_starts_with(
    data: *const u8,
    len: usize,
    prefix: *const u8,
    prefix_len: usize,
) -> bool {
    // SAFETY: Vertrag der Funktion.
    let (text, prefix) = unsafe { (view(data, len), view(prefix, prefix_len)) };
    text.starts_with(prefix)
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_ends_with(
    data: *const u8,
    len: usize,
    suffix: *const u8,
    suffix_len: usize,
) -> bool {
    // SAFETY: Vertrag der Funktion.
    let (text, suffix) = unsafe { (view(data, len), view(suffix, suffix_len)) };
    text.ends_with(suffix)
}

/// Byte an `index` oder -1 außerhalb des Strings.
///
/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_char_at(data: *const u8, len: usize, index: i32) -> i32 {
    // SAFETY: Vertrag der Funktion.
    let bytes = unsafe { view(data, len) };
    usize::try_from(index)
        .ok()
        .and_then(|index| bytes.get(index))
        .map_or(-1, |byte| i32::from(*byte))
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_trim(data: *const u8, len: usize) -> *mut StringSlice {
    // SAFETY: Vertrag der Funktion.
    let bytes = unsafe { view(data, len) };
    match std::str::from_utf8(bytes) {
        Ok(text) => new_string(text.trim().as_bytes()),
        Err(_) => new_string(bytes.trim_ascii()),
    }
}

fn convert_case(bytes: &[u8], upper: bool) -> Vec<u8> {
    match std::str::from_utf8(bytes) {
        Ok(text) if upper => text.to_uppercase().into_bytes(),
        Ok(text) => text.to_lowercase().into_bytes(),
        Err(_) if upper => bytes.to_ascii_uppercase(),
        Err(_) => bytes.to_ascii_lowercase(),
    }
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_to_upper(data: *const u8, len: usize) -> *mut StringSlice {
    // SAFETY: Vertrag der Funktion.
    new_string(&convert_case(unsafe { view(data, len) }, true))
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_to_lower(data: *const u8, len: usize) -> *mut StringSlice {
    // SAFETY: Vertrag der Funktion.
    new_string(&convert_case(unsafe { view(data, len) }, false))
}

/// Ersetzt alle Vorkommen von `from` durch `to`; ein leeres `from` ändert nichts.
///
/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_replace(
    data: *const u8,
    len: usize,
    from: *const u8,
    from_len: usize,
    to: *const u8,
    to_len: usize,
) -> *mut StringSlice {
    // SAFETY: Vertrag der Funktion.
    let (text, from, to) = unsafe { (view(data, len), view(from, from_len), view(to, to_len)) };
    if from.is_empty() {
        return new_string(text);
    }
    let mut out = Vec::with_capacity(text.len());
    let mut rest = text;
    while let Some(position) = find(rest, from) {
        out.extend_from_slice(&rest[..position]);
        out.extend_from_slice(to);
        rest = &rest[position + from.len()..];
    }
    out.extend_from_slice(rest);
    new_string(&out)
}

#[unsafe(no_mangle)]
pub extern "C" fn nortex_convert_int_to_string(value: i32) -> *mut StringSlice {
    new_string(value.to_string().as_bytes())
}

#[unsafe(no_mangle)]
pub extern "C" fn nortex_convert_bool_to_string(value: bool) -> *mut StringSlice {
    new_string(if value { b"true" } else { b"false" })
}

fn parse_int(bytes: &[u8]) -> Option<i32> {
    std::str::from_utf8(bytes).ok()?.trim().parse().ok()
}

/// Liefert 0, wenn der Text keine gültige 32-Bit-Zahl ist; `isInt` unterscheidet beide Fälle.
///
/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_convert_to_int(data: *const u8, len: usize) -> i32 {
    // SAFETY: Vertrag der Funktion.
    parse_int(unsafe { view(data, len) }).unwrap_or(0)
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_convert_is_int(data: *const u8, len: usize) -> bool {
    // SAFETY: Vertrag der Funktion.
    parse_int(unsafe { view(data, len) }).is_some()
}

fn path_of(bytes: &[u8]) -> Option<&std::path::Path> {
    std::str::from_utf8(bytes).ok().map(std::path::Path::new)
}

/// Gesamter Dateiinhalt; bei Fehlern ein leerer String (vorher `File.exists` prüfen).
///
/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_file_read_all_text(
    path: *const u8,
    len: usize,
) -> *mut StringSlice {
    // SAFETY: Vertrag der Funktion.
    let content = path_of(unsafe { view(path, len) })
        .and_then(|path| std::fs::read(path).ok())
        .unwrap_or_default();
    new_string(&content)
}

fn write_file(path: &[u8], content: &[u8], append: bool) -> bool {
    let Some(path) = path_of(path) else {
        return false;
    };
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true);
    if append {
        options.append(true);
    } else {
        options.truncate(true);
    }
    options
        .open(path)
        .and_then(|mut file| file.write_all(content))
        .is_ok()
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_file_write_all_text(
    path: *const u8,
    path_len: usize,
    data: *const u8,
    len: usize,
) -> bool {
    // SAFETY: Vertrag der Funktion.
    unsafe { write_file(view(path, path_len), view(data, len), false) }
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_file_append_all_text(
    path: *const u8,
    path_len: usize,
    data: *const u8,
    len: usize,
) -> bool {
    // SAFETY: Vertrag der Funktion.
    unsafe { write_file(view(path, path_len), view(data, len), true) }
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_file_exists(path: *const u8, len: usize) -> bool {
    // SAFETY: Vertrag der Funktion.
    path_of(unsafe { view(path, len) }).is_some_and(|path| path.is_file())
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_file_delete(path: *const u8, len: usize) -> bool {
    // SAFETY: Vertrag der Funktion.
    path_of(unsafe { view(path, len) }).is_some_and(|path| std::fs::remove_file(path).is_ok())
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_directory_exists(path: *const u8, len: usize) -> bool {
    // SAFETY: Vertrag der Funktion.
    path_of(unsafe { view(path, len) }).is_some_and(|path| path.is_dir())
}

/// Legt das Verzeichnis samt fehlender Elternverzeichnisse an.
///
/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_directory_create(path: *const u8, len: usize) -> bool {
    // SAFETY: Vertrag der Funktion.
    path_of(unsafe { view(path, len) }).is_some_and(|path| std::fs::create_dir_all(path).is_ok())
}

/// Eine Zeile von der Standardeingabe ohne Zeilenende; am Dateiende ein leerer String.
#[unsafe(no_mangle)]
pub extern "C" fn nortex_console_read_line() -> *mut StringSlice {
    let mut line = Vec::new();
    let _ = std::io::stdin().lock().read_until(b'\n', &mut line);
    while matches!(line.last(), Some(b'\n' | b'\r')) {
        line.pop();
    }
    new_string(&line)
}

#[unsafe(no_mangle)]
pub extern "C" fn nortex_convert_to_double(value: i32) -> f64 {
    f64::from(value)
}

/// Schneidet zur Null hin ab und sättigt am Wertebereich; NaN ergibt 0.
#[unsafe(no_mangle)]
pub extern "C" fn nortex_convert_double_to_int(value: f64) -> i32 {
    value as i32
}

/// Kürzeste Rundreise-Darstellung; ganzzahlige Werte erhalten ein ".0".
#[unsafe(no_mangle)]
pub extern "C" fn nortex_convert_double_to_string(value: f64) -> *mut StringSlice {
    let mut text = value.to_string();
    if value.is_finite() && !text.contains('.') {
        text.push_str(".0");
    }
    new_string(text.as_bytes())
}

fn parse_double(bytes: &[u8]) -> Option<f64> {
    let value = std::str::from_utf8(bytes)
        .ok()?
        .trim()
        .parse::<f64>()
        .ok()?;
    value.is_finite().then_some(value)
}

/// Liefert 0.0 bei ungültigem Text oder nicht endlichen Werten (`inf`, `NaN`).
///
/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_convert_parse_double(data: *const u8, len: usize) -> f64 {
    // SAFETY: Vertrag der Funktion.
    parse_double(unsafe { view(data, len) }).unwrap_or(0.0)
}

/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_convert_is_double(data: *const u8, len: usize) -> bool {
    // SAFETY: Vertrag der Funktion.
    parse_double(unsafe { view(data, len) }).is_some()
}

#[unsafe(no_mangle)]
pub extern "C" fn nortex_math_sqrt(value: f64) -> f64 {
    value.sqrt()
}

#[unsafe(no_mangle)]
pub extern "C" fn nortex_math_abs(value: f64) -> f64 {
    value.abs()
}

#[unsafe(no_mangle)]
pub extern "C" fn nortex_math_floor(value: f64) -> f64 {
    value.floor()
}

#[unsafe(no_mangle)]
pub extern "C" fn nortex_math_pow(base: f64, exponent: f64) -> f64 {
    base.powf(exponent)
}

/// Listen sind undurchsichtige Handles (Zeiger auf einen Rust-Vec). Ungültige Indizes
/// lassen `get` 0 bzw. "" liefern und `set` false; `free` gibt die Liste frei.
struct IntList(Vec<i32>);
struct StringList(Vec<Vec<u8>>);

fn into_handle<T>(value: T) -> usize {
    Box::into_raw(Box::new(value)) as usize
}

/// # Safety
/// `handle` stammt aus `into_handle::<T>` und wurde nicht freigegeben.
unsafe fn list_mut<'a, T>(handle: usize) -> Option<&'a mut T> {
    // SAFETY: Vertrag der Funktion; Null wird als ungültig abgelehnt.
    unsafe { (handle as *mut T).as_mut() }
}

/// # Safety
/// `handle` stammt aus `into_handle::<T>` und wurde nicht freigegeben.
unsafe fn drop_handle<T>(handle: usize) -> bool {
    if handle == 0 {
        return false;
    }
    // SAFETY: Vertrag der Funktion.
    drop(unsafe { Box::from_raw(handle as *mut T) });
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn nortex_int_list_create() -> usize {
    into_handle(IntList(Vec::new()))
}

/// # Safety
/// `list` muss ein lebendes `IntList`-Handle sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_int_list_add(list: usize, value: i32) -> bool {
    // SAFETY: Vertrag der Funktion.
    unsafe { list_mut::<IntList>(list) }.is_some_and(|list| {
        list.0.push(value);
        true
    })
}

/// # Safety
/// `list` muss ein lebendes `IntList`-Handle sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_int_list_get(list: usize, index: i32) -> i32 {
    // SAFETY: Vertrag der Funktion.
    unsafe { list_mut::<IntList>(list) }
        .and_then(|list| usize::try_from(index).ok().and_then(|i| list.0.get(i)))
        .copied()
        .unwrap_or(0)
}

/// # Safety
/// `list` muss ein lebendes `IntList`-Handle sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_int_list_set(list: usize, index: i32, value: i32) -> bool {
    // SAFETY: Vertrag der Funktion.
    unsafe { list_mut::<IntList>(list) }
        .and_then(|list| usize::try_from(index).ok().and_then(|i| list.0.get_mut(i)))
        .map(|slot| *slot = value)
        .is_some()
}

/// # Safety
/// `list` muss ein lebendes `IntList`-Handle sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_int_list_length(list: usize) -> i32 {
    // SAFETY: Vertrag der Funktion.
    unsafe { list_mut::<IntList>(list) }.map_or(0, |list| clamp_to_i32(list.0.len()))
}

/// # Safety
/// `list` muss ein lebendes `IntList`-Handle sein; danach ist es ungültig.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_int_list_free(list: usize) -> bool {
    // SAFETY: Vertrag der Funktion.
    unsafe { drop_handle::<IntList>(list) }
}

#[unsafe(no_mangle)]
pub extern "C" fn nortex_string_list_create() -> usize {
    into_handle(StringList(Vec::new()))
}

/// Kopiert den String in die Liste.
///
/// # Safety
/// `list` muss ein lebendes `StringList`-Handle sein; Zeiger/Länge müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_string_list_add(list: usize, data: *const u8, len: usize) -> bool {
    // SAFETY: Vertrag der Funktion.
    let bytes = unsafe { view(data, len) };
    // SAFETY: Vertrag der Funktion.
    unsafe { list_mut::<StringList>(list) }.is_some_and(|list| {
        list.0.push(bytes.to_vec());
        true
    })
}

/// Liefert eine neue Kopie als Heap-String (leer bei ungültigem Index).
///
/// # Safety
/// `list` muss ein lebendes `StringList`-Handle sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_string_list_get(list: usize, index: i32) -> *mut StringSlice {
    // SAFETY: Vertrag der Funktion.
    let item = unsafe { list_mut::<StringList>(list) }
        .and_then(|list| usize::try_from(index).ok().and_then(|i| list.0.get(i)));
    new_string(item.map_or(&[][..], Vec::as_slice))
}

/// # Safety
/// `list` muss ein lebendes `StringList`-Handle sein; Zeiger/Länge müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_string_list_set(
    list: usize,
    index: i32,
    data: *const u8,
    len: usize,
) -> bool {
    // SAFETY: Vertrag der Funktion.
    let bytes = unsafe { view(data, len) };
    // SAFETY: Vertrag der Funktion.
    unsafe { list_mut::<StringList>(list) }
        .and_then(|list| usize::try_from(index).ok().and_then(|i| list.0.get_mut(i)))
        .map(|slot| *slot = bytes.to_vec())
        .is_some()
}

/// # Safety
/// `list` muss ein lebendes `StringList`-Handle sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_string_list_length(list: usize) -> i32 {
    // SAFETY: Vertrag der Funktion.
    unsafe { list_mut::<StringList>(list) }.map_or(0, |list| clamp_to_i32(list.0.len()))
}

/// # Safety
/// `list` muss ein lebendes `StringList`-Handle sein; danach ist es ungültig.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_string_list_free(list: usize) -> bool {
    // SAFETY: Vertrag der Funktion.
    unsafe { drop_handle::<StringList>(list) }
}

/// Teilt an jedem Vorkommen von `sep`; ein leeres `sep` liefert den ganzen Text als einziges Element.
///
/// # Safety
/// Zeiger/Längen-Paare müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_split(
    data: *const u8,
    len: usize,
    sep: *const u8,
    sep_len: usize,
) -> usize {
    // SAFETY: Vertrag der Funktion.
    let (text, sep) = unsafe { (view(data, len), view(sep, sep_len)) };
    let mut parts = Vec::new();
    let mut rest = text;
    if !sep.is_empty() {
        while let Some(position) = find(rest, sep) {
            parts.push(rest[..position].to_vec());
            rest = &rest[position + sep.len()..];
        }
    }
    parts.push(rest.to_vec());
    into_handle(StringList(parts))
}

/// # Safety
/// `list` muss ein lebendes `StringList`-Handle sein; Zeiger/Länge müssen lesbar sein.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nortex_text_join(
    list: usize,
    sep: *const u8,
    sep_len: usize,
) -> *mut StringSlice {
    // SAFETY: Vertrag der Funktion.
    let sep = unsafe { view(sep, sep_len) };
    // SAFETY: Vertrag der Funktion.
    let items = unsafe { list_mut::<StringList>(list) }.map_or(&[][..], |list| list.0.as_slice());
    new_string(&items.join(sep))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles_and_lists_work() {
        assert_eq!(text_of(nortex_convert_double_to_string(2.0)), "2.0");
        assert_eq!(text_of(nortex_convert_double_to_string(-1.25)), "-1.25");
        assert_eq!(nortex_convert_double_to_int(-2.9), -2);
        assert_eq!(nortex_convert_double_to_int(f64::NAN), 0);
        // SAFETY: Alle Zeiger stammen aus lebenden Strings und Handles.
        unsafe {
            assert_eq!(nortex_convert_parse_double(b" 1.5 ".as_ptr(), 5), 1.5);
            assert!(!nortex_convert_is_double(b"inf".as_ptr(), 3));
            assert!(!nortex_convert_is_double(b"x".as_ptr(), 1));
            let ints = nortex_int_list_create();
            assert!(nortex_int_list_add(ints, 7));
            assert!(nortex_int_list_set(ints, 0, 9));
            assert!(!nortex_int_list_set(ints, 1, 9));
            assert_eq!(nortex_int_list_get(ints, 0), 9);
            assert_eq!(nortex_int_list_get(ints, 5), 0);
            assert_eq!(nortex_int_list_length(ints), 1);
            assert!(nortex_int_list_free(ints));
            let parts = nortex_text_split(b"a,b,,c".as_ptr(), 6, b",".as_ptr(), 1);
            assert_eq!(nortex_string_list_length(parts), 4);
            assert_eq!(text_of(nortex_string_list_get(parts, 3)), "c");
            assert_eq!(text_of(nortex_text_join(parts, b"-".as_ptr(), 1)), "a-b--c");
            assert!(nortex_string_list_free(parts));
        }
    }

    fn text_of(descriptor: *mut StringSlice) -> String {
        assert!(!descriptor.is_null());
        // SAFETY: Der Descriptor stammt aus `new_string` und ist gültig.
        let bytes = unsafe { std::slice::from_raw_parts((*descriptor).data, (*descriptor).len) };
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[test]
    fn text_functions_work_on_bytes() {
        let s = "  Grüße, Welt  ";
        // SAFETY: Alle Zeiger stammen aus lebenden Strings.
        unsafe {
            assert_eq!(
                text_of(nortex_text_trim(s.as_ptr(), s.len())),
                "Grüße, Welt"
            );
            assert_eq!(
                text_of(nortex_text_substring(b"abcdef".as_ptr(), 6, 2, 3)),
                "cde"
            );
            assert_eq!(text_of(nortex_text_substring(b"abc".as_ptr(), 3, 9, 3)), "");
            assert_eq!(
                text_of(nortex_text_substring(b"abc".as_ptr(), 3, -2, 2)),
                "ab"
            );
            assert_eq!(
                nortex_text_index_of(b"hello".as_ptr(), 5, b"ll".as_ptr(), 2),
                2
            );
            assert_eq!(
                nortex_text_index_of(b"hello".as_ptr(), 5, b"z".as_ptr(), 1),
                -1
            );
            assert!(nortex_text_starts_with(
                b"hello".as_ptr(),
                5,
                b"he".as_ptr(),
                2
            ));
            assert!(nortex_text_ends_with(
                b"hello".as_ptr(),
                5,
                b"lo".as_ptr(),
                2
            ));
            assert_eq!(nortex_text_char_at(b"A".as_ptr(), 1, 0), 65);
            assert_eq!(nortex_text_char_at(b"A".as_ptr(), 1, 1), -1);
            assert_eq!(text_of(nortex_text_to_upper("äb".as_ptr(), 3)), "ÄB");
            assert_eq!(
                text_of(nortex_text_replace(
                    b"a-b-c".as_ptr(),
                    5,
                    b"-".as_ptr(),
                    1,
                    b"+-".as_ptr(),
                    2
                )),
                "a+-b+-c"
            );
            assert_eq!(
                text_of(nortex_text_replace(
                    b"abc".as_ptr(),
                    3,
                    b"".as_ptr(),
                    0,
                    b"x".as_ptr(),
                    1
                )),
                "abc"
            );
        }
    }

    #[test]
    fn conversions_round_trip() {
        assert_eq!(text_of(nortex_convert_int_to_string(-42)), "-42");
        assert_eq!(text_of(nortex_convert_bool_to_string(true)), "true");
        // SAFETY: Alle Zeiger stammen aus lebenden Strings.
        unsafe {
            assert_eq!(nortex_convert_to_int(b" 17 ".as_ptr(), 4), 17);
            assert_eq!(nortex_convert_to_int(b"x".as_ptr(), 1), 0);
            assert!(nortex_convert_is_int(b"-5".as_ptr(), 2));
            assert!(!nortex_convert_is_int(b"99999999999".as_ptr(), 11));
            assert!(!nortex_convert_is_int(b"".as_ptr(), 0));
        }
    }

    #[test]
    fn files_can_be_written_appended_read_and_deleted() {
        let directory = std::env::temp_dir().join(format!("nortex-text-{}", std::process::id()));
        let directory_text = directory.join("a").join("b").to_string_lossy().into_owned();
        let file_text = format!("{directory_text}\\datei.txt");
        // SAFETY: Alle Zeiger stammen aus lebenden Strings.
        unsafe {
            assert!(!nortex_directory_exists(
                directory_text.as_ptr(),
                directory_text.len()
            ));
            assert!(nortex_directory_create(
                directory_text.as_ptr(),
                directory_text.len()
            ));
            assert!(nortex_directory_exists(
                directory_text.as_ptr(),
                directory_text.len()
            ));
            assert!(nortex_file_write_all_text(
                file_text.as_ptr(),
                file_text.len(),
                "Grüße".as_ptr(),
                "Grüße".len()
            ));
            assert!(nortex_file_append_all_text(
                file_text.as_ptr(),
                file_text.len(),
                b"!".as_ptr(),
                1
            ));
            assert!(nortex_file_exists(file_text.as_ptr(), file_text.len()));
            assert_eq!(
                text_of(nortex_file_read_all_text(
                    file_text.as_ptr(),
                    file_text.len()
                )),
                "Grüße!"
            );
            assert!(nortex_file_delete(file_text.as_ptr(), file_text.len()));
            assert!(!nortex_file_exists(file_text.as_ptr(), file_text.len()));
            assert_eq!(
                text_of(nortex_file_read_all_text(
                    file_text.as_ptr(),
                    file_text.len()
                )),
                ""
            );
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
