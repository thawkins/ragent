//! Tests for validate.rs (M8/T8.4).
//! Compiled as a submodule via #[path], `super::`* resolves to the source module.

use super::*;
use crate::spec::{Spec, SpecId};

fn valid_spec() -> Spec {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test Spec");
    spec.spec_md = r"---
status: draft
id: testspec
---

# Specification: Test Spec

## Executive Summary

A test spec.

## Scope & Objectives

### Scope

In scope.

## Functional Requirements

### FR-001 — Requirement One

`The system shall do X.`

### FR-002 — Requirement Two

`When Y happens, the system shall do Z.`

### NFR-001 — Performance

`The system shall respond within 2 seconds.`

## Non-Functional Requirements

### NFR-002 — Reliability

`The system shall handle errors gracefully.`

## Constraints & Assumptions

### Constraints

1. Must use Rust.

### Assumptions

1. Users have Git.
"
    .to_string();

    spec.plan_md = r"# Implementation Plan: Test Spec

## Overview

A plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-001 | S | High |
| T-002 | Do Z | FR-002 | S | High |
"
    .to_string();
    spec
}

#[test]
fn test_detect_ears_ubiquitous() {
    assert_eq!(
        detect_ears_template("The system shall do X."),
        Some(EarsTemplate::Ubiquitous)
    );
}

#[test]
fn test_detect_ears_event_driven() {
    assert_eq!(
        detect_ears_template("When the button is pressed, the system shall beep."),
        Some(EarsTemplate::EventDriven)
    );
}

#[test]
fn test_detect_ears_state_driven() {
    assert_eq!(
        detect_ears_template("While the engine is running, the system shall monitor temperature."),
        Some(EarsTemplate::StateDriven)
    );
}

#[test]
fn test_detect_ears_optional() {
    assert_eq!(
        detect_ears_template("Where logging is included, the system shall write to a file."),
        Some(EarsTemplate::Optional)
    );
}

#[test]
fn test_detect_ears_unwanted() {
    assert_eq!(
        detect_ears_template("If the temperature exceeds 100C, the system shall shutdown."),
        Some(EarsTemplate::Unwanted)
    );
}

#[test]
fn test_detect_ears_invalid() {
    assert_eq!(
        detect_ears_template("This is just a random sentence."),
        None
    );
}

#[test]
fn test_validate_valid_spec() {
    let spec = valid_spec();
    let report = validate(&spec);
    assert!(
        !report.has_errors(),
        "valid spec should have no errors: {report:?}"
    );
}

#[test]
fn test_validate_missing_section() {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test");
    spec.spec_md = "# Spec\n\n## Executive Summary\n\nX.\n".to_string();
    spec.plan_md = valid_spec().plan_md;
    let report = validate(&spec);
    assert!(report.has_errors());
    let missing = report
        .issues
        .iter()
        .any(|i| i.category == Category::MissingSection);
    assert!(missing, "should flag missing sections");
}

#[test]
fn test_validate_no_requirements() {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test");
    spec.spec_md = r"---
status: draft
---

# Spec

## Executive Summary
X.

## Scope & Objectives
### Scope
X.

## Functional Requirements

None.

## Non-Functional Requirements

None.

## Constraints & Assumptions
### Constraints
1. X.
"
    .to_string();
    spec.plan_md = "# Plan\n".to_string();
    let report = validate(&spec);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.message.contains("No requirements found"))
    );
}

#[test]
fn test_validate_duplicate_req_id() {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test");
    spec.spec_md = r"---
status: draft
---

# Spec

## Executive Summary
X.

## Scope & Objectives
### Scope
X.

## Functional Requirements

### FR-001 — One
`The system shall do X.`

### FR-001 — Duplicate
`The system shall do Y.`

## Non-Functional Requirements

None.

## Constraints & Assumptions
### Constraints
1. X.
"
    .to_string();
    spec.plan_md = "# Plan\n".to_string();
    let report = validate(&spec);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.message.contains("Duplicate"))
    );
}

#[test]
fn test_validate_numbering_gap() {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test");
    spec.spec_md = r"---
status: draft
---

# Spec

## Executive Summary
X.

## Scope & Objectives
### Scope
X.

## Functional Requirements

### FR-001 — One
`The system shall do X.`

### FR-003 — Three
`The system shall do Z.`

## Non-Functional Requirements

None.

## Constraints & Assumptions
### Constraints
1. X.
"
    .to_string();
    spec.plan_md = "# Plan\n".to_string();
    let report = validate(&spec);
    assert!(report.issues.iter().any(|i| i.message.contains("Gap")));
}

#[test]
fn test_validate_invalid_status() {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test");
    spec.spec_md = r"---
status: bananas
---

# Spec

## Executive Summary
X.

## Scope & Objectives
### Scope
X.

## Functional Requirements

### FR-001 — One
`The system shall do X.`

## Non-Functional Requirements

None.

## Constraints & Assumptions
### Constraints
1. X.
"
    .to_string();
    spec.plan_md = "# Plan\n".to_string();
    let report = validate(&spec);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.category == Category::InvalidStatus)
    );
}

#[test]
fn test_validate_plan_empty() {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test");
    spec.spec_md = valid_spec().spec_md;
    spec.plan_md = String::new();
    let report = validate(&spec);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.message.contains("PLAN.md is empty"))
    );
}

#[test]
fn test_validate_plan_missing_section() {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test");
    spec.spec_md = valid_spec().spec_md;
    spec.plan_md = "# Plan\n\nJust a title.\n".to_string();
    let report = validate(&spec);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.category == Category::Plan && i.message.contains("missing section"))
    );
}

#[test]
fn test_validate_unknown_requirement_reference() {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test");
    spec.spec_md = valid_spec().spec_md;
    spec.plan_md = r"# Plan

## Overview

Plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-999 | S | High |
"
    .to_string();
    let report = validate(&spec);
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.message.contains("unknown requirement"))
    );
}

#[test]
fn test_parse_requirements() {
    let md = r"## Functional Requirements

### FR-001 — First
`The system shall do A.`

### FR-002 — Second
`When B, the system shall do C.`

### NFR-001 — Perf
`The system shall respond fast.`
";
    let reqs = parse_requirements(md);
    assert_eq!(reqs.len(), 3);
    assert_eq!(reqs[0].id, "FR-001");
    assert_eq!(reqs[0].ears_text, "The system shall do A.");
    assert_eq!(reqs[1].id, "FR-002");
    assert_eq!(reqs[1].ears_text, "When B, the system shall do C.");
    assert_eq!(reqs[2].id, "NFR-001");
}

#[test]
fn test_parse_requirements_no_ears() {
    let md = "### FR-001 — No EARS\n\nJust text.\n";
    let reqs = parse_requirements(md);
    assert_eq!(reqs.len(), 1);
    assert!(reqs[0].ears_text.is_empty());
}

