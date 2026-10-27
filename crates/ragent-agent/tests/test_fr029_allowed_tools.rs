//! Guard: Claude-spelled `allowed_tools` entries resolve exactly, and an entry
//! that names no tool is reported (spec `uniformtools` FR-029).
//!
//! FR-029 is an *unwanted-behaviour* requirement with two clauses:
//!
//! 1. the thirteen Anthropic spellings the FR-023 rename adopts resolve against
//!    the registry **with no alias translation**, because the registry now holds
//!    those names;
//! 2. an entry that matches no registered tool is **reported once**, naming the
//!    entry and the skill that declared it, rather than silently shipping a
//!    smaller tool surface.
//!
//! | # | Case | Clause it pins |
//! |---|---|---|
//! | 1 | `test_every_adopted_claude_spelling_resolves_without_translation` | all 13 adopted spellings are in the registry, so the lookup alone resolves them |
//! | 2 | `test_the_rename_table_is_not_consulted_when_resolving_an_entry` | a retired spelling is unresolved, never mapped onto its `after` replacement |
//! | 3 | `test_a_skill_declaring_adopted_spellings_resolves_every_entry` | a whole Claude-shaped list resolves and reports nothing |
//! | 4 | `test_an_unknown_entry_is_reported_once_naming_the_entry_and_the_skill` | the report, its text, and its once-per-declarer shape |
//! | 5 | `test_a_second_skill_naming_the_same_entry_still_reports` | the ledger is per declarer, not per entry |
//! | 6 | `test_a_correct_manifest_is_silent` | no report for a manifest that resolves |
//! | 7 | `test_the_allowed_set_holds_the_entry_verbatim` | `build_allowed_tool_set` stores the entry, so the filter compares exact names |
//! | 8 | `test_an_adopted_spelling_matches_exactly_one_definition` | the filtered surface is one definition per adopted entry, never two |
//! | 9 | `test_skill_manage_reports_a_stale_entry_when_it_applies_the_restriction` | the `Skill` wiring consumes the ledger |
//! | 10 | `test_all_three_restriction_sites_report_and_the_prompt_path_does_not` | the other two sites (TUI `/skill`, forked resolver) report, and neither per-turn prompt path does |
//!
//! ## Why there is no `Read` case
//!
//! `Read` is deliberately absent from the adopted set: the derivation rated that
//! spelling `Close`, so `read` kept its name. A skill written for Claude Code that
//! names `Read` is therefore *reported*, not quietly answered by `read` - that is
//! case 2's sibling and is asserted through `Read` in case 3's negative half.
//!
//! ## What this file deliberately does not check
//!
//! - **The `tracing` output itself.** The ledger's returned count makes "once"
//!   assertable, and [`unresolved_entry_report`] makes the wording assertable
//!   without installing a subscriber. Installing one here would race the other
//!   children of this crate's single test binary.
//! - **The TUI's status line.** It shows the same registry answer through
//!   `effective_tool_defs`, which `test_prompt_units.rs` owns.

use std::sync::Arc;

use ragent_agent::agent::AgentInfo;
use ragent_agent::skill::SkillInfo;
use ragent_agent::tool::Tool;
use ragent_agent::tool::allowed_tools::{
    report_unresolved_allowed_tools_once, reset_unresolved_allowed_tools_for_test,
    unresolved_allowed_tools, unresolved_entry_report,
};
use ragent_agent::tool::create_default_registry;
use ragent_agent::tool::rename::TOOL_RENAMES;

/// FR-029's own list of adopted Anthropic spellings.
///
/// Written out rather than derived from [`TOOL_RENAMES`] because the requirement's
/// point is that *these* names resolve; deriving them would make the assertion
/// agree with whatever the table happened to hold.
const ADOPTED_CLAUDE_SPELLINGS: [&str; 13] = [
    "AskUserQuestion",
    "Bash",
    "Edit",
    "EnterPlanMode",
    "ExitPlanMode",
    "Glob",
    "Grep",
    "Skill",
    "TaskCreate",
    "TaskGet",
    "TaskList",
    "TaskUpdate",
    "Write",
];

fn entries(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_string()).collect()
}

// ---------------------------------------------------------------------------
// 1-2: exact resolution, no translation
// ---------------------------------------------------------------------------

