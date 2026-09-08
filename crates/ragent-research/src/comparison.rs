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

    // Profile lookup is keyed once instead of an O(entities x profiles)
    // linear scan per table cell and per profile section.
    let by_entity: std::collections::HashMap<&str, &CompetitiveProfile> =
        profiles.iter().map(|p| (p.entity.as_str(), p)).collect();

    // Filtering + lowercasing a summary is O(summary); prepare each profile
    // once instead of once per (entity, criterion) table cell.
    let mut prepared_by_entity: std::collections::HashMap<&str, PreparedSummary> =
        std::collections::HashMap::new();
    for profile in profiles {
        prepared_by_entity
            .entry(profile.entity.as_str())
            .or_insert_with(|| prepare_summary(&profile.summary));
    }

    for entity in entities {
        let profile = by_entity.get(entity.name.as_str()).copied();
        body.push_str("| ");
        body.push_str(&escape_pipe(&entity.name));
        body.push_str(" |");
        for criterion in criteria {
            let cell = profile
                .and_then(|p| prepared_by_entity.get(p.entity.as_str()))
                .map(|prepared| criterion_cell(prepared, criterion))
                .unwrap_or_else(|| "—".to_string());
            body.push(' ');
            body.push_str(&escape_pipe(&cell));
            body.push_str(" |");
        }
        let profile_summary = profile
            .filter(|p| !p.summary.trim().is_empty())
            .map(|p| compact_profile_summary(&p.summary))
            .unwrap_or_else(|| "—".to_string());
        body.push(' ');
        body.push_str(&escape_pipe(&profile_summary));
        body.push_str(" |\n");
    }
    body.push('\n');

    // ── Entity Profiles ─────────────────────────────────────────────────
    body.push_str("## Entity Profiles\n\n");
    for entity in entities {
        let profile = by_entity.get(entity.name.as_str()).copied();
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
        let summary_text = profile
            .filter(|p| !p.summary.trim().is_empty())
            .map(|p| render_profile_summary(&p.summary))
            .unwrap_or_else(|| "_(no researcher summary available for this entity)_".to_string());
        body.push_str(&summary_text);
        body.push_str("\n\n");
    }

    body
}

/// Labels that begin known researcher-summary boilerplate lines (the woven
/// mission brief, progress counters). Criterion cells and profile snippets
/// must skip these so the cells hold entity attributes, not brief prose.
const RESEARCHER_BOILERPLATE_LABELS: [&str; 9] = [
    "Mission:",
    "Approach:",
    "Output expectation:",
    "Scope note:",
    "Audience:",
    "Success criteria:",
    "Progress:",
    "Scope:",
    "Model:",
];

/// Whether a summary line is mission-brief boilerplate rather than content.
fn is_researcher_boilerplate(line: &str) -> bool {
    let t = line.trim().trim_start_matches('*');
    RESEARCHER_BOILERPLATE_LABELS
        .iter()
        .any(|label| t.starts_with(label))
}

/// A summary prepared once per profile for criterion-cell extraction: the
/// content body with headings and researcher boilerplate removed, plus its
/// lowercased copy for case-insensitive keyword search.
struct PreparedSummary {
    /// Filtered body (heading-style and boilerplate lines removed).
    body: String,
    /// Lowercased copy of `body` for keyword search.
    lower: String,
}

/// Filter heading-style and researcher-boilerplate lines out of a summary,
/// returning the content body and its lowercased copy.
fn prepare_summary(summary: &str) -> PreparedSummary {
    let body = summary
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && !is_researcher_boilerplate(l))
        .collect::<Vec<_>>()
        .join("\n");
    let lower = body.to_lowercase();
    PreparedSummary { body, lower }
}

/// Truncate `s` to at most `max` characters at a char boundary, appending an
/// ellipsis when the text was cut.
fn truncate_cell(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", s[..s.floor_char_boundary(max)].trim_end())
    } else {
        s.to_string()
    }
}

/// Extract a short cell value for `criterion` from a prepared summary.
///
/// The heuristic searches for the criterion keyword and returns the sentence
/// or phrase that contains it, truncated to keep table cells readable. When
/// the criterion is not mentioned, returns `"—"`.
fn criterion_cell(prepared: &PreparedSummary, criterion: &str) -> String {
    if prepared.body.is_empty() {
        // Every line was researcher boilerplate or a heading: there is no
        // profile content to quote, and falling back to the raw summary
        // would put mission-brief prose in the cell.
        return "—".to_string();
    }
    let lower_criterion = criterion.to_lowercase();
    let keyword = lower_criterion
        .split_whitespace()
        .next()
        .unwrap_or(&lower_criterion);

    let Some(idx) = prepared.lower.find(keyword) else {
        return "—".to_string();
    };

    // The end-of-sentence scan begins AFTER the keyword so a criterion
    // containing a '.' (e.g. "Node.js support") does not terminate the
    // snippet mid-keyword.
    let from = (idx + keyword.len()).min(prepared.body.len());
    // Byte offsets found in the lowercased text are only valid for slicing
    // the original text when the lowercase mapping preserved every
    // character's UTF-8 length; guard the boundaries before slicing.
    if prepared.body.is_char_boundary(idx) && prepared.body.is_char_boundary(from) {
        // Find the start of the sentence/line containing the keyword.
        let start = prepared.body[..idx]
            .rfind(['\n', '.', ';'])
            .map(|i| i + 1)
            .unwrap_or(0);
        let end = prepared.body[from..]
            .find(['\n', '.', ';'])
            .map(|i| from + i + 1)
            .unwrap_or(prepared.body.len());
        truncate_cell(prepared.body[start..end].trim(), 120)
    } else {
        // A character changed byte length under `to_lowercase` (e.g. U+0130),
        // so the lowercased offsets cannot be mapped back — degrade to the
        // first content line containing the keyword instead of risking a
        // mid-character slice panic.
        let line = prepared
            .body
            .lines()
            .find(|l| l.to_lowercase().contains(keyword))
            .unwrap_or("—");
        truncate_cell(line.trim(), 120)
    }
}

