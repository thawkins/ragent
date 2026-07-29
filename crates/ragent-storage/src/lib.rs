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
    BackgroundTaskRow, ConversationStats, EmbeddingMatch, InitiativeMilestone, InitiativeRow,
    KgEntityRow, KgRelationshipRow, MemoryRow, MessageEmbeddingMatch, MessageSearchResult,
    RunCostSummaryRow, SessionRow, SessionSearchParams, Storage, TodoRow, decrypt_key,
    deobfuscate_key, encrypt_key, obfuscate_key,
};

// Re-export sanitize module from ragent_types
pub use ragent_types::sanitize;
