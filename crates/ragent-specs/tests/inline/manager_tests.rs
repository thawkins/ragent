//! Inline tests for `manager.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_allowed_transitions() {
    assert!(is_valid_transition(SpecStatus::Draft, SpecStatus::InReview));
    assert!(is_valid_transition(
        SpecStatus::InReview,
        SpecStatus::Approved
    ));
    assert!(is_valid_transition(SpecStatus::InReview, SpecStatus::Draft));
    assert!(is_valid_transition(
        SpecStatus::Approved,
        SpecStatus::InProgress
    ));
    assert!(is_valid_transition(
        SpecStatus::InProgress,
        SpecStatus::Implemented
    ));
    assert!(is_valid_transition(
        SpecStatus::Implemented,
        SpecStatus::Verified
    ));
    assert!(is_valid_transition(
        SpecStatus::Verified,
        SpecStatus::Archived
    ));
    assert!(is_valid_transition(SpecStatus::Archived, SpecStatus::Draft));
}

#[test]
fn test_invalid_transitions() {
    // Same status
    assert!(!is_valid_transition(SpecStatus::Draft, SpecStatus::Draft));
    // Skip ahead
    assert!(!is_valid_transition(
        SpecStatus::Draft,
        SpecStatus::Approved
    ));
    assert!(!is_valid_transition(
        SpecStatus::Draft,
        SpecStatus::Implemented
    ));
    // Backwards
    assert!(!is_valid_transition(
        SpecStatus::Approved,
        SpecStatus::Draft
    ));
    assert!(!is_valid_transition(
        SpecStatus::Implemented,
        SpecStatus::InProgress
    ));
}

#[test]
fn test_next_statuses() {
    let next = next_statuses(SpecStatus::Draft);
    assert_eq!(next, vec![SpecStatus::InReview]);

    let next = next_statuses(SpecStatus::InReview);
    assert_eq!(next, vec![SpecStatus::Draft, SpecStatus::Approved]);
}

#[test]
fn test_update_frontmatter() {
    let content = "# Title\n\nBody.\n";
    let audit = vec![(
        1_700_000_000,
        "draft".to_string(),
        "in_review".to_string(),
        "alice".to_string(),
    )];
    let updated = update_frontmatter(content, SpecStatus::InReview, &audit, &[]).unwrap();
    assert!(updated.starts_with("---\n"));
    assert!(updated.contains("status: in_review"));
    assert!(updated.contains("audit:"));
    assert!(updated.contains("alice"));
    assert!(updated.contains("# Title"));
}

#[test]
fn test_update_frontmatter_preserves_existing_body() {
    let content = "# Spec\n\n## Section\n\nText.\n";
    let updated = update_frontmatter(content, SpecStatus::Approved, &[], &[]).unwrap();
    assert!(updated.contains("## Section"));
    assert!(updated.contains("Text."));
}

#[test]
fn test_update_frontmatter_replaces_old_frontmatter() {
    let content = "---\nstatus: draft\n---\n\n# Title\n";
    let updated = update_frontmatter(content, SpecStatus::Approved, &[], &[]).unwrap();
    // Should only have one frontmatter block
    let count = updated.matches("---").count();
    assert_eq!(count, 2, "should have exactly 2 --- markers");
    assert!(updated.contains("status: approved"));
}

#[test]
fn test_extract_snippets_multibyte_boundary() {
    let text = "Before - the quick brown fox jumps - after";
    let snippets = extract_snippets_lowered(text, &text.to_lowercase(), "fox", 1);
    assert_eq!(snippets.len(), 1);
    assert!(snippets[0].contains("fox"));
}

#[test]
fn test_spec_filter_builder() {
    let f = SpecFilter::new()
        .with_status(SpecStatus::Draft)
        .with_id_prefix("test")
        .with_archived()
        .with_sort(SortBy::Id);
    assert_eq!(f.status, Some(SpecStatus::Draft));
    assert_eq!(f.id_prefix, Some("test".to_string()));
    assert!(f.include_archived);
    assert_eq!(f.sort_by, SortBy::Id);
}

#[test]
fn test_spec_filter_defaults() {
    let f = SpecFilter::new();
    assert!(f.status.is_none());
    assert!(f.id_prefix.is_none());
    assert!(!f.include_archived);
    assert_eq!(f.sort_by, SortBy::ModifiedAt);
}

