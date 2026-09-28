//! MS-04 (SECTASKS T-066) storage guard: a diff line beginning with a
//! multibyte character must not panic the restore path.

use chrono::Utc;
use ragent_storage::snapshot::{IncrementalSnapshot, Snapshot};
use std::collections::HashMap;

#[test]
fn to_full_does_not_panic_on_a_multibyte_first_char() {
    let base = Snapshot {
        id: "base".into(),
        session_id: "s".into(),
        message_id: "m".into(),
        files: HashMap::from([(std::path::PathBuf::from("s.txt"), b"hello\n".to_vec())]),
        created_at: Utc::now(),
    };
    // `split_at(1)` panicked when byte 1 was not a UTF-8 char boundary.
    let delta = IncrementalSnapshot {
        id: "d".into(),
        base_id: "base".into(),
        session_id: "s".into(),
        message_id: "m".into(),
        diffs: HashMap::from([(
            std::path::PathBuf::from("s.txt"),
            "\u{e9} line that does not start with an ASCII tag".to_string(),
        )]),
        added: HashMap::new(),
        deleted: Vec::new(),
        created_at: Utc::now(),
    };
    // The offending line is skipped rather than aborting the process.
    let full = delta.to_full(base);
    assert!(full.is_ok(), "restore must return a Result, not panic");
}
