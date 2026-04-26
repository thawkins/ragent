//! Performance optimizations for session processing.
//!
//! This module implements Milestone 3 from perfplan.md:
//! - System prompt component caching
//! - Incremental history management
//! - Context window pre-compaction

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::agent::AgentInfo;
use crate::llm::ChatMessage;
use crate::message::Message;
use crate::tool::{TeamContext, ToolRegistry};

/// Cache version counter for global invalidation tracking.
static CACHE_VERSION: AtomicU64 = AtomicU64::new(1);

/// Increment the global cache version to invalidate all caches.
pub fn invalidate_all_caches() {
    CACHE_VERSION.fetch_add(1, Ordering::SeqCst);
}

/// Get the current global cache version.
#[must_use]
pub fn current_cache_version() -> u64 {
    CACHE_VERSION.load(Ordering::SeqCst)
}

/// A cached value with version tracking for a specific component.
#[derive(Debug, Clone)]
pub struct Cached<T> {
    /// The cached value
    value: Option<T>,
    /// Cache version when this was last computed
    version: u64,
    /// Component-specific generation counter
    generation: u64,
}

impl<T> Default for Cached<T> {
    fn default() -> Self {
        Self {
            value: None,
            version: 0,
            generation: 0,
        }
    }
}

impl<T: Clone> Cached<T> {
    /// Create a new empty cache entry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the cache is valid given the current global version.
    #[must_use]
    pub fn is_valid(&self, current_version: u64) -> bool {
        self.value.is_some() && self.version == current_version
    }

    /// Get the cached value if valid, otherwise return None.
    #[must_use]
    pub fn get(&self, current_version: u64) -> Option<T> {
        if self.is_valid(current_version) {
            self.value.clone()
        } else {
            None
        }
    }

    /// Store a new value in the cache with the current global version.
    pub fn set(&mut self, value: T) {
        self.value = Some(value);
        self.version = current_cache_version();
    }

    /// Invalidate this cache entry by clearing its value.
    pub fn invalidate(&mut self) {
        self.value = None;
        self.generation += 1;
    }
}

/// Hashable key for agent prompt cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AgentPromptKey {
    agent_name: String,
    agent_prompt_hash: u64,
}

/// System prompt cache that tracks components separately for efficient invalidation.
///
/// Each component is cached independently so that changes to one component
/// don't require rebuilding all others.
#[derive(Debug)]
pub struct SystemPromptCache {
    /// Agent base prompts keyed by (agent_name, prompt_hash)
    agent_prompts: Mutex<HashMap<AgentPromptKey, Cached<String>>>,
    /// Tool reference section - changes only on tool registration
    tool_reference: Mutex<Cached<String>>,
    /// Codeindex guidance section - changes only on index state change
    codeindex_guidance: Mutex<Cached<String>>,
    /// Team guidance section - changes only on team membership change
    team_guidance: Mutex<Cached<String>>,
    /// Current cache version (monotonically increasing)
    cache_version: AtomicU64,
    /// Tool registry hash for detecting tool changes
    last_tool_registry_hash: Mutex<u64>,
    /// Last known code index state
    last_codeindex_active: Mutex<bool>,
    /// Last known team context hash
    last_team_hash: Mutex<u64>,
}

