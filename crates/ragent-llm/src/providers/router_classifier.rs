//! 15-dimension prompt classifier for the Model Router.
//!
//! Scores each prompt across 15 dimensions, computes a weighted composite
//! score, and selects a routing tier (SIMPLE, MEDIUM, COMPLEX, REASONING)
//! based on configurable boundary thresholds.
//!
//! All scoring functions are pure and deterministic (NFR-004): the same prompt
//! and configuration always produce the same tier.

use super::router_config::{BoundaryConfig, Tier, WeightConfig};

/// Summary of non-text attachments present in a chat request.
///
/// Used by dimension 15 (`image_attachment`) to detect prompts that
/// require a vision-capable model. The router should prefer or require
/// models with `Capabilities::vision == true` when `image_count > 0`.
#[derive(Debug, Clone, Default)]
pub struct AttachmentInfo {
    /// Number of image attachments detected (e.g. `ContentPart::ImageUrl`).
    pub image_count: usize,
    /// Number of video attachments detected (future use; currently always 0).
    pub video_count: usize,
}

impl AttachmentInfo {
    /// Create a new `AttachmentInfo` with the given counts.
    pub fn new(image_count: usize, video_count: usize) -> Self {
        Self {
            image_count,
            video_count,
        }
    }

    /// Returns `true` if any image or video attachments are present.
    pub fn has_media(&self) -> bool {
        self.image_count > 0 || self.video_count > 0
    }
}

/// Result of classifying a prompt.
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    /// Individual dimension scores (0.0–1.0), indexed by dimension number (0–14).
    pub dimension_scores: [f64; 15],
    /// Composite weighted score (0.0–1.0).
    pub composite_score: f64,
    /// Selected routing tier.
    pub tier: Tier,
    /// Whether the prompt contains image/video attachments that require
    /// a vision-capable model. When `true`, the router should only consider
    /// models with `Capabilities::vision == true`.
    pub requires_vision: bool,
    /// When the user supplied an explicit tier modifier (e.g. `/reasoning`),
    /// this is the tier requested by the modifier. Otherwise `None`.
    pub modifier_tier: Option<Tier>,
}

/// Dimension names in display order.
const DIMENSION_NAMES: [&str; 15] = [
    "token_count",
    "vocabulary_complexity",
    "syntax_complexity",
    "domain_specificity",
    "ambiguity",
    "context_dependency",
    "reasoning_depth",
    "creativity_level",
    "emotional_complexity",
    "multimodality",
    "instruction_complexity",
    "knowledge_recency",
    "code_complexity",
    "mathematical_complexity",
    "image_attachment",
];

/// Returns the dimension name at the given index.
pub fn dimension_name(index: usize) -> &'static str {
    DIMENSION_NAMES[index]
}

/// Prompt classifier that scores a prompt across 15 dimensions.
pub struct PromptClassifier;

impl Default for PromptClassifier {
    fn default() -> Self {
        Self::new()
    }
}

impl PromptClassifier {
    /// Create a new classifier instance.
    pub fn new() -> Self {
        Self
    }

    /// Classify a prompt, returning dimension scores, composite score, and
    /// the selected tier.
    ///
    /// `history_text` is optional context from prior conversation messages
    /// (used for context-dependency scoring when available).
    /// `attachments` describes any image/video attachments present in the
    /// request, used by dimension 15 (`image_attachment`) to detect prompts
    /// that require a vision-capable model.
    pub fn classify(
        prompt: &str,
        history_text: Option<&str>,
        weights: &WeightConfig,
        boundaries: &BoundaryConfig,
        attachments: &AttachmentInfo,
    ) -> ClassificationResult {
        let dimension_scores = Self::score_all_dimensions(prompt, history_text, attachments);
        let composite_score = Self::compute_composite(&dimension_scores, weights);
        let tier = Self::select_tier(composite_score, boundaries);
        let requires_vision = attachments.has_media();

        ClassificationResult {
            dimension_scores,
            composite_score,
            tier,
            requires_vision,
            modifier_tier: None,
        }
    }

