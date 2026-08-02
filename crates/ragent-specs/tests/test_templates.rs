//! External tests for `tests` from `crates/ragent-specs/src/templates.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_specs::spec::SpecId;
use ragent_specs::templates::{PlanTemplate, SpecTemplate};

#[test]
fn test_spec_template_contains_sections() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate(&id, "Test Title");
    assert!(md.contains("# Specification: Test Title"));
    assert!(md.contains("## Executive Summary"));
    assert!(md.contains("## Functional Requirements"));
    assert!(md.contains("## Non-Functional Requirements"));
    assert!(md.contains("## Constraints & Assumptions"));
    assert!(md.contains("## Interfaces & Dependencies"));
    assert!(md.contains("## Glossary"));
    assert!(md.contains("status: draft"));
    assert!(md.contains("id: test"));
}

#[test]
fn test_spec_template_with_research_emits_related_section() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_research(
        &id,
        "Test Title",
        &["rust-async".to_string(), "tokio-runtime".to_string()],
    );
    assert!(md.contains("## Related Research"));
    assert!(md.contains("`rust-async`"));
    assert!(md.contains("../research/rust-async/RESEARCH.md"));
    assert!(md.contains("`tokio-runtime`"));
    assert!(md.contains("research: [\"rust-async\", \"tokio-runtime\"]"));
}

#[test]
fn test_spec_template_without_research_omits_related_section() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_research(&id, "Test Title", &[]);
    assert!(!md.contains("## Related Research"));
    assert!(!md.contains("research: ["));
}

#[test]
fn test_spec_template_has_ears_examples() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate(&id, "Test");
    assert!(md.contains("The <SYSTEM NAME> shall <SYSTEM RESPONSE>"));
    assert!(md.contains("When <TRIGGER>, the <SYSTEM NAME> shall"));
    assert!(md.contains("While <PRECONDITION>, the <SYSTEM NAME> shall"));
    assert!(md.contains("Where <FEATURE> is included, the <SYSTEM NAME> shall"));
    assert!(md.contains("If <TRIGGER>, the <SYSTEM NAME> shall"));
}

#[test]
fn test_plan_template_contains_sections() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate(&id, "Test Plan");
    assert!(md.contains("# Implementation Plan: Test Plan"));
    assert!(md.contains("## Overview"));
    assert!(md.contains("## Milestones"));
    assert!(md.contains("## Tasks"));
    assert!(md.contains("## Risks & Mitigations"));
    assert!(md.contains("## Definition of Done"));
    assert!(md.contains("spec_id: test"));
}

#[test]
fn test_plan_template_has_task_table() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate(&id, "Test Plan");
    assert!(md.contains("| ID | Title | Requirement | Effort | Priority | Dependencies |"));
    assert!(md.contains("| T-001 |"));
}
