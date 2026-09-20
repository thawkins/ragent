//! External tests for `tests` from `crates/ragent-agent/src/memory/store.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::memory::store::{ForgetFilter, MEMORY_CATEGORIES, StructuredMemory};

#[test]
fn test_validate_category_valid() {
    for cat in MEMORY_CATEGORIES {
        assert!(StructuredMemory::validate_category(cat).is_ok());
    }
}

#[test]
fn test_validate_category_invalid() {
    assert!(StructuredMemory::validate_category("invalid").is_err());
    assert!(StructuredMemory::validate_category("").is_err());
}

#[test]
fn test_validate_confidence() {
    assert!(StructuredMemory::validate_confidence(0.0).is_ok());
    assert!(StructuredMemory::validate_confidence(1.0).is_ok());
    assert!(StructuredMemory::validate_confidence(0.5).is_ok());
    assert!(StructuredMemory::validate_confidence(-0.1).is_err());
    assert!(StructuredMemory::validate_confidence(1.1).is_err());
}

#[test]
fn test_validate_tags() {
    assert!(StructuredMemory::validate_tags(&["valid-tag".to_string(), "v2".to_string()]).is_ok());
    assert!(StructuredMemory::validate_tags(&[String::new()]).is_err());
    assert!(StructuredMemory::validate_tags(&["NotValid".to_string()]).is_err());
    assert!(StructuredMemory::validate_tags(&["bad_tag".to_string()]).is_err());
}

#[allow(clippy::float_cmp)]
#[test]
fn test_structured_memory_new() {
    let mem = StructuredMemory::new("Use Result<T, E>", "pattern");
    assert_eq!(mem.content, "Use Result<T, E>");
    assert_eq!(mem.category, "pattern");
    assert_eq!(mem.confidence, 0.7);
    assert!(mem.tags.is_empty());
}

#[allow(clippy::float_cmp)]
#[test]
fn test_structured_memory_builder() {
    let mem = StructuredMemory::new("Test content", "fact")
        .with_confidence(0.9)
        .with_source("auto-extract")
        .with_project("ragent")
        .with_session_id("sess-1")
        .with_tags(vec!["rust".to_string()]);
    assert_eq!(mem.confidence, 0.9);
    assert_eq!(mem.source, "auto-extract");
    assert_eq!(mem.project, "ragent");
    assert_eq!(mem.session_id, "sess-1");
    assert_eq!(mem.tags, vec!["rust"]);
}

#[allow(clippy::float_cmp)]
#[test]
fn test_confidence_clamped() {
    let mem = StructuredMemory::new("test", "fact").with_confidence(2.0);
    assert_eq!(mem.confidence, 1.0);
    let mem = StructuredMemory::new("test", "fact").with_confidence(-1.0);
    assert_eq!(mem.confidence, 0.0);
}

#[test]
fn test_forget_filter_has_criterion() {
    let filter = ForgetFilter::Filter {
        older_than_days: Some(30),
        max_confidence: None,
        category: None,
        tags: None,
    };
    assert!(filter.has_any_criterion());

    let empty = ForgetFilter::Filter {
        older_than_days: None,
        max_confidence: None,
        category: None,
        tags: None,
    };
    assert!(!empty.has_any_criterion());

    let empty_tags = ForgetFilter::Filter {
        older_than_days: None,
        max_confidence: None,
        category: None,
        tags: Some(vec![]),
    };
    assert!(!empty_tags.has_any_criterion());

    let id_filter = ForgetFilter::Id(42);
    assert!(id_filter.has_any_criterion());
}
