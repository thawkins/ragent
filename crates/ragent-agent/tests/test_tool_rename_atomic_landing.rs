//! Guard: the FR-023 rename lands as one atomic step (spec `uniformtools`
//! FR-034).
//!
//! FR-034 is a *state-driven* requirement: every commit the change lands in
//! must leave the tree with **exactly one** spelling of each FR-023 pair
//! resolving - never both, never neither - because a bisect or a `git stash`
//! that lands on a half-renamed tree reports a bug that does not exist at either
//! end of the change. The requirement names three things such a tree must
//! satisfy: one spelling resolves, the build and the test suites pass, and
//! `--dry-run` reports the FR-030 count.
//!
//! This file is the mechanical part of that requirement. It asserts the
//! **registry state** the landing produces, across every enumeration the build
//! offers, so a tree that registered `Bash` for the model but left `bash` in a
//! key map - or the reverse - fails here rather than after a bisect. It checks:
//!
//! | # | Case | Clause it pins |
//! |---|---|---|
//! | 1 | `test_the_rename_resolves_exactly_one_spelling_of_every_pair` | each pair resolves one spelling, and it is the `after` one |
//! | 2 | `test_no_before_spelling_is_reachable_by_lookup_or_by_enumeration` | no `before` is reachable by `get` or by a key |
//! | 3 | `test_registry_keys_tool_names_and_definition_names_agree` | the three enumerations agree element for element |
//! | 4 | `test_every_registration_is_stored_under_the_name_its_tool_reports` | every key stores a tool reporting that name - the half-renamed detector |
//! | 5 | `test_the_renamed_tools_keep_the_permission_family_they_had_before_the_rename` | FR-028's family counts, including the two canaries |
//! | 6 | `test_the_fr030_surface_total_is_the_baseline_at_the_rename` | the FR-030 total and the `definitions()` length |
//! | 7 | `test_the_registry_holds_no_duplicate_registration` | no enumeration repeats a name |
//!
//! ## What this file deliberately does not check
//!
//! - **The git history.** FR-034's "the same commit registers `after` and
//!   removes `before`" is a property of a commit graph, not of a tree. It is
//!   verified by review (TC-030 step 9 of `TESTPLAN.md`) and recorded in
//!   `specs/uniformtools/T-046-ATOMIC-LANDING.md`; a test that shelled out to
//!   `git log` would fail in a source tarball and would duplicate the review.
//! - **The configured alias table's rejected entries.** A `tools.aliases` entry
//!   whose key or target is a retired spelling is refused when the configuration
//!   loads (FR-031), which T-042 owns: `crates/ragent-config/tests/test_retired_alias_spelling.rs`
//!   asserts the load-time refusal and `test_tool_alias_table.rs` asserts the
//!   resolver half. A case here would be a third guard to keep in step for a
//!   claim FR-034 does not make.
//! - **The `before` literals in non-test source.** The FR-027 source scan owns
//!   that, and `test_escape_hatch_audit.rs` owns the escape-hatch surfaces.
//!   Repeating either here would mean three guards to keep in step.
//! - **The FR-023 table's own shape.** `test_tool_rename_table.rs` owns the
//!   table: its 13 pairs, the ordering and the two column-uniqueness rules.
//!   This file reads the table and asserts the registry agrees with it.

use std::collections::{BTreeMap, HashSet};

use ragent_agent::tool::ToolRegistry;
use ragent_agent::tool::create_default_registry;
use ragent_agent::tool::rename::TOOL_RENAMES;

/// The FR-030 surface size recorded before the rename in
/// `specs/uniformtools/T-041-BASELINE.md`.
///
/// FR-034 requires the tree that lands the rename to report this count. It is
/// pinned as a number rather than derived, because the point of the assertion is
/// that the rename changed names and nothing else: a derived expectation would
/// agree with whatever the tree happened to do.
///
/// **If you legitimately add or remove a tool**, update this constant and the
/// baseline record together. **If you did not**, a failure here means the rename
/// landed half-way - a tool was renamed out of the registry without being
/// renamed back in, or an adapter registered a second copy.
const FR030_TOTAL: usize = 148;

/// The two family counts T-041 records as the canary against a stale binary.
///
/// Both binaries in `target/` report 148 tools, but the stale release build
/// splits the surface `agent` 4 / `team` 19 while the tree that lands the rename
/// reports `agent` 3 / `team` 20. A comparison that checks only the total would
/// pass against the wrong binary; these two numbers are what catches it.
const FR030_CANARY_FAMILIES: [(&str, usize); 2] = [("agent", 3), ("team", 20)];