#[test]
fn test_manager_new() {
    let mgr = SpecManager::new("/tmp/specs");
    assert_eq!(mgr.root(), Path::new("/tmp/specs"));
}

#[tokio::test]
async fn test_manager_discover_and_list() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // Create two specs
    let id1 = SpecId::new("alpha").unwrap();
    let id2 = SpecId::new("beta").unwrap();
    SpecIo::create_spec_dir(root, &id1, "# Alpha\n", "# Plan Alpha\n")
        .await
        .unwrap();
    SpecIo::create_spec_dir(root, &id2, "# Beta\n", "# Plan Beta\n")
        .await
        .unwrap();

    let mgr = SpecManager::new(root);
    let specs = mgr.discover_specs().await.unwrap();
    assert_eq!(specs.len(), 2);

    // List all (no filter)
    let list = mgr.list_specs(&SpecFilter::new()).await.unwrap();
    assert_eq!(list.len(), 2);

    // Filter by prefix
    let filtered = mgr
        .list_specs(&SpecFilter::new().with_id_prefix("alp"))
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id.as_str(), "alpha");
}

#[tokio::test]
async fn test_manager_search() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    let id = SpecId::new("search-test").unwrap();
    let spec_md = "# Search Test\n\nThis spec is about **frogs**.\n";
    let plan_md = "# Plan\n\nWe will study **frogs** in detail.\n";
    SpecIo::create_spec_dir(root, &id, spec_md, plan_md)
        .await
        .unwrap();

    let mgr = SpecManager::new(root);
    let results = mgr.search_specs("frogs").await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].spec.id.as_str(), "search-test");
    assert_eq!(results[0].score, 3); // spec(2) + plan(1)
    assert!(
        !results[0].snippets.is_empty(),
        "first result should have snippets"
    );
}

#[tokio::test]
async fn test_manager_transition() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    let id = SpecId::new("transition-test").unwrap();
    let spec_md = "---\nstatus: draft\n---\n\n# Transition Test\n";
    SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
        .await
        .unwrap();

    let mgr = SpecManager::new(root);
    let mut spec = mgr.read_spec(&id).await.unwrap();
    assert_eq!(spec.status, SpecStatus::Draft);

    mgr.transition(&mut spec, SpecStatus::InReview, "alice")
        .await
        .unwrap();
    assert_eq!(spec.status, SpecStatus::InReview);
    assert_eq!(spec.audit_trail.len(), 2);

    // Re-read from disk and verify frontmatter updated
    let spec2 = mgr.read_spec(&id).await.unwrap();
    assert_eq!(spec2.status, SpecStatus::InReview);
    assert!(spec2.spec_md.contains("status: in_review"));
    assert!(spec2.spec_md.contains("audit:"));
}

#[tokio::test]
async fn test_manager_invalid_transition() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    let id = SpecId::new("invalid-trans").unwrap();
    SpecIo::create_spec_dir(root, &id, "# Invalid\n", "# Plan\n")
        .await
        .unwrap();

    let mgr = SpecManager::new(root);
    let mut spec = mgr.read_spec(&id).await.unwrap();
    let result = mgr
        .transition(&mut spec, SpecStatus::Implemented, "bob")
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_transition_approved_blocked_by_clarifications() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    let id = SpecId::new("clar-block").unwrap();
    let spec_md =
        "---\nstatus: in_review\n---\n\n# Clar Block\n\n[NEEDS CLARIFICATION: what scale?]\n";
    SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
        .await
        .unwrap();

    let mgr = SpecManager::new(root);
    let mut spec = mgr.read_spec(&id).await.unwrap();
    // Force in_review so transition to approved is graph-valid
    spec.status = SpecStatus::InReview;

    let result = mgr
        .transition(&mut spec, SpecStatus::Approved, "alice")
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, SpecError::UnresolvedClarifications { count } if count == 1),
        "expected UnresolvedClarifications error, got {err:?}"
    );
}

