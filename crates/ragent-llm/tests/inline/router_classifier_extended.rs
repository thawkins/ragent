//! Extended classifier tests (M8/T8.1).

use super::*;
use crate::providers::router_config::{BoundaryConfig, WeightConfig};

fn default_weights() -> WeightConfig {
    WeightConfig::default()
}

fn default_boundaries() -> BoundaryConfig {
    BoundaryConfig::default()
}

fn no_attachments() -> AttachmentInfo {
    AttachmentInfo::default()
}

// ── classify_safe fallback tests (FR-039) ────────────────────────────

#[test]
fn test_classify_safe_normal_input() {
    let result = PromptClassifier::classify_safe(
        "Explain async programming",
        None,
        &default_weights(),
        &default_boundaries(),
        &no_attachments(),
    );
    assert!(
        !result.composite_score.is_nan(),
        "safe classify should not return NaN"
    );
    assert!(
        !result.composite_score.is_infinite(),
        "safe classify should not return Inf"
    );
}

#[test]
fn test_classify_safe_empty_prompt() {
    let result = PromptClassifier::classify_safe(
        "",
        None,
        &default_weights(),
        &default_boundaries(),
        &no_attachments(),
    );
    // Empty prompt should not panic; it should produce a valid result
    assert_eq!(result.tier, Tier::Simple, "empty prompt should be SIMPLE");
}

#[test]
fn test_classify_safe_unicode_prompt() {
    let result = PromptClassifier::classify_safe(
        "请解释递归算法的原理，并证明其正确性",
        None,
        &default_weights(),
        &default_boundaries(),
        &no_attachments(),
    );
    assert!(
        !result.composite_score.is_nan(),
        "unicode prompt should not produce NaN"
    );
}

// ── Boundary edge cases (FR-006–FR-009) ──────────────────────────────

#[test]
fn test_select_tier_exact_boundary_simple_medium() {
    let boundaries = BoundaryConfig::default();
    // Score exactly at simple_medium boundary should be MEDIUM (>=)
    let tier = PromptClassifier::select_tier(boundaries.simple_medium, &boundaries);
    assert_eq!(
        tier,
        Tier::Medium,
        "score at simple_medium should be MEDIUM"
    );
}

#[test]
fn test_select_tier_just_below_simple_medium() {
    let boundaries = BoundaryConfig::default();
    let just_below = boundaries.simple_medium - 0.001;
    let tier = PromptClassifier::select_tier(just_below, &boundaries);
    assert_eq!(
        tier,
        Tier::Simple,
        "score just below simple_medium should be SIMPLE"
    );
}

#[test]
fn test_select_tier_exact_boundary_medium_complex() {
    let boundaries = BoundaryConfig::default();
    let tier = PromptClassifier::select_tier(boundaries.medium_complex, &boundaries);
    assert_eq!(
        tier,
        Tier::Complex,
        "score at medium_complex should be COMPLEX"
    );
}

#[test]
fn test_select_tier_exact_boundary_complex_reasoning() {
    let boundaries = BoundaryConfig::default();
    let tier = PromptClassifier::select_tier(boundaries.complex_reasoning, &boundaries);
    assert_eq!(
        tier,
        Tier::Reasoning,
        "score at complex_reasoning should be REASONING"
    );
}

#[test]
fn test_select_tier_custom_boundaries() {
    let boundaries = BoundaryConfig {
        simple_medium: 0.1,
        medium_complex: 0.4,
        complex_reasoning: 0.8,
    };
    assert_eq!(
        PromptClassifier::select_tier(0.05, &boundaries),
        Tier::Simple
    );
    assert_eq!(
        PromptClassifier::select_tier(0.1, &boundaries),
        Tier::Medium
    );
    assert_eq!(
        PromptClassifier::select_tier(0.4, &boundaries),
        Tier::Complex
    );
    assert_eq!(
        PromptClassifier::select_tier(0.8, &boundaries),
        Tier::Reasoning
    );
}

#[test]
fn test_select_tier_zero_score() {
    let boundaries = BoundaryConfig::default();
    assert_eq!(
        PromptClassifier::select_tier(0.0, &boundaries),
        Tier::Simple
    );
}

#[test]
fn test_select_tier_max_score() {
    let boundaries = BoundaryConfig::default();
    assert_eq!(
        PromptClassifier::select_tier(1.0, &boundaries),
        Tier::Reasoning
    );
}

