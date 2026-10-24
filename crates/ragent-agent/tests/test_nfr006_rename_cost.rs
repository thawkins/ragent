//! Guard: the FR-023 rename adds no per-step cost (spec `uniformtools` NFR-006).
//!
//! NFR-006 has three clauses, and each one can be broken silently by an edit
//! that looks helpful:
//!
//! 1. **tool names remain borrowed `&str` values compared directly** - a
//!    `fn name(&self) -> String`, or a resolver that builds a `String` to
//!    compare against, would allocate on every step of every turn;
//! 2. **the FR-023 table is read only by tests and by migration sites, and no
//!    dispatch step consults it to translate a name** - the tempting change
//!    after the rename is a `Rename::after_of(name)` call in the dispatch
//!    pipeline, "so a stale spelling still works"; that is exactly the alias
//!    FR-026 forbids, and it would also put a 13-row linear scan in front of
//!    every tool call;
//! 3. **resolution, the repeat guard and the definition cache key on the new
//!    spelling with the same cost as the old one** - the rename must not have
//!    left one of them keyed on a spelling the registry no longer holds.
//!
//! Clauses 2 and 3 are checked here; clause 1 is checked by the type assertions
//! in [`test_tool_names_stay_borrowed_and_the_table_lookup_borrows_too`], because
//! "borrowed" is a compile-time property that a test can only pin by naming the
//! borrowed type explicitly.
//!
//! ## Why the source scan is function-granular
//!
//! A file-granular exemption would be too coarse to mean anything: four of the
//! twelve legitimate table reads live in a file that *also* holds a dispatch-path
//! function, so exempting `alias_table.rs` wholesale would happily allow
//! `Rename::after_of` inside `resolve_inbound_tool_name` - the resolver the
//! dispatch pipeline calls for every tool call. Every exemption below therefore
//! names the one function it covers, and the scan reports the enclosing `fn` of
//! each hit.
//!
//! ## What this file deliberately does not do
//!
//! - **It does not measure allocation.** There is no counting allocator in this
//!   workspace, and adding one to the binary under test would be a bigger change
//!   than the requirement. The scan plus the borrowed-type assertions are the
//!   mechanical half; `specs/uniformtools/TESTPLAN.md` TC-024 step 4 is the
//!   human half (read the debug log of a two-tool turn and confirm no
//!   rename/translate step appears).
//! - **It does not re-assert what the rename guards already own.** That every
//!   `after` resolves and every `before` does not is
//!   `test_tool_rename_atomic_landing.rs`; the table's own thirteen pairs are
//!   `test_tool_rename_table.rs`; the alias table's narrowing is
//!   `test_tool_alias_table.rs`. This file reads the table and asserts the
//!   *cost* properties.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use ragent_agent::event::EventBus;
use ragent_agent::session::permissions::{
    RepeatTracker, TOOL_REPEAT_LIMIT, check_tool_repeat_guard,
};
use ragent_agent::tool::create_default_registry;
use ragent_agent::tool::rename::{Rename, TOOL_RENAMES};
use ragent_llm::provider::tool_cache::{ToolFormat, cached_tools};
use ragent_types::ToolDefinition;
use serde_json::{Value, json};

/// The symbols that reach the FR-023 table.
///
/// Every way of reading the table in this tree goes through one of these: the
/// constant itself, its module paths, the three lookups on [`Rename`], and the
/// private `after_spelling_of` helper the config loader shares between its two
/// self-mapping rules. A reader that dodged all nine would have to re-declare the
/// pairs, which is the duplication `crates/ragent-types/src/tool_rename.rs`
/// exists to prevent.
const RENAME_TABLE_SYMBOLS: [&str; 9] = [
    "TOOL_RENAMES",
    "tool_rename",
    "Rename::",
    "after_of(",
    "before_of(",
    "is_retired(",
    "is_rename_spelling(",
    "after_spelling_of(",
    "tool::rename",
];

/// One legitimate read of the FR-023 table, named by file **and** the function
/// the read sits in.
///
/// An empty `function` means module scope: an `use` line, a `pub mod`
/// declaration, the table constant itself. Every entry carries the reason it is
/// not a dispatch step, and the scan asserts in both directions, so an entry that
/// stops matching is reported rather than quietly widening the exemption.
struct TableReader {
    /// Workspace-relative path of the file.
    file: &'static str,
    /// The enclosing `fn`, or `""` for module scope.
    function: &'static str,
    /// Why this read is a migration site rather than a per-step translation.
    reason: &'static str,
}

