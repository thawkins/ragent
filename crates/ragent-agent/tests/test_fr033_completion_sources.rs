//! Guard: every surface that offers a tool name is registry-derived, so no
//! retired spelling can be completed (spec `uniformtools` FR-033).
//!
//! FR-033 names the sources a suggestion can be drawn from, and the rule is the
//! same for all of them: the *registry* is the catalogue, and a retired `before`
//! spelling appears in none of them. This file pins the three sources reachable
//! from `ragent-agent`:
//!
//! | # | Case | Source it pins |
//! |---|---|---|
//! | 1 | `test_the_did_you_mean_pool_is_the_registry_not_the_rename_table` | the FR-021 candidate pool |
//! | 2 | `test_no_retired_spelling_is_ever_suggested` | the same pool, against the table |
//! | 3 | `test_tool_info_names_are_the_registry_it_was_handed` | `tool_info` |
//! | 4 | `test_the_dry_run_tool_section_is_built_from_the_registry_it_was_handed` | `--dry-run` |
//! | 5 | `test_commands_info_publishes_the_one_tools_subcommand_table` | `commands_info` |
//! | 6 | `test_the_prompt_tool_reference_is_the_definitions_it_was_handed` | the system-prompt tool list |
//!
//! The fourth source - the slash-command and `/tools` menus - is a TUI render
//! path and is pinned beside it in
//! `crates/ragent-tui/tests/test_fr033_completion_surfaces.rs`.
//!
//! ## Why "derived" is asserted by handing each surface a foreign registry
//!
//! A case that only reads the default registry cannot tell a derived surface from
//! a hard-coded one that happens to agree today: both report 148 names that carry
//! no `before` spelling. Every surface below is therefore *driven with a registry
//! it has never seen* - one stub tool, one made-up name - and the surface has to
//! report that name. A surface that reads a table, a constant or a baseline
//! document keeps reporting 148 and fails.
//!
//! ## What this file deliberately does not check
//!
//! - **That the enumerations agree on the 148-name surface.** FR-030's
//!   `test_fr030_surface_agreement.rs` owns that, element for element. This file
//!   asserts the *derivation* property, not the size.
//! - **That the dispatch path refuses a retired spelling.** FR-021/FR-026's
//!   refusal is `test_tool_rename_atomic_landing.rs` and the dispatch tests. This
//!   file reads the pool that refusal draws its suggestion from, which is the half
//!   FR-033 adds: a refusal whose hint names the dead spelling teaches it back.
//! - **That the slash menu offers nothing dead.** That is the TUI file; the two
//!   halves are split exactly where T-037 split the FR-030 guard.

#![allow(unsafe_code)]
// The dry-run case holds the crate-wide env lock across the report's `await` so the
// ambient configuration stays pinned for the whole run. One consolidated test binary
// on one thread per test: no other task in this test contends for the lock, so it
// cannot deadlock.
#![allow(clippy::await_holding_lock)]

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::sync::Arc;

use async_trait::async_trait;
use ragent_agent::dry_run::DryRunInputs;
use ragent_agent::event::EventBus;
use ragent_agent::session::dispatch::suggest_tool_name;
use ragent_agent::tool::command_catalog::{COMMAND_CATALOG, TOOLS_SUBS};
use ragent_agent::tool::rename::TOOL_RENAMES;
use ragent_agent::tool::{Tool, ToolContext, ToolOutput, ToolRegistry, create_default_registry};
use serde_json::{Value, json};

/// The retired `before` spellings, as one set (FR-023).
fn retired() -> BTreeSet<&'static str> {
    TOOL_RENAMES.iter().map(|pair| pair.before).collect()
}

// ---------------------------------------------------------------------------
// A stub tool, so a surface can be handed a registry it has never seen
// ---------------------------------------------------------------------------

/// A registered tool whose only job is to carry a name the default registry does
/// not have.
struct StubTool(&'static str);

#[async_trait]
impl Tool for StubTool {
    fn name(&self) -> &str {
        self.0
    }

    fn description(&self) -> &'static str {
        "stub"
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    fn permission_category(&self) -> &'static str {
        "none"
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        Ok(ToolOutput {
            content: "stub".to_string(),
            metadata: None,
        })
    }
}

