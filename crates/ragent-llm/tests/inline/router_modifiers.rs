//! Inline tests for router_modifiers (M8/T8.1).

use super::*;

// ── Slash prefix tests ─────────────────────────────────────────────

#[test]
fn test_slash_simple() {
    let result = detect_modifier("/simple hello world").unwrap();
    assert_eq!(result.tier, Tier::Simple);
    assert_eq!(result.remaining_prompt, "hello world");
}

#[test]
fn test_slash_medium() {
    let result = detect_modifier("/medium explain this").unwrap();
    assert_eq!(result.tier, Tier::Medium);
    assert_eq!(result.remaining_prompt, "explain this");
}

#[test]
fn test_slash_complex() {
    let result = detect_modifier("/complex refactor the code").unwrap();
    assert_eq!(result.tier, Tier::Complex);
    assert_eq!(result.remaining_prompt, "refactor the code");
}

#[test]
fn test_slash_max() {
    let result = detect_modifier("/max analyse this").unwrap();
    assert_eq!(result.tier, Tier::Reasoning);
    assert_eq!(result.remaining_prompt, "analyse this");
}

#[test]
fn test_slash_reasoning() {
    let result = detect_modifier("/reasoning prove the theorem").unwrap();
    assert_eq!(result.tier, Tier::Reasoning);
    assert_eq!(result.remaining_prompt, "prove the theorem");
}

#[test]
fn test_slash_think() {
    let result = detect_modifier("/think deeply").unwrap();
    assert_eq!(result.tier, Tier::Reasoning);
    assert_eq!(result.remaining_prompt, "deeply");
}

#[test]
fn test_slash_deep() {
    let result = detect_modifier("/deep analysis").unwrap();
    assert_eq!(result.tier, Tier::Reasoning);
    assert_eq!(result.remaining_prompt, "analysis");
}

#[test]
fn test_slash_basic() {
    let result = detect_modifier("/basic list files").unwrap();
    assert_eq!(result.tier, Tier::Simple);
    assert_eq!(result.remaining_prompt, "list files");
}

#[test]
fn test_slash_cheap() {
    let result = detect_modifier("/cheap what time").unwrap();
    assert_eq!(result.tier, Tier::Simple);
    assert_eq!(result.remaining_prompt, "what time");
}

#[test]
fn test_slash_balanced() {
    let result = detect_modifier("/balanced explain").unwrap();
    assert_eq!(result.tier, Tier::Medium);
    assert_eq!(result.remaining_prompt, "explain");
}

#[test]
fn test_slash_advanced() {
    let result = detect_modifier("/advanced optimise").unwrap();
    assert_eq!(result.tier, Tier::Complex);
    assert_eq!(result.remaining_prompt, "optimise");
}

// ── Bracket prefix tests ────────────────────────────────────────────

#[test]
fn test_bracket_simple() {
    let result = detect_modifier("[simple] refactor").unwrap();
    assert_eq!(result.tier, Tier::Simple);
    assert_eq!(result.remaining_prompt, "refactor");
}

#[test]
fn test_bracket_complex() {
    let result = detect_modifier("[complex] deep analysis").unwrap();
    assert_eq!(result.tier, Tier::Complex);
    assert_eq!(result.remaining_prompt, "deep analysis");
}

#[test]
fn test_bracket_cheap() {
    let result = detect_modifier("[cheap] list files").unwrap();
    assert_eq!(result.tier, Tier::Simple);
    assert_eq!(result.remaining_prompt, "list files");
}

#[test]
fn test_bracket_no_space() {
    let result = detect_modifier("[max]prove this").unwrap();
    assert_eq!(result.tier, Tier::Reasoning);
    assert_eq!(result.remaining_prompt, "prove this");
}

// ── Word prefix tests ──────────────────────────────────────────────

#[test]
fn test_word_deep_mode_colon() {
    let result = detect_modifier("deep mode: prove this").unwrap();
    assert_eq!(result.tier, Tier::Reasoning);
    assert_eq!(result.remaining_prompt, "prove this");
}

#[test]
fn test_word_basic_mode_comma() {
    let result = detect_modifier("basic mode, what time").unwrap();
    assert_eq!(result.tier, Tier::Simple);
    assert_eq!(result.remaining_prompt, "what time");
}

