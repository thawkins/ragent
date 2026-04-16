//! Memory compaction, deduplication, and eviction.
//!
//! This module implements Milestone 6 of the memory system plan:
//!
//! - **Deduplication** ([`deduplicate_memory`]): Detects semantically similar
//!   memories using FTS5 (or cosine similarity when embeddings are enabled)
//!   and merges or proposes merging of duplicates.
//!
//! - **Block compaction** ([`compact_blocks`]): Detects memory blocks that
//!   exceed their size limit and truncates them, logging the original
//!   content to the journal before compaction.
//!
//! - **Stale memory eviction** ([`evict_stale_memories`]): Finds memories
//!   below the minimum confidence threshold that haven't been accessed
//!   recently and proposes them for deletion (or auto-evicts).
//!
//! - **Compaction triggers** ([`CompactionTrigger`]): Determines when
//!   compaction runs based on session start, memory count, and time since
//!   last compaction.

use std::path::Path;
use std::sync::Arc;

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::config::{CompactionConfig, EvictionConfig, MemoryConfig};
use crate::event::EventBus;
use crate::memory::block::BlockScope;
use crate::memory::embedding::{
    EmbeddingProvider, cosine_similarity, deserialise_embedding,
};
use crate::memory::storage::{BlockStorage, FileBlockStorage};
use crate::storage::Storage;

// ── Memory Deduplication ──────────────────────────────────────────────────────

/// Result of a deduplication check for a proposed memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DedupResult {
    /// No duplicate found — safe to store.
    NoDuplicate,
    /// Exact or near-exact duplicate found (>0.95 similarity).
    /// The memory should be merged with the existing one.
    Duplicate {
        /// Row ID of the existing memory.
        existing_id: i64,
        /// Similarity score (0.0–1.0).
        similarity: f64,
        /// Merged content proposal.
        merged_content: String,
        /// Merged confidence (max of both).
        merged_confidence: f64,
        /// Merged tags (union of both).
        merged_tags: Vec<String>,
    },
    /// Near-duplicate found (0.8–0.95 similarity).
    /// A merge could be beneficial but requires user confirmation.
    NearDuplicate {
        /// Row ID of the existing memory.
        existing_id: i64,
        /// Similarity score (0.0–1.0).
        similarity: f64,
        /// Proposed merged content.
        proposed_content: String,
        /// Proposed merged confidence.
        proposed_confidence: f64,
        /// Proposed merged tags.
        proposed_tags: Vec<String>,
    },
}

/// Check whether a proposed memory duplicates an existing one.
///
/// Uses cosine similarity when embeddings are available, falling back
/// to FTS5 keyword overlap when they are not.
///
/// # Arguments
///
/// * `content` - The proposed memory content.
/// * `category` - Category of the proposed memory.
/// * `tags` - Tags of the proposed memory.
/// * `confidence` - Confidence of the proposed memory.
/// * `storage` - SQLite storage backend.
/// * `embedding_provider` - Optional embedding provider for semantic search.
///
/// # Returns
///
/// A [`DedupResult`] indicating whether a duplicate was found and what
/// action to take.
pub fn deduplicate_memory(
    content: &str,
    category: &str,
    tags: &[String],
    confidence: f64,
    storage: &Storage,
    embedding_provider: &dyn EmbeddingProvider,
) -> DedupResult {
    // Strategy 1: If embeddings are available, check cosine similarity.
    if embedding_provider.is_available() {
        if let Ok(query_embedding) = embedding_provider.embed(content) {
            if !query_embedding.is_empty() {
                return deduplicate_by_embedding(
                    content,
                    category,
                    tags,
                    confidence,
                    &query_embedding,
                    storage,
                );
            }
        }
    }

    // Strategy 2: FTS5 keyword overlap fallback.
    deduplicate_by_fts(content, category, tags, confidence, storage)
}

