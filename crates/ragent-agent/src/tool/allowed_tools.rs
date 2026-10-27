//! Resolution and reporting of `allowed_tools` entries (spec `uniformtools`
//! FR-029).
//!
//! A skill's `SKILL.md` frontmatter (`allowed-tools`) restricts the tools a run
//! may call. Entries are resolved by comparing them, byte for byte, with the names
//! the [`ToolRegistry`] holds: [`super::build_allowed_tool_set`] stores each entry
//! verbatim and the per-step filter keeps a definition when that stored name
//! equals the definition's name. [`unresolved_allowed_tools`] is the same
//! comparison, made once when the restriction is applied.
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
//! derivation rated that spelling `Close`, and `read` kept its name.
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

/// The entries of `allowed` that name no registered tool (FR-029).
///
/// The comparison is a plain registry lookup: an entry naming a tool the registry
/// holds is resolved, and one that names nothing is returned. Order follows the
/// manifest, so a report reads like the list it came from.
///
/// Nothing is canonicalised on the way in or out - a retired `before` spelling is
/// unresolved, not mapped onto its `after` replacement, which is the point of
/// FR-026 and of adopting the Claude spellings in FR-023.
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
///     "Read".to_string(),     // never adopted: matches nothing
///     "bash".to_string(),     // retired spelling: matches nothing
/// ];
/// assert_eq!(unresolved_allowed_tools(&registry, &declared), vec!["Read", "bash"]);
/// ```
#[must_use]
pub fn unresolved_allowed_tools<'a>(
    registry: &ToolRegistry,
    allowed: &'a [String],
) -> Vec<&'a str> {
    allowed
        .iter()
        .filter(|entry| registry.get(entry).is_none())
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
