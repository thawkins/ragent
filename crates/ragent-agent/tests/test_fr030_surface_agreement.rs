//! Guard: the tool surface is unchanged in size and every enumeration agrees
//! with the registry (spec `uniformtools` FR-030, task T-037).
//!
//! FR-030 has two halves and this file pins both, against the numbers
//! `specs/uniformtools/T-041-BASELINE.md` recorded **before** the rename:
//!
//! | # | Case | Surface it pins |
//! |---|---|---|
//! | 1 | `test_every_registry_enumeration_reports_the_fr030_total` | `list`, `definitions`, `all_tools` |
//! | 2 | `test_the_dry_run_tools_section_agrees_with_the_registry` | `--dry-run` (human) |
//! | 3 | `test_the_dry_run_json_tools_section_agrees_with_the_registry` | `--dry-run --json` |
//! | 4 | `test_tool_info_reports_the_registry_total_and_every_name` | `tool_info` |
//! | 5 | `test_commands_info_catalogue_is_intact_and_names_no_retired_tool` | `commands_info` |
//! | 6 | `test_the_outbound_provider_tool_list_is_the_registry_surface` | the wire list sent to a provider |
//!
//! The `/tools list` and `/bug-report` halves live beside the surfaces they
//! render, in `crates/ragent-tui/tests/test_fr030_surface_agreement.rs`: both are
//! TUI render paths and this crate cannot reach them.
//!
//! ## Why the count alone is not the assertion
//!
//! T-041 §2.1 records the trap this file exists to prevent: the stale release
//! binary in `target/` also reports "148 visible tools", but splits the surface
//! `agent` 4 / `team` 19 while the tree that landed the rename reports
//! `agent` 3 / `team` 20. A comparison that checked only the total would pass
//! against the wrong binary, so case 2 compares the **whole** family table and
//! calls out the two canaries in its failure message.
//!
//! ## What this file deliberately does not check
//!
//! - **The rename pair-by-pair state.** `test_tool_rename_atomic_landing.rs`
//!   owns FR-034 (one `after` spelling resolving, no `before` reachable); a
//!   second copy here would be a guard to keep in step.
//! - **The completion surfaces.** FR-033's "no retired spelling is ever
//!   completed" is T-045's; case 5 asserts only that the `commands_info`
//!   catalogue is intact and names no retired spelling *as a tool*.
//! - **The permission taxonomy.** FR-028's 32 categories stay put, asserted by
//!   `test_fr028_state_survival.rs`; case 2 reads the categories only to derive
//!   the family table the `--dry-run` report prints.

#![allow(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::OsString;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::{Event, EventBus};
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool::command_catalog::COMMAND_CATALOG;
use ragent_agent::tool::rename::TOOL_RENAMES;
use ragent_agent::tool::{ToolContext, ToolRegistry, create_default_registry};
use ragent_config::{Capabilities, Config as RagentConfig, Cost};
use serde_json::Value;

/// The FR-030 surface size `specs/uniformtools/T-041-BASELINE.md` measured
/// before the rename.
///
/// Pinned as a number rather than derived from the registry, because the point
/// of the assertion is that the rename changed names and nothing else: a derived
/// expectation would agree with whatever the tree happened to do. `mf_screenshot`
/// is absent from the default build by design (it registers only when a drivable
/// headless browser resolves on `PATH`), so the default build advertises 148.
const FR030_TOTAL: usize = 148;

/// One canary family per T-041 §2.1: the pair whose split tells this tree's build
/// from the stale release binary beside it.
const FR030_CANARY_FAMILIES: [(&str, usize); 2] = [("agent", 3), ("team", 20)];

/// The family count T-041 records beside the total (21 families summing to 148).
const FR030_FAMILY_COUNT: usize = 21;

/// The `permission_category()` family of one registered tool: everything before
/// the first `:` (`file:read` -> `file`), which is how `--dry-run` groups.
fn family_of(registry: &ToolRegistry, name: &str) -> Option<String> {
    let tool = registry.get(name)?;
    let category = tool.permission_category();
    Some(category.split(':').next().unwrap_or(category).to_string())
}

