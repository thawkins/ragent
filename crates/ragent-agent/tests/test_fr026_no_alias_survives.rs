//! Guard: no `before` spelling survives the rename (spec `uniformtools`
//! FR-026, task T-032).
//!
//! FR-026 has three clauses and each one can fail silently:
//!
//! 1. the tool is exposed under its `after` name **only** - `Tool::name()`, the
//!    registry key, `definitions()` and every surface derived from them;
//! 2. `before` is **not** registered a second time, and no forwarding wrapper or
//!    adapter stands under it;
//! 3. a call or lookup using `before` **fails as an unknown tool** carrying the
//!    did-you-mean hint, `before` is not accepted at any inbound resolution
//!    stage, and that refusal reaches the caller on **every** inbound path (the
//!    TUI prompt, `ragent run`, the OpenAI-compatible `/v1` surface, ACP).
//!
//! | # | Case | Clause it pins |
//! |---|---|---|
//! | 1 | `test_no_second_registration_stands_under_a_before_spelling` | (2) the registry, three ways |
//! | 2 | `test_no_retired_spelling_is_a_registry_key_a_tool_name_or_a_definition` | (1) + (2) keys, `name()`, `definitions()` |
//! | 3 | `test_no_tool_delegates_to_one_whose_name_pair_it_shares` | (2) the forwarding-wrapper half |
//! | 4 | `test_the_alias_table_refuses_both_columns_of_every_rename_pair` | (3) the one inbound translation stage |
//! | 5 | `test_every_before_spelling_resolves_to_nothing_with_a_did_you_mean_hint` | (3) the resolution stage |
//! | 6 | `test_the_agent_loop_refuses_a_before_call_and_appends_its_refusal` | (3) the path every surface shares |
//! | 7 | `test_the_inbound_paths_share_one_resolution_chain` | (3) every inbound path |
//! | 8 | `test_a_seeded_second_registration_is_detected` | the negative control for (1) and (2) |
//! | 9 | `test_the_builtin_alias_list_names_no_rename_spelling` | (3) `tools.aliases: true` |
//!
//! ## Why case 3 does not fail on the `multiedit` alias
//!
//! `crates/ragent-agent/src/tool/mod.rs` registers `multiedit` as
//! `LegacyMultiEditAlias`, a wrapper that overrides `name()` and forwards to the
//! core `multi_edit` tool. That is a forwarding alias, and FR-026 names
//! forwarding wrappers as forbidden - but `multiedit` is **not** a row of the
//! FR-023 table. It is `editrenewal`'s separate deprecation of the pre-existing
//! `multi_edit`/`multiedit` pair, which `uniformtools` neither renames nor
//! retires. Clause 2 is stated "for every row of FR-023", so this file's wrapper
//! checks are scoped to those pairs, and case 3 says so in an assertion rather
//! than a comment: if `multiedit` ever becomes an FR-023 row, case 3 fails and
//! names the fix.
//!
//! ## Why case 6 drives the real loop rather than calling the resolver
//!
//! Cases 4 and 5 call the resolution functions directly, which proves the
//! functions. That is a different claim from "the refusal reaches the caller":
//! the observable FR-026 promises is a **recorded** `MessagePart::ToolCall` in
//! `Error` status carrying the refusal sentence, and only the agent loop
//! produces one. Case 6 scripts a provider that emits the retired spelling and
//! reads the turn's reply, with auto-approval **on** - so a name accepted
//! anywhere would be executed rather than merely refused at a prompt.
//!
//! ## Why the wrapper checks are the whole of clause 2, not a grep
//!
//! "No forwarding wrapper or adapter" could be read as a source scan, and T-032
//! tried to prove it that way first. A source scan cannot do it: the workspace's
//! one forwarding alias (`LegacyMultiEditAlias`) is built from a `&'static str`
//! field, so the only literal in its file is a variable initialiser rather than
//! a call, and a scan keyed on a `before` literal finds nothing in a file that
//! plainly contains the retired spelling. Worse, a *seeded* wrapper registered
//! under a retired spelling survived five separate scans during that attempt.
//! The registry is where the two spellings become indistinguishable, so the
//! registry is where this file looks: case 3 compares the keys against the names
//! the tools report, and case 8 proves that comparison sees a seeded wrapper.
//!
//! ## What this file deliberately does not check
//!
//! - **The other inbound surfaces' own adapters.** The `/v1` adapter and the ACP
//!   server both drive `SessionProcessor::process_message`; case 7 pins that at
//!   the source level (their construction sites call it, and neither resolves a
//!   tool name of its own), and case 6 drives the function they call.
//!   `crates/ragent-server/tests/test_openai_surface.rs` and
//!   `crates/ragent-agent/tests/test_acp_server.rs` already drive the transports.
//! - **The no-alias source scan.** That no non-test source literal spells a
//!   `before` name is `scripts/check-tool-rename.sh` (FR-027, T-034). This file
//!   proves the *behavioural* absence, which a literal scan cannot see.
//! - **The `allowed_tools` surface.** Resolving a Claude-spelled entry exactly,
//!   and reporting one that matches nothing, is `test_fr029_allowed_tools.rs`
//!   (FR-029, T-036).
//! - **The completion and `/tools` menus.** FR-033's "no retired spelling is
//!   ever completed" is `test_fr033_completion_sources.rs` and its TUI half.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use async_trait::async_trait;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::EventBus;
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::message::{Message, MessagePart, ToolCallStatus};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::dispatch::{canonical_dispatch_name, resolve_call, suggest_tool_name};
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool::alias_table::{AliasTable, resolve_inbound_tool_name};
use ragent_agent::tool::rename::TOOL_RENAMES;
use ragent_agent::tool::{Tool, ToolContext, ToolOutput, ToolRegistry, create_default_registry};
use ragent_config::{
    AliasesConfig, Capabilities, Config as RagentConfig, Cost, StreamConfig, ToolsConfig,
};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// The tables these cases read
// ---------------------------------------------------------------------------

