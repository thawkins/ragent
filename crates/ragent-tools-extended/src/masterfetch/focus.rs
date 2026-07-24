//! Query-focused content filtering for masterfetch.
//!
//! Implements **FR-004** and **NFR-003**.
//!
//! When the agent passes a `focus` query to `mf_fetch` or `mf_crawl`, the
//! extracted markdown is filtered to the blocks (paragraphs / headings /
//! tables / lists) most relevant to the query, so the agent loads less context
//! on long pages. Inspired by `Crawl4AI`'s `BM25ContentFilter`, implemented
//! locally with no extra dependency.
//!
//! # Design
//!
//! Focus runs **post-extraction** (and therefore post-cache). The full
//! extracted text is cached once per URL; different focus queries are just
//! different views over the same cached content, so focusing never causes a
//! re-fetch and two different focuses on the same URL share one cache entry.
//!
//! ## BM25 parameters
//!
//! Uses BM25 with `k1 = 1.5` and `b = 0.75` and an **always-positive IDF**
//! (the `+1` inside the logarithm) so that a block with a single query-term
//! occurrence gets a positive score and is kept at the default threshold.
//!
//! ## Block splitting
//!
//! Markdown is split into blocks separated by blank lines. A block is a
//! heading, paragraph, table, or list — kept verbatim with order preserved.
//!
//! ## Heading preservation
//!
//! A heading immediately preceding a kept non-heading block is preserved for
//! context.
//!
//! ## Fallback
//!
//! If no blocks clear the relevance threshold, the closest blocks (by BM25
//! score) are returned so the agent has something to judge rather than an
//! empty page.
//!
//! ## No-op conditions
//!
//! Focus is a no-op (returns the original text unchanged) when:
//!
//! - The query is empty or whitespace-only.
//! - The text is empty or whitespace-only.
//! - The text has one or fewer blocks.
//! - The query yields no usable terms (all tokens shorter than 2 chars).
//!
//! All public functions are pure — no network I/O — enabling unit tests
//! without live pages (NFR-003).
//!
//! # Examples
//!
//! ```
//! use ragent_tools_extended::masterfetch::focus::focus_content;
//!
//! let text = "# Heading\n\nRust is a systems programming language.\n\n\
//!             Python is great for data science.\n\n\
//!             Go is simple and fast.";
//! let focused = focus_content(text, "rust programming");
//! // The Rust block is kept; the header is preserved for context.
//! assert!(focused.contains("Rust is a systems programming language."));
//! assert!(focused.contains("# Heading"));
//! ```

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// BM25 term-frequency saturation parameter.
///
/// Controls how quickly an increase in term frequency stops adding to the
/// score. `k1 = 1.5` is the standard value used by Hound.
pub const DEFAULT_K1: f64 = 1.5;

/// BM25 length-normalisation parameter.
///
/// Controls how much document length affects scoring. `b = 0.75` is the
/// standard value used by Hound.
pub const DEFAULT_B: f64 = 0.75;

/// Default BM25 score threshold above which a block is kept.
///
/// A block scoring `>= 1.0` is kept. With the always-positive IDF a single
/// query-term occurrence in a block typically yields a positive score.
pub const DEFAULT_THRESHOLD: f64 = 1.0;

/// Default number of closest blocks to keep when nothing clears the threshold.
pub const DEFAULT_FALLBACK_TOP: usize = 5;

/// Minimum token length (in characters) to be considered a query/content term.
///
/// Single-character tokens are dropped to avoid noise (e.g. "a", "1").
const MIN_TOKEN_LEN: usize = 2;

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

/// Tunable parameters for [`focus_content_with_params`].
///
/// All fields have sensible defaults accessible via [`FocusParams::default`].
#[derive(Debug, Clone)]
pub struct FocusParams {
    /// BM25 score threshold above which a block is kept (default `1.0`).
    pub threshold: f64,
    /// BM25 `k1` saturation parameter (default `1.5`).
    pub k1: f64,
    /// BM25 `b` length-normalisation parameter (default `0.75`).
    pub b: f64,
    /// Number of closest blocks to keep when nothing clears the threshold
    /// (default `5`).
    pub fallback_top: usize,
}

