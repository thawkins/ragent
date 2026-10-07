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
//! - Security guards (`guard`)
//! - Activity-log event schema and types
//! - Cron scheduling types

pub mod activity;
pub mod cron;
pub mod embedding;
pub mod error;
pub mod event;
/// SEC-ragent-*-001 (SECTASKS MS-05 T-067/T-068): shared security guards.
pub mod guard;
pub mod html;
pub mod id;
pub mod llm;
pub mod message;
pub mod panic_guard;
pub mod permission;
pub mod resource;
/// Secret-redaction helpers for scrubbing sensitive patterns from text.
pub mod sanitize;
/// SEC-ragent-types-001 (SECTASKS T-023): terminal-escape neutralisation.
pub mod sanitize_terminal;
pub mod startup;
pub mod stderr_spool;
pub mod strutil;
pub mod thinking;
pub mod trigger;

// Re-export commonly used types
pub use activity::{
    ACTIVITY_EVENT_SCHEMA_VERSION, ActivityEvent, BoundaryTarget, ConsistencyError, EventKind,
    Principal, ProjectedCheckpoint, ProjectedMessage, ProjectedPermission, ProjectedToolCall,
    ProjectedToolResult, Projection, ResumeResult, RollbackResult, RunStatus, TerminationReason,
    validate_event_log_consistency,
};
pub use cron::{
    CronEvent, CronForm, CronSchedule, DurationParseError, ParsedSchedule, ScheduleParseError,
    parse_duration, parse_schedule,
};
pub use error::RagentError;
pub use event::{Event, EventBus};
// Shared security guards (SECTASKS MS-05): one implementation for the guard
// shapes each earlier milestone re-implemented per crate.
pub use guard::{
    MAX_IDENTIFIER_LEN, MAX_RETRY_AFTER, cap_read, clamp_retry_after, contained_join,
    is_safe_operand, reject_option_like, validate_identifier, validate_relative_component,
};
pub use id::{EventId, MessageId, RunId, SessionId};
pub use llm::{
    ChatContent, ChatMessage, ChatRequest, ContentPart, LlmFinishReason, StreamEvent,
    ToolDefinition,
};
pub use message::{ImageData, Message, MessagePart, Role, ToolCallState, ToolCallStatus};
pub use permission::PermissionDecision;
pub use startup::StartupTimings;
pub use thinking::{ThinkingConfig, ThinkingDisplay, ThinkingLevel};
// Re-export string utilities for convenient access
pub use strutil::{
    floor_char_boundary, format_size, truncate_bytes, truncate_bytes_no_ellipsis, truncate_chars,
};
pub use trigger::{
    TriggerActionKind, TriggerEnvelope, TriggerFired, TriggerRule, TriggerRuleId,
    TriggerRuleStatus, TriggerSourceKind,
};