/// The family count T-041 records beside the total (21 families summing to 148).
const FR030_FAMILY_COUNT: usize = 21;

/// The three enumerations of one registry, as sorted name lists.
///
/// `ToolRegistry` offers three ways to ask "what is registered": the key set
/// (`list`), the tool objects (`all_tools`) and the model-facing descriptors
/// (`definitions`). FR-034's "exactly one spelling" has to hold for all three,
/// because different surfaces read different ones - `tool_info` reads
/// `all_tools`, the outbound provider list reads `definitions`, and the TUI's
/// `/tools` reads `definitions` too while `/bug-report` reads the length.
struct Enumerations {
    /// Registry keys, ascending.
    keys: Vec<String>,
    /// Names reported by the registered tool objects, ascending.
    tool_names: Vec<String>,
    /// Names carried by the model-facing definitions, ascending.
    definition_names: Vec<String>,
}

impl Enumerations {
    /// Collects the three enumerations from one registry.
    fn of(registry: &ToolRegistry) -> Self {
        let keys = registry.list();
        let mut tool_names: Vec<String> = registry
            .all_tools()
            .iter()
            .map(|tool| tool.name().to_string())
            .collect();
        tool_names.sort();
        let mut definition_names: Vec<String> = registry
            .definitions()
            .iter()
            .map(|definition| definition.name.clone())
            .collect();
        definition_names.sort();
        Self {
            keys,
            tool_names,
            definition_names,
        }
    }

