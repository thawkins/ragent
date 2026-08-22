//! External tests for `tests` from `crates/ragent-specs/src/templates.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_specs::constitution::parse_constitution;
use ragent_specs::spec::SpecId;
use ragent_specs::templates::{ConstitutionTemplate, FeedbackTemplate, PlanTemplate, SpecTemplate};

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
    assert!(
        md.contains("| ID | Title | Requirement | Effort | Priority | Status | Dependencies |")
    );
    assert!(md.contains("| T-001 |"));
}
// ── Quality checklist tests (T-010, FR-006) ──────────────────────────────────

#[test]
fn test_spec_template_with_checklist_contains_section() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_checklist(&id, "Test Title", &[], true);
    assert!(
        md.contains("## Quality Checklist"),
        "should contain Quality Checklist section: {md}"
    );
}

#[test]
fn test_spec_template_with_checklist_has_completeness_items() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_checklist(&id, "Test Title", &[], true);
    assert!(
        md.contains("valid EARS notation"),
        "should mention EARS notation completeness"
    );
    assert!(
        md.contains("independently testable"),
        "should mention testability"
    );
    assert!(
        md.contains("No speculative features"),
        "should mention absence of speculative features"
    );
}

#[test]
fn test_spec_template_without_checklist_omits_section() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_checklist(&id, "Test Title", &[], false);
    assert!(
        !md.contains("## Quality Checklist"),
        "should NOT contain Quality Checklist when include_checklist is false"
    );
}

#[test]
fn test_spec_template_generate_has_no_checklist_by_default() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate(&id, "Test Title");
    assert!(
        !md.contains("## Quality Checklist"),
        "generate() should NOT contain Quality Checklist by default"
    );
}

#[test]
fn test_spec_template_with_research_and_checklist() {
    let id = SpecId::new("test").unwrap();
    let md =
        SpecTemplate::generate_with_checklist(&id, "Test Title", &["rust-async".to_string()], true);
    assert!(
        md.contains("## Related Research"),
        "should have research section"
    );
    assert!(
        md.contains("## Quality Checklist"),
        "should have quality checklist section"
    );
}
// ── Plan quality checklist tests (T-011, FR-006) ─────────────────────────────

#[test]
fn test_plan_template_with_checklist_contains_section() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate_with_checklist(&id, "Test Plan", true);
    assert!(
        md.contains("## Quality Checklist"),
        "should contain Quality Checklist section: {md}"
    );
}

#[test]
fn test_plan_template_with_checklist_has_traceability_items() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate_with_checklist(&id, "Test Plan", true);
    assert!(
        md.contains("references at least one requirement"),
        "should mention requirement traceability"
    );
    assert!(
        md.contains("acceptance criterion"),
        "should mention task testability"
    );
    assert!(
        md.contains("No speculative tasks"),
        "should mention absence of speculative tasks"
    );
}

#[test]
fn test_plan_template_without_checklist_omits_section() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate_with_checklist(&id, "Test Plan", false);
    assert!(
        !md.contains("## Quality Checklist"),
        "should NOT contain Quality Checklist when include_checklist is false"
    );
}

#[test]
fn test_plan_template_generate_has_no_checklist_by_default() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate(&id, "Test Plan");
    assert!(
        !md.contains("## Quality Checklist"),
        "generate() should NOT contain Quality Checklist by default"
    );
}
// ── FeedbackTemplate tests (T-031, FR-017) ──────────────────────────────────

#[test]
fn test_feedback_template_contains_header() {
    let md = FeedbackTemplate::generate("My Spec");
    assert!(md.contains("# Feedback: My Spec"));
}

#[test]
fn test_feedback_template_contains_notes_table() {
    let md = FeedbackTemplate::generate("Test");
    assert!(md.contains("## Feedback Notes"));
    assert!(md.contains("| Date | Source | Note |"));
    assert!(md.contains("| [YYYY-MM-DD] | [metric/incident/user] | [Feedback note] |"));
}

