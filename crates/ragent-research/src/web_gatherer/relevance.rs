//! Relevance scoring — compute a deterministic relevance label for a captured
//! web source based on the search query, title, snippet, and URL.
//!
//! These helpers were previously inline in `web_gatherer.rs`.

#[allow(dead_code)]
// reason: only consumed inside this crate - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
pub fn compute_relevance_label(
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
        if term_matches(term, &hay) {
            hits += 1;
            if term_matches(term, &title_lc) {
                title_hits += 1;
            }
            if term_matches(term, &snippet_lc) {
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
    } else if ratio >= 0.25 && title_hits >= 2 {
        // Title-signal rescue: decomposed sub-queries are often verbose
        // ("how to write goals and configure AI agent loops" — 6 terms), so
        // an on-topic title like "Designing agentic loops" only matches 2 of
        // them (ratio 0.33) and would fall below the Medium floor. Two or
        // more distinct query terms in the *title* is a strong topical
        // signal on its own; retain it.
        "Medium — multiple title terms match query"
    } else if ratio >= 0.35 {
        "Medium — partial query match"
    } else if ratio >= 0.2 {
        "Low — weak query match"
    } else {
        "Very low — no clear query match"
    };

    let retained = !label.starts_with("Low") && !label.starts_with("Very low");
    (label.into(), retained)
}

/// Case-insensitive morphological term matching.
///
/// Returns true when `term` appears in `hay` either literally or via a
/// morphological variant (plural, gerund, or derivational form), so that
/// queries using e.g. "loops" match titles using "loop" and "agentic"
/// matches "agent" (and vice versa). Prevents the lexical pre-filter from
/// rejecting on-topic results whose vocabulary differs only by inflection.
///
/// This is intentionally exposed at module scope so benchmarks and unit tests
/// can measure it in isolation (Milestone B-003).
// reason: only consumed inside this crate - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
pub fn term_matches(term: &str, hay: &str) -> bool {
    if hay.contains(term) {
        return true;
    }
    morph_variants(term).into_iter().any(|v| hay.contains(&v))
}

/// Derive the morphological variants of an already-lowercased term.
///
/// Two families are covered:
/// - inflectional suffixes (`-ies`, `-es`, `-s`): "loops" -> "loop",
///   "queries" -> "query";
/// - derivational suffixes (`-ing`, `-ics`, `-ic`, `-ly`, `-ment`, `-ness`,
///   `-ation`, `-tion`, `-sion`, `-ity`): "agentic" -> "agent",
///   "engineering" -> "engine", "prompting" -> "prompt".
fn morph_variants(term: &str) -> Vec<String> {
    const INFLECTIONS: &[&str] = &["ies", "es", "s"];
    const DERIVATIONS: &[&str] = &[
        "ing", "ics", "ic", "ly", "ment", "ness", "ation", "tion", "sion", "ity",
    ];

    let mut variants = Vec::new();
    for suffix in INFLECTIONS {
        if let Some(stem) = term.strip_suffix(suffix) {
            if stem.len() >= 3 {
                if *suffix == "ies" {
                    variants.push(format!("{stem}y"));
                } else {
                    variants.push(stem.to_string());
                    if *suffix == "es" {
                        variants.push(format!("{stem}e"));
                    }
                }
            }
            break;
        }
    }
    for suffix in DERIVATIONS {
        if let Some(stem) = term.strip_suffix(suffix) {
            if stem.len() >= 3 {
                // Gerund doubling: "running" -> "run" (strip one doubled
                // final consonant, keeping vowels intact).
                if *suffix == "ing" && stem.len() >= 2 {
                    let bytes = stem.as_bytes();
                    if bytes[bytes.len() - 1] == bytes[bytes.len() - 2]
                        && !stem.ends_with(['a', 'e', 'i', 'o', 'u'])
                    {
                        variants.push(stem[..stem.len() - 1].to_string());
                    }
                }
                variants.push(stem.to_string());
                // One chained agentive strip: "engineering" -> "engineer"
                // still fails to match "engine", so also strip a trailing
                // "-er"/"-eer"/"-or" from the derived stem.
                for agentive in ["eer", "er", "or"] {
                    if let Some(stem2) = stem.strip_suffix(agentive) {
                        if stem2.len() >= 3 {
                            variants.push(stem2.to_string());
                        }
                        break;
                    }
                }
            }
            break;
        }
    }
    variants
}

/// Normalize a query into lowercase, deduplicated terms suitable for
/// case-insensitive matching. Stopwords and very short tokens are removed.
///
/// This is intentionally exposed at module scope so benchmarks and unit tests
/// can measure it in isolation (Milestone B-003).
// reason: only consumed inside this crate - `pub` here never escapes the crate.
#[allow(unreachable_pub)]
pub fn normalize_query_terms(query: &str) -> Vec<String> {
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
///
/// Returns true for common English stopwords that should not dilute the
/// relevance ratio. Removing them prevents a question like "What is Rust?"
/// from being scored as low relevance just because the auxiliary words do not
/// appear in the title or snippet.
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