/// The registry's own family table: family -> tool count.
///
/// This is the expectation case 2 compares the `--dry-run` report against, taken
/// from the same public API the report uses rather than from the baseline
/// document, so a disagreement names the surface that drifted.
fn registry_family_table(registry: &ToolRegistry) -> BTreeMap<String, usize> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for name in registry.list() {
        if let Some(family) = family_of(registry, &name) {
            *counts.entry(family).or_insert(0) += 1;
        }
    }
    counts
}

/// The `tools` section of a readiness report, whichever encoding it arrived in.
fn tools_section(
    report: &ragent_agent::dry_run::ReadinessReport,
) -> &ragent_agent::dry_run::ReadinessSection {
    report
        .sections
        .iter()
        .find(|section| section.name == "tools")
        .expect("FR-030: the readiness report has a `tools` section")
}

// ---------------------------------------------------------------------------
// 1. The three registry enumerations
// ---------------------------------------------------------------------------

#[test]
fn test_every_registry_enumeration_reports_the_fr030_total() {
    let registry = create_default_registry();

    let keys = registry.list();
    let definitions = registry.definitions();
    let definition_names: Vec<String> = definitions.iter().map(|d| d.name.clone()).collect();
    let mut tool_names: Vec<String> = registry
        .all_tools()
        .iter()
        .map(|tool| tool.name().to_string())
        .collect();
    tool_names.sort();

    // A rename adds, removes, merges and splits nothing, so all three lengths are
    // the baseline. `tool_info` reads `all_tools`, the outbound list and `/tools`
    // read `definitions`, and `--dry-run` reads `list`, so each length is the
    // figure one surface reports.
    assert_eq!(
        keys.len(),
        FR030_TOTAL,
        "FR-030: `ToolRegistry::list()` reports {} names, not the {FR030_TOTAL} \
         `T-041-BASELINE.md` recorded before the rename",
        keys.len()
    );
    assert_eq!(
        definitions.len(),
        FR030_TOTAL,
        "FR-030: `definitions()` (the outbound provider list and `/tools`) reports \
         {} tools, not the {FR030_TOTAL} the baseline records",
        definitions.len()
    );
    assert_eq!(
        tool_names.len(),
        FR030_TOTAL,
        "FR-030: `all_tools()` (`tool_info`) reports {} tools, not the {FR030_TOTAL} \
         the baseline records",
        tool_names.len()
    );

    // Agreement, not just the count: a registry that registered one spelling as a
    // key and another as a name would satisfy three equal lengths and still
    // advertise a name it cannot dispatch.
    assert_eq!(
        keys, definition_names,
        "FR-030: the key set and the model-facing definitions disagree on which \
         tools are registered"
    );
    assert_eq!(
        keys, tool_names,
        "FR-030: the key set and the names the tools report disagree"
    );

    for (label, names) in [
        ("ToolRegistry::list()", &keys),
        ("definitions()", &definition_names),
        ("all_tools()", &tool_names),
    ] {
        let unique: BTreeSet<&String> = names.iter().collect();
        assert_eq!(
            unique.len(),
            names.len(),
            "FR-030: `{label}` repeats a name, so a tool is registered twice"
        );
    }
}

// ---------------------------------------------------------------------------
// 1b. The names themselves: T-041 §6 points 3, 4, 5 and 6
// ---------------------------------------------------------------------------

/// The sorted names joined by a single `\n` with **no** trailing newline, as
/// `T-041-BASELINE.md` §3.1 specifies for this digest.
///
/// The document records both forms because `sha256sum` on a shell-written file
/// includes the final newline and this in-process digest does not; the "after"
/// digest without the newline is `308d92d9...`. Comparing the in-process form
/// against the file form would report a mismatch that is a newline, not a
/// missing tool, so this helper names the form it produces.
fn names_digest(names: &[String]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(names.join("\n").as_bytes()))
}

