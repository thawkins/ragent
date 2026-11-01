//! Resolution and reporting of `allowed_tools` entries (spec `uniformtools`
//! FR-029).
//!
//! A skill's `SKILL.md` frontmatter (`allowed-tools`) restricts the tools a run
//! may call. Entries are resolved by comparing them, case-insensitively, with the
//! names the [`ToolRegistry`] holds: [`super::build_allowed_tool_set`] stores each
//! entry under its **registry name** and the per-step filter compares names with
//! [`eq_ignore_ascii_case`]. [`unresolved_allowed_tools`] is the same comparison,
//! made once when the restriction is applied.
//!
//! ## Two spellings resolve to one name
//!
//! `Bash`, `bash` and `BASH` all name the tool the registry spells `Bash`, so a
//! manifest may write an entry in whatever case reads best. The *lookup* is what
//! is forgiving: the entry is stored under the registry's own spelling, so two
//! spellings of one tool cannot become two keys in the allowed set, and the
//! restriction keeps containing exactly the names the registry holds.
//!
//! ## `any` - the one entry that names every tool
//!
//! The pseudo entry `any` (any case) lifts the restriction: the allowed set is
//! returned empty, which every consumer already reads as "no allowlist, hide
//! nothing". It is honoured without consulting the registry, because it names a
//! *policy* rather than a tool, so it never appears in the unresolved report and
//! does not depend on a tool of that name existing (or not).
//!
//! ## There is no translation layer, deliberately
//!
//! The FR-023 rename adopts the Claude Code spellings (`AskUserQuestion`, `Bash`,
//! `Edit`, `Glob`, `Grep`, `EnterPlanMode`, `ExitPlanMode`, `Skill`, `TaskCreate`,
//! `TaskGet`, `TaskList`, `TaskUpdate`, `Write`), so a Claude-written manifest
//! resolves on the registry lookup itself - the cheapest and most reliable match
//! there is. Nothing here consults the rename table: mapping an entry through it
//! would reintroduce the second vocabulary FR-026 forbids and would make a retired
//! spelling work from a manifest, which is exactly what the rename removed. A
//! `Read` entry is therefore reported rather than quietly answered by `read` - the
//! derivation rated that spelling `Close`, and `read` kept its name. Case folding
//! is *not* a mapping of that kind: it resolves one tool, it does not resolve a
//! different one.
//!
//! ## A typo is reported, not silently honoured
//!
//! An entry that matches no registered tool removes a capability without a word:
//! the model is never offered that tool and the run merely looks less capable.
//! [`report_unresolved_allowed_tools_once`] names the entry and the skill that
//! declared it, so the author can fix the manifest.
//!
//! "Once" is per *entry and declarer*, not per run: a skill may be invoked many
//! times in one session, and a naive `warn!` at the invocation site would repeat
//! the same sentence and train the reader to ignore it. The ledger is process-wide
//! and poison-tolerant, matching the other `OnceLock` registries in this crate (see
//! [`super::alias_table::report_unresolved_targets_once`] and
//! [`crate::hooks::report_retired_matcher_spellings_once`]), so a second run of the
//! same skill is silent while a *different* skill naming the same stale entry still
//! reports - that skill has its own manifest to fix.
//!
//! ## Why the report is made where a restriction is applied
//!
//! The three call sites are the places a skill's list is turned into a real
//! restriction: `Skill` (`read`/`load`), the TUI's `/skill` and bare
//! `/<name>` trigger, and the forked-skill resolver. Reporting on the
//! system-prompt path instead would warn about every discovered manifest on every
//! turn - including third-party plugin skills the session never runs - and would
//! add a per-turn scan for an entry that constrains nothing that turn (NFR-006).
//!
//! `AgentInfo::allowed_tools` is only ever populated from a skill's list
//! (`skill::invoke`), so an agent profile cannot be the declarer today; when a
//! loader starts reading such a key it should call
//! [`report_unresolved_allowed_tools_once`] with the profile's name, and the ledger
//! keeps the two reports distinct.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock, PoisonError};