    /// Build the MEDIUM-tier fallback result shared by the non-finite-score
    /// guard and the panic guard in [`Self::classify_safe`] (FR-039).
    fn medium_fallback(
        boundaries: &BoundaryConfig,
        attachments: &AttachmentInfo,
    ) -> ClassificationResult {
        ClassificationResult {
            dimension_scores: [0.0; 15],
            composite_score: boundaries.simple_medium,
            tier: Tier::Medium,
            requires_vision: attachments.has_media(),
            modifier_tier: None,
        }
    }

    /// Classify a prompt with error fallback to MEDIUM tier (FR-039).
    ///
    /// If the classifier encounters any error (panic, NaN, or Inf in the
    /// composite score), the method falls back to `Tier::Medium` and returns
    /// zeroed dimension scores.
    pub fn classify_safe(
        prompt: &str,
        history_text: Option<&str>,
        weights: &WeightConfig,
        boundaries: &BoundaryConfig,
        attachments: &AttachmentInfo,
    ) -> ClassificationResult {
        let result = ragent_types::panic_guard::run(|| {
            Self::classify(prompt, history_text, weights, boundaries, attachments)
        });

        match result {
            Ok(r) => {
                // Guard against NaN/Inf in composite score (FR-039).
                if r.composite_score.is_nan() || r.composite_score.is_infinite() {
                    tracing::warn!(
                        composite = r.composite_score,
                        "Classifier produced invalid composite score, falling back to MEDIUM tier"
                    );
                    Self::medium_fallback(boundaries, attachments)
                } else {
                    r
                }
            }
            Err(_) => {
                tracing::warn!("Classifier panicked, falling back to MEDIUM tier (FR-039)");
                Self::medium_fallback(boundaries, attachments)
            }
        }
    }

    /// Score all 15 dimensions for a prompt.
    pub fn score_all_dimensions(
        prompt: &str,
        history_text: Option<&str>,
        attachments: &AttachmentInfo,
    ) -> [f64; 15] {
        let history = history_text.unwrap_or("");
        // Pre-compute lowercased prefix for keyword scanning only.
        let lower_prompt_scan = scan_prefix(prompt).to_lowercase();
        let lower_history_scan = scan_prefix(history).to_lowercase();
        [
            Self::score_token_count(prompt),
            Self::score_vocabulary_complexity(prompt),
            Self::score_syntax_complexity(prompt),
            Self::score_domain_specificity_lower(&lower_prompt_scan),
            Self::score_ambiguity_lower(&lower_prompt_scan),
            Self::score_context_dependency_lower(&lower_prompt_scan, &lower_history_scan),
            Self::score_reasoning_depth_lower(&lower_prompt_scan),
            Self::score_creativity_level_lower(&lower_prompt_scan),
            Self::score_emotional_complexity_lower(&lower_prompt_scan),
            Self::score_multimodality_lower(&lower_prompt_scan),
            Self::score_instruction_complexity(prompt, &lower_prompt_scan),
            Self::score_knowledge_recency_lower(&lower_prompt_scan),
            Self::score_code_complexity(prompt, &lower_prompt_scan),
            Self::score_mathematical_complexity_lower(&lower_prompt_scan),
            Self::score_image_attachment(attachments),
        ]
    }

    /// Compute the weighted composite score from dimension scores.
    ///
    /// Uses a sparse-weighted approach: dimensions with scores above a
    /// minimum threshold contribute to the composite. A dimension-density
    /// boost is applied when multiple dimensions fire, reflecting that a
    /// prompt hitting several complexity axes is inherently more complex
    /// than one hitting just one.
    pub fn compute_composite(scores: &[f64; 15], weights: &WeightConfig) -> f64 {
        let mut weighted_sum = 0.0;
        let mut active_weight_sum = 0.0;
        let mut active_count = 0usize;
        const MIN_SIGNAL: f64 = 0.05;

        for (i, score) in scores.iter().enumerate().take(15) {
            let w = weights.weight_by_index(i);
            if *score > MIN_SIGNAL {
                weighted_sum = f64::mul_add(*score, w, weighted_sum);
                active_weight_sum += w;
                active_count += 1;
            }
        }

        // If no dimensions are active, return 0.
        if active_weight_sum < 1e-10 {
            return 0.0;
        }

        // Normalise by active weights (sparse average).
        let mut composite = weighted_sum / active_weight_sum;

        // Dimension-density boost: more active dimensions → higher score.
        // 1-2 active dimensions → no boost; 3+ → gradual boost up to 15%.
        let density_boost = if active_count <= 2 {
            1.0
        } else {
            1.0 + 0.05 * (active_count - 2) as f64 / 12.0
        };
        composite *= density_boost;

        // Clamp to [0.0, 1.0]
        composite.clamp(0.0, 1.0)
    }