// ── Custom weights (FR-010, FR-024) ──────────────────────────────────

#[test]
fn test_classify_with_custom_weights() {
    let mut weights = WeightConfig {
        reasoning_depth: 0.5,
        ..WeightConfig::default()
    };
    weights.normalise();

    let result = PromptClassifier::classify(
        "Prove that the algorithm terminates. Therefore deduce the conclusion.",
        None,
        &weights,
        &default_boundaries(),
        &no_attachments(),
    );
    // With heavy reasoning weight, this should score higher
    assert!(
        result.composite_score > 0.0,
        "prompt with reasoning keywords should score > 0 with custom weights"
    );
}

#[test]
fn test_classify_with_zeroed_weights() {
    let weights = WeightConfig {
        token_count: 0.0,
        vocabulary_complexity: 0.0,
        syntax_complexity: 0.0,
        domain_specificity: 0.0,
        ambiguity: 0.0,
        context_dependency: 0.0,
        reasoning_depth: 1.0, // only this one matters
        creativity_level: 0.0,
        emotional_complexity: 0.0,
        multimodality: 0.0,
        instruction_complexity: 0.0,
        knowledge_recency: 0.0,
        code_complexity: 0.0,
        mathematical_complexity: 0.0,
        image_attachment: 0.0,
    };
    let result = PromptClassifier::classify(
        "What is 2+2?",
        None,
        &weights,
        &default_boundaries(),
        &no_attachments(),
    );
    // Simple question should have near-zero reasoning depth
    assert!(
        result.composite_score < 0.3,
        "simple question with only-reasoning weight should score low, got {}",
        result.composite_score,
    );
}

// ── Public convenience wrappers ──────────────────────────────────────

#[test]
fn test_public_convenience_wrappers_match_lower() {
    let prompt = "The patient presents with clinical pathology";
    let lower = prompt.to_lowercase();
    assert_eq!(
        PromptClassifier::score_domain_specificity(prompt),
        PromptClassifier::score_domain_specificity_lower(&lower),
        "convenience wrapper should match _lower variant"
    );
}

#[test]
fn test_ambiguity_convenience_matches_lower() {
    let prompt = "Could you explore what if we brainstorm?";
    let lower = prompt.to_lowercase();
    assert_eq!(
        PromptClassifier::score_ambiguity(prompt),
        PromptClassifier::score_ambiguity_lower(&lower),
    );
}

#[test]
fn test_reasoning_depth_convenience_matches_lower() {
    let prompt = "Prove that therefore x implies y";
    let lower = prompt.to_lowercase();
    assert_eq!(
        PromptClassifier::score_reasoning_depth(prompt),
        PromptClassifier::score_reasoning_depth_lower(&lower),
    );
}

#[test]
fn test_creativity_level_convenience_matches_lower() {
    let prompt = "Imagine and design a creative fictional story";
    let lower = prompt.to_lowercase();
    assert_eq!(
        PromptClassifier::score_creativity_level(prompt),
        PromptClassifier::score_creativity_level_lower(&lower),
    );
}

#[test]
fn test_emotional_complexity_convenience_matches_lower() {
    let prompt = "Handle this with empathy and nuance";
    let lower = prompt.to_lowercase();
    assert_eq!(
        PromptClassifier::score_emotional_complexity(prompt),
        PromptClassifier::score_emotional_complexity_lower(&lower),
    );
}

#[test]
fn test_multimodality_convenience_matches_lower() {
    let prompt = "Analyse this image and screenshot";
    let lower = prompt.to_lowercase();
    assert_eq!(
        PromptClassifier::score_multimodality(prompt),
        PromptClassifier::score_multimodality_lower(&lower),
    );
}

#[test]
fn test_knowledge_recency_convenience_matches_lower() {
    let prompt = "What are the latest news in 2025?";
    let lower = prompt.to_lowercase();
    assert_eq!(
        PromptClassifier::score_knowledge_recency(prompt),
        PromptClassifier::score_knowledge_recency_lower(&lower),
    );
}

#[test]
fn test_mathematical_complexity_convenience_matches_lower() {
    let prompt = "Prove the theorem using \\frac and \\sum";
    let lower = prompt.to_lowercase();
    assert_eq!(
        PromptClassifier::score_mathematical_complexity(prompt),
        PromptClassifier::score_mathematical_complexity_lower(&lower),
    );
}

