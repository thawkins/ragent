//! The declarative provider-name alias table and its inbound resolver
//! (spec `uniformtools` FR-015, decisions D5 and D7).
//!
//! A model, an ACP client or an MCP server may name a tool with a spelling
//! ragent does not register - the Anthropic spellings ragent's own rename
//! refused (`WebFetch`, `WebSearch`) and the OpenAI hosted-tool spellings
//! (`web_search`, `file_search`). [`AliasTable`] is the one place that maps such
//! a spelling onto a ragent tool name, and [`resolve_inbound_tool_name`] is the
//! one function that applies it, in the dispatch pipeline, before the
//! unknown-tool check.
//!
//! ## Why this is data and not a tool
//!
//! [`super::aliases`] holds *alias tools*: whole [`Tool`](super::Tool)
//! implementations registered under a second name (`update_file`, `run_code`).
//! Each one is a real registry entry, so it is advertised to the model, counted
//! by `tool_info`, listed by `/tools` and dispatched like any other. A table
//! entry none of those things: it is a spelling resolved on the way in and never
//! appears in an outbound tool list unless `tools.expose_alias_names` asks for it
//! (FR-015, task T-021). That keeps the surface count honest (FR-030).
//!
//! ## The table is narrowed by the rename, and the rename wins (D7)
//!
//! FR-023 to FR-030 rename thirteen ragent tools to their Claude Code spellings.
//! After that rename `Bash`, `Edit`, `Write`, `Glob`, `Grep`, `Skill`, the four
//! `Task*` tools, `AskUserQuestion`, `EnterPlanMode` and `ExitPlanMode` **are**
//! ragent names: they resolve on the registry lookup itself, which is the
//! cheapest and most reliable match there is. FR-026 forbids a second spelling
//! for a renamed tool and FR-031 refuses an alias whose key or target is a
//! retired spelling, so the built-in table holds only spellings that are ragent
//! names neither before nor after the rename.
//!
//! That is why [`BUILTIN_PROVIDER_ALIASES`] holds no mapping onto a spelling the
//! FR-023 rename retired, even though such a mapping would look useful today. It
//! is deliberately the post-rename table from day one: one table, never pruned
//! or re-pointed as the rename lands.
//!
//! Two spellings are deliberately absent:
//!
//! - `code_interpreter` is an OpenAI *hosted* tool with no local execution; the
//!   specification's Out of Scope section maps it to `No equivalent`. Mapping it
//!   onto any registered tool would claim ragent can run it, so its absence is
//!   the requirement, not an omission.
//! - `Read` is not an alias. The derivation rates that Anthropic target `Close`
//!   and leaves `read` under its own name; the case-insensitive hint in the
//!   unknown-tool error already suggests `read` for a call to `Read`, so an entry
//!   would add a second spelling of one tool for no gain.

use std::sync::Arc;

use ragent_config::{AliasesConfig, ToolsConfig};
use ragent_types::ToolDefinition;

use super::ToolRegistry;
use super::rename::Rename;

/// The `(spelling, target)` pairs an unresolved-target warning has already been
/// emitted for (FR-031).
///
/// Process-wide rather than per-session because the same configuration resolves
/// the same table in every session; per-entry rather than per-table because a
/// re-pointed entry is a different mistake and must report.
fn unresolved_targets_seen()
-> &'static std::sync::Mutex<std::collections::HashSet<(String, String)>> {
    static SEEN: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashSet<(String, String)>>,
    > = std::sync::OnceLock::new();
    SEEN.get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()))
}

/// Clears the unresolved-target ledger, so a test can observe the first report.
///
/// The ledger is process-wide (see [`report_unresolved_targets_once`]), and the
/// test binary shares one process across every file, so a test that asserts "the
/// first call reports and the second is silent" needs to start from a known
/// state.
#[doc(hidden)]
pub fn reset_unresolved_targets_for_test() {
    unresolved_targets_seen()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

/// One built-in row of the FR-015 alias table.
///
/// `spelling` is the provider spelling a client or model may emit; `ragent_name`
/// is the registered ragent tool it resolves to. Both are exact, case-sensitive
/// and ASCII (NFR-005).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderAlias {
    /// The provider spelling, e.g. `WebFetch`.
    pub spelling: &'static str,
    /// The ragent tool name it resolves to, e.g. `webfetch`.
    pub ragent_name: &'static str,
}

/// The built-in provider-spelling table requested by `tools.aliases: true`
/// (FR-015).
///
/// Ascending by [`ProviderAlias::spelling`] so the list reads like the other
/// checked-in tables in this workspace and a test can assert the order without
/// sorting.
///
/// Every target is a tool the default registry registers, and every spelling is
/// neither a retired ragent name nor one the FR-023 rename adopts, so no entry
/// can shadow a real registration or duplicate a rename (FR-026, FR-031).
pub const BUILTIN_PROVIDER_ALIASES: [ProviderAlias; 4] = [
    ProviderAlias {
        spelling: "WebFetch",
        ragent_name: "webfetch",
    },
    ProviderAlias {
        spelling: "WebSearch",
        ragent_name: "websearch",
    },
    ProviderAlias {
        spelling: "file_search",
        ragent_name: "codeindex_search",
    },
    ProviderAlias {
        spelling: "web_search",
        ragent_name: "websearch",
    },
];