    /// Select a tier based on the composite score and boundary thresholds.
    pub fn select_tier(composite: f64, boundaries: &BoundaryConfig) -> Tier {
        if composite >= boundaries.complex_reasoning {
            Tier::Reasoning
        } else if composite >= boundaries.medium_complex {
            Tier::Complex
        } else if composite >= boundaries.simple_medium {
            Tier::Medium
        } else {
            Tier::Simple
        }
    }

    // ─────────────────────────────────────────────────────────────────────
    // Dimension Scorers
    // ─────────────────────────────────────────────────────────────────────

    /// Dimension 1: Token count — length of the message.
    ///
    /// Short prompts (<50 chars) → low score; very long prompts (>5000 chars)
    /// → high score. Uses character-count buckets as a proxy for token count.
    fn score_token_count(prompt: &str) -> f64 {
        let len = prompt.len();
        match len {
            0..=50 => 0.05,
            51..=200 => 0.15,
            201..=500 => 0.3,
            501..=1000 => 0.5,
            1001..=2500 => 0.65,
            2501..=5000 => 0.8,
            _ => 0.95,
        }
    }

    /// Dimension 2: Vocabulary complexity — ratio of long/unique words.
    ///
    /// Measures the proportion of words with 8+ characters and the
    /// type-token ratio (unique words / total words). Short prompts
    /// (<10 words) are penalised because high type-token ratios are
    /// expected and not meaningful.
    ///
    /// For performance, only the first 500 words are sampled for the
    /// type-token ratio calculation.
    fn score_vocabulary_complexity(prompt: &str) -> f64 {
        let words: Vec<&str> = prompt.split_whitespace().take(500).collect();
        if words.is_empty() {
            return 0.0;
        }

        let total = words.len() as f64;
        let long_words = words.iter().filter(|w| w.len() >= 8).count() as f64;
        let unique: std::collections::HashSet<&str> = words.iter().copied().collect();
        let type_token_ratio = unique.len() as f64 / total;

        // Weight long-word ratio and type-token ratio equally.
        let long_ratio = long_words / total;
        let raw = type_token_ratio.mul_add(0.5, long_ratio * 0.5);

        // Short prompts naturally have high type-token ratios; scale down.
        let length_penalty = if total < 10.0 {
            0.4
        } else if total < 20.0 {
            0.7
        } else {
            1.0
        };

        (raw * length_penalty).clamp(0.0, 1.0)
    }

    /// Dimension 3: Syntax complexity — depth of nested clauses and conditionals.
    ///
    /// Counts commas, semicolons, parentheses, and conditional keywords as
    /// proxies for syntactic complexity.
    fn score_syntax_complexity(prompt: &str) -> f64 {
        // Deliberately case-insensitive keyword scan: conditional markers like
        // "If" / " IF " must count like " if ", so scan the lowercased prefix
        // (matching the `_lower` scorer convention) instead of the raw prompt.
        let lower = scan_prefix(prompt).to_lowercase();
        let comma_count = prompt.matches(',').count() as f64;
        let semi_count = prompt.matches(';').count() as f64;
        let paren_depth = Self::max_paren_depth(prompt) as f64;
        let conditional_count = count_keywords_lower(
            &lower,
            &[
                " if ",
                " unless ",
                " either",
                " neither",
                " otherwise",
                " provided ",
                " assuming ",
                " whenever ",
            ],
        ) as f64;

        // Normalise: commas per 50 chars, semicolons per 50 chars
        let len_factor = (prompt.len() as f64 / 50.0).max(1.0);
        let comma_score = (comma_count / len_factor * 2.0).min(1.0);
        let semi_score = (semi_count / len_factor * 4.0).min(1.0);
        let paren_score = (paren_depth / 3.0).min(1.0);
        let cond_score = (conditional_count / 2.0).min(1.0);

        cond_score
            .mul_add(
                0.3,
                paren_score.mul_add(0.2, semi_score.mul_add(0.2, comma_score * 0.3)),
            )
            .clamp(0.0, 1.0)
    }