#[test]
fn test_the_registry_names_match_the_recorded_after_rename_digest() {
    // T-041 §6 points 3 to 6 in one place: the *names* the surface carries, not
    // only how many of them there are. Three separate defects pass a count check
    // and fail here - a rename that missed a tool, one that renamed an extra
    // tool, and one that renamed correctly but left the old spelling registered
    // beside the new one.
    let registry = create_default_registry();

    let names = registry.list();
    let unique: BTreeSet<&String> = names.iter().collect();
    assert_eq!(
        unique.len(),
        FR030_TOTAL,
        "FR-030: {} distinct names in a surface of {FR030_TOTAL}, so a tool is \
         registered twice",
        unique.len()
    );

    // Point 3 and 4: the whole list, pinned by digest, and the uppercase count
    // that is the sharpest cheap check. Before the rename no registered name
    // carried an uppercase letter; after it exactly the 13 adopted Claude
    // spellings do. Missing one leaves 12, renaming an extra takes it to 14.
    let upper: Vec<&String> = names
        .iter()
        .filter(|name| name.chars().any(|c| c.is_ascii_uppercase()))
        .collect();
    assert_eq!(
        upper.len(),
        TOOL_RENAMES.len(),
        "FR-030: {} registered names carry an uppercase letter, not the {} names \
         the FR-023 table renames: {upper:?}",
        upper.len(),
        TOOL_RENAMES.len()
    );

    // The uppercase names are exactly the table's `after` column - not merely the
    // same number of them. A rename that adopted a spelling the table does not
    // list, or skipped one and picked up an unrelated capitalised name, is a
    // different rename than the one the spec records.
    let adopted: BTreeSet<&str> = TOOL_RENAMES.iter().map(|pair| pair.after).collect();
    let upper_set: BTreeSet<&str> = upper.iter().map(|name| name.as_str()).collect();
    assert_eq!(
        upper_set, adopted,
        "FR-030: the capitalised names in the registry are not the FR-023 `after` \
         column"
    );

    // Point 6: the digest of the list T-041 §3.1 predicts for the post-rename
    // tree. This is the one assertion that fails when *any* single name is
    // wrong, and its failure message says which digest form it used.
    const AFTER_RENAME_DIGEST: &str =
        "308d92d9d975666cf1f815369e18cc7b457c1c0dcbab004c45f2835100368321";
    assert_eq!(
        names_digest(&names),
        AFTER_RENAME_DIGEST,
        "FR-030: the registry's {FR030_TOTAL} sorted names do not hash to the \
         post-rename digest `T-041-BASELINE.md` §3.1 records (joined by a single \
         newline, no trailing newline)"
    );

    // Point 3 from the other side: the retired spellings are not merely unnamed,
    // they are absent from the enumeration that `/tools` and the provider list
    // read. `test_tool_rename_atomic_landing.rs` owns the pair-by-pair state; this
    // asserts the list a user is shown.
    for pair in TOOL_RENAMES {
        assert!(
            !names.iter().any(|name| name == pair.before),
            "FR-030: the registry still enumerates the retired spelling `{}`",
            pair.before
        );
        assert!(
            names.iter().any(|name| name == pair.after),
            "FR-030: the registry does not enumerate `{}`, the spelling the FR-023 \
             table replaces `{}` with",
            pair.after,
            pair.before
        );
    }
}

// ---------------------------------------------------------------------------
// 2 and 3. The `--dry-run` report
// ---------------------------------------------------------------------------

/// Points `XDG_CONFIG_HOME` at an empty temp dir and clears the two config
/// override variables for as long as the guard lives.
///
/// `run_dry_run` reads the ambient global config, the project config and both
/// override variables, so without this a developer's real
/// `~/.config/ragent/ragent.json` would take part in the report. The mutation is
/// serialised both by `#[serial]` (the family `test_dry_run_extra.rs` uses) and
/// by the crate-wide `crate::global_env_lock()` (the family `test_plugin_bridge.rs`
/// uses): the two families are different mutexes, so taking both is what excludes
/// this test from every other env-mutating test in the binary.
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

