//! Percent-encoding helpers for VCS API query/path components (FUNC-063).
//!
//! These replace the hand-rolled encoders that iterated over `char`s and emitted
//! `%{:02X}` on the char's scalar value — which is wrong for any non-ASCII
//! character (e.g. `é` is U+00E9, but must encode as the two UTF-8 bytes
//! `%C3%A9`, not `%E9`). Encoding over `&str::bytes()` produces the correct
//! UTF-8 percent-encoding.

/// Percent-encode a single URI component, leaving RFC 3986 unreserved
/// characters (`A–Z`, `a–z`, `0–9`, `-`, `_`, `.`, `~`) untouched.
///
/// Every other byte (including every byte of a multibyte UTF-8 character) is
/// emitted as `%XX`. Used for query-parameter values and path segments that may
/// contain non-ASCII text or reserved characters such as `&`, `=`, `#`, or `/`.
#[must_use]
pub fn encode_component(input: &str) -> String {
    use std::fmt::Write as _;
    // Worst case is 3 bytes out per byte in (`%XX`); 1.5x avoids most reallocations
    // without over-reserving for typical ASCII-dominated inputs.
    let mut out = String::with_capacity(input.len() + input.len() / 2);
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            // write! into a String is infallible.
            _ => {
                write!(out, "%{byte:02X}").expect("BUG: write to String is infallible");
            }
        }
    }
    out
}

/// Percent-encode a GitLab project path, encoding the `/` separators as `%2F`
/// so a nested `namespace/project` is passed as a single `:id` path segment.
///
/// FUNC-063: delegates to [`encode_component`] so non-ASCII namespace segments
/// encode correctly (the previous implementation only replaced `/`).
#[must_use]
pub fn encode_project_path(path: &str) -> String {
    encode_component(path)
}