#[test]
fn test_extract_sections() {
    let md = "## A\n\n### B\n\n## C\n";
    let sections = extract_sections(md);
    assert_eq!(sections.len(), 3);
    assert_eq!(sections[0], (2, "A".to_string(), 1));
    assert_eq!(sections[1], (3, "B".to_string(), 3));
    assert_eq!(sections[2], (2, "C".to_string(), 5));
}

#[test]
fn test_report_sorting() {
    let mut report = Report::new();
    report.add(Issue::new(Severity::Info, Category::Structure, "Info").with_line(10));
    report.add(Issue::new(Severity::Error, Category::Structure, "Error").with_line(5));
    report.add(Issue::new(Severity::Warning, Category::Structure, "Warn").with_line(3));
    report.sort();
    assert_eq!(report.issues[0].severity, Severity::Error);
    assert_eq!(report.issues[0].line, Some(5));
    assert_eq!(report.issues[1].severity, Severity::Warning);
    assert_eq!(report.issues[1].line, Some(3));
    assert_eq!(report.issues[2].severity, Severity::Info);
    assert_eq!(report.issues[2].line, Some(10));
}

#[test]
fn test_report_format() {
    let mut report = Report::new();
    report.requirement_count = 2;
    report.valid_ears_count = 1;
    report.add(
        Issue::new(Severity::Error, Category::EarsSyntax, "Bad syntax")
            .with_line(5)
            .with_id("FR-001"),
    );
    let formatted = report.format("my-spec");
    assert!(formatted.contains("Validation Report for `my-spec`"));
    assert!(formatted.contains("Requirements: 2 total, 1 EARS-valid"));
    assert!(formatted.contains("1 error(s)"));
    assert!(formatted.contains("[FR-001]"));
    assert!(formatted.contains("line 5"));
}

#[test]
fn test_report_has_errors() {
    let mut report = Report::new();
    assert!(!report.has_errors());
    report.add(Issue::new(Severity::Warning, Category::Structure, "Warn"));
    assert!(!report.has_errors());
    report.add(Issue::new(Severity::Error, Category::Structure, "Err"));
    assert!(report.has_errors());
}

#[test]
fn test_report_count_by_severity() {
    let mut report = Report::new();
    report.add(Issue::new(Severity::Error, Category::Structure, "E1"));
    report.add(Issue::new(Severity::Error, Category::Structure, "E2"));
    report.add(Issue::new(Severity::Warning, Category::Structure, "W1"));
    assert_eq!(report.count_by_severity(Severity::Error), 2);
    assert_eq!(report.count_by_severity(Severity::Warning), 1);
    assert_eq!(report.count_by_severity(Severity::Info), 0);
}

// ── Clarification marker detection tests (T-007, FR-002) ──────────────────

#[test]
fn test_detect_clarification_markers_single() {
    let content = "Some text [NEEDS CLARIFICATION: what scale?] more text";
    let markers = detect_clarification_markers(content);
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].question, "what scale?");
    assert_eq!(markers[0].line, 1);
}

#[test]
fn test_detect_clarification_markers_multiple() {
    let content = "\
Line one
[NEEDS CLARIFICATION: first question?]
Line three with [NEEDS CLARIFICATION: second question?] inline
";
    let markers = detect_clarification_markers(content);
    assert_eq!(markers.len(), 2);
    assert_eq!(markers[0].line, 2);
    assert_eq!(markers[0].question, "first question?");
    assert_eq!(markers[1].line, 3);
    assert_eq!(markers[1].question, "second question?");
}

#[test]
fn test_detect_clarification_markers_case_insensitive() {
    let content = "[needs clarification: lowercase test?]";
    let markers = detect_clarification_markers(content);
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].question, "lowercase test?");
}

#[test]
fn test_detect_clarification_markers_none() {
    let content = "Just regular text without any markers.\nAnother line.";
    let markers = detect_clarification_markers(content);
    assert!(markers.is_empty());
}

#[test]
fn test_category_clarification_display() {
    assert_eq!(format!("{}", Category::Clarification), "Clarification");
}
// ── Ambiguity detection tests (T-026, FR-015) ──────────────────────────────

#[test]
fn test_detect_ambiguity_vague_terms() {
    let content = "## Functional Requirements\n\n### FR-001 - Performance\n\n`The system shall be fast and scalable.`\n";
    let issues = detect_ambiguity(content);
    assert!(
        issues
            .iter()
            .any(|i| i.term == "fast" && i.kind == AmbiguityKind::VagueTerm)
    );
    assert!(
        issues
            .iter()
            .any(|i| i.term == "scalable" && i.kind == AmbiguityKind::VagueTerm)
    );
}

#[test]
fn test_detect_ambiguity_no_vague_terms() {
    let content = "## Functional Requirements\n\n### FR-001 - Search\n\n`The system shall respond within 200 milliseconds.`\n";
    let issues = detect_ambiguity(content);
    assert!(issues.iter().all(|i| i.kind != AmbiguityKind::VagueTerm));
}

#[test]
fn test_detect_ambiguity_vague_term_has_requirement_id() {
    let content =
        "## Functional Requirements\n\n### FR-003 - UX\n\n`The system shall be user-friendly.`\n";
    let issues = detect_ambiguity(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].requirement_id.as_deref(), Some("FR-003"));
    assert_eq!(issues[0].kind, AmbiguityKind::VagueTerm);
    assert_eq!(issues[0].term, "user-friendly");
}

#[test]
fn test_detect_ambiguity_undefined_acronym() {
    let content = "## Functional Requirements\n\n### FR-001 - Integration\n\n`The system shall expose a REST API for data access.`\n";
    let issues = detect_ambiguity(content);
    // "REST" and "API" are both potential acronyms; neither is defined in the spec.
    let acronyms: Vec<&str> = issues
        .iter()
        .filter(|i| i.kind == AmbiguityKind::UndefinedAcronym)
        .map(|i| i.term.as_str())
        .collect();
    assert!(acronyms.contains(&"REST"));
    assert!(acronyms.contains(&"API"));
}

#[test]
fn test_detect_ambiguity_defined_acronym_not_flagged() {
    let content = "\
## Glossary

Representational State Transfer (REST): a web service architecture.

## Functional Requirements

### FR-001 - Integration

`The system shall expose a REST API for data access.`
";
    let issues = detect_ambiguity(content);
    // REST is defined, so it should not be flagged. API is not defined.
    let undefined: Vec<&str> = issues
        .iter()
        .filter(|i| i.kind == AmbiguityKind::UndefinedAcronym)
        .map(|i| i.term.as_str())
        .collect();
    assert!(!undefined.contains(&"REST"));
    assert!(undefined.contains(&"API"));
}

#[test]
fn test_detect_ambiguity_acronym_defined_before_paren() {
    let content = "\
## Glossary

API (Application Programming Interface): a set of endpoints.

## Functional Requirements

### FR-001 - Integration

`The system shall expose an API for data access.`
";
    let issues = detect_ambiguity(content);
    let undefined: Vec<&str> = issues
        .iter()
        .filter(|i| i.kind == AmbiguityKind::UndefinedAcronym)
        .map(|i| i.term.as_str())
        .collect();
    assert!(!undefined.contains(&"API"));
}