/// Deduplicate using cosine similarity on stored embeddings.
fn deduplicate_by_embedding(
    content: &str,
    _category: &str,
    tags: &[String],
    confidence: f64,
    query_embedding: &[f32],
    storage: &Storage,
) -> DedupResult {
    let embeddings = match storage.list_memory_embeddings() {
        Ok(e) => e,
        Err(e) => {
            warn!(error = %e, "Failed to list memory embeddings for dedup");
            return DedupResult::NoDuplicate;
        }
    };

    let dims = query_embedding.len();

    for (row_id, blob) in &embeddings {
        if let Ok(stored_embedding) = deserialise_embedding(blob, dims) {
            let similarity = cosine_similarity(query_embedding, &stored_embedding) as f64;

            if similarity > 0.95 {
                // Exact or near-exact duplicate.
                let existing = match storage.get_memory(*row_id) {
                    Ok(Some(m)) => m,
                    _ => continue,
                };

                return DedupResult::Duplicate {
                    existing_id: *row_id,
                    similarity,
                    merged_content: merge_content(&existing.content, content),
                    merged_confidence: existing.confidence.max(confidence),
                    merged_tags: merge_tags(
                        &storage.get_memory_tags(*row_id).unwrap_or_default(),
                        tags,
                    ),
                };
            }

            if similarity > 0.8 {
                // Near-duplicate — propose merge.
                let existing = match storage.get_memory(*row_id) {
                    Ok(Some(m)) => m,
                    _ => continue,
                };

                return DedupResult::NearDuplicate {
                    existing_id: *row_id,
                    similarity,
                    proposed_content: merge_content(&existing.content, content),
                    proposed_confidence: existing.confidence.max(confidence),
                    proposed_tags: merge_tags(
                        &storage.get_memory_tags(*row_id).unwrap_or_default(),
                        tags,
                    ),
                };
            }
        }
    }

    DedupResult::NoDuplicate
}

/// Deduplicate using FTS5 keyword overlap as a proxy for similarity.
fn deduplicate_by_fts(
    content: &str,
    category: &str,
    tags: &[String],
    confidence: f64,
    storage: &Storage,
) -> DedupResult {
    // Extract key terms for FTS search.
    let terms: Vec<&str> = content
        .split_whitespace()
        .filter(|w| w.len() > 4 && !is_stop_word(w))
        .take(8)
        .collect();

    if terms.is_empty() {
        return DedupResult::NoDuplicate;
    }

    let query = terms.join(" ");
    let cat_list: Vec<String> = vec![category.to_string()];
    let results = match storage.search_memories(&query, Some(&cat_list), None, 5, 0.0) {
        Ok(r) => r,
        Err(e) => {
            warn!(error = %e, "FTS dedup search failed");
            return DedupResult::NoDuplicate;
        }
    };

    for existing in &results {
        let overlap = word_overlap(content, &existing.content);

        if overlap > 0.85 {
            return DedupResult::Duplicate {
                existing_id: existing.id,
                similarity: overlap,
                merged_content: merge_content(&existing.content, content),
                merged_confidence: existing.confidence.max(confidence),
                merged_tags: merge_tags(
                    &storage.get_memory_tags(existing.id).unwrap_or_default(),
                    tags,
                ),
            };
        }

        if overlap > 0.65 {
            return DedupResult::NearDuplicate {
                existing_id: existing.id,
                similarity: overlap,
                proposed_content: merge_content(&existing.content, content),
                proposed_confidence: existing.confidence.max(confidence),
                proposed_tags: merge_tags(
                    &storage.get_memory_tags(existing.id).unwrap_or_default(),
                    tags,
                ),
            };
        }
    }

    DedupResult::NoDuplicate
}

/// Merge two content strings by combining unique sentences.
pub fn merge_content(existing: &str, new: &str) -> String {
    let mut sentences: Vec<String> = existing
        .split(". ")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    for sentence in new.split(". ") {
        let trimmed = sentence.trim().to_string();
        if !trimmed.is_empty()
            && !sentences
                .iter()
                .any(|s| s.to_lowercase() == trimmed.to_lowercase())
        {
            sentences.push(trimmed);
        }
    }

    let mut result = sentences.join(". ");
    if !result.ends_with('.') && !result.is_empty() {
        result.push('.');
    }
    result
}

/// Merge two tag lists by taking the union.
pub fn merge_tags(existing: &[String], new: &[String]) -> Vec<String> {
    let mut merged: Vec<String> = existing.to_vec();
    for tag in new {
        if !merged.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
            merged.push(tag.clone());
        }
    }
    merged.sort();
    merged.dedup();
    merged
}

