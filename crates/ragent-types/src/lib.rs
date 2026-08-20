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
//! - Cron scheduling types

pub mod cron;
pub mod error;
pub mod event;
pub mod html;
pub mod id;
pub mod llm;
pub mod message;
pub mod panic_guard;
pub mod permission;
pub mod resource;
pub mod sanitize;
pub mod startup;
pub mod strutil;
pub mod thinking;
pub mod trigger;

// Re-export commonly used types
pub use cron::{
    CronEvent, CronForm, CronSchedule, DurationParseError, ParsedSchedule, ScheduleParseError,
    parse_duration, parse_schedule,
};
pub use error::RagentError;
pub use event::{Event, EventBus};
pub use id::{MessageId, SessionId};
pub use llm::{
    ChatContent, ChatMessage, ChatRequest, ContentPart, LlmFinishReason, StreamEvent,
    ToolDefinition,
};
pub use message::{ImageData, Message, MessagePart, Role, ToolCallState, ToolCallStatus};
pub use permission::PermissionDecision;
pub use startup::StartupTimings;
pub use thinking::{ThinkingConfig, ThinkingDisplay, ThinkingLevel};
// Re-export string utilities for convenient access
pub use strutil::{truncate_bytes, truncate_bytes_no_ellipsis, truncate_chars};
pub use trigger::{
    TriggerActionKind, TriggerEnvelope, TriggerFired, TriggerRule, TriggerRuleId,
    TriggerRuleStatus, TriggerSourceKind,
};