/// One configured or built-in alias row, owned because `tools.aliases` is.
type OwnedAlias = (String, String);

/// The resolved alias table for one configuration (FR-015).
///
/// Built once per turn from `tools.aliases` so an inbound resolution is a short
/// linear scan over a handful of entries rather than a config read on the
/// dispatch path. Empty means "alias resolution is off", which is the default
/// and the pre-existing behaviour FR-015 requires when neither switch is set.
///
/// An entry is dropped rather than kept when it would change nothing: a
/// self-mapping (`Bash` -> `Bash`) maps a spelling onto the identical string, and
/// D7 says to delete such an entry rather than carry it as a no-op.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AliasTable {
    /// `(provider spelling, ragent tool name)`, ascending by spelling.
    entries: Vec<OwnedAlias>,
}

impl AliasTable {
    /// Builds the table for `tools` (FR-015).
    ///
    /// Returns an empty table when `tools.aliases` is absent, `false`, or an
    /// empty object, so an unconfigured build resolves every name against the
    /// registry alone.
    #[must_use]
    pub fn from_tools_config(tools: &ToolsConfig) -> Self {
        Self::from_aliases(&tools.aliases)
    }

    /// Builds the table for one `tools.aliases` value (FR-015).
    ///
    /// The two accepted input forms compose: `aliases: true` contributes
    /// [`BUILTIN_PROVIDER_ALIASES`] and any explicit entries are added on top,
    /// with an explicit entry winning a spelling collision because the user's
    /// own statement is the more specific one.
    #[must_use]
    pub fn from_aliases(aliases: &AliasesConfig) -> Self {
        if !aliases.is_enabled() {
            return Self::default();
        }
        let mut entries: Vec<OwnedAlias> = Vec::new();
        if aliases.uses_builtin_table() {
            for row in BUILTIN_PROVIDER_ALIASES {
                entries.push((row.spelling.to_string(), row.ragent_name.to_string()));
            }
        }
        for (spelling, target) in aliases.entries() {
            entries.retain(|(existing, _)| existing != spelling);
            if spelling.is_empty() || target.is_empty() || spelling == target {
                continue;
            }
            // FR-031: a retired ragent spelling is never consulted. The config
            // loader refuses such an entry and stops the load, so this arm is
            // unreachable from a file; it is the defence in depth that keeps the
            // *resolver* honest for a section built in memory (a test, a program
            // that assembles a `ToolsConfig` directly, a future caller), which is
            // the half of FR-031 a loader check alone cannot guarantee.
            if Rename::after_of(spelling).is_some() || Rename::after_of(target).is_some() {
                continue;
            }
            entries.push((spelling.clone(), target.clone()));
        }
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        Self { entries }
    }

    /// The entries whose target this build does not register (FR-031).
    ///
    /// An alias target that names no registered tool is *not* a rename problem:
    /// a plugin or MCP tool may be bridged after the configuration was loaded, so
    /// the entry cannot be refused at load time. Instead it is reported once -
    /// see [`AliasTable::report_unresolved_targets_once`] - and refused at the
    /// moment it is used, which [`resolve_inbound_tool_name`] already does by
    /// falling through to the unknown-tool refusal when the target is absent
    /// from the registry.
    ///
    /// Returns `(spelling, target)` pairs ascending by spelling, so a report
    /// reads like the table it came from.
    #[must_use]
    pub fn unresolved_targets(&self, registry: &ToolRegistry) -> Vec<(&str, &str)> {
        self.entries()
            .filter(|(_, target)| registry.get(target).is_none())
            .collect()
    }

    /// Logs each unresolved alias target once per process (FR-031).
    ///
    /// "Reported once" is per *entry*, not per turn: the table is rebuilt from the
    /// configuration on every turn, so a naive `warn!` here would repeat the same
    /// sentence on every step and train the reader to ignore it. The seen-set is
    /// process-wide and poison-tolerant, matching the other `OnceLock` registries
    /// in this crate; a repeated warning for the same `(spelling, target)` pair is
    /// suppressed, and a new or re-pointed entry still reports.
    ///
    /// Returns the number of entries reported by this call, so a caller (and a
    /// test) can tell "first report" from "already reported".
    pub fn report_unresolved_targets_once(&self, registry: &ToolRegistry) -> usize {
        let unresolved = self.unresolved_targets(registry);
        if unresolved.is_empty() {
            return 0;
        }
        let seen = unresolved_targets_seen();
        let mut seen = seen
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut reported = 0usize;
        for (spelling, target) in unresolved {
            if !seen.insert((spelling.to_string(), target.to_string())) {
                continue;
            }
            reported += 1;
            tracing::warn!(
                spelling = %spelling,
                target = %target,
                "tools.aliases entry targets a tool this build does not register; the entry is \
                 unresolved and any call through it will be refused (FR-031)"
            );
        }
        reported
    }

