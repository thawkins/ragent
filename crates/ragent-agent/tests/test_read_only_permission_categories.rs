//! Guard: the read-only permission-category allowlist classifies every tool
//! the registry actually ships.
//!
//! `READ_ONLY_PERMISSION_CATEGORIES` answers one question - "can a run offered
//! this tool mutate the workspace?" - and its failure mode is silent: a
//! category the list forgets is treated as WRITE-capable, so a pure inspection
//! sub-agent (an `explore` run offered only `git_status`, `github_list_issues`
//! and `codeindex_search`) receives the automatic YOLO write scope instead of
//! the ordinary prompt policy.
//!
//! Two things are asserted here, deliberately at different strengths:
//!
//! | Case | What it pins | Why this strength |
//! |---|---|---|
//! | 1 | every registry tool's category is a *known* category | a typo (`git:reed`) is invisible to the classifier and must fail loudly |
//! | 2 | the named read surfaces are read-only | the literal requirement; derived from the registry it would agree with any regression |
//! | 3 | the named write surfaces are write-capable | the pessimistic half - a misclassification here is what costs safety |
//!
//! Case 1 is the one that catches the real bug the audit found: it does not
//! assert a *classification*, only that the vocabulary is closed, so adding a
//! tool with a brand-new category fails here and forces a decision.

use ragent_agent::session::permissions::{
    READ_ONLY_PERMISSION_CATEGORIES, is_write_capable_permission_category,
};
use ragent_agent::tool::create_default_registry;

/// Every category a registered tool in the default build declares.
///
/// Collected from the registry rather than from the tool files, so a tool that
/// is registered but never exported still contributes its category.
fn declared_categories() -> Vec<(String, String)> {
    let registry = create_default_registry();
    registry
        .all_tools()
        .into_iter()
        .map(|tool| {
            (
                tool.name().to_string(),
                tool.permission_category().to_string(),
            )
        })
        .collect()
}

/// The closed set of permission-category strings this workspace declares.
///
/// Kept beside [`READ_ONLY_PERMISSION_CATEGORIES`] so the two lists can be
/// compared: every entry here is either read-only or explicitly write-capable,
/// and nothing may fall outside both.
const KNOWN_CATEGORIES: &[&str] = &[
    // read-only
    "file:read",
    "read",
    "codeindex",
    "codeindex:read",
    "web",
    "web:fetch",
    "web:search",
    "web:read",
    "network",
    "model",
    "model:read",
    "shell:open",
    "memory:read",
    "spec:read",
    "skill:read",
    "git:read",
    "github:read",
    "gitlab:read",
    "cron:read",
    "agent:read",
    "team:read",
    "task",
    "system",
    // write-capable
    "file:write",
    "git:write",
    "github:write",
    "gitlab:write",
    "bash:execute",
    "shell:execute",
    "network:fetch",
    "codeindex:write",
    "spec:write",
    "skill:manage",
    "cron:write",
    "agent:spawn",
    "agent:control",
    "team:manage",
    "team:communicate",
    "team:tasks",
    "storage:write",
    "plan",
    "mcp",
    "ask_user",
    "stub:exec",
    // the permission layer never consults these
    "none",
];

#[test]
fn test_every_registered_category_is_known() {
    let categories = declared_categories();
    assert!(
        !categories.is_empty(),
        "the default registry must expose tools to classify"
    );
    let mut unknown: Vec<String> = categories
        .iter()
        .filter(|(_, category)| !KNOWN_CATEGORIES.contains(&category.as_str()))
        .map(|(tool, category)| format!("{tool} -> \"{category}\""))
        .collect();
    unknown.sort();
    unknown.dedup();
    assert!(
        unknown.is_empty(),
        "registered tool(s) declare a permission category this test does not know, so the \
         read-only classifier has never been asked about it: {unknown:?}"
    );
}

#[test]
fn test_read_surfaces_are_read_only() {
    // The seven categories the audit found missing, plus the two that share a
    // tool family with a mutating member and must stay classified deliberately.
    for category in [
        "git:read",
        "github:read",
        "gitlab:read",
        "cron:read",
        "agent:read",
        "team:read",
        "model:read",
        "file:read",
        "spec:read",
        "web:read",
    ] {
        assert!(
            READ_ONLY_PERMISSION_CATEGORIES.contains(&category),
            "`{category}` reads and must be read-only, or an inspection run is handed the \
             YOLO write scope"
        );
        assert!(
            !is_write_capable_permission_category(category),
            "`{category}` classified as write-capable"
        );
    }
}

#[test]
fn test_write_surfaces_are_write_capable() {
    for category in [
        "file:write",
        "git:write",
        "github:write",
        "gitlab:write",
        "bash:execute",
        "cron:write",
        "agent:spawn",
        "agent:control",
        "team:manage",
        "team:communicate",
        // `team_task_claim` / `team_task_complete` share this with the
        // read-only `team_task_list`; the pessimistic answer wins.
        "team:tasks",
        "storage:write",
        "spec:write",
        "skill:manage",
        "codeindex:write",
        "shell:execute",
        "network:fetch",
        // No category at all means the permission layer is bypassed, so the
        // classifier must never call that read-only.
        "none",
        "",
    ] {
        assert!(
            is_write_capable_permission_category(category),
            "`{category}` mutates (or bypasses the permission layer) and must be \
             write-capable"
        );
    }
}

#[test]
fn test_read_only_list_has_no_duplicates() {
    let mut seen = std::collections::HashSet::new();
    for category in READ_ONLY_PERMISSION_CATEGORIES {
        assert!(
            seen.insert(*category),
            "`{category}` appears twice in READ_ONLY_PERMISSION_CATEGORIES"
        );
    }
}

#[test]
fn test_explore_surface_is_read_only() {
    // The worked example from the audit: the tools an `explore` sub-agent is
    // offered in practice must all classify read-only, or the run gets the
    // YOLO write scope.
    let categories = declared_categories();
    for name in [
        "Read",
        "Grep",
        "Glob",
        "list",
        "codeindex_search",
        "git_status",
        "git_log",
        "git_diff",
        "github_list_issues",
        "gitlab_list_pipelines",
    ] {
        let Some((_, category)) = categories.iter().find(|(tool, _)| tool == name) else {
            // Not every build registers every tool; a missing one is not this
            // test's failure (test_every_registered_category_is_known covers
            // what is present).
            continue;
        };
        assert!(
            !is_write_capable_permission_category(category),
            "`{name}` declares `{category}` and must classify read-only"
        );
    }
}
