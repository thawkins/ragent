//! FR-005: loading and re-rendering an unmodified note is byte-identical.

use ragent_hyperresearch::models::note::Note;

const NOTE: &str =
    "---\nid: alpha\ntitle: \"Quoted: title\"\ntags:\n  - one\n  - two\n---\nBody text.\n";

#[test]
fn test_note_roundtrip_is_byte_identical() {
    let note = Note::parse("alpha.md", NOTE).expect("note must parse");
    assert_eq!(note.meta.id, "alpha");
    assert_eq!(note.meta.tags, vec!["one".to_string(), "two".to_string()]);
    assert_eq!(note.render(), NOTE);
}

#[test]
fn test_note_without_frontmatter_roundtrips_body() {
    let note = Note::parse("plain.md", "just text\n").expect("note must parse");
    assert_eq!(note.render(), "just text\n");
}
