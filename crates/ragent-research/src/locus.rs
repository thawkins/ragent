//! Loci analysis and depth investigation (FR-005, T-008).
//!
//! A *locus* is a key claim or dimension that recurs across the gathered
//! corpus. The analysis in this module is intentionally deterministic and
//! LLM-free: it scans source bodies for a small set of research dimensions
//! (e.g. `performance`, `cost`, `safety`) and records which sources mention
//! each dimension and how often. The depth investigation then classifies each
//! locus as `Surface`, `Moderate`, or `Deep` based on the number of distinct
//! sources that support it.
//!
//! Like the contradiction graph, this is a starting heuristic. Later tasks
//! can replace or augment the keyword lists while keeping the same output
//! structures.

use crate::source::Source;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The keyword is matched case-insensitively against source bodies; the label is
/// rendered in the report.
const DIMENSIONS: &[(&str, &str)] = &[
    ("performance", "Performance"),
    ("cost", "Cost"),
    ("safety", "Safety"),
    ("efficacy", "Efficacy"),
    ("adoption", "Adoption"),
    ("mortality", "Mortality"),
    ("mechanism", "Mechanism"),
    ("risk", "Risk"),
    ("benefit", "Benefit"),
    ("side effects", "Side effects"),
    ("quality", "Quality"),
    ("usability", "Usability"),
    ("scalability", "Scalability"),
    ("reliability", "Reliability"),
    ("accessibility", "Accessibility"),
];

/// Evidence-depth classification for a single locus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DepthLevel {
    /// Only one source mentions the locus.
    #[default]
    Surface,
    /// Two or three sources mention the locus.
    Moderate,
    /// Four or more sources mention the locus.
    Deep,
}

impl DepthLevel {
    /// Human-readable label for report rendering.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Surface => "surface",
            Self::Moderate => "moderate",
            Self::Deep => "deep",
        }
    }
}

/// One detected locus in the gathered corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Locus {
    /// Short dimension keyword (e.g. `"performance"`).
    pub keyword: String,
    /// Human-readable label (e.g. `"Performance"`).
    pub label: String,
    /// 1-based indices of sources that mention this locus, in ascending order.
    pub source_indices: Vec<usize>,
    /// Short representative snippets from source bodies, one per source (up to
    /// a small limit).
    pub snippets: Vec<String>,
    /// Total number of distinct source bodies that mention this locus.
    pub mentions: usize,
}

/// A collection of loci discovered in the corpus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LocusSet {
    /// Loci sorted from most to least supported.
    pub loci: Vec<Locus>,
}

impl LocusSet {
    /// Create an empty locus set.
    #[must_use]
    pub fn empty() -> Self {
        Self { loci: Vec::new() }
    }

    /// Return `true` when no loci were detected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.loci.is_empty()
    }

    /// Number of detected loci.
    #[must_use]
    pub fn len(&self) -> usize {
        self.loci.len()
    }
}

/// Depth-investigation result for a single locus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DepthInvestigation {
    /// Dimension keyword this result investigates.
    pub keyword: String,
    /// Human-readable label for the locus.
    pub label: String,
    /// Classified evidence depth.
    pub depth: DepthLevel,
    /// Number of distinct sources that mention this locus.
    pub source_count: usize,
    /// Human-readable note summarising the depth and coverage.
    pub note: String,
    /// Up to three representative 1-based source indices.
    pub representative_sources: Vec<usize>,
}

/// Analyse the gathered sources and return the set of detected loci.
///
/// The analysis is deterministic: it scans every source body for the dimension
/// keywords in [`DIMENSIONS`] and counts how many distinct sources mention
/// each keyword. The result is sorted by mention count descending, then by
/// keyword label. Only loci with at least one source are included.
#[must_use]
pub fn analyze_loci(sources: &[Source]) -> LocusSet {
    if sources.len() < 2 {
        return LocusSet::empty();
    }

    let mut hits: HashMap<&str, Vec<(usize, String)>> = HashMap::new();

    for (idx, src) in sources.iter().enumerate() {
        let body = source_body_text(src);
        if body.len() < 20 {
            continue;
        }
        let lower = body.to_lowercase();
        for (keyword, _label) in DIMENSIONS {
            if lower.contains(keyword) {
                let one_based = idx + 1;
                let snippet = extract_snippet(&body, keyword);
                hits.entry(keyword).or_default().push((one_based, snippet));
            }
        }
    }

    let mut loci: Vec<Locus> = hits
        .into_iter()
        .map(|(keyword, entries)| {
            let label = dimension_label(keyword).to_string();
            let mut indices: Vec<usize> = entries.iter().map(|(i, _)| *i).collect();
            indices.sort_unstable();
            indices.dedup();
            let snippets: Vec<String> = entries
                .into_iter()
                .map(|(_, s)| s)
                .filter(|s| !s.is_empty())
                .take(3)
                .collect();
            let mentions = indices.len();
            Locus {
                keyword: keyword.to_string(),
                label,
                source_indices: indices,
                snippets,
                mentions,
            }
        })
        .filter(|l| !l.source_indices.is_empty())
        .collect();

    // Sort by descending support, then label for stability.
    loci.sort_by(|a, b| b.mentions.cmp(&a.mentions).then(a.label.cmp(&b.label)));
    LocusSet { loci }
}