#[test]
fn test_detect_ambiguity_no_requirements() {
    let content = "# Specification\n\nSome intro text with no requirements.\n";
    let issues = detect_ambiguity(content);
    assert!(issues.is_empty());
}

#[test]
fn test_detect_ambiguity_no_ears_text() {
    let content = "## Functional Requirements\n\n### FR-001 - Something\n\nNo backticks here.\n";
    let issues = detect_ambiguity(content);
    assert!(issues.is_empty());
}

#[test]
fn test_detect_ambiguity_vague_term_as_needed() {
    let content = "## Functional Requirements\n\n### FR-001 - Flexibility\n\n`The system shall reload data as needed.`\n";
    let issues = detect_ambiguity(content);
    assert!(issues.iter().any(|i| i.term == "as needed"));
}

#[test]
fn test_detect_ambiguity_line_number() {
    let content =
        "## Functional Requirements\n\n### FR-001 - Speed\n\n`The system shall be fast.`\n";
    let issues = detect_ambiguity(content);
    // The EARS text is on line 5 of the content
    assert_eq!(issues[0].line, 5);
}

#[test]
fn test_category_ambiguity_display() {
    assert_eq!(format!("{}", Category::Ambiguity), "Ambiguity");
}

#[test]
fn test_ambiguity_kind_display() {
    assert_eq!(format!("{}", AmbiguityKind::VagueTerm), "vague term");
    assert_eq!(
        format!("{}", AmbiguityKind::UndefinedAcronym),
        "undefined acronym"
    );
}
// ── Edge-case tests for clarification markers (T-041, NFR-004) ──────────────

#[test]
fn test_detect_clarification_markers_multiple_same_line() {
    let content = "[NEEDS CLARIFICATION: question one?] and [NEEDS CLARIFICATION: question two?]";
    let markers = detect_clarification_markers(content);
    assert_eq!(markers.len(), 2, "should find two markers on the same line");
    assert_eq!(markers[0].line, 1);
    assert_eq!(markers[0].question, "question one?");
    assert_eq!(markers[1].line, 1);
    assert_eq!(markers[1].question, "question two?");
}

#[test]
fn test_detect_clarification_markers_special_chars() {
    let content = "[NEEDS CLARIFICATION: what about {braces} and [nested] brackets?]";
    let markers = detect_clarification_markers(content);
    // The regex is non-greedy, so it captures up to the first `]` — but
    // nested brackets may affect capture. Verify we find at least one.
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].line, 1);
}

#[test]
fn test_detect_clarification_markers_multiline_content() {
    let content = "\
Line 1: no marker
Line 2: [NEEDS CLARIFICATION: line 2 question?]
Line 3: no marker
Line 4: [NEEDS CLARIFICATION: line 4 question?]
Line 5: no marker
";
    let markers = detect_clarification_markers(content);
    assert_eq!(markers.len(), 2);
    assert_eq!(markers[0].line, 2);
    assert_eq!(markers[0].question, "line 2 question?");
    assert_eq!(markers[1].line, 4);
    assert_eq!(markers[1].question, "line 4 question?");
}

// ── Clarification reporting in Report tests (T-008, FR-002) ────────────────

#[test]
fn test_validate_clarifications_adds_warning_issues() {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test Spec");
    spec.spec_md = r"---
status: draft
id: testspec
---

# Specification: Test Spec

## Executive Summary

A test spec.

## Scope & Objectives

### Scope

In scope.

## Functional Requirements

### FR-001 — Requirement One

`The system shall do X.`

[NEEDS CLARIFICATION: what scale?]

## Non-Functional Requirements

### NFR-001 — Performance

`The system shall respond within 2 seconds.`

## Constraints & Assumptions

### Constraints

1. Must use Rust.
"
    .to_string();
    spec.plan_md = "# Implementation Plan\n## Overview\nA plan.\n## Milestones\n1. M1\n## Tasks\n| ID | Title | Requirement | Effort | Priority |\n|---|---|---|---|---|\n| T-001 | Do X | FR-001 | S | High |\n".to_string();

    let report = validate(&spec);
    assert!(
        report.has_clarifications(),
        "report should contain clarification issues"
    );
    assert_eq!(
        report.clarification_count(),
        1,
        "should detect exactly one clarification marker"
    );
    // Should be a warning, not an error (FR-002: must not reject)
    let clar_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.category == Category::Clarification)
        .collect();
    assert_eq!(clar_issues.len(), 1);
    assert_eq!(clar_issues[0].severity, Severity::Warning);
    assert!(clar_issues[0].message.contains("what scale?"));
}

#[test]
fn test_validate_clarifications_no_markers() {
    let spec = valid_spec();
    let report = validate(&spec);
    assert!(
        !report.has_clarifications(),
        "valid spec without markers should have no clarification issues"
    );
    assert_eq!(report.clarification_count(), 0);
}

#[test]
fn test_validate_clarifications_multiple_markers() {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test Spec");
    spec.spec_md = r"---
status: draft
id: testspec
---

# Specification: Test Spec

## Executive Summary

A test spec.

## Scope & Objectives

### Scope

In scope.

## Functional Requirements

### FR-001 — Requirement One

`The system shall do X.`

[NEEDS CLARIFICATION: first question?]
[NEEDS CLARIFICATION: second question?]

## Non-Functional Requirements

### NFR-001 — Performance

`The system shall respond within 2 seconds.`

## Constraints & Assumptions

### Constraints

1. Must use Rust.
"
    .to_string();
    spec.plan_md = "# Implementation Plan\n## Overview\nA plan.\n## Milestones\n1. M1\n## Tasks\n| ID | Title | Requirement | Effort | Priority |\n|---|---|---|---|---|\n| T-001 | Do X | FR-001 | S | High |\n".to_string();

    let report = validate(&spec);
    assert_eq!(report.clarification_count(), 2);
}

#[test]
fn test_report_format_includes_clarification_summary() {
    let mut report = Report::new();
    report.requirement_count = 1;
    report.valid_ears_count = 1;
    report.add(
        Issue::new(
            Severity::Warning,
            Category::Clarification,
            "Unresolved clarification marker: [NEEDS CLARIFICATION: what scale?]",
        )
        .with_line(10),
    );
    let formatted = report.format("my-spec");
    assert!(
        formatted.contains("Clarification markers: 1 unresolved"),
        "format should include clarification summary: {formatted}"
    );
}

#[test]
fn test_report_format_no_clarification_summary_when_none() {
    let mut report = Report::new();
    report.requirement_count = 1;
    report.valid_ears_count = 1;
    report.add(Issue::new(Severity::Error, Category::EarsSyntax, "Bad syntax").with_line(5));
    let formatted = report.format("my-spec");
    assert!(
        !formatted.contains("Clarification markers"),
        "format should NOT include clarification summary when none: {formatted}"
    );
}