    /// Dimension 4: Domain specificity (pre-lowered input).
    fn score_domain_specificity_lower(lower: &str) -> f64 {
        let mut domain_hits = 0u64;
        let total_domains = 6u64;

        if contains_any(
            lower,
            &[
                "diagnosis",
                "patient",
                "symptom",
                "clinical",
                "pathology",
                "pharmacology",
                "etiology",
                "prognosis",
                "therapeutic",
            ],
        ) {
            domain_hits += 1;
        }
        if contains_any(
            lower,
            &[
                "statute",
                "liability",
                "jurisdiction",
                "plaintiff",
                "defendant",
                "tort",
                "contractual",
                "fiduciary",
                "litigation",
            ],
        ) {
            domain_hits += 1;
        }
        if contains_any(
            lower,
            &[
                "equity",
                "derivative",
                "portfolio",
                "arbitrage",
                "hedge",
                "yield",
                "dividend",
                "capitalization",
                "amortization",
            ],
        ) {
            domain_hits += 1;
        }
        if contains_any(
            lower,
            &[
                "thermal",
                "hydraulic",
                "structural",
                "circuit",
                "voltage",
                "torque",
                "calibration",
                "tolerance",
                "fabrication",
            ],
        ) {
            domain_hits += 1;
        }
        if contains_any(
            lower,
            &[
                "hypothesis",
                "experiment",
                "variable",
                "correlation",
                "statistical",
                "empirical",
                "methodology",
                "peer-reviewed",
                "replication",
            ],
        ) {
            domain_hits += 1;
        }
        if contains_any(
            lower,
            &[
                "microservice",
                "architecture",
                "dependency injection",
                "middleware",
                "refactor",
                "concurrency",
                "async",
                "framework",
                "api",
                "endpoint",
                "deployment",
                "pipeline",
                "container",
                "kubernetes",
                "serverless",
                "scalability",
                "infrastructure",
                "debugging",
                "compilation",
            ],
        ) {
            domain_hits += 1;
        }

        (domain_hits as f64 / total_domains as f64).clamp(0.0, 1.0)
    }

    /// Dimension 4: Domain specificity — presence of specialised terminology.
    ///
    /// Checks for keywords from medical, legal, financial, engineering,
    /// scientific, and software engineering domains.
    pub fn score_domain_specificity(prompt: &str) -> f64 {
        Self::score_domain_specificity_lower(&prompt.to_lowercase())
    }

    /// Dimension 5: Ambiguity (pre-lowered input).
    fn score_ambiguity_lower(lower: &str) -> f64 {
        let markers = [
            "what if",
            "could you",
            "might ",
            "perhaps",
            "explore",
            "brainstorm",
            "consider",
            "imagine if",
            "maybe",
            "possibly",
            "suggest",
            "any ideas",
            "open to",
            "flexible",
            "various ways",
        ];
        let count = count_keywords_lower(lower, &markers) as f64;
        (count / 4.0).min(1.0)
    }

    /// Dimension 5: Ambiguity — open-endedness of the request.
    pub fn score_ambiguity(prompt: &str) -> f64 {
        Self::score_ambiguity_lower(&prompt.to_lowercase())
    }

    /// Dimension 6: Context dependency (pre-lowered inputs).
    fn score_context_dependency_lower(lower: &str, lower_history: &str) -> f64 {
        let words: Vec<&str> = lower.split_whitespace().collect();
        if words.is_empty() {
            return 0.0;
        }

        let context_markers = [
            "it",
            "that",
            "this",
            "those",
            "these",
            "the above",
            "previous",
            "earlier",
            "before",
            "just mentioned",
            "as said",
            "from before",
            "we discussed",
            "you said",
            "like before",
        ];
        let pronoun_count = count_keywords_lower(lower, &context_markers) as f64;
        let pronoun_density = pronoun_count / words.len() as f64;
        let history_factor = if lower_history.is_empty() { 0.0 } else { 0.15 };

        pronoun_density.mul_add(3.0, history_factor).clamp(0.0, 1.0)
    }

