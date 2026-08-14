//! Backward-compatibility tests for FR-018.
//!
//! Verifies that existing spec directories containing only `SPEC.md` and
//! `PLAN.md` (no `CONSTITUTION.md`, `TASKS.md`, `data-model.md`,
//! `contracts/`, `quickstart.md`, or `FEEDBACK.md`) continue to validate,
//! list, search, and transition without modification.

use ragent_specs::io::SpecIo;
use ragent_specs::manager::SpecManager;
use ragent_specs::spec::{Spec, SpecId, SpecStatus};
use ragent_specs::validate::{Category, SddFlags, validate, validate_with_flags};

// ── Helper: create a minimal legacy spec ───────────────────────────────────

/// Markdown for a minimal legacy spec that predates the SDD back-fill.
/// Contains only the sections required by the core validator.
const LEGACY_SPEC_MD: &str = "\
---\nstatus: draft\n---\n\n\
# Legacy Feature\n\n\
## Executive Summary\n\n\
A legacy spec for backward-compatibility testing.\n\n\
## Scope & Objectives\n\n\
Verify that existing specs work without new artifacts.\n\n\
## Functional Requirements\n\n\
### FR-001 — Core Feature\n\n\
`The system shall provide a core feature.`\n\n\
## Non-Functional Requirements\n\n\
### NFR-001 — Performance\n\n\
`The system shall respond within 500 milliseconds.`\n\n\
## Constraints & Assumptions\n\n\
No new artifacts are required.\n";

/// Markdown for a minimal legacy plan.
const LEGACY_PLAN_MD: &str = "\
# Plan\n\n\
## Tasks\n\n\
| ID | Description | Status |\n\
|----|-------------|--------|\n\
| T-001 | Implement core feature | pending |\n";

/// Create a minimal legacy spec directory (only SPEC.md + PLAN.md).
async fn create_legacy_spec(root: &std::path::Path, id: &str) -> std::path::PathBuf {
    let spec_id = SpecId::new(id).unwrap();
    let dir = SpecIo::create_spec_dir(root, &spec_id, LEGACY_SPEC_MD, LEGACY_PLAN_MD)
        .await
        .unwrap();
    // Verify no new artifacts exist — only SPEC.md and PLAN.md.
    assert!(dir.join("SPEC.md").is_file(), "SPEC.md must exist");
    assert!(dir.join("PLAN.md").is_file(), "PLAN.md must exist");
    assert!(
        !dir.join("CONSTITUTION.md").exists(),
        "CONSTITUTION.md should not exist in a legacy spec"
    );
    assert!(
        !dir.join("TASKS.md").exists(),
        "TASKS.md should not exist in a legacy spec"
    );
    assert!(
        !dir.join("data-model.md").exists(),
        "data-model.md should not exist in a legacy spec"
    );
    assert!(
        !dir.join("contracts").exists(),
        "contracts/ should not exist in a legacy spec"
    );
    assert!(
        !dir.join("quickstart.md").exists(),
        "quickstart.md should not exist in a legacy spec"
    );
    assert!(
        !dir.join("FEEDBACK.md").exists(),
        "FEEDBACK.md should not exist in a legacy spec"
    );
    assert!(
        !dir.join("TESTPLAN.md").exists(),
        "TESTPLAN.md should not exist in a legacy spec"
    );
    dir
}

// ── Tests: validate with SDD flags disabled ────────────────────────────────

