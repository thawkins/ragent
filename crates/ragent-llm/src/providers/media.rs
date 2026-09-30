//! Shared helpers for decoding `data:` URIs carrying inline media.
//!
//! Anthropic and Bedrock (Anthropic Messages API) both inline image/PDF
//! attachments as `data:<mime>;base64,<payload>` URIs and previously carried
//! byte-identical copies of these two parsers (see `ANTIPAT.md` M3.6).

/// Extract the MIME type from a `data:<mime>;base64,<data>` URI.
///
/// Returns `None` when the URI does not begin with `data:`.
#[must_use]
pub fn extract_mime_from_data_uri(uri: &str) -> Option<&str> {
    uri.strip_prefix("data:").and_then(|s| s.split(';').next())
}

/// Extract the raw base64 payload from a `data:<mime>;base64,<data>` URI.
///
/// Returns `None` when the URI carries no comma-separated payload.
#[must_use]
pub fn extract_base64_from_data_uri(uri: &str) -> Option<&str> {
    uri.find(",base64,")
        .map(|i| &uri[i + 8..])
        .or_else(|| uri.find(',').map(|i| &uri[i + 1..]))
}
