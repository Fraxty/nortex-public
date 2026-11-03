//! Textabgleich zwischen egui-Eingabe und Editorpuffer, Zoom-Eingabe.

use crate::*;

pub(crate) fn sync_buffer(buffer: &mut EditorBuffer, old: &str, new: &str) {
    if old == new {
        return;
    }
    let mut prefix = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(left, right)| left == right)
        .count();
    while prefix > 0 && (!old.is_char_boundary(prefix) || !new.is_char_boundary(prefix)) {
        prefix -= 1;
    }
    let max_suffix = old.len().min(new.len()).saturating_sub(prefix);
    let mut suffix = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(max_suffix)
        .take_while(|(left, right)| left == right)
        .count();
    while suffix > 0
        && (!old.is_char_boundary(old.len() - suffix) || !new.is_char_boundary(new.len() - suffix))
    {
        suffix -= 1;
    }
    let new_end = new.len() - suffix;
    buffer.replace_range(prefix..old.len() - suffix, &new[prefix..new_end]);
}

pub(crate) fn zoom_input(zoom: f32) -> String {
    format!("{:.0}%", zoom * 100.0)
}

pub(crate) fn parse_zoom_input(input: &str) -> Option<f32> {
    let percent = input.trim().strip_suffix('%').unwrap_or(input.trim());
    let percent = percent.trim().parse::<f32>().ok()?;
    if !percent.is_finite() || !(70.0..=200.0).contains(&percent) {
        return None;
    }
    Some(percent / 100.0)
}