/// The retired `before` spellings (FR-023).
fn retired() -> Vec<&'static str> {
    TOOL_RENAMES.iter().map(|pair| pair.before).collect()
}

/// The workspace root, resolved from this crate's manifest directory.
fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crates/<name> sits two levels below the workspace root")
        .to_path_buf()
}

// ---------------------------------------------------------------------------
// Cases 1, 2, 8: no second registration, no `before` under any label
// ---------------------------------------------------------------------------

/// A forwarding wrapper: it reports `name` as its own name and delegates every
/// call to another tool, which is the shape FR-026 clause 2 forbids under a
/// retired spelling and the shape case 8 seeds.
struct ForwardingStub {
    name: &'static str,
    inner: Arc<dyn Tool>,
}

#[async_trait]
impl Tool for ForwardingStub {
    fn name(&self) -> &str {
        self.name
    }

    fn description(&self) -> &'static str {
        "FR-026 forwarding stub"
    }

    fn parameters_schema(&self) -> Value {
        self.inner.parameters_schema()
    }

    fn permission_category(&self) -> &str {
        self.inner.permission_category()
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        self.inner.execute(input, ctx).await
    }
}

/// A tool that registers under whatever name it is given, so a case can seed a
/// deliberately wrong registration.
struct NamedStub(&'static str);

#[async_trait]
impl Tool for NamedStub {
    fn name(&self) -> &str {
        self.0
    }

    fn description(&self) -> &'static str {
        "FR-026 test stub"
    }

    fn parameters_schema(&self) -> Value {
        json!({ "type": "object", "properties": {} })
    }

    fn permission_category(&self) -> &str {
        // A literal bound would trip `clippy::unnecessary_literal_bound`; the
        // stub reads its own field instead, which is what the trait allows.
        self.0
    }

    async fn execute(&self, _input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        Ok(ToolOutput {
            content: "stub".to_string(),
            metadata: None,
        })
    }
}

/// FR-026 clause 2: the registry holds no `before` spelling, checked through
/// `get`, the key list and a case-folded key comparison.
///
/// A "second registration" is not only a duplicate key: an adapter stored under a
/// key its own `Tool::name()` does not report is the shape a half-finished rename
/// leaves behind, and case 3 reads that half.
#[test]
fn test_no_second_registration_stands_under_a_before_spelling() {
    let registry = create_default_registry();
    let keys = registry.list();

    let mut sorted = keys.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        keys.len(),
        sorted.len(),
        "the registry holds a duplicate key, so one tool is registered twice (FR-026 clause 2)"
    );

    for name in retired() {
        // `get` is one of the surfaces FR-026's case-folding note still refuses:
        // the registry is a map, its key is the adopted spelling, and an exact
        // lookup of the retired one finds nothing.
        assert!(
            registry.get(name).is_none(),
            "`{name}` is a retired spelling but `get` resolves it: a rename that keeps the old \
             name registered is an alias (FR-026 clause 2)"
        );
        assert!(
            !keys.iter().any(|key| key == name),
            "`{name}` appears in the registry's key list (FR-026 clause 2)"
        );
    }

    // The permitted arm, stated as an assertion so the line between it and the
    // refusals above is checked rather than described: for the nine case-only
    // pairs the retired spelling IS the adopted name in another letter case, and
    // the dispatch lookup folds it (FR-026's case-folding note). What must never
    // happen - and what the loop above proves - is the folded name reaching a
    // *surface*: a key, a listing, a `tool_info` row or an outbound definition.
    for pair in TOOL_RENAMES.iter() {
        if pair.before.eq_ignore_ascii_case(pair.after) {
            assert_eq!(
                canonical_dispatch_name(&registry, pair.before).as_deref(),
                Some(pair.after),
                "`{}` must fold onto `{}` at the dispatch lookup; the fold is a match between \
                 two forms of one word, not a second vocabulary (FR-026)",
                pair.before,
                pair.after
            );
        } else {
            assert_eq!(
                canonical_dispatch_name(&registry, pair.before),
                None,
                "`{}` must not reach `{}` through the case fold: the two differ by more than \
                 letter case, so a fold here would be a translation (FR-026)",
                pair.before,
                pair.after
            );
        }
    }

    for pair in TOOL_RENAMES.iter() {
        let matches = keys.iter().filter(|key| key.as_str() == pair.after).count();
        assert_eq!(
            matches, 1,
            "`{}` must be registered exactly once (FR-026 clause 1)",
            pair.after
        );
    }
}

