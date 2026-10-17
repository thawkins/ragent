//! Isolation coverage for the message-parts update paths
//! (`update_message` / `update_message_parts_skip_fts`).
//!
//! These methods rewrite the serialised `parts` JSON (and, for the FTS-syncing
//! variant, delete + re-insert the `messages_fts` row). The tests pin the
//! observable post-condition of both variants: the parts blob changes and the
//! FTS index is either resynced or deliberately left untouched.

use ragent_storage::storage::Storage;
use ragent_types::message::{Message, MessagePart};

fn storage() -> Storage {
    let s = Storage::open_in_memory().expect("storage");
    s.create_session("sess-1", "/tmp/project").expect("session");
    s
}

/// `update_message` rewrites the parts JSON and resyncs the FTS entry so a
/// search for the *new* text finds the message and the old text does not.
#[test]
fn update_message_rewrites_parts_and_resyncs_fts() {
    let storage = storage();
    let mut msg = Message::user_text("sess-1", "alpha bravo");
    storage.create_message(&msg).expect("create");

    // Before the update the old text is searchable.
    assert_eq!(
        storage
            .search_conversation("sess-1", "alpha", 10)
            .expect("search")
            .len(),
        1
    );

    msg.parts = vec![
        MessagePart::Text {
            text: "charlie delta".to_string(),
        },
        MessagePart::Reasoning {
            text: "reasoning text".to_string(),
        },
    ];
    msg.touch();
    storage.update_message(&msg).expect("update");

    let stored = storage.get_messages("sess-1").expect("get");
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].text_content(), "charlie delta");
    assert!(
        !stored[0].parts.iter().any(|p| matches!(
            p,
            MessagePart::Text { text } if text.contains("alpha")
        )),
        "old text replaced in the parts blob"
    );

    let hits = storage
        .search_conversation("sess-1", "charlie", 10)
        .expect("search new");
    assert_eq!(hits.len(), 1, "FTS resynced to the new content");
    assert!(hits[0].content.contains("charlie delta"));

    let stale = storage
        .search_conversation("sess-1", "alpha", 10)
        .expect("search old");
    assert!(stale.is_empty(), "old FTS content removed");
}

/// `update_message_parts_skip_fts` rewrites the parts blob but leaves the FTS
/// entry pointing at the *old* text, so an in-progress interim save does not
/// churn the search index.
#[test]
fn update_message_parts_skip_fts_leaves_index_untouched() {
    let storage = storage();
    let mut msg = Message::user_text("sess-1", "findable original");
    storage.create_message(&msg).expect("create");

    msg.parts = vec![MessagePart::Text {
        text: "unindexed replacement".to_string(),
    }];
    msg.touch();
    storage
        .update_message_parts_skip_fts(&msg)
        .expect("skip-fts update");

    // The base row carries the new parts ...
    let stored = storage.get_messages("sess-1").expect("get");
    assert_eq!(stored[0].text_content(), "unindexed replacement");

    // ... while the FTS entry still holds the pre-update text.
    let old_hits = storage
        .search_conversation("sess-1", "findable", 10)
        .expect("search old");
    assert_eq!(old_hits.len(), 1, "FTS entry not touched");
    let new_hits = storage
        .search_conversation("sess-1", "unindexed", 10)
        .expect("search new");
    assert!(new_hits.is_empty(), "new text not indexed yet");
}

/// A subsequent full `update_message` finalises the interim save and brings
/// the FTS entry back into sync.
#[test]
fn full_update_after_skip_fts_resyncs() {
    let storage = storage();
    let mut msg = Message::user_text("sess-1", "original text");
    storage.create_message(&msg).expect("create");

    msg.parts = vec![MessagePart::Text {
        text: "interim text".to_string(),
    }];
    storage
        .update_message_parts_skip_fts(&msg)
        .expect("interim");

    msg.parts = vec![MessagePart::Text {
        text: "final text".to_string(),
    }];
    storage.update_message(&msg).expect("final");

    let hits = storage
        .search_conversation("sess-1", "final", 10)
        .expect("search");
    assert_eq!(hits.len(), 1);
    assert!(hits[0].content.contains("final text"));
}