#[test]
fn test_feedback_template_contains_advisory_note() {
    let md = FeedbackTemplate::generate("Test");
    assert!(md.contains("advisory"));
    assert!(md.contains("do not block validation"));
}
// ── Edge-case tests for templates (T-041, NFR-004) ────────────────────────────

#[test]
fn test_feedback_template_empty_title() {
    let md = FeedbackTemplate::generate("");
    assert!(
        md.contains("# Feedback:"),
        "should contain header even with empty title: {md}"
    );
    assert!(md.contains("## Feedback Notes"));
}

#[test]
fn test_spec_template_checklist_has_clarification_item() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_checklist(&id, "Test Title", &[], true);
    assert!(
        md.contains("NEEDS CLARIFICATION"),
        "checklist should mention NEEDS CLARIFICATION markers: {md}"
    );
}

#[test]
fn test_plan_template_checklist_has_dependency_item() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate_with_checklist(&id, "Test Plan", true);
    assert!(
        md.contains("acyclic"),
        "checklist should mention acyclic dependencies: {md}"
    );
}

// ── Comprehensive checklist embedding tests (T-012, FR-006, NFR-004) ──────────

/// Verify the spec checklist includes the "no missing requirements" item.
#[test]
fn test_spec_template_checklist_has_no_missing_requirements_item() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_checklist(&id, "Test Title", &[], true);
    assert!(
        md.contains("no missing requirements"),
        "checklist should mention missing requirements: {md}"
    );
}

/// Verify the spec checklist includes the "no speculative features" item.
#[test]
fn test_spec_template_checklist_has_no_speculative_features_item() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_checklist(&id, "Test Title", &[], true);
    assert!(
        md.contains("No speculative features"),
        "checklist should mention no speculative features: {md}"
    );
    assert!(
        md.contains("gold-plating"),
        "checklist should mention gold-plating: {md}"
    );
}

/// Verify the spec checklist has the self-review intro text.
#[test]
fn test_spec_template_checklist_has_self_review_intro() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_checklist(&id, "Test Title", &[], true);
    assert!(
        md.contains("Self-review before transitioning to `approved`"),
        "checklist should have self-review intro: {md}"
    );
}

/// Verify all spec checklist items use unchecked `- [ ]` checkbox format.
#[test]
fn test_spec_template_checklist_items_are_unchecked_boxes() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_checklist(&id, "Test Title", &[], true);
    let checklist_start = md
        .find("## Quality Checklist")
        .expect("checklist section should exist");
    let checklist_end = md
        .find("## Glossary")
        .or_else(|| md.find("*End of Specification*"))
        .expect("section after checklist should exist");
    let checklist = &md[checklist_start..checklist_end];
    let unchecked_count = checklist.matches("- [ ]").count();
    assert_eq!(
        unchecked_count, 5,
        "spec checklist should have exactly 5 unchecked items, found {unchecked_count}: {checklist}"
    );
    assert!(
        !checklist.contains("- [x]"),
        "checklist items should not be pre-checked: {checklist}"
    );
}

/// Verify the spec checklist is placed before the Glossary section.
#[test]
fn test_spec_template_checklist_placed_before_glossary() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_checklist(&id, "Test Title", &[], true);
    let checklist_pos = md
        .find("## Quality Checklist")
        .expect("checklist should exist");
    let glossary_pos = md.find("## Glossary").expect("glossary should exist");
    assert!(
        checklist_pos < glossary_pos,
        "checklist should appear before Glossary"
    );
}

/// Verify the plan checklist includes the requirement coverage item.
#[test]
fn test_plan_template_checklist_has_coverage_item() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate_with_checklist(&id, "Test Plan", true);
    assert!(
        md.contains("All requirements from the SPEC.md are covered"),
        "checklist should mention requirement coverage: {md}"
    );
}

/// Verify the plan checklist includes the gold-plating item.
#[test]
fn test_plan_template_checklist_has_gold_plating_item() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate_with_checklist(&id, "Test Plan", true);
    assert!(
        md.contains("gold-plating"),
        "checklist should mention gold-plating: {md}"
    );
}