    /// Returns `true` when the table holds no entry, i.e. resolution is off.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the number of entries the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns the entries as `(provider spelling, ragent tool name)` pairs,
    /// ascending by spelling.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries
            .iter()
            .map(|(spelling, target)| (spelling.as_str(), target.as_str()))
    }

    /// Returns the outbound tool list for `definitions`: the registry's own
    /// definitions when `expose` is `false`, or those definitions plus one
    /// advertised entry per alias spelling when it is `true` (FR-015).
    ///
    /// The un-advertised case returns a copy, so the wire path keeps a single
    /// `Arc<Vec<ToolDefinition>>` type and a build that never set
    /// `tools.expose_alias_names` sends byte-identical names.
    ///
    /// An advertised alias is a **copy** of its target's definition carrying the
    /// spelling as its name, and is emitted only when all four hold:
    ///
    /// - the spelling is not itself registered - a registration always wins, so
    ///   an entry can never advertise a second entry of a real tool (FR-026);
    /// - its target **is** registered - an entry whose target this build does not
    ///   hold is refused at use (FR-031) and must not be advertised as callable;
    /// - the spelling is not already in the list - so no name is advertised
    ///   twice, whatever the caller's list contains.
    ///
    /// There is deliberately **no** FR-023 table read here. FR-031 forbids
    /// advertising a retired ragent spelling, and the table already refuses to
    /// hold one ([`AliasTable::from_aliases`], its single once-per-turn read), so
    /// a second lookup would buy nothing and would put a rename scan on the
    /// outbound path every tool call pays for (NFR-006). The outbound guard is
    /// therefore structural rather than duplicated: an entry that cannot exist
    /// cannot be advertised.
    ///
    /// The result is ascending by name, like every other registry enumeration
    /// (FR-030), so an advertised spelling sorts among the canonical names rather
    /// than being appended in table order.
    #[must_use]
    pub fn outbound_definitions(
        &self,
        definitions: &[ToolDefinition],
        expose: bool,
    ) -> Arc<Vec<ToolDefinition>> {
        if !expose || self.is_empty() {
            return self.borrow_definitions(definitions);
        }
        let mut merged: Vec<ToolDefinition> = definitions.to_vec();
        for (spelling, target) in self.entries() {
            if merged.iter().any(|def| def.name == spelling) {
                continue;
            }
            let Some(source) = merged.iter().find(|def| def.name == target).cloned() else {
                continue;
            };
            merged.push(ToolDefinition {
                name: spelling.to_string(),
                ..source
            });
        }
        if merged.len() == definitions.len() {
            // Nothing was advertised (every entry was skipped), so the caller
            // keeps the list it passed in rather than a copy of it.
            return self.borrow_definitions(definitions);
        }
        merged.sort_by(|left, right| left.name.cmp(&right.name));
        Arc::new(merged)
    }

    /// Returns `definitions` as the shared `Arc` the outbound path needs.
    ///
    /// A single allocation per call: the wire path takes an
    /// `Arc<Vec<ToolDefinition>>`, and the un-advertised case must not add a
    /// second copy on top of the one the registry cache already shares.
    fn borrow_definitions(&self, definitions: &[ToolDefinition]) -> Arc<Vec<ToolDefinition>> {
        Arc::new(definitions.to_vec())
    }

    /// Returns the ragent tool name `name` maps onto, or `None` when the table
    /// holds no entry for it.
    ///
    /// The lookup is an exact, case-sensitive string comparison: a spelling the
    /// table does not hold is never guessed at (D5), so a near miss still
    /// reaches the unknown-tool error with its did-you-mean hint.
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(spelling, _)| spelling == name)
            .map(|(_, target)| target.as_str())
    }
}

/// Resolves an inbound tool name through the alias table, before the
/// unknown-tool check (FR-015).
///
/// Returns the canonical ragent tool name when `name` is
///
/// - **not** itself registered - a real registration always wins, so a stray
///   table entry can never shadow the tool it names, and after the rename an
///   adopted spelling such as `Bash` resolves on the registry rather than on a
///   table read (NFR-006);
/// - **held** by the table, and
/// - mapped onto a name this build **does** register - an alias whose target is
///   unregistered is ignored here, so the call reaches the unknown-tool error
///   naming the spelling the client actually sent, with the did-you-mean hint
///   pointing at the real tool.
///
/// Returns `None` in every other case, which leaves the caller's dispatch
/// pipeline behaving exactly as it did before alias resolution existed.
#[must_use]
pub fn resolve_inbound_tool_name(
    registry: &ToolRegistry,
    table: &AliasTable,
    name: &str,
) -> Option<String> {
    if table.is_empty() || registry.get(name).is_some() {
        return None;
    }
    let target = table.resolve(name)?;
    if target == name || registry.get(target).is_none() {
        return None;
    }
    Some(target.to_string())
}