    /// The three enumerations with a label, for a failure message that names the
    /// surface the reader has to fix.
    fn labelled(&self) -> [(&'static str, &[String]); 3] {
        [
            ("ToolRegistry::list() keys", &self.keys),
            ("ToolRegistry::all_tools() names", &self.tool_names),
            ("ToolRegistry::definitions() names", &self.definition_names),
        ]
    }
}

/// Returns how many times `name` appears in `names`.
fn occurrences(names: &[String], name: &str) -> usize {
    names.iter().filter(|candidate| *candidate == name).count()
}

#[test]
fn test_the_rename_resolves_exactly_one_spelling_of_every_pair() {
    let registry = create_default_registry();
    let enumerations = Enumerations::of(&registry);

    for (label, names) in enumerations.labelled() {
        for pair in &TOOL_RENAMES {
            let before = occurrences(names, pair.before);
            let after = occurrences(names, pair.after);
            assert_eq!(
                (before, after),
                (0, 1),
                "FR-034: `{label}` must resolve exactly one spelling of the pair \
                 {:?} -> {:?}, but it holds {before} {:?} and {after} {:?}. \
                 Both present is a half-renamed tree that still resolves the \
                 retired spelling; neither present is a tree the rename removed \
                 {:?} from without adding {:?} back.",
                pair.before,
                pair.after,
                pair.before,
                pair.after,
                pair.before,
                pair.after,
            );
        }
    }
}

#[test]
fn test_no_before_spelling_is_reachable_by_lookup_or_by_enumeration() {
    let registry = create_default_registry();
    let enumerations = Enumerations::of(&registry);

    for pair in &TOOL_RENAMES {
        assert!(
            registry.get(pair.before).is_none(),
            "FR-034: `registry.get({:?})` still returns a tool. A `before` \
             spelling that resolves is the second spelling FR-026 forbids and \
             FR-034 forbids landing.",
            pair.before
        );
        // The lookup and the enumerations are separate code paths (`get` reads
        // the map, `list` reads its keys, `all_tools`/`definitions` read the
        // values), so a tree can disagree with itself; check all of them.
        assert!(
            !enumerations.keys.iter().any(|key| key == pair.before),
            "FR-034: the retired spelling {:?} is still a registry key",
            pair.before
        );
    }
}

#[test]
fn test_registry_keys_tool_names_and_definition_names_agree() {
    let registry = create_default_registry();
    let enumerations = Enumerations::of(&registry);

    assert_eq!(
        enumerations.keys.len(),
        enumerations.tool_names.len(),
        "FR-034: the registry holds {} keys but {} tool objects",
        enumerations.keys.len(),
        enumerations.tool_names.len()
    );
    assert_eq!(
        enumerations.keys, enumerations.definition_names,
        "FR-034: the registry's key set and the definitions offered to the model \
         differ, so one surface advertises a spelling the other cannot dispatch"
    );
    assert_eq!(
        enumerations.keys, enumerations.tool_names,
        "FR-034: the registry's key set and the names its tools report differ, so \
         a lookup and a listing disagree about what is registered"
    );
}

#[test]
fn test_every_registration_is_stored_under_the_name_its_tool_reports() {
    // The half-renamed detector. A tree in which the key map says `Bash` but
    // the stored tool still reports `bash` compiles, passes a key-only check,
    // and resolves the retired spelling at every surface that reads
    // `Tool::name()` - `definitions()`, `all_tools()`, the permission layer's
    // special cases and the repeat guard's key.
    let registry = create_default_registry();

    for pair in &TOOL_RENAMES {
        let tool = registry.get(pair.after).unwrap_or_else(|| {
            panic!(
                "FR-034: `after` {:?} is not registered, so the rename did not land",
                pair.after
            )
        });
        assert_eq!(
            tool.name(),
            pair.after,
            "FR-034: the tool registered under {:?} reports the name {:?}",
            pair.after,
            tool.name()
        );
    }

    for tool in registry.all_tools() {
        assert!(
            !TOOL_RENAMES.iter().any(|pair| pair.before == tool.name()),
            "FR-034: a registered tool still reports the retired spelling {:?}. \
             The registry key and `Tool::name()` both have to move for the rename \
             to be atomic.",
            tool.name()
        );
    }
}

#[test]
fn test_the_renamed_tools_keep_the_permission_family_they_had_before_the_rename() {
    // FR-028 holds `permission_category()` unchanged across the rename, so the
    // per-family counts are the sharpest evidence that a tool was renamed rather
    // than replaced: a rename keeps a family's count, a registration swap does
    // not. The two canary families are the ones T-041 measured against the stale
    // release binary.
    let registry = create_default_registry();
    let mut family_counts: BTreeMap<String, usize> = BTreeMap::new();
    for tool in registry.all_tools() {
        let category = tool.permission_category();
        let family = category.split(':').next().unwrap_or(category).to_string();
        *family_counts.entry(family).or_insert(0) += 1;
    }

    assert_eq!(
        family_counts.len(),
        FR030_FAMILY_COUNT,
        "FR-030/FR-034: {} families, not the {} the baseline records: {:?}",
        family_counts.len(),
        FR030_FAMILY_COUNT,
        family_counts
    );
    for (family, expected) in FR030_CANARY_FAMILIES {
        assert_eq!(
            family_counts.get(family).copied(),
            Some(expected),
            "FR-030/FR-034: the `{family}` family holds {:?} tools, not the {expected} \
             the baseline records - this is the canary T-041 uses to tell this \
             tree's build from the stale release binary beside it",
            family_counts.get(family)
        );
    }
}

#[test]
fn test_the_fr030_surface_total_is_the_baseline_at_the_rename() {
    let registry = create_default_registry();

    // Every surface that reports a count reads one of these two, so asserting
    // both pins the number FR-030 acceptance #9 compares.
    assert_eq!(
        registry.list().len(),
        FR030_TOTAL,
        "FR-030/FR-034: the registry holds {} names, not the {FR030_TOTAL} the \
         baseline records. If you did not add or remove a tool, the rename landed \
         half-way: a renamed-out tool was not renamed back in.",
        registry.list().len()
    );
    assert_eq!(
        registry.definitions().len(),
        FR030_TOTAL,
        "FR-030/FR-034: `definitions()` reports {} tools, not the {FR030_TOTAL} \
         the baseline records",
        registry.definitions().len()
    );
}

#[test]
fn test_the_registry_holds_no_duplicate_registration() {
    // A duplicate can only arrive by registering the same name twice, which
    // `register` collapses - but a duplicate *definition* would mean two tools
    // report one name while the map holds one key. This case is the cheap proof
    // that the map key is not hiding a second registration (NFR-008 predicate
    // (f)'s tree-state half).
    let registry = create_default_registry();
    let enumerations = Enumerations::of(&registry);

    for (label, names) in enumerations.labelled() {
        let unique: HashSet<&String> = names.iter().collect();
        assert_eq!(
            unique.len(),
            names.len(),
            "FR-034: `{label}` repeats a name, so one spelling is registered twice"
        );
    }
}