/// FR-026 clauses 1 and 2: no `before` spelling is a registry key, a tool's
/// reported name or a model-facing definition, and each `after` spelling is the
/// single registration that carries its pair's descriptor.
#[test]
fn test_no_retired_spelling_is_a_registry_key_a_tool_name_or_a_definition() {
    let registry = create_default_registry();
    let definitions = registry.definitions();
    let definition_names: Vec<&str> = definitions.iter().map(|d| d.name.as_str()).collect();
    let tool_names: Vec<String> = registry
        .all_tools()
        .iter()
        .map(|tool| tool.name().to_string())
        .collect();
    let keys = registry.list();

    for name in retired() {
        assert!(
            !definition_names.contains(&name),
            "the outbound provider tool list advertises `{name}`, a retired spelling \
             (FR-026 clause 1)"
        );
        assert!(
            !tool_names.iter().any(|tool| tool == name),
            "a registered tool reports the retired spelling `{name}` as its own name \
             (FR-026 clause 1)"
        );
        assert!(
            !keys.iter().any(|key| key == name),
            "`{name}` is a registry key (FR-026 clause 1)"
        );
    }

    for pair in TOOL_RENAMES.iter() {
        let Some(tool) = registry.get(pair.after) else {
            panic!(
                "`{}` is not registered; FR-034 forbids a half-renamed tree",
                pair.after
            );
        };
        assert_eq!(
            tool.name(),
            pair.after,
            "the tool at key `{}` reports `{}`, so the key and the name disagree \
             (FR-026 clause 1)",
            pair.after,
            tool.name()
        );
        assert!(
            definition_names.contains(&pair.after),
            "`{}` has no model-facing definition, so the model can never call it \
             (FR-026 clause 1)",
            pair.after
        );
    }
}

/// The negative control for cases 1 and 2: a registry with a deliberate second
/// registration must be detectable by the comparison those cases make.
///
/// Without this, both could pass against a registry whose `get`, `list` and
/// `definitions` agreed by being consistently wrong.
#[test]
fn test_a_seeded_second_registration_is_detected() {
    let seeded = ToolRegistry::new();
    seeded.register(Arc::new(NamedStub("Bash")));
    seeded.register(Arc::new(NamedStub("bash")));

    // The forwarding-wrapper half as well: a wrapper that reports the retired
    // spelling and delegates to the adopted tool is the other way a second
    // registration arrives, and it produces the same two rows.
    let wrapped = ToolRegistry::new();
    let adopted = wrapped.get("Bash").unwrap_or_else(|| {
        wrapped.register(Arc::new(NamedStub("Bash")));
        wrapped.get("Bash").expect("just registered")
    });
    wrapped.register(Arc::new(ForwardingStub {
        name: "bash",
        inner: adopted,
    }));
    assert!(
        wrapped.get("bash").is_some() && wrapped.get("Bash").is_some(),
        "a wrapper registered under the retired spelling must be visible to this case's \
         comparison, which is why both spellings resolving is a failure"
    );
    assert_eq!(
        wrapped.list().len(),
        2,
        "the wrapper is the second row the rename allows nowhere"
    );

    assert!(
        seeded.get("bash").is_some(),
        "the seeded registry should hold the second registration"
    );
    assert!(
        seeded.get("Bash").is_some(),
        "and the adopted spelling beside it"
    );
    assert_eq!(
        seeded.list().len(),
        2,
        "the seeded defect is two rows where the rename allows one"
    );

    // The real registry is the opposite in exactly that way: for every pair one
    // row, and it is the adopted spelling.
    let real = create_default_registry();
    for pair in TOOL_RENAMES.iter() {
        assert!(
            real.get(pair.before).is_none(),
            "`{}` resolves",
            pair.before
        );
        assert!(real.get(pair.after).is_some(), "`{}` is gone", pair.after);
    }
}