#[test]
fn test_report_has_clarifications() {
    let mut report = Report::new();
    assert!(!report.has_clarifications());
    report.add(Issue::new(
        Severity::Warning,
        Category::Clarification,
        "Unresolved clarification marker",
    ));
    assert!(report.has_clarifications());
    assert_eq!(report.clarification_count(), 1);
}

// ── Contradiction detection tests (T-027, FR-015) ──────────────────────────

#[test]
fn test_detect_contradictions_negation_conflict() {
    let content = "\
## Functional Requirements

### FR-001 - Store Data

`The system shall store all user data.`

### FR-002 - No Storage

`The system shall not store all user data.`
";
    let issues = detect_contradictions(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, ContradictionKind::NegationConflict);
    assert_eq!(issues[0].req_a, "FR-001");
    assert_eq!(issues[0].req_b, "FR-002");
    assert_eq!(issues[0].term, "store");
}

#[test]
fn test_detect_contradictions_negation_with_never() {
    let content = "\
## Functional Requirements

### FR-001 - Log Events

`The system shall log events.`

### FR-002 - No Logging

`The system shall never log events.`
";
    let issues = detect_contradictions(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, ContradictionKind::NegationConflict);
}

#[test]
fn test_detect_contradictions_no_conflict_same_polarity() {
    let content = "\
## Functional Requirements

### FR-001 - Store Data

`The system shall store all user data.`

### FR-002 - Backup Data

`The system shall store backup data.`
";
    let issues = detect_contradictions(content);
    assert!(
        issues.is_empty(),
        "two positive requirements with same verb but different objects should not conflict"
    );
}

#[test]
fn test_detect_contradictions_opposite_actions() {
    let content = "\
## Functional Requirements

### FR-001 - Accept Input

`The system shall accept user requests.`

### FR-002 - Reject Input

`The system shall reject user requests.`
";
    let issues = detect_contradictions(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, ContradictionKind::OppositeAction);
    assert!(issues[0].term.contains("accept"));
    assert!(issues[0].term.contains("reject"));
}

#[test]
fn test_detect_contradictions_opposite_actions_no_overlap() {
    let content = "\
## Functional Requirements

### FR-001 - Enable Feature

`The system shall enable dark mode.`

### FR-002 - Disable Feature

`The system shall disable notifications.`
";
    let issues = detect_contradictions(content);
    assert!(
        issues.is_empty(),
        "opposite verbs with unrelated objects should not conflict"
    );
}

#[test]
fn test_detect_contradictions_no_contradictions() {
    let content = "\
## Functional Requirements

### FR-001 - Process Data

`The system shall process input data.`

### FR-002 - Render Output

`The system shall render output data.`
";
    let issues = detect_contradictions(content);
    assert!(issues.is_empty());
}

#[test]
fn test_detect_contradictions_no_requirements() {
    let content = "## Functional Requirements\n\nNo requirements here.\n";
    let issues = detect_contradictions(content);
    assert!(issues.is_empty());
}

#[test]
fn test_detect_contradictions_no_ears_text() {
    let content = "\
## Functional Requirements

### FR-001 - No EARS

No backtick text here.
";
    let issues = detect_contradictions(content);
    assert!(issues.is_empty());
}

#[test]
fn test_detect_contradictions_description_populated() {
    let content = "\
## Functional Requirements

### FR-001 - Store Data

`The system shall store all user data.`

### FR-002 - No Storage

`The system shall not store all user data.`
";
    let issues = detect_contradictions(content);
    assert_eq!(issues.len(), 1);
    assert!(issues[0].description.contains("FR-001"));
    assert!(issues[0].description.contains("FR-002"));
    assert!(issues[0].description.contains("store"));
}

#[test]
fn test_detect_contradictions_line_numbers() {
    let content = "\
## Functional Requirements

### FR-001 - Store Data

`The system shall store all user data.`

### FR-002 - No Storage

`The system shall not store all user data.`
";
    let issues = detect_contradictions(content);
    assert_eq!(issues.len(), 1);
    assert!(issues[0].line_a > 0);
    assert!(issues[0].line_b > 0);
    assert_ne!(issues[0].line_a, issues[0].line_b);
}

#[test]
fn test_detect_contradictions_multiple_pairs() {
    let content = "\
## Functional Requirements

### FR-001 - Store Data

`The system shall store all user data.`

### FR-002 - No Storage

`The system shall not store all user data.`

### FR-003 - Accept Input

`The system shall accept user requests.`

### FR-004 - Reject Input

`The system shall reject user requests.`
";
    let issues = detect_contradictions(content);
    assert_eq!(issues.len(), 2);
    assert!(
        issues
            .iter()
            .any(|i| i.kind == ContradictionKind::NegationConflict)
    );
    assert!(
        issues
            .iter()
            .any(|i| i.kind == ContradictionKind::OppositeAction)
    );
}

#[test]
fn test_detect_contradictions_three_requirements() {
    let content = "\
## Functional Requirements

### FR-001 - Store Data

`The system shall store all user data.`

### FR-002 - Process Data

`The system shall process input data.`

### FR-003 - No Storage

`The system shall not store all user data.`
";
    let issues = detect_contradictions(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, ContradictionKind::NegationConflict);
    assert_eq!(issues[0].req_a, "FR-001");
    assert_eq!(issues[0].req_b, "FR-003");
}

#[test]
fn test_detect_contradictions_different_objects_no_conflict() {
    let content = "\
## Functional Requirements

### FR-001 - Store User Data

`The system shall store user data.`

### FR-002 - No Log Storage

`The system shall not store log data.`
";
    let issues = detect_contradictions(content);
    assert!(
        issues.is_empty(),
        "same verb but different objects should not conflict"
    );
}

#[test]
fn test_contradiction_kind_display() {
    assert_eq!(
        ContradictionKind::NegationConflict.to_string(),
        "negation conflict"
    );
    assert_eq!(
        ContradictionKind::OppositeAction.to_string(),
        "opposite action"
    );
}

#[test]
fn test_category_contradiction_display() {
    assert_eq!(Category::Contradiction.to_string(), "Contradiction");
}

#[test]
fn test_detect_contradictions_inline_format() {
    let content = "\
## Functional Requirements

FR-001. The system shall store all user data.

FR-002. The system shall not store all user data.
";
    let issues = detect_contradictions(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, ContradictionKind::NegationConflict);
}

#[test]
fn test_detect_contradictions_opposite_enable_disable() {
    let content = "\
## Functional Requirements

### FR-001 - Enable Cache

`The system shall enable caching for requests.`

### FR-002 - Disable Cache

`The system shall disable caching for requests.`
";
    let issues = detect_contradictions(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, ContradictionKind::OppositeAction);
    assert!(issues[0].term.contains("enable"));
    assert!(issues[0].term.contains("disable"));
}

// ── Gap detection tests (T-028, FR-015) ─────────────────────────────────────

