//! Performance optimizations for session processing.
//!
//! This module implements Milestone 3 from perfplan.md:
//! - System prompt component caching
//! - Incremental history management
//! - Context window pre-compression

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use rustc_hash::FxHasher;

use crate::agent::AgentInfo;
use crate::llm::ChatMessage;
use crate::tool::{TeamContext, ToolRegistry};
use ragent_types::ThinkingConfig;

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
    /// Tool registry version for detecting tool changes (PERF-012).
    ///
    /// Stores the [`ToolRegistry::version`] observed at the time the
    /// tool-reference cache was last populated. The next `get_tool_reference`
    /// call compares the registry's current version against this value in
    /// O(1) instead of re-hashing all ~111 tool definitions.
    last_tool_registry_version: Mutex<u64>,
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
            last_tool_registry_version: Mutex::new(0),
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
        // PERF-031: FxHash (~2–5× faster than `DefaultHasher` for short
        // non-adversarial cache keys).
        let prompt_hash = agent
            .prompt
            .as_ref()
            .map(|p| {
                use std::hash::Hasher;
                let mut hasher = FxHasher::default();
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

        // PERF-012: compare the registry's monotonic version counter
        // (O(1)) instead of hashing all ~111 tool definitions (O(n)).
        let current_version = tool_registry.version();

        let mut last_version = self.last_tool_registry_version.lock().ok()?;
        let mut cache = self.tool_reference.lock().ok()?;

        // Check if tools changed or cache is invalid
        if *last_version == current_version {
            if let Some(value) = cache.get(version) {
                return Some(value);
            }
        }

        // Compute and cache
        let value = compute(tool_registry);
        cache.set(value.clone());
        *last_version = current_version;

        Some(value)
    }

    /// Get or compute the cached codeindex guidance section.
    ///
    /// The two compute closures are passed as a single `compute` callback
    /// that receives a `bool` indicating the active/disabled state, which
    /// sidesteps the "no two closures have the same type" issue without
    /// requiring callers to box their closures.
    pub fn get_codeindex_guidance<F>(&self, code_index_active: bool, compute: F) -> Option<String>
    where
        F: FnOnce(bool) -> String,
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

        let value = compute(code_index_active);
        cache.set(value.clone());
        *last_active = code_index_active;

        Some(value)
    }

    /// Backwards-compatible wrapper that accepts two separate closures.
    /// Provided so existing call sites that already pass two closures
    /// continue to work after the API change to `get_codeindex_guidance`.
    pub fn get_codeindex_guidance_with<F>(
        &self,
        code_index_active: bool,
        compute_active: F,
        compute_disabled: F,
    ) -> Option<String>
    where
        F: FnOnce() -> String,
    {
        self.get_codeindex_guidance(code_index_active, |is_active| {
            if is_active {
                compute_active()
            } else {
                compute_disabled()
            }
        })
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
        // PERF-012: reset the stored version so the next lookup sees a
        // mismatch and rebuilds. Using `0` as the sentinel works because the
        // registry version starts at `0` and is bumped on every `register()`,
        // so any live registry will have `version > 0` after the first tool
        // is registered.
        if let Ok(mut version) = self.last_tool_registry_version.lock() {
            *version = 0; // Force recompute
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

    /// Compute a hash of the team context for change detection.
    /// PERF-031: FxHash for short non-adversarial cache keys.
    fn hash_team_context(context: &TeamContext) -> u64 {
        use std::hash::Hasher;
        let mut hasher = FxHasher::default();
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
    /// Last time the cache was updated
    last_updated: std::time::Instant,
    /// Session ID this state belongs to
    session_id: String,
    /// Current session-level thinking configuration.
    thinking: ThinkingConfig,
    /// History version last time the cache was populated.
    ///
    /// FR-006 / FR-007 (AgentPerf T-007): the agent loop computes a
    /// `history_version` (count, last-id, last-modified-ms) at the start
    /// of each step and only re-runs `history_to_chat_messages` when the
    /// version changes.  This avoids re-converting the same message list
    /// to provider-specific chat messages on every iteration of the
    /// tool-call loop.
    last_history_version: u64,
    /// Cached serialised form of the most recent `ChatRequest` whose
    /// history did not change.  Used by the FR-007 fast path so a step
    /// that does not mutate the history can skip the JSON serialisation
    /// step as well.
    cached_serialised: Option<Vec<u8>>,
}

impl SessionState {
    /// Create a new session state for the given session ID.
    #[must_use]
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            cached_chat_messages: Vec::new(),
            last_message_count: 0,
            last_updated: std::time::Instant::now(),
            session_id: session_id.into(),
            thinking: ThinkingConfig::default(),
            last_history_version: 0,
            cached_serialised: None,
        }
    }

    /// Get the session ID.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the session's current thinking configuration.
    #[must_use]
    pub fn thinking(&self) -> &ThinkingConfig {
        &self.thinking
    }

    /// Update the session's thinking configuration.
    pub fn set_thinking(&mut self, thinking: ThinkingConfig) {
        self.thinking = thinking;
    }

    /// Clear all cached state (e.g., after compression or reset).
    pub fn clear(&mut self) {
        self.cached_chat_messages.clear();
        self.last_message_count = 0;
        self.last_updated = std::time::Instant::now();
        self.last_history_version = 0;
        self.cached_serialised = None;
    }

    /// Return the cached chat-message list when the supplied history
    /// version is unchanged (FR-006).
    ///
    /// The caller is responsible for computing the version (typically
    /// `(count, last_id, last_modified_ms)`).  When the version matches
    /// the cached one, the previously-converted list is returned
    /// without re-running `history_to_chat_messages`.  When it differs,
    /// the version is recorded and `None` is returned, signalling that
    /// the caller should rebuild the list.
    pub fn cached_chat_messages_for_version(
        &mut self,
        history_version: u64,
    ) -> Option<&[ChatMessage]> {
        if self.last_history_version == history_version && !self.cached_chat_messages.is_empty() {
            Some(&self.cached_chat_messages)
        } else {
            self.last_history_version = history_version;
            None
        }
    }

    /// Store the rebuilt chat-message list and a serialised snapshot for
    /// the FR-007 fast path.  Companion to
    /// [`Self::cached_chat_messages_for_version`].
    pub fn store_chat_messages(&mut self, messages: Vec<ChatMessage>, serialised: Option<Vec<u8>>) {
        self.cached_chat_messages = messages;
        self.last_message_count = self.cached_chat_messages.len();
        self.cached_serialised = serialised;
    }

    /// Return the cached serialised `ChatRequest` body from the previous
    /// step, if any (FR-007).
    #[must_use]
    pub fn cached_serialised(&self) -> Option<&[u8]> {
        self.cached_serialised.as_deref()
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
    fn test_session_state_stores_thinking_config() {
        let mut state = SessionState::new("test-session");
        state.set_thinking(ThinkingConfig::off());
        assert_eq!(state.thinking(), &ThinkingConfig::off());
    }
}
