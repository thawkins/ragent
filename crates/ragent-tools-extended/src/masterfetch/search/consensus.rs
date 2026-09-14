//! Cross-engine consensus merge, ranking, and related-query mining.
//!
//! Implements **FR-008**, **FR-009**, and **NFR-003** (T-015).
//!
//! This module merges results from multiple keyless search backends
//! (OpenAlex, Wikipedia, …), deduplicates by normalised URL, boosts results
//! that appear across multiple engines (cross-engine consensus), assigns a
//! normalised relevance score (0.0–1.0), derives a coarse `fetch_relevance`
//! tier (`high` / `med` / `low`), mines related queries from titles and
//! snippets, and computes a `fetch_hint` for the agent.
//!
//! # Consensus boost
//!
//! When the same URL is returned by multiple engines, its relevance score is
//! boosted. The boost is proportional to the number of *distinct* engines
//! that returned it — two engines returning the same URL is a stronger trust
//! signal than one engine returning it at position 1.
//!
//! # Relevance scoring
//!
//! Each result's base score is derived from its rank position within each
//! engine (higher rank = higher score, using a decay function). The final
//! score combines:
//!
//! 1. **Best rank score** — the highest positional score across all engines
//!    that returned this URL.
//! 2. **Consensus boost** — `+0.15` per additional engine beyond the first.
//! 3. **Normalisation** — clamped to `[0.0, 1.0]`.
//!
//! # `fetch_relevance` tier
//!
//! - `high` — score ≥ 0.6
//! - `med`  — score ≥ 0.3 and < 0.6
//! - `low`  — score < 0.3
//!
//! # `related_queries` mining
//!
//! Common terms (excluding stopwords) that appear across multiple result
//! titles and snippets are extracted as related queries. These help the
//! agent refine its search.
//!
//! # `fetch_hint`
//!
//! A short string advising the agent whether to fetch the result's full
//! content:
//!
//! - `"high relevance — fetch recommended"` — score ≥ 0.6
//! - `"medium relevance — fetch if relevant"` — score 0.3–0.6
//! - `"low relevance — skip unless needed"` — score < 0.3
//!
//! # Testability (NFR-003)
//!
//! The merge and ranking functions are pure — they take `[EngineReport]` or
//! `[RawResult]` and return [`ConsensusResult`] without any network I/O.
//!
//! # Examples
//!
//! Merge results from two engines:
//!
//! ```
//! use ragent_tools_extended::masterfetch::search::consensus::merge_and_rank;
//! use ragent_tools_extended::masterfetch::search::engine::{
//!     EngineReport, RawResult,
//! };
//!
//! let reports = vec![
//!     EngineReport::ok("duckduckgo", vec![
//!         RawResult::new("Rust Docs", "https://doc.rust-lang.org", "The Rust guide.", "duckduckgo"),
//!         RawResult::new("Other", "https://other.com", "Other content.", "duckduckgo"),
//!     ]),
//!     EngineReport::ok("brave", vec![
//!         RawResult::new("Rust Docs", "https://doc.rust-lang.org/std", "Rust standard library.", "brave"),
//!     ]),
//! ];
//! let result = merge_and_rank(&reports, "rust");
//! assert!(!result.results.is_empty());
//! // The URL that appeared in both engines should be ranked first.
//! assert_eq!(result.results[0].title, "Rust Docs");
//! // It should have consensus = "2/2".
//! assert!(result.results[0].engines_consensus.contains("2"));
//! ```

use std::collections::{HashMap, HashSet};

