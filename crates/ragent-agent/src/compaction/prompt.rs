//! Summarisation prompt builder for OpenCode-derived context compaction.
//!
//! This module defines the Markdown template and the prompt assembly logic
//! used to ask the LLM for a compacted summary of conversation history.
//! It is a Rust port of the `SessionCompaction.buildPrompt` function in
//! `~/Projects/opencode/packages/core/src/session/compaction.ts`.
//!
//! # Template
//!
//! The LLM is asked to emit exactly the following Markdown structure:
//!
//! ```markdown
//! ## Objective
//! - [one or two brief sentences describing what the user is trying to accomplish]
//!
//! ## Important Details
//! - [constraints/preferences, decisions and why, important facts/assumptions, ...]
//!
//! ## Work State
//! ### Completed
//! - [...]
//!
//! ### Active
//! - [...]
//!
//! ### Blocked
//! - [...]
//!
//! ## Next Move
//! 1. [immediate concrete action]
//! 2. [next action if known]
//!
//! ## Relevant Files
//! - [file or directory path: why it matters]
//! ```
//!
//! When a previous compaction summary exists, the prompt asks the model to
//! update it rather than recreate it from scratch.

/// Default maximum tokens to request for a compaction summary.
///
/// Matches OpenCode's `SUMMARY_OUTPUT_TOKENS`.
pub const SUMMARY_OUTPUT_TOKENS: usize = 4_096;

/// The Markdown template the LLM must fill in. Keep the section order unchanged.
const SUMMARY_TEMPLATE: &str = r#"Output exactly the Markdown structure shown inside <template> and keep the section order unchanged. Do not include the <template> tags in your response.
<template>
## Objective
- [one or two brief sentences describing what the user is trying to accomplish]

## Important Details
- [constraints/preferences, decisions and why, important facts/assumptions, exact context needed to continue, or "(none)"]

## Work State
### Completed
- [finished work, verified facts, or changes made; otherwise "(none)"]

### Active
- [current work, partial changes, or investigation state; otherwise "(none)"]

### Blocked
- [blockers, failing commands, or unknowns; otherwise "(none)"]

## Next Move
1. [immediate concrete action, or "(none)"]
2. [next action if known, or "(none)"]

## Relevant Files
- [file or directory path: why it matters, or "(none)"]
</template>

Rules:
- Keep every section, even when empty.
- Use terse bullets, not prose paragraphs.
- Preserve exact file paths, symbols, commands, error strings, URLs, and identifiers when known.
- Do not mention the summary process or that context was compacted."#;

/// Build a compaction summarisation prompt.
///
/// When `previous_summary` is supplied, the prompt instructs the model to update
/// the existing summary by preserving still-true details, removing stale ones,
/// and merging in the new context. Otherwise it asks for a new anchored
/// summary.
///
/// # Arguments
///
/// * `previous_summary` — an existing compaction summary, if any.
/// * `context` — one or more context strings (typically the serialised recent
///   turns plus the head of the conversation to summarise).
#[must_use]
pub fn build_prompt(previous_summary: Option<&str>, context: &[&str]) -> String {
    let instruction = if let Some(prev) = previous_summary {
        format!(
            "Update the anchored summary below using the conversation history above.\n\
             Preserve still-true details, remove stale details, and merge in the new facts.\n\
             <previous-summary>\n{prev}\n</previous-summary>"
        )
    } else {
        "Create a new anchored summary from the conversation history.".to_string()
    };

    let mut parts = vec![instruction, SUMMARY_TEMPLATE.to_string()];
    parts.extend(context.iter().map(|s| s.to_string()));
    parts.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_new_summary() {
        let prompt = build_prompt(None, &["[User]: hello\n\n[Assistant]: hi"]);
        assert!(prompt.contains("Create a new anchored summary"));
        assert!(prompt.contains("## Objective"));
        assert!(prompt.contains("## Work State"));
        assert!(prompt.contains("[User]: hello"));
    }

    #[test]
    fn test_build_prompt_update_summary() {
        let prompt = build_prompt(Some("## Objective\n- Old goal"), &["[User]: new info"]);
        assert!(prompt.contains("Update the anchored summary"));
        assert!(prompt.contains("## Objective"));
        assert!(prompt.contains("<previous-summary>"));
        assert!(prompt.contains("[User]: new info"));
    }

    #[test]
    fn test_build_prompt_multiple_contexts() {
        let prompt = build_prompt(None, &["context A", "context B"]);
        let a_pos = prompt.find("context A").unwrap();
        let b_pos = prompt.find("context B").unwrap();
        assert!(b_pos > a_pos);
    }
}