#[tokio::test]
async fn test_transition_approved_allowed_without_clarifications() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    let id = SpecId::new("clar-ok").unwrap();
    let spec_md = "---\nstatus: in_review\n---\n\n# Clar OK\n\nNo markers here.\n";
    SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
        .await
        .unwrap();

    let mgr = SpecManager::new(root);
    let mut spec = mgr.read_spec(&id).await.unwrap();
    spec.status = SpecStatus::InReview;

    mgr.transition(&mut spec, SpecStatus::Approved, "alice")
        .await
        .unwrap();
    assert_eq!(spec.status, SpecStatus::Approved);
}

#[tokio::test]
async fn test_transition_non_approved_ignores_clarifications() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    let id = SpecId::new("clar-non-approved").unwrap();
    let spec_md =
        "---\nstatus: draft\n---\n\n# Non Approved\n\n[NEEDS CLARIFICATION: something?]\n";
    SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
        .await
        .unwrap();

    let mgr = SpecManager::new(root);
    let mut spec = mgr.read_spec(&id).await.unwrap();
    // Draft -> InReview should succeed even with clarification markers
    mgr.transition(&mut spec, SpecStatus::InReview, "alice")
        .await
        .unwrap();
    assert_eq!(spec.status, SpecStatus::InReview);
}

#[tokio::test]
async fn test_manager_list_sorting() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    let id1 = SpecId::new("zeta").unwrap();
    let id2 = SpecId::new("alpha").unwrap();
    SpecIo::create_spec_dir(root, &id1, "# Zeta\n", "# Plan\n")
        .await
        .unwrap();
    SpecIo::create_spec_dir(root, &id2, "# Alpha\n", "# Plan\n")
        .await
        .unwrap();

    let mgr = SpecManager::new(root);
    let by_id = mgr
        .list_specs(&SpecFilter::new().with_sort(SortBy::Id))
        .await
        .unwrap();
    assert_eq!(by_id[0].id.as_str(), "alpha");
    assert_eq!(by_id[1].id.as_str(), "zeta");

    let by_title = mgr
        .list_specs(&SpecFilter::new().with_sort(SortBy::Title))
        .await
        .unwrap();
    assert_eq!(by_title[0].title, "Alpha");
    assert_eq!(by_title[1].title, "Zeta");
}

#[tokio::test]
async fn test_manager_list_exclude_archived() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    let id1 = SpecId::new("active").unwrap();
    let id2 = SpecId::new("archived").unwrap();
    SpecIo::create_spec_dir(root, &id1, "# Active\n", "# Plan\n")
        .await
        .unwrap();
    SpecIo::create_spec_dir(
        root,
        &id2,
        "---\nstatus: archived\n---\n\n# Archived\n",
        "# Plan\n",
    )
    .await
    .unwrap();

    let mgr = SpecManager::new(root);
    let default_list = mgr.list_specs(&SpecFilter::new()).await.unwrap();
    assert_eq!(default_list.len(), 1);
    assert_eq!(default_list[0].id.as_str(), "active");

    let with_archived = mgr
        .list_specs(&SpecFilter::new().with_archived())
        .await
        .unwrap();
    assert_eq!(with_archived.len(), 2);
}

// ── transition_with_flags tests (T-036, FR-019) ─────────────────────────

#[tokio::test]
async fn test_transition_with_flags_disabled_allows_clarifications() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    let id = SpecId::new("clar-flag-off").unwrap();
    let spec_md =
        "---\nstatus: in_review\n---\n\n# Clar Flag Off\n\n[NEEDS CLARIFICATION: what scale?]\n";
    SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
        .await
        .unwrap();

    let mgr = SpecManager::new(root);
    let mut spec = mgr.read_spec(&id).await.unwrap();
    spec.status = SpecStatus::InReview;

    // With clarification_markers disabled, transition should succeed
    // even though markers are present.
    let flags = crate::validate::SddFlags::all_disabled();
    mgr.transition_with_flags(&mut spec, SpecStatus::Approved, "alice", &flags)
        .await
        .unwrap();
    assert_eq!(spec.status, SpecStatus::Approved);
}

