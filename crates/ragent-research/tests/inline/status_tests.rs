//! Inline tests for `status.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn default_is_draft() {
    assert_eq!(ResearchStatus::default(), ResearchStatus::Draft);
}

#[test]
fn as_str_returns_kebab_case() {
    assert_eq!(ResearchStatus::Draft.as_str(), "draft");
    assert_eq!(ResearchStatus::InProgress.as_str(), "in-progress");
    assert_eq!(ResearchStatus::Complete.as_str(), "complete");
    assert_eq!(ResearchStatus::Archived.as_str(), "archived");
}

#[test]
fn parse_round_trips_through_as_str() {
    for &status in ResearchStatus::ALL {
        assert_eq!(ResearchStatus::parse(status.as_str()), Some(status));
    }
}

#[test]
fn parse_accepts_common_aliases() {
    assert_eq!(
        ResearchStatus::parse("in_progress"),
        Some(ResearchStatus::InProgress)
    );
    assert_eq!(
        ResearchStatus::parse("in-progress"),
        Some(ResearchStatus::InProgress)
    );
    assert_eq!(
        ResearchStatus::parse("inprogress"),
        Some(ResearchStatus::InProgress)
    );
    assert_eq!(
        ResearchStatus::parse("completed"),
        Some(ResearchStatus::Complete)
    );
    assert_eq!(
        ResearchStatus::parse("done"),
        Some(ResearchStatus::Complete)
    );
    assert_eq!(
        ResearchStatus::parse("archive"),
        Some(ResearchStatus::Archived)
    );
}

#[test]
fn parse_returns_none_for_unknown() {
    assert_eq!(ResearchStatus::parse(""), None);
    assert_eq!(ResearchStatus::parse("unknown"), None);
    assert_eq!(ResearchStatus::parse("DRAFT"), None);
}

#[test]
fn terminal_predicate() {
    assert!(ResearchStatus::Archived.is_terminal());
    assert!(!ResearchStatus::Draft.is_terminal());
    assert!(!ResearchStatus::InProgress.is_terminal());
    assert!(!ResearchStatus::Complete.is_terminal());
}

#[test]
fn finished_predicate() {
    assert!(ResearchStatus::Complete.is_finished());
    assert!(!ResearchStatus::Draft.is_finished());
    assert!(!ResearchStatus::InProgress.is_finished());
    assert!(!ResearchStatus::Archived.is_finished());
}

#[test]
fn serde_round_trip() {
    for &status in ResearchStatus::ALL {
        let json = serde_json::to_string(&status).unwrap();
        let back: ResearchStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, status);
    }
}

#[test]
fn display_matches_as_str() {
    for &status in ResearchStatus::ALL {
        assert_eq!(status.to_string(), status.as_str());
    }
}