// ---------------------------------------------------------------------------
// Case 3: no forwarding wrapper or adapter under a `before` name
// ---------------------------------------------------------------------------

/// FR-026 clause 2's "forwarding wrapper or adapter" half, scoped to the FR-023
/// pairs.
///
/// The rename's own failure mode is a *second* registration: an adapter that
/// overrides `name()` to return the retired spelling and forwards to the renamed
/// tool. A literal scan cannot see it (the wrapper builds the string) and a key
/// comparison alone cannot either (the key is the wrapper's own), but two
/// comparisons can: a pair with **both** spellings resolving is the renamed tool
/// plus something standing under the old name, and a key that disagrees with the
/// name its tool reports is a wrapper registered under a different label.
#[test]
fn test_no_tool_delegates_to_one_whose_name_pair_it_shares() {
    let registry = create_default_registry();

    for pair in TOOL_RENAMES.iter() {
        let old = registry.get(pair.before);
        let new = registry.get(pair.after);
        assert!(
            !(old.is_some() && new.is_some()),
            "both `{}` and `{}` resolve, so a forwarding wrapper or adapter stands under the \
             retired spelling (FR-026 clause 2)",
            pair.before,
            pair.after
        );
    }

    for key in registry.list() {
        if let Some(tool) = registry.get(&key) {
            assert_eq!(
                tool.name(),
                key,
                "the tool stored at key `{key}` reports `{}` as its name; an adapter stored \
                 under a second label is a second spelling (FR-026 clause 2)",
                tool.name()
            );
        }
    }

    // The one forwarding wrapper this workspace ships is the `multiedit`
    // deprecation alias (`LegacyMultiEditAlias`), which is not an FR-023 row.
    // Asserting that keeps this case's scope honest: were `multiedit` a rename
    // pair, the loop above would fire and it would have to be retired.
    assert!(
        registry.get("multiedit").is_some(),
        "the `multiedit` deprecation alias is gone, so this case's stated scope (the wrapper \
         is deliberately outside FR-023) no longer matches the tree"
    );
    for pair in TOOL_RENAMES.iter() {
        assert_ne!(
            pair.before, "multiedit",
            "`multiedit` is an FR-023 row now, so its forwarding alias is an FR-026 violation \
             and `crates/ragent-agent/src/tool/mod.rs` must retire it"
        );
    }
}

// ---------------------------------------------------------------------------
// Case 4 and 9: the one inbound translation stage refuses a `before`
// ---------------------------------------------------------------------------

/// Build an `AliasesConfig` from explicit entries, through the supported JSON
/// form (the type's fields are private and the object form is the contract).
fn aliases_from(entries: &[(&str, &str)]) -> AliasesConfig {
    let object: serde_json::Map<String, Value> = entries
        .iter()
        .map(|(key, value)| ((*key).to_string(), json!(value)))
        .collect();
    serde_json::from_value(Value::Object(object))
        .expect("the object form of `tools.aliases` parses")
}

/// FR-026 clause 3: the alias table is the only inbound translation stage, and it
/// refuses a `before` spelling in **both** columns.
///
/// The config loader refuses such an entry outright and stops the load (FR-031),
/// so the arm this case reaches is the defence-in-depth one - a `ToolsConfig`
/// built in memory by a test, a program or a future caller. FR-026's "shall not
/// accept `before` at any inbound tool-name resolution stage" is the requirement
/// that arm implements, and the loader is not the only way a table comes into
/// existence.
#[test]
fn test_the_alias_table_refuses_both_columns_of_every_rename_pair() {
    let registry = create_default_registry();

    for pair in TOOL_RENAMES.iter() {
        // The pre-rename mapping as an alias: `before` -> `after`.
        let by_source = AliasTable::from_aliases(&aliases_from(&[(pair.before, pair.after)]));
        assert_eq!(
            by_source.resolve(pair.before),
            None,
            "the alias table maps the retired `{}` onto `{}`, so the inbound stage would \
             translate it and FR-026's refusal would never be reached",
            pair.before,
            pair.after
        );
        assert_eq!(
            resolve_inbound_tool_name(&registry, &by_source, pair.before),
            None,
            "the retired `{}` resolved through the inbound alias stage (FR-026 clause 3)",
            pair.before
        );

        // A source that is not a rename spelling, resolving *onto* the retired
        // one: the entry asks for the dead name as the result of a translation.
        let by_target =
            AliasTable::from_aliases(&aliases_from(&[("NotARenameSpelling", pair.before)]));
        assert_eq!(
            by_target.resolve("NotARenameSpelling"),
            None,
            "the alias table resolves onto the retired `{}`, a second spelling of it \
             (FR-026 clause 3)",
            pair.before
        );
        assert_eq!(
            by_target.unresolved_targets(&registry),
            Vec::<(&str, &str)>::new(),
            "a dropped entry still reports an unresolved target, so it was kept"
        );

        // A retired source pointed at an unrelated tool: still refused, because
        // the source column alone is the defect.
        let by_source_elsewhere = AliasTable::from_aliases(&aliases_from(&[(pair.before, "read")]));
        assert_eq!(
            by_source_elsewhere.resolve(pair.before),
            None,
            "the retired `{}` is accepted as an alias source (FR-026 clause 3)",
            pair.before
        );
    }
}