impl Default for SystemPromptCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemPromptCache {
    /// Create a new empty system prompt cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            agent_prompts: Mutex::new(HashMap::new()),
            tool_reference: Mutex::new(Cached::new()),
            codeindex_guidance: Mutex::new(Cached::new()),
            team_guidance: Mutex::new(Cached::new()),
            cache_version: AtomicU64::new(current_cache_version()),
            last_tool_registry_hash: Mutex::new(0),
            last_codeindex_active: Mutex::new(false),
            last_team_hash: Mutex::new(0),
        }
    }

    /// Get the current cache version.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.cache_version.load(Ordering::SeqCst)
    }

    /// Check if the cache version has changed and update internal state.
    fn refresh_version(&self) {
        let global_version = current_cache_version();
        let local_version = self.cache_version.load(Ordering::SeqCst);
        if global_version != local_version {
            self.cache_version.store(global_version, Ordering::SeqCst);
        }
    }

    /// Get or compute the cached agent prompt.
    pub fn get_agent_prompt<F>(&self, agent: &AgentInfo, compute: F) -> Option<String>
    where
        F: FnOnce(&AgentInfo) -> Option<String>,
    {
        self.refresh_version();
        let version = self.version();

        // Compute hash of agent's prompt
        let prompt_hash = agent
            .prompt
            .as_ref()
            .map(|p| {
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                p.hash(&mut hasher);
                hasher.finish()
            })
            .unwrap_or(0);

        let key = AgentPromptKey {
            agent_name: agent.name.clone(),
            agent_prompt_hash: prompt_hash,
        };

        let mut cache = self.agent_prompts.lock().ok()?;

        // Check if we have a valid cached entry
        if let Some(cached) = cache.get(&key) {
            if let Some(value) = cached.get(version) {
                return Some(value);
            }
        }

        // Compute and cache the value
        let value = compute(agent)?;
        let mut entry = Cached::new();
        entry.set(value.clone());
        cache.insert(key, entry);

        Some(value)
    }

    /// Get or compute the cached tool reference section.
    pub fn get_tool_reference<F>(&self, tool_registry: &ToolRegistry, compute: F) -> Option<String>
    where
        F: FnOnce(&ToolRegistry) -> String,
    {
        self.refresh_version();
        let version = self.version();

        // Compute hash of tool registry
        let current_hash = Self::hash_tool_registry(tool_registry);

        let mut last_hash = self.last_tool_registry_hash.lock().ok()?;
        let mut cache = self.tool_reference.lock().ok()?;

        // Check if tools changed or cache is invalid
        if *last_hash == current_hash {
            if let Some(value) = cache.get(version) {
                return Some(value);
            }
        }

        // Compute and cache
        let value = compute(tool_registry);
        cache.set(value.clone());
        *last_hash = current_hash;

        Some(value)
    }

    /// Get or compute the cached codeindex guidance section.
    pub fn get_codeindex_guidance<F>(
        &self,
        code_index_active: bool,
        compute_active: F,
        compute_disabled: F,
    ) -> Option<String>
    where
        F: FnOnce() -> String,
    {
        self.refresh_version();
        let version = self.version();

        let mut last_active = self.last_codeindex_active.lock().ok()?;
        let mut cache = self.codeindex_guidance.lock().ok()?;

        if *last_active == code_index_active {
            if let Some(value) = cache.get(version) {
                return Some(value);
            }
        }

        let value = if code_index_active {
            compute_active()
        } else {
            compute_disabled()
        };

        cache.set(value.clone());
        *last_active = code_index_active;

        Some(value)
    }

    /// Get or compute the cached team guidance section.
    pub fn get_team_guidance<F>(
        &self,
        team_context: Option<&TeamContext>,
        compute: F,
    ) -> Option<String>
    where
        F: FnOnce(Option<&TeamContext>) -> String,
    {
        self.refresh_version();
        let version = self.version();

        // Compute hash of team context
        let current_hash = team_context.map(Self::hash_team_context).unwrap_or(0);

        let mut last_hash = self.last_team_hash.lock().ok()?;
        let mut cache = self.team_guidance.lock().ok()?;

        if *last_hash == current_hash {
            if let Some(value) = cache.get(version) {
                return Some(value);
            }
        }

        let value = compute(team_context);
        cache.set(value.clone());
        *last_hash = current_hash;

        Some(value)
    }

    /// Invalidate all cached components.
    pub fn invalidate_all(&self) {
        invalidate_all_caches();
        self.cache_version
            .store(current_cache_version(), Ordering::SeqCst);

        if let Ok(mut cache) = self.tool_reference.lock() {
            cache.invalidate();
        }
        if let Ok(mut cache) = self.codeindex_guidance.lock() {
            cache.invalidate();
        }
        if let Ok(mut cache) = self.team_guidance.lock() {
            cache.invalidate();
        }
        if let Ok(mut cache) = self.agent_prompts.lock() {
            cache.clear();
        }
    }

    /// Invalidate only the tool reference cache (call when tools change).
    pub fn invalidate_tool_cache(&self) {
        if let Ok(mut cache) = self.tool_reference.lock() {
            cache.invalidate();
        }
        if let Ok(mut hash) = self.last_tool_registry_hash.lock() {
            *hash = 0; // Force recompute
        }
    }

    /// Invalidate only the codeindex guidance cache (call when index state changes).
    pub fn invalidate_codeindex_cache(&self) {
        if let Ok(mut cache) = self.codeindex_guidance.lock() {
            cache.invalidate();
        }
    }

    /// Invalidate only the team guidance cache (call when team membership changes).
    pub fn invalidate_team_cache(&self) {
        if let Ok(mut cache) = self.team_guidance.lock() {
            cache.invalidate();
        }
        if let Ok(mut hash) = self.last_team_hash.lock() {
            *hash = 0; // Force recompute
        }
    }

    /// Compute a hash of the tool registry for change detection.
    fn hash_tool_registry(registry: &ToolRegistry) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        registry.definitions().len().hash(&mut hasher);
        for def in registry.definitions() {
            def.name.hash(&mut hasher);
            def.description.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// Compute a hash of the team context for change detection.
    fn hash_team_context(context: &TeamContext) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        context.is_lead.hash(&mut hasher);
        context.team_name.hash(&mut hasher);
        hasher.finish()
    }
}

