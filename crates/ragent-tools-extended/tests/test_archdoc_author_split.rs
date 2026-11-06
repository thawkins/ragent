//! Regression tests for the shared `/spec govcreate` authored-body splitter
//! (`ragent_tools_extended::archdoc::split_authored_sections`).
//!
//! Both the TUI slash surface and the `ragent spec govcreate` CLI parity path
//! call the shared splitter; these tests pin the contract at its canonical
//! home so the two surfaces cannot drift again (the earlier hand-synced
//! copies diverged on marker-reorder semantics).

use ragent_tools_extended::archdoc::split_authored_sections;

#[test]
fn test_split_authored_sections_no_markers_treats_body_as_spec() {
    let (spec, plan, testplan) = split_authored_sections("# The spec body\n");
    assert_eq!(spec, "# The spec body\n");
    assert!(plan.contains("to be filled"), "plan placeholder: {plan}");
    assert!(
        testplan.contains("to be filled"),
        "testplan placeholder: {testplan}"
    );
}

#[test]
fn test_split_authored_sections_three_markers_in_order() {
    let body = "1. `out/specs/x/SPEC.md`\nspec body\n2. `out/specs/x/PLAN.md`\nplan body\n3. `out/specs/x/TESTPLAN.md`\ntest body\n";
    let (spec, plan, testplan) = split_authored_sections(body);
    assert!(spec.contains("spec body"), "spec section: {spec}");
    assert!(!spec.contains("plan body"), "spec leaked plan: {spec}");
    assert!(plan.contains("plan body"), "plan section: {plan}");
    assert!(
        testplan.contains("test body"),
        "testplan section: {testplan}"
    );
}

#[test]
fn test_split_authored_sections_reordered_markers_assign_by_identity() {
    // A model that emits PLAN before SPEC must still land each section in
    // its own file: assignment follows the marker name, not the position.
    // The earlier TUI copy assigned positionally and swapped the contents.
    let body =
        "1. `x/PLAN.md`\nplan body\n2. `x/SPEC.md`\nspec body\n3. `x/TESTPLAN.md`\ntest body\n";
    let (spec, plan, testplan) = split_authored_sections(body);
    assert!(
        spec.contains("spec body"),
        "spec must hold the SPEC.md tail, got: {spec}"
    );
    assert!(
        plan.contains("plan body"),
        "plan must hold the PLAN.md tail, got: {plan}"
    );
    assert!(
        testplan.contains("test body"),
        "testplan must hold the TESTPLAN.md tail, got: {testplan}"
    );
}

#[test]
fn test_split_authored_sections_single_marker_splits_prefix_into_spec() {
    let (spec, plan, _testplan) =
        split_authored_sections("intro prose\n1. `x/PLAN.md`\nplan body\n");
    assert!(
        spec.contains("intro prose"),
        "prefix folds into spec: {spec}"
    );
    assert!(plan.contains("plan body"), "plan tail: {plan}");
}

#[test]
fn test_split_authored_sections_case_insensitive_markers() {
    let body = "1. spec.md\nspec body\n2. plan.md\nplan body\n";
    let (spec, plan, testplan) = split_authored_sections(body);
    assert!(spec.contains("spec body"), "spec section: {spec}");
    assert!(plan.contains("plan body"), "plan section: {plan}");
    assert!(
        testplan.contains("to be filled"),
        "unmentioned testplan gets placeholder: {testplan}"
    );
}

#[test]
fn test_split_authored_sections_non_ascii_before_marker_does_not_panic() {
    // Lowercasing `\u{1E9E}` shrinks it from 3 bytes to 2, so an offset taken
    // from a lowercased copy used to land inside the euro sign and panic.
    let body = "\u{1E9E}\u{20AC}SPEC.md\nspec body\n";
    let (spec, _, _) = split_authored_sections(body);
    assert!(spec.starts_with("SPEC.md"), "spec section: {spec}");
}

#[test]
fn test_split_authored_sections_plan_marker_not_matched_inside_testplan() {
    // TESTPLAN before PLAN: `plan.md` used to match inside `TESTPLAN.md`, so the
    // test-plan body was filed as PLAN.md.
    let body =
        "1. `x/SPEC.md`\nspec body\n2. `x/TESTPLAN.md`\ntest body\n3. `x/PLAN.md`\nplan body\n";
    let (_, plan, testplan) = split_authored_sections(body);
    assert!(plan.starts_with("PLAN.md"), "plan section: {plan}");
    assert!(plan.contains("plan body"), "plan section: {plan}");
    assert!(
        testplan.contains("test body"),
        "testplan section: {testplan}"
    );
    assert!(!plan.contains("test body"), "plan section: {plan}");
}