/// A registry holding exactly `names`, in descending order so nothing about the
/// insertion order can stand in for the derived order.
fn registry_of(names: &[&'static str]) -> Arc<ToolRegistry> {
    let registry = ToolRegistry::new();
    for name in names.iter().rev() {
        registry.register(Arc::new(StubTool(name)));
    }
    Arc::new(registry)
}

/// A `ToolContext` whose tool registry is `registry`.
fn ctx_with(registry: Arc<ToolRegistry>) -> ToolContext {
    ToolContext {
        session_id: "fr033".to_string(),
        working_dir: std::path::PathBuf::from("target/temp"),
        event_bus: Arc::new(EventBus::new(16)),
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
        tool_registry: registry,
    }
}

// ---------------------------------------------------------------------------
// 1 and 2. The did-you-mean candidate pool (FR-021)
// ---------------------------------------------------------------------------

#[test]
fn test_the_did_you_mean_pool_is_the_registry_not_the_rename_table() {
    // The pool is the registry, proved by handing it one. If `suggest_tool_name`
    // drew its candidates from anything else - the rename table, a static tool
    // list, the FR-030 baseline - this makes up a name that appears in none of
    // them and asks for it back.
    let registry = registry_of(&["zzq_probe_tool"]);
    assert_eq!(
        suggest_tool_name(&registry, "zzq_probe_tol").as_deref(),
        Some("zzq_probe_tool"),
        "FR-033: the did-you-mean pool did not offer the one tool the registry \
         holds, so it is not derived from the registry"
    );

    // An empty registry has nothing to offer: a hard-coded pool would still
    // answer.
    let empty = registry_of(&[]);
    assert_eq!(
        suggest_tool_name(&empty, "zzq_probe_tol"),
        None,
        "FR-033: an empty registry still produced a suggestion, so the pool is \
         not the registry"
    );

    // And the pool is not the rename table either: a name one edit away from a
    // retired spelling, in a registry holding only that spelling's *replacement*,
    // must suggest the replacement - the name that is registered - never the
    // retired one. `Bash` and `bash` are the pair whose spellings are the same
    // letters, so this is the sharpest case.
    let bash_only = registry_of(&["Bash"]);
    for typed in ["Bash", "bsh", "Bas"] {
        let suggestion = suggest_tool_name(&bash_only, typed);
        assert_eq!(
            suggestion.as_deref(),
            Some("Bash"),
            "FR-033: typing {typed:?} against a registry holding `Bash` suggested \
             {suggestion:?}"
        );
    }
}

#[test]
fn test_no_retired_spelling_is_ever_suggested() {
    let registry = create_default_registry();

    // The pool may not *answer* with a retired spelling, for any input - including
    // the retired spelling itself, which is what a model typing the old vocabulary
    // produces.
    for pair in TOOL_RENAMES {
        let suggestion = suggest_tool_name(&registry, pair.before);
        assert_ne!(
            suggestion.as_deref(),
            Some(pair.before),
            "FR-033/FR-021: typing the retired spelling `{}` was answered with that \
             same spelling, so the hint teaches the dead name back",
            pair.before
        );
    }

    // Whatever the pool answers with is a name that resolves: the hint can never
    // point at a spelling that then fails. The pool ranks structural matches
    // (equality, prefix, containment) above edit distance, so a retired spelling
    // that shares a word with an unrelated registered tool (`task_create` and the
    // `create` tool) is answered by that tool rather than by `TaskCreate` - a
    // weaker hint, but never a dead one. FR-033 forbids the dead answer, not a
    // suboptimal one, so this case asserts reachability and leaves FR-021's ranking
    // policy to its own guard.
    for pair in TOOL_RENAMES {
        for typed in [pair.before, pair.after] {
            if let Some(suggestion) = suggest_tool_name(&registry, typed) {
                assert!(
                    registry.get(&suggestion).is_some(),
                    "FR-033: typing {typed:?} produced the suggestion {suggestion:?}, \
                     which the registry does not hold - the model would be told to \
                     call a name that fails"
                );
                assert!(
                    !retired().contains(suggestion.as_str()),
                    "FR-033: typing {typed:?} produced the suggestion {suggestion:?}, \
                     which is a retired spelling"
                );
            }
        }
    }

    // Nothing reachable from the pool's *output* is a retired spelling, over the
    // whole registry: every registered name is a suggestion candidate, so this is
    // the statement that the candidate set itself is clean.
    let names = registry.list();
    let retired = retired();
    let leaked: Vec<&String> = names
        .iter()
        .filter(|n| retired.contains(n.as_str()))
        .collect();
    assert!(
        leaked.is_empty(),
        "FR-033: the pool holds retired spellings {leaked:?}, so it can suggest one"
    );
}

// ---------------------------------------------------------------------------
// 3. `tool_info`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tool_info_names_are_the_registry_it_was_handed() {
    let registry = registry_of(&["zzq_alpha", "zzq_beta"]);
    let tool = create_default_registry()
        .get("tool_info")
        .expect("tool_info is the tool under test");

    let output = tool
        .execute(json!({}), &ctx_with(Arc::clone(&registry)))
        .await
        .expect("execute");
    let parsed: Value = serde_json::from_str(&output.content).expect("tool_info returns JSON");

    assert_eq!(
        parsed["total"].as_u64(),
        Some(2),
        "FR-033: `tool_info` reported {:?} tools for a registry holding two, so it \
         is not reading the registry it was handed",
        parsed["total"]
    );
    let mut names: Vec<String> = parsed["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .map(|entry| {
            entry["name"]
                .as_str()
                .expect("every entry names its tool")
                .to_string()
        })
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec!["zzq_alpha".to_string(), "zzq_beta".to_string()],
        "FR-033: `tool_info` enumerated a different set of tools than the registry \
         it was handed"
    );

    // And against the live registry it names no retired spelling.
    let live = Arc::new(create_default_registry());
    let live_names = live.list();
    for before in retired() {
        assert!(
            !live_names.iter().any(|name| name == before),
            "FR-033: the registry `tool_info` publishes still holds `{before}`"
        );
    }
}