impl Default for FocusParams {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_THRESHOLD,
            k1: DEFAULT_K1,
            b: DEFAULT_B,
            fallback_top: DEFAULT_FALLBACK_TOP,
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Filter `text` to the blocks most relevant to `query` using BM25 scoring.
///
/// This is the primary entry point. It uses the default BM25 parameters
/// (`k1=1.5`, `b=0.75`, threshold `1.0`, fallback top `5`). For custom
/// parameters use [`focus_content_with_params`].
///
/// # No-op conditions
///
/// Returns the original `text` unchanged when:
/// - `query` is empty or whitespace-only.
/// - `text` is empty or whitespace-only.
/// - `text` has one or fewer blocks.
/// - `query` yields no usable terms.
///
/// # Output
///
/// When focus is applied, the result is prefixed with a header line:
///
/// ```text
/// [Focus: 'query'; showing X of N blocks by BM25 relevance.
///  Pass focus='' for the full page.]
/// ```
///
/// followed by the kept blocks joined by blank lines.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::focus::focus_content;
///
/// // Empty query → no-op.
/// assert_eq!(focus_content("some text", ""), "some text");
///
/// // Single block → no-op.
/// assert_eq!(focus_content("only one block", "query"), "only one block");
/// ```
#[must_use]
pub fn focus_content(text: &str, query: &str) -> String {
    focus_content_with_params(text, query, &FocusParams::default())
}

/// Filter `text` to the blocks most relevant to `query` using BM25 scoring
/// with custom parameters.
///
/// See [`focus_content`] for behaviour and [`FocusParams`] for the tunable
/// parameters.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::focus::{
///     FocusParams, focus_content_with_params,
/// };
///
/// let text = "alpha beta gamma\n\ndelta epsilon zeta\n\nalpha beta delta";
/// let params = FocusParams {
///     threshold: 0.0, // keep everything that scores > 0
///     ..FocusParams::default()
/// };
/// let focused = focus_content_with_params(text, "alpha", &params);
/// assert!(focused.contains("alpha beta gamma"));
/// ```
pub fn focus_content_with_params(text: &str, query: &str, params: &FocusParams) -> String {
    // No-op: empty query or empty text.
    if query.trim().is_empty() || text.trim().is_empty() {
        return text.to_string();
    }

    let blocks = split_blocks(text);
    if blocks.len() <= 1 {
        return text.to_string();
    }

    let qterms: Vec<String> = tokenize(query)
        .into_iter()
        .filter(|t| t.len() >= MIN_TOKEN_LEN)
        .collect();
    let qterms_set: std::collections::HashSet<&str> = qterms.iter().map(String::as_str).collect();
    if qterms_set.is_empty() {
        return text.to_string();
    }

    let block_tokens: Vec<Vec<String>> = blocks.iter().map(|b| tokenize(b)).collect();
    let n = blocks.len();

    // Average document (block) length in tokens.
    let total_tokens: usize = block_tokens.iter().map(std::vec::Vec::len).sum();
    let avgdl = if n > 0 && total_tokens > 0 {
        total_tokens as f64 / n as f64
    } else {
        1.0
    };

    // Document frequency per term (number of blocks containing the term).
    let mut df: HashMap<&str, usize> = HashMap::new();
    for toks in &block_tokens {
        let unique: std::collections::HashSet<&str> = toks.iter().map(String::as_str).collect();
        for t in unique {
            *df.entry(t).or_insert(0) += 1;
        }
    }

    // Compute BM25 score for each block.
    let scores: Vec<f64> = (0..n)
        .map(|i| {
            let toks = &block_tokens[i];
            if toks.is_empty() {
                return 0.0;
            }
            let dl = toks.len() as f64;
            let denom_len = params.k1 * (1.0 - params.b + params.b * dl / avgdl);

            // Term frequencies within this block.
            let mut tf: HashMap<&str, usize> = HashMap::new();
            for t in toks {
                *tf.entry(t.as_str()).or_insert(0) += 1;
            }

            let mut s = 0.0;
            for term in &qterms_set {
                if let Some(&f) = tf.get(term) {
                    let idf_val = idf(term, n, &df);
                    let f_f = f as f64;
                    s += idf_val * (f_f * (params.k1 + 1.0)) / (f_f + denom_len);
                }
            }
            s
        })
        .collect();

    // Keep blocks above threshold.
    let mut keep: Vec<usize> = (0..n).filter(|&i| scores[i] >= params.threshold).collect();

    if keep.is_empty() {
        // Fallback: keep the top-N closest blocks by score.
        let mut indexed: Vec<(usize, f64)> = (0..n).map(|i| (i, scores[i])).collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        keep = indexed
            .into_iter()
            .take(params.fallback_top)
            .map(|(i, _)| i)
            .collect();
        keep.sort_unstable();
    }

    // Preserve a heading immediately preceding a kept non-heading block.
    let mut keep_set: std::collections::HashSet<usize> = keep.iter().copied().collect();
    for &i in &keep {
        if i > 0 && is_heading(&blocks[i - 1]) && !is_heading(&blocks[i]) {
            keep_set.insert(i - 1);
        }
    }

    // Assemble the kept blocks in original order.
    let kept: Vec<&str> = (0..n)
        .filter(|i| keep_set.contains(i))
        .map(|i| blocks[i].as_str())
        .collect();
    let kept_text = kept.join("\n\n");

    let header = format!(
        "[Focus: {:?}; showing {} of {} blocks by BM25 relevance. \
         Pass focus='' for the full page.]",
        query,
        keep_set.len(),
        n
    );

    format!("{header}\n\n{kept_text}")
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Tokenise text into lowercase alphanumeric tokens.
///
/// Splits on any non-alphanumeric character, lowercases the result, and
/// returns all tokens (filtering by minimum length is done by the caller).
fn tokenize(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

/// Compute the always-positive IDF for a term.
///
/// Uses the BM25+ variant: `ln((n - df + 0.5) / (df + 0.5) + 1)`. The `+1`
/// inside the logarithm keeps the IDF positive so a single occurrence always
/// scores above zero.
fn idf(term: &str, n: usize, df: &HashMap<&str, usize>) -> f64 {
    let d = *df.get(term).unwrap_or(&0) as f64;
    let nf = n as f64;
    ((nf - d + 0.5) / (d + 0.5)).ln_1p()
}

/// Check whether a block's first non-blank line is a markdown heading.
///
/// A heading starts with `#` (after optional leading whitespace).
fn is_heading(block: &str) -> bool {
    for line in block.lines() {
        let trimmed = line.trim_start();
        if !trimmed.is_empty() {
            return trimmed.starts_with('#');
        }
    }
    false
}

/// Split markdown text into blocks separated by blank lines.
///
/// A block is a heading, paragraph, table, or list. Blocks are kept verbatim
/// (internal whitespace preserved) and order is maintained. Trailing
/// whitespace-only lines are not emitted as empty blocks.
fn split_blocks(text: &str) -> Vec<String> {
    let mut blocks: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();

    for line in text.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current.join("\n"));
    }
    blocks
}
