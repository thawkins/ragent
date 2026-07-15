//! Configuration types for the Model Router virtual provider.
//!
//! Defines [`RouterConfig`], [`TierConfig`], [`WeightConfig`], and
//! [`BoundaryConfig`] — the serialisable configuration model that lives in
//! `ragent.json` under `provider.router`. All types implement `Default` with
//! built-in defaults so the router functions without explicit configuration
//! (FR-024).

use serde::{Deserialize, Serialize};

/// A single provider/model entry in a tier's fallback chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TierEntry {
    /// Provider identifier (e.g. `"anthropic"`, `"openai"`).
    pub provider: String,
    /// Model identifier (e.g. `"claude-sonnet-4-20250514"`).
    pub model: String,
}

/// Configuration for a single routing tier.
///
/// Each tier maps to an ordered list of [`TierEntry`] values where the first
/// entry is the primary target and subsequent entries are fallback candidates.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TierConfig {
    /// Ordered list of provider/model pairs for this tier.
    #[serde(default)]
    pub models: Vec<TierEntry>,
    /// Optional per-tier timeout in milliseconds, overriding the default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// The four routing tiers, ordered by complexity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Tier {
    /// Simple prompts — short, low-complexity requests.
    Simple,
    /// Medium prompts — moderate complexity, typical coding questions.
    Medium,
    /// Complex prompts — advanced reasoning, multi-step tasks.
    Complex,
    /// Reasoning prompts — deep analysis, proofs, mathematical reasoning.
    Reasoning,
}

impl Tier {
    /// Returns all tier variants in ascending complexity order.
    pub fn all() -> &'static [Tier] {
        &[Tier::Simple, Tier::Medium, Tier::Complex, Tier::Reasoning]
    }

    /// Returns the single-character abbreviation for this tier.
    pub fn initial(&self) -> char {
        match self {
            Tier::Simple => 'S',
            Tier::Medium => 'M',
            Tier::Complex => 'C',
            Tier::Reasoning => 'R',
        }
    }

    /// Parse a tier from a case-insensitive string.
    pub fn from_str_insensitive(s: &str) -> Option<Tier> {
        match s.to_uppercase().as_str() {
            "SIMPLE" => Some(Tier::Simple),
            "MEDIUM" => Some(Tier::Medium),
            "COMPLEX" => Some(Tier::Complex),
            "REASONING" => Some(Tier::Reasoning),
            _ => None,
        }
    }
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tier::Simple => write!(f, "SIMPLE"),
            Tier::Medium => write!(f, "MEDIUM"),
            Tier::Complex => write!(f, "COMPLEX"),
            Tier::Reasoning => write!(f, "REASONING"),
        }
    }
}

/// Classifier dimension weights.
///
/// Each field corresponds to one of the 15 classification dimensions and
/// represents its contribution to the composite complexity score. Weights
/// should sum to approximately 1.0; if not, they are normalised at load time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightConfig {
    /// Token count dimension weight.
    #[serde(default = "WeightConfig::default_token_count")]
    pub token_count: f64,
    /// Vocabulary complexity dimension weight.
    #[serde(default = "WeightConfig::default_vocabulary_complexity")]
    pub vocabulary_complexity: f64,
    /// Syntax complexity dimension weight.
    #[serde(default = "WeightConfig::default_syntax_complexity")]
    pub syntax_complexity: f64,
    /// Domain specificity dimension weight.
    #[serde(default = "WeightConfig::default_domain_specificity")]
    pub domain_specificity: f64,
    /// Ambiguity dimension weight.
    #[serde(default = "WeightConfig::default_ambiguity")]
    pub ambiguity: f64,
    /// Context dependency dimension weight.
    #[serde(default = "WeightConfig::default_context_dependency")]
    pub context_dependency: f64,
    /// Reasoning depth dimension weight.
    #[serde(default = "WeightConfig::default_reasoning_depth")]
    pub reasoning_depth: f64,
    /// Creativity level dimension weight.
    #[serde(default = "WeightConfig::default_creativity_level")]
    pub creativity_level: f64,
    /// Emotional complexity dimension weight.
    #[serde(default = "WeightConfig::default_emotional_complexity")]
    pub emotional_complexity: f64,
    /// Multimodality dimension weight.
    #[serde(default = "WeightConfig::default_multimodality")]
    pub multimodality: f64,
    /// Instruction complexity dimension weight.
    #[serde(default = "WeightConfig::default_instruction_complexity")]
    pub instruction_complexity: f64,
    /// Knowledge recency dimension weight.
    #[serde(default = "WeightConfig::default_knowledge_recency")]
    pub knowledge_recency: f64,
    /// Code complexity dimension weight.
    #[serde(default = "WeightConfig::default_code_complexity")]
    pub code_complexity: f64,
    /// Mathematical complexity dimension weight.
    #[serde(default = "WeightConfig::default_mathematical_complexity")]
    pub mathematical_complexity: f64,
    /// Image attachment dimension weight.
    /// Fires when the request contains actual image/video attachments,
    /// requiring a vision-capable model.
    #[serde(default = "WeightConfig::default_image_attachment")]
    pub image_attachment: f64,
}