/// `tools.aliases: true` is a supported configuration and the likeliest route by
/// which a second spelling could come back; the built-in list names neither
/// column of any rename pair (FR-026, FR-015).
#[test]
fn test_the_builtin_alias_list_names_no_rename_spelling() {
    let registry = create_default_registry();
    let table = AliasTable::from_tools_config(&ToolsConfig {
        aliases: aliases_from(&[]).clone(),
        ..Default::default()
    });
    let builtin = AliasTable::from_aliases(
        &serde_json::from_value(json!(true)).expect("the boolean form of `tools.aliases` parses"),
    );

    for pair in TOOL_RENAMES.iter() {
        for (label, table) in [
            ("the empty object form", &table),
            ("`aliases: true`", &builtin),
        ] {
            assert_eq!(
                table.resolve(pair.before),
                None,
                "{label} maps the retired `{}` onto a ragent tool (FR-026)",
                pair.before
            );
            assert_eq!(
                table.resolve(pair.after),
                None,
                "{label} maps the adopted `{}`, which asks for two spellings of one tool \
                 (FR-026)",
                pair.after
            );
            assert_eq!(
                resolve_inbound_tool_name(&registry, table, pair.before),
                None,
                "{label} resolves the retired `{}` (FR-026 clause 3)",
                pair.before
            );
        }
    }

    assert!(
        !builtin.is_empty(),
        "the built-in alias list is empty, so the `aliases: true` half of this case proves \
         nothing (FR-015)"
    );
}

// ---------------------------------------------------------------------------
// Case 5: the resolution stage refuses, with its hint
// ---------------------------------------------------------------------------

/// FR-026 clause 3: a `before` spelling reaches neither the alias stage nor a
/// tool, the refusal carries the did-you-mean hint, and the one stage that does
/// accept it - the permitted case fold - accepts nothing but another letter case
/// of the same word.
///
/// The stages are the three the agent loop applies in order
/// (`crates/ragent-agent/src/session/processor.rs`): the declarative alias table,
/// the registry's unique ASCII-case-insensitive fold, and the pipeline's
/// decode-resolve-validate step. The fold is deliberately asserted in its
/// permitted form rather than removed from the chain: FR-026's case-folding note
/// names it, and a case that demanded the fold refuse `bash` would contradict the
/// requirement it claims to enforce.
#[test]
fn test_every_before_spelling_resolves_to_nothing_with_a_did_you_mean_hint() {
    let registry = create_default_registry();
    let empty = AliasTable::default();
    let mut hinted = 0usize;

    for pair in TOOL_RENAMES.iter() {
        // Stage 1: the declarative alias table.
        assert_eq!(
            resolve_inbound_tool_name(&registry, &empty, pair.before),
            None,
            "`{}` resolved through the inbound alias stage (FR-026 clause 3)",
            pair.before
        );

        // Stage 2: the registry's unique ASCII-case-insensitive fold. This is the
        // one inbound stage that MAY accept a `before` spelling, and only for the
        // nine pairs that differ by letter case alone: FR-026's case-folding note
        // calls that a match between two forms of one word. `bash` -> `Bash`,
        // `write` -> `Write` and so on must fold; a pair that differs by more
        // than case must not, because folding `ask_user` onto `AskUserQuestion`
        // would be a translation rather than a spelling match.
        let folded = canonical_dispatch_name(&registry, pair.before);
        if pair.before.eq_ignore_ascii_case(pair.after) {
            assert_eq!(
                folded.as_deref(),
                Some(pair.after),
                "`{}` must fold onto `{}` at the dispatch lookup (FR-026's case-folding note)",
                pair.before,
                pair.after
            );
        } else {
            assert_eq!(
                folded, None,
                "`{}` is folded onto `{folded:?}`, so the case fold is translating a retired \
                 name instead of matching two forms of one word (FR-026 clause 3)",
                pair.before
            );
        }

        // The consequence at the lookup an incoming *call* actually reaches: for
        // a case-only pair the retired spelling is dispatched as its adopted
        // name, and for every other pair the pipeline refuses it outright below.
        // Either way nothing advertises the retired spelling - cases 1 to 3 read
        // the keys, names and definitions, and FR-033 reads the menus.

        // Stage 3: the pipeline's own resolve step, called with the spelling as
        // sent. `resolve_call` looks the name up exactly, so it refuses every
        // retired spelling - the case-only pairs included. The fold that accepts
        // `bash` lives one stage earlier (asserted above) and rewrites the pending
        // call before this function runs; `resolve_call` is public and a direct
        // caller need not have canonicalised, which is why the exact refusal has
        // to hold here as well.
        let resolved = resolve_call(&registry, pair.before, "{}");
        let refusal = resolved.tool.as_ref().err().unwrap_or_else(|| {
            panic!(
                "`{}` resolved to a registered tool (FR-026 clause 3)",
                pair.before
            )
        });
        assert!(
            refusal
                .to_string()
                .contains(&format!("Unknown tool: '{}'", pair.before)),
            "the refusal for `{}` does not name the spelling the client sent: {refusal}",
            pair.before
        );
        assert_eq!(
            refusal.status(),
            ToolCallStatus::Error,
            "a refused call must be recorded as an error, never as a success (FR-026)"
        );
        assert!(refusal.is_error(), "and carry `is_error` (FR-014)");

        // The hint. FR-026 narrows its promise ("which will name `after` wherever
        // the similarity threshold allows"), so the assertion is conditional -
        // but a pair whose `before` is `after` modulo case MUST be hinted, and
        // that is the pair a ported Claude Code skill runs into.
        let hint = suggest_tool_name(&registry, pair.before);
        if pair.before.eq_ignore_ascii_case(pair.after) {
            assert_eq!(
                hint.as_deref(),
                Some(pair.after),
                "typing the retired `{}` must suggest `{}`: the two spellings differ only by \
                 letter case, which the pool ranks as an equality (FR-021)",
                pair.before,
                pair.after
            );
        }
        if let Some(hint) = hint {
            hinted += 1;
            assert!(
                registry.get(&hint).is_some(),
                "the hint for `{}` names `{hint}`, which is not a registered tool - a hint \
                 drawn from anything but the registry teaches a dead name (FR-033)",
                pair.before
            );
            assert!(
                !retired().contains(&hint.as_str()),
                "the hint for `{}` names the retired spelling `{hint}` (FR-033)",
                pair.before
            );
        }
    }

    assert!(
        hinted > 0,
        "no retired spelling produced a hint at all, so the mechanism is unreachable and this \
         case proves nothing (FR-021, FR-026)"
    );
}