    /// Dimension 6: Context dependency — reliance on prior conversation.
    pub fn score_context_dependency(prompt: &str, history: &str) -> f64 {
        Self::score_context_dependency_lower(&prompt.to_lowercase(), &history.to_lowercase())
    }

    /// Dimension 7: Reasoning depth (pre-lowered input).
    fn score_reasoning_depth_lower(lower: &str) -> f64 {
        let markers = [
            "therefore",
            "because",
            "implies",
            "prove",
            "deduce",
            "hypothesis",
            "conclude",
            "infer",
            "reasoning",
            "logic",
            "premise",
            "syllogism",
            "contradiction",
            "counterexample",
            "necessary",
            "sufficient",
            "if and only if",
            "necessary condition",
            "causality",
            "correlation does not imply",
        ];
        let count = count_keywords_lower(lower, &markers) as f64;
        (count / 2.5).min(1.0)
    }

    /// Dimension 7: Reasoning depth — number of logical inference steps required.
    pub fn score_reasoning_depth(prompt: &str) -> f64 {
        Self::score_reasoning_depth_lower(&prompt.to_lowercase())
    }

    /// Dimension 8: Creativity level (pre-lowered input).
    fn score_creativity_level_lower(lower: &str) -> f64 {
        let markers = [
            "imagine",
            "design",
            "create",
            "invent",
            "compose",
            "write a story",
            "write a poem",
            "creative",
            "fictional",
            "fantasy",
            "original",
            "novel",
            "innovative",
            "artistic",
            "generate ideas",
            "come up with",
        ];
        let count = count_keywords_lower(lower, &markers) as f64;
        (count / 3.0).min(1.0)
    }

    /// Dimension 8: Creativity level — degree of original generation needed.
    pub fn score_creativity_level(prompt: &str) -> f64 {
        Self::score_creativity_level_lower(&prompt.to_lowercase())
    }

    /// Dimension 9: Emotional complexity (pre-lowered input).
    fn score_emotional_complexity_lower(lower: &str) -> f64 {
        let sentiment_markers = [
            "subtle",
            "delicate",
            "nuanced",
            "sensitive",
            "empathetic",
            "compassionate",
            "controversial",
            "ethical",
            "moral dilemma",
            "emotional",
            "feelings",
            "perspective",
            "empathy",
            "understanding",
            "respectful",
            "tactful",
        ];
        let count = count_keywords_lower(lower, &sentiment_markers) as f64;
        (count / 3.0).min(1.0)
    }

    /// Dimension 9: Emotional complexity — nuance in tone or sentiment.
    pub fn score_emotional_complexity(prompt: &str) -> f64 {
        Self::score_emotional_complexity_lower(&prompt.to_lowercase())
    }

    /// Dimension 10: Multimodality (pre-lowered input).
    fn score_multimodality_lower(lower: &str) -> f64 {
        let markers = [
            "image",
            "diagram",
            "screenshot",
            "picture",
            "photo",
            "figure",
            "chart",
            "graph",
            "visual",
            "video",
            "audio",
            "file",
            "attachment",
            "upload",
            "download",
            "base64",
            ".png",
            ".jpg",
            ".gif",
            ".svg",
            ".pdf",
        ];
        let count = count_keywords_lower(lower, &markers) as f64;
        (count / 3.0).min(1.0)
    }

    /// Dimension 10: Multimodality — references to images, files, or non-text content.
    pub fn score_multimodality(prompt: &str) -> f64 {
        Self::score_multimodality_lower(&prompt.to_lowercase())
    }