#[test]
fn test_every_adopted_claude_spelling_resolves_without_translation() {
    let registry = create_default_registry();
    for spelling in ADOPTED_CLAUDE_SPELLINGS {
        assert!(
            registry.get(spelling).is_some(),
            "FR-029: `{spelling}` is an adopted Claude spelling and must be registered, \
             because a manifest naming it is resolved by the registry lookup alone"
        );
        assert_eq!(
            unresolved_allowed_tools(&registry, &entries(&[spelling])),
            Vec::<&str>::new(),
            "FR-029: `{spelling}` must resolve - an entry that needed translation would \
             mean the registry does not hold the name the rename adopted"
        );
    }

    // The same 13 spellings are exactly the `after` column of the rename table, so
    // no spelling can resolve through a registration that is not a rename target.
    let table: Vec<&str> = TOOL_RENAMES.iter().map(|pair| pair.after).collect();
    for spelling in ADOPTED_CLAUDE_SPELLINGS {
        assert!(
            table.contains(&spelling),
            "FR-029: `{spelling}` resolves but is not an `after` spelling of the FR-023 \
             table - the two lists must agree"
        );
    }
}

#[test]
fn test_the_rename_table_is_not_consulted_when_resolving_an_entry() {
    let registry = create_default_registry();
    for pair in TOOL_RENAMES {
        // The retired spelling names no tool, so it is unresolved...
        assert_eq!(
            unresolved_allowed_tools(&registry, &entries(&[pair.before])),
            vec![pair.before],
            "FR-029 (FR-026): the retired spelling `{}` must not resolve - translating it \
             onto `{}` would keep two vocabularies alive",
            pair.before,
            pair.after
        );
        // ...and the adopted spelling resolves, which is what makes the first half a
        // statement about the rename and not about a broken registry.
        assert_eq!(
            unresolved_allowed_tools(&registry, &entries(&[pair.after])),
            Vec::<&str>::new(),
            "FR-029: `{}` must resolve",
            pair.after
        );
    }

    // `Read` is the spelling the derivation rated `Close` and did not adopt; it is
    // unresolved, not answered by the unchanged `read`.
    assert_eq!(
        unresolved_allowed_tools(&registry, &entries(&["Read"])),
        vec!["Read"],
        "FR-029: `Read` was not adopted, so a manifest naming it is reported rather than \
         silently served by `read`"
    );
    assert!(
        registry.get("read").is_some(),
        "`read` itself is registered"
    );
}

// ---------------------------------------------------------------------------
// 3-6: the report
// ---------------------------------------------------------------------------

#[test]
#[serial_test::serial]
fn test_a_skill_declaring_adopted_spellings_resolves_every_entry() {
    reset_unresolved_allowed_tools_for_test();
    let registry = create_default_registry();
    // The TC-026 manifest shape: adopted spellings, plus the unchanged `read`.
    let mut skill = SkillInfo::new("rename-check", "Read notes.txt");
    skill.allowed_tools = entries(&["read", "Grep", "Glob"]);

    assert_eq!(
        unresolved_allowed_tools(&registry, &skill.allowed_tools),
        Vec::<&str>::new(),
        "FR-029: every entry of this manifest names a registered tool"
    );
    assert_eq!(
        report_unresolved_allowed_tools_once(
            &registry,
            &format!("skill `{}`", skill.name),
            &skill.allowed_tools
        ),
        0,
        "FR-029: a manifest that resolves is silent"
    );
}

#[test]
#[serial_test::serial]
fn test_an_unknown_entry_is_reported_once_naming_the_entry_and_the_skill() {
    reset_unresolved_allowed_tools_for_test();
    let registry = create_default_registry();
    // The TESTPLAN's own typo: `ReadFile` names nothing.
    let allowed = entries(&["read", "Grep", "Glob", "ReadFile"]);
    let declarer = "skill `rename-check`";

    assert_eq!(
        unresolved_allowed_tools(&registry, &allowed),
        vec!["ReadFile"],
        "FR-029: only the entry that names no tool is unresolved"
    );
    assert_eq!(
        report_unresolved_allowed_tools_once(&registry, declarer, &allowed),
        1,
        "FR-029: the first sight of the stale entry is reported"
    );
    assert_eq!(
        report_unresolved_allowed_tools_once(&registry, declarer, &allowed),
        0,
        "FR-029: 'once' means once - a per-run repeat is a warning a reader learns to ignore"
    );

    // The sentence names both halves FR-029 requires.
    let sentence = unresolved_entry_report(declarer, "ReadFile");
    assert!(
        sentence.contains("ReadFile"),
        "FR-029: the report names the entry; got: {sentence}"
    );
    assert!(
        sentence.contains("rename-check"),
        "FR-029: the report names the declaring skill; got: {sentence}"
    );
}