#[test]
fn test_detect_gaps_vague_outcome() {
    let content = "\
## Functional Requirements

### FR-001 - Quality

`The system shall be robust.`
";
    let issues = detect_gaps(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, GapKind::VagueOutcome);
    assert_eq!(issues[0].requirement_id, "FR-001");
    assert!(issues[0].suggestion.contains("testable action"));
}

#[test]
fn test_detect_gaps_no_measurable_criterion() {
    let content = "\
## Functional Requirements

### FR-001 - Handle Errors

`The system shall handle errors gracefully.`
";
    let issues = detect_gaps(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, GapKind::NoMeasurableCriterion);
    assert_eq!(issues[0].requirement_id, "FR-001");
    assert!(issues[0].suggestion.contains("measurable"));
}

#[test]
fn test_detect_gaps_testable_verb_no_digit_not_flagged() {
    let content = "\
## Functional Requirements

### FR-001 - Store Data

`The system shall store all user data.`
";
    let issues = detect_gaps(content);
    assert!(
        issues.is_empty(),
        "testable verb 'store' should not be flagged even without a digit"
    );
}

#[test]
fn test_detect_gaps_has_digit_not_flagged() {
    let content = "\
## Functional Requirements

### FR-001 - Performance

`The system shall respond within 2 seconds.`
";
    let issues = detect_gaps(content);
    assert!(
        issues.is_empty(),
        "requirement with a digit should not be flagged"
    );
}

#[test]
fn test_detect_gaps_vague_outcome_with_digit_not_flagged_for_no_measurable() {
    let content = "\
## Functional Requirements

### FR-001 - Availability

`The system shall be available 99.9 percent of the time.`
";
    let issues = detect_gaps(content);
    // "shall be" triggers VagueOutcome, but has_digit so NoMeasurableCriterion
    // should NOT fire. Only VagueOutcome is expected.
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, GapKind::VagueOutcome);
}

#[test]
fn test_detect_gaps_no_requirements() {
    let content = "## Functional Requirements\n\nNo requirements here.\n";
    let issues = detect_gaps(content);
    assert!(issues.is_empty());
}

#[test]
fn test_detect_gaps_no_ears_text() {
    let content = "\
## Functional Requirements

### FR-001 - No EARS

No backtick text here.
";
    let issues = detect_gaps(content);
    assert!(issues.is_empty());
}

#[test]
fn test_detect_gaps_multiple_issues() {
    let content = "\
## Functional Requirements

### FR-001 - Quality

`The system shall be robust.`

### FR-002 - Performance

`The system shall respond within 2 seconds.`

### FR-003 - Error Handling

`The system shall handle errors gracefully.`
";
    let issues = detect_gaps(content);
    assert_eq!(issues.len(), 2);
    assert!(issues.iter().any(|i| i.kind == GapKind::VagueOutcome));
    assert!(
        issues
            .iter()
            .any(|i| i.kind == GapKind::NoMeasurableCriterion)
    );
}

#[test]
fn test_detect_gaps_line_number_populated() {
    let content = "\
## Functional Requirements

### FR-001 - Quality

`The system shall be robust.`
";
    let issues = detect_gaps(content);
    assert_eq!(issues.len(), 1);
    assert!(issues[0].line > 0);
}

#[test]
fn test_detect_gaps_ears_text_populated() {
    let content = "\
## Functional Requirements

### FR-001 - Quality

`The system shall be robust.`
";
    let issues = detect_gaps(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].ears_text, "The system shall be robust.");
}

#[test]
fn test_detect_gaps_suggestion_populated() {
    let content = "\
## Functional Requirements

### FR-001 - Quality

`The system shall handle things appropriately.`
";
    let issues = detect_gaps(content);
    assert_eq!(issues.len(), 1);
    assert!(!issues[0].suggestion.is_empty());
}

#[test]
fn test_gap_kind_display() {
    assert_eq!(
        GapKind::NoMeasurableCriterion.to_string(),
        "no measurable criterion"
    );
    assert_eq!(GapKind::VagueOutcome.to_string(), "vague outcome");
}

#[test]
fn test_category_gap_display() {
    assert_eq!(Category::Gap.to_string(), "Gap");
}

#[test]
fn test_detect_gaps_testable_verds_not_flagged() {
    let content = "\
## Functional Requirements

### FR-001 - Display Result

`The system shall display the search results.`

### FR-002 - Log Events

`The system shall log all authentication events.`
";
    let issues = detect_gaps(content);
    assert!(
        issues.is_empty(),
        "testable verbs (display, log) should not be flagged"
    );
}

#[test]
fn test_detect_gaps_event_driven_with_digit() {
    let content = "\
## Functional Requirements

### FR-001 - Rate Limit

`When a user exceeds 100 requests, the system shall reject the request.`
";
    let issues = detect_gaps(content);
    assert!(
        issues.is_empty(),
        "event-driven requirement with digit and testable verb should not be flagged"
    );
}

#[test]
fn test_detect_gaps_event_driven_vague() {
    let content = "\
## Functional Requirements

### FR-001 - Graceful Handling

`When an error occurs, the system shall handle it gracefully.`
";
    let issues = detect_gaps(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, GapKind::NoMeasurableCriterion);
}

#[test]
fn test_detect_gaps_shall_not_testable() {
    let content = "\
## Functional Requirements

### FR-001 - No Storage

`The system shall not store passwords.`
";
    let issues = detect_gaps(content);
    assert!(
        issues.is_empty(),
        "'shall not store' uses a testable verb, should not be flagged"
    );
}

#[test]
fn test_detect_gaps_shall_be_with_testable_verb_after() {
    // "shall be able to store" — extract_action gets "be" as the verb,
    // so this is flagged as VagueOutcome. This is intentional: "shall be
    // able to" is weaker than "shall store".
    let content = "\
## Functional Requirements

### FR-001 - Capability

`The system shall be able to store user data.`
";
    let issues = detect_gaps(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, GapKind::VagueOutcome);
}

#[test]
fn test_detect_gaps_inline_format() {
    let content = "\
## Functional Requirements

FR-001. The system shall be scalable.
";
    let issues = detect_gaps(content);
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, GapKind::VagueOutcome);
}

// ── SDD flag-gated validation tests (T-036, FR-019) ───────────────────────

#[test]
fn test_sdd_flags_all_enabled() {
    let flags = SddFlags::all_enabled();
    assert!(flags.clarification_markers);
    assert!(flags.quality_checklists);
    assert!(flags.consistency_checks);
    assert!(flags.phase_minus_one_gates);
    assert!(flags.constitution);
    assert!(flags.feedback_loop);
}

#[test]
fn test_sdd_flags_all_disabled() {
    let flags = SddFlags::all_disabled();
    assert!(!flags.clarification_markers);
    assert!(!flags.quality_checklists);
    assert!(!flags.consistency_checks);
    assert!(!flags.phase_minus_one_gates);
    assert!(!flags.constitution);
    assert!(!flags.feedback_loop);
}

#[test]
fn test_sdd_flags_default_is_all_disabled() {
    let flags = SddFlags::default();
    assert_eq!(flags, SddFlags::all_disabled());
}