use super::ToolRegistry;

/// The `(declarer, entry)` pairs already reported (FR-029).
///
/// Process-wide rather than per-session because the same manifest restricts the
/// same run in every session; per *entry and declarer* rather than per list so
/// that adding a second stale entry still reports.
fn unresolved_entries_seen() -> &'static Mutex<HashSet<(String, String)>> {
    static SEEN: OnceLock<Mutex<HashSet<(String, String)>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Clears the unresolved-entry ledger, so a test can observe the first report.
///
/// The ledger is process-wide (see [`report_unresolved_allowed_tools_once`]) and
/// this crate's test binary shares one process across every file, so a test that
/// asserts "the first call reports and the second is silent" has to start from a
/// known state. Mirrors [`super::alias_table::reset_unresolved_targets_for_test`].
#[doc(hidden)]
pub fn reset_unresolved_allowed_tools_for_test() {
    unresolved_entries_seen()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clear();
}

/// The pseudo `allowed-tools` entry that lifts the restriction entirely.
///
/// A manifest may write it in any case (`any`, `Any`, `ANY`); it names a policy
/// rather than a tool, so it never has to be a registered tool name.
pub const ANY_TOOL_ENTRY: &str = "any";

/// `true` when `entry` is the [`ANY_TOOL_ENTRY`] pseudo entry, in any case.
///
/// A skill declaring it restricts nothing - the same outcome as an omitted list.
///
/// # Examples
///
/// ```
/// use ragent_agent::tool::allowed_tools::is_any_tool_entry;
///
/// assert!(is_any_tool_entry("any"));
/// assert!(is_any_tool_entry("Any"));
/// assert!(!is_any_tool_entry("Anything"));
/// ```
#[must_use]
pub fn is_any_tool_entry(entry: &str) -> bool {
    entry.trim().eq_ignore_ascii_case(ANY_TOOL_ENTRY)
}

/// `true` when `entry` names `tool_name`, ignoring ASCII case.
///
/// Names are ASCII identifiers, so ASCII folding suffices and avoids a Unicode
/// case mapping (which could fold two distinct names together).
#[must_use]
pub fn entry_matches_tool(entry: &str, tool_name: &str) -> bool {
    entry.trim().eq_ignore_ascii_case(tool_name)
}

/// Resolve one `allowed_tools` entry to the registry's own spelling.
///
/// Returns the [`ANY_TOOL_ENTRY`] pseudo entry as [`ANY_TOOL_ENTRY`] (so a caller
/// can recognise it), the registry name when the entry names a registered tool in
/// any case, and `None` when it names nothing.
///
/// The name is owned because [`ToolRegistry::list`] yields owned names. The scan is
/// linear, but it runs once per entry at the moment a restriction is applied, never
/// per turn (NFR-006).
#[must_use]
pub fn resolve_allowed_tool_entry(registry: &ToolRegistry, entry: &str) -> Option<String> {
    if is_any_tool_entry(entry) {
        return Some(ANY_TOOL_ENTRY.to_string());
    }
    registry
        .list()
        .into_iter()
        .find(|name| entry_matches_tool(entry, name))
}