// ---------------------------------------------------------------------------
// Case 6: the agent loop refuses the call, on the path every surface shares
// ---------------------------------------------------------------------------

/// A scripted provider whose first reply is one retired-spelling tool call and
/// whose every later reply is a plain "done".
///
/// The second reply matters: a loop whose call is refused asks the model again,
/// so a client that always answered with a tool call would drive the run until
/// the step limit and produce a thousand recorded calls. Answering once and then
/// closing keeps the case to one recorded refusal.
struct ScriptedProvider {
    retired: &'static str,
    calls: Arc<Mutex<usize>>,
}

struct ScriptedClient {
    retired: &'static str,
    calls: Arc<Mutex<usize>>,
}

#[async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        let first = {
            let mut calls = self.calls.lock().expect("call counter lock");
            *calls += 1;
            *calls == 1
        };
        let events = if first {
            let args = json!({ "path": "notes.txt" }).to_string();
            vec![
                StreamEvent::ToolCallStart {
                    id: "call_1".to_string(),
                    name: self.retired.to_string(),
                },
                StreamEvent::ToolCallDelta {
                    id: "call_1".to_string(),
                    args_json: args,
                },
                StreamEvent::ToolCallEnd {
                    id: "call_1".to_string(),
                },
                StreamEvent::Usage {
                    input_tokens: 3,
                    output_tokens: 1,
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::ToolUse,
                },
            ]
        } else {
            vec![
                StreamEvent::TextDelta {
                    text: "done".to_string(),
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::Stop,
                },
            ]
        };
        Ok(Box::pin(stream::iter(events)))
    }
}

#[async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
        "FR-026 probe provider"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "fr026-probe".to_string(),
            provider_id: "ollama".to_string(),
            name: "Probe".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
            },
            context_window: 32_000,
            max_output: Some(4_096),
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
        _options: &HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        Ok(Box::new(ScriptedClient {
            retired: self.retired,
            calls: Arc::clone(&self.calls),
        }))
    }
}

