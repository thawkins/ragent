//! Tests for the LLM report-title normalisation used by `/research create`.
//!
//! The `/research create` pipeline names the finished report from its
//! executive summary. Providers decorate a one-line answer in predictable
//! ways (`Title:` prefixes, quotes, markdown headings, trailing full stops,
//! follow-up commentary), so `clean_generated_title` normalises the reply
//! before it reaches the `title` frontmatter field and falls back to the
//! summary truncation when nothing usable comes back.

use ragent_research::analysis::clean_generated_title;
use ragent_research::item::RESEARCH_TITLE_MAX_CHARS;

#[test]
fn test_title_extraction_plain_line_is_kept() {
    assert_eq!(
        clean_generated_title("An AI Economy in 2026: Deflation, Hyperscaling and Unit Costs"),
        Some("An AI Economy in 2026: Deflation, Hyperscaling and Unit Costs".to_string())
    );
}

#[test]
fn test_title_extraction_strips_title_prefix() {
    assert_eq!(
        clean_generated_title("Title: AI Deflation Meets Hyperscaler Economics"),
        Some("AI Deflation Meets Hyperscaler Economics".to_string())
    );
    assert_eq!(
        clean_generated_title("report title: AI Deflation Meets Hyperscaler Economics"),
        Some("AI Deflation Meets Hyperscaler Economics".to_string())
    );
}

#[test]
fn test_title_extraction_strips_markdown_and_quotes() {
    assert_eq!(
        clean_generated_title("## \"AI Deflation and the Hyperscaling Bill\""),
        Some("AI Deflation and the Hyperscaling Bill".to_string())
    );
    assert_eq!(
        clean_generated_title("**AI Deflation and the Hyperscaling Bill.**"),
        Some("AI Deflation and the Hyperscaling Bill".to_string())
    );
}

#[test]
fn test_title_extraction_keeps_first_line_only() {
    assert_eq!(
        clean_generated_title(
            "AI Deflation and the Hyperscaling Bill\n\nThis title captures the report's focus."
        ),
        Some("AI Deflation and the Hyperscaling Bill".to_string())
    );
    assert_eq!(
        clean_generated_title("\n\n  AI Deflation and the Hyperscaling Bill  \n"),
        Some("AI Deflation and the Hyperscaling Bill".to_string())
    );
}

#[test]
fn test_title_extraction_caps_long_output_on_word_boundary() {
    let long = "AI Deflation Hyperscaling Economics Unit Costs Business Models and the \
                Structural Consequences of Falling Inference Prices Across Providers";
    let title = clean_generated_title(long).expect("usable title");
    assert!(title.chars().count() <= RESEARCH_TITLE_MAX_CHARS + 3);
    assert!(title.ends_with("..."));
    assert!(!title.contains("  "));
}

#[test]
fn test_title_extraction_rejects_unusable_output() {
    // Empty and whitespace-only replies carry no title.
    assert_eq!(clean_generated_title(""), None);
    assert_eq!(clean_generated_title("   \n\n  "), None);
    // A single word is a refusal or a stray marker, not a report label.
    assert_eq!(clean_generated_title("Denied"), None);
    // Punctuation only.
    assert_eq!(clean_generated_title("\"\" ..."), None);
    // A meta-reply whose "title" is an empty label.
    assert_eq!(clean_generated_title("Title:"), None);
}

#[test]
fn test_title_extraction_does_not_split_a_label_without_space() {
    // A colon that is part of the title itself survives: only a leading
    // `Title:` label is removed.
    assert_eq!(
        clean_generated_title("OpenAI vs Anthropic: The Unit-Cost Race"),
        Some("OpenAI vs Anthropic: The Unit-Cost Race".to_string())
    );
}