#[test]
fn test_legacy_spec_validates_with_sdd_disabled_no_sdd_issues() {
    let id = SpecId::new("legacy").unwrap();
    let mut spec = Spec::new(id, "legacy");
    spec.spec_md = LEGACY_SPEC_MD.to_string();
    spec.plan_md = LEGACY_PLAN_MD.to_string();

    let report = validate_with_flags(&spec, &SddFlags::all_disabled());

    // Core checks should pass (no errors).
    assert!(
        !report.has_errors(),
        "Legacy spec should have no validation errors with SDD disabled:\n{}",
        report.format("legacy")
    );

    // No SDD-specific issues should be present.
    assert_eq!(
        report.count_by_category(Category::Clarification),
        0,
        "No clarification issues with SDD disabled"
    );
    assert_eq!(
        report.count_by_category(Category::Ambiguity),
        0,
        "No ambiguity issues with SDD disabled"
    );
    assert_eq!(
        report.count_by_category(Category::Contradiction),
        0,
        "No contradiction issues with SDD disabled"
    );
    assert_eq!(
        report.count_by_category(Category::Gap),
        0,
        "No gap issues with SDD disabled"
    );
    assert_eq!(
        report.count_by_category(Category::PhaseMinusOneGate),
        0,
        "No Phase -1 gate issues with SDD disabled"
    );
}

#[test]
fn test_legacy_spec_validates_with_sdd_enabled_no_errors() {
    // Even with all SDD flags enabled, a legacy spec should not produce
    // *errors* — only warnings for missing optional artifacts.
    let id = SpecId::new("legacy").unwrap();
    let mut spec = Spec::new(id, "legacy");
    spec.spec_md = LEGACY_SPEC_MD.to_string();
    spec.plan_md = LEGACY_PLAN_MD.to_string();

    let report = validate_with_flags(&spec, &SddFlags::all_enabled());

    // SDD checks may produce warnings (e.g., missing Phase -1 gates) but
    // must not produce errors.
    assert!(
        !report.has_errors(),
        "Legacy spec should have no errors even with SDD enabled:\n{}",
        report.format("legacy")
    );
}

#[test]
fn test_legacy_spec_legacy_validate_function_works() {
    // The original validate() function (all_enabled flags) must still work.
    let id = SpecId::new("legacy").unwrap();
    let mut spec = Spec::new(id, "legacy");
    spec.spec_md = LEGACY_SPEC_MD.to_string();
    spec.plan_md = LEGACY_PLAN_MD.to_string();

    let report = validate(&spec);
    assert!(
        !report.has_errors(),
        "validate() should not produce errors for a legacy spec:\n{}",
        report.format("legacy")
    );
}

// ── Tests: discover, list, read, search ────────────────────────────────────

#[tokio::test]
async fn test_legacy_spec_discover() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_legacy_spec(root, "legacy-discover").await;

    let mgr = SpecManager::new(root);
    let specs = mgr.discover_specs().await.unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].id.as_str(), "legacy-discover");
    assert_eq!(specs[0].title, "Legacy Feature");
}

#[tokio::test]
async fn test_legacy_spec_list() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_legacy_spec(root, "legacy-list").await;

    let mgr = SpecManager::new(root);
    let specs = mgr
        .list_specs(&ragent_specs::manager::SpecFilter::new())
        .await
        .unwrap();
    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0].id.as_str(), "legacy-list");
}

#[tokio::test]
async fn test_legacy_spec_read() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_legacy_spec(root, "legacy-read").await;

    let mgr = SpecManager::new(root);
    let spec = mgr
        .read_spec(&SpecId::new("legacy-read").unwrap())
        .await
        .unwrap();
    assert_eq!(spec.id.as_str(), "legacy-read");
    assert_eq!(spec.title, "Legacy Feature");
    assert_eq!(spec.status, SpecStatus::Draft);
    // feedback_md should default to empty (no FEEDBACK.md).
    assert!(spec.feedback_md.is_empty());
}

#[tokio::test]
async fn test_legacy_spec_search() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_legacy_spec(root, "legacy-search").await;

    let mgr = SpecManager::new(root);
    let results = mgr.search_specs("core feature").await.unwrap();
    assert!(!results.is_empty(), "Search should find the legacy spec");
    assert_eq!(results[0].spec.id.as_str(), "legacy-search");
}

// ── Tests: status transitions with SDD flags disabled ──────────────────────