// ── Composite score robustness ───────────────────────────────────────

#[test]
fn test_composite_all_zeros() {
    let scores = [0.0; 15];
    let composite = PromptClassifier::compute_composite(&scores, &default_weights());
    assert_eq!(composite, 0.0, "all-zero scores should give composite 0");
}

#[test]
fn test_composite_all_ones() {
    let scores = [1.0; 15];
    let composite = PromptClassifier::compute_composite(&scores, &default_weights());
    // All dimensions active, density boost applies
    assert!(
        composite > 0.9,
        "all-ones scores should give high composite, got {composite}"
    );
    assert!(composite <= 1.0, "composite should be clamped to 1.0");
}

#[test]
fn test_dimension_scores_in_range() {
    let prompt = "Write a complex function that analyses medical data using \
                      mathematical proofs and screenshots, requiring deep reasoning";
    let scores = PromptClassifier::score_all_dimensions(prompt, None, &no_attachments());
    for (i, &score) in scores.iter().enumerate() {
        assert!(
            (0.0..=1.0).contains(&score),
            "dimension {} score {} should be in [0, 1]",
            dimension_name(i),
            score,
        );
    }
}

// ── NFR-004: Determinism across 100 iterations ────────���──────────────

#[test]
fn test_classify_deterministic_100_iterations() {
    let prompt = "Analyse the observer pattern in software engineering with \
                      concurrent microservices and async middleware";
    let first = PromptClassifier::classify(
        prompt,
        None,
        &default_weights(),
        &default_boundaries(),
        &no_attachments(),
    );
    for _ in 0..99 {
        let result = PromptClassifier::classify(
            prompt,
            None,
            &default_weights(),
            &default_boundaries(),
            &no_attachments(),
        );
        assert_eq!(
            result.dimension_scores, first.dimension_scores,
            "NFR-004: classifier must be deterministic"
        );
        assert_eq!(
            result.composite_score, first.composite_score,
            "NFR-004: composite must be deterministic"
        );
        assert_eq!(
            result.tier, first.tier,
            "NFR-004: tier must be deterministic"
        );
    }
}

// ── Context-aware classification ──────────────────────────────────────

#[test]
fn test_classify_with_history_increases_context_dependency() {
    let prompt = "Can you change it like we discussed earlier?";
    let no_history = PromptClassifier::classify(
        prompt,
        None,
        &default_weights(),
        &default_boundaries(),
        &no_attachments(),
    );
    let with_history = PromptClassifier::classify(
        prompt,
        Some("We previously discussed the architecture"),
        &default_weights(),
        &default_boundaries(),
        &no_attachments(),
    );
    // With history, context dependency score should be higher
    assert!(
        with_history.dimension_scores[5] >= no_history.dimension_scores[5],
        "history should increase context dependency score (dim 5)"
    );
}

// ── Image attachment and vision routing ─────────────────────��────────

#[test]
fn test_classify_with_image_sets_requires_vision() {
    let attachments = AttachmentInfo::new(2, 0);
    let result = PromptClassifier::classify(
        "Describe what you see",
        None,
        &default_weights(),
        &default_boundaries(),
        &attachments,
    );
    assert!(
        result.requires_vision,
        "image attachment should set requires_vision"
    );
    assert_eq!(
        result.dimension_scores[14], 1.0,
        "2 images should score 1.0 on dim 14"
    );
}

#[test]
fn test_classify_with_video_sets_requires_vision() {
    let attachments = AttachmentInfo::new(0, 1);
    let result = PromptClassifier::classify(
        "Describe this video",
        None,
        &default_weights(),
        &default_boundaries(),
        &attachments,
    );
    assert!(
        result.requires_vision,
        "video attachment should set requires_vision"
    );
    assert_eq!(
        result.dimension_scores[14], 1.0,
        "video should score 1.0 on dim 14"
    );
}

#[test]
fn test_classify_safe_preserves_requires_vision_on_error() {
    let attachments = AttachmentInfo::new(1, 0);
    // Even on fallback, requires_vision should be preserved
    let result = PromptClassifier::classify_safe(
        "",
        None,
        &default_weights(),
        &default_boundaries(),
        &attachments,
    );
    assert!(
        result.requires_vision,
        "requires_vision should survive classify_safe fallback"
    );
}