#[test]
fn test_sdd_flags_from_bools() {
    let flags = SddFlags::from_bools(true, false, true, false, true, false);
    assert!(flags.clarification_markers);
    assert!(!flags.quality_checklists);
    assert!(flags.consistency_checks);
    assert!(!flags.phase_minus_one_gates);
    assert!(flags.constitution);
    assert!(!flags.feedback_loop);
}

/// Spec with a `[NEEDS CLARIFICATION]` marker — used by flag-gating tests.
fn spec_with_clarification_marker() -> Spec {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test Spec");
    spec.spec_md = r"---
status: draft
id: testspec
---

# Specification: Test Spec

## Executive Summary

A test spec.

## Scope & Objectives

### Scope

In scope.

## Functional Requirements

### FR-001 — Requirement One

`The system shall do X.`

[NEEDS CLARIFICATION: what scale?]

## Non-Functional Requirements

### NFR-001 — Performance

`The system shall respond within 2 seconds.`

## Constraints & Assumptions

### Constraints

1. Must use Rust.
"
    .to_string();
    spec.plan_md = r"# Implementation Plan: Test Spec

## Overview

A plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-001 | S | High |
"
    .to_string();
    spec
}

#[test]
fn test_validate_with_flags_all_disabled_skips_clarifications() {
    let spec = spec_with_clarification_marker();
    let flags = SddFlags::all_disabled();
    let report = validate_with_flags(&spec, &flags);
    assert!(
        !report.has_clarifications(),
        "all_disabled should skip clarification marker detection"
    );
    assert_eq!(report.clarification_count(), 0);
}

#[test]
fn test_validate_with_flags_clarification_enabled_includes_markers() {
    let spec = spec_with_clarification_marker();
    let flags = SddFlags {
        clarification_markers: true,
        ..SddFlags::all_disabled()
    };
    let report = validate_with_flags(&spec, &flags);
    assert!(
        report.has_clarifications(),
        "clarification_markers=true should detect markers"
    );
    assert_eq!(report.clarification_count(), 1);
}

#[test]
fn test_validate_backward_compat_includes_clarifications() {
    // The legacy validate() function should still run all checks.
    let spec = spec_with_clarification_marker();
    let report = validate(&spec);
    assert!(
        report.has_clarifications(),
        "validate() backward compat should include clarification markers"
    );
    assert_eq!(report.clarification_count(), 1);
}

/// Spec with a vague term — used by consistency-check flag-gating tests.
fn spec_with_vague_term() -> Spec {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test Spec");
    spec.spec_md = r"---
status: draft
id: testspec
---

# Specification: Test Spec

## Executive Summary

A test spec.

## Scope & Objectives

### Scope

In scope.

## Functional Requirements

### FR-001 — Performance

`The system shall be fast and scalable.`

## Non-Functional Requirements

### NFR-001 — Reliability

`The system shall handle errors gracefully.`

## Constraints & Assumptions

### Constraints

1. Must use Rust.
"
    .to_string();
    spec.plan_md = r"# Implementation Plan: Test Spec

## Overview

A plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-001 | S | High |
"
    .to_string();
    spec
}

#[test]
fn test_validate_with_flags_all_disabled_skips_consistency() {
    let spec = spec_with_vague_term();
    let flags = SddFlags::all_disabled();
    let report = validate_with_flags(&spec, &flags);
    let ambiguity_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.category == Category::Ambiguity)
        .collect();
    assert!(
        ambiguity_issues.is_empty(),
        "all_disabled should skip consistency (ambiguity) checks"
    );
}

#[test]
fn test_validate_with_flags_consistency_enabled_includes_ambiguity() {
    let spec = spec_with_vague_term();
    let flags = SddFlags {
        consistency_checks: true,
        ..SddFlags::all_disabled()
    };
    let report = validate_with_flags(&spec, &flags);
    let ambiguity_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.category == Category::Ambiguity)
        .collect();
    assert!(
        !ambiguity_issues.is_empty(),
        "consistency_checks=true should detect vague terms"
    );
    // Should detect "fast" and "scalable"
    assert!(ambiguity_issues.iter().any(|i| i.message.contains("fast")));
    assert!(
        ambiguity_issues
            .iter()
            .any(|i| i.message.contains("scalable"))
    );
}

#[test]
fn test_validate_backward_compat_includes_consistency() {
    let spec = spec_with_vague_term();
    let report = validate(&spec);
    let ambiguity_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.category == Category::Ambiguity)
        .collect();
    assert!(
        !ambiguity_issues.is_empty(),
        "validate() backward compat should include consistency checks"
    );
}

#[test]
fn test_validate_with_flags_core_checks_always_run() {
    // Even with all flags disabled, core checks (structure, EARS, plan)
    // should still run.
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test Spec");
    spec.spec_md = r"---
status: draft
id: testspec
---

# Specification: Test Spec

## Executive Summary

A test spec.

## Scope & Objectives

### Scope

In scope.

## Functional Requirements

### FR-001 — Requirement One

`This is not EARS syntax.`

## Non-Functional Requirements

### NFR-001 — Performance

`The system shall respond within 2 seconds.`

## Constraints & Assumptions

### Constraints

1. Must use Rust.
"
    .to_string();
    spec.plan_md = String::new(); // empty plan → should produce error

    let flags = SddFlags::all_disabled();
    let report = validate_with_flags(&spec, &flags);
    // Core checks should still produce errors
    assert!(
        report.has_errors(),
        "core checks should run even with all SDD flags disabled"
    );
    // Should have EARS syntax error and plan error
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.category == Category::EarsSyntax)
    );
    assert!(report.issues.iter().any(|i| i.category == Category::Plan));
}

#[test]
fn test_validate_with_flags_both_sdd_checks_enabled() {
    let spec = spec_with_clarification_marker();
    let flags = SddFlags {
        clarification_markers: true,
        consistency_checks: true,
        ..SddFlags::all_disabled()
    };
    let report = validate_with_flags(&spec, &flags);
    assert!(report.has_clarifications());
    // The "do X" requirement may trigger gap detection (no measurable criterion)
    // but that's OK — we just verify both categories can coexist.
}

#[test]
fn test_validate_with_flags_selective_enable() {
    // Only quality_checklists enabled — should not affect validation
    // (quality_checklists gates template generation, not validation).
    let spec = spec_with_clarification_marker();
    let flags = SddFlags {
        quality_checklists: true,
        ..SddFlags::all_disabled()
    };
    let report = validate_with_flags(&spec, &flags);
    assert!(
        !report.has_clarifications(),
        "quality_checklists flag should not gate validation checks"
    );
}

// ── Phase -1 Gate validation tests (FR-008, T-016) ───────────────────────

