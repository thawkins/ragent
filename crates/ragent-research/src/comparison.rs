//! Comparison-table synthesis for `/research --mode competitive`.
//!
//! Implements FR-006, FR-014 and FR-016 of specs/opendeepresearch: a dedicated
//! `comparison-table` format must produce per-entity profiles and a Markdown
//! cross-entity comparison table with explicit criteria.
//!
//! The rendering is deterministic and LLM-agnostic so the artifact always
//! ships with the explicit table required by FR-016, even when the synthesis
//! model returns only compressed researcher notes.

use crate::entities::CompetitiveEntity;

/// A concise profile for one competitive entity.
#[derive(Debug, Clone)]
pub struct CompetitiveProfile {
    /// Entity display name.
    pub entity: String,
    /// Optional category label, e.g. "inference provider".
    pub category: Option<String>,
    /// Compressed findings / summary from the entity's researcher.
    pub summary: String,
}

impl CompetitiveProfile {
    /// Build a profile from a raw researcher summary.
    #[must_use]
    pub fn new(entity: &CompetitiveEntity, summary: impl Into<String>) -> Self {
        Self {
            entity: entity.name.clone(),
            category: entity.category.clone(),
            summary: summary.into(),
        }
    }
}

/// Build the Markdown comparison-artifact body for a competitive-analysis run.
///
/// The returned block contains:
///
/// 1. `## Comparison Criteria` — explicit axes detected for the comparison.
/// 2. `## Comparison Table` — a Markdown table with one row per entity and
///    one column per criterion plus a `Profile` column.
/// 3. `## Entity Profiles` — one subsection per entity with the full compressed
///    researcher summary.
///
/// The table cells are extracted heuristically from each profile summary by
/// scanning for the criterion keyword; when no evidence is found the cell
/// renders `—`.
#[must_use]
pub fn build_comparison_table_body(
    entities: &[CompetitiveEntity],
    criteria: &[String],
    profiles: &[CompetitiveProfile],
) -> String {
    if entities.is_empty() {
        return String::new();
    }

    let mut body = String::new();

    // ── Comparison Criteria ───────────────────────────────────────────────
    body.push_str("## Comparison Criteria\n\n");
    if criteria.is_empty() {
        body.push_str(
            "_(no explicit comparison criteria were detected — the comparison is general)_\n\n",
        );
    } else {
        for criterion in criteria {
            body.push_str(&format!("- {}\n", escape_markdown(criterion)));
        }
        body.push('\n');
    }

    // ── Comparison Table ────────────────────────────────────────────────
    body.push_str("## Comparison Table\n\n");
    body.push_str("| Entity |");
    for criterion in criteria {
        body.push_str(&format!(" {} |", escape_pipe(criterion)));
    }
    body.push_str(" Profile |\n");

    body.push_str("| --- |");
    for _ in criteria {
        body.push_str(" --- |");
    }
    body.push_str(" --- |\n");

    for entity in entities {
        let profile = profiles.iter().find(|p| p.entity == entity.name);
        body.push_str(&format!("| {} |", escape_pipe(&entity.name)));
        for criterion in criteria {
            let cell = profile
                .map(|p| extract_criterion_cell(&p.summary, criterion))
                .unwrap_or_else(|| "—".to_string());
            body.push_str(&format!(" {} |", escape_pipe(&cell)));
        }
        let profile_summary = profile
            .map(|p| compact_profile_summary(&p.summary))
            .unwrap_or_else(|| "—".to_string());
        body.push_str(&format!(" {} |\n", escape_pipe(&profile_summary)));
    }
    body.push('\n');

    // ── Entity Profiles ─────────────────────────────────────────────────
    body.push_str("## Entity Profiles\n\n");
    for entity in entities {
        let profile = profiles.iter().find(|p| p.entity == entity.name);
        let category_note = entity
            .category
            .as_ref()
            .map(|c| format!(" ({c})"))
            .unwrap_or_default();
        body.push_str(&format!(
            "### {}{}\n\n",
            escape_markdown(&entity.name),
            escape_markdown(&category_note)
        ));
        if let Some(profile) = profile {
            body.push_str(&escape_markdown(&profile.summary).trim());
            body.push_str("\n\n");
        } else {
            body.push_str("_(no researcher summary available for this entity)_\n\n");
        }
    }

    body
}

/// Extract a short cell value for `criterion` from `summary`.
///
/// The heuristic searches for the criterion keyword and returns the sentence
/// or phrase that contains it, truncated to keep table cells readable. When
/// the criterion is not mentioned, returns `"—"`.
fn extract_criterion_cell(summary: &str, criterion: &str) -> String {
    let lower_summary = summary.to_lowercase();
    let lower_criterion = criterion.to_lowercase();
    let keyword = lower_criterion
        .split_whitespace()
        .next()
        .unwrap_or(&lower_criterion);

    if let Some(idx) = lower_summary.find(keyword) {
        // Find the start of the sentence/line containing the keyword.
        let start = summary[..idx]
            .rfind(['\n', '.', ';'])
            .map(|i| i + 1)
            .unwrap_or(0);
        let end = summary[idx..]
            .find(['\n', '.', ';'])
            .map(|i| idx + i + 1)
            .unwrap_or(summary.len());
        let snippet = summary[start..end].trim();
        if snippet.len() > 120 {
            format!("{}…", snippet[..120].trim_end())
        } else {
            snippet.to_string()
        }
    } else {
        "—".to_string()
    }
}

/// Return a compact one-line profile summary for the table's Profile column.
fn compact_profile_summary(summary: &str) -> String {
    let first_line = summary.lines().next().unwrap_or(summary).trim();
    if first_line.len() > 140 {
        format!("{}…", first_line[..140].trim_end())
    } else {
        first_line.to_string()
    }
}

/// Escape pipe characters for Markdown tables.
fn escape_pipe(s: &str) -> String {
    s.replace('|', "\\|")
}

/// Escape characters that would break Markdown headings / lists.
fn escape_markdown(s: &str) -> String {
    s.replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(name: &str) -> CompetitiveEntity {
        CompetitiveEntity {
            name: name.to_string(),
            category: None,
        }
    }

    #[test]
    fn empty_entities_yields_empty_body() {
        let body = build_comparison_table_body(&[], &["pricing".into()], &[]);
        assert!(body.is_empty());
    }

    #[test]
    fn table_includes_entities_and_criteria() {
        let entities = vec![entity("Groq"), entity("Fireworks AI")];
        let criteria = vec!["pricing".to_string(), "speed/latency".to_string()];
        let profiles = vec![
            CompetitiveProfile::new(
                &entities[0],
                "Groq offers aggressive per-token pricing and very low latency.",
            ),
            CompetitiveProfile::new(
                &entities[1],
                "Fireworks AI emphasizes batch pricing and throughput over raw latency.",
            ),
        ];
        let body = build_comparison_table_body(&entities, &criteria, &profiles);
        assert!(body.contains("## Comparison Criteria"));
        assert!(body.contains("## Comparison Table"));
        assert!(body.contains("## Entity Profiles"));
        assert!(body.contains("| Groq |"));
        assert!(body.contains("| Fireworks AI |"));
        assert!(body.contains(" pricing |"));
        assert!(body.contains(" speed/latency |"));
    }

    #[test]
    fn missing_criterion_renders_dash() {
        let entities = vec![entity("A")];
        let criteria = vec!["security".to_string()];
        let profiles = vec![CompetitiveProfile::new(
            &entities[0],
            "A is fast and cheap.",
        )];
        let body = build_comparison_table_body(&entities, &criteria, &profiles);
        assert!(body.contains("| — |"));
    }
}