#[tokio::test]
async fn test_legacy_spec_transition_to_in_progress_not_blocked() {
    // A legacy spec without Phase -1 gates should be able to transition
    // to InProgress when the phase_minus_one_gates flag is disabled.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_legacy_spec(root, "legacy-transition").await;

    let mgr = SpecManager::new(root);
    let mut spec = mgr
        .read_spec(&SpecId::new("legacy-transition").unwrap())
        .await
        .unwrap();

    // Transition: Draft → InReview → Approved → InProgress
    let flags = SddFlags::all_disabled();
    mgr.transition_with_flags(&mut spec, SpecStatus::InReview, "tester", &flags)
        .await
        .unwrap();
    assert_eq!(spec.status, SpecStatus::InReview);

    mgr.transition_with_flags(&mut spec, SpecStatus::Approved, "tester", &flags)
        .await
        .unwrap();
    assert_eq!(spec.status, SpecStatus::Approved);

    // This must succeed — no Phase -1 gates to block it.
    mgr.transition_with_flags(&mut spec, SpecStatus::InProgress, "tester", &flags)
        .await
        .unwrap();
    assert_eq!(spec.status, SpecStatus::InProgress);
}

#[tokio::test]
async fn test_legacy_spec_transition_to_approved_not_blocked() {
    // A legacy spec without clarification markers should be able to
    // transition to Approved even with clarification_markers enabled.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_legacy_spec(root, "legacy-approve").await;

    let mgr = SpecManager::new(root);
    let mut spec = mgr
        .read_spec(&SpecId::new("legacy-approve").unwrap())
        .await
        .unwrap();
    spec.status = SpecStatus::InReview;

    // Even with clarification_markers enabled, no markers = no block.
    let flags = SddFlags {
        clarification_markers: true,
        ..SddFlags::all_disabled()
    };
    mgr.transition_with_flags(&mut spec, SpecStatus::Approved, "tester", &flags)
        .await
        .unwrap();
    assert_eq!(spec.status, SpecStatus::Approved);
}

// ── Tests: real-world fixture ───────────────────────────────────────────────

#[test]
fn test_real_fixture_validates_with_sdd_disabled() {
    // The existing testspec fixture (SPEC.md + PLAN.md only) should
    // validate without errors when SDD flags are disabled.
    let spec_md = include_str!("fixtures/testspec/SPEC.md");
    let plan_md = include_str!("fixtures/testspec/PLAN.md");

    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "testspec");
    spec.spec_md = spec_md.to_string();
    spec.plan_md = plan_md.to_string();

    let report = validate_with_flags(&spec, &SddFlags::all_disabled());
    assert!(
        !report.has_errors(),
        "Existing testspec fixture should have no errors with SDD disabled:\n{}",
        report.format("testspec")
    );
}

#[test]
fn test_real_fixture_validates_with_sdd_enabled_no_errors() {
    // The existing testspec fixture should also not produce errors when
    // all SDD flags are enabled (warnings are acceptable).
    let spec_md = include_str!("fixtures/testspec/SPEC.md");
    let plan_md = include_str!("fixtures/testspec/PLAN.md");

    let id = SpecId::new("testspec").unwrap();
    let mut spec = Spec::new(id, "testspec");
    spec.spec_md = spec_md.to_string();
    spec.plan_md = plan_md.to_string();

    let report = validate_with_flags(&spec, &SddFlags::all_enabled());
    assert!(
        !report.has_errors(),
        "Existing testspec fixture should have no errors even with SDD enabled:\n{}",
        report.format("testspec")
    );
}

#[tokio::test]
async fn test_real_fixture_discover_and_read() {
    // The existing testspec fixture should be discoverable and readable.
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mgr = SpecManager::new(&root);
    let specs = mgr.discover_specs().await.unwrap();
    let testspec = specs
        .into_iter()
        .find(|s| s.id.as_str() == "testspec")
        .expect("testspec fixture must be discoverable");

    // The title is extracted from the first H1 heading in SPEC.md, which
    // for this fixture is "Specification: ragent Spec Management System".
    assert!(
        testspec.title.contains("Spec Management"),
        "testspec title should contain 'Spec Management', got: {}",
        testspec.title
    );
    // No new artifacts should be loaded (feedback_md is empty).
    assert!(testspec.feedback_md.is_empty());
}