/// A processor over an in-memory store and the default registry, with
/// auto-approval **on**.
///
/// Auto-approval is the point: a `before` call must be refused *before* the
/// permission stage, so the refusal must not be something only a denied prompt
/// can produce. If the name were accepted anywhere, an auto-approving loop would
/// run it.
fn make_processor(retired: &'static str) -> Result<SessionProcessor> {
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        retired,
        calls: Arc::new(Mutex::new(0)),
    }));

    let event_bus = Arc::new(EventBus::new(1024));
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    // The credential lives in this case's in-memory database rather than an
    // environment variable: no process-global state is touched, so no sibling
    // test can observe it and AGENTS.md's env-isolation rules do not apply.
    storage
        .set_provider_auth("ollama", "fr026-probe-key")
        .expect("provider credential stored");
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));

    Ok(SessionProcessor {
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
            config: Arc::new(RagentConfig::default()),
            file_mtimes: Vec::new(),
            env_overrides_present: false,
        })),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
        auto_approve: true,
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
        pending_verdicts: ragent_agent::session::pending::PendingVerdicts::new(),
    })
}

/// The agent the probe drives.
fn probe_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "fr026-probe".to_string(),
    });
    agent
}

/// The `(tool, status, error)` of every recorded tool call in a reply.
fn recorded_calls(reply: &Message) -> Vec<(String, String, Option<String>)> {
    reply
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::ToolCall { tool, state, .. } => Some((
                tool.clone(),
                format!("{:?}", state.status),
                state.error.clone(),
            )),
            _ => None,
        })
        .collect()
}