/// Compute word overlap ratio between two strings.
fn word_overlap(a: &str, b: &str) -> f64 {
    use std::collections::HashSet;
    let a_lower = a.to_lowercase();
    let b_lower = b.to_lowercase();
    let words_a: HashSet<&str> = a_lower.split_whitespace().collect();
    let words_b: HashSet<&str> = b_lower.split_whitespace().collect();
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    let intersection = words_a.intersection(&words_b).count();
    intersection as f64 / words_a.len().max(words_b.len()) as f64
}

/// Common English stop words for dedup queries.
fn is_stop_word(word: &str) -> bool {
    const STOP_WORDS: &[&str] = &[
        "the", "and", "for", "that", "with", "this", "from", "are", "was", "were", "been", "have",
        "has", "had", "will", "would", "could", "should", "into", "about", "which", "their",
        "other", "than", "then", "also", "when", "what", "each", "does", "just", "more", "some",
        "over", "such", "after", "before", "between", "through", "during", "without", "using",
        "project", "files", "memory",
    ];
    STOP_WORDS.contains(&word.to_lowercase().as_str())
}

/// Apply a dedup result: merge or update the existing memory.
///
/// Called after the user confirms a near-duplicate merge, or automatically
/// for exact duplicates.
///
/// # Returns
///
/// The row ID of the merged memory (same as `existing_id`).
pub fn apply_dedup_merge(
    existing_id: i64,
    merged_content: &str,
    merged_confidence: f64,
    merged_tags: &[String],
    storage: &Storage,
) -> anyhow::Result<i64> {
    // Update the existing memory's content.
    storage.update_memory_content(existing_id, merged_content)?;
    // Update confidence.
    storage.update_memory_confidence(existing_id, merged_confidence)?;
    // Update tags.
    storage.set_memory_tags(existing_id, merged_tags)?;
    info!(id = existing_id, "Applied dedup merge to memory");
    Ok(existing_id)
}

// ── Block Compaction ──────────────────────────────────────────────────────────

/// Result of a block compaction pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionResult {
    /// Number of blocks checked.
    pub blocks_checked: usize,
    /// Number of blocks compacted.
    pub blocks_compacted: usize,
    /// Labels of compacted blocks.
    pub compacted_labels: Vec<String>,
}

/// Check memory blocks for size overflow and compact those exceeding limits.
///
/// For each block whose content exceeds 90% of `block_size_limit` bytes:
///
/// 1. Log the original content to the journal (for recovery).
/// 2. Truncate the content, preserving frontmatter and a summary.
/// 3. Save the compacted block back.
///
/// # Arguments
///
/// * `storage` - SQLite storage for journal logging.
/// * `working_dir` - Project directory for block storage.
/// * `config` - Memory configuration (for block size limit).
///
/// # Returns
///
/// A [`CompactionResult`] summarising what was done.
pub fn compact_blocks(
    storage: &Storage,
    working_dir: &Path,
    config: &MemoryConfig,
) -> CompactionResult {
    let block_storage = FileBlockStorage::new();
    let mut result = CompactionResult {
        blocks_checked: 0,
        blocks_compacted: 0,
        compacted_labels: Vec::new(),
    };

    // Default block size limit: 4096 bytes.
    let size_limit = config.block_size_limit();

    for scope in &[BlockScope::Project, BlockScope::Global] {
        let labels = match block_storage.list(scope, &working_dir.to_path_buf()) {
            Ok(l) => l,
            Err(e) => {
                warn!(scope = ?scope, error = %e, "Failed to list blocks for compaction");
                continue;
            }
        };

        for label in &labels {
            result.blocks_checked += 1;

            let block = match block_storage.load(label, scope, &working_dir.to_path_buf()) {
                Ok(Some(b)) => b,
                Ok(None) => continue,
                Err(e) => {
                    warn!(label, error = %e, "Failed to load block for compaction");
                    continue;
                }
            };

            // Check if block exceeds 90% of the size limit.
            if block.content.len() > (size_limit as f64 * 0.9) as usize {
                info!(
                    label,
                    size = block.content.len(),
                    limit = size_limit,
                    "Block exceeds 90% of size limit, compacting"
                );

                // Log original to journal before compacting.
                let _ = storage.create_journal_entry(
                    &uuid::Uuid::new_v4().to_string(),
                    &format!("Block compaction: {}", label),
                    &block.content,
                    working_dir
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                        .as_str(),
                    "system",
                    &["compaction".to_string(), "memory-block".to_string()],
                );
                // Compact: keep frontmatter + truncated content + note.
                let compacted = compact_block_content(&block.content, size_limit);

                // Save the compacted block.
                let mut compacted_block = block.clone();
                compacted_block.content = compacted;

                if let Err(e) = block_storage.save(&compacted_block, &working_dir.to_path_buf()) {
                    warn!(label, error = %e, "Failed to save compacted block");
                    continue;
                }

                result.blocks_compacted += 1;
                result.compacted_labels.push(label.clone());
            }
        }
    }

    if result.blocks_compacted > 0 {
        info!(
            blocks_checked = result.blocks_checked,
            blocks_compacted = result.blocks_compacted,
            "Block compaction complete"
        );
    }

    result
}