impl WeightConfig {
    // Default weight values for each dimension (must sum to ~1.0).
    fn default_token_count() -> f64 {
        0.07
    }
    fn default_vocabulary_complexity() -> f64 {
        0.07
    }
    fn default_syntax_complexity() -> f64 {
        0.07
    }
    fn default_domain_specificity() -> f64 {
        0.08
    }
    fn default_ambiguity() -> f64 {
        0.07
    }
    fn default_context_dependency() -> f64 {
        0.07
    }
    fn default_reasoning_depth() -> f64 {
        0.08
    }
    fn default_creativity_level() -> f64 {
        0.07
    }
    fn default_emotional_complexity() -> f64 {
        0.05
    }
    fn default_multimodality() -> f64 {
        0.07
    }
    fn default_instruction_complexity() -> f64 {
        0.08
    }
    fn default_knowledge_recency() -> f64 {
        0.05
    }
    fn default_code_complexity() -> f64 {
        0.07
    }
    fn default_mathematical_complexity() -> f64 {
        0.05
    }
    fn default_image_attachment() -> f64 {
        0.05
    }

    /// Returns the weight for a dimension by its 0-based index (0–14).
    ///
    /// # Panics
    ///
    /// Panics if `index >= 15`.
    pub fn weight_by_index(&self, index: usize) -> f64 {
        match index {
            0 => self.token_count,
            1 => self.vocabulary_complexity,
            2 => self.syntax_complexity,
            3 => self.domain_specificity,
            4 => self.ambiguity,
            5 => self.context_dependency,
            6 => self.reasoning_depth,
            7 => self.creativity_level,
            8 => self.emotional_complexity,
            9 => self.multimodality,
            10 => self.instruction_complexity,
            11 => self.knowledge_recency,
            12 => self.code_complexity,
            13 => self.mathematical_complexity,
            14 => self.image_attachment,
            _ => panic!("weight_by_index: index {index} out of range (0..15)"),
        }
    }

    /// Returns the dimension name by its 0-based index (0–14).
    ///
    /// # Panics
    ///
    /// Panics if `index >= 15`.
    pub fn dimension_name(index: usize) -> &'static str {
        match index {
            0 => "token_count",
            1 => "vocabulary_complexity",
            2 => "syntax_complexity",
            3 => "domain_specificity",
            4 => "ambiguity",
            5 => "context_dependency",
            6 => "reasoning_depth",
            7 => "creativity_level",
            8 => "emotional_complexity",
            9 => "multimodality",
            10 => "instruction_complexity",
            11 => "knowledge_recency",
            12 => "code_complexity",
            13 => "mathematical_complexity",
            14 => "image_attachment",
            _ => panic!("dimension_name: index {index} out of range (0..15)"),
        }
    }

    /// Returns the sum of all dimension weights.
    pub fn sum(&self) -> f64 {
        self.token_count
            + self.vocabulary_complexity
            + self.syntax_complexity
            + self.domain_specificity
            + self.ambiguity
            + self.context_dependency
            + self.reasoning_depth
            + self.creativity_level
            + self.emotional_complexity
            + self.multimodality
            + self.instruction_complexity
            + self.knowledge_recency
            + self.code_complexity
            + self.mathematical_complexity
            + self.image_attachment
    }

    /// Normalise weights so they sum to 1.0.
    ///
    /// If the current sum is near zero, weights are left unchanged.
    pub fn normalise(&mut self) {
        let sum = self.sum();
        if sum.abs() < 1e-10 {
            return;
        }
        self.token_count /= sum;
        self.vocabulary_complexity /= sum;
        self.syntax_complexity /= sum;
        self.domain_specificity /= sum;
        self.ambiguity /= sum;
        self.context_dependency /= sum;
        self.reasoning_depth /= sum;
        self.creativity_level /= sum;
        self.emotional_complexity /= sum;
        self.multimodality /= sum;
        self.instruction_complexity /= sum;
        self.knowledge_recency /= sum;
        self.code_complexity /= sum;
        self.mathematical_complexity /= sum;
        self.image_attachment /= sum;
    }
}

