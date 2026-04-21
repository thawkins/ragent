//! `memory_replace` — Re-export of the memory replace tool from the memory_write module.
//!
//! The [`MemoryReplaceTool`] is implemented in [`super::memory_write`] alongside
//! the other memory tools. This module exists purely to keep the tool module
//! structure consistent (one module per tool).

pub use super::memory_write::MemoryReplaceTool;