/// Every legitimate reader of the FR-023 table in non-test source (NFR-006).
///
/// Six *functions* and six module-scope items, in four crates. Two of them sit in
/// the dispatch crate and are the reason the exemption has to be per function:
/// see the module docs.
const TABLE_READERS: &[TableReader] = &[
    TableReader {
        file: "crates/ragent-types/src/tool_rename.rs",
        function: "",
        reason: "the table itself and its `impl` - the one definition the rest reads",
    },
    TableReader {
        file: "crates/ragent-types/src/tool_rename.rs",
        function: "after_of",
        reason: "the table's own `before` -> `after` lookup; pure, borrowed, no allocation",
    },
    TableReader {
        file: "crates/ragent-types/src/tool_rename.rs",
        function: "before_of",
        reason: "the table's own `after` -> `before` lookup; guards and reports only",
    },
    TableReader {
        file: "crates/ragent-types/src/tool_rename.rs",
        function: "is_retired",
        reason: "a predicate over `after_of`, used by guards and migration sites",
    },
    TableReader {
        file: "crates/ragent-types/src/tool_rename.rs",
        function: "is_rename_spelling",
        reason: "the both-columns predicate the config self-mapping rule and the audit key on",
    },
    TableReader {
        file: "crates/ragent-types/src/lib.rs",
        function: "",
        reason: "the module declaration and its re-export; declares the table, reads nothing",
    },
    TableReader {
        file: "crates/ragent-agent/src/tool/rename.rs",
        function: "",
        reason: "the `ragent-agent` re-export, so existing reader paths keep working",
    },
    // `alias_table.rs` also imports the type (`use super::rename::Rename;`), which
    // spells none of the scanned symbols - the table is reached through it only by
    // the call below, so that call is the read this list covers. Naming the import
    // as a second entry would be an exemption nothing could ever match, and the
    // reverse check rejects those.
    TableReader {
        file: "crates/ragent-agent/src/tool/alias_table.rs",
        function: "from_aliases",
        reason: "drops a configured `tools.aliases` entry naming a retired spelling; runs \
                 once per turn when the table is built from the config, never per tool call",
    },
    TableReader {
        file: "crates/ragent-agent/src/hooks/mod.rs",
        function: "",
        reason: "module import for the load-time matcher report below",
    },
    TableReader {
        file: "crates/ragent-agent/src/hooks/mod.rs",
        function: "retired_matcher_spellings",
        reason: "FR-028's load-time report of a hook matcher naming a retired spelling; \
                 derived once per turn from the hook list, never on a tool call",
    },
    TableReader {
        file: "crates/ragent-config/src/tools.rs",
        function: "validate_alias_spellings",
        reason: "FR-031's config-load refusal of a retired alias key or target; runs on load",
    },
    TableReader {
        file: "crates/ragent-config/src/tools.rs",
        function: "after_spelling_of",
        reason: "the both-columns lookup the load-time self-mapping rule needs",
    },
    TableReader {
        file: "crates/ragent-tui/src/widgets/message_widget.rs",
        function: "",
        reason: "module import for the transcript display arm below",
    },
    TableReader {
        file: "crates/ragent-tui/src/widgets/message_widget.rs",
        function: "canonical_tool_name",
        reason: "paints a STORED pre-rename tool-call record (FR-028 replays it verbatim); \
                 a TUI render arm, not an agent-loop stage, and it never reaches dispatch",
    },
    TableReader {
        file: "crates/ragent-types/src/tool_bridge.rs",
        function: "",
        reason: "module import of the table for the FR-032 bridge rule below",
    },
    TableReader {
        file: "crates/ragent-types/src/tool_bridge.rs",
        function: "check_dynamic_bridge",
        reason: "FR-032's refusal of a dynamically sourced tool whose surfaced name is a \
                 retired spelling; runs once per bridged tool at registration, never on a \
                 tool call (and with a namespaced name the lookup cannot match at all)",
    },
];