/// Verify the plan checklist includes the task ID existence item.
#[test]
fn test_plan_template_checklist_has_task_id_existence_item() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate_with_checklist(&id, "Test Plan", true);
    assert!(
        md.contains("all referenced task IDs exist"),
        "checklist should mention task ID existence: {md}"
    );
}

/// Verify the plan checklist has the self-review intro text.
#[test]
fn test_plan_template_checklist_has_self_review_intro() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate_with_checklist(&id, "Test Plan", true);
    assert!(
        md.contains("Self-review before transitioning to `approved`"),
        "checklist should have self-review intro: {md}"
    );
}

/// Verify all plan checklist items use unchecked `- [ ]` checkbox format.
#[test]
fn test_plan_template_checklist_items_are_unchecked_boxes() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate_with_checklist(&id, "Test Plan", true);
    let checklist_start = md
        .find("## Quality Checklist")
        .expect("checklist section should exist");
    let checklist_end = md
        .find("## Definition of Done")
        .expect("section after checklist should exist");
    let checklist = &md[checklist_start..checklist_end];
    let unchecked_count = checklist.matches("- [ ]").count();
    assert_eq!(
        unchecked_count, 5,
        "plan checklist should have exactly 5 unchecked items, found {unchecked_count}: {checklist}"
    );
    assert!(
        !checklist.contains("- [x]"),
        "checklist items should not be pre-checked: {checklist}"
    );
}

/// Verify the plan checklist is placed before the Definition of Done section.
#[test]
fn test_plan_template_checklist_placed_before_definition_of_done() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate_with_checklist(&id, "Test Plan", true);
    let checklist_pos = md
        .find("## Quality Checklist")
        .expect("checklist should exist");
    let dod_pos = md
        .find("## Definition of Done")
        .expect("definition of done should exist");
    assert!(
        checklist_pos < dod_pos,
        "checklist should appear before Definition of Done"
    );
}

/// Verify checklist is omitted when `generate_with_research` is called
/// (which delegates to `generate_with_checklist` with `include_checklist=false`).
#[test]
fn test_spec_template_generate_with_research_has_no_checklist() {
    let id = SpecId::new("test").unwrap();
    let md = SpecTemplate::generate_with_research(&id, "Test Title", &["item".to_string()]);
    assert!(
        !md.contains("## Quality Checklist"),
        "generate_with_research should NOT include checklist: {md}"
    );
}

// ── File Creation Order tests (T-024, FR-014) ──────────────────────────────

/// Verify the plan template includes a `## File Creation Order` section.
#[test]
fn test_plan_template_has_file_creation_order_section() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate(&id, "Test Plan");
    assert!(
        md.contains("## File Creation Order"),
        "plan template should contain File Creation Order section: {md}"
    );
}

/// Verify the file creation order lists contracts first.
#[test]
fn test_plan_template_file_order_contracts_first() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate(&id, "Test Plan");
    let order_pos = md
        .find("## File Creation Order")
        .expect("File Creation Order section should exist");
    let risks_pos = md
        .find("## Risks & Mitigations")
        .expect("Risks section should exist");
    let section = &md[order_pos..risks_pos];
    assert!(
        section.contains("Contracts"),
        "file creation order should mention Contracts: {section}"
    );
    // Contracts should be item 1
    let contracts_pos = section
        .find("Contracts")
        .expect("Contracts should be mentioned");
    let first_numbered = section.find("1.").expect("should have numbered item 1");
    assert!(
        contracts_pos > first_numbered,
        "Contracts should be the first item in the ordering"
    );
}

