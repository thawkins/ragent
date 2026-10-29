//! Integration coverage for the shared team-teardown guard (spec `teamssetup`
//! FR-022 / F-22, F-23).
//!
//! `/team cleanup`, `/team delete`, and the `team_cleanup` tool each used to
//! carry their own rule:
//!
//! | Surface | Pre-fix rule | Wrongly refused | Wrongly allowed |
//! |---------|--------------|-----------------|-----------------|
//! | `/team cleanup` | `Working` only | `Spawning`, `ShuttingDown` | `Blocked` |
//! | `/team delete` | `Working` only (active team) | `Spawning`, `ShuttingDown` | `Blocked` |
//! | `team_cleanup` tool | `!= Stopped` | `Idle`, `PlanPending`, `Blocked`, `Suspended` | - |
//!
//! The three now call [`ragent_agent::team::cleanup_refusal`], so agreement is
//! structural. These tests pin the predicate, the refusal text, and the tool's
//! use of it.

use ragent_agent::team::{
    MemberStatus, blocking_members, blocks_teardown, cleanup_refusal, delete_confirmation,
};

/// Every status a member can hold, so a new variant cannot be added without this
/// test failing to enumerate it.
const ALL_STATUSES: [MemberStatus; 9] = [
    MemberStatus::Spawning,
    MemberStatus::Working,
    MemberStatus::Idle,
    MemberStatus::PlanPending,
    MemberStatus::Blocked,
    MemberStatus::Suspended,
    MemberStatus::ShuttingDown,
    MemberStatus::Stopped,
    MemberStatus::Failed,
];

// -- predicate ---------------------------------------------------------------

#[test]
fn teardown_is_blocked_by_exactly_the_live_and_shutting_down_statuses() {
    // FR-022 states the rule as "no member is `is_live()` or `ShuttingDown`".
    // `is_live()` already includes `ShuttingDown`; both terms are spelled out so
    // the status that *sounds* terminal is visibly covered.
    let expected_blocking = [
        MemberStatus::Spawning,
        MemberStatus::Working,
        MemberStatus::ShuttingDown,
    ];
    for status in ALL_STATUSES {
        let should_block = expected_blocking.contains(&status);
        assert_eq!(
            blocks_teardown(status),
            should_block,
            "{status:?} should{} block teardown",
            if should_block { "" } else { " not" }
        );
    }
}

#[test]
fn every_status_is_classified_exactly_once_by_the_three_predicates() {
    // The guard is only correct if the status machine partitions cleanly: a
    // status that were both live and parked would make the teardown rule
    // ambiguous, and one that were neither would silently permit teardown.
    for status in ALL_STATUSES {
        let live = status.is_live();
        let parked = status.is_parked();
        let terminal = status.is_terminal();
        let classified = [live, parked, terminal].iter().filter(|&&b| b).count();
        assert_eq!(
            classified, 1,
            "{status:?} must satisfy exactly one predicate (live={live}, \
             parked={parked}, terminal={terminal})"
        );
        // The teardown rule covers the live third, which by construction
        // includes `ShuttingDown`.
        assert_eq!(blocks_teardown(status), live, "{status:?}");
    }
}

#[test]
fn parked_members_do_not_block_teardown() {
    // This is the case the pre-fix `team_cleanup` tool got wrong: it refused
    // while any member was `!= Stopped`, which includes every parked status.
    for status in [
        MemberStatus::Idle,
        MemberStatus::PlanPending,
        MemberStatus::Blocked,
        MemberStatus::Suspended,
    ] {
        assert!(
            !blocks_teardown(status),
            "{status:?} is parked, not live, and must not block teardown"
        );
    }
}

// -- blocking_members --------------------------------------------------------

#[test]
fn blocking_members_names_only_the_blocking_ones_with_canonical_status() {
    let roster = [
        ("alpha", MemberStatus::Working),
        ("beta", MemberStatus::Idle),
        ("gamma", MemberStatus::ShuttingDown),
        ("delta", MemberStatus::Stopped),
        ("epsilon", MemberStatus::Spawning),
    ];
    assert_eq!(
        blocking_members(roster),
        ["alpha [working]", "gamma [stopping]", "epsilon [spawning]"]
    );
}

