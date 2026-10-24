//! The rule that keeps a dynamically sourced tool out of the bare ragent
//! namespace, and refuses a bridge whose surfaced name is a retired spelling
//! (spec `uniformtools` FR-032).
//!
//! ## Why a dynamic source needs its own rule
//!
//! A built-in tool's name is written by hand in this workspace, so the FR-023
//! rename (T-031) reached every one of them and the compiler carried the change.
//! A dynamic tool's name is written by *someone else*: an MCP server's tool list, a
//! connector's servers (which reach the registry as the bridged
//! `<connector-id>.<server-id>`), or a plugin's `ragent.register_tool({ name })`.
//! Nothing in this build can rename those, so the bridge is the only place the name
//! may enter and the only place the rule can be applied.
//!
//! FR-032 has two clauses, and both are answered here:
//!
//! 1. **A dynamic tool is namespaced** - `mcp_<server>_<tool>` for an MCP or
//!    connector server (with `.`, `-` and `/` folded to `_`), `plugin_<id>_<tool>`
//!    for a plugin tool. A namespaced name cannot occupy a bare ragent name, which
//!    is the property;
//!    [`check_dynamic_bridge`] refuses a surfaced name outside its source's
//!    prefix so the property cannot be lost by an edit at one call site.
//! 2. **A name that reaches the bare namespace anyway is refused** - with an
//!    error naming the source, the spelling and its `after` replacement, keeping
//!    every other tool from that source working, reported once rather than once
//!    per turn, and never taking the session down.
//!
//! The refusal is per *tool*, not per source: losing one tool to a stale name is a
//! smaller harm than losing a server the user installed, and losing the session to
//! it is smaller than both (`specs/uniformtools/PLAN.md`, "A refused dynamic bridge
//! must not take the session down").
//!
//! ## The rule is about the *surfaced* name, not the advertised one
//!
//! Clause 2 refuses a surfaced name equal to a `before` spelling. It deliberately
//! does **not** refuse a properly namespaced tool whose server-side name happens to
//! be `bash`: `mcp_shell_bash` is not `bash`, calling it cannot make `bash`
//! resolvable, and a shell server is precisely the kind of source that would offer
//! one. Refusing it would break a legitimate integration to satisfy a sentence that
//! was written about bare names.
//!
//! The retired-spelling check still earns its place as the *backstop* the
//! requirement calls it: if a bridge ever passes the advertised name through
//! unchanged, the surfaced name equals the advertised one, and this check fires
//! with the useful sentence ("`bash` was retired; spell it `Bash`") instead of the
//! generic namespacing one.
//!
//! ## Why this lives in `ragent-types`
//!
//! Three call sites in two crates ask the same question - `ragent-agent`'s MCP
//! registration (and the TUI's second one) and `ragent-plugins`' tool bridge - and
//! `ragent-plugins` cannot depend on `ragent-agent` (the dependency runs the other
//! way). `ragent-types` already owns the FR-023 table ([`crate::tool_rename`]) the
//! rule is written against, so it sits in the leaf crate both of them depend on.
//! One implementation keeps the call sites from answering the same question three
//! slightly different ways.
//!
//! A skill contributes no tool in this build - a `SKILL.md` `allowed-tools` list
//! *restricts* the registry rather than adding to it (FR-029) - so no
//! `skill_<name>_` prefix exists here. The first bridge that does contribute a tool
//! from a skill calls [`check_dynamic_bridge`] with its own prefix, which is all
//! that bridge needs; declaring a constant nothing reads would be a claim the tree
//! cannot back.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock, PoisonError};

use crate::tool_rename::TOOL_RENAMES;

/// The registry-name prefix every MCP-bridged tool carries (`mcp_<server>_`).
pub const MCP_TOOL_PREFIX: &str = "mcp_";

/// The registry-name prefix every plugin-contributed tool carries
/// (`plugin_<id>_`).
pub const PLUGIN_TOOL_PREFIX: &str = "plugin_";