#[test]
fn test_word_simple_mode_colon() {
    let result = detect_modifier("simple mode: hello").unwrap();
    assert_eq!(result.tier, Tier::Simple);
    assert_eq!(result.remaining_prompt, "hello");
}

#[test]
fn test_word_complex_mode_colon() {
    let result = detect_modifier("complex mode: refactor").unwrap();
    assert_eq!(result.tier, Tier::Complex);
    assert_eq!(result.remaining_prompt, "refactor");
}

#[test]
fn test_word_reasoning_mode_colon() {
    let result = detect_modifier("reasoning mode: prove this").unwrap();
    assert_eq!(result.tier, Tier::Reasoning);
    assert_eq!(result.remaining_prompt, "prove this");
}

#[test]
fn test_word_medium_mode_colon() {
    let result = detect_modifier("medium mode: explain").unwrap();
    assert_eq!(result.tier, Tier::Medium);
    assert_eq!(result.remaining_prompt, "explain");
}

// ── No modifier (fall-through) tests ────────────────────────────────

#[test]
fn test_no_modifier_plain() {
    assert!(detect_modifier("What is 2+2?").is_none());
}

#[test]
fn test_no_modifier_question() {
    assert!(detect_modifier("How do I refactor this code?").is_none());
}

#[test]
fn test_no_modifier_slash_other() {
    // A slash command that isn't a router modifier
    assert!(detect_modifier("/help").is_none());
}

#[test]
fn test_no_modifier_bracket_other() {
    // A bracket that isn't a modifier
    assert!(detect_modifier("[note] something").is_none());
}

// ── Stripping tests ────────────────────────────────────────────────

#[test]
fn test_modifier_stripped_from_slash() {
    let result = detect_modifier("/simple hello world").unwrap();
    assert!(!result.remaining_prompt.contains("/simple"));
    assert_eq!(result.remaining_prompt, "hello world");
}

#[test]
fn test_modifier_stripped_from_bracket() {
    let result = detect_modifier("[complex] refactor").unwrap();
    assert!(!result.remaining_prompt.contains("[complex]"));
    assert_eq!(result.remaining_prompt, "refactor");
}

#[test]
fn test_modifier_stripped_from_word() {
    let result = detect_modifier("deep mode: prove this").unwrap();
    assert!(!result.remaining_prompt.contains("deep mode:"));
    assert_eq!(result.remaining_prompt, "prove this");
}

// ── Edge cases ──────────────────────────────────────────────────────

#[test]
fn test_slash_with_leading_whitespace() {
    let result = detect_modifier("  /simple hello").unwrap();
    assert_eq!(result.tier, Tier::Simple);
    assert_eq!(result.remaining_prompt, "hello");
}

#[test]
fn test_empty_remaining_after_strip() {
    // Modifier with no remaining prompt
    let result = detect_modifier("/simple").is_none();
    // "/simple" with no space after has no remaining word
    // split_first_word returns None because there's no whitespace
    assert!(result);
}

#[test]
fn test_alias_case_insensitive() {
    let result = detect_modifier("/SIMPLE hello").unwrap();
    assert_eq!(result.tier, Tier::Simple);
    assert_eq!(result.remaining_prompt, "hello");
}

#[test]
fn test_alias_mixed_case() {
    let result = detect_modifier("/Complex refactor").unwrap();
    assert_eq!(result.tier, Tier::Complex);
    assert_eq!(result.remaining_prompt, "refactor");
}

// ── Extended modifier detection tests (T-028) ───────────────────────────

// ── FR-016: Slash prefix modifier tests ──────────────────────────────

#[test]
fn test_slash_all_simple_aliases() {
    // FR-019: simple, basic, cheap → SIMPLE
    for alias in &["simple", "basic", "cheap"] {
        let prompt = format!("/{} hello world", alias);
        let result = detect_modifier(&prompt)
            .unwrap_or_else(|| panic!("/{} should be detected as modifier", alias));
        assert_eq!(result.tier, Tier::Simple, "/{} should map to SIMPLE", alias);
        assert_eq!(result.remaining_prompt, "hello world");
    }
}

