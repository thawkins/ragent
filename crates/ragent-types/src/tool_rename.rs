//! The tool-rename table (spec `uniformtools` FR-023, FR-031).
//!
//! This module holds one checked-in table of `(before, after)` pairs: the
//! ragent tool spellings the build retires, and the Claude Code spellings that
//! replace them. Every migration site, guard and test reads this slice rather
//! than repeating the pairs, so the rename exists in exactly one place.
//!
//! It lives in `ragent-types` rather than in `ragent-agent` because **two**
//! crates must read it and they must not disagree: `ragent-agent` reads it on
//! the migration and guard paths ([`Rename::after_of`], [`Rename::before_of`],
//! [`Rename::is_retired`]), and `ragent-config` reads it when a configuration is
//! loaded, to refuse a `tools.aliases` entry that names a retired spelling
//! (FR-031, task T-042). `ragent-config` cannot depend on `ragent-agent` -
//! `ragent-agent` already depends on `ragent-config` - so the table sits in the
//! leaf crate both of them already depend on. `ragent_agent::tool::rename`
//! re-exports it, so every existing reader keeps its path.
//!
//! The set is derived from `tools.xlsx` by the FR-024 two-sheet exact-match
//! rule and the FR-025 exact-spelling tie-break; the derivation, the per-clause
//! accounting and the two refused targets (`WebFetch`, `WebSearch`) are
//! recorded in `specs/uniformtools/T-029-DERIVATION.md`. The invariant test
//! (`crates/ragent-agent/tests/test_tool_rename_table.rs`) never opens the
//! workbook - it compares the table against the live registry, and cross-
//! checking the table against the workbook is the manual step TC-019 of
//! `TESTPLAN.md` records. Reading `tools.xlsx` from a test would need an XLSX
//! parser, and adding one would breach NFR-002.
//!
//! The table is ascending by [`Rename::after`], so a reader can scan it and a
//! test can assert the order without sorting.
//!
//! This table is not a translation layer: the registry holds [`Rename::after`]
//! itself, and nothing consults the table on a dispatch path (NFR-006). `before`
//! spellings are retired, not aliased (FR-026).
//!
//! ## The FR-023 invariants name the registry as the table lands
//!
//! FR-023 orders the work as table-then-rename, and states the invariants
//! against the registry as it stands when the table lands: every `before` **is**
//! a registered name, and every `after` **is not**. The table landed in that
//! pre-rename state (task T-030), so the invariant test asserts exactly that.
//!
//! The rename (T-031) flips the two registry-facing assertions in the same
//! change that renames the tools, which is FR-034's atomic-landing rule: the
//! cases are renamed with them (`..._every_before_is_retired...` and
//! `..._every_after_is_registered...`) rather than left asserting a state the
//! tree no longer has. The table's own facts - the exact thirteen pairs,
//! ASCII-identifier shape, both columns unique, ascending by `after` - do not
//! change across that flip, and neither flipped assertion may be relaxed to
//! make the flip easy.

/// One row of the FR-023 rename table.
///
/// `before` is the spelling ragent registered in the pre-rename build; `after`
/// is the Claude Code spelling that replaces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rename {
    /// The retired ragent spelling, e.g. `bash`.
    pub before: &'static str,
    /// The adopted Claude Code spelling, e.g. `Bash`.
    pub after: &'static str,
}

/// The complete set of tool renames, ascending by [`Rename::after`] (FR-023).
///
/// Exactly thirteen pairs: the workbook's 15 exact-match candidate targets
/// less the two FR-025 refusals. Nine are a change of case only (`Bash`,
/// `Edit`, `Glob`, `Grep`, `Write` and the four `Task*` spellings) and four
/// change the words as well (`AskUserQuestion`, `EnterPlanMode`,
/// `ExitPlanMode`, `Skill`), so this table is data, not a capitalisation
/// transform.
pub const TOOL_RENAMES: [Rename; 13] = [
    Rename {
        before: "ask_user",
        after: "AskUserQuestion",
    },
    Rename {
        before: "bash",
        after: "Bash",
    },
    Rename {
        before: "edit",
        after: "Edit",
    },
    Rename {
        before: "plan_enter",
        after: "EnterPlanMode",
    },
    Rename {
        before: "plan_exit",
        after: "ExitPlanMode",
    },
    Rename {
        before: "glob",
        after: "Glob",
    },
    Rename {
        before: "grep",
        after: "Grep",
    },
    Rename {
        before: "skill_manage",
        after: "Skill",
    },
    Rename {
        before: "task_create",
        after: "TaskCreate",
    },
    Rename {
        before: "task_get",
        after: "TaskGet",
    },
    Rename {
        before: "task_list",
        after: "TaskList",
    },
    Rename {
        before: "task_update",
        after: "TaskUpdate",
    },
    Rename {
        before: "write",
        after: "Write",
    },
];

impl Rename {
    /// Returns the replacement for a retired spelling, or `None` when `name`
    /// was never renamed.
    ///
    /// Used by migration sites that read a stored or configured spelling (a
    /// hook matcher, a persisted tool-call record, a legacy config value) and
    /// need the name the registry now holds, and by the config loader's refusal
    /// of a `tools.aliases` entry that names a retired spelling (FR-031).
    #[must_use]
    pub fn after_of(name: &str) -> Option<&'static str> {
        TOOL_RENAMES
            .iter()
            .find(|pair| pair.before == name)
            .map(|pair| pair.after)
    }

    /// Returns the retired spelling that a current name replaced, or `None`
    /// when `name` is not an adopted spelling.
    ///
    /// Used by guards that must reject a retired spelling wherever one can be
    /// reintroduced (an alias source key, a config value, a bridged tool name).
    #[must_use]
    pub fn before_of(name: &str) -> Option<&'static str> {
        TOOL_RENAMES
            .iter()
            .find(|pair| pair.after == name)
            .map(|pair| pair.before)
    }

    /// Returns `true` when `name` is one of the retired spellings.
    #[must_use]
    pub fn is_retired(name: &str) -> bool {
        Self::after_of(name).is_some()
    }

    /// Returns `true` when `name` is either spelling of an FR-023 pair.
    ///
    /// A mechanism *called* an adopted spelling is as effective an alias as one
    /// called a retired spelling, so the config loader's self-mapping rule and
    /// the escape-hatch audit both key on both columns.
    #[must_use]
    pub fn is_rename_spelling(name: &str) -> bool {
        Self::after_of(name).is_some() || Self::before_of(name).is_some()
    }
}
