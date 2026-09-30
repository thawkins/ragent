//! Terminal-escape and control-character neutralisation.
//!
//! SEC-ragent-types-001 (SECTASKS T-023): the shared sanitizer crate exposed
//! only secret redaction, HTML-tag stripping, and UTF-8-safe truncation. None
//! of them removed C0/C1 control characters or ESC-initiated sequences, so an
//! LLM-authored tool result, a fetched web page, a repository file, or an MCP
//! response containing `\x1b]52;c;<base64>\x07` (OSC-52 clipboard write),
//! `\x1b]0;title\x07` (window title), or `\x1b[2J` (clear screen) was written
//! straight into a ratatui `Span` and interpreted by the terminal emulator -
//! allowing rendered output to be overwritten and a spoofed
//! "permission granted" line to be drawn.
//!
//! [`sanitize_terminal`] performs one pass that drops control characters
//! (except `\n` and `\t`), removes bidi-override characters, and consumes whole
//! ESC sequences (CSI/OSC/DCS/APC/PM/SOS) plus bare C1 introducers.

use std::borrow::Cow;

/// Neutralise terminal escape sequences and control characters in `input`.
///
/// Returns [`Cow::Borrowed`] when the input needs no change, so the common case
/// (plain text) allocates nothing.
///
/// Kept characters: `\n` (line feed) and `\t` (horizontal tab). Every other
/// C0 control character (`0x00`-`0x1F`), `DEL` (`0x7F`), and C1 control
/// character (`0x80`-`0x9F`) is dropped. Unicode bidi-override and
/// format characters (U+202A-U+202E, U+2066-U+2069) are dropped too, because
/// they can reorder rendered text to disguise content.
///
/// # Examples
///
/// ```
/// use ragent_types::sanitize_terminal::sanitize_terminal;
///
/// let cleaned = sanitize_terminal("\x1b]0;evil\x07hello");
/// assert_eq!(cleaned, "hello");
///
/// let cleaned = sanitize_terminal("keep\n\tthis");
/// assert_eq!(cleaned, "keep\n\tthis");
/// ```
#[must_use]
pub fn sanitize_terminal(input: &str) -> Cow<'_, str> {
    if !needs_sanitising(input) {
        return Cow::Borrowed(input);
    }
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => {
                // ESC: consume the whole sequence it introduces.
                consume_escape(&mut chars);
            }
            '\n' | '\t' => out.push(c),
            c if is_droppable_control(c) => {}
            c if is_bidi_or_format_override(c) => {}
            c => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// Cheap pre-scan: does `input` contain anything the sanitiser would remove?
fn needs_sanitising(input: &str) -> bool {
    input
        .chars()
        .any(|c| c == '\u{1b}' || is_droppable_control(c) || is_bidi_or_format_override(c))
}

/// Drop-all C0/C1 controls except the two the renderer handles itself.
fn is_droppable_control(c: char) -> bool {
    matches!(c, '\u{00}'..='\u{1f}' | '\u{7f}' | '\u{80}'..='\u{9f}') && c != '\n' && c != '\t'
}

/// Bidi-override and directional-format characters that can reorder text.
fn is_bidi_or_format_override(c: char) -> bool {
    matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// Consume the byte sequence that follows an ESC introducer.
///
/// - `ESC [` (CSI): parameters/intermediates until a final byte `0x40`-`0x7E`.
/// - `ESC ]` (OSC): until `BEL` or `ESC \` (ST).
/// - `ESC P` / `ESC X` / `ESC ^` / `ESC _` (DCS/APC/PM/SOS): until `ESC \`.
/// - Anything else: consume a single following character.
fn consume_escape(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    match chars.peek().copied() {
        Some('[') => {
            chars.next();
            for c in chars.by_ref() {
                if matches!(c, '\u{40}'..='\u{7e}') {
                    break;
                }
            }
        }
        Some(']') => {
            chars.next();
            while let Some(c) = chars.next() {
                match c {
                    '\u{7}' => break,
                    '\u{1b}' => {
                        // Expect the string terminator `ESC \`.
                        let _ = chars.next();
                        break;
                    }
                    _ => {}
                }
            }
        }
        Some('P' | 'X' | '^' | '_') => {
            chars.next();
            while let Some(c) = chars.next() {
                if c == '\u{1b}' {
                    let _ = chars.next();
                    break;
                }
            }
        }
        Some(_) => {
            chars.next();
        }
        None => {}
    }
}
