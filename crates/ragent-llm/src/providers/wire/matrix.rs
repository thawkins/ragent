//! The FR-016 wire matrix: every registered provider's declared contract as one
//! aligned ASCII table.
//!
//! FR-016 asks for a printable, per-provider view of the nine
//! [`ToolWireProfile`] fields, produced without contacting a provider and
//! without executing a tool. This module is the renderer; the `--wire-matrix`
//! flag in the `ragent` binary is the caller that supplies the registered
//! provider ids (see [`render`]).
//!
//! Two properties make the table worth printing rather than cosmetic:
//!
//! - **Every cell is a declared value, never a default.** The cells come from
//!   the same [`profile_for`] lookup the encoders and the accumulator dispatch
//!   on, so the table cannot disagree with what a session actually sends. A
//!   provider with no row is reported as an error naming it rather than printed
//!   with blank cells, because a blank cell would be the silent wrong answer
//!   FR-002 exists to forbid.
//! - **Nothing is blank.** Every label is a fixed ASCII string
//!   ([`ToolEnvelope::as_str`] and its siblings, or `yes`/`no`), which is what
//!   NFR-005 requires of the printed table.
//!
//! The renderer is pure: it takes provider ids and reads the `const` table. No
//! registry is built here, no environment is read, no I/O happens, so a test can
//! assert the exact bytes.

use std::fmt;

use super::ToolWireProfile;
use super::profile_table::profile_for;

/// The column headings, in print order: the provider id, then the nine
/// [`ToolWireProfile`] fields of FR-016.
///
/// One entry per printed column, so a column added to [`render`] without its
/// heading (or the reverse) is a compile error rather than a misaligned table.
pub const HEADINGS: [&str; 10] = [
    "PROVIDER",
    "ENVELOPE",
    "CARRIER",
    "ID FIELD",
    "ARGUMENTS",
    "STREAM KEYING",
    "PARALLEL",
    "IS ERROR",
    "REASONING",
    "SYNTHETIC IDS",
];

/// One or providers were asked for a contract they never declared (FR-002).
///
/// Returned instead of printing a row with blank cells: a provider with no
/// declared profile is a build error the coverage assertion in
/// `tests/test_wire_profile_table.rs` catches, and if one ever reaches the CLI
/// the table must say so rather than describe a contract nothing speaks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingProfile {
    /// The provider ids that have no row in
    /// [`PROVIDER_WIRE`](super::profile_table::PROVIDER_WIRE), in the order
    /// they were supplied.
    pub providers: Vec<String>,
}

impl fmt::Display for MissingProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "no tool-calling profile declared for provider(s): {}",
            self.providers.join(", ")
        )
    }
}

impl std::error::Error for MissingProfile {}

/// Render the wire matrix for `provider_ids` (FR-016).
///
/// One row per supplied id, sorted by id so the printed table does not depend on
/// the order a registry happens to enumerate (a `HashMap`, so it would otherwise
/// change between runs). Columns are left-aligned, padded to the widest cell of
/// each column, and separated by two spaces; the heading is followed by a rule of
/// `-` the width of the table.
///
/// # Errors
///
/// Returns [`MissingProfile`] naming every supplied id that has no declared
/// profile. Nothing is rendered in that case, so a partial table is never
/// printed as if it were complete.
///
/// # Examples
///
/// ```
/// use ragent_llm::provider::wire::matrix::{MissingProfile, render};
///
/// fn main() -> Result<(), MissingProfile> {
///     let table = render(&["gemini", "openai"])?;
///     let lines: Vec<&str> = table.lines().collect();
///     assert!(lines[0].starts_with("PROVIDER"));
///     assert!(lines[1].starts_with("---"));
///     assert!(lines[2].starts_with("gemini"));
///     // Sorted by provider id, not by the order supplied.
///     assert!(lines[3].starts_with("openai"));
///     Ok(())
/// }
/// ```
pub fn render(provider_ids: &[&str]) -> Result<String, MissingProfile> {
    let mut rows: Vec<(&str, ToolWireProfile)> = Vec::with_capacity(provider_ids.len());
    let mut missing: Vec<String> = Vec::new();
    for id in provider_ids {
        match profile_for(id) {
            Some(profile) => rows.push((id, profile)),
            None => missing.push((*id).to_string()),
        }
    }
    if !missing.is_empty() {
        return Err(MissingProfile { providers: missing });
    }
    rows.sort_by(|a, b| a.0.cmp(b.0));
    Ok(render_rows(&rows))
}

/// Lay out already-resolved rows (the print half of [`render`]).
fn render_rows(rows: &[(&str, ToolWireProfile)]) -> String {
    let table: Vec<[String; HEADINGS.len()]> = rows
        .iter()
        .map(|(id, profile)| {
            [
                (*id).to_string(),
                profile.tool_envelope.as_str().to_string(),
                profile.result_carrier.as_str().to_string(),
                profile.id_field.as_str().to_string(),
                profile.args_encoding.as_str().to_string(),
                profile.stream_keying.as_str().to_string(),
                yes_no(profile.supports_parallel).to_string(),
                yes_no(profile.supports_is_error).to_string(),
                yes_no(profile.echo_reasoning).to_string(),
                yes_no(profile.synthetic_ids).to_string(),
            ]
        })
        .collect();

    // Column widths come from the widest cell *including* the heading, so a
    // long heading never pushes the first data row out of alignment.
    let mut widths = [0usize; HEADINGS.len()];
    for (index, heading) in HEADINGS.iter().enumerate() {
        widths[index] = heading.chars().count();
    }
    for row in &table {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }

    let mut out = String::new();
    push_row(&mut out, &HEADINGS.map(str::to_string), &widths);
    let rule: usize = widths.iter().sum::<usize>() + 2 * (widths.len() - 1);
    out.push_str(&"-".repeat(rule));
    out.push('\n');
    for row in &table {
        push_row(&mut out, row, &widths);
    }
    out
}

/// Append one padded, space-separated row plus its newline.
///
/// The last column is padded like the others so every line is exactly the rule's
/// width, then trailing spaces are trimmed: the table aligns without any line
/// carrying invisible trailing whitespace.
fn push_row(out: &mut String, cells: &[String; HEADINGS.len()], widths: &[usize; HEADINGS.len()]) {
    let mut line = String::new();
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            line.push_str("  ");
        }
        line.push_str(cell);
        for _ in cell.chars().count()..widths[index] {
            line.push(' ');
        }
    }
    out.push_str(line.trim_end());
    out.push('\n');
}

/// The printed spelling of a boolean field: never blank (NFR-005).
const fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}
