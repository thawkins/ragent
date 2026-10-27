//! Guard: permission categories, hook matchers and stored history survive the
//! rename (spec `uniformtools` FR-028).
//!
//! FR-028 is a *state-driven* requirement: it holds for the lifetime of a
//! configuration file or a saved session, both of which outlive the rename. It
//! names four duties, one per case group here:
//!
//! | # | Case | Clause it pins |
//! |---|---|---|
//! | 1 | `test_every_renamed_tool_keeps_the_permission_category_it_had_before_the_rename` | each of the 13 renamed tools still reports the category it reported pre-rename |
//! | 2 | `test_the_permission_category_vocabulary_is_unchanged` | the 32 distinct categories, including the two that collide with a retired tool name |
//! | 3 | `test_a_hook_matcher_naming_only_a_retired_spelling_is_reported_once` | the load-time report, and its once-per-entry-per-process shape |
//! | 4 | `test_a_hook_matcher_naming_the_adopted_spelling_or_no_matcher_is_silent` | the report is not noise for a correct matcher |
//! | 5 | `test_a_guard_matcher_contributes_its_bare_tool_name` | `bash(git commit:*)` reports, `Bash(git commit:*)` does not |
//! | 6 | `test_a_matcher_naming_several_retired_spellings_reports_each` | a list matcher reports every stale part |
//! | 7 | `test_a_stored_tool_call_record_replays_to_the_provider_verbatim` | `MessagePart::ToolCall` is not rewritten on the way out |
//! | 8 | `test_a_stored_record_survives_a_storage_round_trip_unrewritten` | the same for a persisted row |
//! | 9 | `test_an_unrecognised_stored_tool_name_is_classified_non_destructive` | a name the classifier does not know is not destructive, and never panics |
//!
//! ## Why the categories are asserted as literals
//!
//! `permission_category()` is permission *vocabulary*, derived independently of
//! the tool name. A tool that was renamed keeps its category, so the categories
//! cannot be derived from the FR-023 table - deriving them would make the
//! assertion agree with whatever the tree happened to do, which is the one thing
//! FR-028 forbids. The expected values below are therefore written out, and the
//! two traps are called out where they bite:
//!
//! - the family `ask_user` holds a tool now called `AskUserQuestion`;
//! - the family `bash` holds a tool now called `Bash`, and reports
//!   `bash:execute`.
//!
//! Both look like a missed migration and are the requirement. `T-041-BASELINE.md`
//! records the same two as deliberate.
//!
//! ## What this file deliberately does not check
//!
//! - **The per-family counts.** `test_tool_rename_atomic_landing.rs` case 5 owns
//!   those (21 families, the `agent` 4 / `team` 19 canaries - the split
//!   `permission_category()` gives, since `team_wait` reports `agent:spawn`).
//!   Repeating them here would be two guards to keep in step.
//! - **`permission.rs`'s `Permission` spellings.** Those are the permission
//!   *type* vocabulary (`Read`, `Bash`, `Edit`, ...) parsed from a rule's
//!   `permission` field, one step removed from a category, and FR-028 keeps them
//!   unchanged too - but they are covered by `ragent-config`'s own tests, and the
//!   escape-hatch audit records them as a reasoned exemption rather than a site
//!   to migrate.
//! - **The on-disk bytes of a stored record.** Case 8 round-trips through
//!   storage and compares the deserialised name, which is the property FR-028
//!   states ("replayed to the provider verbatim, unrewritten"). Asserting the
//!   exact JSON would pin `MessagePart`'s serde tag, which is not this task's
//!   subject.

use ragent_agent::hooks::{
    HookConfig, HookTrigger, report_retired_matcher_spellings_once,
    reset_retired_matcher_spellings_for_test, retired_matcher_report,
    retired_matcher_spellings_for_test,
};
use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use ragent_agent::session::history::history_to_chat_messages;
use ragent_agent::session::permissions::is_destructive_tool_for_test;
use ragent_agent::storage::Storage;
use ragent_agent::tool::create_default_registry;
use ragent_agent::tool::rename::TOOL_RENAMES;
use ragent_types::llm::{ChatContent, ContentPart};
use serde_json::json;
use std::collections::BTreeSet;

