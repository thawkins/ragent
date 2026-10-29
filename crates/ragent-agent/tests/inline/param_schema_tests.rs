//! Inline tests for the machine-readable reason codes carried by tool errors
//! (spec `agentssetup` FR-010, NFR-005).

use super::*;
use serde_json::json;

/// `reason_in` recovers the code from a message `reason_error` built, and
/// reports "no code" for prose that predates the convention.
#[test]
fn test_reason_code_round_trip() {
    let err = reason_error("limit_reached", "2 of 2 background slots are in use");
    let message = err.to_string();
    assert_eq!(reason_in(&message), Some("limit_reached"));
    assert!(
        message.contains("2 of 2 background slots are in use"),
        "the human half must survive the prefix: {message}"
    );
    assert!(
        message.starts_with("reason_code: "),
        "the code must be visibly prefixed: {message}"
    );

    // A plain message carries no code, so a caller cannot mistake prose for one.
    assert_eq!(reason_in("Missing required parameter: agent"), None);
    assert_eq!(reason_in(""), None);
    // A code is lowercase snake case, never empty and never padded.
    assert_eq!(reason_in("_leading: x"), None);
    assert_eq!(reason_in("trailing_: x"), None);
    assert_eq!(reason_in("UPPER: x"), None);
    assert_eq!(reason_in("no_separator"), None);
}

/// `with_reason` attaches the code to a result's metadata without disturbing
/// the fields already there.
#[test]
fn test_with_reason_attaches_to_metadata() {
    let metadata = with_reason(
        Some(json!({ "task_id": "explore-1", "status": "running" })),
        "x",
    );
    assert_eq!(metadata["reason"], json!("x"));
    assert_eq!(metadata["task_id"], json!("explore-1"));
    assert_eq!(metadata["status"], json!("running"));

    // Absent metadata still yields an object carrying the reason.
    let bare = with_reason(None, "not_found");
    assert_eq!(bare["reason"], json!("not_found"));
}