#[test]
fn test_slash_all_medium_aliases() {
    // FR-019: medium, balanced → MEDIUM
    for alias in &["medium", "balanced"] {
        let prompt = format!("/{} explain this", alias);
        let result = detect_modifier(&prompt)
            .unwrap_or_else(|| panic!("/{} should be detected as modifier", alias));
        assert_eq!(result.tier, Tier::Medium, "/{} should map to MEDIUM", alias);
        assert_eq!(result.remaining_prompt, "explain this");
    }
}

#[test]
fn test_slash_all_complex_aliases() {
    // FR-019: complex, advanced → COMPLEX
    for alias in &["complex", "advanced"] {
        let prompt = format!("/{} deep analysis", alias);
        let result = detect_modifier(&prompt)
            .unwrap_or_else(|| panic!("/{} should be detected as modifier", alias));
        assert_eq!(
            result.tier,
            Tier::Complex,
            "/{} should map to COMPLEX",
            alias
        );
        assert_eq!(result.remaining_prompt, "deep analysis");
    }
}

#[test]
fn test_slash_all_reasoning_aliases() {
    // FR-019: max, reasoning, think, deep → REASONING
    for alias in &["max", "reasoning", "think", "deep"] {
        let prompt = format!("/{} prove theorem", alias);
        let result = detect_modifier(&prompt)
            .unwrap_or_else(|| panic!("/{} should be detected as modifier", alias));
        assert_eq!(
            result.tier,
            Tier::Reasoning,
            "/{} should map to REASONING",
            alias
        );
        assert_eq!(result.remaining_prompt, "prove theorem");
    }
}

#[test]
fn test_slash_case_insensitive_all() {
    // FR-016 modifiers are case-insensitive
    let cases = [
        ("/SIMPLE test", Tier::Simple),
        ("/Simple test", Tier::Simple),
        ("/MEDIUM test", Tier::Medium),
        ("/Medium test", Tier::Medium),
        ("/COMPLEX test", Tier::Complex),
        ("/Complex test", Tier::Complex),
        ("/REASONING test", Tier::Reasoning),
        ("/Reasoning test", Tier::Reasoning),
        ("/THINK test", Tier::Reasoning),
        ("/Think test", Tier::Reasoning),
        ("/DEEP test", Tier::Reasoning),
        ("/Deep test", Tier::Reasoning),
        ("/MAX test", Tier::Reasoning),
        ("/Max test", Tier::Reasoning),
    ];
    for (prompt, expected_tier) in cases {
        let result =
            detect_modifier(prompt).unwrap_or_else(|| panic!("'{}' should be detected", prompt));
        assert_eq!(result.tier, expected_tier, "'{}' tier mismatch", prompt);
    }
}

// ── FR-017: Bracket prefix modifier tests ────────────────────────────

#[test]
fn test_bracket_all_tiers() {
    let cases = [
        ("[simple] hello", Tier::Simple),
        ("[medium] hello", Tier::Medium),
        ("[complex] hello", Tier::Complex),
        ("[reasoning] hello", Tier::Reasoning),
        ("[basic] hello", Tier::Simple),
        ("[balanced] hello", Tier::Medium),
        ("[advanced] hello", Tier::Complex),
        ("[max] hello", Tier::Reasoning),
        ("[think] hello", Tier::Reasoning),
        ("[deep] hello", Tier::Reasoning),
        ("[cheap] hello", Tier::Simple),
    ];
    for (prompt, expected_tier) in cases {
        let result = detect_modifier(prompt)
            .unwrap_or_else(|| panic!("'{}' should be detected as bracket modifier", prompt));
        assert_eq!(result.tier, expected_tier, "'{}' tier mismatch", prompt);
    }
}

#[test]
fn test_bracket_case_insensitive() {
    let cases = [
        ("[SIMPLE] test", Tier::Simple),
        ("[Medium] test", Tier::Medium),
        ("[COMPLEX] test", Tier::Complex),
        ("[REASONING] test", Tier::Reasoning),
    ];
    for (prompt, expected_tier) in cases {
        let result =
            detect_modifier(prompt).unwrap_or_else(|| panic!("'{}' should be detected", prompt));
        assert_eq!(result.tier, expected_tier, "'{}' tier mismatch", prompt);
    }
}

#[test]
fn test_bracket_no_space_after() {
    // Bracket modifier immediately followed by text (no space)
    let result = detect_modifier("[simple]hello").unwrap();
    assert_eq!(result.tier, Tier::Simple);
    assert_eq!(result.remaining_prompt, "hello");
}

