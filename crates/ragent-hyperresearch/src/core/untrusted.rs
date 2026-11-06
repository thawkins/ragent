//! Untrusted-content fencing (FR-015).
//!
//! Every string that originated from a network fetch must pass through this
//! module before reaching a prompt or a rendered page. The fence delimits the
//! content with an explicit marker carrying its source URL, so an agent reading
//! the text can see exactly where attacker-controlled bytes begin and end.

/// Marker that opens a fenced block.
pub const FENCE_OPEN: &str = "<<<BEGIN UNTRUSTED CONTENT";

/// Marker that closes a fenced block.
pub const FENCE_CLOSE: &str = "<<<END UNTRUSTED CONTENT>>>";

/// A piece of network-originated text, already fenced.
///
/// The inner text is deliberately not exposed as a `&str` accessor that could
/// be interpolated into a prompt by accident: callers display the wrapper.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Untrusted {
    source: String,
    body: String,
}

impl Untrusted {
    /// Fence `body`, attributing it to `source`.
    ///
    /// Any fence markers already present in `body` are neutralised, so fetched
    /// text cannot close the fence early and escape into the surrounding
    /// prompt.
    pub fn fence(source: &str, body: &str) -> Self {
        Self {
            source: sanitise_marker(source),
            body: sanitise_marker(body),
        }
    }

    /// The source URL (or other provenance) this content came from.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Render the fenced block, ready to place in a prompt or a page.
    pub fn render(&self) -> String {
        format!(
            "{FENCE_OPEN} source={}>>>\n{}\n{FENCE_CLOSE}",
            self.source, self.body
        )
    }
}

impl std::fmt::Display for Untrusted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render())
    }
}

/// Break up any embedded fence marker so it cannot terminate the block.
fn sanitise_marker(text: &str) -> String {
    text.replace(FENCE_OPEN, "<<<BEGIN UNTRUSTED CONTENT[escaped]")
        .replace(FENCE_CLOSE, "<<<END UNTRUSTED CONTENT[escaped]>>>")
}
