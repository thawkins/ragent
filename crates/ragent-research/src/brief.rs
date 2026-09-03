//! Research-brief generator for `/research` (FR-004, FR-009).
//!
//! [`generate_research_brief`] turns a free-form user topic into a first-person
//! mission statement that downstream agents use as their guide. The generator is
//! deterministic and does not require an LLM call: it inspects the topic for
//! named entities, comparison keywords, question words, and the requested output
//! format, then composes a concise but concrete brief.
//!
//! The brief is used in two places:
//!
//! 1. As a replacement for the raw topic in the synthesis prompt preamble.
//! 2. As a `## Research Brief` section in the assembled `RESEARCH.md`.

use crate::run_config::{OutputFormat, ResearchMode};

/// Generate a first-person research brief from the user's prompt.
///
/// The brief captures the mission, scope, success criteria, intended audience,
/// and output expectations. It is intentionally generated without an LLM so it
/// can be produced before any expensive web searches or model calls.
///
/// When `mode` is [`ResearchMode::Supervisor`] or [`ResearchMode::Competitive`],
/// the brief explicitly calls out parallel sub-researchers or entity comparison.
/// When `format` is [`OutputFormat::ComparisonTable`], the brief asks for a
/// structured comparison table.
#[must_use]
pub fn generate_research_brief(
    topic: &str,
    mode: Option<ResearchMode>,
    format: Option<OutputFormat>,
) -> String {
    let trimmed = topic.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let lower = trimmed.to_lowercase();

    // Detect comparison / competitive intent.
    let comparison_words = [
        "compare",
        "versus",
        "vs",
        "difference between",
        "competitor",
    ];
    let is_comparison = comparison_words.iter().any(|w| lower.contains(w));

    // Detect explicit question / explain intent.
    let question_words = [
        "what is",
        "how does",
        "why",
        "explain",
        "overview of",
        "tell me about",
    ];
    let is_question = question_words
        .iter()
        .any(|w| lower.starts_with(w) || lower.contains(w));

    // Extract likely named entities (capitalized words/phrases) for scope.
    let entities: Vec<String> = extract_entities(trimmed);

    let mode_phrase = match mode {
        Some(ResearchMode::Supervisor) => {
            "Use a supervisor/researcher graph: delegate independent sub-topics to parallel researchers, then synthesize their compressed findings into one coherent report."
        }
        Some(ResearchMode::Competitive) => {
            "Use the competitive-analysis mode: decompose the topic into comparable entities, run one parallel researcher per entity, and synthesize per-entity profiles plus a cross-entity comparison table."
        }
        _ if is_comparison => {
            "Structure the investigation as a comparison across the named entities, with per-entity profiles and a cross-entity comparison table when possible."
        }
        _ => {
            "Follow the standard tiered research pipeline unless a multi-agent mode is explicitly configured."
        }
    };

    let format_phrase = match format {
        Some(OutputFormat::ExecutiveSummary) => {
            "Produce a concise one-page executive summary suitable for decision-makers."
        }
        Some(OutputFormat::ComparisonTable) => {
            "Deliver a focused artifact containing per-entity profiles and a Markdown comparison table with explicit comparison criteria."
        }
        Some(OutputFormat::SourceBibliography) => {
            "Produce an annotated bibliography that summarizes each major source's contribution."
        }
        Some(OutputFormat::Imrad) => {
            "Structure the final report in IMRaD form: Abstract, Introduction, Methods, Results, Discussion, and References Index."
        }
        _ => {
            "Produce a full multi-section research report with findings, implications, open questions, and a references index."
        }
    };

    // Lower the threshold to 1+ entities so single proper nouns like "Groq"
    // or "Rust" are surfaced, while still joining multi-word names.
    let entity_clause = if !entities.is_empty() {
        format!(
            "Key entities to cover: {}.",
            entities[..entities.len().min(5)].join(", ")
        )
    } else {
        String::new()
    };

    let scope_clause = if is_question {
        format!(
            "I need a well-grounded explanation of '{trimmed}' that defines the subject, explains how it works in practice, and identifies its main trade-offs and implications."
        )
    } else {
        format!(
            "I need to thoroughly investigate '{trimmed}', gathering concrete evidence from web sources and any in-project material so I can draw reliable conclusions."
        )
    };

    let audience = if is_comparison || format == Some(OutputFormat::ComparisonTable) {
        "The audience is someone choosing between options or evaluating competitors, so prioritize actionable contrasts, criteria, and caveats."
    } else if is_question {
        "The audience wants a clear, evidence-based primer, so prioritize accuracy, concrete examples, and practical implications."
    } else {
        "The audience wants a rigorous, well-cited research report that can be used for technical or strategic decisions."
    };

    let mut brief = format!(
        "**Mission:** {scope}\n\n\
         **Approach:** {mode}\n\n\
         **Output expectation:** {fmt}\n\n",
        scope = scope_clause,
        mode = mode_phrase,
        fmt = format_phrase,
    );

    if !entity_clause.is_empty() {
        brief.push_str(&format!("**Scope note:** {entity_clause}\n\n"));
    }

    brief.push_str(&format!(
        "**Audience:** {audience}\n\n\
         **Success criteria:** every finding cites at least one captured source using `[#N]`; conflicting evidence is noted explicitly; open questions and limitations are surfaced honestly.",
        audience = audience
    ));

    brief
}