#[test]
fn test_bracket_with_spaces() {
    let result = detect_modifier("[complex]  multiple  spaces").unwrap();
    assert_eq!(result.tier, Tier::Complex);
    assert_eq!(result.remaining_prompt, "multiple  spaces");
}

// ── FR-018: Word prefix modifier tests ───────────────────────────────

#[test]
fn test_word_all_tiers_with_colon() {
    let cases = [
        ("simple mode: hello", Tier::Simple),
        ("medium mode: hello", Tier::Medium),
        ("complex mode: hello", Tier::Complex),
        ("reasoning mode: hello", Tier::Reasoning),
        ("deep mode: hello", Tier::Reasoning),
        ("basic mode: hello", Tier::Simple),
    ];
    for (prompt, expected_tier) in cases {
        let result = detect_modifier(prompt)
            .unwrap_or_else(|| panic!("'{}' should be detected as word modifier", prompt));
        assert_eq!(result.tier, expected_tier, "'{}' tier mismatch", prompt);
    }
}

#[test]
fn test_word_all_tiers_with_comma() {
    let cases = [
        ("simple mode, hello", Tier::Simple),
        ("deep mode, prove this", Tier::Reasoning),
        ("basic mode, test", Tier::Simple),
    ];
    for (prompt, expected_tier) in cases {
        let result = detect_modifier(prompt).unwrap_or_else(|| {
            panic!(
                "'{}' should be detected as word modifier with comma",
                prompt
            )
        });
        assert_eq!(result.tier, expected_tier, "'{}' tier mismatch", prompt);
    }
}

#[test]
fn test_word_case_insensitive() {
    let cases = [
        ("Simple mode: test", Tier::Simple),
        ("SIMPLE MODE: test", Tier::Simple),
        ("Deep Mode: test", Tier::Reasoning),
        ("REASONING MODE: test", Tier::Reasoning),
    ];
    for (prompt, expected_tier) in cases {
        let result =
            detect_modifier(prompt).unwrap_or_else(|| panic!("'{}' should be detected", prompt));
        assert_eq!(result.tier, expected_tier, "'{}' tier mismatch", prompt);
    }
}

// ── FR-020: Modifiers must be stripped ───────────────────────────────

#[test]
fn test_slash_modifier_fully_stripped() {
    let result = detect_modifier("/reasoning prove this theorem").unwrap();
    assert!(
        !result.remaining_prompt.contains("/reasoning"),
        "FR-020: slash modifier must be stripped"
    );
    assert!(!result.remaining_prompt.starts_with('/'));
}

#[test]
fn test_bracket_modifier_fully_stripped() {
    let result = detect_modifier("[max] solve equation").unwrap();
    assert!(
        !result.remaining_prompt.contains("[max]"),
        "FR-020: bracket modifier must be stripped"
    );
    assert!(!result.remaining_prompt.starts_with('['));
}

#[test]
fn test_word_modifier_fully_stripped() {
    let result = detect_modifier("deep mode: solve equation").unwrap();
    assert!(
        !result.remaining_prompt.contains("deep mode:"),
        "FR-020: word modifier must be stripped"
    );
    assert!(!result.remaining_prompt.starts_with("deep"));
}

// ── Non-modifier edge cases ──────────────────────────────────────────

#[test]
fn test_no_modifier_regular_slash() {
    assert!(detect_modifier("/help").is_none());
    assert!(detect_modifier("/agent coder").is_none());
    assert!(detect_modifier("/model gpt-4").is_none());
}

#[test]
fn test_no_modifier_text_containing_modifier_words() {
    // "simple" in the middle of text, not at the start
    assert!(detect_modifier("Explain this simple concept").is_none());
    assert!(detect_modifier("The deep ocean is vast").is_none());
}

#[test]
fn test_no_modifier_empty_bracket() {
    assert!(detect_modifier("[] hello").is_none());
}

#[test]
fn test_no_modifier_unclosed_bracket() {
    assert!(detect_modifier("[simple hello").is_none());
}

#[test]
fn test_modifier_result_fields() {
    let result = detect_modifier("/complex refactor this code").unwrap();
    assert_eq!(result.tier, Tier::Complex);
    assert_eq!(result.remaining_prompt, "refactor this code");
}
