//! Shared zebra-band and block-cursor row painting for the popup tables.
//!
//! The Agents window ([`crate::layout_active_agents`]) and the Teams window
//! ([`crate::layout_teams`]) paint their tables with the same two decorations:
//! a subtle background on alternating rows, and a brighter block cursor on the
//! keyboard-selected row. Both used to carry a byte-identical copy of
//! [`cursor_line`], [`band_line`], [`BAND_BG`] and [`CURSOR_BG`], so a change to
//! either tone or to the pad-width arithmetic had to be made twice.
//!
//! These definitions live here so the two windows cannot drift apart: the
//! "selection reads identically in both windows" property is now structural
//! rather than a comment promising it.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Subtle band background painted on alternating table rows.
///
/// The modal clears to black underneath, so a near-black blue-grey stripe on
/// every other data row separates neighbouring rows without shouting; the
/// strip's two background tones sit in the same family for a coherent panel.
pub(crate) const BAND_BG: Color = Color::Rgb(22, 22, 30);

/// Block-cursor background painted on the keyboard-selected row.
///
/// A slate blue that is clearly lighter than [`BAND_BG`] (so a cursor row never
/// reads as a band) but stays dim enough that the row text remains legible; the
/// same tone family as the teammate-strip focus highlight.
pub(crate) const CURSOR_BG: Color = Color::Rgb(50, 50, 90);

/// Number of display columns a line has already painted.
///
/// Counts `char`s rather than bytes so a multi-byte glyph contributes one
/// column, which is what the pad arithmetic below needs.
fn painted_width(line: &Line) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

/// Paint the block cursor over one painted table row, full table width.
///
/// The cursor is the selection affordance replaced by keyboard navigation
/// (`Up`/`Down`/`PgUp`/`PgDn` move it, `Space`/`Del`/`Enter` act on it), so it
/// overrides the zebra band: the row is restyled with one distinct background
/// across every span, and a trailing pad fills the unpainted remainder edge to
/// edge (the same trick [`band_line`] uses) so the cursor rectangle reads as one
/// block.
pub(crate) fn cursor_line(line: Line, width: usize) -> Line {
    let painted = painted_width(&line);
    let mut spans: Vec<Span> = line
        .spans
        .into_iter()
        .map(|span| {
            Span::styled(
                span.content.to_string(),
                span.style.bg(CURSOR_BG).add_modifier(Modifier::BOLD),
            )
        })
        .collect();
    if width > painted {
        spans.push(Span::styled(
            " ".repeat(width - painted),
            Style::default().bg(CURSOR_BG),
        ));
    }
    Line::from(spans).style(Style::default().bg(CURSOR_BG))
}

/// Give one painted table row the zebra-band background across the full width.
///
/// A trailing background-coloured spacer fills the unpainted remainder so the
/// band covers the row from edge to edge instead of stopping at the last
/// control column. The cut never panics on a row wider than `width`: there is
/// simply nothing left to pad.
pub(crate) fn band_line(line: Line, width: usize) -> Line {
    let painted = painted_width(&line);
    let mut spans = line.spans;
    if width > painted {
        spans.push(Span::styled(
            " ".repeat(width - painted),
            Style::default().bg(BAND_BG),
        ));
    }
    Line::from(spans).style(Style::default().bg(BAND_BG))
}