#[test]
fn blocking_members_is_empty_for_a_fully_parked_or_terminal_roster() {
    let roster = [
        ("beta", MemberStatus::Idle),
        ("gamma", MemberStatus::Suspended),
        ("delta", MemberStatus::Stopped),
        ("zeta", MemberStatus::Failed),
    ];
    assert_eq!(blocking_members(roster), Vec::<String>::new());
}

// -- cleanup_refusal ---------------------------------------------------------

#[test]
fn cleanup_refusal_is_none_when_nothing_blocks() {
    let roster = [
        ("beta", MemberStatus::Idle),
        ("gamma", MemberStatus::Blocked),
        ("delta", MemberStatus::Stopped),
    ];
    assert_eq!(cleanup_refusal("tguard", roster), None);
    // An empty roster is not a refusal either.
    assert_eq!(cleanup_refusal("tguard", []), None);
}

#[test]
fn cleanup_refusal_names_entity_state_and_next_action() {
    // FR-021 requires every refusal to name the offending entity, its state, and
    // a concrete next action; FR-022 requires the same text on all surfaces.
    let roster = [
        ("alpha", MemberStatus::Working),
        ("beta", MemberStatus::ShuttingDown),
        ("gamma", MemberStatus::Idle),
    ];
    let text = cleanup_refusal("tguard", roster).expect("two members block teardown");

    // Entity + state, per blocking member.
    assert!(text.contains("alpha [working]"), "{text}");
    assert!(text.contains("beta [stopping]"), "{text}");
    // Parked members are not named as blockers.
    assert!(!text.contains("gamma"), "{text}");
    // The team name and the count are both present.
    assert!(text.contains("team 'tguard'"), "{text}");
    assert!(text.contains("2 teammate(s)"), "{text}");
    // Two concrete next actions: the graceful one and the forced one.
    assert!(text.contains("team_shutdown_teammate"), "{text}");
    assert!(text.contains("forcecleanup"), "{text}");
}

#[test]
fn cleanup_refusal_is_byte_identical_for_the_same_roster() {
    // The three surfaces pass different iterators over differently-sourced
    // members; what they must produce is the same string. Building it twice from
    // independently-constructed inputs proves the text is a pure function of the
    // roster and not of the caller.
    let from_tui = cleanup_refusal("tguard", [("alpha", MemberStatus::Working)]);
    let from_tool = cleanup_refusal("tguard", [("alpha", MemberStatus::Working)]);
    assert_eq!(from_tui, from_tool);
    assert!(from_tui.is_some());
}

// -- delete_confirmation -----------------------------------------------------
//
// Spec `teamssetup` T-028 / T-043 / FR-023 / FR-030 / FR-045 / F-28. The
// `force` override that passes `cleanup_refusal` is not itself the delete:
// `/team delete --force` on a live roster is refused with this text and only the
// second, identical command performs the removal. FR-045 extends the same
// requirement to a roster with nothing live: the directory is still removed for
// good, so every named team is confirmed.

#[test]
fn delete_confirmation_fires_even_when_cleanup_refusal_would_pass() {
    // FR-045: the two guards no longer partition the roster. A quiet team is
    // allowed by `cleanup_refusal` and still confirmed by `delete_confirmation`,
    // because what is left to lose is the directory rather than a session.
    let parked = [
        ("idle", MemberStatus::Idle),
        ("planning", MemberStatus::PlanPending),
        ("blocked", MemberStatus::Blocked),
        ("suspended", MemberStatus::Suspended),
        ("stopped", MemberStatus::Stopped),
        ("failed", MemberStatus::Failed),
    ];
    assert_eq!(cleanup_refusal("tguard", parked), None);
    let text = delete_confirmation("tguard", parked, [])
        .expect("a quiet roster is still a permanent deletion");
    assert!(text.contains("'tguard'"), "{text}");
    assert!(
        text.contains("permanently"),
        "the text must state what is removed: {text}"
    );
    assert!(text.contains("/team delete tguard --force"), "{text}");
    assert!(text.contains("Nothing has been deleted"), "{text}");
    // An empty roster - a team whose members have all been cleaned up - is
    // confirmed on the same rule: the directory exists and would be removed.
    assert!(
        delete_confirmation("tguard", [], []).is_some(),
        "an empty roster still has a directory to remove"
    );
}

