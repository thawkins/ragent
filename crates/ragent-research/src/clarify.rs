//! Scope-clarification heuristic for `/research` (FR-005, FR-017).
//!
//! Detects ambiguous research topics and generates a single clarifying question
//! before any web searches are performed. The heuristic is intentionally
//! lightweight and deterministic so it can run without an LLM; later tasks
//! may layer an LLM-based ambiguity detector on top.

use regex::Regex;

/// Returns a single clarifying question when the topic looks ambiguous.
///
/// A topic is considered ambiguous when it is very short, lacks concrete
/// entities, or contains broad/vague phrases such as "research the market" or
/// "tell me about X". If the topic already names specific entities, time
/// periods, or comparison dimensions, no clarification is requested.
#[must_use]
pub fn needs_clarification(topic: &str) -> Option<String> {
    let trimmed = topic.trim();
    if trimmed.is_empty() {
        return Some(
            "What specific subject, product, or question should this research focus on?"
                .to_string(),
        );
    }

    let lower = trimmed.to_lowercase();

    // Very short topics without any named entity are likely ambiguous.
    if lower.split_whitespace().count() <= 2 {
        return Some(format!(
            "Could you narrow this down? For example, which aspect of '{trimmed}' should the research cover?"
        ));
    }

    // Broad/vague phrases that signal under-specified scope.
    let vague_phrases = [
        "research the",
        "research on",
        "tell me about",
        "explain",
        "what is",
        "overview of",
        "market",
        "latest trends in",
        "everything about",
        "all about",
    ];
    for phrase in &vague_phrases {
        if lower.contains(phrase) {
            return Some(format!(
                "This topic is a bit broad. What specific angle, time frame, or comparison should the research focus on for '{trimmed}'?"
            ));
        }
    }

    // If the topic contains numbers, years, or product/version identifiers,
    // treat it as sufficiently concrete.
    if Regex::new(r"\b(20\d{2}|19\d{2}|v\d+\.\d+|version \d+\.?d+)")
        .ok()?
        .is_match(&lower)
    {
        return None;
    }

    // If the topic names at least two proper-looking entities (case-sensitive)
    // or contains a comparison word plus entity, assume it is well-scoped.
    let proper_entity_count = trimmed
        .split_whitespace()
        .filter(|w| {
            w.chars()
                .next()
                .map(|c| c.is_ascii_uppercase())
                .unwrap_or(false)
        })
        .count();
    let comparison_words = ["compare", "versus", "vs", "difference between"];
    let is_comparison = comparison_words.iter().any(|w| lower.contains(w));
    if proper_entity_count >= 2 || (is_comparison && proper_entity_count >= 1) {
        return None;
    }

    // Default ambiguity fallback for mid-length but generic topics.
    Some(format!(
        "To make the research useful, what output, audience, or specific question should guide the investigation of '{trimmed}'?"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_topic_needs_clarification() {
        assert!(needs_clarification("").is_some());
    }

    #[test]
    fn short_topic_needs_clarification() {
        let q = needs_clarification("rust").unwrap();
        assert!(q.contains("narrow this down"));
    }

    #[test]
    fn broad_phrase_needs_clarification() {
        assert!(needs_clarification("research the inference market").is_some());
        assert!(needs_clarification("tell me about Rust").is_some());
    }

    #[test]
    fn specific_comparison_is_clear() {
        assert!(
            needs_clarification("Compare Fireworks AI and Together.ai for LLM inference").is_none()
        );
    }

    #[test]
    fn year_makes_topic_concrete() {
        assert!(needs_clarification("Rust async runtimes in 2024").is_none());
    }
}
