#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-specs/src/spec.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_specs::spec::{EarsTemplate, Plan, Requirement, Spec, SpecId, SpecStatus, TaskStatus};
use std::path::Path;

#[test]
fn test_spec_id_valid() {
    assert!(SpecId::new("my-spec-1").is_some());
    assert!(SpecId::new("testspec").is_some());
    assert!(SpecId::new("spec_mgt_v1").is_some());
}

#[test]
fn test_spec_id_invalid() {
    assert!(SpecId::new("").is_none());
    assert!(SpecId::new("my spec").is_none());
    assert!(SpecId::new("my/spec").is_none());
    assert!(SpecId::new("my.spec").is_none());
}

#[test]
fn test_spec_status_parse() {
    assert_eq!(SpecStatus::parse("draft"), Some(SpecStatus::Draft));
    assert_eq!(SpecStatus::parse("in_review"), Some(SpecStatus::InReview));
    assert_eq!(SpecStatus::parse("approved"), Some(SpecStatus::Approved));
    assert_eq!(
        SpecStatus::parse("in_progress"),
        Some(SpecStatus::InProgress)
    );
    assert_eq!(
        SpecStatus::parse("implemented"),
        Some(SpecStatus::Implemented)
    );
    assert_eq!(SpecStatus::parse("verified"), Some(SpecStatus::Verified));
    assert_eq!(SpecStatus::parse("archived"), Some(SpecStatus::Archived));
    assert_eq!(SpecStatus::parse("unknown"), None);
}

#[test]
fn test_spec_status_display() {
    assert_eq!(SpecStatus::Draft.to_string(), "draft");
    assert_eq!(SpecStatus::InReview.to_string(), "in_review");
}

#[test]
fn test_spec_new() {
    let id = SpecId::new("test").unwrap();
    let spec = Spec::new(id, "Test Spec");
    assert_eq!(spec.id.as_str(), "test");
    assert_eq!(spec.status, SpecStatus::Draft);
    assert_eq!(spec.title, "Test Spec");
    assert_eq!(spec.coverage_pct(), 0.0);
}

#[test]
fn test_spec_paths() {
    let id = SpecId::new("testspec").unwrap();
    let spec = Spec::new(id, "Test");
    let root = Path::new("specs");
    assert_eq!(spec.dir_path(root), Path::new("specs/testspec"));
    assert_eq!(spec.spec_md_path(root), Path::new("specs/testspec/SPEC.md"));
    assert_eq!(spec.plan_md_path(root), Path::new("specs/testspec/PLAN.md"));
}

#[test]
fn test_spec_transition() {
    let id = SpecId::new("test").unwrap();
    let mut spec = Spec::new(id, "Test");
    spec.transition(SpecStatus::InReview, "alice");
    assert_eq!(spec.status, SpecStatus::InReview);
    assert_eq!(spec.audit_trail.len(), 2);
    assert_eq!(spec.audit_trail[1].1, "draft");
    assert_eq!(spec.audit_trail[1].2, "in_review");
    assert_eq!(spec.audit_trail[1].3, "alice");
}

#[test]
fn test_spec_coverage() {
    let id = SpecId::new("test").unwrap();
    let mut spec = Spec::new(id, "Test");
    spec.requirements = vec![
        Requirement {
            id: "FR-001".to_string(),
            text: "The system shall do X.".to_string(),
            template: EarsTemplate::Ubiquitous,
            implemented: true,
        },
        Requirement {
            id: "FR-002".to_string(),
            text: "The system shall do Y.".to_string(),
            template: EarsTemplate::Ubiquitous,
            implemented: false,
        },
    ];
    assert!((spec.coverage_pct() - 50.0).abs() < f64::EPSILON);
}

#[test]
fn test_task_status() {
    assert_eq!(TaskStatus::Pending.as_str(), "pending");
    assert_eq!(TaskStatus::Completed.as_str(), "completed");
}

#[test]
fn test_ears_template_as_str() {
    assert_eq!(EarsTemplate::Ubiquitous.as_str(), "ubiquitous");
    assert_eq!(EarsTemplate::EventDriven.as_str(), "event_driven");
    assert_eq!(EarsTemplate::StateDriven.as_str(), "state_driven");
    assert_eq!(EarsTemplate::Optional.as_str(), "optional");
    assert_eq!(EarsTemplate::Unwanted.as_str(), "unwanted");
}

#[test]
fn test_plan_new() {
    let id = SpecId::new("test").unwrap();
    let plan = Plan::new(id, "Test Plan");
    assert_eq!(plan.title, "Test Plan");
    assert!(plan.tasks.is_empty());
}

#[test]
fn test_spec_status_all() {
    assert_eq!(SpecStatus::ALL.len(), 7);
    assert!(SpecStatus::ALL.contains(&SpecStatus::Draft));
    assert!(SpecStatus::ALL.contains(&SpecStatus::Archived));
}