/// Extract capitalized proper-noun-like phrases from a topic.
///
/// This is intentionally lightweight. It collects runs of words that start with
/// an uppercase letter, ignoring common stop words that sometimes appear
/// capitalized mid-sentence.
fn extract_entities(topic: &str) -> Vec<String> {
    let stop = [
        "The", "A", "An", "This", "That", "These", "Those", "And", "Or", "But", "For", "With",
        "In", "On", "At", "To", "Of", "From", "By", "Is", "Are", "Was", "Were", "Be", "Been", "It",
        "Its", "They", "Their", "We", "Our", "You", "Your", "I", "My",
    ];
    let stop_set: std::collections::HashSet<&str> = stop.iter().copied().collect();

    let mut entities = Vec::new();
    let mut current: Vec<String> = Vec::new();

    for word in topic.split_whitespace() {
        let stripped = word
            .trim_start_matches('(')
            .trim_end_matches(|c: char| c.is_ascii_punctuation() || c == ')')
            .to_string();
        if stripped.is_empty() {
            continue;
        }
        // Split joined tokens like "Together.ai" into separate words so each
        // capitalized fragment can count as its own entity token.
        let parts: Vec<String> = stripped
            .split(['.', '-', '\u{0026}'])
            .map(|s| s.to_string())
            .collect();
        for part in parts {
            if part.is_empty() {
                continue;
            }
            if part
                .chars()
                .next()
                .map(|c| c.is_ascii_uppercase())
                .unwrap_or(false)
                && !stop_set.contains(part.as_str())
            {
                current.push(part);
            } else {
                if current.len() >= 2 {
                    entities.push(current.join(" "));
                }
                current.clear();
            }
        }
    }
    if current.len() >= 2 {
        entities.push(current.join(" "));
    }

    // Deduplicate while preserving order.
    let mut seen = std::collections::HashSet::new();
    entities
        .into_iter()
        .filter(|e| seen.insert(e.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_topic_returns_empty_brief() {
        assert_eq!(generate_research_brief("   ", None, None), "");
    }

    #[test]
    fn brief_contains_mission_and_approach() {
        let brief = generate_research_brief("Rust async runtimes", None, None);
        assert!(brief.contains("**Mission:**"));
        assert!(brief.contains("Rust async runtimes"));
        assert!(brief.contains("**Approach:**"));
        assert!(brief.contains("**Output expectation:**"));
        assert!(brief.contains("**Success criteria:**"));
    }

    #[test]
    fn supervisor_mode_mentions_supervisor_graph() {
        let brief = generate_research_brief(
            "Explain Rust async runtimes",
            Some(ResearchMode::Supervisor),
            None,
        );
        assert!(brief.contains("supervisor/researcher graph"));
        assert!(brief.contains("parallel researchers"));
    }

    #[test]
    fn competitive_mode_mentions_entity_comparison() {
        let brief = generate_research_brief(
            "Compare Fireworks AI, Together.ai, and Groq for LLM inference",
            Some(ResearchMode::Competitive),
            None,
        );
        assert!(brief.contains("competitive-analysis mode"));
        assert!(brief.contains("comparison table"));
    }

    #[test]
    fn comparison_table_format_shortens_output() {
        let brief = generate_research_brief(
            "Compare Fireworks AI and Together.ai",
            Some(ResearchMode::Competitive),
            Some(OutputFormat::ComparisonTable),
        );
        assert!(brief.contains("per-entity profiles"));
        assert!(brief.contains("Markdown comparison table"));
    }

    #[test]
    fn extracts_entities_from_topic() {
        let brief = generate_research_brief(
            "Compare Fireworks AI, Together.ai, and Groq for LLM inference",
            None,
            None,
        );
        assert!(
            brief.contains("Key entities to cover:") || brief.contains("Scope note:"),
            "brief missing entity/scope clause:\n{brief}"
        );
        assert!(brief.contains("Fireworks AI"));
        assert!(brief.contains("Groq"));
    }
}