/// The `permission_category()` every renamed tool reported *before* the rename
/// and must still report after it (FR-028).
///
/// Keyed by the adopted (`after`) spelling, because that is the name the registry
/// now holds. Nine of the thirteen share a category with another tool, so this
/// table is also the statement that a rename moved a *name* only.
const CATEGORY_AFTER_RENAME: [(&str, &str); 13] = [
    ("AskUserQuestion", "ask_user"),
    ("Bash", "bash:execute"),
    ("Edit", "file:write"),
    ("EnterPlanMode", "plan"),
    ("ExitPlanMode", "plan"),
    ("Glob", "file:read"),
    ("Grep", "file:read"),
    ("Skill", "skill:manage"),
    ("TaskCreate", "task"),
    ("TaskGet", "task"),
    ("TaskList", "task"),
    ("TaskUpdate", "task"),
    ("Write", "file:write"),
];

/// The 32 distinct `permission_category()` values across the 148 tools.
///
/// Recorded in `specs/uniformtools/T-041-BASELINE.md` section 4 ("32 distinct
/// `permission_category()` values across the 148, collapsing to the 21
/// families"). A rename adds, removes and re-spells nothing here - which is why
/// `ask_user` and `bash:execute` are still present even though the *tools* that
/// report them are now called `AskUserQuestion` and `Bash`.
const CATEGORY_VOCABULARY: [&str; 32] = [
    "agent:control",
    "agent:spawn",
    "ask_user",
    "bash:execute",
    "codeindex:read",
    "codeindex:write",
    "cron:read",
    "cron:write",
    "file:read",
    "file:write",
    "git:read",
    "git:write",
    "github:read",
    "github:write",
    "gitlab:read",
    "gitlab:write",
    "model:read",
    "network:fetch",
    "none",
    "plan",
    "shell:execute",
    "skill:manage",
    "spec:read",
    "spec:write",
    "storage:write",
    "system",
    "task",
    "team:communicate",
    "team:manage",
    "team:read",
    "team:tasks",
    "web",
];

/// A hook config with a `matcher` and a command that never runs in this file.
fn hook_with_matcher(matcher: &str) -> HookConfig {
    HookConfig {
        trigger: HookTrigger::PreToolUse,
        command: "true".to_string(),
        timeout_secs: 1,
        plugin_root: None,
        matcher: Some(matcher.to_string()),
    }
}

/// The `(retired spelling, adopted spelling)` pairs one matcher reports.
///
/// Goes through the public accessor the warning is built on, so a change to the
/// splitting or the lookup shows up here rather than only in a log line.
fn reported_pairs(matcher: &str) -> Vec<(String, String)> {
    retired_matcher_spellings_for_test(&hook_with_matcher(matcher))
}

// ---------------------------------------------------------------------------
// 1-2: permission categories are unchanged
// ---------------------------------------------------------------------------

#[test]
fn test_every_renamed_tool_keeps_the_permission_category_it_had_before_the_rename() {
    let registry = create_default_registry();
    for (after, expected) in CATEGORY_AFTER_RENAME {
        let tool = registry
            .get(after)
            .unwrap_or_else(|| panic!("FR-028: `{after}` is not registered"));
        assert_eq!(
            tool.permission_category(),
            expected,
            "FR-028: `{after}` reports the category {:?}, not the {expected:?} it reported \
             before the rename. The category is permission vocabulary, derived \
             independently of the tool name - a rename must not move it, because a \
             `permission` rule written against {expected:?} has to keep working.",
            tool.permission_category()
        );
    }
}

#[test]
fn test_the_permission_category_vocabulary_is_unchanged() {
    let registry = create_default_registry();
    let tools = registry.all_tools();
    let seen: BTreeSet<&str> = tools
        .iter()
        .map(|tool| tool.permission_category())
        .collect();
    let expected: BTreeSet<&str> = CATEGORY_VOCABULARY.into_iter().collect();
    assert_eq!(
        seen,
        expected,
        "FR-028: the permission_category() vocabulary changed. Missing: {:?}; unexpected: {:?}",
        expected.difference(&seen).collect::<Vec<_>>(),
        seen.difference(&expected).collect::<Vec<_>>()
    );

    // The two collisions, asserted by name because they are the traps: both read
    // like a missed migration and are the requirement.
    assert!(
        seen.contains("ask_user"),
        "FR-028: the family `ask_user` must survive the rename of the tool \
         `AskUserQuestion` - the family is permission vocabulary, not a tool name"
    );
    assert!(
        seen.contains("bash:execute"),
        "FR-028: the family `bash` must survive the rename of the tool `Bash`"
    );
}