/// Run the readiness report with an isolated config environment and return it
/// together with the guards that keep the isolation alive.
async fn isolated_dry_run() -> (
    ragent_agent::dry_run::ReadinessReport,
    std::sync::MutexGuard<'static, ()>,
    EnvGuard,
    tempfile::TempDir,
    tempfile::TempDir,
) {
    let lock = crate::global_env_lock();
    let config_home = tempfile::tempdir().expect("config-home tempdir");
    let project = tempfile::tempdir().expect("project tempdir");
    let env = EnvGuard::isolating(config_home.path());

    let config_path = project.path().join("ragent.json");
    std::fs::write(
        &config_path,
        r#"{ "defaultAgent": "general", "provider": { "anthropic": { "env": [] } } }"#,
    )
    .expect("write config");

    let inputs = ragent_agent::dry_run::DryRunInputs {
        config_path: Some(config_path),
        agent_name: "general".to_string(),
        model_override: None,
        provider_registry: Arc::new(ragent_agent::provider::create_default_registry()),
        tool_registry: Arc::new(create_default_registry()),
        working_dir: project.path().to_path_buf(),
    };
    let (report, _exit_code) = ragent_agent::dry_run::run_dry_run(inputs).await;
    (report, lock, env, config_home, project)
}

#[tokio::test]
#[serial_test::serial]
async fn test_the_dry_run_tools_section_agrees_with_the_registry() {
    let (report, _lock, _env, _config_home, _project) = isolated_dry_run().await;
    let section = tools_section(&report);
    let registry = create_default_registry();

    // `ragent --dry-run` prints this item first: `total: N visible tools`.
    let total = section
        .items
        .iter()
        .find(|item| item.name == "total")
        .expect("FR-030: the `tools` section reports a total");
    assert_eq!(
        total.message,
        format!("{FR030_TOTAL} visible tools"),
        "FR-030: `--dry-run` reports {:?}, not the {FR030_TOTAL} baseline",
        total.message
    );

    // The family rows, compared row for row against the registry's own table.
    let mut reported: BTreeMap<String, usize> = BTreeMap::new();
    for item in &section.items {
        if item.name == "total" {
            continue;
        }
        assert_ne!(
            item.name, "uncategorized",
            "FR-030: `--dry-run` reports tools with no family prefix, so the \
             family table no longer accounts for the whole surface: {:?}",
            item
        );
        let count: usize = item
            .message
            .split_whitespace()
            .next()
            .and_then(|n| n.parse().ok())
            .unwrap_or_else(|| panic!("FR-030: unparseable family count {:?}", item.message));
        reported.insert(item.name.clone(), count);
    }

    let expected = registry_family_table(&registry);
    assert_eq!(
        reported, expected,
        "FR-030: the families `--dry-run` reports are not the families the \
         registry holds"
    );
    assert_eq!(
        reported.len(),
        FR030_FAMILY_COUNT,
        "FR-030: {} families, not the {FR030_FAMILY_COUNT} the baseline records: {:?}",
        reported.len(),
        reported
    );
    for (family, count) in FR030_CANARY_FAMILIES {
        assert_eq!(
            reported.get(family).copied(),
            Some(count),
            "FR-030: the `{family}` family holds {:?} tools, not the {count} the \
             baseline records - this is the canary that tells this tree's build \
             from a stale one beside it, which also reports {FR030_TOTAL}",
            reported.get(family)
        );
    }
    assert_eq!(
        reported.values().sum::<usize>(),
        FR030_TOTAL,
        "FR-030: the family counts sum to {}, not {FR030_TOTAL}",
        reported.values().sum::<usize>()
    );
}

