use nortex_ide::editor::EditorBuffer;

#[test]
fn large_multiline_unicode_document_survives_edit_undo_redo() {
    let source = (0..12_000)
        .map(|line| format!("int value_{line} = {line};"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut buffer = EditorBuffer::from_text(source.clone());
    assert_eq!(buffer.line_count(), 12_000);

    let offset = buffer
        .line_column_to_offset(6_000, 0)
        .expect("middle line should have a start offset");
    buffer.replace_range(offset..offset, "string greeting = \"Grüße 😊\";\n");
    assert_eq!(buffer.line_count(), 12_001);
    assert_eq!(
        buffer.line(6_000).as_deref(),
        Some("string greeting = \"Grüße 😊\";")
    );

    assert!(buffer.undo());
    assert_eq!(buffer.materialize(), source);
    assert_eq!(buffer.line_count(), 12_000);
    assert!(buffer.redo());
    assert_eq!(buffer.line_count(), 12_001);
}
