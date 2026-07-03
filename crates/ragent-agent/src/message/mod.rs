//! Conversation message types.
//!
//! This module is a thin re-export of the canonical implementation in
//! [`ragent_types::message`]. The agent crate previously held its own copy of
//! `Message` / `MessagePart` / `ImageData`; those have been consolidated into
//! `ragent-types` to eliminate the duplication (see `REMPLAN.md` M1 / T1.1).
//!
//! All types below are re-exported verbatim, so existing `use crate::message::*`
//! sites continue to resolve unchanged.

pub use ragent_types::message::{
    ImageData, Message, MessagePart, Role, ToolCallState, ToolCallStatus,
};