/// Spec helper with a PLAN.md containing all required Phase -1 gates checked.
fn spec_with_all_gates_checked() -> Spec {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test Spec");
    spec.spec_md = r"---
status: draft
id: testspec
---

# Specification: Test Spec

## Executive Summary

A test spec.

## Scope & Objectives

### Scope

In scope.

## Functional Requirements

### FR-001 — Requirement One

`The system shall do X.`

## Non-Functional Requirements

### NFR-001 — Performance

`The system shall respond within 2 seconds.`

## Constraints & Assumptions

### Constraints

1. Must use Rust.
"
    .to_string();
    spec.plan_md = r"# Implementation Plan: Test Spec

## Overview

A plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-001 | S | High |

## Phase -1 Gates

- [x] Simplicity
- [x] Anti-Abstraction
- [x] Integration-First
"
    .to_string();
    spec
}

/// Spec helper with a PLAN.md containing some unchecked Phase -1 gates.
fn spec_with_unchecked_gates() -> Spec {
    let mut spec = spec_with_all_gates_checked();
    spec.plan_md = r"# Implementation Plan: Test Spec

## Overview

A plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-001 | S | High |

## Phase -1 Gates

- [x] Simplicity
- [ ] Anti-Abstraction
- [ ] Integration-First
"
    .to_string();
    spec
}

/// Spec helper with a PLAN.md that has no Phase -1 Gates section at all.
fn spec_with_no_gates_section() -> Spec {
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test Spec");
    spec.spec_md = r"---
status: draft
id: testspec
---

# Specification: Test Spec

## Executive Summary

A test spec.

## Scope & Objectives

### Scope

In scope.

## Functional Requirements

### FR-001 — Requirement One

`The system shall do X.`

## Non-Functional Requirements

### NFR-001 — Performance

`The system shall respond within 2 seconds.`

## Constraints & Assumptions

### Constraints

1. Must use Rust.
"
    .to_string();
    spec.plan_md = r"# Implementation Plan: Test Spec

## Overview

A plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-001 | S | High |
"
    .to_string();
    spec
}

#[test]
fn test_validate_phase_gates_all_checked_no_issues() {
    let spec = spec_with_all_gates_checked();
    let flags = SddFlags {
        phase_minus_one_gates: true,
        ..SddFlags::all_disabled()
    };
    let report = validate_with_flags(&spec, &flags);
    assert!(
        !report.has_phase_gate_issues(),
        "all gates checked should produce no Phase -1 gate issues: {report:?}"
    );
}

#[test]
fn test_validate_phase_gates_unchecked_produces_warnings() {
    let spec = spec_with_unchecked_gates();
    let flags = SddFlags {
        phase_minus_one_gates: true,
        ..SddFlags::all_disabled()
    };
    let report = validate_with_flags(&spec, &flags);
    assert!(
        report.has_phase_gate_issues(),
        "unchecked gates should produce Phase -1 gate issues: {report:?}"
    );
    // Should have 2 warnings (Anti-Abstraction and Integration-First)
    assert_eq!(report.phase_gate_issue_count(), 2);
    // All should be warnings, not errors
    let gate_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.category == Category::PhaseMinusOneGate)
        .collect();
    assert!(gate_issues.iter().all(|i| i.severity == Severity::Warning));
    // Should mention the unchecked gate names
    assert!(
        gate_issues
            .iter()
            .any(|i| i.message.contains("Anti-Abstraction"))
    );
    assert!(
        gate_issues
            .iter()
            .any(|i| i.message.contains("Integration-First"))
    );
}

#[test]
fn test_validate_phase_gates_missing_section_produces_warning() {
    let spec = spec_with_no_gates_section();
    let flags = SddFlags {
        phase_minus_one_gates: true,
        ..SddFlags::all_disabled()
    };
    let report = validate_with_flags(&spec, &flags);
    assert!(
        report.has_phase_gate_issues(),
        "missing Phase -1 Gates section should produce an issue: {report:?}"
    );
    assert_eq!(report.phase_gate_issue_count(), 1);
    let gate_issues: Vec<_> = report
        .issues
        .iter()
        .filter(|i| i.category == Category::PhaseMinusOneGate)
        .collect();
    assert!(gate_issues[0].message.contains("missing"));
    assert!(gate_issues[0].message.contains("Phase -1 Gates"));
}

#[test]
fn test_validate_phase_gates_disabled_skips_check() {
    let spec = spec_with_unchecked_gates();
    let flags = SddFlags::all_disabled();
    let report = validate_with_flags(&spec, &flags);
    assert!(
        !report.has_phase_gate_issues(),
        "phase_minus_one_gates=false should skip the check entirely"
    );
}

#[test]
fn test_validate_phase_gates_backward_compat_all_enabled() {
    // validate() uses all_enabled() — should include Phase -1 gate checks
    let spec = spec_with_unchecked_gates();
    let report = validate(&spec);
    assert!(
        report.has_phase_gate_issues(),
        "validate() backward compat should include Phase -1 gate checks"
    );
}

#[test]
fn test_validate_phase_gates_empty_plan_skipped() {
    // When plan_md is empty, validate_plan already reports the error;
    // validate_phase_minus_one_gates should skip silently.
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test Spec");
    spec.spec_md = r"---
status: draft
id: testspec
---

# Specification: Test Spec

## Executive Summary

A test spec.

## Scope & Objectives

### Scope

In scope.

## Functional Requirements

### FR-001 — Requirement One

`The system shall do X.`

## Non-Functional Requirements

### NFR-001 — Performance

`The system shall respond within 2 seconds.`

## Constraints & Assumptions

### Constraints

1. Must use Rust.
"
    .to_string();
    spec.plan_md = String::new();

    let flags = SddFlags {
        phase_minus_one_gates: true,
        ..SddFlags::all_disabled()
    };
    let report = validate_with_flags(&spec, &flags);
    // Should have plan error from validate_plan, but NO Phase -1 gate issues
    assert!(
        !report.has_phase_gate_issues(),
        "empty plan should not produce Phase -1 gate issues"
    );
    assert!(
        report.has_errors(),
        "empty plan should produce a plan error"
    );
}

#[test]
fn test_validate_phase_gates_all_three_checked_explicitly() {
    // Test with explicit [x] for all three — verify zero gate issues
    let spec = spec_with_all_gates_checked();
    let report = validate_phase_minus_one_gates_standalone(&spec);
    assert!(
        report.issues.is_empty(),
        "all checked should yield no issues"
    );
}

#[test]
fn test_validate_phase_gates_only_one_checked() {
    let mut spec = spec_with_all_gates_checked();
    spec.plan_md = r"# Implementation Plan: Test Spec

## Overview

A plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-001 | S | High |

## Phase -1 Gates

- [x] Simplicity
- [ ] Anti-Abstraction
- [ ] Integration-First
"
    .to_string();
    let report = validate_phase_minus_one_gates_standalone(&spec);
    assert_eq!(report.issues.len(), 2);
    assert!(
        report
            .issues
            .iter()
            .all(|i| i.severity == Severity::Warning)
    );
}

/// Standalone helper to call validate_phase_minus_one_gates directly.
fn validate_phase_minus_one_gates_standalone(spec: &Spec) -> Report {
    let mut report = Report::new();
    validate_phase_minus_one_gates(spec, &mut report);
    report
}
// ── Consistency-check Report tests (T-029, FR-015) ───────────────────────

