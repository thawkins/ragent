//! Coverage for [`FinishReason`]'s three label mappings.
//!
//! `as_str`, the `Display` impl, and `from_label` must agree so the persisted
//! label and the remote relay's inverse mapping cannot drift.

use ragent_types::event::FinishReason;

const LABELS: [(&str, FinishReason); 6] = [
    ("stop", FinishReason::Stop),
    ("tool_use", FinishReason::ToolUse),
    ("length", FinishReason::Length),
    ("content_filter", FinishReason::ContentFilter),
    ("cancelled", FinishReason::Cancelled),
    ("truncation", FinishReason::Truncation),
];

#[test]
fn as_str_matches_every_reason() {
    for (label, reason) in &LABELS {
        assert_eq!(reason.as_str(), *label);
    }
}

#[test]
fn display_matches_as_str() {
    for (label, reason) in &LABELS {
        assert_eq!(reason.to_string(), *label);
    }
}

#[test]
fn from_label_round_trips_every_reason() {
    for (label, reason) in &LABELS {
        assert_eq!(FinishReason::from_label(label), *reason);
    }
}

#[test]
fn from_label_defaults_unknown_to_stop() {
    assert_eq!(FinishReason::from_label(""), FinishReason::Stop);
    assert_eq!(FinishReason::from_label("nonsense"), FinishReason::Stop);
    assert_eq!(FinishReason::from_label("STOP"), FinishReason::Stop);
}