/// The workspace root, resolved from this crate's manifest directory.
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> sits two levels below the workspace root")
        .to_path_buf()
}

/// Every non-test `.rs` file under `dir`, recursively.
///
/// `tests/`, `benches/`, `examples/` and `target/` are skipped: a test may read
/// the table freely (NFR-006 says so explicitly), and a bench or example is not a
/// step of the agent loop.
fn collect_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if matches!(name, "tests" | "benches" | "examples" | "target" | ".git") {
                continue;
            }
            collect_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && path.file_name().is_some_and(|n| n != "build.rs")
        {
            out.push(path);
        }
    }
}

/// One line that reaches the FR-023 table, with the `fn` it sits in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TableRead {
    file: String,
    function: String,
    line: usize,
    text: String,
}

impl TableRead {
    /// `file:line: (fn) text`, the form the failure message quotes.
    fn render(&self) -> String {
        let function = if self.function.is_empty() {
            "<module scope>"
        } else {
            self.function.as_str()
        };
        format!("{}:{}: ({function}) {}", self.file, self.line, self.text)
    }
}

/// The `fn` a line sits in, matched the way rustdoc attributes a hit: the
/// nearest preceding `fn` signature at any indentation.
///
/// Only the *signature* is matched, so a call to `foo(...)` cannot be mistaken for
/// a definition. A free function, an inherent method and a trait impl method are
/// all reported by their bare name, which is what the exemption list names.
fn enclosing_function(lines: &[&str], upto: usize) -> String {
    for line in lines[..=upto].iter().rev() {
        let trimmed = line.trim_start();
        // `fn name(` / `async fn name(` / `pub fn name(` / `pub(crate) fn name(`,
        // and `fn name<` / `fn name()` for a generic or unit signature.
        let mut rest = trimmed;
        for prefix in [
            "pub(crate) ",
            "pub(super) ",
            "pub(in ",
            "pub ",
            "async ",
            "unsafe ",
            "const ",
            "extern ",
        ] {
            if let Some(stripped) = rest.strip_prefix(prefix) {
                rest = stripped;
            }
        }
        let Some(after_fn) = rest.strip_prefix("fn ") else {
            continue;
        };
        let name: String = after_fn
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            return name;
        }
    }
    String::new()
}

/// `true` when a line is a comment, and therefore prose rather than a read.
///
/// Several doc comments in this tree *state* that the table is not consulted on a
/// dispatch path; treating those sentences as hits would make the guard demand an
/// exemption for explaining itself.
fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with("//")
}

/// Every non-test source line in the workspace that reaches the FR-023 table.
fn table_reads() -> Vec<TableRead> {
    let root = workspace_root();
    let mut files = Vec::new();
    collect_sources(&root.join("crates"), &mut files);
    collect_sources(&root.join("src"), &mut files);
    files.sort();

    let mut reads = Vec::new();
    for path in files {
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let relative = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let lines: Vec<&str> = source.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if is_comment(line) {
                continue;
            }
            if !RENAME_TABLE_SYMBOLS
                .iter()
                .any(|symbol| line.contains(symbol))
            {
                continue;
            }
            reads.push(TableRead {
                file: relative.clone(),
                function: enclosing_function(&lines, index),
                line: index + 1,
                text: line.trim().to_string(),
            });
        }
    }
    reads
}

/// `true` when `read` is covered by an exemption.
fn is_exempt(read: &TableRead) -> bool {
    TABLE_READERS
        .iter()
        .any(|reader| reader.file == read.file && reader.function == read.function)
}

/// The written reason `read` is exempt, if it is.
fn reason_for(read: &TableRead) -> Option<&'static str> {
    TABLE_READERS
        .iter()
        .find(|reader| reader.file == read.file && reader.function == read.function)
        .map(|reader| reader.reason)
}