#[test]
fn test_report_has_consistency_issues() {
    let mut report = Report::new();
    assert!(!report.has_consistency_issues());
    report.add(Issue::new(
        Severity::Warning,
        Category::Ambiguity,
        "vague term: fast",
    ));
    assert!(report.has_consistency_issues());
    assert_eq!(report.consistency_issue_count(), 1);
}

#[test]
fn test_report_consistency_count_includes_all_three() {
    let mut report = Report::new();
    report.add(Issue::new(
        Severity::Warning,
        Category::Ambiguity,
        "vague term",
    ));
    report.add(Issue::new(
        Severity::Warning,
        Category::Contradiction,
        "negation conflict",
    ));
    report.add(Issue::new(
        Severity::Warning,
        Category::Gap,
        "no measurable criterion",
    ));
    assert_eq!(report.consistency_issue_count(), 3);
    assert!(report.has_consistency_issues());
}

#[test]
fn test_report_consistency_count_excludes_other_categories() {
    let mut report = Report::new();
    report.add(Issue::new(
        Severity::Warning,
        Category::Clarification,
        "marker",
    ));
    report.add(Issue::new(
        Severity::Error,
        Category::EarsSyntax,
        "bad syntax",
    ));
    report.add(Issue::new(
        Severity::Warning,
        Category::Plan,
        "missing section",
    ));
    assert!(!report.has_consistency_issues());
    assert_eq!(report.consistency_issue_count(), 0);
}

#[test]
fn test_report_count_by_category() {
    let mut report = Report::new();
    report.add(Issue::new(Severity::Warning, Category::Ambiguity, "a"));
    report.add(Issue::new(Severity::Warning, Category::Ambiguity, "b"));
    report.add(Issue::new(Severity::Warning, Category::Contradiction, "c"));
    assert_eq!(report.count_by_category(Category::Ambiguity), 2);
    assert_eq!(report.count_by_category(Category::Contradiction), 1);
    assert_eq!(report.count_by_category(Category::Gap), 0);
}

#[test]
fn test_report_format_includes_consistency_summary() {
    let mut report = Report::new();
    report.requirement_count = 2;
    report.valid_ears_count = 2;
    report.add(Issue::new(
        Severity::Warning,
        Category::Ambiguity,
        "vague term: fast",
    ));
    report.add(Issue::new(
        Severity::Warning,
        Category::Contradiction,
        "negation conflict between FR-001 and FR-002",
    ));
    report.add(Issue::new(
        Severity::Warning,
        Category::Gap,
        "no measurable criterion for FR-003",
    ));
    let formatted = report.format("my-spec");
    assert!(
        formatted.contains("Consistency issues: 3"),
        "format should include consistency summary: {formatted}"
    );
    assert!(
        formatted.contains("ambiguity: 1"),
        "format should break down ambiguity count: {formatted}"
    );
    assert!(
        formatted.contains("contradictions: 1"),
        "format should break down contradiction count: {formatted}"
    );
    assert!(
        formatted.contains("gaps: 1"),
        "format should break down gap count: {formatted}"
    );
}

#[test]
fn test_report_format_no_consistency_summary_when_none() {
    let mut report = Report::new();
    report.requirement_count = 1;
    report.valid_ears_count = 1;
    report.add(Issue::new(
        Severity::Error,
        Category::EarsSyntax,
        "bad syntax",
    ));
    let formatted = report.format("my-spec");
    assert!(
        !formatted.contains("Consistency issues"),
        "format should NOT include consistency summary when none: {formatted}"
    );
}

#[test]
fn test_validate_with_consistency_enabled_reports_all_three_types() {
    // Spec with vague terms AND a contradiction AND a gap
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test Spec");
    spec.spec_md = r"---
status: draft
id: testspec
---

# Specification: Test Spec

## Executive Summary

A test spec.

## Scope & Objectives

### Scope

In scope.

## Functional Requirements

### FR-001 — Store Data

`The system shall store all user data.`

### FR-002 — No Storage

`The system shall not store all user data.`

### FR-003 — Quality

`The system shall be robust.`

## Non-Functional Requirements

### NFR-001 — Performance

`The system shall respond within 2 seconds.`

## Constraints & Assumptions

### Constraints

1. Must use Rust.
"
    .to_string();
    spec.plan_md = r"# Implementation Plan: Test Spec

## Overview

A plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-001 | S | High |
"
    .to_string();

    let flags = SddFlags {
        consistency_checks: true,
        ..SddFlags::all_disabled()
    };
    let report = validate_with_flags(&spec, &flags);

    // Should have ambiguity (vague term "robust" → VagueOutcome gap)
    // Should have contradiction (FR-001 vs FR-002 — negation conflict)
    // Should have gap (FR-003 "shall be robust" → VagueOutcome)
    assert!(
        report.has_consistency_issues(),
        "should have consistency issues: {report:?}"
    );

    // Check format includes the summary
    let formatted = report.format("testspec");
    assert!(
        formatted.contains("Consistency issues"),
        "format should include consistency summary: {formatted}"
    );
}

#[test]
fn test_validate_backward_compat_includes_consistency_in_format() {
    // validate() uses all_enabled() — consistency checks should run
    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "Test Spec");
    spec.spec_md = r"---
status: draft
id: testspec
---

# Specification: Test Spec

## Executive Summary

A test spec.

## Scope & Objectives

### Scope

In scope.

## Functional Requirements

### FR-001 — Quality

`The system shall be robust.`

## Non-Functional Requirements

### NFR-001 — Performance

`The system shall respond within 2 seconds.`

## Constraints & Assumptions

### Constraints

1. Must use Rust.
"
    .to_string();
    spec.plan_md = r"# Implementation Plan: Test Spec

## Overview

A plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-001 | S | High |
"
    .to_string();

    let report = validate(&spec);
    let formatted = report.format("testspec");
    // "robust" should trigger VagueOutcome gap detection
    assert!(
        report.has_consistency_issues(),
        "validate() backward compat should include consistency checks"
    );
    assert!(
        formatted.contains("Consistency issues"),
        "format should include consistency summary: {formatted}"
    );
}

#[test]
fn test_consistency_summary_with_only_ambiguity() {
    let mut report = Report::new();
    report.requirement_count = 1;
    report.valid_ears_count = 1;
    report.add(Issue::new(
        Severity::Warning,
        Category::Ambiguity,
        "vague term",
    ));
    let formatted = report.format("my-spec");
    assert!(
        formatted.contains("Consistency issues: 1"),
        "should show count 1: {formatted}"
    );
    assert!(
        formatted.contains("ambiguity: 1"),
        "should show ambiguity: 1: {formatted}"
    );
    assert!(
        formatted.contains("contradictions: 0"),
        "should show contradictions: 0: {formatted}"
    );
    assert!(
        formatted.contains("gaps: 0"),
        "should show gaps: 0: {formatted}"
    );
}
