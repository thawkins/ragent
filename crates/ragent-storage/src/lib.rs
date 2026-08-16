//! `SQLite` persistence layer for ragent
//!
//! This crate provides:
//! - Session storage (conversations, messages, history)
//! - Memory storage (memory blocks, structured memories, embeddings)
//! - Snapshot storage (file snapshots, diffs)
//! - Team storage (team state, mailboxes, tasks)

pub mod snapshot;
pub mod storage;

// Re-export commonly used types
pub use snapshot::{IncrementalSnapshot, Snapshot};
pub use storage::{
    BackgroundTaskRow, ConversationStats, CronEventRow, CycleError, EmbeddingMatch,
    InitiativeMilestone, InitiativeRow, KgEntityRow, KgRelationshipRow, MemoryRow,
    MessageEmbeddingMatch, MessageSearchResult, RunCostSummaryRow, SessionRow, SessionSearchParams,
    Storage, TaskDerived, TaskRow, TaskUpdateParams, TaskView, compute_task_dag, decrypt_key,
    deobfuscate_key, detect_cycle, encrypt_key, obfuscate_key,
};

// Backward-compatible aliases for code that has not yet migrated (e.g.
// `crates/ragent-tui`).  These are intentionally narrow and do NOT
// represent the canonical API — new code should use `TaskRow` and the
// `*_simple` / full task methods.
#[doc(hidden)]
pub use storage::TaskRow as TodoRow;

// `Storage::get_todos` is kept as an inherent method alias in
// `storage.rs`; see there for the deprecated shim.

// Re-export sanitize module from ragent_types
pub use ragent_types::sanitize;