#[test]
#[serial_test::serial]
fn test_a_second_skill_naming_the_same_entry_still_reports() {
    reset_unresolved_allowed_tools_for_test();
    let registry = create_default_registry();
    let allowed = entries(&["ReadFile"]);

    assert_eq!(
        report_unresolved_allowed_tools_once(&registry, "skill `first`", &allowed),
        1
    );
    assert_eq!(
        report_unresolved_allowed_tools_once(&registry, "skill `second`", &allowed),
        1,
        "FR-029: the ledger is keyed per declarer too - a second skill has its own \
         manifest to fix, so suppressing it would hide a second defect"
    );

    // A *different* stale entry in the same skill reports as well.
    assert_eq!(
        report_unresolved_allowed_tools_once(&registry, "skill `first`", &entries(&["ReadFile"])),
        0,
        "the same (declarer, entry) pair is suppressed"
    );
    assert_eq!(
        report_unresolved_allowed_tools_once(&registry, "skill `first`", &entries(&["WriteFile"])),
        1,
        "a second distinct stale entry in the same skill is a second defect"
    );
}

#[test]
fn test_a_correct_manifest_is_silent() {
    let registry = create_default_registry();
    // An empty list and a fully-resolving list both report nothing, and neither
    // touches the ledger (so this test needs no serialisation).
    assert_eq!(
        report_unresolved_allowed_tools_once(&registry, "skill `empty`", &[]),
        0,
        "FR-029: a skill with no restriction declares nothing to resolve"
    );
    assert_eq!(
        report_unresolved_allowed_tools_once(
            &registry,
            "skill `bundled-debug`",
            &entries(&["Bash", "read", "Grep"])
        ),
        0,
        "FR-029: the bundled debug skill's own list resolves, so it must be silent"
    );
    for skill in ragent_agent::skill::bundled::bundled_skills() {
        assert_eq!(
            unresolved_allowed_tools(&registry, &skill.allowed_tools),
            Vec::<&str>::new(),
            "FR-029: every bundled skill's `allowed_tools` must resolve - `{}` does not",
            skill.name
        );
    }
}

// ---------------------------------------------------------------------------
// 7-8: the restriction itself is an exact-name filter
// ---------------------------------------------------------------------------

#[test]
fn test_the_allowed_set_holds_the_entry_verbatim() {
    let allowed = entries(&["Bash", "read"]);
    let set = ragent_agent::tool::build_allowed_tool_set(Some(allowed.as_slice()));
    assert!(
        set.contains("Bash") && set.contains("read"),
        "FR-029: an entry is stored exactly as declared, so the filter compares exact names"
    );
    assert!(
        !set.contains("bash"),
        "FR-029 (FR-026): the set must not gain a retired spelling - a second key would be \
         an alias by another route"
    );
}

#[test]
fn test_an_adopted_spelling_matches_exactly_one_definition() {
    let registry = create_default_registry();
    let defs = registry.definitions();
    for spelling in ADOPTED_CLAUDE_SPELLINGS {
        let matches: Vec<&str> = defs
            .iter()
            .filter(|d| d.name == spelling)
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(
            matches,
            vec![spelling],
            "FR-029: `{spelling}` must appear in the advertised definitions exactly once"
        );
    }

    // An agent whose allowance names no tool advertises no tool of its own, which
    // is the failure FR-029's report exists to make visible rather than silent.
    let mut agent = AgentInfo::new("typo-agent", "declares a stale entry");
    agent.allowed_tools = Some(entries(&["ReadFile"]));
    let allowed_set = ragent_agent::tool::build_allowed_tool_set(agent.allowed_tools.as_deref());
    // The set holds the entry verbatim (`test_the_allowed_set_holds_the_entry_verbatim`),
    // so the property that matters here is the *filter*: no definition is admitted by
    // a name that is not registered.
    assert!(
        !defs.iter().any(|d| d.name == "ReadFile"),
        "FR-029: the stale entry names no definition, so it can admit none"
    );
    assert!(
        !ragent_agent::tool::is_allowed_tool("read", &allowed_set),
        "FR-029: a stale entry matches nothing - it must not be translated onto `read`"
    );
    // The always-allowed control surface is still folded in, so the restriction is
    // a real restriction rather than an empty allowlist (which would mean "all").
    assert!(
        ragent_agent::tool::is_allowed_tool("think", &allowed_set),
        "FR-029: the always-allowed control tools stay available to a restricted agent"
    );
}

// ---------------------------------------------------------------------------
// 9-10: the report is wired where a restriction is actually applied
// ---------------------------------------------------------------------------

