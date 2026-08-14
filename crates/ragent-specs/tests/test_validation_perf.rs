//! Performance benchmark for NFR-001.
//!
//! Verifies that `validate_with_flags` with all SDD checks enabled completes
//! in under 500 ms for a spec containing 50 requirements.

use ragent_specs::spec::{Spec, SpecId};
use ragent_specs::validate::{SddFlags, validate_with_flags};
use std::time::Instant;

// ── Synthetic spec generator ───────────────────────────────────────────────

/// Generate a spec markdown string with `n_fr` functional requirements and
/// `n_nfr` non-functional requirements, each using valid EARS syntax.
///
/// A few requirements intentionally include vague terms (e.g. "fast",
/// "efficient") to exercise the ambiguity detector, and some include
/// `[NEEDS CLARIFICATION]` markers to exercise the clarification detector.
fn generate_spec_md(n_fr: usize, n_nfr: usize) -> String {
    let mut md = String::with_capacity(20_000);

    md.push_str("---\nstatus: draft\n---\n\n");
    md.push_str("# Benchmark Spec\n\n");
    md.push_str("## Executive Summary\n\n");
    md.push_str("A synthetic spec with 50 requirements for performance benchmarking.\n\n");
    md.push_str("## Scope & Objectives\n\n");
    md.push_str("Verify that validation completes within the NFR-001 performance budget.\n\n");

    // Functional Requirements
    md.push_str("## Functional Requirements\n\n");
    for i in 1..=n_fr {
        let id = format!("FR-{i:03}");
        // Every 7th requirement uses a vague term to trigger ambiguity detection.
        let ears = if i % 7 == 0 {
            "`The system shall process requests in a fast and efficient manner. [NEEDS CLARIFICATION]`"
                .to_string()
        } else if i % 11 == 0 {
            format!("`The system shall provide a user-friendly interface for feature {i}.`")
        } else {
            format!("`The system shall validate input parameter {i} before processing.`")
        };
        md.push_str(&format!("### {id} — Requirement {i}\n\n{ears}\n\n"));
    }

    // Non-Functional Requirements
    md.push_str("## Non-Functional Requirements\n\n");
    for i in 1..=n_nfr {
        let id = format!("NFR-{i:03}");
        let ears = if i % 5 == 0 {
            "`The system shall be scalable and robust under load.`".to_string()
        } else {
            {
                let ms = i * 10;
                format!("`The system shall respond to request {i} within {ms} milliseconds.`")
            }
        };
        md.push_str(&format!(
            "### {id} — Non-Functional Requirement {i}\n\n{ears}\n\n"
        ));
    }

    md.push_str("## Constraints & Assumptions\n\n");
    md.push_str("No external dependencies are required for benchmarking.\n");

    md
}

/// Generate a plan markdown with a task table and Phase -1 Gates section.
fn generate_plan_md(n_tasks: usize) -> String {
    let mut md = String::with_capacity(8_000);

    md.push_str("# Plan\n\n");
    md.push_str("## Tasks\n\n");
    md.push_str("| ID | Description | Status |\n");
    md.push_str("|----|-------------|--------|\n");
    for i in 1..=n_tasks {
        md.push_str(&format!(
            "| T-{:03} | Implement requirement {} | pending |\n",
            i, i
        ));
    }

    // Phase -1 Gates section (all checked)
    md.push_str("\n## Phase -1 Gates\n\n");
    md.push_str("- [x] Simplicity — The design uses the minimum code necessary.\n");
    md.push_str("- [x] Anti-Abstraction — No speculative abstractions are introduced.\n");
    md.push_str("- [x] Integration-First — The feature integrates with existing systems.\n");

    md
}

fn build_benchmark_spec() -> Spec {
    let id = SpecId::new("bench").unwrap();
    let mut spec = Spec::new(id, "Benchmark Spec");
    spec.spec_md = generate_spec_md(40, 10); // 50 total requirements
    spec.plan_md = generate_plan_md(50);
    spec
}

// ── Benchmark tests ────────────────────────────────────────────────────────

#[test]
fn test_validate_50_requirements_under_500ms_all_enabled() {
    let spec = build_benchmark_spec();

    // Warm-up call — ensures any lazy initialization is accounted for.
    let _ = validate_with_flags(&spec, &SddFlags::all_enabled());

    // Measured run with all SDD checks enabled (clarification, consistency,
    // phase-minus-one gates).
    let start = Instant::now();
    let report = validate_with_flags(&spec, &SddFlags::all_enabled());
    let elapsed = start.elapsed();

    let ms = elapsed.as_millis();
    let req_count = 50;
    let issue_count = report.issues.len();

    println!(
        "NFR-001 benchmark: {req_count} requirements, {issue_count} issues, \
         {ms} ms (budget: 500 ms)"
    );

    assert!(
        elapsed.as_millis() < 500,
        "Validation of 50-requirement spec took {ms} ms, exceeding 500 ms budget (NFR-001)"
    );
}

#[test]
fn test_validate_50_requirements_under_500ms_all_disabled() {
    // Also verify that the core-only path (SDD disabled) is well within budget.
    let spec = build_benchmark_spec();

    let _ = validate_with_flags(&spec, &SddFlags::all_disabled());

    let start = Instant::now();
    let _report = validate_with_flags(&spec, &SddFlags::all_disabled());
    let elapsed = start.elapsed();

    let ms = elapsed.as_millis();
    println!("NFR-001 benchmark (core only): 50 requirements, {ms} ms (budget: 500 ms)");

    assert!(
        elapsed.as_millis() < 500,
        "Core-only validation of 50-requirement spec took {ms} ms, exceeding 500 ms budget"
    );
}

#[test]
fn test_validate_50_requirements_produces_results() {
    // Sanity check: the benchmark spec should actually trigger the SDD
    // checks and produce issues (clarifications, ambiguity). This ensures
    // the benchmark is exercising the real code paths, not just no-ops.
    let spec = build_benchmark_spec();
    let report = validate_with_flags(&spec, &SddFlags::all_enabled());

    // Should have at least some issues from ambiguity/clarification checks.
    assert!(
        !report.issues.is_empty(),
        "Benchmark spec should produce validation issues from SDD checks, got 0"
    );

    // Verify requirement count was parsed correctly.
    assert_eq!(
        report.requirement_count, 50,
        "Should parse exactly 50 requirements"
    );
}
