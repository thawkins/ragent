//! Compression configuration types for ragent.json.
//!
//! These types define the `compression` section of ragent's configuration file.
//! They are defined here (in `ragent-config`) so they can be parsed at config
//! load time without requiring the `headroom-core` dependency. The actual
//! Headroom integration code lives in `ragent-agent::compression` behind the
//! `compression` Cargo feature flag.

use serde::{Deserialize, Serialize};

/// Top-level compression configuration.
///
/// Corresponds to the `compression` key in `ragent.json`. When `enabled` is
/// `false`, the agent falls back to the existing `compact_history_with_atomic_tool_calls`
/// behaviour.
///
/// # Example
///
/// ```json
/// {
///   "compression": {
///     "enabled": true,
///     "auto_threshold": 0.80,
///     "ccr": {
///       "backend": "sqlite",
///       "capacity": 1000,
///       "ttl_secs": 300
///     },
///     "compressors": {
///       "json": true,
///       "diff": true,
///       "log": true,
///       "search": true,
///       "code": false,
///       "prose": false
///     },
///     "relevance": {
///       "enabled": false,
///       "scorer": "bm25",
///       "keep_top_k": 20
///     },
///     "tokenizer": {
///       "backend": "auto"
///     }
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CompressionConfig {
    /// Whether Headroom-based compression is enabled.
    ///
    /// When `false`, the agent uses the existing `compact_history_with_atomic_tool_calls`
    /// truncation behaviour. Default: `true`.
    pub enabled: bool,
    /// Fraction of the context window at which automatic compression triggers.
    ///
    /// For example, `0.80` means compression triggers when token usage exceeds 80%
    /// of the context window. Default: `0.80`.
    pub auto_threshold: f64,
    /// CCR (Compress-Cache-Retrieve) store configuration.
    pub ccr: CcrConfig,
    /// Per-content-type compressor toggles.
    pub compressors: CompressorConfig,
    /// Relevance filtering configuration.
    pub relevance: RelevanceConfig,
    /// Tokenizer backend selection.
    pub tokenizer: TokenizerConfig,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_threshold: 0.80,
            ccr: CcrConfig::default(),
            compressors: CompressorConfig::default(),
            relevance: RelevanceConfig::default(),
            tokenizer: TokenizerConfig::default(),
        }
    }
}

/// CCR store backend configuration.
///
/// Controls how original (pre-compression) content is stored so the LLM can
/// retrieve it on demand via the `headroom_retrieve` tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CcrConfig {
    /// CCR storage backend: `"sqlite"` (production default) or `"memory"` (testing).
    ///
    /// SQLite persists across restarts; memory is lost on restart but faster
    /// for testing. Default: `"sqlite"`.
    pub backend: String,
    /// Maximum number of entries in the CCR store before LRU eviction.
    ///
    /// Matches `headroom_core::ccr::DEFAULT_CAPACITY`. Default: `1000`.
    pub capacity: usize,
    /// Time-to-live in seconds for CCR entries.
    ///
    /// Matches `headroom_core::ccr::DEFAULT_TTL` (5 minutes). Default: `300`.
    pub ttl_secs: u64,
}

impl Default for CcrConfig {
    fn default() -> Self {
        Self {
            backend: "sqlite".to_string(),
            capacity: 1000,
            ttl_secs: 300,
        }
    }
}

/// Per-content-type compressor toggles.
///
/// Each flag controls whether the corresponding Headroom compressor is active.
/// Disabling a compressor means that content type passes through unmodified.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CompressorConfig {
    /// SmartCrusher for JSON tool outputs. Default: `true`.
    pub json: bool,
    /// DiffCompressor for diff/patch content. Default: `true`.
    pub diff: bool,
    /// LogCompressor for log output. Default: `true`.
    pub log: bool,
    /// SearchCompressor for code-search results. Default: `true`.
    pub search: bool,
    /// CodeCompressor for AST-based compression. Default: `false` (experimental).
    pub code: bool,
    /// Prose compression via Kompress-base ML model. Default: `false`.
    pub prose: bool,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            json: true,
            diff: true,
            log: true,
            search: true,
            code: false,
            prose: false,
        }
    }
}

/// Relevance filtering configuration.
///
/// When enabled, BM25 scoring ranks conversation messages by relevance to the
/// current query, keeping the most relevant ones during aggressive compression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RelevanceConfig {
    /// Whether relevance filtering is enabled. Default: `false`.
    pub enabled: bool,
    /// Scorer backend: `"bm25"` (keyword-based) or `"hybrid"` (keyword + embedding).
    /// Default: `"bm25"`.
    pub scorer: String,
    /// Keep at most K most relevant messages. Default: `20`.
    pub keep_top_k: usize,
}

impl Default for RelevanceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            scorer: "bm25".to_string(),
            keep_top_k: 20,
        }
    }
}

/// Tokenizer backend configuration.
///
/// Controls which token-counting backend is used for context window estimation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TokenizerConfig {
    /// Tokenizer backend: `"auto"` (tiktoken with estimation fallback),
    /// `"tiktoken"` (tiktoken only), or `"estimate"` (chars/4 heuristic only).
    /// Default: `"auto"`.
    pub backend: String,
}

impl Default for TokenizerConfig {
    fn default() -> Self {
        Self {
            backend: "auto".to_string(),
        }
    }
}