#[tokio::test]
#[serial_test::serial]
async fn test_the_dry_run_json_tools_section_agrees_with_the_registry() {
    // TC-024 step 7 compares the JSON report's `tools.total` against the human
    // report's. Both come from `tools_section`, so this pins that the JSON
    // encoding of that section carries the same names and numbers.
    let (report, _lock, _env, _config_home, _project) = isolated_dry_run().await;
    let human = tools_section(&report);
    let json: Value = serde_json::from_str(&report.to_json().expect("report serialises"))
        .expect("report is valid JSON");
    let json_tools = json["sections"]
        .as_array()
        .expect("sections array")
        .iter()
        .find(|section| section["name"] == "tools")
        .expect("FR-030: the JSON report has a `tools` section");

    let total = json_tools["items"]
        .as_array()
        .expect("items array")
        .iter()
        .find(|item| item["name"] == "total")
        .expect("FR-030: the JSON `tools` section reports a total");
    assert_eq!(
        total["message"].as_str(),
        Some(format!("{FR030_TOTAL} visible tools").as_str()),
        "FR-030: the JSON readiness report reports {:?} tools, not {FR030_TOTAL}",
        total["message"]
    );

    let json_names: Vec<&str> = json_tools["items"]
        .as_array()
        .expect("items array")
        .iter()
        .filter_map(|item| item["name"].as_str())
        .collect();
    let human_names: Vec<&str> = human.items.iter().map(|item| item.name.as_str()).collect();
    assert_eq!(
        json_names, human_names,
        "FR-030: the JSON and human `tools` sections enumerate different items"
    );
}

// ---------------------------------------------------------------------------
// 4. `tool_info`
// ---------------------------------------------------------------------------

fn registry_ctx(registry: Arc<ToolRegistry>) -> ToolContext {
    ToolContext {
        session_id: "fr030".to_string(),
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
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: registry,
    }
}

