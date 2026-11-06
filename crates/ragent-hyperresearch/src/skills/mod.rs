//! Embedded skill pipeline (FR-027) and its golden stability check (FR-028).
//!
//! The 18 numbered pipeline stages plus the umbrella skill are embedded with
//! `include_str!` and exposed by name. The golden fixture makes prompt drift a
//! visible diff rather than a silent change.

pub mod pipeline;
