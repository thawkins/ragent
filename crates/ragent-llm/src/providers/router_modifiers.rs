//! Prompt modifier detection and stripping for the Model Router.
//!
//! Detects three modifier formats (slash, bracket, word) that allow users to
//! explicitly override the classifier's tier decision. When a modifier is
//! detected, it is stripped from the prompt before forwarding to the LLM
//! (FR-020).
//!
//! Modifier aliases map to tiers per FR-019:
//! - `simple`, `basic`, `cheap` → SIMPLE
//! - `medium`, `balanced` → MEDIUM
//! - `complex`, `advanced` → COMPLEX
//! - `max`, `reasoning`, `think`, `deep` → REASONING

use super::router_config::Tier;

/// Result of modifier detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifierResult {
    /// The tier to route to (from the modifier).
    pub tier: Tier,
    /// The remaining prompt text with the modifier stripped.
    pub remaining_prompt: String,
}

/// Detect and strip a prompt modifier, returning the target tier and
/// the remaining prompt text.
///
/// Checks three modifier formats in order:
/// 1. Slash prefix: `/simple <prompt>`, `/max <prompt>`, etc.
/// 2. Bracket prefix: `[simple] <prompt>`, `[complex] <prompt>`, etc.
/// 3. Word prefix: `simple mode: <prompt>`, `deep mode: <prompt>`, etc.
///
/// Returns `None` if no modifier is detected (fall through to classifier).
pub fn detect_modifier(prompt: &str) -> Option<ModifierResult> {
    let trimmed = prompt.trim();

    // 1. Slash prefix: /simple <prompt>, /max <prompt>, etc.
    if let Some(rest) = trimmed.strip_prefix('/')
        && let Some((alias, remaining)) = split_first_word(rest)
        && let Some(tier) = alias_to_tier(alias)
    {
        return Some(ModifierResult {
            tier,
            remaining_prompt: remaining.trim().to_string(),
        });
    }

    // 2. Bracket prefix: [simple] <prompt>, [complex] <prompt>, etc.
    if trimmed.starts_with('[')
        && let Some(close_bracket) = trimmed.find(']')
    {
        let alias = &trimmed[1..close_bracket];
        let remaining = &trimmed[close_bracket + 1..];
        if let Some(tier) = alias_to_tier(alias) {
            return Some(ModifierResult {
                tier,
                remaining_prompt: remaining.trim().to_string(),
            });
        }
    }

    // 3. Word prefix: simple mode: <prompt>, deep mode: <prompt>, etc.
    // Also handles "simple mode, <prompt>" and "deep mode <prompt>"
    if let Some((alias, rest)) = split_first_word(trimmed) {
        let rest_trimmed = rest.trim_start();
        let rest_lower = rest_trimmed.to_lowercase();
        if rest_lower.starts_with("mode") {
            // Strip "mode" and any following separator (colon, comma, space)
            let after_mode = &rest_trimmed["mode".len()..];
            let separator_stripped = after_mode.trim_start_matches([':', ',', ' ']).to_string();
            if let Some(tier) = alias_to_tier(alias) {
                return Some(ModifierResult {
                    tier,
                    remaining_prompt: separator_stripped,
                });
            }
        }
    }

    None
}

/// Map a modifier alias to a tier (FR-019).
fn alias_to_tier(alias: &str) -> Option<Tier> {
    let lower = alias.to_lowercase();
    match lower.as_str() {
        "simple" | "basic" | "cheap" => Some(Tier::Simple),
        "medium" | "balanced" => Some(Tier::Medium),
        "complex" | "advanced" => Some(Tier::Complex),
        "max" | "reasoning" | "think" | "deep" => Some(Tier::Reasoning),
        _ => None,
    }
}

/// Split a string at the first whitespace boundary, returning
/// (first_word, remainder).
fn split_first_word(s: &str) -> Option<(&str, &str)> {
    let trimmed = s.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(pos) = trimmed.find(char::is_whitespace) {
        Some((&trimmed[..pos], &trimmed[pos..]))
    } else {
        None
    }
}

#[cfg(test)]

#[cfg(test)]
#[path = "../../tests/inline/router_modifiers.rs"]
mod router_modifiers_tests;