use super::engine::{
    EngineReport, RawResult, collect_all_results, normalise_result_url, report_is_failed,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Score boost per additional engine beyond the first that returned the same
/// URL (consensus boost).
const CONSENSUS_BOOST_PER_ENGINE: f64 = 0.15;

/// Minimum score for the `high` `fetch_relevance` tier.
const HIGH_TIER_THRESHOLD: f64 = 0.6;

/// Minimum score for the `med` `fetch_relevance` tier.
const MED_TIER_THRESHOLD: f64 = 0.3;

/// Maximum number of related queries to extract.
const MAX_RELATED_QUERIES: usize = 10;

/// Minimum term length for related-query mining (shorter terms are noise).
const MIN_TERM_LEN: usize = 3;

/// Stopwords excluded from related-query mining.
const STOPWORDS: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "any", "can", "had", "her", "was",
    "one", "our", "out", "day", "get", "has", "him", "his", "how", "its", "let", "may", "new",
    "now", "old", "see", "way", "who", "did", "yes", "yet", "this", "that", "with", "have", "from",
    "they", "will", "what", "about", "which", "when", "your", "here", "there", "their", "would",
    "could", "other", "more", "some", "such", "only", "into", "than", "them", "also", "been",
    "were", "over", "very", "much", "most", "many", "like", "just", "make", "made", "page", "site",
    "search", "results", "best", "top", "list", "guide",
];

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// A single merged and ranked search result.
///
/// This is the output of the consensus pipeline. Each field corresponds to a
/// column in the `mf_search` tool's text report and a key in its metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsensusResult {
    /// Result title (from the first engine that returned this URL).
    pub title: String,
    /// Result URL (normalised).
    pub url: String,
    /// Short snippet (from the first engine that returned this URL).
    pub snippet: String,
    /// Source: comma-separated list of engine names that returned this URL
    /// (e.g. `"duckduckgo, brave"`).
    pub source: String,
    /// 1-based position in the final ranked list.
    pub position: usize,
    /// Relevance score in the range `0.0..=1.0`.
    pub relevance_score: f64,
    /// Coarse relevance tier: `"high"`, `"med"`, or `"low"`.
    pub fetch_relevance: String,
    /// Cross-engine consensus label (e.g. `"2/3"` meaning 2 of 3 engines
    /// returned this URL).
    pub engines_consensus: String,
    /// Hint for the agent about whether to fetch this result's content.
    pub fetch_hint: String,
    /// Author name when one of the contributing engines exposed one in its
    /// result payload (e.g. Exa's `author` field or OpenAlex's joined
    /// `authorships` names). `None` when no engine provided an author.
    pub author: Option<String>,
}

