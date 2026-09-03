#![allow(clippy::assert_is_empty)]
//! Tests for spec validation: sections, requirements, EARS templates, and reports.

use ragent_specs::spec::{EarsTemplate, Spec, SpecId};
use ragent_specs::validate::{
    Category, Issue, Report, Severity, detect_ears_template, extract_sections, parse_requirements,
    validate,
};
use std::collections::HashMap;

#[test]
fn test_validate_real_project_spec() {
    let id = SpecId::new("testspec").unwrap();
    let spec_md = include_str!("fixtures/testspec/SPEC.md");
    let plan_md = include_str!("fixtures/testspec/PLAN.md");

    let mut spec = Spec::new(id, "testspec");
    spec.spec_md = spec_md.to_string();
    spec.plan_md = plan_md.to_string();

    let report = validate(&spec);

    println!("{}", report.format("testspec"));

    // The real project's spec should have no structural errors
    assert!(
        !report.has_errors(),
        "Project spec should have no validation errors:\n{}",
        report.format("testspec")
    );
}

#[test]
fn test_parse_requirements_from_real_spec() {
    let spec_md = include_str!("fixtures/testspec/SPEC.md");
    let reqs = parse_requirements(spec_md);

    // Real spec has 15 FRs and 7 NFRs
    assert!(
        reqs.len() >= 10,
        "Expected at least 10 requirements, got {}",
        reqs.len()
    );

    // Check FR-001 exists
    let fr001 = reqs.iter().find(|r| r.id == "FR-001");
    assert!(fr001.is_some(), "FR-001 should be found");

    // Check EARS text is extracted
    let fr001 = fr001.unwrap();
    assert!(
        fr001.ears_text.contains("shall"),
        "FR-001 should contain 'shall': {}",
        fr001.ears_text
    );
}

#[test]
fn test_detect_all_ears_templates_in_real_spec() {
    let spec_md = include_str!("fixtures/testspec/SPEC.md");
    let reqs = parse_requirements(spec_md);

    let mut template_counts: HashMap<EarsTemplate, usize> = HashMap::new();
    let mut unrecognised = Vec::new();

    for req in &reqs {
        if req.ears_text.is_empty() {
            continue;
        }
        match detect_ears_template(&req.ears_text) {
            Some(t) => {
                *template_counts.entry(t).or_insert(0) += 1;
            }
            None => {
                unrecognised.push((req.id.clone(), req.ears_text.clone()));
            }
        }
    }

    println!("Template counts: {template_counts:?}");
    println!("Unrecognised: {unrecognised:?}");

    // Most real requirements should match a template
    let total_with_ears = reqs.iter().filter(|r| !r.ears_text.is_empty()).count();
    let recognised: usize = template_counts.values().sum();
    let recognition_rate = (recognised as f64) / (total_with_ears as f64) * 100.0;

    println!("Recognition rate: {recognition_rate}%");

    // At least 70% recognition on the real spec (some edge cases are expected)
    assert!(
        recognition_rate >= 70.0,
        "Expected >=70% EARS recognition, got {recognition_rate}%\nUnrecognised: {unrecognised:?}"
    );
}

#[test]
fn test_extract_sections_from_real_spec() {
    let spec_md = include_str!("fixtures/testspec/SPEC.md");
    let sections = extract_sections(spec_md);

    let names: Vec<String> = sections.iter().map(|(_, n, _)| n.clone()).collect();

    assert!(
        names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Executive Summary")),
        "Should have Executive Summary"
    );
    assert!(
        names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Scope & Objectives")),
        "Should have Scope & Objectives"
    );
    assert!(
        names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Functional Requirements")),
        "Should have Functional Requirements"
    );
    assert!(
        names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Non-Functional Requirements")),
        "Should have Non-Functional Requirements"
    );
    assert!(
        names
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Constraints & Assumptions")),
        "Should have Constraints & Assumptions"
    );
}

#[test]
fn test_validate_empty_spec() {
    let id = SpecId::new("empty").unwrap();
    let spec = Spec::new(id, "Empty Spec");

    let report = validate(&spec);
    assert!(report.has_errors());
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.message.contains("No requirements found"))
    );
    assert!(
        report
            .issues
            .iter()
            .any(|i| i.message.contains("PLAN.md is empty"))
    );
}

#[test]
fn test_report_methods() {
    let mut report = Report::new();
    report.add(Issue::new(Severity::Error, Category::EarsSyntax, "Error 1").with_line(5));
    report.add(Issue::new(Severity::Warning, Category::Plan, "Warn 1").with_line(10));
    report.add(Issue::new(Severity::Info, Category::Structure, "Info 1"));

    report.sort();

    assert_eq!(report.count_by_severity(Severity::Error), 1);
    assert_eq!(report.count_by_severity(Severity::Warning), 1);
    assert_eq!(report.count_by_severity(Severity::Info), 1);
    assert!(report.has_errors());
    assert!(report.has_warnings());

    let formatted = report.format("test");
    assert!(formatted.contains("Validation Report for `test`"));
    assert!(formatted.contains("1 error(s)"));
    assert!(formatted.contains("1 warning(s)"));
}