/// Case 1 of NFR-006: no dispatch step consults the FR-023 table.
///
/// The dispatch surface is the three directory trees the agent loop resolves,
/// guards, validates and executes through. Every hit inside them that is *not*
/// the alias table's build-time refusal would be a translation on the per-call
/// path, so the assertion is equality, not "at most": a new read fails here even
/// if it is added to `TABLE_READERS`, because the dispatch trees are checked
/// separately from the workspace-wide exemption list.
#[test]
fn test_no_dispatch_step_consults_the_rename_table() {
    let dispatch_trees = [
        "crates/ragent-agent/src/session/",
        "crates/ragent-agent/src/tool/",
        "crates/ragent-agent/src/backend/",
    ];
    // The two reads that legitimately live inside a dispatch *tree*: the table's
    // own re-export, and the alias table's once-per-turn build. Neither runs per
    // tool call - `resolve_inbound_tool_name`, the function the pipeline calls for
    // every call, reads the built `AliasTable`, not the FR-023 table.
    let allowed_in_dispatch_trees = [
        ("crates/ragent-agent/src/tool/rename.rs", ""),
        (
            "crates/ragent-agent/src/tool/alias_table.rs",
            "from_aliases",
        ),
    ];

    let offending: Vec<String> = table_reads()
        .into_iter()
        .filter(|read| {
            dispatch_trees
                .iter()
                .any(|tree| read.file.starts_with(tree))
        })
        .filter(|read| {
            !allowed_in_dispatch_trees
                .iter()
                .any(|(file, function)| *file == read.file && *function == read.function)
        })
        .map(|read| read.render())
        .collect();

    assert!(
        offending.is_empty(),
        "a dispatch step consults the FR-023 table, so every tool call pays for a \
         rename lookup and a stale spelling would be translated rather than refused \
         (NFR-006, FR-026). Move the read to a migration site and drop it:\n{}",
        offending.join("\n")
    );
}

/// Case 2 of NFR-006: the table is read only by the listed migration sites.
///
/// Checked in both directions. A read that is not exempted fails (a new per-step
/// reader); an exemption that no longer matches anything also fails (a stale
/// entry, which would silently widen the next time the file changed).
#[test]
fn test_the_rename_table_is_read_only_by_listed_migration_sites() {
    let reads = table_reads();

    let unexempted: Vec<String> = reads
        .iter()
        .filter(|read| !is_exempt(read))
        .map(TableRead::render)
        .collect();
    assert!(
        unexempted.is_empty(),
        "the FR-023 table gained a reader that is not a listed migration site \
         (NFR-006). Either remove the read, or add a `TABLE_READERS` entry naming the \
         function and the written reason it is not a per-step translation:\n{}",
        unexempted.join("\n")
    );

    let unmatched: Vec<String> = TABLE_READERS
        .iter()
        .filter(|reader| {
            !reads
                .iter()
                .any(|read| read.file == reader.file && read.function == reader.function)
        })
        .map(|reader| {
            format!(
                "{} ({}) - {}",
                reader.file,
                if reader.function.is_empty() {
                    "<module scope>"
                } else {
                    reader.function
                },
                reader.reason
            )
        })
        .collect();
    assert!(
        unmatched.is_empty(),
        "a `TABLE_READERS` exemption no longer matches any read, so the list no longer \
         describes the tree:\n{}",
        unmatched.join("\n")
    );

    // Every exemption is also reachable as a *reason*, which is what makes the
    // list an audit rather than a filter: a reader that is exempt but whose reason
    // was never written would have failed to compile into this slice.
    for read in &reads {
        assert!(
            reason_for(read).is_some_and(|reason| !reason.trim().is_empty()),
            "exempted read with no written reason: {}",
            read.render()
        );
    }
}

/// Case 1 of NFR-006, at the type level: names stay borrowed `&str` values, and so
/// does the table's answer.
///
/// The bindings below name `&str` and `Option<&'static str>` explicitly rather than
/// leaning on inference, so a change to `fn name(&self) -> String` or to
/// `Rename::after_of -> Option<String>` stops this file compiling. That is the
/// strongest assertion available: an allocation added to either signature cannot
/// pass a test that stores the result in a borrowed type.
#[test]
fn test_tool_names_stay_borrowed_and_the_table_lookup_borrows_too() {
    let registry = create_default_registry();

    for pair in TOOL_RENAMES {
        let tool = registry
            .get(pair.after)
            .unwrap_or_else(|| panic!("{} must be registered", pair.after));

        // `Tool::name` yields a borrow of the tool's own field; nothing is
        // allocated to ask a tool what it is called.
        let borrowed: &str = tool.name();
        assert_eq!(borrowed, pair.after);
        // The borrowed name is the registry's own key, so the lookup needs no
        // translation: the same `&str` goes straight into the map.
        assert!(
            registry.get(borrowed).is_some(),
            "the name a tool reports must be the key it is stored under"
        );

        // The table's own answer is a `&'static str`, not an owned `String`.
        let after: Option<&'static str> = Rename::after_of(pair.before);
        assert_eq!(after, Some(pair.after));
        let before: Option<&'static str> = Rename::before_of(pair.after);
        assert_eq!(before, Some(pair.before));
    }
}