/// The complete output of the consensus merge pipeline.
#[derive(Debug, Clone, Default)]
pub struct MergeOutput {
    /// Ranked results, best first.
    pub results: Vec<ConsensusResult>,
    /// Related queries mined from titles and snippets.
    pub related_queries: Vec<String>,
    /// Names of engines that were blocked or errored.
    pub blocked_engines: Vec<String>,
    /// Total number of results before dedup.
    pub total_raw_results: usize,
    /// Number of results after dedup.
    pub total_merged_results: usize,
    /// Number of engines that produced results.
    pub engines_with_results: usize,
    /// Total number of engines queried.
    pub total_engines: usize,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Merge, dedup, rank, and enrich results from multiple search engines.
///
/// This is the primary entry point for the consensus pipeline (FR-008,
/// FR-009). It:
///
/// 1. Collects all results from all engine reports.
/// 2. Deduplicates by normalised URL, merging entries from multiple engines.
/// 3. Scores each result based on best rank position + consensus boost.
/// 4. Sorts by score (descending).
/// 5. Assigns `fetch_relevance` tier and `fetch_hint`.
/// 6. Mines related queries from titles and snippets.
/// 7. Returns a [`MergeOutput`] with ranked results and metadata.
///
/// # Arguments
///
/// - `reports` — the [`EngineReport`]s from all queried backends.
/// - `query` — the original search query (used for related-query mining and
///   to exclude query terms from related queries).
///
/// # Returns
///
/// A [`MergeOutput`] with ranked [`ConsensusResult`]s, related queries, and
/// engine metadata.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::consensus::merge_and_rank;
/// use ragent_tools_extended::masterfetch::search::engine::{
///     EngineReport, RawResult,
/// };
///
/// let reports = vec![
///     EngineReport::ok("ddg", vec![
///         RawResult::new("A", "https://a.com", "Snip A", "ddg"),
///     ]),
///     EngineReport::ok("brave", vec![
///         RawResult::new("B", "https://b.com", "Snip B", "brave"),
///     ]),
/// ];
/// let output = merge_and_rank(&reports, "test");
/// assert_eq!(output.results.len(), 2);
/// assert_eq!(output.total_engines, 2);
/// ```
#[must_use]
pub fn merge_and_rank(reports: &[EngineReport], query: &str) -> MergeOutput {
    let total_engines = reports.len();
    let total_raw_results = reports.iter().map(|r| r.result_count).sum();
    let engines_with_results = reports.iter().filter(|r| r.has_results()).count();
    // Surface the block reason alongside the engine name so a bare "blocked"
    // list still tells the caller *why* (rate-limit vs quota vs HTTP error)
    // without forcing them into tracing logs (FR-008).
    let blocked_engines: Vec<String> = reports
        .iter()
        .filter(|r| report_is_failed(r))
        .map(|r| {
            if r.error.is_empty() {
                r.engine.clone()
            } else {
                format!("{}: {}", r.engine, r.error)
            }
        })
        .collect();

    // Collect all results (for related-query mining).
    let all_results = collect_all_results(reports);

    // Group by normalised URL, preserving per-engine ranks.
    let groups = group_by_url(reports);

    // Score each group.
    let mut scored: Vec<ScoredResult> = groups
        .into_iter()
        .map(|(norm_url, entries)| score_group(&norm_url, &entries, total_engines))
        .collect();

    scored.sort_by(|a, b| b.score.total_cmp(&a.score));

    // Build ConsensusResult with positions.
    let results: Vec<ConsensusResult> = scored
        .iter()
        .enumerate()
        .map(|(i, sr)| ConsensusResult {
            title: sr.title.clone(),
            url: sr.url.clone(),
            snippet: sr.snippet.clone(),
            source: sr.source.clone(),
            position: i + 1,
            relevance_score: sr.score.clamp(0.0, 1.0),
            fetch_relevance: tier_from_score(sr.score).to_string(),
            engines_consensus: format!("{}/{}", sr.engine_count, total_engines),
            fetch_hint: hint_from_score(sr.score).to_string(),
            author: sr.author.clone(),
        })
        .collect();

    // Mine related queries.
    let related_queries = mine_related_queries(&all_results, query);

    let total_merged_results = results.len();

    MergeOutput {
        results,
        related_queries,
        blocked_engines,
        total_raw_results,
        total_merged_results,
        engines_with_results,
        total_engines,
    }
}

// ---------------------------------------------------------------------------
// Internal: grouping and scoring
// ---------------------------------------------------------------------------

/// A scored result group (all entries for one normalised URL).
#[derive(Debug, Clone)]
struct ScoredResult {
    title: String,
    url: String,
    snippet: String,
    source: String,
    score: f64,
    engine_count: usize,
    author: Option<String>,
}

/// An entry in a result group, carrying the entry's rank position within its
/// own engine's result list (0-based) for rank scoring.
///
/// Ranks are per-engine rather than per flattened cross-engine list: the
/// first engine's result count must not penalise later engines' entries.
#[derive(Debug, Clone)]
struct GroupEntry {
    result: RawResult,
    rank_in_engine: usize,
}

/// Group raw results by their normalised URL, preserving each entry's rank
/// position within its own engine's result list.
///
/// Returns a map from normalised URL → list of group entries.
fn group_by_url(reports: &[EngineReport]) -> HashMap<String, Vec<GroupEntry>> {
    let mut groups: HashMap<String, Vec<GroupEntry>> = HashMap::new();
    for report in reports {
        for (rank, result) in report.results.iter().enumerate() {
            let norm = normalise_result_url(&result.url);
            groups.entry(norm).or_default().push(GroupEntry {
                result: result.clone(),
                rank_in_engine: rank,
            });
        }
    }
    groups
}

/// Score a group of results (all sharing the same normalised URL).
///
/// The score combines:
/// - Best entry score across all engines. When an engine provides its own
///   relevance score (`RawResult.score`, e.g. OpenAlex's `relevance_score`),
///   that engine-provided score is used directly. Otherwise a positional
///   rank score is derived from the entry's rank within its own engine's
///   result list (never from the flattened cross-engine list — an engine
///   returning many results must not bury later engines' entries).
/// - Consensus boost: +0.15 per additional engine beyond the first.
fn score_group(norm_url: &str, entries: &[GroupEntry], _total_engines: usize) -> ScoredResult {
    // Count distinct engines.
    let distinct_engines: HashSet<&str> =
        entries.iter().map(|e| e.result.source.as_str()).collect();
    let engine_count = distinct_engines.len();

    // Best entry score: use the engine-provided score when available (e.g.
    // OpenAlex's normalised `relevance_score`); fall back to the entry's
    // per-engine rank score for keyless backends that do not provide scores.
    let best_entry_score = entries
        .iter()
        .map(|e| {
            e.result
                .score
                .unwrap_or_else(|| rank_score(e.rank_in_engine))
        })
        .fold(0.0_f64, f64::max);

    // Consensus boost.
    let consensus_boost = (engine_count.saturating_sub(1)) as f64 * CONSENSUS_BOOST_PER_ENGINE;

    // Final score (unclamped — may exceed 1.0 for strong consensus).
    // Clamping to [0, 1] happens after sorting, in the ConsensusResult.
    let score = best_entry_score + consensus_boost;

    // Use the first entry's title, URL, and snippet.
    let first = entries.first().expect("group must have at least one entry");

    // Build source string: comma-separated engine names (sorted for
    // deterministic output — HashSet iteration order is random).
    let mut engine_names: Vec<&str> = distinct_engines.iter().copied().collect();
    engine_names.sort_unstable();
    let source = engine_names.join(", ");

    ScoredResult {
        title: first.result.title.clone(),
        url: norm_url.to_string(),
        snippet: first.result.snippet.clone(),
        source,
        score,
        engine_count,
        // Prefer the first engine that reported an author for this URL.
        author: entries.iter().find_map(|e| e.result.author.clone()),
    }
}

/// Compute a positional score from a 0-based rank index.
///
/// Uses a rational decay: `score = 1.0 / (1.0 + rank * 0.15)`.
///
/// The smoothing constant `0.15` prevents division by zero and ensures that even
/// the lowest-ranked results retain a non-zero score. This value was chosen to
/// provide a gentle decay curve where position matters but lower-ranked results
/// still contribute meaningfully to the consensus score.
///
/// Examples:
/// - Rank 0 → 1.0
/// - Rank 1 → ~0.87
/// - Rank 5 → ~0.57
/// - Rank 10 → ~0.40
/// - Rank 20 → ~0.25
fn rank_score(rank: usize) -> f64 {
    1.0 / (rank as f64).mul_add(0.15, 1.0)
}

/// Derive the `fetch_relevance` tier from a score.
fn tier_from_score(score: f64) -> &'static str {
    if score >= HIGH_TIER_THRESHOLD {
        "high"
    } else if score >= MED_TIER_THRESHOLD {
        "med"
    } else {
        "low"
    }
}

