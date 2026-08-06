//! Relevance scoring — compute a deterministic relevance label for a captured
//! web source based on the search query, title, snippet, and URL.
//!
//! These helpers were previously inline in `web_gatherer.rs`.

#[allow(dead_code)]
pub(crate) fn compute_relevance_label(
    query: &str,
    title: &str,
    snippet: &str,
    url: &str,
) -> (String, bool) {
    let query_terms = normalize_query_terms(query);
    if query_terms.is_empty() {
        return ("Match score unavailable".into(), true);
    }

    let title_lc = title.to_lowercase();
    let snippet_lc = snippet.to_lowercase();
    let url_lc = url.to_lowercase();
    let hay = format!("{} {} {}", title_lc, snippet_lc, url_lc);

    let mut hits = 0usize;
    let mut title_hits = 0usize;
    let mut snippet_hits = 0usize;
    for term in &query_terms {
        if hay.contains(term) {
            hits += 1;
            if title_lc.contains(term) {
                title_hits += 1;
            }
            if snippet_lc.contains(term) {
                snippet_hits += 1;
            }
        }
    }
    let ratio = hits as f64 / query_terms.len() as f64;

    let label = if !title.is_empty() && title_lc == query.to_lowercase() {
        "Very high — exact title match"
    } else if ratio >= 0.75 && title_hits > 0 && snippet_hits > 0 {
        "High — title + snippet match query"
    } else if ratio >= 0.6 && title_hits > 0 {
        "High — title matches query"
    } else if ratio >= 0.6 && snippet_hits > 0 {
        "Medium-high — snippet matches query"
    } else if ratio >= 0.45 {
        "Medium — partial query match"
    } else if ratio >= 0.2 {
        "Low — weak query match"
    } else {
        "Very low — no clear query match"
    };

    let retained = !label.starts_with("Low") && !label.starts_with("Very low");
    (label.into(), retained)
}

/// Normalize a query into lowercase, deduplicated terms suitable for
/// case-insensitive matching. Stopwords and very short tokens are removed.
///
/// This is intentionally exposed at module scope so benchmarks and unit tests
/// can measure it in isolation (Milestone B-003).
pub(crate) fn normalize_query_terms(query: &str) -> Vec<String> {
    let query_lc = query.to_lowercase();
    query_lc
        .split_whitespace()
        .filter(|t| !is_stopword_lc(t))
        .filter(|t| t.len() > 2 || t.chars().any(char::is_alphabetic))
        .map(std::string::ToString::to_string)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect()
}

/// Case-insensitive stopword check for an already-lowercased token.
fn is_stopword_lc(word: &str) -> bool {
    const STOPWORDS: &[&str] = &[
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "must", "can",
        "shall", "of", "in", "on", "at", "to", "for", "with", "from", "by", "about", "as", "and",
        "or", "but", "not", "no", "yes", "what", "which", "who", "when", "where", "why", "how",
        "this", "that", "these", "those", "i", "you", "he", "she", "it", "we", "they", "their",
        "there", "them", "his", "her", "its", "our", "your", "my", "me", "him", "us",
    ];
    STOPWORDS.contains(&word)
}

/// Returns true for common English stopwords that should not dilute the
/// relevance ratio. Removing them prevents a question like "What is Rust?"
/// from being scored as low relevance just because the auxiliary words do not
/// appear in the title or snippet.
#[allow(dead_code)]
fn is_stopword(word: &str) -> bool {
    is_stopword_lc(&word.to_lowercase())
}