/// Compact block content by preserving frontmatter and truncating the body.
///
/// If the content has YAML frontmatter (delimited by `---`), we preserve it
/// and truncate the body. Otherwise, we truncate from the end.
///
/// A note is appended indicating the compaction and that the full content
/// was saved to the journal.
pub fn compact_block_content(content: &str, size_limit: usize) -> String {
    let target_size = (size_limit as f64 * 0.75) as usize; // Compact to 75% of limit

    // Check for YAML frontmatter.
    if content.starts_with("---\n") {
        if let Some(end_pos) = content[4..].find("\n---\n") {
            let frontmatter_end = end_pos + 8; // "---\n" + content + "\n---\n"
            let frontmatter = &content[..frontmatter_end];
            let body = &content[frontmatter_end..];

            if body.len() > target_size {
                let truncated_body = &body[..body.len().min(target_size)];
                return format!(
                    "{}\n{}\n\n---\n*This block was compacted. The full content was saved to the journal before compaction.*\n",
                    frontmatter.trim_end(),
                    truncated_body.trim_end()
                );
            }
        }
    }

    // No frontmatter or short frontmatter — just truncate.
    if content.len() > target_size {
        let truncated = &content[..target_size];
        format!(
            "{}\n\n---\n*This block was compacted. The full content was saved to the journal before compaction.*\n",
            truncated.trim_end()
        )
    } else {
        content.to_string()
    }
}

// ── Stale Memory Eviction ─────────────────────────────────────────────────────

/// Result of a stale memory eviction pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictionResult {
    /// Number of memories considered for eviction.
    pub memories_considered: usize,
    /// Number of memories evicted (deleted).
    pub memories_evicted: usize,
    /// IDs of evicted memories.
    pub evicted_ids: Vec<i64>,
}

/// Evict stale memories that have decayed below the minimum confidence
/// and haven't been accessed recently.
///
/// For each memory with `confidence < min_confidence` AND
/// `last_accessed > stale_days` days ago:
///
/// 1. Log the memory content to the journal (for recovery).
/// 2. Delete the memory (or queue for user confirmation if `auto_evict` is false).
///
/// # Arguments
///
/// * `storage` - SQLite storage backend.
/// * `config` - Eviction configuration.
/// * `working_dir` - Project directory (for journal context).
/// * `auto_evict` - If `true`, delete automatically. If `false`, just log proposals.
///
/// # Returns
///
/// An [`EvictionResult`] summarising what was done.
pub fn evict_stale_memories(
    storage: &Storage,
    config: &EvictionConfig,
    working_dir: &Path,
    auto_evict: bool,
) -> EvictionResult {
    let stale_threshold = Utc::now() - Duration::days(config.stale_days as i64);
    let min_confidence = config.min_confidence;

    let memories = match storage.list_memories("", 100_000) {
        Ok(m) => m,
        Err(e) => {
            warn!(error = %e, "Failed to list memories for eviction");
            return EvictionResult {
                memories_considered: 0,
                memories_evicted: 0,
                evicted_ids: Vec::new(),
            };
        }
    };

    let mut result = EvictionResult {
        memories_considered: 0,
        memories_evicted: 0,
        evicted_ids: Vec::new(),
    };

    for mem in &memories {
        // Check confidence threshold.
        if mem.confidence >= min_confidence {
            continue;
        }

        // Check staleness (last accessed or updated).
        let reference_time = mem
            .last_accessed
            .as_ref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|| {
                chrono::DateTime::parse_from_rfc3339(&mem.updated_at)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            })
            .or_else(|| {
                chrono::DateTime::parse_from_rfc3339(&mem.created_at)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });

        let Some(reference_time) = reference_time else {
            continue;
        };

        if reference_time > stale_threshold {
            continue;
        }

        result.memories_considered += 1;

        // Log to journal before eviction.
        let project_name = working_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let tags = storage.get_memory_tags(mem.id).unwrap_or_default();
        let tag_strs: Vec<&str> = tags.iter().map(|s| s.as_str()).collect();

        let _ = storage.create_journal_entry(
            &uuid::Uuid::new_v4().to_string(),
            &format!(
                "Memory evicted: {}",
                &mem.content[..mem.content.len().min(80)]
            ),
            &format!(
                "Evicted memory (confidence: {:.2}, category: {}):\n\n{}",
                mem.confidence, mem.category, mem.content
            ),
            &project_name,
            "system",
            &tag_strs
                .iter()
                .map(|_s| format!("eviction"))
                .chain(std::iter::once("memory-eviction".to_string()))
                .collect::<Vec<_>>(),
        );

        if auto_evict {
            // Delete the memory.
            match storage.delete_memory(mem.id) {
                Ok(_) => {
                    info!(id = mem.id, category = %mem.category, "Evicted stale memory");
                    result.memories_evicted += 1;
                    result.evicted_ids.push(mem.id);
                }
                Err(e) => {
                    warn!(id = mem.id, error = %e, "Failed to evict stale memory");
                }
            }
        } else {
            info!(
                id = mem.id,
                category = %mem.category,
                confidence = mem.confidence,
                "Stale memory identified for eviction (auto_evict disabled)"
            );
        }
    }

    if result.memories_evicted > 0 {
        info!(
            considered = result.memories_considered,
            evicted = result.memories_evicted,
            "Memory eviction complete"
        );
    }

    result
}