/// Case 3a of NFR-006: resolution keys on the new spelling at the same cost.
///
/// Three properties together: the adopted spelling resolves on the plain registry
/// lookup (no table read - the scan above is what proves *that* half), the retired
/// spelling is absent from the map so `get` refuses it, and the case-folded
/// fallback of `resolve` is what answers a case variant - and it answers with the
/// *registered* spelling, not with the spelling it was handed.
#[test]
fn test_resolution_keys_on_the_new_spelling() {
    let registry = create_default_registry();

    for pair in TOOL_RENAMES {
        let tool = registry
            .get(pair.after)
            .unwrap_or_else(|| panic!("{} must be registered", pair.after));
        assert_eq!(
            tool.name(),
            pair.after,
            "the tool reports the adopted spelling"
        );
        assert!(
            registry.get(pair.before).is_none(),
            "{} must not be a registry key (FR-026)",
            pair.before
        );
        assert!(
            !registry.list().contains(&pair.before.to_string()),
            "{} must not be listed",
            pair.before
        );

        // A word-changing pair (`ask_user` -> `AskUserQuestion`) is not a case
        // variant, so the folded fallback cannot reach it either.
        if !pair.before.eq_ignore_ascii_case(pair.after) {
            assert!(
                registry.resolve(pair.before).is_none(),
                "{} differs from {} by more than letter case, so it must not resolve",
                pair.before,
                pair.after
            );
            continue;
        }

        // A case-only pair (`bash` -> `Bash`) is reachable by the folded fallback,
        // and the answer carries the registered spelling - which is why every later
        // stage sees one name.
        let folded = registry
            .resolve(pair.before)
            .unwrap_or_else(|| panic!("{} must reach {} by case folding", pair.before, pair.after));
        assert_eq!(
            folded.name(),
            pair.after,
            "the folded lookup must answer with the registered spelling"
        );
    }
}

/// Case 3b of NFR-006: the definition cache keys on the new spelling.
///
/// Two halves, because the tree has two caches in this path:
///
/// - `ToolRegistry::definitions` reuses one sorted `Arc<Vec<ToolDefinition>>` while
///   the registry is unchanged, and the names in it are the adopted spellings;
/// - `ragent_llm`'s serialised-tool cache fingerprints the definition *names*, so a
///   set spelled with a retired name is a different entry rather than a stale hit
///   on the adopted one. That second half is what stops a rename from being served
///   out of a cache populated before it.
#[test]
fn test_the_definition_cache_keys_on_the_new_spelling() {
    let registry = create_default_registry();
    let defs = registry.definitions();
    assert!(
        Arc::ptr_eq(&defs, &registry.definitions()),
        "the sorted-definition cache is reused while the registry is unchanged"
    );
    let cached_names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    for pair in TOOL_RENAMES {
        assert!(
            cached_names.contains(&pair.after),
            "the definition cache must hold the adopted spelling {}",
            pair.after
        );
        assert!(
            !cached_names.contains(&pair.before),
            "the definition cache must not hold the retired spelling {}",
            pair.before
        );
    }
    assert!(
        defs.windows(2).all(|w| w[0].name <= w[1].name),
        "the cache is sorted by name, as it was before the rename"
    );

    // The serialised cache. Two definition sets identical except for one tool's
    // spelling are two entries, so the rename is not shadowed by a pre-rename
    // buffer; each set is stable on a repeat call, so the hit path is unchanged.
    //
    // Compared by *bytes*, not by `Arc::ptr_eq`: the cache is process-wide and
    // other child files of this binary call `invalidate_tool_cache()`, so pointer
    // identity is not a property this file can hold. Byte equality is exactly the
    // claim (`a hit is byte-identical to a miss`, FR-003) and it survives a clear.
    let schema = json!({ "type": "object", "properties": {}, "required": [] });
    let definition = |name: &str| ToolDefinition {
        name: name.to_string(),
        description: "same description".to_string(),
        parameters: schema.clone(),
    };
    let adopted = vec![definition("Bash")];
    let retired = vec![definition("bash")];

    let adopted_first = cached_tools(ToolFormat::OpenAi, &adopted);
    let retired_first = cached_tools(ToolFormat::OpenAi, &retired);
    assert_ne!(
        adopted_first.bytes, retired_first.bytes,
        "the serialised cache must key on the tool name, so a retired spelling is a \
         separate entry rather than a hit on the adopted one"
    );
    assert_eq!(
        cached_tools(ToolFormat::OpenAi, &adopted).bytes,
        adopted_first.bytes,
        "a repeated encode of the same tool set is byte-identical"
    );

    let payload = String::from_utf8_lossy(&adopted_first.bytes).to_string();
    assert!(
        payload.contains("\"name\":\"Bash\""),
        "the encoded tool list must carry the adopted spelling: {payload}"
    );
    assert!(
        !payload.contains("\"bash\""),
        "the encoded tool list must not carry a retired spelling: {payload}"
    );
}