/// Investigate the depth of each detected locus.
///
/// Returns one [`DepthInvestigation`] per locus with a depth classification
/// based on source coverage and a short human-readable note.
#[must_use]
pub fn investigate_depth(loci: &LocusSet) -> Vec<DepthInvestigation> {
    loci.loci
        .iter()
        .map(|locus| {
            let depth = match locus.source_indices.len() {
                0 => DepthLevel::Surface, // should not happen because empty sets are filtered
                1 => DepthLevel::Surface,
                2 | 3 => DepthLevel::Moderate,
                _ => DepthLevel::Deep,
            };
            let representative_sources = locus.source_indices.iter().copied().take(3).collect();
            let note = format!(
                "Detected in {} source{} (depth: {}).",
                locus.source_indices.len(),
                if locus.source_indices.len() == 1 {
                    ""
                } else {
                    "s"
                },
                depth.as_str()
            );
            DepthInvestigation {
                keyword: locus.keyword.clone(),
                label: locus.label.clone(),
                depth,
                source_count: locus.source_indices.len(),
                note,
                representative_sources,
            }
        })
        .collect()
}

/// Extract a short snippet around the first occurrence of `keyword` in `body`.
///
/// Returns up to ~100 characters centred on the keyword, or an empty string
/// if the keyword cannot be found.
fn extract_snippet(body: &str, keyword: &str) -> String {
    let lower_body = body.to_lowercase();
    let lower_kw = keyword.to_lowercase();
    let Some(pos) = lower_body.find(&lower_kw) else {
        return String::new();
    };

    let start = pos.saturating_sub(40);
    let end = (pos + keyword.len() + 40).min(body.len());
    let snippet = &body[start..end];
    snippet.replace('\n', " ").trim().to_string()
}

/// Look up the human-readable label for a dimension keyword.
fn dimension_label(keyword: &str) -> &'static str {
    DIMENSIONS
        .iter()
        .find(|(kw, _)| *kw == keyword)
        .map(|(_, label)| *label)
        .unwrap_or("Unknown")
}

/// Extract searchable body text from a source.
fn source_body_text(source: &Source) -> String {
    match source {
        Source::Web { body, .. } => body.clone(),
        Source::Local { body, .. } => body.clone(),
        Source::Spec { spec_id, .. } => spec_id.clone(),
        Source::Other { body, .. } => body.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Source;
    use std::path::PathBuf;

    fn web_source(index: usize, body: &str) -> Source {
        Source::Web {
            url: format!("https://example.com/{index}"),
            title: format!("Source {index}"),
            captured_at: chrono::Utc::now(),
            published_at: None,
            body_path: PathBuf::new(),
            body: body.into(),
            relevance: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: None,
            page_type: None,
            media_type: "page".into(),
            language: None,
            oa_recovery: None,
        }
    }

    #[test]
    fn loci_empty_when_fewer_than_two_sources() {
        let sources = vec![web_source(1, "The system improves performance.")];
        let set = analyze_loci(&sources);
        assert!(set.is_empty());
    }

    #[test]
    fn loci_detects_shared_dimension() {
        let sources = vec![
            web_source(1, "The system improves performance dramatically."),
            web_source(2, "Performance remains a key concern for users."),
        ];
        let set = analyze_loci(&sources);
        let locus = set
            .loci
            .iter()
            .find(|l| l.keyword == "performance")
            .expect("performance locus should be detected");
        assert_eq!(locus.mentions, 2);
        assert!(locus.source_indices.contains(&1));
        assert!(locus.source_indices.contains(&2));
        assert_eq!(locus.label, "Performance");
    }

    #[test]
    fn loci_sorted_by_mentions() {
        let sources = vec![
            web_source(1, "Performance is great and cost is low."),
            web_source(2, "Performance is excellent."),
            web_source(3, "Performance is okay."),
            web_source(4, "Cost is the only metric."),
        ];
        let set = analyze_loci(&sources);
        assert!(!set.is_empty());
        assert_eq!(set.loci[0].keyword, "performance");
        assert_eq!(set.loci[0].mentions, 3);
    }

    #[test]
    fn depth_classifies_by_source_count() {
        let loci = LocusSet {
            loci: vec![
                Locus {
                    keyword: "performance".into(),
                    label: "Performance".into(),
                    source_indices: vec![1],
                    snippets: Vec::new(),
                    mentions: 1,
                },
                Locus {
                    keyword: "cost".into(),
                    label: "Cost".into(),
                    source_indices: vec![1, 2],
                    snippets: Vec::new(),
                    mentions: 2,
                },
                Locus {
                    keyword: "safety".into(),
                    label: "Safety".into(),
                    source_indices: vec![1, 2, 3, 4],
                    snippets: Vec::new(),
                    mentions: 4,
                },
            ],
        };
        let depths = investigate_depth(&loci);
        assert_eq!(depths.len(), 3);
        assert_eq!(depths[0].depth, DepthLevel::Surface);
        assert_eq!(depths[1].depth, DepthLevel::Moderate);
        assert_eq!(depths[2].depth, DepthLevel::Deep);
    }

    #[test]
    fn snippet_extracts_context_around_keyword() {
        let body = "The quick brown fox improves performance under load and scales well.";
        let snippet = extract_snippet(body, "performance");
        assert!(snippet.to_lowercase().contains("performance"));
        assert!(snippet.contains("improves"));
    }

    #[test]
    fn loci_case_insensitive_match() {
        let sources = vec![
            web_source(1, "PERFORMANCE is critical."),
            web_source(2, "We measured Performance carefully."),
        ];
        let set = analyze_loci(&sources);
        assert!(set.loci.iter().any(|l| l.keyword == "performance"));
    }

    #[test]
    fn loci_ignores_short_bodies() {
        let sources = vec![
            web_source(1, "Performance"),
            web_source(2, "Performance is great and performance is good."),
        ];
        let set = analyze_loci(&sources);
        let perf = set.loci.iter().find(|l| l.keyword == "performance");
        assert_eq!(perf.map(|l| l.mentions), Some(1));
    }
}