/// Characters folded to `_` in a registry-name segment.
///
/// A registry name is compared byte for byte against a model's
/// `tool_calls[].name`, an `allowed_tools` entry and a hook matcher, so each
/// segment is kept to one identifier-shaped word. `-`, `.` and `/` are the three
/// that occur in real MCP server and tool names (`mongodb.mongodb`,
/// `mcp-server-fetch`, `@scope/pkg`, `do-it`).
const SEGMENT_FOLDS: [char; 3] = ['-', '.', '/'];

/// Folds `segment` into the one word a registry-name segment may be.
fn fold_segment(segment: &str) -> String {
    segment.replace(SEGMENT_FOLDS, "_")
}

/// The registry-name prefix an MCP server's tools carry (`mcp_<server>_`).
///
/// The single implementation of the MCP mangling. `ragent-agent`'s
/// `McpToolWrapper::ragent_name_for`, the TUI's stale-server prefix scan and the
/// FR-032 check all derive it from here, so a tool cannot be registered under one
/// name and looked for under another.
///
/// # Examples
///
/// ```
/// use ragent_types::tool_bridge::mcp_tool_prefix;
///
/// assert_eq!(mcp_tool_prefix("srv"), "mcp_srv_");
/// assert_eq!(mcp_tool_prefix("mongodb.mongodb"), "mcp_mongodb_mongodb_");
/// ```
#[must_use]
pub fn mcp_tool_prefix(server_id: &str) -> String {
    format!("{MCP_TOOL_PREFIX}{}_", fold_segment(server_id))
}

/// The registry name an MCP server's tool is bridged under
/// (`mcp_<server>_<tool>`), folding `-`, `.` and `/` in both segments.
///
/// # Examples
///
/// ```
/// use ragent_types::tool_bridge::mcp_tool_name;
///
/// assert_eq!(mcp_tool_name("srv", "notes_read"), "mcp_srv_notes_read");
/// assert_eq!(mcp_tool_name("my-server", "do-it"), "mcp_my_server_do_it");
/// ```
#[must_use]
pub fn mcp_tool_name(server_id: &str, tool_name: &str) -> String {
    format!("{}{}", mcp_tool_prefix(server_id), fold_segment(tool_name))
}

/// The registry-name prefix a plugin's tools carry (`plugin_<id>_`).
///
/// Deliberately does **not** fold the plugin id: the id is the key its store entry
/// and its `plugin:<id>` permission category use, and a registry name that
/// disagreed with the id would leave the two unjoinable.
///
/// # Examples
///
/// ```
/// use ragent_types::tool_bridge::plugin_tool_prefix;
///
/// assert_eq!(plugin_tool_prefix("codex-weather"), "plugin_codex-weather_");
/// ```
#[must_use]
pub fn plugin_tool_prefix(plugin_id: &str) -> String {
    format!("{PLUGIN_TOOL_PREFIX}{plugin_id}_")
}

/// The registry name a plugin's tool is bridged under
/// (`plugin_<plugin-id>_<tool>`).
///
/// # Examples
///
/// ```
/// use ragent_types::tool_bridge::plugin_tool_name;
///
/// assert_eq!(
///     plugin_tool_name("codex-weather", "get_weather"),
///     "plugin_codex-weather_get_weather"
/// );
/// ```
#[must_use]
pub fn plugin_tool_name(plugin_id: &str, tool_name: &str) -> String {
    format!("{}{tool_name}", plugin_tool_prefix(plugin_id))
}

/// How an MCP server is named in a refusal or a report, e.g. ``MCP server `srv` ``.
///
/// One wording per source kind, so a refusal, a startup log line and a `/tools
/// list` row cannot describe the same source three ways.
///
/// # Examples
///
/// ```
/// use ragent_types::tool_bridge::mcp_source_label;
///
/// assert_eq!(mcp_source_label("srv"), "MCP server `srv`");
/// ```
#[must_use]
pub fn mcp_source_label(server_id: &str) -> String {
    format!("MCP server `{server_id}`")
}

/// How a plugin is named in a refusal or a report, e.g. ``plugin `codex-weather` ``.
///
/// # Examples
///
/// ```
/// use ragent_types::tool_bridge::plugin_source_label;
///
/// assert_eq!(plugin_source_label("codex-weather"), "plugin `codex-weather`");
/// ```
#[must_use]
pub fn plugin_source_label(plugin_id: &str) -> String {
    format!("plugin `{plugin_id}`")
}

