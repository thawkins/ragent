//! Tests for validate.rs (M8/T8.4).
//! Compiled as a submodule via #[path], super::* resolves to the source module.

    use super::*;
    use crate::spec::{Spec, SpecId};

    fn valid_spec() -> Spec {
        let id = SpecId::new("testspec").unwrap();
        let mut spec = Spec::new(id.clone(), "Test Spec");
        spec.spec_md = r#"---
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
"#
        .to_string();

        spec.plan_md = r#"# Implementation Plan: Test Spec

## Overview

A plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-001 | S | High |
| T-002 | Do Z | FR-002 | S | High |
"#
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
            detect_ears_template(
                "While the engine is running, the system shall monitor temperature."
            ),
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
            "valid spec should have no errors: {:?}",
            report
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
        spec.spec_md = r#"---
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
"#
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
        spec.spec_md = r#"---
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
"#
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
        spec.spec_md = r#"---
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
"#
        .to_string();
        spec.plan_md = "# Plan\n".to_string();
        let report = validate(&spec);
        assert!(report.issues.iter().any(|i| i.message.contains("Gap")));
    }

    #[test]
    fn test_validate_invalid_status() {
        let id = SpecId::new("testspec").unwrap();
        let mut spec = Spec::new(id, "Test");
        spec.spec_md = r#"---
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
"#
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
        spec.plan_md = r#"# Plan

## Overview

Plan.

## Milestones

1. M1

## Tasks

| ID | Title | Requirement | Effort | Priority |
|---|---|---|---|---|
| T-001 | Do X | FR-999 | S | High |
"#
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
        let md = r#"## Functional Requirements

### FR-001 — First
`The system shall do A.`

### FR-002 — Second
`When B, the system shall do C.`

### NFR-001 — Perf
`The system shall respond fast.`
"#;
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