/// The entries of `allowed` that name no registered tool (FR-029).
///
/// The comparison is a case-insensitive registry lookup: an entry naming a tool
/// the registry holds is resolved - under any spelling case - and one that names
/// nothing is returned. The [`ANY_TOOL_ENTRY`] pseudo entry always resolves, so it
/// is never reported. Order follows the manifest, so a report reads like the list
/// it came from.
///
/// Nothing is canonicalised onto a *different* tool on the way in or out - a
/// retired `before` spelling is unresolved, not translated onto its `after`
/// replacement, which is the point of FR-026 and of adopting the Claude spellings
/// in FR-023.
///
/// # Examples
///
/// ```
/// use ragent_agent::tool::allowed_tools::unresolved_allowed_tools;
/// use ragent_agent::tool::create_default_registry;
///
/// let registry = create_default_registry();
/// let declared = vec![
///     "Bash".to_string(),     // adopted Claude spelling: resolves
///     "read".to_string(),     // ragent spelling, unchanged by the rename: resolves
///     "grep".to_string(),     // lower case of the registry's `Grep`: resolves
///     "Read".to_string(),     // case folding of the registered `read`: resolves
///     "any".to_string(),      // pseudo entry: always resolves
///     "ReadFile".to_string(), // typo: matches nothing
///     "Bashh".to_string(),    // typo: matches nothing
/// ];
/// assert_eq!(
///     unresolved_allowed_tools(&registry, &declared),
///     vec!["ReadFile", "Bashh"]
/// );
/// ```
#[must_use]
pub fn unresolved_allowed_tools<'a>(
    registry: &ToolRegistry,
    allowed: &'a [String],
) -> Vec<&'a str> {
    allowed
        .iter()
        .filter(|entry| resolve_allowed_tool_entry(registry, entry).is_none())
        .map(String::as_str)
        .collect()
}

/// The sentence a report emits for one unresolved entry (FR-029).
///
/// Split out from [`report_unresolved_allowed_tools_once`] so the wording - which
/// must name both the entry and the declarer - is assertable without capturing a
/// tracing subscriber: the ledger already makes "once" testable through the
/// returned count, and this makes "names the entry and the skill" testable through
/// the text.
#[must_use]
pub fn unresolved_entry_report(declarer: &str, entry: &str) -> String {
    format!(
        "allowed_tools entry `{entry}` declared by {declarer} names no tool this build \
         registers; it matches nothing, so the declaring manifest silently loses that \
         capability - spell the entry exactly as the registry spells the tool (FR-029)"
    )
}

/// Logs each unresolved `allowed_tools` entry once per declarer and process.
///
/// `declarer` names the manifest the list came from, e.g. ``skill `rename-check` ``,
/// so the report points at the file to fix rather than at the run that suffered.
///
/// Returns the number of entries reported by this call, so a caller (and a test)
/// can tell "first report" from "already reported". An empty list, or a list whose
/// every entry resolves, reports nothing and returns zero - a correct manifest is
/// silent, which is the property that makes the report worth reading.
pub fn report_unresolved_allowed_tools_once(
    registry: &ToolRegistry,
    declarer: &str,
    allowed: &[String],
) -> usize {
    let unresolved = unresolved_allowed_tools(registry, allowed);
    if unresolved.is_empty() {
        return 0;
    }
    let seen = unresolved_entries_seen();
    let mut seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
    let mut reported = 0usize;
    for entry in unresolved {
        if !seen.insert((declarer.to_string(), entry.to_string())) {
            continue;
        }
        reported += 1;
        tracing::warn!(
            entry = %entry,
            declared_by = %declarer,
            "{}",
            unresolved_entry_report(declarer, entry)
        );
    }
    reported
}

/// [`report_unresolved_allowed_tools_once`] for a skill, naming it as the
/// declarer (FR-029).
///
/// Takes the skill's name and list rather than a [`SkillInfo`] so all three call
/// sites - `Skill`, the TUI `/skill` trigger and the forked-skill resolver - reach
/// it with the same arguments; the forked path only carries a `SkillInvocation`,
/// which has the name and the list but is not a `SkillInfo`. Keeping the declarer
/// text here means the ledger key cannot disagree between the three.
///
/// [`SkillInfo`]: crate::skill::SkillInfo
pub fn report_skill_allowed_tools_once(
    registry: &ToolRegistry,
    skill_name: &str,
    allowed: &[String],
) -> usize {
    if allowed.is_empty() {
        return 0;
    }
    report_unresolved_allowed_tools_once(registry, &format!("skill `{skill_name}`"), allowed)
}