#[tokio::test]
async fn test_transition_with_flags_enabled_blocks_clarifications() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    let id = SpecId::new("clar-flag-on").unwrap();
    let spec_md =
        "---\nstatus: in_review\n---\n\n# Clar Flag On\n\n[NEEDS CLARIFICATION: what scale?]\n";
    SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
        .await
        .unwrap();

    let mgr = SpecManager::new(root);
    let mut spec = mgr.read_spec(&id).await.unwrap();
    spec.status = SpecStatus::InReview;

    // With clarification_markers enabled, transition should be blocked.
    let flags = crate::validate::SddFlags {
        clarification_markers: true,
        ..crate::validate::SddFlags::all_disabled()
    };
    let result = mgr
        .transition_with_flags(&mut spec, SpecStatus::Approved, "alice", &flags)
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, SpecError::UnresolvedClarifications { count } if count == 1),
        "expected UnresolvedClarifications error, got {err:?}"
    );
}

#[tokio::test]
async fn test_transition_backward_compat_blocks_clarifications() {
    // The legacy transition() should still block (all_enabled flags).
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    let id = SpecId::new("clar-compat").unwrap();
    let spec_md = "---\nstatus: in_review\n---\n\n# Clar Compat\n\n[NEEDS CLARIFICATION: what?]\n";
    SpecIo::create_spec_dir(root, &id, spec_md, "# Plan\n")
        .await
        .unwrap();

    let mgr = SpecManager::new(root);
    let mut spec = mgr.read_spec(&id).await.unwrap();
    spec.status = SpecStatus::InReview;

    let result = mgr
        .transition(&mut spec, SpecStatus::Approved, "alice")
        .await;
    assert!(result.is_err());
}

// ── Phase -1 gate transition tests (T-017, FR-008) ──────────────────────

/// Helper: create a spec dir with a PLAN.md containing the given gate
/// markdown.
async fn make_gate_spec(root: &Path, id: &str, plan_md: &str) {
    let spec_id = SpecId::new(id).unwrap();
    let spec_md = "---\nstatus: draft\n---\n\n# Gate Test\n\nSome requirement.\n";
    SpecIo::create_spec_dir(root, &spec_id, spec_md, plan_md)
        .await
        .unwrap();
}

const GATES_ALL_CHECKED: &str = "\
# Plan

## Phase -1 Gates

- [x] **Simplicity**: The design is minimal.
- [x] **Anti-Abstraction**: No speculative abstractions.
- [x] **Integration-First**: Integration points identified.

## Tasks

| ID | Description | Status |
|----|-------------|--------|
| T-001 | Do something | pending |
";

const GATES_ONE_UNCHECKED: &str = "\
# Plan

## Phase -1 Gates

- [x] **Simplicity**: The design is minimal.
- [ ] **Anti-Abstraction**: No speculative abstractions.
- [x] **Integration-First**: Integration points identified.

## Tasks

| ID | Description | Status |
|----|-------------|--------|
| T-001 | Do something | pending |
";

const GATES_ALL_UNCHECKED: &str = "\
# Plan

## Phase -1 Gates

- [ ] **Simplicity**: The design is minimal.
- [ ] **Anti-Abstraction**: No speculative abstractions.
- [ ] **Integration-First**: Integration points identified.

## Tasks

| ID | Description | Status |
|----|-------------|--------|
| T-001 | Do something | pending |
";

const GATES_SECTION_MISSING: &str = "\
# Plan

## Tasks

| ID | Description | Status |
|----|-------------|--------|
| T-001 | Do something | pending |
";

#[tokio::test]
async fn test_transition_in_progress_blocked_when_gates_unchecked() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    make_gate_spec(root, "gate-unchecked", GATES_ONE_UNCHECKED).await;

    let mgr = SpecManager::new(root);
    let mut spec = mgr
        .read_spec(&SpecId::new("gate-unchecked").unwrap())
        .await
        .unwrap();
    spec.status = SpecStatus::Approved;

    let flags = SddFlags {
        phase_minus_one_gates: true,
        ..SddFlags::all_disabled()
    };
    let result = mgr
        .transition_with_flags(&mut spec, SpecStatus::InProgress, "alice", &flags)
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(&err, SpecError::UncheckedPhaseGates { gates } if gates.len() == 1 && gates[0] == "Anti-Abstraction"),
        "expected UncheckedPhaseGates with Anti-Abstraction, got {err:?}"
    );
}

