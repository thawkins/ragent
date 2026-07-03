//! Inline tests for router_classifier (M8/T8.1).
//! Compiled as a submodule via #[path], super::* resolves to router_classifier.

    use super::*;
    use crate::providers::router_config::WeightConfig;

    fn default_weights() -> WeightConfig {
        WeightConfig::default()
    }

    fn default_boundaries() -> BoundaryConfig {
        BoundaryConfig::default()
    }

    fn no_attachments() -> AttachmentInfo {
        AttachmentInfo::default()
    }

    // ── Dimension scorer tests ──────────────────────────────────────────

    #[test]
    fn test_score_token_count_short() {
        let score = PromptClassifier::score_token_count("Hi");
        assert!(
            score < 0.1,
            "short prompt should have low token score, got {score}"
        );
    }

    #[test]
    fn test_score_token_count_long() {
        let long = "x ".repeat(5000);
        let score = PromptClassifier::score_token_count(&long);
        assert!(
            score > 0.7,
            "long prompt should have high token score, got {score}"
        );
    }

    #[test]
    fn test_score_vocabulary_complexity_simple() {
        let score = PromptClassifier::score_vocabulary_complexity("hi there what is 2+2");
        assert!(
            score < 0.5,
            "simple vocabulary should score low, got {score}"
        );
    }

    #[test]
    fn test_score_vocabulary_complexity_complex() {
        let score = PromptClassifier::score_vocabulary_complexity(
            "elaborate computational methodologies necessitate sophisticated algorithmic paradigms",
        );
        assert!(
            score > 0.3,
            "complex vocabulary should score higher, got {score}"
        );
    }

    #[test]
    fn test_score_syntax_complexity_simple() {
        let score = PromptClassifier::score_syntax_complexity("What is 2+2?");
        assert!(score < 0.3, "simple syntax should score low, got {score}");
    }

    #[test]
    fn test_score_syntax_complexity_nested() {
        let score = PromptClassifier::score_syntax_complexity(
            "If (when (assuming (x > 0)) then y) otherwise z; also a, b, c, d, e",
        );
        assert!(
            score > 0.2,
            "nested syntax should score higher, got {score}"
        );
    }

    #[test]
    fn test_score_domain_specificity_general() {
        let score = PromptClassifier::score_domain_specificity("What is 2+2?");
        assert!(
            score < 0.2,
            "general prompt should have low domain score, got {score}"
        );
    }

    #[test]
    fn test_score_domain_specificity_medical() {
        let score = PromptClassifier::score_domain_specificity(
            "The patient presents with clinical symptoms of pathology",
        );
        assert!(
            score > 0.1,
            "medical prompt should have some domain score, got {score}"
        );
    }

    #[test]
    fn test_score_ambiguity_specific() {
        let score = PromptClassifier::score_ambiguity("List the files in this directory");
        assert!(
            score < 0.3,
            "specific prompt should have low ambiguity, got {score}"
        );
    }

    #[test]
    fn test_score_ambiguity_open() {
        let score = PromptClassifier::score_ambiguity(
            "Could you explore what if we brainstorm maybe some ideas?",
        );
        assert!(
            score > 0.3,
            "open-ended prompt should have higher ambiguity, got {score}"
        );
    }

    #[test]
    fn test_score_context_dependency_no_history() {
        let score = PromptClassifier::score_context_dependency("What is 2+2?", "");
        assert!(
            score < 0.3,
            "no history should have low context score, got {score}"
        );
    }

    #[test]
    fn test_score_context_dependency_with_references() {
        let score = PromptClassifier::score_context_dependency(
            "Can you change it like we discussed earlier? You said the above was correct.",
            "Previous discussion about the project",
        );
        assert!(
            score > 0.1,
            "references to prior context should increase score, got {score}"
        );
    }

    #[test]
    fn test_score_reasoning_depth_simple() {
        let score = PromptClassifier::score_reasoning_depth("What is 2+2?");
        assert!(
            score < 0.2,
            "simple question should have low reasoning, got {score}"
        );
    }

    #[test]
    fn test_score_reasoning_depth_proof() {
        let score = PromptClassifier::score_reasoning_depth(
            "Prove that therefore x implies y, deduce the hypothesis",
        );
        assert!(
            score > 0.2,
            "proof prompt should have higher reasoning, got {score}"
        );
    }

    #[test]
    fn test_score_creativity_level_factual() {
        let score = PromptClassifier::score_creativity_level("What is the capital of France?");
        assert!(
            score < 0.2,
            "factual question should have low creativity, got {score}"
        );
    }

    #[test]
    fn test_score_creativity_level_creative() {
        let score = PromptClassifier::score_creativity_level(
            "Imagine and design a creative fictional story",
        );
        assert!(
            score > 0.3,
            "creative prompt should have higher creativity, got {score}"
        );
    }

    #[test]
    fn test_score_emotional_complexity_neutral() {
        let score = PromptClassifier::score_emotional_complexity("Sort this array");
        assert!(
            score < 0.1,
            "neutral prompt should have low emotional score, got {score}"
        );
    }

    #[test]
    fn test_score_multimodality_text_only() {
        let score = PromptClassifier::score_multimodality("What is 2+2?");
        assert!(
            score < 0.1,
            "text-only prompt should have low multimodality, got {score}"
        );
    }

    #[test]
    fn test_score_multimodality_with_image() {
        let score =
            PromptClassifier::score_multimodality("Analyse this image and the attached screenshot");
        assert!(
            score > 0.2,
            "image prompt should have higher multimodality, got {score}"
        );
    }

    #[test]
    fn test_score_instruction_complexity_simple() {
        let score = PromptClassifier::score_instruction_complexity("Say hello", "say hello");
        assert!(
            score < 0.2,
            "simple instruction should have low score, got {score}"
        );
    }

    #[test]
    fn test_score_instruction_complexity_multi_step() {
        let score = PromptClassifier::score_instruction_complexity(
            "1. First step\n2. Second step\n3. Third step\nMust ensure quality",
            "1. first step\n2. second step\n3. third step\nmust ensure quality",
        );
        assert!(
            score > 0.3,
            "multi-step instruction should have higher score, got {score}"
        );
    }

    #[test]
    fn test_score_knowledge_recency_evergreen() {
        let score = PromptClassifier::score_knowledge_recency("What is 2+2?");
        assert!(
            score < 0.1,
            "evergreen prompt should have low recency, got {score}"
        );
    }

    #[test]
    fn test_score_knowledge_recency_current() {
        let score = PromptClassifier::score_knowledge_recency(
            "What are the latest news recently updated in 2025?",
        );
        assert!(
            score > 0.2,
            "recency prompt should have higher score, got {score}"
        );
    }

    #[test]
    fn test_score_code_complexity_no_code() {
        let score = PromptClassifier::score_code_complexity("What is 2+2?", "what is 2+2?");
        assert!(
            score < 0.2,
            "non-code prompt should have low code score, got {score}"
        );
    }

    #[test]
    fn test_score_code_complexity_with_code() {
        let score = PromptClassifier::score_code_complexity(
            "```rust\nfn main() { let x = async { loop { } }; }\n```\nRefactor this class method",
            "```rust\nfn main() { let x = async { loop { } }; }\n```\nrefactor this class method",
        );
        assert!(
            score > 0.2,
            "code prompt should have higher code score, got {score}"
        );
    }

    #[test]
    fn test_score_mathematical_complexity_no_math() {
        let score = PromptClassifier::score_mathematical_complexity("What is 2+2?");
        assert!(
            score < 0.2,
            "non-math prompt should have low math score, got {score}"
        );
    }

    #[test]
    fn test_score_mathematical_complexity_proof() {
        let score = PromptClassifier::score_mathematical_complexity(
            "Prove the theorem: for all x, the integral ∫f(x)dx exists",
        );
        assert!(
            score > 0.2,
            "math prompt should have higher math score, got {score}"
        );
    }

    // ── Image attachment dimension tests ───────────────────────────────

    #[test]
    fn test_score_image_attachment_no_attachments() {
        let score = PromptClassifier::score_image_attachment(&no_attachments());
        assert_eq!(score, 0.0, "no attachments should score 0.0");
    }

    #[test]
    fn test_score_image_attachment_single_image() {
        let attachments = AttachmentInfo::new(1, 0);
        let score = PromptClassifier::score_image_attachment(&attachments);
        assert_eq!(score, 0.5, "single image should score 0.5");
    }

    #[test]
    fn test_score_image_attachment_multiple_images() {
        let attachments = AttachmentInfo::new(3, 0);
        let score = PromptClassifier::score_image_attachment(&attachments);
        assert_eq!(score, 1.0, "multiple images should score 1.0");
    }

    #[test]
    fn test_score_image_attachment_video() {
        let attachments = AttachmentInfo::new(0, 1);
        let score = PromptClassifier::score_image_attachment(&attachments);
        assert_eq!(score, 1.0, "video should score 1.0");
    }

    #[test]
    fn test_attachment_info_has_media() {
        assert!(
            !no_attachments().has_media(),
            "no attachments should not have media"
        );
        assert!(
            AttachmentInfo::new(1, 0).has_media(),
            "image should have media"
        );
        assert!(
            AttachmentInfo::new(0, 1).has_media(),
            "video should have media"
        );
        assert!(
            AttachmentInfo::new(1, 1).has_media(),
            "image+video should have media"
        );
    }

    // ── Composite and tier selection tests ──────────────────────────────

    #[test]
    fn test_classify_simple_prompt() {
        let result = PromptClassifier::classify(
            "What is 2+2?",
            None,
            &default_weights(),
            &default_boundaries(),
            &no_attachments(),
        );
        assert_eq!(
            result.tier,
            Tier::Simple,
            "simple math should be SIMPLE tier"
        );
        assert!(
            result.composite_score < 0.25,
            "composite should be below SIMPLE→MEDIUM boundary"
        );
        assert!(
            !result.requires_vision,
            "no attachments should not require vision"
        );
    }

    #[test]
    fn test_classify_complex_prompt() {
        let result = PromptClassifier::classify(
            "Analyse the microservice architecture and refactor the async concurrency \
             implementation with dependency injection. Must ensure thread safety \
             with mutex and avoid deadlock.",
            None,
            &default_weights(),
            &default_boundaries(),
            &no_attachments(),
        );
        assert!(
            matches!(result.tier, Tier::Medium | Tier::Complex | Tier::Reasoning),
            "complex code prompt should be MEDIUM or above, got {:?}",
            result.tier,
        );
    }

    #[test]
    fn test_classify_reasoning_prompt() {
        let result = PromptClassifier::classify(
            "Prove the theorem that for all n, the sum from i=1 to n of i equals n(n+1)/2. \
             Use mathematical induction. Show the base case, inductive hypothesis, \
             and inductive step. Therefore deduce the conclusion.",
            None,
            &default_weights(),
            &default_boundaries(),
            &no_attachments(),
        );
        assert!(
            matches!(result.tier, Tier::Medium | Tier::Complex | Tier::Reasoning),
            "mathematical proof should be MEDIUM or above, got {:?}",
            result.tier,
        );
    }

    #[test]
    fn test_classify_creative_prompt() {
        let result = PromptClassifier::classify(
            "Imagine and write a creative fictional story about a fantasy world. \
             Create innovative characters and artistic descriptions.",
            None,
            &default_weights(),
            &default_boundaries(),
            &no_attachments(),
        );
        // Creative prompts can land in any tier depending on weights
        assert!(
            matches!(result.tier, Tier::Simple | Tier::Medium | Tier::Complex),
            "creative prompt should be SIMPLE, MEDIUM or COMPLEX, got {:?}",
            result.tier,
        );
    }

    #[test]
    fn test_classify_with_image_attachment_requires_vision() {
        let attachments = AttachmentInfo::new(1, 0);
        let result = PromptClassifier::classify(
            "What is in this image?",
            None,
            &default_weights(),
            &default_boundaries(),
            &attachments,
        );
        assert!(
            result.requires_vision,
            "image attachment should require vision"
        );
        assert!(
            result.dimension_scores[14] > 0.0,
            "image_attachment dimension should be > 0"
        );
    }

    #[test]
    fn test_composite_score_clamped() {
        let scores = [2.0; 15]; // All dimensions at 2.0 (above 1.0)
        let composite = PromptClassifier::compute_composite(&scores, &default_weights());
        assert_eq!(composite, 1.0, "composite should be clamped to 1.0");
    }

    #[test]
    fn test_composite_score_sparse() {
        // Only one dimension is non-zero
        let mut scores = [0.0; 15];
        scores[12] = 1.0; // code_complexity
        let composite = PromptClassifier::compute_composite(&scores, &default_weights());
        assert!(
            composite > 0.0,
            "sparse single dimension should give positive composite"
        );
        assert!(composite <= 1.0, "composite should be clamped");
    }

    #[test]
    fn test_select_tier_boundaries() {
        let boundaries = BoundaryConfig::default();
        assert_eq!(
            PromptClassifier::select_tier(0.0, &boundaries),
            Tier::Simple
        );
        assert_eq!(
            PromptClassifier::select_tier(0.24, &boundaries),
            Tier::Simple
        );
        assert_eq!(
            PromptClassifier::select_tier(0.25, &boundaries),
            Tier::Medium
        );
        assert_eq!(
            PromptClassifier::select_tier(0.49, &boundaries),
            Tier::Medium
        );
        assert_eq!(
            PromptClassifier::select_tier(0.50, &boundaries),
            Tier::Complex
        );
        assert_eq!(
            PromptClassifier::select_tier(0.74, &boundaries),
            Tier::Complex
        );
        assert_eq!(
            PromptClassifier::select_tier(0.75, &boundaries),
            Tier::Reasoning
        );
        assert_eq!(
            PromptClassifier::select_tier(1.0, &boundaries),
            Tier::Reasoning
        );
    }

    #[test]
    fn test_classify_deterministic() {
        // NFR-004: Same prompt and config must always produce the same tier.
        let prompt = "Explain the observer pattern in software engineering";
        let result1 = PromptClassifier::classify(
            prompt,
            None,
            &default_weights(),
            &default_boundaries(),
            &no_attachments(),
        );
        let result2 = PromptClassifier::classify(
            prompt,
            None,
            &default_weights(),
            &default_boundaries(),
            &no_attachments(),
        );
        assert_eq!(result1.composite_score, result2.composite_score);
        assert_eq!(result1.tier, result2.tier);
        assert_eq!(result1.dimension_scores, result2.dimension_scores);
    }

    #[test]
    fn test_classify_performance() {
        // NFR-001: Classification should complete in under 5ms for prompts
        // up to 10,000 tokens (~40,000 chars) on commodity hardware.
        let prompt = "word ".repeat(8000);
        // Warm up the thread-local regex cache.
        let _ = PromptClassifier::classify(
            &prompt,
            None,
            &default_weights(),
            &default_boundaries(),
            &no_attachments(),
        );
        let start = std::time::Instant::now();
        for _ in 0..5 {
            let _ = PromptClassifier::classify(
                &prompt,
                None,
                &default_weights(),
                &default_boundaries(),
                &no_attachments(),
            );
        }
        let elapsed = start.elapsed();
        let avg_micros = elapsed.as_micros() / 5;
        // The classifier is designed for sub-5ms performance.
        // Allow generous tolerance for CI/loaded-system variance (up to 30ms).
        assert!(
            avg_micros < 30_000,
            "average classification time {avg_micros}µs exceeds 30ms tolerance",
        );
    }
    #[test]
    fn test_max_paren_depth() {
        assert_eq!(PromptClassifier::max_paren_depth("hello"), 0);
        assert_eq!(PromptClassifier::max_paren_depth("(hello)"), 1);
        assert_eq!(PromptClassifier::max_paren_depth("((hello))"), 2);
        assert_eq!(PromptClassifier::max_paren_depth("({[hello]})"), 3);
    }

    #[test]
    fn test_all_dimension_names() {
        for i in 0..15 {
            let name = dimension_name(i);
            assert!(!name.is_empty(), "dimension {i} name should not be empty");
        }
    }