    /// Dimension 11: Instruction complexity (uses original prompt for regex,
    /// pre-lowered for keyword search).
    fn score_instruction_complexity(prompt: &str, lower: &str) -> f64 {
        let numbered = regex_count(r"(?m)^\s*\d+[\.\)]", prompt);
        let bullets = regex_count(r"(?m)^\s*[-•*]\s", prompt);
        let constraint_markers = [
            "must",
            "should",
            "never",
            "always",
            "ensure",
            "require",
            "mandatory",
            "necessary",
            "critical",
            "important",
            "essential",
            "forbidden",
            "prohibited",
            "avoid",
        ];
        let constraints = count_keywords_lower(lower, &constraint_markers) as f64;

        let steps_score = ((numbered + bullets) as f64 / 5.0).min(1.0);
        let constraint_score = (constraints / 4.0).min(1.0);

        constraint_score
            .mul_add(0.4, steps_score * 0.6)
            .clamp(0.0, 1.0)
    }

    /// Dimension 12: Knowledge recency (pre-lowered input).
    fn score_knowledge_recency_lower(lower: &str) -> f64 {
        let markers = [
            "latest",
            "current",
            "recent",
            "recently",
            "updated",
            "news",
            "today",
            "this year",
            "this month",
            "this week",
            "2024",
            "2025",
            "2026",
            "now",
            "modern",
            "cutting-edge",
            "state-of-the-art",
            "newest",
            "up-to-date",
        ];
        let count = count_keywords_lower(lower, &markers) as f64;
        (count / 3.0).min(1.0)
    }

    /// Dimension 12: Knowledge recency — need for up-to-date information.
    pub fn score_knowledge_recency(prompt: &str) -> f64 {
        Self::score_knowledge_recency_lower(&prompt.to_lowercase())
    }

    /// Dimension 13: Code complexity (uses original prompt for code fences,
    /// pre-lowered for keyword search).
    fn score_code_complexity(prompt: &str, lower: &str) -> f64 {
        let code_fences = prompt.matches("```").count() as f64 / 2.0;
        let fence_score = (code_fences / 1.5).min(1.0);

        let prog_markers = [
            "function",
            "class",
            "method",
            "variable",
            "loop",
            "recursion",
            "async",
            "concurrency",
            "thread",
            "mutex",
            "deadlock",
            "compile",
            "runtime",
            "debug",
            "refactor",
            "test",
        ];
        let prog_count = count_keywords_lower(lower, &prog_markers) as f64;
        let prog_score = (prog_count / 3.0).min(1.0);

        let arch_markers = [
            "mvc",
            "microservice",
            "dependency injection",
            "observer",
            "factory pattern",
            "singleton",
            "middleware",
            "orm",
            "rest api",
            "graphql",
            "grpc",
            "event sourcing",
        ];
        let arch_count = count_keywords_lower(lower, &arch_markers) as f64;
        let arch_score = (arch_count / 1.5).min(1.0);

        arch_score
            .mul_add(0.25, prog_score.mul_add(0.5, fence_score * 0.25))
            .clamp(0.0, 1.0)
    }

    /// Dimension 14: Mathematical complexity (pre-lowered input).
    fn score_mathematical_complexity_lower(lower: &str) -> f64 {
        let latex_markers = [
            "\\frac",
            "\\sum",
            "\\int",
            "\\prod",
            "\\sqrt",
            "\\alpha",
            "\\beta",
            "\\gamma",
            "\\delta",
            "\\theta",
            "\\lambda",
            "\\pi",
            "\\sigma",
            "\\omega",
            "\\infty",
            "\\forall",
            "\\exists",
            "\\Rightarrow",
            "\\Leftrightarrow",
            "\\times",
            "\\div",
            "\\leq",
            "\\geq",
            "\\neq",
            "\\in",
        ];
        let latex_count = count_keywords_lower(lower, &latex_markers) as f64;
        let latex_score = (latex_count / 2.0).min(1.0);

        let eq_markers = [
            "∫", "∑", "∏", "√", "π", "∞", "∀", "∃", "⇒", "⇔", "≤", "≥", "≠", "∈", "⊂", "∪", "∩",
        ];
        let eq_count = count_keywords_lower(lower, &eq_markers) as f64;
        let eq_score = (eq_count / 2.0).min(1.0);

        let proof_markers = [
            "theorem",
            "lemma",
            "corollary",
            "proof",
            "proposition",
            "axiom",
            "conjecture",
            "derive",
            "integral",
            "derivative",
            "differential",
            "polynomial",
            "matrix",
            "eigenvector",
        ];
        let proof_count = count_keywords_lower(lower, &proof_markers) as f64;
        let proof_score = (proof_count / 2.0).min(1.0);

        proof_score
            .mul_add(0.3, eq_score.mul_add(0.35, latex_score * 0.35))
            .clamp(0.0, 1.0)
    }