// ── Compaction Triggers ──────────────────────────────────────────────────────

/// Tracks when compaction was last run and how many memories have been
/// stored in the current session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionTrigger {
    /// Timestamp of the last compaction run (ISO 8601).
    pub last_compaction: Option<String>,
    /// Number of memories stored since the last compaction.
    pub memories_stored_since_compaction: usize,
    /// Maximum number of memories before triggering compaction.
    pub memory_count_threshold: usize,
    /// Minimum interval between compactions in hours.
    pub min_interval_hours: u64,
}

impl CompactionTrigger {
    /// Create a new compaction trigger with default settings.
    pub fn new(config: &CompactionConfig) -> Self {
        Self {
            last_compaction: None,
            memories_stored_since_compaction: 0,
            memory_count_threshold: config.memory_count_threshold,
            min_interval_hours: config.min_interval_hours,
        }
    }

    /// Record that a memory was stored, incrementing the counter.
    pub fn record_memory_stored(&mut self) {
        self.memories_stored_since_compaction += 1;
    }

    /// Check whether compaction should run now based on trigger conditions.
    ///
    /// Returns `true` if any of these conditions are met:
    ///
    /// 1. **Session start**: No compaction has been run yet.
    /// 2. **Time-based**: It has been more than `min_interval_hours` since
    ///    the last compaction.
    /// 3. **Count-based**: More than `memory_count_threshold` memories have
    ///    been stored since the last compaction.
    /// 4. **Total count**: The total number of stored memories exceeds
    ///    `memory_count_threshold`.
    pub fn should_compact(&self, storage: &Storage) -> bool {
        // Condition 1: Never compacted.
        if self.last_compaction.is_none() {
            debug!("Compaction trigger: first run");
            return true;
        }

        // Condition 2: Time-based.
        if let Some(ref last) = self.last_compaction {
            if let Ok(last_time) = chrono::DateTime::parse_from_rfc3339(last) {
                let hours_since = (Utc::now() - last_time.with_timezone(&Utc)).num_hours();
                if hours_since as u64 > self.min_interval_hours {
                    debug!(hours_since, "Compaction trigger: time-based");
                    return true;
                }
            }
        }

        // Condition 3: Count-based (memories stored since last compaction).
        if self.memories_stored_since_compaction > 10 {
            debug!(
                count = self.memories_stored_since_compaction,
                "Compaction trigger: count-based (>10 memories stored)"
            );
            return true;
        }

        // Condition 4: Total memory count.
        if let Ok(total) = storage.count_memories() {
            if total as usize > self.memory_count_threshold {
                debug!(
                    total,
                    threshold = self.memory_count_threshold,
                    "Compaction trigger: total count exceeds threshold"
                );
                return true;
            }
        }

        false
    }

