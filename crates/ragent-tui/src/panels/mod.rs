//! TUI overlay panels for memory browsing and journal viewing.

pub mod journal_viewer;
pub mod memory_browser;

pub use journal_viewer::{JournalViewerState, render_journal_viewer};
pub use memory_browser::{MemoryBrowserState, render_memory_browser};
