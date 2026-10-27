//! The one did-you-mean candidate pool for an unregistered tool name (spec
//! `uniformtools` FR-021, FR-033).
//!
//! Every surface that answers "you typed a tool name that does not exist, did
//! you mean ...?" reads this module: the unknown-tool refusal the agent loop
//! builds ([`crate::session::processor`]), the refusal the ordered dispatch
//! pipeline builds (FR-012), and the guard in
//! `crates/ragent-agent/tests/test_fr033_completion_sources.rs`.
//!
//! ## Why the pool is the registry and nothing else
//!
//! FR-033 states the rule for every completion and suggestion surface: the
//! *registry* is the catalogue, and a retired `before` spelling (FR-023) appears
//! in none of them. A pool that drew its candidates from the rename table - the
//! natural place to look, since the table holds every name a model may still be
//! using - would answer a retired spelling with that same retired spelling, and
//! the hint would teach the dead name back. [`suggest_tool_name`] therefore
//! iterates [`ToolRegistry::list`](crate::tool::ToolRegistry::list) and never
//! consults [`crate::tool::rename::TOOL_RENAMES`].
//!
//! Before this module the ranking lived twice, once as
//! `processor::unknown_tool_error` and once as `session::dispatch::suggest_tool_name`,
//! each with its own copy of the ranking and of the edit-distance helper. Two
//! copies of a ranking are two rankings as soon as one is tuned.

/// The closest registered tool name to `name`, or `None` when nothing is within
/// the similarity threshold (FR-021's did-you-mean hint).
///
/// Similarity is deliberately cheap: case-insensitive equality, prefix match,
/// containment, or a small edit distance against the registry's tool list.
/// Structural matches (equality/prefix/containment) always outrank edit
/// distance, and among equals the smallest distance wins - a `find` over
/// registry order would otherwise let a loose containment match on an
/// early-listed tool beat a minimal-distance match later on.
///
/// # Examples
///
/// ```
/// use ragent_agent::session::tool_suggestion::suggest_tool_name;
/// use ragent_agent::tool::create_default_registry;
///
/// let registry = create_default_registry();
/// assert_eq!(
///     suggest_tool_name(&registry, "Bsh").as_deref(),
///     Some("Bash")
/// );
/// ```
#[must_use]
pub fn suggest_tool_name(registry: &crate::tool::ToolRegistry, name: &str) -> Option<String> {
    let lowered = name.to_lowercase();
    let lowered_len = lowered.chars().count();
    let max_distance = usize::max(2, lowered_len / 3);
    // Rank tiers: 0 = equality, 1 = prefix (either direction), 2 = containment
    // (either direction), 3 = edit distance. Within a tier, smaller edit
    // distance wins; ties keep the first candidate.
    let mut best: Option<(u8, usize, String)> = None;
    for candidate in registry.list() {
        let cand_lower = candidate.to_lowercase();
        let (rank, distance) = if cand_lower == lowered {
            (0, 0)
        } else if cand_lower.starts_with(&lowered) || lowered.starts_with(&cand_lower) {
            (1, edit_distance(&cand_lower, &lowered))
        } else if cand_lower.contains(&lowered) || lowered.contains(&cand_lower) {
            (2, edit_distance(&cand_lower, &lowered))
        } else {
            // Bail before building the distance matrix when the length
            // difference alone already exceeds the budget.
            let cand_len = cand_lower.chars().count();
            if cand_len.abs_diff(lowered_len) > max_distance {
                continue;
            }
            let distance = edit_distance(&cand_lower, &lowered);
            if distance > max_distance {
                continue;
            }
            (3, distance)
        };
        let better = match &best {
            None => true,
            Some((best_rank, best_distance, _)) => {
                rank < *best_rank || (rank == *best_rank && distance < *best_distance)
            }
        };
        if better {
            best = Some((rank, distance, candidate));
        }
    }
    best.map(|(_, _, candidate)| candidate)
}

/// Levenshtein edit distance between two tool names.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, a_char) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, b_char) in b.iter().enumerate() {
            let substitution_cost = usize::from(a_char != b_char);
            current[j + 1] = (previous[j + 1] + 1)
                .min(current[j] + 1)
                .min(previous[j] + substitution_cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[b.len()]
}