/// Verify the file creation order lists test types in the correct sequence:
/// contract tests, then integration tests, then e2e tests, then unit tests.
#[test]
fn test_plan_template_file_order_test_sequence() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate(&id, "Test Plan");
    let order_pos = md
        .find("## File Creation Order")
        .expect("File Creation Order section should exist");
    let risks_pos = md
        .find("## Risks & Mitigations")
        .expect("Risks section should exist");
    let section = &md[order_pos..risks_pos];

    let contract_pos = section
        .find("Contract tests")
        .expect("should mention Contract tests");
    let integration_pos = section
        .find("Integration tests")
        .expect("should mention Integration tests");
    let e2e_pos = section
        .find("End-to-end")
        .expect("should mention End-to-end tests");
    let unit_pos = section
        .find("Unit tests")
        .expect("should mention Unit tests");

    assert!(
        contract_pos < integration_pos,
        "Contract tests should come before Integration tests"
    );
    assert!(
        integration_pos < e2e_pos,
        "Integration tests should come before End-to-end tests"
    );
    assert!(
        e2e_pos < unit_pos,
        "End-to-end tests should come before Unit tests"
    );
}

/// Verify the file creation order lists source files last.
#[test]
fn test_plan_template_file_order_source_last() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate(&id, "Test Plan");
    let order_pos = md
        .find("## File Creation Order")
        .expect("File Creation Order section should exist");
    let risks_pos = md
        .find("## Risks & Mitigations")
        .expect("Risks section should exist");
    let section = &md[order_pos..risks_pos];

    let unit_pos = section
        .find("Unit tests")
        .expect("should mention Unit tests");
    let source_pos = section
        .find("Source files")
        .expect("should mention Source files");

    assert!(
        unit_pos < source_pos,
        "Unit tests should come before Source files (source files last)"
    );
}

/// Verify the file creation order section mentions the advisory nature
/// and references FR-014.
#[test]
fn test_plan_template_file_order_mentions_advisory() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate(&id, "Test Plan");
    let order_pos = md
        .find("## File Creation Order")
        .expect("File Creation Order section should exist");
    let risks_pos = md
        .find("## Risks & Mitigations")
        .expect("Risks section should exist");
    let section = &md[order_pos..risks_pos];
    assert!(
        section.contains("advisory"),
        "file creation order should mention it is advisory: {section}"
    );
    assert!(
        section.contains("FR-014"),
        "file creation order should reference FR-014: {section}"
    );
}

/// Verify the file creation order section is present even when
/// the quality checklist is included.
#[test]
fn test_plan_template_file_order_present_with_checklist() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate_with_checklist(&id, "Test Plan", true);
    assert!(
        md.contains("## File Creation Order"),
        "File Creation Order should be present with checklist: {md}"
    );
    assert!(
        md.contains("## Quality Checklist"),
        "Quality Checklist should also be present: {md}"
    );
}

/// Verify the file creation order section is placed after the Tasks table
/// and before Risks & Mitigations.
#[test]
fn test_plan_template_file_order_placement() {
    let id = SpecId::new("test").unwrap();
    let md = PlanTemplate::generate(&id, "Test Plan");
    let tasks_pos = md.find("## Tasks").expect("Tasks section should exist");
    let order_pos = md
        .find("## File Creation Order")
        .expect("File Creation Order section should exist");
    let risks_pos = md
        .find("## Risks & Mitigations")
        .expect("Risks section should exist");
    assert!(
        tasks_pos < order_pos,
        "File Creation Order should appear after Tasks"
    );
    assert!(
        order_pos < risks_pos,
        "File Creation Order should appear before Risks & Mitigations"
    );
}

// ── ConstitutionTemplate tests (T-014, FR-007) ─────────────────────────────

/// Verify the constitution template has a `# Constitution` header.
#[test]
fn test_constitution_template_has_header() {
    let md = ConstitutionTemplate::generate();
    assert!(
        md.contains("# Constitution"),
        "should contain # Constitution header: {md}"
    );
}

/// Verify the constitution template includes an introductory paragraph.
#[test]
fn test_constitution_template_has_intro() {
    let md = ConstitutionTemplate::generate();
    assert!(
        md.contains("Immutable architectural principles"),
        "should contain intro about immutable principles: {md}"
    );
}