#[tokio::test]
async fn test_tool_info_reports_the_registry_total_and_every_name() {
    let registry = Arc::new(create_default_registry());
    let tool = registry.get("tool_info").expect("tool_info is registered");
    let ctx = registry_ctx(Arc::clone(&registry));

    let output = tool
        .execute(serde_json::json!({}), &ctx)
        .await
        .expect("execute");
    let parsed: Value = serde_json::from_str(&output.content).expect("tool_info returns JSON");

    assert_eq!(
        parsed["total"].as_u64(),
        Some(FR030_TOTAL as u64),
        "FR-030: `tool_info` reports {:?} tools, not the {FR030_TOTAL} baseline",
        parsed["total"]
    );

    let entries = parsed["tools"].as_array().expect("tools array");
    let mut reported: Vec<String> = entries
        .iter()
        .map(|entry| {
            entry["name"]
                .as_str()
                .expect("every entry names its tool")
                .to_string()
        })
        .collect();
    reported.sort();
    assert_eq!(
        reported,
        registry.list(),
        "FR-030: `tool_info` enumerates a different set of tools than the registry"
    );

    // Every entry must be actionable: a name with no permission category or no
    // source would be a catalogue row the model cannot use.
    for entry in entries {
        let name = entry["name"].as_str().expect("name");
        assert!(
            entry["permission_category"]
                .as_str()
                .is_some_and(|c| !c.is_empty()),
            "FR-030/FR-028: `tool_info`'s entry for `{name}` has no permission category"
        );
        assert!(
            entry["source"].as_str().is_some_and(|s| !s.is_empty()),
            "FR-030: `tool_info`'s entry for `{name}` has no source"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. `commands_info`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_commands_info_catalogue_is_intact_and_names_no_retired_tool() {
    let registry = Arc::new(create_default_registry());
    let tool = registry
        .get("commands_info")
        .expect("commands_info is registered");
    let ctx = registry_ctx(Arc::clone(&registry));

    let output = tool
        .execute(serde_json::json!({}), &ctx)
        .await
        .expect("execute");
    let parsed: Value = serde_json::from_str(&output.content).expect("commands_info returns JSON");

    // `commands_info` is the slash-command catalogue, not a tool count (T-041 §4):
    // consistency for it means every built-in entry survived the rename and none
    // of them names a retired spelling *as a tool*.
    let builtin = parsed["builtin"].as_array().expect("builtin array");
    assert_eq!(
        builtin.len(),
        COMMAND_CATALOG.len(),
        "FR-030: `commands_info` reports {} built-in commands, not the {} the \
         catalog declares",
        builtin.len(),
        COMMAND_CATALOG.len()
    );

    // Every slash trigger is unique, so no command was shadowed by another.
    let triggers: BTreeSet<&str> = builtin
        .iter()
        .map(|entry| entry["trigger"].as_str().expect("trigger is a string"))
        .collect();
    assert_eq!(
        triggers.len(),
        builtin.len(),
        "FR-030: `commands_info` reports a duplicate slash trigger"
    );

    let retired: BTreeSet<&str> = TOOL_RENAMES.iter().map(|pair| pair.before).collect();
    for entry in builtin {
        let trigger = entry["trigger"].as_str().expect("trigger");

        // The structured suggestion fields are the ones a completion list is
        // built from. A retired spelling may appear there only when the same word
        // is itself a slash command (`/bash` is a command, not the retired `bash`
        // tool): slash vocabulary and tool vocabulary share words, and FR-033
        // governs only surfaces that offer a *tool* name.
        for field in ["subcommands", "flags"] {
            for token in entry[field].as_array().expect("array of strings") {
                let token = token.as_str().expect("subcommand/flag is a string");
                if retired.contains(token) {
                    assert!(
                        triggers.contains(token),
                        "FR-030/FR-033: the `/{trigger}` entry offers `{token}` as a \
                         {field} suggestion, and `{token}` is a retired tool spelling \
                         that is not a slash command either - a completion list would \
                         teach the dead name back"
                    );
                }
            }
        }

        // A description may mention a tool only in backticks (the convention the
        // catalogue uses for code); a backticked retired spelling in a description
        // is a tool reference, and the `/bash` and `/init` descriptions that carry
        // the bare words `bash` and `write` are verbs and command names, not tool
        // references.
        let description = entry["description"].as_str().expect("description");
        for token in retired.iter() {
            let backticked = format!("`{token}`");
            if description.contains(&backticked) && !triggers.contains(token) {
                panic!(
                    "FR-030: the `/{trigger}` description names the retired tool `{token}` \
                     in backticks: {description:?}"
                );
            }
        }
    }

    // The `/tools` entry is the one command that offers tool names; its
    // subcommands are exactly what the handler accepts.
    let tools_entry = builtin
        .iter()
        .find(|entry| entry["trigger"] == "tools")
        .expect("FR-030: the catalogue lists `/tools`");
    let subcommands: Vec<&str> = tools_entry["subcommands"]
        .as_array()
        .expect("subcommands array")
        .iter()
        .filter_map(|value| value.as_str())
        .collect();
    assert_eq!(
        subcommands,
        vec!["list", "show", "help"],
        "FR-030: `/tools` offers {subcommands:?}, not the `list`, `show`, `help` \
         the handler accepts"
    );
}

// ---------------------------------------------------------------------------
// 6. The outbound provider tool list
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum ScriptedReply {
    /// Text-only response ending with `Finish { Stop }`.
    Text(&'static str),
}

#[derive(Clone)]
struct ScriptedProvider {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    captured: Arc<Mutex<Vec<ChatRequest>>>,
}

struct ScriptedClient {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    captured: Arc<Mutex<Vec<ChatRequest>>>,
}

#[async_trait::async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.captured
            .lock()
            .expect("captured requests lock")
            .push(request);
        let reply = {
            let mut replies = self.replies.lock().expect("replies lock");
            replies.pop().unwrap_or(ScriptedReply::Text("done"))
        };
        match reply {
            ScriptedReply::Text(text) => {
                let events: [StreamEvent; 2] = [
                    StreamEvent::TextDelta {
                        text: text.to_string(),
                    },
                    StreamEvent::Finish {
                        reason: LlmFinishReason::Stop,
                    },
                ];
                Ok(Box::pin(stream::iter(events)))
            }
        }
    }
}

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
        "Scripted Provider"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "qwen3:latest".to_string(),
            provider_id: "ollama".to_string(),
            name: "Qwen3".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
                reasoning: false,
                streaming: true,
            },
            context_window: 128_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
        }]
    }

    fn as_any_static(&self) -> &(dyn std::any::Any + 'static) {
        self
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, serde_json::Value>,
    ) -> Result<Box<dyn LlmClient>> {
        Ok(Box::new(ScriptedClient {
            replies: Arc::clone(&self.replies),
            captured: Arc::clone(&self.captured),
        }))
    }
}

