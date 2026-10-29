//! Guard: one permission taxonomy for one task registry (spec `agentssetup`
//! FR-005).
//!
//! Four tools read and write a single object - `AgentManager::tasks` - and the
//! permission layer is the only place a policy can say "spawn, but do not
//! cancel". FR-005 therefore fixes the classification, clause by clause:
//!
//! | # | Clause | Case |
//! |---|---|---|
//! | 1 | `agent:spawn` for `new_agent` | `test_new_agent_is_the_only_agent_spawn_category` |
//! | 2 | `agent:control` for `cancel_agent` and `wait_agents` | `test_cancel_agent_and_wait_agents_share_the_control_category` |
//! | 3 | a read-only category for `list_agents` | `test_list_agents_is_read_only_never_none` |
//! | 4 | no sub-agent tool is `none` | `test_no_sub_agent_tool_is_uncategorised` |
//!
//! ## Why the categories are asserted as literals
//!
//! `permission_category()` is permission *vocabulary*: it decides which
//! `permission` rule a call is judged against. Deriving the expectation from the
//! registry would make the assertion agree with whatever the tree happened to
//! do, which is exactly what the requirement forbids - a tool that silently
//! regains `none` would then pass.
//!
//! ## What this file deliberately does not check
//!
//! - **Auto-approval.** `list_agents` and `cancel_agent` remain in
//!   `AUTO_APPROVED_AGENT_TOOLS`, so an interactive session never prompts for
//!   them. That is a separate decision and a separate helper
//!   (`is_hardwired_auto_approved_tool`, covered by `test_permission_helpers`);
//!   FR-005 constrains the *classification*, not the prompt outcome. Case 5
//!   pins only that the two do not contradict each other today, so a future
//!   change that starts prompting for an observation has to say so here.
//! - **The `Permission` enum mapping.** `agent:read` normalises to
//!   `Permission::Read` and `agent:control` / `agent:spawn` to `Custom`; that is
//!   `ragent-config`'s vocabulary and its own tests own it.

use ragent_agent::session::permissions::is_hardwired_auto_approved_tool_for_test;
use ragent_agent::tool::cancel_agent::CancelAgentTool;
use ragent_agent::tool::list_agents::ListAgentsTool;
use ragent_agent::tool::new_agent::NewAgentTool;
use ragent_agent::tool::wait_agents::WaitAgentsTool;
use ragent_agent::tool::{Tool, create_default_registry};
use std::sync::Arc;

/// The four tools FR-005 classifies, as `(name, tool)`, taken from the default
/// registry so a tool that stops being registered fails here rather than
/// quietly dropping out of the family.
fn family() -> Vec<(&'static str, Arc<dyn Tool>)> {
    let registry = create_default_registry();
    ["new_agent", "cancel_agent", "list_agents", "wait_agents"]
        .into_iter()
        .filter_map(|name| registry.get(name).map(|tool| (name, tool)))
        .collect()
}

#[test]
fn test_new_agent_is_the_only_agent_spawn_category() {
    // FR-005 clause 1: spawning is the one action that creates a task, so it is
    // the one tool that reports `agent:spawn`.
    assert_eq!(
        NewAgentTool.permission_category(),
        "agent:spawn",
        "FR-005: `new_agent` must report `agent:spawn` - it is the only tool in \
         the family that creates a task"
    );

    let spawners: Vec<&str> = family()
        .iter()
        .filter(|(_, tool)| tool.permission_category() == "agent:spawn")
        .map(|(name, _)| *name)
        .collect();
    assert_eq!(
        spawners,
        vec!["new_agent"],
        "FR-005: `agent:spawn` is the classification of exactly one tool, \
         `new_agent`; {spawners:?} report it. A control action that reports \
         `agent:spawn` cannot be granted separately from spawning, which is the \
         split FR-005 exists to make"
    );
}

#[test]
fn test_cancel_agent_and_wait_agents_share_the_control_category() {
    // FR-005 clause 2: cancelling and awaiting both act on a task that already
    // exists, so one `agent:control` rule covers both.
    assert_eq!(
        CancelAgentTool.permission_category(),
        "agent:control",
        "FR-005: `cancel_agent` is a *control* action, not a spawn - it creates \
         nothing. Reporting `agent:spawn` conflated \"start work\" with \"stop \
         work\""
    );
    assert_eq!(
        WaitAgentsTool.permission_category(),
        "agent:control",
        "FR-005: `wait_agents` must report `agent:control`, the category \
         `cancel_agent` shares"
    );
}

#[test]
fn test_list_agents_is_read_only_never_none() {
    // FR-005 clause 3: the read-only member of the family is categorised as a
    // read, not as "no category".
    let category = ListAgentsTool.permission_category();
    assert_eq!(
        category, "agent:read",
        "FR-005: `list_agents` is a read-only observation of the task registry \
         and must report a read category, not {category:?}"
    );
    assert_ne!(
        category, "none",
        "FR-005: `none` means \"this tool needs no permission\". Listing \
         sub-agent tasks reads another agent's work, so it needs one - a policy \
         must be able to deny observation without denying the spawn"
    );
    assert!(
        category.ends_with(":read"),
        "FR-005: the read-only member of the family reports a read category, \
         got {category:?}"
    );
}

#[test]
fn test_no_sub_agent_tool_is_uncategorised() {
    // FR-005 clause 4: the blanket clause. Asserted over the *registry*, not
    // the four constants above, so a fifth tool added to the family is caught.
    let uncategorised: Vec<&str> = family()
        .iter()
        .filter(|(_, tool)| {
            let category = tool.permission_category();
            category.is_empty() || category == "none"
        })
        .map(|(name, _)| *name)
        .collect();
    assert!(
        uncategorised.is_empty(),
        "FR-005: no sub-agent tool may be classified `none`; {uncategorised:?} \
         are. An uncategorised tool bypasses the permission layer entirely \
         (processor.rs short-circuits on `none`), so a policy written against \
         its category silently never runs"
    );

    // The whole family is still registered: FR-005 classifies four tools, and a
    // missing one would make every assertion above vacuous.
    assert_eq!(
        family().len(),
        4,
        "FR-005: the sub-agent family is `new_agent`, `cancel_agent`, \
         `list_agents`, `wait_agents`; the registry does not hold all four"
    );
}

#[test]
fn test_the_new_categories_do_not_change_the_prompt_outcome() {
    // `list_agents` moved off `none` in FR-005 and `cancel_agent` off
    // `agent:spawn`. Both stay in `AUTO_APPROVED_AGENT_TOOLS`, so an
    // interactive session still never prompts for them - the classification
    // tightened, the outcome did not. Pinned here so a change that starts
    // prompting on an observation has to update this case deliberately.
    for tool in ["list_agents", "cancel_agent", "wait_agents", "new_agent"] {
        assert!(
            is_hardwired_auto_approved_tool_for_test(tool),
            "FR-005: `{tool}` reports a real category now, so it reaches the \
             permission path; it must stay hardwired auto-approved or every \
             sub-agent call gains an interactive prompt"
        );
    }
}