/// Case 3c of NFR-006: the repeat guard keys on the new spelling.
///
/// The guard's key is `(tool_name, canonical args)` with the name as a borrowed
/// `&str`, so the rename changed the *key value* and nothing else:
///
/// - five identical `Bash` calls pass and the sixth is denied, which is the
///   unchanged budget under the new spelling (a name the guard special-cased would
///   not accumulate at all);
/// - alternating a retired spelling with the adopted one never accumulates,
///   because the two are different key values - a stale name cannot spend, or
///   reset, another tool's budget;
/// - a different adopted spelling resets the counter, so the guard is still
///   per-tool and not per-session.
#[tokio::test]
async fn test_the_repeat_guard_keys_on_the_new_spelling() {
    let trackers: Arc<Mutex<HashMap<String, RepeatTracker>>> = Arc::new(Mutex::new(HashMap::new()));
    let bus = Arc::new(EventBus::new(16));
    let input: Value = json!({ "command": "ls -la" });

    for _ in 0..TOOL_REPEAT_LIMIT {
        let outcome =
            check_tool_repeat_guard(&trackers, &bus, "s1", "Bash", &input, true, false, 0).await;
        assert!(
            outcome.is_none(),
            "five identical `Bash` calls must pass under the unchanged limit"
        );
    }
    let denied = check_tool_repeat_guard(&trackers, &bus, "s1", "Bash", &input, true, false, 0)
        .await
        .expect("the sixth identical `Bash` call must be denied");
    assert!(
        denied.contains("auto-denied"),
        "the unattended denial sentence is unchanged: {denied}"
    );

    // A retired spelling is a different key: alternating it with the adopted one
    // resets the consecutive counter on every call, so nothing is ever denied.
    let alternators: Arc<Mutex<HashMap<String, RepeatTracker>>> =
        Arc::new(Mutex::new(HashMap::new()));
    for step in 0..=(2 * TOOL_REPEAT_LIMIT) {
        let name = if step % 2 == 0 { "bash" } else { "Bash" };
        let outcome =
            check_tool_repeat_guard(&alternators, &bus, "s2", name, &input, true, false, 0).await;
        assert!(
            outcome.is_none(),
            "`bash` and `Bash` are different key values, so neither accumulates \
             (step {step})"
        );
    }

    // A different adopted spelling resets the counter: the key is per tool.
    let resets: Arc<Mutex<HashMap<String, RepeatTracker>>> = Arc::new(Mutex::new(HashMap::new()));
    for _ in 0..TOOL_REPEAT_LIMIT {
        let _ = check_tool_repeat_guard(&resets, &bus, "s3", "Bash", &input, true, false, 0).await;
    }
    let outcome =
        check_tool_repeat_guard(&resets, &bus, "s3", "Write", &input, true, false, 0).await;
    assert!(
        outcome.is_none(),
        "a different tool name resets the consecutive counter"
    );
}