/// Why a dynamic bridge was refused (FR-032).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeRefusalReason {
    /// The surfaced name equals a retired `before` spelling, so bridging it would
    /// make a name the rename removed resolvable again (FR-026, FR-031).
    RetiredSpelling {
        /// The retired spelling the surfaced name equals, e.g. `bash`.
        spelling: &'static str,
        /// The name the registry holds for that tool instead, e.g. `Bash`.
        after: &'static str,
    },
    /// The surfaced name is not under the source's namespace prefix, so it could
    /// occupy a bare ragent name (FR-032 clause 1).
    NotNamespaced {
        /// The prefix the source's tools must carry, e.g. `mcp_srv_`.
        prefix: String,
    },
}

impl BridgeRefusalReason {
    /// The suffix of the refusal sentence that explains *this* reason.
    ///
    /// Kept beside [`BridgeRefusal::sentence`] so a caller that has only the reason
    /// (a log field, a status row) still has one wording to print.
    #[must_use]
    pub fn explanation(&self) -> String {
        match self {
            Self::RetiredSpelling { spelling, after } => format!(
                "which this build retired; the bridge is refused and `{spelling}` stays \
                 unresolvable - spell the tool `{after}` in the source"
            ),
            Self::NotNamespaced { prefix } => format!(
                "which is not namespaced under `{prefix}`; the bridge is refused, because a \
                 dynamically sourced tool must not occupy a bare ragent name"
            ),
        }
    }
}

/// One refused bridge: which source offered what, and why (FR-032).
///
/// Carries the *source* as a label rather than as a type because every front end
/// words it for its reader (``MCP server `srv` ``, ``plugin `codex-weather` ``,
/// ``connector `postgres` ``); a refusal a reader cannot attribute to a file to
/// edit is a refusal they will not fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeRefusal {
    /// The source, phrased for a sentence, e.g. ``MCP server `srv` ``.
    pub source: String,
    /// The registry name the bridge would have registered, e.g. `mcp_srv_read`.
    pub surfaced: String,
    /// Why it was refused.
    pub reason: BridgeRefusalReason,
}

impl BridgeRefusal {
    /// The one sentence a report emits for this refusal (FR-032).
    ///
    /// Split out from [`report_bridge_refusals_once`] so the wording - which must
    /// name the source, the spelling and the `after` replacement - is assertable
    /// without capturing a tracing subscriber, exactly as
    /// `allowed_tools::unresolved_entry_report` does for FR-029.
    ///
    /// The closing clause is fixed: "every other tool from that source keeps
    /// working" is the half of FR-032 a reader most needs told, because it is the
    /// half they are least likely to believe from a warning.
    #[must_use]
    pub fn sentence(&self) -> String {
        format!(
            "{} offered the registry name `{}`, {}; every other tool from that source keeps \
             working (FR-032)",
            self.source,
            self.surfaced,
            self.reason.explanation()
        )
    }

    /// The ledger key: which source offered which name.
    ///
    /// Per *source and name*, not per source: a second server offering the same
    /// retired spelling is a different file to fix and must still report.
    fn ledger_key(&self) -> (String, String) {
        (self.source.clone(), self.surfaced.clone())
    }
}