impl Default for WeightConfig {
    fn default() -> Self {
        Self {
            token_count: Self::default_token_count(),
            vocabulary_complexity: Self::default_vocabulary_complexity(),
            syntax_complexity: Self::default_syntax_complexity(),
            domain_specificity: Self::default_domain_specificity(),
            ambiguity: Self::default_ambiguity(),
            context_dependency: Self::default_context_dependency(),
            reasoning_depth: Self::default_reasoning_depth(),
            creativity_level: Self::default_creativity_level(),
            emotional_complexity: Self::default_emotional_complexity(),
            multimodality: Self::default_multimodality(),
            instruction_complexity: Self::default_instruction_complexity(),
            knowledge_recency: Self::default_knowledge_recency(),
            code_complexity: Self::default_code_complexity(),
            mathematical_complexity: Self::default_mathematical_complexity(),
            image_attachment: Self::default_image_attachment(),
        }
    }
}

/// Tier boundary thresholds.
///
/// The three boundaries partition the composite score range [0.0, 1.0] into
/// four tiers: SIMPLE, MEDIUM, COMPLEX, and REASONING.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryConfig {
    /// SIMPLE → MEDIUM boundary (default 0.25).
    #[serde(default = "BoundaryConfig::default_simple_medium")]
    pub simple_medium: f64,
    /// MEDIUM → COMPLEX boundary (default 0.50).
    #[serde(default = "BoundaryConfig::default_medium_complex")]
    pub medium_complex: f64,
    /// COMPLEX → REASONING boundary (default 0.75).
    #[serde(default = "BoundaryConfig::default_complex_reasoning")]
    pub complex_reasoning: f64,
}

impl BoundaryConfig {
    fn default_simple_medium() -> f64 {
        0.25
    }
    fn default_medium_complex() -> f64 {
        0.50
    }
    fn default_complex_reasoning() -> f64 {
        0.75
    }

    /// Validate that boundaries are in ascending order and within [0.0, 1.0].
    ///
    /// Returns `Ok(())` if valid, or an error message describing the problem.
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.simple_medium) {
            return Err(format!(
                "simple_medium boundary {} is outside [0.0, 1.0]",
                self.simple_medium
            ));
        }
        if !(0.0..=1.0).contains(&self.medium_complex) {
            return Err(format!(
                "medium_complex boundary {} is outside [0.0, 1.0]",
                self.medium_complex
            ));
        }
        if !(0.0..=1.0).contains(&self.complex_reasoning) {
            return Err(format!(
                "complex_reasoning boundary {} is outside [0.0, 1.0]",
                self.complex_reasoning
            ));
        }
        if self.simple_medium >= self.medium_complex {
            return Err(format!(
                "simple_medium ({}) must be less than medium_complex ({})",
                self.simple_medium, self.medium_complex
            ));
        }
        if self.medium_complex >= self.complex_reasoning {
            return Err(format!(
                "medium_complex ({}) must be less than complex_reasoning ({})",
                self.medium_complex, self.complex_reasoning
            ));
        }
        Ok(())
    }
}

impl Default for BoundaryConfig {
    fn default() -> Self {
        Self {
            simple_medium: Self::default_simple_medium(),
            medium_complex: Self::default_medium_complex(),
            complex_reasoning: Self::default_complex_reasoning(),
        }
    }
}

