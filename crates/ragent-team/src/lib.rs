//! Team runtime and team coordination tools for ragent.
//!
//! This crate owns the Milestone 8 extracted team layer while using
//! compatibility re-exports from `ragent-agent` for the shared orchestration
//! types that the copied team runtime still depends on.

pub use ragent_agent::{agent, config, event, message, session};

pub mod team;
pub mod tool;
pub mod tools;

pub use team::*;