#[test]
fn delete_confirmation_fires_for_every_status_the_cleanup_guard_refuses() {
    // Whatever the roster holds, a delete is always confirmed: the override is
    // never silently honoured and a quiet team is never deleted on one command.
    for status in ALL_STATUSES {
        let roster = [("alpha", status)];
        let confirmed = cleanup_refusal("tguard", roster).is_some();
        let text = delete_confirmation("tguard", roster, []).unwrap_or_else(|| {
            panic!("{status:?} must be confirmed (cleanup guard refuses it: {confirmed})")
        });
        assert!(text.contains("'tguard'"), "{status:?}: {text}");
        assert!(text.contains("--force"), "{status:?}: {text}");
        if confirmed {
            assert!(
                text.contains("alpha ["),
                "{status:?}: a live roster must name its blocking member: {text}"
            );
        }
    }
}

#[test]
fn delete_confirmation_is_none_for_an_excluded_team() {
    // An excluded name cannot be deleted at all, so no confirmation is offered
    // for it: a `Some(_)` here would let a caller fall through to a delete.
    let live = [("alpha", MemberStatus::Working)];
    assert_eq!(delete_confirmation("tguard", live, ["tguard"]), None);
    assert_eq!(
        delete_confirmation("tguard", [], ["tguard"]),
        None,
        "exclusion wins for a quiet roster too"
    );
    assert_eq!(
        delete_confirmation("tguard", live, ["other", "tguard"]),
        None,
        "exclusion is matched by name, not by position"
    );
    assert!(
        delete_confirmation("tguard", live, ["other"]).is_some(),
        "a different excluded name must not suppress the confirmation"
    );
}

// -- tool ---------------------------------------------------------------

mod tool {
    use std::sync::Arc;

    use ragent_agent::event::EventBus;
    use ragent_agent::team::{MemberStatus, TeamMember, TeamStore};
    use ragent_agent::tool::team_cleanup::TeamCleanupTool;
    use ragent_agent::tool::{Tool, ToolContext, create_default_registry};
    use serde_json::json;
    use tempfile::TempDir;

    /// Build a team directory with the given members and return a `ToolContext`
    /// rooted at its workspace.
    fn workspace_with(
        team: &str,
        members: &[(&str, &str, MemberStatus)],
    ) -> (TempDir, ToolContext) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let workspace = tmp.path().join("ws");
        std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");
        let mut store = TeamStore::create(team, "lead-1", &workspace, true).expect("create team");
        for (name, id, status) in members {
            let mut member = TeamMember::new(*name, *id, "general");
            member.status = *status;
            store.add_member(member).expect("add member");
        }
        store.save().expect("persist roster");
        drop(store);