// ---------------------------------------------------------------------------
// 3-6: a hook matcher naming a retired spelling is reported
// ---------------------------------------------------------------------------

#[test]
#[serial_test::serial]
fn test_a_hook_matcher_naming_only_a_retired_spelling_is_reported_once() {
    // The ledger is process-wide and this crate's test binary is one process, so
    // an unrelated test that reports the same pair would steal the first report.
    reset_retired_matcher_spellings_for_test();
    let hooks = [hook_with_matcher("bash")];

    assert_eq!(
        reported_pairs("bash"),
        vec![("bash".to_string(), "Bash".to_string())],
        "FR-028: the matcher names the retired spelling and its replacement"
    );
    // The sentence the warning emits has to name the spelling, its replacement
    // and the trigger, or a reader cannot fix the file it is about.
    let sentence = retired_matcher_report("bash", "Bash", "pre_tool_use");
    for needle in ["bash", "Bash", "pre_tool_use"] {
        assert!(
            sentence.contains(needle),
            "FR-028: the report must name {needle:?}; got {sentence:?}"
        );
    }
    assert_eq!(
        report_retired_matcher_spellings_once(&hooks),
        1,
        "the first sight of a stale matcher is reported"
    );
    assert_eq!(
        report_retired_matcher_spellings_once(&hooks),
        0,
        "a per-turn repeat is a warning a reader learns to ignore, so the ledger \
         suppresses the same entry"
    );

    // The adopted spelling is a *different* entry, so a config carrying both
    // reports the second one rather than treating the pair as seen.
    let both = [hook_with_matcher("bash"), hook_with_matcher("write")];
    assert_eq!(
        report_retired_matcher_spellings_once(&both),
        1,
        "a new stale entry still reports"
    );
}

#[test]
fn test_a_hook_matcher_naming_the_adopted_spelling_or_no_matcher_is_silent() {
    let registry_names: Vec<&str> = TOOL_RENAMES.iter().map(|pair| pair.after).collect();
    // Every adopted spelling a matcher may legitimately name.
    for after in &registry_names {
        assert_eq!(
            reported_pairs(after),
            Vec::new(),
            "FR-028: a matcher naming the adopted spelling `{after}` is correct and must \
             not be reported"
        );
    }
    // A hook with no matcher applies to every tool; nothing to report.
    let no_matcher = HookConfig {
        matcher: None,
        ..hook_with_matcher("unused")
    };
    assert_eq!(
        retired_matcher_spellings_for_test(&no_matcher),
        Vec::new(),
        "a hook with no matcher names no spelling"
    );
    // A name the rename never touched is not a rename problem either, and nor is
    // an empty or whitespace-only matcher - the splitter treats one as "matches
    // everything", which no renamed tool is.
    assert_eq!(
        reported_pairs("think"),
        Vec::new(),
        "`think` was not renamed"
    );
    assert_eq!(reported_pairs("read"), Vec::new(), "`read` was not renamed");
    assert_eq!(reported_pairs(""), Vec::new(), "an empty matcher is silent");
    assert_eq!(
        reported_pairs("  "),
        Vec::new(),
        "a whitespace-only matcher is silent"
    );
}

#[test]
fn test_a_guard_matcher_contributes_its_bare_tool_name() {
    // The Claude guard form: only the name before `(` is a tool name, and the
    // pattern is matched against arguments. An adopted spelling is silent ...
    assert_eq!(
        reported_pairs("Bash(git commit:*)"),
        Vec::new(),
        "FR-028: `Bash(git commit:*)` names the adopted spelling"
    );
    // ... while a stale one is reported, with the guard left out of the name.
    assert_eq!(
        reported_pairs("bash(git commit:*)"),
        vec![("bash".to_string(), "Bash".to_string())],
        "FR-028: the guard pattern is not part of the tool name"
    );
}

#[test]
fn test_a_matcher_naming_several_retired_spellings_reports_each() {
    assert_eq!(
        reported_pairs("bash|write,task_create"),
        vec![
            ("bash".to_string(), "Bash".to_string()),
            ("write".to_string(), "Write".to_string()),
            ("task_create".to_string(), "TaskCreate".to_string()),
        ],
        "FR-028: every stale part of a list matcher is reported, in matcher order"
    );
    // A mixed list reports only the stale parts.
    assert_eq!(
        reported_pairs("Edit|grep|Write"),
        vec![("grep".to_string(), "Grep".to_string())],
        "FR-028: an adopted spelling beside a stale one is not reported"
    );
}