/// Verify the constitution template includes the Library-First article.
#[test]
fn test_constitution_template_has_library_first_article() {
    let md = ConstitutionTemplate::generate();
    assert!(
        md.contains("## Article 1: Library-First"),
        "should contain Article 1: Library-First: {md}"
    );
    assert!(
        md.contains("Depend on libraries, not frameworks"),
        "should contain Library-First principle text: {md}"
    );
}

/// Verify the constitution template includes the Simplicity article.
#[test]
fn test_constitution_template_has_simplicity_article() {
    let md = ConstitutionTemplate::generate();
    assert!(
        md.contains("## Article 2: Simplicity"),
        "should contain Article 2: Simplicity: {md}"
    );
    assert!(
        md.contains("simplest thing that works"),
        "should contain Simplicity principle text: {md}"
    );
}

/// Verify the constitution template includes the Anti-Abstraction article.
#[test]
fn test_constitution_template_has_anti_abstraction_article() {
    let md = ConstitutionTemplate::generate();
    assert!(
        md.contains("## Article 3: Anti-Abstraction"),
        "should contain Article 3: Anti-Abstraction: {md}"
    );
    assert!(
        md.contains("three concrete examples"),
        "should contain Anti-Abstraction principle text: {md}"
    );
}

/// Verify the constitution template includes the Integration-First Testing article.
#[test]
fn test_constitution_template_has_integration_first_testing_article() {
    let md = ConstitutionTemplate::generate();
    assert!(
        md.contains("## Article 4: Integration-First Testing"),
        "should contain Article 4: Integration-First Testing: {md}"
    );
    assert!(
        md.contains("Test through public interfaces"),
        "should contain Integration-First Testing principle text: {md}"
    );
}

/// Verify the constitution template includes the Constitutional Amendment Process article.
#[test]
fn test_constitution_template_has_amendment_process_article() {
    let md = ConstitutionTemplate::generate();
    assert!(
        md.contains("## Article 5: Constitutional Amendment Process"),
        "should contain Article 5: Constitutional Amendment Process: {md}"
    );
    assert!(
        md.contains("dated changelog entry"),
        "should contain amendment process principle text: {md}"
    );
}

/// Verify the constitution template has exactly 5 articles with sequential numbering.
#[test]
fn test_constitution_template_has_five_sequential_articles() {
    let md = ConstitutionTemplate::generate();
    for n in 1..=5 {
        assert!(
            md.contains(&format!("## Article {n}: ")),
            "should contain Article {n}: {md}"
        );
    }
    // Ensure there is no Article 6
    assert!(
        !md.contains("## Article 6:"),
        "should not contain Article 6: {md}"
    );
}

/// Verify the constitution template mentions the amendment footer.
#[test]
fn test_constitution_template_has_amendment_footer() {
    let md = ConstitutionTemplate::generate();
    assert!(
        md.contains("Amendments require a dated changelog entry"),
        "should contain amendment footer text: {md}"
    );
}

/// Verify the generated constitution template is round-trip parseable
/// by `parse_constitution` with all 5 articles extracted.
#[test]
fn test_constitution_template_round_trip_parse() {
    let md = ConstitutionTemplate::generate();
    let constitution = parse_constitution(&md);
    assert_eq!(
        constitution.articles.len(),
        5,
        "parse_constitution should extract 5 articles from the template"
    );
    assert_eq!(constitution.articles[0].number, 1);
    assert_eq!(constitution.articles[0].title, "Library-First");
    assert_eq!(constitution.articles[1].number, 2);
    assert_eq!(constitution.articles[1].title, "Simplicity");
    assert_eq!(constitution.articles[2].number, 3);
    assert_eq!(constitution.articles[2].title, "Anti-Abstraction");
    assert_eq!(constitution.articles[3].number, 4);
    assert_eq!(constitution.articles[3].title, "Integration-First Testing");
    assert_eq!(constitution.articles[4].number, 5);
    assert_eq!(
        constitution.articles[4].title,
        "Constitutional Amendment Process"
    );
}