/// Return a compact one-line profile summary for the table's Profile column.
/// Heading-style lines (e.g. the researcher header `# Researcher …`) and
/// mission-brief boilerplate lines are skipped so the cell shows actual
/// profile content.
fn compact_profile_summary(summary: &str) -> String {
    let first_line = summary
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#') && !is_researcher_boilerplate(l))
        .unwrap_or(summary.lines().next().unwrap_or(summary).trim());
    truncate_cell(first_line, 140)
}

/// Render a researcher summary under an entity's `### {Entity}` subsection.
///
/// The researcher boilerplate header (`# Researcher researcher-N: ...`) is
/// dropped and every remaining heading inside the summary is demoted to
/// `####` so the summary's internal sections (`Summary`, `Findings`, ...)
/// nest below the entity heading instead of escaping the document outline.
/// Newlines are preserved: flattening them (the previous behaviour) fused
/// the whole profile into a single paragraph that inherited the header's
/// heading style, which made the section unreadable.
fn render_profile_summary(summary: &str) -> String {
    let mut rendered = String::new();
    for line in summary.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            let heading_text = trimmed.trim_start_matches('#').trim();
            if heading_text.starts_with("Researcher ") {
                continue;
            }
            rendered.push_str("#### ");
            rendered.push_str(heading_text);
            rendered.push('\n');
        } else {
            rendered.push_str(line);
            rendered.push('\n');
        }
    }
    rendered.trim().to_string()
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
        assert_eq!(body, "");
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

    #[test]
    fn compact_profile_summary_skips_researcher_header() {
        let summary = "# Researcher researcher-2: Research Together.ai for 'topic'\n\n\
                       Together AI competes on catalog breadth and workflow depth.";
        let cell = compact_profile_summary(summary);
        assert!(cell.starts_with("Together AI competes"));
        assert!(!cell.contains("Researcher"));
    }

    #[test]
    fn criterion_cell_skips_researcher_header() {
        let summary = "# Researcher researcher-3: Research Groq for 'topic'\n\n\
                       Groq delivers the fastest LLM inference via custom silicon.";
        let prepared = prepare_summary(summary);
        let cell = criterion_cell(&prepared, "LLM inference");
        assert!(cell.contains("fastest LLM inference"));
        assert!(!cell.contains("Researcher"));
    }

    #[test]
    fn dotted_criterion_does_not_truncate_mid_keyword() {
        // The end-of-sentence scan must not stop at the '.' inside the
        // matched keyword itself.
        let entities = vec![entity("Acme")];
        let criteria = vec!["Node.js support".to_string()];
        let profiles = vec![CompetitiveProfile::new(
            &entities[0],
            "Acme ships first-class Node.js support with a native SDK.",
        )];
        let body = build_comparison_table_body(&entities, &criteria, &profiles);
        assert!(body.contains("Node.js support with a native SDK"), "{body}");
    }

    #[test]
    fn length_changing_lowercase_does_not_panic() {
        // U+0130 (İ) lowercases to a 3-byte sequence, shifting every later
        // byte offset between the lowercased probe text and the original
        // summary; the non-ASCII keyword can then land mid-character in the
        // original. Must degrade gracefully instead of panicking.
        let entities = vec![entity("Test")];
        let criteria = vec!["Ω".to_string()];
        let profiles = vec![CompetitiveProfile::new(&entities[0], "İΩ pricing cheap.")];
        let body = build_comparison_table_body(&entities, &criteria, &profiles);
        assert!(
            body.contains("Ω pricing cheap.") || body.contains("| — |"),
            "{body}"
        );
    }

    #[test]
    fn entity_profile_preserves_newlines_and_drops_researcher_header() {
        let entities = vec![entity("Groq")];
        let criteria = vec!["speed".to_string()];
        let summary = "# Researcher researcher-3: Research Groq for 'topic'\n\n\
                       ## Summary\n\nGroq is the fastest open-model inference provider.\n\n\
                       ## Findings\n\n- LPU silicon delivers 750 tok/s on Llama 3 8B.";
        let profiles = vec![CompetitiveProfile::new(&entities[0], summary)];
        let body = build_comparison_table_body(&entities, &criteria, &profiles);
        let idx = body.find("## Entity Profiles").expect("profiles section");
        let section = &body[idx..];
        assert!(!section.contains("Researcher"), "{section}");
        assert!(
            section.contains("#### Summary\n\nGroq is the fastest"),
            "{section}"
        );
        assert!(
            section.contains("#### Findings\n\n- LPU silicon delivers"),
            "{section}"
        );
    }

    #[test]
    fn entity_profile_empty_summary_renders_placeholder() {
        let entities = vec![entity("Groq")];
        let profiles = vec![CompetitiveProfile::new(&entities[0], "")];
        let body = build_comparison_table_body(&entities, &["speed".to_string()], &profiles);
        assert!(body.contains("_(no researcher summary available for this entity)_"));
        assert!(
            body.contains("| — |"),
            "empty profile cell must render an em dash"
        );
    }
}