    /// Dimension 14: Mathematical complexity — formal mathematics, proofs, calculations.
    pub fn score_mathematical_complexity(prompt: &str) -> f64 {
        Self::score_mathematical_complexity_lower(&prompt.to_lowercase())
    }

    /// Dimension 15: Image attachment — whether the prompt contains actual
    /// image or video attachments that require a vision-capable model.
    ///
    /// Unlike dimension 10 (`multimodality`), which detects *textual references*
    /// to images in the prompt, this dimension detects *actual* image/video
    /// content parts attached to the request. This is a hard constraint: if
    /// the score is above zero, the router should only select models with
    /// `Capabilities::vision == true`.
    ///
    /// Scoring:
    /// - 0 images, 0 videos → 0.0
    /// - 1 image → 0.5
    /// - 2+ images → 1.0
    /// - Any video → 1.0
    fn score_image_attachment(attachments: &AttachmentInfo) -> f64 {
        if attachments.video_count > 0 {
            return 1.0;
        }
        match attachments.image_count {
            0 => 0.0,
            1 => 0.5,
            _ => 1.0,
        }
    }

    /// Dimension 15: Image attachment — public convenience wrapper.
    pub fn score_image_attachment_pub(attachments: &AttachmentInfo) -> f64 {
        Self::score_image_attachment(attachments)
    }

    /// Compute the maximum nesting depth of parentheses in `text`.
    fn max_paren_depth(text: &str) -> usize {
        let mut depth = 0usize;
        let mut max_depth = 0usize;
        for ch in text.chars() {
            match ch {
                '(' | '[' | '{' => {
                    depth += 1;
                    max_depth = max_depth.max(depth);
                }
                ')' | ']' | '}' => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }
        max_depth
    }
}

/// Count how many of `keywords` appear in `lower_text` (already lowercased).
fn count_keywords_lower(lower_text: &str, keywords: &[&str]) -> u64 {
    let mut count = 0u64;
    for kw in keywords {
        if lower_text.contains(kw) {
            count += 1;
        }
    }
    count
}

/// Check if any of `keywords` appear in `lower_text` (already lowercased).
fn contains_any(lower_text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| lower_text.contains(kw))
}

/// Maximum number of characters to scan for keyword-based dimensions.
/// Scanning the first 2000 characters is sufficient for classification
/// accuracy while keeping performance under 5ms for long prompts.
const KEYWORD_SCAN_LIMIT: usize = 2000;

/// Truncate a string for keyword scanning. Returns the original string
/// if it's under the scan limit, otherwise returns a prefix.
fn scan_prefix(text: &str) -> &str {
    if text.len() <= KEYWORD_SCAN_LIMIT {
        text
    } else {
        // Find a valid char boundary near the limit
        let mut end = KEYWORD_SCAN_LIMIT;
        while end < text.len() && !text.is_char_boundary(end) {
            end += 1;
        }
        &text[..end.min(text.len())]
    }
}

/// Count regex matches, returning 0 on invalid patterns or errors.
///
/// Uses a thread-local cache for compiled patterns so repeated calls with
/// the same pattern do not recompile.
fn regex_count(pattern: &str, text: &str) -> u64 {
    use regex::Regex;
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static CACHE: RefCell<HashMap<String, Regex>> = RefCell::new(HashMap::new());
    }

    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        let re = match cache.entry(pattern.to_string()) {
            std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
            std::collections::hash_map::Entry::Vacant(e) => match Regex::new(pattern) {
                Ok(regex) => e.insert(regex),
                Err(_) => return 0,
            },
        };
        re.find_iter(text).count() as u64
    })
}

#[cfg(test)]
#[path = "../../tests/inline/router_classifier.rs"]
mod router_classifier_tests;

#[cfg(test)]
#[path = "../../tests/inline/router_classifier_extended.rs"]
mod router_classifier_tests_extended;