// ---------------------------------------------------------------------------
// 4. `--dry-run`
// ---------------------------------------------------------------------------

/// Points `XDG_CONFIG_HOME` at an empty temp dir and clears the two config
/// override variables for as long as the guard lives, so the ambient
/// configuration cannot take part in the report.
struct EnvGuard {
    saved: Vec<(&'static str, Option<OsString>)>,
}

impl EnvGuard {
    fn isolating(xdg: &std::path::Path) -> Self {
        let keys = ["XDG_CONFIG_HOME", "RAGENT_CONFIG", "RAGENT_CONFIG_CONTENT"];
        let saved: Vec<(&'static str, Option<OsString>)> = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect();
        // SAFETY: process-global mutation, serialised by `#[serial]` and the
        // crate-wide env lock held for this guard's lifetime.
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", xdg);
            std::env::remove_var("RAGENT_CONFIG");
            std::env::remove_var("RAGENT_CONFIG_CONTENT");
        }
        Self { saved }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (key, value) in &self.saved {
            // SAFETY: restoring the value captured in `isolating`.
            unsafe {
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
    }
}

#[tokio::test]
#[serial_test::serial]
async fn test_the_dry_run_tool_section_is_built_from_the_registry_it_was_handed() {
    // `std::sync::Mutex` is not reentrant: this test takes the crate-wide env
    // lock once, for as long as the guard below lives, and never again.
    let _lock = crate::global_env_lock();
    let config_home = tempfile::tempdir().expect("config-home tempdir");
    let project = tempfile::tempdir().expect("project tempdir");
    let _env = EnvGuard::isolating(config_home.path());
    let config_path = project.path().join("ragent.json");
    std::fs::write(
        &config_path,
        r#"{ "defaultAgent": "general", "provider": { "anthropic": { "env": [] } } }"#,
    )
    .expect("write config");

    let registry = registry_of(&["zzq_alpha", "zzq_beta", "zzq_gamma"]);
    let inputs = DryRunInputs {
        config_path: Some(config_path),
        agent_name: "general".to_string(),
        model_override: None,
        provider_registry: Arc::new(ragent_agent::provider::create_default_registry()),
        tool_registry: Arc::clone(&registry),
        working_dir: project.path().to_path_buf(),
    };
    let (report, _exit) = ragent_agent::dry_run::run_dry_run(inputs).await;

    let section = report
        .sections
        .iter()
        .find(|section| section.name == "tools")
        .expect("the readiness report has a `tools` section");
    let total = section
        .items
        .iter()
        .find(|item| item.name == "total")
        .expect("the `tools` section reports a total");
    assert_eq!(
        total.message, "3 visible tools",
        "FR-033: `--dry-run` reported {:?} for a registry holding three tools, so \
         it is not reading the registry it was handed",
        total.message
    );

    // The section's row *names* are permission families, never tool names: they
    // come from `permission_category()`, which FR-028 keeps independent of the
    // name. That is why a retired spelling can appear here as a family (`bash`) and
    // must never appear as a tool (`Bash`), and why the report cannot offer one.
    let row_names: Vec<&str> = section
        .items
        .iter()
        .map(|item| item.name.as_str())
        .filter(|name| *name != "total")
        .collect();
    let registry_families: BTreeSet<String> = registry
        .list()
        .iter()
        .filter_map(|name| registry.get(name))
        .map(|tool| {
            let category = tool.permission_category();
            category.split(':').next().unwrap_or(category).to_string()
        })
        .collect();
    for name in &row_names {
        assert!(
            registry_families.contains(*name),
            "FR-033: the `--dry-run` tools section reported {name:?}, which is not a \
             permission family of the registry it was handed"
        );
    }
    assert!(
        !row_names.contains(&"Bash"),
        "FR-033: the `--dry-run` tools section listed the tool `Bash` as an item; \
         the section enumerates families, so a tool name here means it stopped \
         deriving from `permission_category()`"
    );
}

// ---------------------------------------------------------------------------
// 5. `commands_info`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_commands_info_publishes_the_one_tools_subcommand_table() {
    let registry = Arc::new(create_default_registry());
    let tool = registry
        .get("commands_info")
        .expect("commands_info is the tool under test");
    let output = tool
        .execute(json!({}), &ctx_with(Arc::clone(&registry)))
        .await
        .expect("execute");
    let parsed: Value = serde_json::from_str(&output.content).expect("commands_info returns JSON");
    let builtin = parsed["builtin"].as_array().expect("builtin array");

    // `/tools` is the one catalogue entry whose subject is the tool surface. Its
    // subcommands must be the single `TOOLS_SUBS` table the TUI menu and the
    // `/tools` handler also read - the mirror may not carry a fourth copy.
    let tools_entry = builtin
        .iter()
        .find(|entry| entry["trigger"] == "tools")
        .expect("FR-033: the catalogue lists `/tools`");
    let published: Vec<&str> = tools_entry["subcommands"]
        .as_array()
        .expect("subcommands array")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(
        published, TOOLS_SUBS,
        "FR-033: `commands_info` publishes {published:?} for `/tools`, not the \
         single `TOOLS_SUBS` table the menu and the handler read"
    );

    // No catalogue entry offers a retired tool spelling in a suggestion column,
    // unless the same word is also a slash command - slash vocabulary and tool
    // vocabulary share words, and FR-033 governs only surfaces that offer a *tool*
    // name (`/bash` is a command, not the retired `bash` tool).
    let triggers: BTreeSet<&str> = builtin
        .iter()
        .filter_map(|entry| entry["trigger"].as_str())
        .collect();
    let retired = retired();
    for entry in builtin {
        let trigger = entry["trigger"].as_str().expect("trigger");
        for field in ["subcommands", "flags"] {
            for token in entry[field]
                .as_array()
                .expect("array of strings")
                .iter()
                .filter_map(Value::as_str)
            {
                if retired.contains(token) {
                    assert!(
                        triggers.contains(token),
                        "FR-033: the `/{trigger}` entry offers `{token}` as a \
                         {field} suggestion, and `{token}` is a retired tool \
                         spelling that is not a slash command either"
                    );
                }
            }
        }
    }

    // Every entry in the catalogue is a slash command the TUI registers, so no
    // entry can be a stale copy that a completion list would offer.
    let tui: BTreeSet<&str> = ragent_agent::tool::command_catalog::COMMAND_CATALOG
        .iter()
        .map(|entry| entry.trigger)
        .collect();
    assert_eq!(
        tui.len(),
        COMMAND_CATALOG.len(),
        "FR-033: the catalogue repeats a trigger, so one command shadows another"
    );
}

// ---------------------------------------------------------------------------
// 6. The system-prompt tool list
// ---------------------------------------------------------------------------

#[test]
fn test_the_prompt_tool_reference_is_the_definitions_it_was_handed() {
    use ragent_agent::session::prompt_builders::build_tool_reference_from_defs;

    let defs = vec![
        ragent_types::ToolDefinition {
            name: "zzq_alpha".to_string(),
            description: "first stub".to_string(),
            parameters: json!({ "type": "object", "properties": {} }),
        },
        ragent_types::ToolDefinition {
            name: "zzq_beta".to_string(),
            description: "second stub".to_string(),
            parameters: json!({ "type": "object", "properties": {} }),
        },
    ];
    let section = build_tool_reference_from_defs(&defs);
    assert!(
        section.contains("`zzq_alpha`") && section.contains("`zzq_beta`"),
        "FR-033: the prompt tool reference did not name the definitions it was \
         handed: {section:?}"
    );
    assert!(
        !section.contains("`Bash`") && !section.contains("`read`"),
        "FR-033: the prompt tool reference named tools that were not in the list \
         it was handed, so it is not derived from the definitions"
    );

    // The empty surface renders nothing at all, which is what a tool-free agent
    // relies on (FR-009) and what keeps a stale list from being pasted in.
    assert_eq!(build_tool_reference_from_defs(&[]), String::new());

    // Against the live registry the reference is the registry, and carries no
    // retired spelling.
    let defs = create_default_registry().definitions();
    let section = build_tool_reference_from_defs(&defs);
    for before in retired() {
        assert!(
            !section.contains(&format!("`{before}`")),
            "FR-033: the prompt tool reference names the retired spelling `{before}`"
        );
    }
    for def in defs.iter() {
        assert!(
            section.contains(&format!("`{}`", def.name)),
            "FR-033: the prompt tool reference omits the registered tool {:?}",
            def.name
        );
    }
}