/// Decides whether `source` may bridge one tool under `surfaced` (FR-032).
///
/// `source` is the label a report prints (``MCP server `srv` ``); `prefix` is the
/// namespace prefix the source's tool names must carry (`mcp_srv_`); `surfaced` is
/// the registry name the bridge would register (`mcp_srv_read`).
///
/// Returns `Err` naming the reason - a retired `before` spelling, or a name outside
/// `prefix` - and `Ok(())` when the bridge may proceed. The retired-spelling check
/// runs first: when a surfaced name is both bare and retired, the sentence that
/// names the replacement is the more useful of the two.
///
/// # Examples
///
/// ```
/// use ragent_types::tool_bridge::{BridgeRefusalReason, check_dynamic_bridge};
///
/// // A namespaced tool with an ordinary name bridges.
/// assert!(check_dynamic_bridge("MCP server `srv`", "mcp_srv_", "mcp_srv_notes_read").is_ok());
///
/// // A server-local `bash` tool bridges under the namespace: `mcp_srv_bash` is
/// // not the bare `bash` the rename retired, so it cannot resurrect it.
/// assert!(check_dynamic_bridge("MCP server `srv`", "mcp_srv_", "mcp_srv_bash").is_ok());
///
/// // A bridge that passes the advertised name through unchanged is refused, and
/// // the refusal names the replacement.
/// let refusal = check_dynamic_bridge("plugin `p`", "plugin_p_", "bash")
///     .expect_err("a bare `bash` would make the retired spelling resolvable");
/// assert_eq!(
///     refusal.reason,
///     BridgeRefusalReason::RetiredSpelling { spelling: "bash", after: "Bash" }
/// );
/// assert!(refusal.sentence().contains('`'));
///
/// // Any other bare name is refused for not being namespaced.
/// assert!(matches!(
///     check_dynamic_bridge("plugin `p`", "plugin_p_", "Bash").unwrap_err().reason,
///     BridgeRefusalReason::NotNamespaced { .. }
/// ));
/// ```
pub fn check_dynamic_bridge(
    source: &str,
    prefix: &str,
    surfaced: &str,
) -> Result<(), BridgeRefusal> {
    let refusal = |reason| BridgeRefusal {
        source: source.to_string(),
        surfaced: surfaced.to_string(),
        reason,
    };
    // The pair itself, so the refusal carries the table's own `&'static str`
    // values rather than re-borrowed copies of the caller's.
    if let Some(pair) = TOOL_RENAMES.iter().find(|pair| pair.before == surfaced) {
        return Err(refusal(BridgeRefusalReason::RetiredSpelling {
            spelling: pair.before,
            after: pair.after,
        }));
    }
    if !surfaced.starts_with(prefix) {
        return Err(refusal(BridgeRefusalReason::NotNamespaced {
            prefix: prefix.to_string(),
        }));
    }
    Ok(())
}

/// The `(source, surfaced)` pairs already reported (FR-032).
///
/// Process-wide rather than per-session because the same server offers the same
/// stale name to every session that connects it; per *source and name* rather than
/// per source so a second offender still reports. Mirrors the other ledgers in this
/// workspace (`alias_table`'s unresolved targets, `allowed_tools`' stale entries,
/// the hook matcher report).
fn refused_bridges_seen() -> &'static Mutex<HashSet<(String, String)>> {
    static SEEN: OnceLock<Mutex<HashSet<(String, String)>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Clears the refusal ledger, so a test can observe the first report.
///
/// The ledger is process-wide (see [`report_bridge_refusals_once`]) and this
/// workspace's test binary shares one process across every file, so a test that
/// asserts "the first call reports and the second is silent" has to start from a
/// known state.
#[doc(hidden)]
pub fn reset_bridge_refusals_for_test() {
    refused_bridges_seen()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}

/// Logs each refused bridge once per source and process, returning how many this
/// call reported (FR-032).
///
/// "Once" is per refused *bridge*, not per turn: the bridge runs again on every
/// reconnect and every session start, so a naive `warn!` at the registration site
/// would repeat the same sentence and train the reader to ignore it. A refused
/// bridge therefore reports the first time its source offers it and is silent
/// afterwards, while a *different* source offering the same spelling still
/// reports - that source is a different file to fix.
///
/// Never fails and never panics: a refused bridge is a contained loss, so the
/// caller keeps the rest of the source working and the session usable.
pub fn report_bridge_refusals_once(refusals: &[BridgeRefusal]) -> usize {
    if refusals.is_empty() {
        return 0;
    }
    let seen = refused_bridges_seen();
    let mut seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
    let mut reported = 0usize;
    for refusal in refusals {
        if !seen.insert(refusal.ledger_key()) {
            continue;
        }
        reported += 1;
        tracing::warn!(
            source = %refusal.source,
            surfaced = %refusal.surfaced,
            "{}",
            refusal.sentence()
        );
    }
    reported
}
