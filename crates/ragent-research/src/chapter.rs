//! Dissertation chapter partitioning (FR-013, T-016).
//!
//! When a research run is started with `--tier dissertation --chapter-count N`,
//! the topic is split into `N` ordered chapters. Each chapter receives a title
//! and a focused set of search queries so the dissertation pipeline can gather
//! evidence chapter-by-chapter rather than treating the whole topic as a single
//! monolithic query.
//!
//! The partitioner is intentionally deterministic and lightweight: it does not
//! require an LLM call, so it works offline and is easy to test. Complex topics
//! are split on sentence/clause boundaries when possible; short topics are
//! expanded with standard dissertation dimensions (background, methods,
//! findings, discussion, future work, …).

use serde::{Deserialize, Serialize};

/// Default chapter count used when `--tier dissertation` is requested without
/// an explicit `--chapter-count`.
pub const DEFAULT_CHAPTER_COUNT: usize = 5;

/// A single chapter inside a dissertation research plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Chapter {
    /// Human-readable chapter title.
    pub title: String,
    /// Search queries that will drive evidence gathering for this chapter.
    pub queries: Vec<String>,
}

/// Full chapter plan produced by [`partition_topic`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ChapterPlan {
    /// Ordered chapters from first to last.
    pub chapters: Vec<Chapter>,
}

impl ChapterPlan {
    /// Total number of queries across all chapters.
    #[must_use]
    pub fn total_queries(&self) -> usize {
        self.chapters.iter().map(|c| c.queries.len()).sum()
    }

/// Collect all chapter titles in order.
    #[must_use]
    pub fn titles(&self) -> Vec<String> {
        self.chapters.iter().map(|c| c.title.clone()).collect()
    }

    /// Collect queries per chapter for progress events / JSON output.
    #[must_use]
    pub fn queries_by_chapter(&self) -> Vec<Vec<String>> {
        self.chapters.iter().map(|c| c.queries.clone()).collect()
    }
}

/// Partition `topic` into `count` dissertation chapters.
///
/// The algorithm prefers natural boundaries:
///
/// 1. Split the topic on `;`, `.`, `,`, ` and `, ` or `, or ` / ` into
///    segments.
/// 2. If enough non-empty segments exist, group them evenly into `count`
///    chapters and derive each chapter title from the first segment of the
///    group.
/// 3. Otherwise, fall back to a fixed set of dissertation dimensions
///    (overview, methods, findings, analysis, implications, …) appended to the
///    original topic.
///
/// `count` is clamped to at least 1 and at most 12 to keep plans readable.
#[must_use]
pub fn partition_topic(topic: &str, count: usize) -> ChapterPlan {
    let count = count.clamp(1, 12);
    let topic = topic.trim();
    if topic.is_empty() {
        return ChapterPlan {
            chapters: (1..=count)
                .map(|i| Chapter {
                    title: format!("Chapter {i}"),
                    queries: Vec::new(),
                })
                .collect(),
        };
    }

    let segments = split_segments(topic);
    if segments.len() >= count {
        build_plan_from_segments(&segments, count)
    } else {
        build_plan_from_dimensions(topic, count)
    }
}

/// Split a topic into candidate segments on common clause/phrase separators.
fn split_segments(topic: &str) -> Vec<String> {
    // Replace a few common conjunctions/punctuation with a sentinel so a single
    // split covers most cases.
    let normalized = topic
        .replace(" and ", " | ")
        .replace(" or ", " | ")
        .replace(" / ", " | ");
    normalized
        .split(|c: char| c == ';' || c == '.' || c == ',' || c == '|')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Build a chapter plan by grouping `segments` into `count` chapters.
fn build_plan_from_segments(segments: &[String], count: usize) -> ChapterPlan {
    let count = count.min(segments.len()).max(1);
    let base = segments.len() / count;
    let mut extra = segments.len() % count;
    let mut chapters = Vec::new();
    let mut pos = 0;
    for i in 0..count {
        let group_len = base + if extra > 0 { extra -= 1; 1 } else { 0 };
        let group = &segments[pos..pos + group_len];
        pos += group_len;
        let title = if group_len == 1 {
            group[0].clone()
        } else {
            format!("{} … {}", group[0], group.last().unwrap())
        };
        chapters.push(Chapter {
            title,
            queries: group.to_vec(),
        });
    }
    // Renumber with "Chapter N:" prefix for a consistent dissertation look.
    for (i, chapter) in chapters.iter_mut().enumerate() {
        chapter.title = format!("Chapter {}: {}", i + 1, chapter.title);
    }
    ChapterPlan { chapters }
}

/// Build a chapter plan from standard dissertation dimensions.
fn build_plan_from_dimensions(topic: &str, count: usize) -> ChapterPlan {
    let dimensions = [
        "overview and background",
        "methods and data",
        "empirical findings",
        "comparative analysis",
        "discussion and implications",
        "limitations and critique",
        "future directions",
        "conclusions",
    ];
    let count = count.min(dimensions.len()).max(1);
    let chapters: Vec<Chapter> = dimensions
        .iter()
        .take(count)
        .enumerate()
        .map(|(i, dim)| {
            let title = format!("Chapter {}: {} — {}", i + 1, topic, dim);
            let query = format!("{} {}", topic, dim);
            Chapter {
                title,
                queries: vec![query],
            }
        })
        .collect();
    ChapterPlan { chapters }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_defaults_to_one_when_count_zero() {
        let plan = partition_topic("impact of GLP-1 drugs", 0);
        assert_eq!(plan.chapters.len(), 1);
    }

    #[test]
    fn partition_clamps_count_to_twelve() {
        let plan = partition_topic("topic", 100);
        assert_eq!(plan.chapters.len(), 12);
    }

    #[test]
    fn partition_empty_topic_produces_numbered_chapters() {
        let plan = partition_topic("", 3);
        assert_eq!(plan.chapters.len(), 3);
        assert_eq!(plan.chapters[0].title, "Chapter 1");
        assert!(plan.chapters[0].queries.is_empty());
    }

    #[test]
    fn partition_uses_segments_when_enough_available() {
        let topic = "background; methods; findings; discussion; conclusion";
        let plan = partition_topic(topic, 3);
        assert_eq!(plan.chapters.len(), 3);
        assert!(plan.chapters[0].title.starts_with("Chapter 1:"));
        assert!(plan.chapters[0].queries.contains(&"background".to_string()));
        assert!(plan.total_queries() >= 5);
    }

    #[test]
    fn partition_falls_back_to_dimensions_for_short_topic() {
        let plan = partition_topic("GLP-1 cardiovascular outcomes", 5);
        assert_eq!(plan.chapters.len(), 5);
        assert!(plan.chapters[0].title.contains("overview and background"));
        assert!(plan.chapters[4].title.contains("implications"));
    }

    #[test]
    fn chapter_plan_collects_titles_and_queries() {
        let plan = partition_topic("a; b; c; d", 2);
        assert_eq!(plan.titles().len(), 2);
        assert_eq!(plan.queries_by_chapter().len(), 2);
    }
}