/// Derive the `fetch_hint` from a score.
fn hint_from_score(score: f64) -> &'static str {
    if score >= HIGH_TIER_THRESHOLD {
        "high relevance — fetch recommended"
    } else if score >= MED_TIER_THRESHOLD {
        "medium relevance — fetch if relevant"
    } else {
        "low relevance — skip unless needed"
    }
}

// ---------------------------------------------------------------------------
// Related-query mining
// ---------------------------------------------------------------------------

/// Mine related queries from result titles and snippets.
///
/// Extracts common terms (excluding stopwords and the original query terms)
/// that appear across multiple results. Terms are ranked by frequency and
/// truncated to [`MAX_RELATED_QUERIES`].
///
/// This is a pure function — no network I/O.
#[must_use]
pub fn mine_related_queries(results: &[RawResult], query: &str) -> Vec<String> {
    // Tokenise the query to exclude its terms from related queries.
    let query_terms: HashSet<String> = query
        .to_ascii_lowercase()
        .split_whitespace()
        .map(String::from)
        .collect();

    let stopwords: HashSet<&str> = STOPWORDS.iter().copied().collect();

    // Count term frequency across all titles and snippets.
    let mut term_counts: HashMap<String, usize> = HashMap::new();

    for result in results {
        let text = format!("{} {}", result.title, result.snippet).to_ascii_lowercase();
        for term in text.split_whitespace() {
            let cleaned = term
                .trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                .to_string();
            if cleaned.len() < MIN_TERM_LEN {
                continue;
            }
            if stopwords.contains(cleaned.as_str()) {
                continue;
            }
            if query_terms.contains(&cleaned) {
                continue;
            }
            *term_counts.entry(cleaned).or_insert(0) += 1;
        }
    }

    // Sort by frequency (descending), then alphabetically.
    let mut sorted: Vec<(String, usize)> = term_counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    // Take top N.
    sorted
        .into_iter()
        .take(MAX_RELATED_QUERIES)
        .map(|(term, _)| term)
        .collect()
}

// ---------------------------------------------------------------------------
// Helper: merge with max_results cap
// ---------------------------------------------------------------------------