    /// Mark compaction as completed, resetting the counter.
    pub fn mark_compacted(&mut self) {
        self.last_compaction = Some(Utc::now().to_rfc3339());
        self.memories_stored_since_compaction = 0;
    }
}

/// Run a full compaction pass: dedup, block compaction, and eviction.
///
/// This is the main entry point called by the compaction trigger system.
/// It runs deduplication, block compaction, and stale eviction in sequence.
///
/// # Returns
///
/// A [`FullCompactionResult`] summarising all actions taken.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullCompactionResult {
    /// Number of duplicates merged.
    pub duplicates_merged: usize,
    /// Block compaction result.
    pub block_compaction: CompactionResult,
    /// Eviction result.
    pub eviction: EvictionResult,
}

/// Run a full compaction pass.
pub fn run_compaction(
    storage: &Storage,
    working_dir: &Path,
    config: &MemoryConfig,
    event_bus: &Arc<EventBus>,
    session_id: &str,
) -> FullCompactionResult {
    info!("Running full memory compaction pass");

    // 1. Block compaction.
    let block_result = if config.compaction.enabled {
        compact_blocks(storage, working_dir, config)
    } else {
        CompactionResult {
            blocks_checked: 0,
            blocks_compacted: 0,
            compacted_labels: Vec::new(),
        }
    };

    // 2. Stale memory eviction.
    let eviction_result =
        evict_stale_memories(storage, &config.eviction, working_dir, config.eviction.auto);

    // 3. Deduplication pass (check all existing memories against each other).
    let duplicates_merged = deduplicate_existing_memories(storage, event_bus, session_id);

    info!(
        duplicates_merged = duplicates_merged,
        blocks_compacted = block_result.blocks_compacted,
        memories_evicted = eviction_result.memories_evicted,
        "Compaction pass complete"
    );

    FullCompactionResult {
        duplicates_merged,
        block_compaction: block_result,
        eviction: eviction_result,
    }
}

/// Find and merge duplicate memories in the existing store.
///
/// Compares each memory against others in the same category and merges
/// those with high similarity. Returns the number of merges performed.
fn deduplicate_existing_memories(
    storage: &Storage,
    event_bus: &Arc<EventBus>,
    session_id: &str,
) -> usize {
    let memories = match storage.list_memories("", 10_000) {
        Ok(m) => m,
        Err(e) => {
            warn!(error = %e, "Failed to list memories for dedup");
            return 0;
        }
    };

    let mut merged = 0;
    let mut checked: std::collections::HashSet<i64> = std::collections::HashSet::new();

    for mem in &memories {
        if checked.contains(&mem.id) {
            continue;
        }

        let tags = storage.get_memory_tags(mem.id).unwrap_or_default();
        let dedup = deduplicate_by_fts(&mem.content, &mem.category, &tags, mem.confidence, storage);

        match dedup {
            DedupResult::Duplicate {
                existing_id,
                merged_content,
                merged_confidence,
                merged_tags,
                ..
            } => {
                if existing_id != mem.id && !checked.contains(&existing_id) {
                    if apply_dedup_merge(
                        existing_id,
                        &merged_content,
                        merged_confidence,
                        &merged_tags,
                        storage,
                    )
                    .is_ok()
                    {
                        // Delete the duplicate.
                        let _ = storage.delete_memory(mem.id);
                        checked.insert(existing_id);
                        checked.insert(mem.id);
                        merged += 1;

                        event_bus.publish(Event::MemoryCandidateExtracted {
                            session_id: session_id.to_string(),
                            content: format!("Merged duplicate memory into ID {}", existing_id),
                            category: "compaction".to_string(),
                            tags: vec!["dedup".to_string()],
                            confidence: merged_confidence,
                            source: "auto-compact/dedup".to_string(),
                            reason: "Automatically merged duplicate memories".to_string(),
                        });
                    }
                }
            }
            DedupResult::NearDuplicate { .. } => {
                // Near-duplicates require user confirmation — just note them.
                debug!(id = mem.id, "Near-duplicate found, requires confirmation");
            }
            DedupResult::NoDuplicate => {}
        }
    }

    merged
}

use crate::event::Event;
