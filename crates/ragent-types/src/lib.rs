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
pub mod intern;
pub mod llm;
pub mod message;
pub mod permission;
pub mod resource;
pub mod sanitize;
pub mod thinking;

// Re-export commonly used types
pub use error::RagentError;
pub use event::{Event, EventBus};
pub use id::{MessageId, SessionId};
pub use llm::{LlmProvider, LlmResponse, ModelInfo, ProviderConfig, ToolDefinition};
pub use message::{Message, Role};
pub use permission::{Permission, PermissionDecision, PermissionRequest};
pub use thinking::{ThinkingConfig, ThinkingDisplay, ThinkingLevel};