/// Build a processor over the default registry whose provider replays `replies`.
///
/// Copied from `test_loop_steps_harness.rs` (an external `tests/` crate cannot
/// reach the `pub(crate)` loop-step methods, so the turn is driven through the
/// public `process_message`). `max_retries` is zeroed so nothing sleeps.
fn make_processor(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
) -> (
    SessionProcessor,
    tempfile::TempDir,
    Arc<Mutex<Vec<ChatRequest>>>,
) {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        replies: Arc::new(Mutex::new(replies)),
        captured: Arc::clone(&captured),
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let config: RagentConfig = serde_json::from_str("{}").expect("valid config JSON");

    let processor = SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(HashMap::new()),
        cached_config: parking_lot::Mutex::new(Some(CachedConfig {
            config: Arc::new(config),
            file_mtimes: Vec::new(),
            env_overrides_present: false,
        })),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(HashMap::new()),
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    (processor, tmp, captured)
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });
    agent
}

#[tokio::test]
async fn test_the_outbound_provider_tool_list_is_the_registry_surface() {
    // T-041 §4: the list ragent sends a provider is `get_cached_tool_definitions`,
    // which returns `tool_registry.definitions()` - not a parallel copy that could
    // drift. This case proves it end to end: one real turn through a scripted
    // provider, and the `ChatRequest` that actually left the process is compared
    // against the registry.
    let event_bus = Arc::new(EventBus::new(512));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::Text("the surface is unchanged")],
    );
    let session = processor
        .session_manager
        .create_session(working_dir.path().to_path_buf())
        .expect("session created");

    processor
        .process_message(
            &session.id,
            "report the tool surface",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the turn succeeds");

    let requests = captured.lock().expect("captured lock").clone();
    assert_eq!(requests.len(), 1, "one LLM request for a one-step turn");
    let tools = &requests[0].tools;

    let mut sent: Vec<String> = tools
        .iter()
        .map(|definition| definition.name.clone())
        .collect();
    assert_eq!(
        sent.len(),
        FR030_TOTAL,
        "FR-030: the outbound provider list carries {} tools, not the {FR030_TOTAL} \
         the baseline records",
        sent.len()
    );
    let registry = create_default_registry();
    sent.sort();
    assert_eq!(
        sent,
        registry.list(),
        "FR-030: the tool list sent to the provider is not the registry's surface"
    );
    assert!(
        tools
            .iter()
            .all(|definition| !definition.description.is_empty()),
        "FR-030: a tool reached the provider with no description, so the rename \
         dropped prompt text"
    );

    // The `ToolsSent` event the TUI and the SSE stream render is built from the
    // same vector, so its names must agree too.
    let mut reported: Option<Vec<String>> = None;
    while let Ok(event) = rx.try_recv() {
        if let Event::ToolsSent { tools, .. } = event {
            reported = Some(tools);
        }
    }
    let mut reported = reported.expect("FR-030: the turn publishes a `ToolsSent` event");
    reported.sort();
    assert_eq!(
        reported,
        registry.list(),
        "FR-030: the `ToolsSent` event advertises a different surface than the \
         request it describes"
    );
}