// ---------------------------------------------------------------------------
// 7-8: stored ToolCall records replay verbatim
// ---------------------------------------------------------------------------

/// An assistant message whose sole part is a tool call under `tool`.
fn stored_tool_call(tool: &str) -> Message {
    Message::new(
        "sess-fr028",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: tool.to_string(),
            call_id: "call-1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"command": "ls"}),
                output: Some(json!("file.txt")),
                error: None,
                duration_ms: Some(7),
            }),
        }],
    )
}

/// The tool name the provider would see for the first `ToolUse` of `messages`.
async fn replayed_tool_name(messages: &[Message]) -> String {
    let chat = history_to_chat_messages(messages).await;
    let mut names = Vec::new();
    for message in &chat {
        if let ChatContent::Parts(parts) = &message.content {
            for part in parts {
                if let ContentPart::ToolUse { name, .. } = part {
                    names.push(name.clone());
                }
            }
        }
    }
    assert_eq!(
        names.len(),
        1,
        "the stored record produces exactly one ToolUse part, got {names:?}"
    );
    names.remove(0)
}

#[tokio::test]
async fn test_a_stored_tool_call_record_replays_to_the_provider_verbatim() {
    // FR-028: a record written before the rename replays verbatim. The retired
    // spelling is what the pre-rename build actually sent the provider, and a
    // continued turn has to match it: rewriting the name here would make the
    // assistant's tool_use disagree with the tool_result that answers it.
    assert_eq!(
        replayed_tool_name(&[stored_tool_call("bash")]).await,
        "bash",
        "FR-028: the stored name is replayed unrewritten"
    );
    // The same holds for every retired spelling, not just the shell tool.
    for pair in TOOL_RENAMES {
        assert_eq!(
            replayed_tool_name(&[stored_tool_call(pair.before)]).await,
            pair.before,
            "FR-028: `{}` replays unrewritten",
            pair.before
        );
    }
    // And a name that no longer exists at all is replayed as it was stored.
    assert_eq!(
        replayed_tool_name(&[stored_tool_call("tool_that_never_existed")]).await,
        "tool_that_never_existed",
        "FR-028: replay does not consult the registry"
    );
}

#[test]
fn test_a_stored_record_survives_a_storage_round_trip_unrewritten() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-fr028", "/tmp/fr028")
        .expect("create session");
    let stored = stored_tool_call("bash");
    storage.create_message(&stored).expect("store message");

    let reloaded = storage.get_messages("sess-fr028").expect("read messages");
    assert_eq!(reloaded.len(), 1);
    let tool = reloaded[0]
        .parts
        .iter()
        .find_map(|part| match part {
            MessagePart::ToolCall { tool, .. } => Some(tool.clone()),
            _ => None,
        })
        .expect("the reloaded message still holds its ToolCall part");
    assert_eq!(
        tool, "bash",
        "FR-028: a persisted record keeps the spelling it was written with - a resumed \
         session must not be rewritten on read, on disk or in memory"
    );
}

// ---------------------------------------------------------------------------
// 9: an unknown stored name is non-destructive
// ---------------------------------------------------------------------------

#[test]
fn test_an_unrecognised_stored_tool_name_is_classified_non_destructive() {
    // FR-028: a tool-name-keyed classifier that does not recognise a stored name
    // treats it as non-destructive rather than failing or panicking. The
    // destructive-action checkpoint is the classifier that matters: a stored
    // record replayed into a resumed loop passes its name to it on every call.
    let destructive_bash = json!({"command": "rm -rf target"});
    assert!(
        is_destructive_tool_for_test("Bash", &destructive_bash),
        "the adopted spelling still reaches the shell branch"
    );
    assert!(
        !is_destructive_tool_for_test("bash", &destructive_bash),
        "FR-028: a retired spelling is an unrecognised name now, so it is treated as \
         non-destructive - the classifier may not invent a mapping the dispatch path \
         does not have, and it must not fail"
    );
    assert!(!is_destructive_tool_for_test(
        "tool_that_never_existed",
        &json!({})
    ));
    // An unknown name must not be read as a git or config tool either, even with
    // arguments that would make one destructive.
    assert!(!is_destructive_tool_for_test(
        "gti_push",
        &json!({"force": true})
    ));
    assert!(!is_destructive_tool_for_test(
        "Write",
        &json!({"path": "src/main.rs"})
    ));
}
