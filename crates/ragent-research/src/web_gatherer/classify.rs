//! URL/source classification — classify a web URL by content type and host
//! (page, PDF, YouTube).
//!
//! These helpers were previously inline in `web_gatherer.rs`.

/// Classified kind of a captured web source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebSourceKind {
    /// A normal web page (article, blog post, documentation, etc.).
    Page,
    /// A PDF document detected by `Content-Type` or URL extension.
    Pdf,
    /// A YouTube video URL. When the fetch layer extracts a transcript the
    /// captured body contains the caption text; otherwise the body contains the
    /// watch-page chrome and description.
    YouTube,
}

/// Classify a web URL by its `Content-Type` and host.
///
/// PDFs are recognised by an `application/pdf` content type or by a `.pdf`
/// path extension. YouTube URLs are recognised by host (`youtube.com` or
/// `youtu.be`). Everything else is treated as a generic page.
#[must_use]
pub fn classify_web_source(url: &str, content_type: Option<&str>) -> WebSourceKind {
    if content_type.is_some_and(|ct| ct.to_ascii_lowercase().contains("application/pdf"))
        || url.to_ascii_lowercase().ends_with(".pdf")
    {
        return WebSourceKind::Pdf;
    }
    if let Ok(parsed) = url::Url::parse(url) {
        let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
        if host.contains("youtube.com") || host.contains("youtu.be") {
            return WebSourceKind::YouTube;
        }
    }
    WebSourceKind::Page
}

impl WebSourceKind {
    #[allow(dead_code)]
    /// Human-readable classifier used when serialising web sources.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Page => "page",
            Self::Pdf => "pdf",
            Self::YouTube => "youtube",
        }
    }
}