/// Top-level router configuration.
///
/// Stored in `ragent.json` under `provider.router`. All fields have defaults
/// so the router can function without explicit configuration (FR-024).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    /// Whether the router is active. When `false`, requests pass through to
    /// the MEDIUM tier's default model without classification (FR-026).
    #[serde(default)]
    pub enabled: bool,

    /// Tier definitions mapping each tier to an ordered model fallback list.
    #[serde(default)]
    pub tiers: std::collections::HashMap<String, TierConfig>,

    /// Classifier dimension weights.
    #[serde(default)]
    pub weights: WeightConfig,

    /// Tier boundary thresholds.
    #[serde(default)]
    pub boundaries: BoundaryConfig,

    /// Number of recent conversation messages to include in classification
    /// context (default 3).
    #[serde(default = "RouterConfig::default_context_messages")]
    pub context_messages: usize,

    /// Default request timeout in milliseconds (default 30000).
    #[serde(default = "RouterConfig::default_timeout_ms")]
    pub default_timeout_ms: u64,
}

impl RouterConfig {
    fn default_context_messages() -> usize {
        3
    }
    fn default_timeout_ms() -> u64 {
        30000
    }

    /// Returns the [`TierConfig`] for the given tier, or the tier's built-in
    /// default if not configured.
    pub fn tier_config(&self, tier: Tier) -> TierConfig {
        let key = tier.to_string();
        self.tiers
            .get(&key)
            .cloned()
            .unwrap_or_else(|| default_tier_config(tier))
    }

    /// Validates the configuration, returning an error if boundaries are
    /// invalid or weights are degenerate.
    pub fn validate(&self) -> Result<(), String> {
        self.boundaries.validate()?;
        let weight_sum = self.weights.sum();
        if weight_sum < 0.01 {
            return Err(format!(
                "classifier weights sum to {weight_sum:.4}, which is too small"
            ));
        }
        Ok(())
    }
}

/// Built-in default tier models.
///
/// The router never imposes hard-coded provider/model pairs.  Tiers start empty
/// so that users (or the TUI `/provider router` setup flow) explicitly choose
/// discovered models for each bucket (FR-024).
fn default_tier_config(_tier: Tier) -> TierConfig {
    TierConfig {
        models: Vec::new(),
        timeout_ms: None,
    }
}