        let ctx = ToolContext {
            session_id: "lead-1".to_string(),
            working_dir: workspace,
            event_bus: Arc::new(EventBus::new(64)),
            storage: None,
            agent_manager: None,
            active_model: None,
            provider_registry: None,
            team_context: None,
            team_manager: None,
            code_index: None,
            bg_service: None,
            spec_manager: None,
            active_spec_id: None,
            config: None,
            allowed_roots: Vec::new(),
            read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
            permission_checker: None,
            canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
            tool_registry: ToolContext::default_tool_registry(),
        };
        (tmp, ctx)
    }

    /// Run `team_cleanup` and return its result.
    async fn run_cleanup(ctx: &ToolContext, team: &str, force: bool) -> anyhow::Result<String> {
        let tool = TeamCleanupTool;
        let input = json!({ "team_name": team, "force": force });
        tool.execute(input, ctx).await.map(|out| out.content)
    }

    #[tokio::test]
    async fn tool_refuses_while_a_member_is_working() {
        let (_tmp, ctx) = workspace_with(
            "tguard",
            &[
                ("alpha", "tm-001", MemberStatus::Working),
                ("beta", "tm-002", MemberStatus::Idle),
            ],
        );
        let err = run_cleanup(&ctx, "tguard", false)
            .await
            .expect_err("a working member must block teardown");
        let text = err.to_string();
        // The refusal is the shared one, naming the member and its status.
        assert!(text.contains("alpha [working]"), "{text}");
        // The parked member is not blamed.
        assert!(!text.contains("beta"), "{text}");
        // The team directory survives.
        assert!(
            ragent_agent::team::find_team_dir(&ctx.working_dir, "tguard").is_some(),
            "a refused teardown must not delete the directory"
        );
    }

    #[tokio::test]
    async fn tool_refuses_while_a_member_is_shutting_down() {
        // `ShuttingDown` sounds terminal but the session still owes a
        // `team_shutdown_ack`; the pre-fix tool guard (`!= Stopped`) refused it
        // for the wrong reason and the pre-fix TUI guards allowed it outright.
        let (_tmp, ctx) =
            workspace_with("tguard", &[("alpha", "tm-001", MemberStatus::ShuttingDown)]);
        let err = run_cleanup(&ctx, "tguard", false)
            .await
            .expect_err("a shutting-down member must block teardown");
        assert!(err.to_string().contains("alpha [stopping]"));
    }

    #[tokio::test]
    async fn tool_proceeds_while_every_member_is_parked() {
        // FR-022's whole point: `Idle`, `PlanPending`, `Blocked`, and `Suspended`
        // are parked, have nothing in flight, and must not block teardown. The
        // pre-fix tool refused all four.
        let (_tmp, ctx) = workspace_with(
            "tguard",
            &[
                ("idle", "tm-001", MemberStatus::Idle),
                ("planning", "tm-002", MemberStatus::PlanPending),
                ("blocked", "tm-003", MemberStatus::Blocked),
                ("suspended", "tm-004", MemberStatus::Suspended),
            ],
        );
        let content = run_cleanup(&ctx, "tguard", false)
            .await
            .expect("parked members must not block teardown");
        assert!(content.contains("cleaned up"), "{content}");
        assert!(
            ragent_agent::team::find_team_dir(&ctx.working_dir, "tguard").is_none(),
            "a permitted teardown must remove the directory"
        );
    }

    #[tokio::test]
    async fn tool_proceeds_on_an_empty_roster() {
        let (_tmp, ctx) = workspace_with("tguard", &[]);
        run_cleanup(&ctx, "tguard", false)
            .await
            .expect("an empty roster must not refuse itself");
    }

    #[tokio::test]
    async fn force_still_overrides_the_shared_guard() {
        // `force: true` is the documented escape hatch; the shared guard must not
        // remove it. (T-041 makes `forcecleanup` subject to the same rule.)
        let (_tmp, ctx) = workspace_with("tguard", &[("alpha", "tm-001", MemberStatus::Working)]);
        let content = run_cleanup(&ctx, "tguard", true)
            .await
            .expect("force must override the guard");
        assert!(content.contains("cleaned up"), "{content}");
    }

    #[test]
    fn tool_is_registered_under_the_expected_name() {
        // The three-surface agreement is only meaningful if the third surface
        // actually exists in the default registry.
        let registry = create_default_registry();
        assert!(
            registry
                .definitions()
                .iter()
                .any(|d| d.name == "team_cleanup"),
            "team_cleanup must be in the default tool registry"
        );
        assert_eq!(TeamCleanupTool.name(), "team_cleanup");
    }
}