#[tokio::test]
#[serial_test::serial]
async fn test_skill_manage_reports_a_stale_entry_when_it_applies_the_restriction() {
    // FR-029 is only satisfied if the report happens at the place a skill's list
    // becomes a restriction. `skill_manage` is one of the three such places
    // (the TUI `/skill` trigger and the forked resolver are the others); this case
    // drives the tool end to end and proves it reported by observing that the
    // ledger already holds the entry when the test asks for it again.
    reset_unresolved_allowed_tools_for_test();
    let tmp = tempfile::tempdir().expect("tempdir");
    let skill_dir = tmp.path().join(".ragent/skills/fr029-typo");
    std::fs::create_dir_all(&skill_dir).expect("mkdir skill");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: fr029-typo\ndescription: declares one adopted and one stale entry\n\
         allowed-tools:\n  - Bash\n  - ReadFile\n---\n\nRead notes.txt and report the word.\n",
    )
    .expect("write SKILL.md");

    let registry = Arc::new(create_default_registry());
    let ctx = ragent_agent::tool::ToolContext {
        session_id: "fr029-sess".to_string(),
        working_dir: tmp.path().to_path_buf(),
        event_bus: Arc::new(ragent_agent::event::EventBus::new(16)),
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
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: Arc::clone(&registry),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    };

    let out = ragent_agent::tool::skill_manage::SkillManageTool
        .execute(
            serde_json::json!({"action": "read", "name": "fr029-typo"}),
            &ctx,
        )
        .await
        .expect("read the skill");
    assert!(
        out.content.contains("Read notes.txt"),
        "the skill prompt is still returned to the model: {}",
        out.content
    );

    // The tool reported the stale entry, so asking again is silent.
    assert_eq!(
        report_unresolved_allowed_tools_once(
            &registry,
            "skill `fr029-typo`",
            &entries(&["Bash", "ReadFile"]),
        ),
        0,
        "FR-029: `Skill` must report the stale entry when it applies the \
         restriction; the ledger already holding it here is the proof it did"
    );
}

// ---------------------------------------------------------------------------
// 10: all three restriction sites report, and no per-turn path does
// ---------------------------------------------------------------------------

/// The workspace root, resolved from this crate's manifest directory.
fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crates/<name> sits two levels below the workspace root")
        .to_path_buf()
}

fn read_workspace_file(relative: &str) -> String {
    let path = workspace_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

#[test]
fn test_all_three_restriction_sites_report_and_the_prompt_path_does_not() {
    // FR-029's second clause is only met if the report happens where a skill's
    // list becomes a *restriction*. `Skill` and the TUI `/skill` trigger are
    // driven end to end above and in `crates/ragent-tui/tests/test_skill_command.rs`;
    // the forked resolver needs a live `SessionProcessor`, so the wiring of all
    // three - and, more importantly, the *absence* of the fourth - is pinned by
    // reading the sources. A per-turn prompt path is the failure mode the
    // requirement warns about: it would warn about every discovered manifest,
    // including plugin skills the session never runs.
    let sites = [
        "crates/ragent-agent/src/tool/skill_manage.rs",
        "crates/ragent-tui/src/app/slash.rs",
        "crates/ragent-agent/src/skill/invoke.rs",
    ];
    for relative in sites {
        let source = read_workspace_file(relative);
        assert!(
            source.contains("report_skill_allowed_tools_once"),
            "{relative} must report a stale `allowed_tools` entry where it applies the \
             restriction (FR-029)"
        );
    }

    // The negative half: the paths that walk every manifest on every turn.
    for relative in [
        "crates/ragent-agent/src/session/prompt_builders.rs",
        "crates/ragent-agent/src/session/loop_steps.rs",
        "crates/ragent-tui/src/app/prompt.rs",
    ] {
        let source = read_workspace_file(relative);
        assert!(
            !source.contains("report_skill_allowed_tools_once")
                && !source.contains("report_unresolved_allowed_tools_once"),
            "{relative} builds the tool surface every turn; reporting there would warn \
             about manifests the session never runs and add a per-turn scan for an entry \
             that constrains nothing that turn (FR-029, NFR-006)"
        );
    }

    // One definition, in the module that owns the ledger.
    let module = read_workspace_file("crates/ragent-agent/src/tool/allowed_tools.rs");
    let definitions = module
        .lines()
        .filter(|line| line.starts_with("pub fn report_skill_allowed_tools_once"))
        .count();
    assert_eq!(
        definitions, 1,
        "the skill-declarer form must be defined exactly once, so all three sites share \
         one declarer text and the ledger key cannot disagree (FR-029)"
    );
}