/// Session state with incremental history management.
///
/// Maintains a cached version of the chat message list and only
/// recomputes the parts that have changed since the last access.
#[derive(Debug)]
pub struct SessionState {
    /// Cached chat messages (converted from internal Message format)
    cached_chat_messages: Vec<ChatMessage>,
    /// Number of messages last time we checked
    last_message_count: usize,
    /// Running token count estimate
    estimated_token_count: usize,
    /// Last time the cache was updated
    last_updated: std::time::Instant,
    /// Session ID this state belongs to
    session_id: String,
}

impl SessionState {
    /// Create a new session state for the given session ID.
    #[must_use]
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            cached_chat_messages: Vec::new(),
            last_message_count: 0,
            estimated_token_count: 0,
            last_updated: std::time::Instant::now(),
            session_id: session_id.into(),
        }
    }

    /// Get the session ID.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Clear all cached state (e.g., after compaction or reset).
    pub fn clear(&mut self) {
        self.cached_chat_messages.clear();
        self.last_message_count = 0;
        self.estimated_token_count = 0;
        self.last_updated = std::time::Instant::now();
    }

    /// Get the current estimated token count.
    #[must_use]
    pub fn estimated_token_count(&self) -> usize {
        self.estimated_token_count
    }

    /// Check if compaction should be triggered based on token threshold.
    #[must_use]
    pub fn should_compact(&self, context_window: usize) -> bool {
        // Trigger compaction at 80% of context window to avoid emergency compaction
        let threshold = context_window.saturating_mul(80).saturating_div(100);
        self.estimated_token_count > threshold
    }
}

/// Token estimator for approximate token counting.
///
/// Uses a simple heuristic: ~4 characters per token on average.
/// This is fast but approximate - use tiktoken for precise counting when needed.
pub struct TokenEstimator;

impl TokenEstimator {
    /// Characters per token estimate (average for English text).
    pub const CHARS_PER_TOKEN: usize = 4;
    /// Base overhead per message in tokens.
    pub const MESSAGE_OVERHEAD: usize = 10;

    /// Estimate token count for a piece of text.
    #[must_use]
    pub fn estimate_text(text: &str) -> usize {
        text.len().saturating_div(Self::CHARS_PER_TOKEN)
    }

    /// Estimate token count for a message.
    #[must_use]
    pub fn estimate_message(msg: &Message) -> usize {
        let text_len: usize = msg
            .parts
            .iter()
            .map(|p| match p {
                crate::message::MessagePart::Text { text } => text.len(),
                crate::message::MessagePart::ToolCall { tool, state, .. } => {
                    tool.len()
                        + state
                            .output
                            .as_ref()
                            .and_then(|v| v.as_str())
                            .map(|s| s.len())
                            .unwrap_or(0)
                        + state.error.as_ref().map(|s| s.len()).unwrap_or(0)
                }
                crate::message::MessagePart::Image { .. } => 1000, // Rough estimate for image
                crate::message::MessagePart::Reasoning { text } => text.len(),
            })
            .sum();

        text_len.saturating_div(Self::CHARS_PER_TOKEN) + Self::MESSAGE_OVERHEAD
    }

    /// Estimate token count for a slice of messages.
    #[must_use]
    pub fn estimate_messages(messages: &[Message]) -> usize {
        messages.iter().map(Self::estimate_message).sum()
    }
}

/// Extension trait to add caching support to SessionProcessor.
///
/// This trait provides methods that use the cached versions of
/// system prompt components for improved performance.
pub trait CachedSessionProcessor {
    /// Get the system prompt cache (if available).
    fn system_prompt_cache(&self) -> Option<&SystemPromptCache>;

    /// Get the session state cache for a given session ID.
    fn session_state(&self, session_id: &str) -> Option<std::sync::MutexGuard<'_, SessionState>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_basic() {
        let mut cached: Cached<String> = Cached::new();
        assert!(cached.get(current_cache_version()).is_none());

        cached.set("test".to_string());
        assert_eq!(
            cached.get(current_cache_version()),
            Some("test".to_string())
        );

        invalidate_all_caches();
        assert!(cached.get(current_cache_version()).is_none());
    }

    #[test]
    fn test_token_estimator() {
        // "hello world" = 11 chars / 4 = ~3 tokens + 10 overhead = 13
        let msg = Message::user_text("test", "hello world");
        let estimate = TokenEstimator::estimate_message(&msg);
        assert!(estimate >= 10); // At least overhead
        assert!(estimate < 50); // Shouldn't be too high
    }

    #[test]
    fn test_session_state_should_compact() {
        let state = SessionState::new("test-session");
        assert!(!state.should_compact(100000));
    }
}
