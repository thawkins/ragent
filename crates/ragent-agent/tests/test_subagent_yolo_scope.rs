//! The automatic sub-agent YOLO scope.
//!
//! A write-capable tool in an unattended sub-agent run has no user to answer a
//! permission prompt, so the prompt would only stall the run until the watchdog.
//! Such a call bypasses the *interactive prompt* regardless of the global
//! `/yolo` flag - and nothing else about the run changes: the primary session is
//! untouched, a read-only run is untouched, explicit policy rules still decide,
//! the tool-repeat guard still applies, and a forced destructive-action
//! checkpoint still prompts (FR-024).
//!
//! These cases pin the two halves of the membership test
//! (`subagent_yolo_bypasses_prompt`) and the category classification
//! (`is_write_capable_permission_category`) it is built from. The prompt path
//! itself is asserted at the end against `check_permission_with_prompt`, so the
//! flag cannot be computed and then dropped on the floor.

use std::collections::HashSet;
use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionAction;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::session::permissions::{
    check_permission_with_prompt, is_write_capable_permission_category,
    subagent_yolo_bypasses_prompt,
};

/// A read-only category is never write-capable - the read half of the
/// classification, stated as the categories a tool surface actually declares.
#[test]
fn read_only_categories_are_not_write_capable() {
    for category in ["file:read", "codeindex", "web", "network", "model"] {
        assert!(
            !is_write_capable_permission_category(category),
            "'{category}' cannot mutate the workspace and must not put a run in \
             the automatic YOLO scope"
        );
    }
}

/// Every category a tool surface declares for a mutating tool is write-capable,
/// and an uncategorised tool is write-capable too: `none` bypasses the
/// permission layer entirely, so treating it as harmless would let an
/// uncategorised tool into a read-only run's surface silently.
#[test]
fn mutating_and_uncategorised_categories_are_write_capable() {
    for category in [
        "file:write",
        "bash",
        "team:manage",
        "git",
        "memory",
        "none",
        "",
    ] {
        assert!(
            is_write_capable_permission_category(category),
            "'{category}' must be treated as write-capable"
        );
    }
}

/// The scope needs BOTH halves: an unattended run (sub-agent) and a
/// write-capable tool. A primary session never gets it, and a read-only tool
/// never gets it.
#[test]
fn subagent_yolo_requires_an_unattended_run_and_a_write_capable_tool() {
    let write_capable: HashSet<String> = ["Bash", "edit"].into_iter().map(str::to_string).collect();

    assert!(
        subagent_yolo_bypasses_prompt(true, &write_capable, "Bash"),
        "a write-capable tool in a sub-agent run is in the automatic scope"
    );
    assert!(
        !subagent_yolo_bypasses_prompt(false, &write_capable, "Bash"),
        "the primary session must never inherit the automatic scope"
    );
    assert!(
        !subagent_yolo_bypasses_prompt(true, &write_capable, "read"),
        "a read-only tool stays under the ordinary prompt policy"
    );
    assert!(
        !subagent_yolo_bypasses_prompt(true, &HashSet::new(), "Bash"),
        "a run whose surface cannot write gets no automatic scope at all"
    );
}

/// The last line of defence: the flag actually suppresses the interactive
/// prompt. Without this the classification above could be correct and unused.
#[tokio::test]
async fn subagent_yolo_answers_the_prompt_while_a_read_only_run_is_prompted() {
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    // No rule, no allowlist entry, an absolute path outside the project, and a
    // 1-second prompt window so the prompted case fails fast instead of waiting
    // out the production timeout: this case is about which branch is taken, not
    // how long a timeout lasts.
    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-subagent",
        "file:write",
        "/nonexistent/outside/victim.txt",
        "edit",
        Some(false),
        None,
        false,
        1,
        false,
        true,
    )
    .await
    .expect("the automatic scope resolves without a prompt");
    assert_eq!(
        action,
        PermissionAction::Allow,
        "a write-capable tool in a sub-agent run is auto-approved"
    );

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-primary",
        "file:write",
        "/nonexistent/outside/victim.txt",
        "edit",
        Some(false),
        None,
        false,
        1,
        false,
        false,
    )
    .await
    .expect("a timed-out prompt counts as a denial, not an error");
    assert_eq!(
        action,
        PermissionAction::Deny,
        "the same call outside the automatic scope still prompts, and an \
         unanswered prompt is a denial"
    );
}

/// FR-024 containment: the automatic scope suppresses the prompt but never a
/// forced destructive-action checkpoint. A destructive call under the scope is
/// still checkpointed, and with the prompt unanswered it is denied.
#[tokio::test]
async fn subagent_yolo_never_bypasses_a_forced_checkpoint() {
    let checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::new())));
    let event_bus = Arc::new(EventBus::new(16));

    let action = check_permission_with_prompt(
        &checker,
        &event_bus,
        "session-subagent",
        "file:write",
        "/nonexistent/outside/victim.txt",
        "rm",
        None,
        None,
        true,
        1,
        false,
        true,
    )
    .await
    .expect("the checkpoint prompt resolves by timeout into a denial");
    assert_eq!(
        action,
        PermissionAction::Deny,
        "FR-024: a forced checkpoint outranks the automatic scope"
    );
}
