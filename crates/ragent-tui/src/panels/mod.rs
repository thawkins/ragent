//! TUI overlay panels for memory browsing and the internal-LLM chat window.

pub mod internal_llm_chat;
pub mod memory_browser;

pub use internal_llm_chat::{InternalLlmChatState, render_internal_llm_chat};
pub use memory_browser::{MemoryBrowserState, render_memory_browser};