impl Default for RouterConfig {
    fn default() -> Self {
        let mut tiers = std::collections::HashMap::new();
        for tier in Tier::all() {
            tiers.insert(tier.to_string(), default_tier_config(*tier));
        }
        Self {
            enabled: false,
            tiers,
            weights: WeightConfig::default(),
            boundaries: BoundaryConfig::default(),
            context_messages: Self::default_context_messages(),
            default_timeout_ms: Self::default_timeout_ms(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_weights_sum_to_one() {
        let weights = WeightConfig::default();
        let sum = weights.sum();
        assert!(
            (sum - 1.0).abs() < 0.01,
            "default weights should sum to ~1.0, got {sum}"
        );
    }

    #[test]
    fn test_weight_normalisation() {
        let mut weights = WeightConfig::default();
        // Double all weights
        weights.token_count *= 2.0;
        weights.vocabulary_complexity *= 2.0;
        weights.syntax_complexity *= 2.0;
        weights.domain_specificity *= 2.0;
        weights.ambiguity *= 2.0;
        weights.context_dependency *= 2.0;
        weights.reasoning_depth *= 2.0;
        weights.creativity_level *= 2.0;
        weights.emotional_complexity *= 2.0;
        weights.multimodality *= 2.0;
        weights.instruction_complexity *= 2.0;
        weights.knowledge_recency *= 2.0;
        weights.code_complexity *= 2.0;
        weights.mathematical_complexity *= 2.0;
        weights.image_attachment *= 2.0;

        let doubled_sum = weights.sum();
        assert!(
            (doubled_sum - 2.0).abs() < 0.02,
            "doubled sum should be ~2.0, got {doubled_sum}"
        );
        weights.normalise();
        assert!(
            (weights.sum() - 1.0).abs() < 0.001,
            "normalised weights should sum to ~1.0, got {}",
            weights.sum()
        );
    }

    #[test]
    fn test_boundary_validation_valid() {
        let boundaries = BoundaryConfig::default();
        assert!(boundaries.validate().is_ok());
    }

    #[test]
    fn test_boundary_validation_out_of_range() {
        let boundaries = BoundaryConfig {
            simple_medium: -0.1,
            medium_complex: 0.5,
            complex_reasoning: 0.75,
        };
        assert!(boundaries.validate().is_err());
    }

    #[test]
    fn test_boundary_validation_not_ascending() {
        let boundaries = BoundaryConfig {
            simple_medium: 0.5,
            medium_complex: 0.3,
            complex_reasoning: 0.75,
        };
        assert!(boundaries.validate().is_err());
    }

    #[test]
    fn test_default_router_config_has_all_tiers() {
        let config = RouterConfig::default();
        assert!(config.tiers.contains_key("SIMPLE"));
        assert!(config.tiers.contains_key("MEDIUM"));
        assert!(config.tiers.contains_key("COMPLEX"));
        assert!(config.tiers.contains_key("REASONING"));
    }

    #[test]
    fn test_default_router_config_disabled() {
        let config = RouterConfig::default();
        assert!(!config.enabled);
    }

    #[test]
    fn test_tier_from_str_insensitive() {
        assert_eq!(Tier::from_str_insensitive("simple"), Some(Tier::Simple));
        assert_eq!(Tier::from_str_insensitive("MEDIUM"), Some(Tier::Medium));
        assert_eq!(Tier::from_str_insensitive("Complex"), Some(Tier::Complex));
        assert_eq!(
            Tier::from_str_insensitive("reasoning"),
            Some(Tier::Reasoning)
        );
        assert_eq!(Tier::from_str_insensitive("unknown"), None);
    }

    #[test]
    fn test_tier_display() {
        assert_eq!(Tier::Simple.to_string(), "SIMPLE");
        assert_eq!(Tier::Medium.to_string(), "MEDIUM");
        assert_eq!(Tier::Complex.to_string(), "COMPLEX");
        assert_eq!(Tier::Reasoning.to_string(), "REASONING");
    }

    #[test]
    fn test_tier_initial() {
        assert_eq!(Tier::Simple.initial(), 'S');
        assert_eq!(Tier::Medium.initial(), 'M');
        assert_eq!(Tier::Complex.initial(), 'C');
        assert_eq!(Tier::Reasoning.initial(), 'R');
    }

    #[test]
    fn test_tier_config_default_models_empty() {
        let config = RouterConfig::default();
        // Default router tiers are intentionally empty: ragent no longer
        // hard-codes provider/model pairs. Users configure the cluster via
        // `/provider router` or `provider.router` in `ragent.json`.
        for tier in Tier::all() {
            let tc = config.tier_config(*tier);
            assert!(
                tc.models.is_empty(),
                "default tier {:?} should contain no hard-coded models",
                tier
            );
            assert_eq!(tc.timeout_ms, None);
        }
        let all_default_providers: std::collections::HashSet<String> = Tier::all()
            .iter()
            .flat_map(|t| {
                config
                    .tier_config(*t)
                    .models
                    .into_iter()
                    .map(|e| e.provider)
            })
            .collect();
        assert!(
            !all_default_providers.contains("ollama"),
            "default tiers should not impose local Ollama models"
        );
        assert!(
            !all_default_providers.contains("anthropic"),
            "default tiers should not silently fall back to Anthropic"
        );
    }

    #[test]
    fn test_router_config_validate() {
        let config = RouterConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_weight_by_index() {
        let weights = WeightConfig::default();
        assert!((weights.weight_by_index(0) - 0.07).abs() < 1e-10);
        assert!((weights.weight_by_index(6) - 0.08).abs() < 1e-10);
        assert!((weights.weight_by_index(14) - 0.05).abs() < 1e-10);
    }

    #[test]
    fn test_dimension_name() {
        assert_eq!(WeightConfig::dimension_name(0), "token_count");
        assert_eq!(WeightConfig::dimension_name(13), "mathematical_complexity");
        assert_eq!(WeightConfig::dimension_name(14), "image_attachment");
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = RouterConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let deserialized: RouterConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.enabled, config.enabled);
        assert_eq!(deserialized.context_messages, config.context_messages);
        assert_eq!(deserialized.default_timeout_ms, config.default_timeout_ms);
    }
}
