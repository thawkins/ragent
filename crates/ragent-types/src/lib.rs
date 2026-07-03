//! Core types and traits for ragent
//!
//! This crate provides the foundation types used across all ragent crates:
//! - Message and conversation types
//! - Error types
//! - ID generation
//! - Event bus
//! - LLM provider traits
//! - Resource management
//! - Utility functions

pub mod error;
pub mod event;
pub mod id;
pub mod llm;
pub mod message;
pub mod permission;
pub mod resource;
pub mod sanitize;
pub mod strutil;
pub mod thinking;

// Re-export commonly used types
pub use error::RagentError;
pub use event::{Event, EventBus};
pub use id::{MessageId, SessionId};
pub use llm::{
    ChatContent, ChatMessage, ChatRequest, ContentPart, LlmFinishReason, StreamEvent,
    ToolDefinition,
};
pub use message::{ImageData, Message, MessagePart, Role, ToolCallState, ToolCallStatus};
pub use permission::PermissionDecision;
pub use thinking::{ThinkingConfig, ThinkingDisplay, ThinkingLevel};
// Re-export string utilities for convenient access
pub use strutil::{truncate_bytes, truncate_chars};