#[tokio::test]
async fn test_transition_in_progress_blocked_when_all_gates_unchecked() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    make_gate_spec(root, "gate-all-unchecked", GATES_ALL_UNCHECKED).await;

    let mgr = SpecManager::new(root);
    let mut spec = mgr
        .read_spec(&SpecId::new("gate-all-unchecked").unwrap())
        .await
        .unwrap();
    spec.status = SpecStatus::Approved;

    let flags = SddFlags {
        phase_minus_one_gates: true,
        ..SddFlags::all_disabled()
    };
    let result = mgr
        .transition_with_flags(&mut spec, SpecStatus::InProgress, "alice", &flags)
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(&err, SpecError::UncheckedPhaseGates { gates } if gates.len() == 3),
        "expected 3 unchecked gates, got {err:?}"
    );
}

#[tokio::test]
async fn test_transition_in_progress_blocked_when_gate_section_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    make_gate_spec(root, "gate-missing", GATES_SECTION_MISSING).await;

    let mgr = SpecManager::new(root);
    let mut spec = mgr
        .read_spec(&SpecId::new("gate-missing").unwrap())
        .await
        .unwrap();
    spec.status = SpecStatus::Approved;

    let flags = SddFlags {
        phase_minus_one_gates: true,
        ..SddFlags::all_disabled()
    };
    let result = mgr
        .transition_with_flags(&mut spec, SpecStatus::InProgress, "alice", &flags)
        .await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(&err, SpecError::UncheckedPhaseGates { gates } if gates.len() == 3),
        "expected 3 missing gates, got {err:?}"
    );
}

#[tokio::test]
async fn test_transition_in_progress_allowed_when_gates_checked() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    make_gate_spec(root, "gate-checked", GATES_ALL_CHECKED).await;

    let mgr = SpecManager::new(root);
    let mut spec = mgr
        .read_spec(&SpecId::new("gate-checked").unwrap())
        .await
        .unwrap();
    spec.status = SpecStatus::Approved;

    let flags = SddFlags {
        phase_minus_one_gates: true,
        ..SddFlags::all_disabled()
    };
    mgr.transition_with_flags(&mut spec, SpecStatus::InProgress, "alice", &flags)
        .await
        .unwrap();
    assert_eq!(spec.status, SpecStatus::InProgress);
}

#[tokio::test]
async fn test_transition_in_progress_allowed_when_gate_flag_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    make_gate_spec(root, "gate-flag-off", GATES_ALL_UNCHECKED).await;

    let mgr = SpecManager::new(root);
    let mut spec = mgr
        .read_spec(&SpecId::new("gate-flag-off").unwrap())
        .await
        .unwrap();
    spec.status = SpecStatus::Approved;

    // With phase_minus_one_gates disabled, transition should succeed
    // even though all gates are unchecked.
    let flags = SddFlags::all_disabled();
    mgr.transition_with_flags(&mut spec, SpecStatus::InProgress, "alice", &flags)
        .await
        .unwrap();
    assert_eq!(spec.status, SpecStatus::InProgress);
}

#[tokio::test]
async fn test_transition_backward_compat_blocks_unchecked_gates() {
    // The legacy transition() uses all_enabled flags, so it should block.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    make_gate_spec(root, "gate-compat", GATES_ONE_UNCHECKED).await;

    let mgr = SpecManager::new(root);
    let mut spec = mgr
        .read_spec(&SpecId::new("gate-compat").unwrap())
        .await
        .unwrap();
    spec.status = SpecStatus::Approved;

    let result = mgr
        .transition(&mut spec, SpecStatus::InProgress, "alice")
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_transition_non_in_progress_not_blocked_by_gates() {
    // Transitions to statuses other than in_progress should not be
    // affected by Phase -1 gate checks.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    make_gate_spec(root, "gate-other", GATES_ALL_UNCHECKED).await;

    let mgr = SpecManager::new(root);
    let mut spec = mgr
        .read_spec(&SpecId::new("gate-other").unwrap())
        .await
        .unwrap();
    spec.status = SpecStatus::InReview;

    // Transition to Approved should not be blocked by gates (only by
    // clarification markers, which are disabled here).
    let flags = SddFlags {
        phase_minus_one_gates: true,
        ..SddFlags::all_disabled()
    };
    mgr.transition_with_flags(&mut spec, SpecStatus::Approved, "alice", &flags)
        .await
        .unwrap();
    assert_eq!(spec.status, SpecStatus::Approved);
}