/// FR-026 clause 3, end to end: a turn whose model emits a retired spelling
/// records a refused call.
///
/// This is the observable the TUI prompt, `ragent run`, the `/v1` adapter and the
/// ACP server all inherit, because all four call
/// [`SessionProcessor::process_message`] and none resolves a tool name of its own
/// (case 7). The call is asserted to be recorded under the spelling the model
/// sent - not the adopted one - so a silent translation fails here, and its status
/// is asserted to be `Error` with the refusal sentence attached.
#[tokio::test]
async fn test_the_agent_loop_refuses_a_before_call_and_appends_its_refusal() -> Result<()> {
    // A directory of this case's own, so nothing reads or writes the repository.
    let dir = tempfile::tempdir().expect("session dir");

    // Two pairs, one from each half of the table: `write` -> `Write` differs by
    // letter case only, so the call is canonicalised and then refused for its
    // payload; `skill_manage` -> `Skill` differs in words, so the call names no
    // tool at all. A translation on either route fails the assertions below.
    // Neither tool is ever executed: both are absent from
    // `SKILL_ALWAYS_ALLOWED_TOOLS`, so they reach the permission layer, which this
    // harness answers `Ask` -> denied.
    let mut refused = 0usize;
    for pair in TOOL_RENAMES
        .iter()
        .filter(|pair| matches!(pair.before, "write" | "skill_manage"))
    {
        let processor = make_processor(pair.before)?;
        let session = processor
            .session_manager
            .create_session(dir.path().to_path_buf())
            .expect("session created");

        // The turn is boxed: a whole session's future does not belong on the
        // stack (clippy's `large_futures`).
        let reply = Box::pin(processor.process_message(
            &session.id,
            "call the retired tool",
            &probe_agent(),
            Arc::new(AtomicBool::new(false)),
        ))
        .await?;

        let calls = recorded_calls(&reply);
        assert_eq!(
            calls.len(),
            1,
            "the retired `{}` call was not recorded exactly once (FR-026): {calls:?}",
            pair.before
        );
        let (tool, status, error) = &calls[0];

        // The name the loop recorded is the one that reached the pipeline. A pair
        // whose spellings differ only by case is canonicalised first - FR-026's
        // case-folding note permits exactly that - so its record carries the
        // adopted spelling. Every other pair keeps the spelling the model sent, so
        // the refusal can name it and the hint can point at the replacement.
        let expected = if pair.before.eq_ignore_ascii_case(pair.after) {
            pair.after
        } else {
            pair.before
        };
        assert_eq!(
            tool, expected,
            "the recorded call names `{tool}`, so an inbound stage translated the retired \
             `{}` onto something other than `{expected}` (FR-026 clause 3)",
            pair.before
        );
        assert_eq!(
            status, "Error",
            "the retired `{}` call is recorded as `{status}`; it must be an error, never a \
             success (FR-014, FR-026)",
            pair.before
        );
        let error = error
            .as_deref()
            .unwrap_or_else(|| panic!("the refused `{}` call carries no error", pair.before));
        // `unknown_tool` quotes the spelling as an "Unknown tool: '<name>'" clause
        // and `schema_violation` as "Invalid arguments for tool '<name>'", so the
        // assertion accepts either shape rather than pinning the punctuation of a
        // sentence another module owns.
        assert!(
            error.contains(&format!("'{expected}'")),
            "the refusal sentence does not name `{expected}`: {error}"
        );
        if expected == pair.before {
            assert!(
                error.contains("Unknown tool"),
                "a spelling that names no registered tool must be refused as unknown, not as a \
                 bad payload: {error}"
            );
        }
        assert!(
            !error.contains("was executed"),
            "the refusal for `{}` claims the tool ran: {error}",
            pair.before
        );
        if expected == pair.after {
            // Nothing was written: the case's directory is empty.
            let entries: Vec<_> = std::fs::read_dir(dir.path())
                .expect("read the case directory")
                .flatten()
                .map(|entry| entry.file_name())
                .collect();
            assert!(
                entries.is_empty(),
                "the retired `{}` call executed and wrote into the session directory: \
                 {entries:?}",
                pair.before
            );
        }
        refused += 1;
    }

    assert_eq!(
        refused, 2,
        "both probe pairs must be exercised; a filter that matched fewer would make the \
         assertions above vacuous"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Case 7: every inbound path reaches the one resolution chain
// ---------------------------------------------------------------------------

/// Read one workspace file, panicking with the path when it is missing.
fn read_source(relative: &str) -> String {
    let path = workspace_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// `file:line: text` for every **code** line of `source` containing one of
/// `needles`.
///
/// Comment lines are skipped: several of these files explain in prose that they
/// delegate to the shared path, and a scan that read those sentences would demand
/// an edit to the explanation.
fn hits(source: &str, needles: &[&str]) -> Vec<String> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.trim_start().starts_with("//"))
        .filter(|(_, line)| needles.iter().any(|needle| line.contains(needle)))
        .map(|(index, line)| format!("{}: {}", index + 1, line.trim()))
        .collect()
}

/// FR-026 clause 3's "at every inbound path" half, pinned where the claim holds:
/// each of the four surfaces drives [`SessionProcessor::process_message`], and
/// none of them resolves a tool name of its own.
///
/// - **the TUI prompt** - `crates/ragent-tui/src/app/session_ops.rs` (the
///   `process_message` call the prompt submit makes);
/// - **`ragent run`** - `src/main.rs` (the `Commands::Run` arm, which is where
///   the headless turn is built - `src/cli.rs` only parses the arguments);
/// - **the OpenAI-compatible `/v1` surface** - `crates/ragent-server/src/routes/openai.rs`;
/// - **the ACP server** - `crates/ragent-agent/src/acp/server.rs`.
///
/// The second half of each check is the one that matters: a surface that resolved
/// a name itself would have to re-implement FR-026's refusal, and could regress
/// on its own. Case 6 proves the shared function's behaviour once, for all four.
#[test]
fn test_the_inbound_paths_share_one_resolution_chain() {
    let surfaces: [(&str, &str); 4] = [
        ("crates/ragent-tui/src/app/session_ops.rs", "the TUI prompt"),
        ("src/main.rs", "`ragent run`"),
        (
            "crates/ragent-server/src/routes/openai.rs",
            "the OpenAI-compatible /v1 surface",
        ),
        ("crates/ragent-agent/src/acp/server.rs", "the ACP server"),
    ];
    // A surface resolving a tool name itself would bypass the shared refusal.
    let private_resolution = [
        "resolve_inbound_tool_name",
        "canonical_dispatch_name",
        "resolve_call(",
        "TOOL_RENAMES",
        "tool::rename",
    ];

    for (file, label) in surfaces {
        let source = read_source(file);
        let drives_the_loop = hits(&source, &["process_message("]);
        assert!(
            !drives_the_loop.is_empty(),
            "{label} ({file}) does not call `process_message`, so it runs its own tool path \
             and FR-026's refusal would have to be re-implemented there"
        );
        let own = hits(&source, &private_resolution);
        assert!(
            own.is_empty(),
            "{label} ({file}) resolves a tool name itself, so a retired spelling could be \
             handled differently on this path than on the shared one (FR-026 clause 3):\n{}",
            own.join("\n")
        );
    }

    // The pipeline's resolve step is what all four reach; it must still build the
    // refusal and fill the hint, and must not consult the rename table.
    let dispatch = read_source("crates/ragent-agent/src/session/dispatch.rs");
    assert!(
        !hits(
            &dispatch,
            &["ToolInputError::unknown_tool(name, suggestion.as_deref())"]
        )
        .is_empty(),
        "the dispatch pipeline no longer builds the unknown-tool refusal, so the sentence \
         FR-026 promises to every surface has moved and this case did not follow it"
    );
    assert!(
        !hits(&dispatch, &["suggest_tool_name(registry, name)"]).is_empty(),
        "the dispatch pipeline no longer fills the did-you-mean hint (FR-021, FR-026)"
    );
    let table_hits = hits(&dispatch, &["TOOL_RENAMES", "Rename::"]);
    assert!(
        table_hits.is_empty(),
        "the dispatch pipeline consults the FR-023 rename table, which is the alias FR-026 \
         forbids:\n{}",
        table_hits.join("\n")
    );
}