/// Merge and rank results, capping to `max_results`.
///
/// Convenience wrapper around [`merge_and_rank`] that truncates the result
/// list to `max_results`. When two or more engines returned results, the
/// truncation is **diversity-aware**: no single engine may occupy more than
/// half of the capped slots, so an engine contributing a large scored result
/// block (e.g. OpenAlex returning up to 75 hits) cannot crowd every other
/// engine out of the final list. Slots no other engine can fill are handed
/// back to the highest-scoring entries of the dominant engine, so a run where
/// only one engine contributed still returns the full `max_results`.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::consensus::merge_and_rank_with_cap;
/// use ragent_tools_extended::masterfetch::search::engine::{
///     EngineReport, RawResult,
/// };
///
/// let reports = vec![
///     EngineReport::ok("ddg", vec![
///         RawResult::new("A", "https://a.com", "", "ddg"),
///         RawResult::new("B", "https://b.com", "", "ddg"),
///         RawResult::new("C", "https://c.com", "", "ddg"),
///     ]),
/// ];
/// let output = merge_and_rank_with_cap(&reports, "test", 2);
/// assert_eq!(output.results.len(), 2);
/// ```
#[must_use]
pub fn merge_and_rank_with_cap(
    reports: &[EngineReport],
    query: &str,
    max_results: usize,
) -> MergeOutput {
    let mut output = merge_and_rank(reports, query);
    output.results = diversity_truncate(&output.results, max_results);
    // Re-sync metadata and positions after truncation.
    output.total_merged_results = output.results.len();
    for (i, r) in output.results.iter_mut().enumerate() {
        r.position = i + 1;
    }
    output
}

/// Per-engine share limit applied by [`merge_and_rank_with_cap`].
///
/// A single engine may hold at most half of the merged slots (minimum 2) when
/// more than one engine returned results; with only one contributing engine
/// the limit is not applied.
fn per_engine_share_limit(max_results: usize) -> usize {
    (max_results.div_ceil(2)).max(2)
}

/// Count how many results each engine contributes across a result list.
///
/// The `source` field is a comma-separated engine list for consensus URLs
/// (one URL returned by several engines); each listed engine is credited
/// with the result so a consensus URL consumes budget in every engine that
/// backed it.
fn count_engines<'a>(results: impl Iterator<Item = &'a ConsensusResult>) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for result in results {
        for engine in result
            .source
            .split(',')
            .map(str::trim)
            .filter(|e| !e.is_empty())
        {
            *counts.entry(engine.to_string()).or_insert(0) += 1;
        }
    }
    counts
}

/// Truncate the ranked result list to `max_results` with a per-engine share
/// limit, so no single engine can monopolise the capped merge (see
/// [`merge_and_rank_with_cap`]).
///
/// Pass 1 keeps results whose every contributing engine is still under the
/// share limit. Pass 2 fills any remaining slots with the highest-scoring
/// skipped results regardless of engine, so a merge fed by engines with few
/// candidates still returns the full budget.
fn diversity_truncate(results: &[ConsensusResult], max_results: usize) -> Vec<ConsensusResult> {
    if results.len() <= max_results {
        return results.to_vec();
    }

    // One contributing engine → plain truncation keeps full fidelity.
    let engine_counts = count_engines(results.iter());
    if engine_counts.len() <= 1 {
        return results[..max_results].to_vec();
    }

    let limit = per_engine_share_limit(max_results);
    let mut kept: Vec<ConsensusResult> = Vec::with_capacity(max_results);
    let mut held = count_engines(std::iter::empty());
    let mut skipped: Vec<&ConsensusResult> = Vec::new();

    for result in results {
        if kept.len() == max_results {
            break;
        }
        let engines: Vec<&str> = result
            .source
            .split(',')
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .collect();
        let under_limit = engines
            .iter()
            .all(|e| held.get(*e).copied().unwrap_or(0) < limit);
        if under_limit {
            for e in &engines {
                *held.entry((*e).to_string()).or_insert(0) += 1;
            }
            kept.push(result.clone());
        } else {
            skipped.push(result);
        }
    }

    // Fill leftover slots (engines with fewer candidates than their share)
    // with the highest-scoring skipped results.
    for result in skipped {
        if kept.len() == max_results {
            break;
        }
        kept.push(result.clone());
    }

    kept
}
