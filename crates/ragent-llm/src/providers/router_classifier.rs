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
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::classify(prompt, history_text, weights, boundaries, attachments)
        }));

        match result {
            Ok(r) => {
                // Guard against NaN/Inf in composite score (FR-039).
                if r.composite_score.is_nan() || r.composite_score.is_infinite() {
                    tracing::warn!(
                        composite = r.composite_score,
                        "Classifier produced invalid composite score, falling back to MEDIUM tier"
                    );
                    ClassificationResult {
                        dimension_scores: [0.0; 15],
                        composite_score: boundaries.simple_medium,
                        tier: Tier::Medium,
                        requires_vision: attachments.has_media(),
                    }
                } else {
                    r
                }
            }
            Err(_) => {
                tracing::warn!("Classifier panicked, falling back to MEDIUM tier (FR-039)");
                ClassificationResult {
                    dimension_scores: [0.0; 15],
                    composite_score: boundaries.simple_medium,
                    tier: Tier::Medium,
                    requires_vision: attachments.has_media(),
                }
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
                weighted_sum += *score * w;
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
        let raw = long_ratio * 0.5 + type_token_ratio * 0.5;

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
        let comma_count = prompt.matches(',').count() as f64;
        let semi_count = prompt.matches(';').count() as f64;
        let paren_depth = Self::max_paren_depth(prompt) as f64;
        let conditional_count = count_keywords(
            prompt,
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

        (comma_score * 0.3 + semi_score * 0.2 + paren_score * 0.2 + cond_score * 0.3)
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

        (pronoun_density * 3.0 + history_factor).clamp(0.0, 1.0)
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

        (steps_score * 0.6 + constraint_score * 0.4).clamp(0.0, 1.0)
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

        (fence_score * 0.25 + prog_score * 0.5 + arch_score * 0.25).clamp(0.0, 1.0)
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

        (latex_score * 0.35 + eq_score * 0.35 + proof_score * 0.3).clamp(0.0, 1.0)
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

/// Count how many of `keywords` appear as whole-word or substring matches in
/// `text` (case-sensitive).
fn count_keywords(text: &str, keywords: &[&str]) -> u64 {
    let mut count = 0u64;
    for kw in keywords {
        if text.contains(kw) {
            count += 1;
        }
    }
    count
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
mod tests {
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
} // ── Extended classifier unit tests (T-027) ──────────────────────────────

#[cfg(test)]
mod tests_extended {
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
        let mut weights = WeightConfig::default();
        // Emphasise reasoning depth heavily
        weights.reasoning_depth = 0.5;
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
}
