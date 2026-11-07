//! Shared result formatting (FR-024).
//!
//! One module emits every command result so plain and JSON output cannot drift
//! apart between subcommands.

/// Output format selected on the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Format {
    /// Human-readable text.
    #[default]
    Plain,
    /// Machine-readable JSON.
    Json,
}

/// Emit `value` as JSON on stdout.
///
/// # Errors
///
/// Returns a JSON error when the value cannot be serialised.
pub fn emit_json(value: &impl serde::Serialize) -> crate::core::error::Result<()> {
    let text = serde_json::to_string_pretty(value)?;
    ragent_types::cliprintln!("{text}");
    Ok(())
}
