//! Constitution artifact parsing for the spec system.
//!
//! This module implements the `Constitution` struct and parser for
//! `CONSTITUTION.md` files (FR-007). A constitution is a markdown file in the
//! specs root directory containing immutable architectural principles
//! (articles) that govern generated implementations.
//!
//! ## Supported Format
//!
//! ```markdown
//! # Constitution
//!
//! ## Article 1: Library-First
//!
//! Prefer composing from small libraries over building monolithic subsystems.
//!
//! ## Article 2: Simplicity
//!
//! ...
//!
//! ## Amendment Log
//!
//! | Date       | Article   | Rationale | Compatibility |
//! |------------|-----------|-----------|---------------|
//! | 2025-01-15 | Article 3 | ...       | ...           |
//! ```
//!
//! Articles are identified by `## Article <n>: <title>` or
//! `## Article <n> — <title>` headings. The amendment log is a markdown table
//! under a `## Amendment Log` heading.

use crate::error::SpecError;
use std::path::{Path, PathBuf};

/// An immutable architectural principle from the constitution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Article {
    /// Article number (1-based, parsed from the heading).
    pub number: u32,
    /// Short title of the article (e.g., "Library-First").
    pub title: String,
    /// Full description / principle text (trimmed).
    pub body: String,
}

/// A dated constitutional amendment entry from the amendment log (FR-016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Amendment {
    /// ISO-8601 date string (e.g., "2025-01-15").
    pub date: String,
    /// Which article was amended (e.g., "Article 3").
    pub article: String,
    /// Rationale for the amendment.
    pub rationale: String,
    /// Backward-compatibility assessment.
    pub compatibility: String,
}

/// Parsed representation of a `CONSTITUTION.md` file (FR-007).
///
/// Contains the raw markdown content alongside structured article and
/// amendment data. When no `CONSTITUTION.md` exists, [`Constitution::empty`]
/// returns a value with no articles — callers should treat this as "no
/// constitution configured" and not block validation (FR-018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constitution {
    /// Raw markdown content of the file.
    pub content: String,
    /// Parsed articles, one per `## Article N: Title` heading.
    pub articles: Vec<Article>,
    /// Parsed amendment log entries, if an `## Amendment Log` section exists.
    pub amendments: Vec<Amendment>,
}

impl Constitution {
    /// Create an empty constitution (no articles, no amendments).
    ///
    /// Used when `CONSTITUTION.md` does not exist (FR-018 backward
    /// compatibility).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            content: String::new(),
            articles: Vec::new(),
            amendments: Vec::new(),
        }
    }

    /// Returns `true` if the constitution has no articles.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.articles.is_empty()
    }

    /// Look up an article by its 1-based number.
    #[must_use]
    pub fn article_by_number(&self, number: u32) -> Option<&Article> {
        self.articles.iter().find(|a| a.number == number)
    }

    /// Look up an article by its title (case-sensitive).
    #[must_use]
    pub fn article_by_title(&self, title: &str) -> Option<&Article> {
        self.articles.iter().find(|a| a.title == title)
    }

    /// Get the path to `CONSTITUTION.md` relative to the specs root.
    #[must_use]
    pub fn path(specs_root: &Path) -> PathBuf {
        specs_root.join("CONSTITUTION.md")
    }

    /// Apply a constitutional amendment, returning updated markdown (FR-016).
    ///
    /// Validates that the amendment request has:
    /// - A non-empty date in ISO-8601 format (`YYYY-MM-DD`)
    /// - A non-empty article reference
    /// - A non-empty rationale (explicit justification)
    /// - A non-empty compatibility assessment (backward-compatibility)
    ///
    /// The amendment is appended as a new row in the `## Amendment Log`
    /// table. If no such section exists, one is created at the end of the
    /// document.
    ///
    /// # Errors
    ///
    /// Returns [`SpecError::AmendmentError`] if the date format is invalid,
    /// or if rationale/compatibility are empty.
    pub fn apply_amendment(&self, request: &AmendmentRequest) -> Result<String, SpecError> {
        validate_amendment_date(&request.date)?;
        if request.article.trim().is_empty() {
            return Err(SpecError::AmendmentError(
                "amendment article reference is required".to_string(),
            ));
        }
        if request.rationale.trim().is_empty() {
            return Err(SpecError::AmendmentError(
                "amendment rationale is required (FR-016)".to_string(),
            ));
        }
        if request.compatibility.trim().is_empty() {
            return Err(SpecError::AmendmentError(
                "amendment backward-compatibility assessment is required (FR-016)".to_string(),
            ));
        }
        Ok(self.append_amendment_row(request))
    }

    /// Validate all amendments in the log have explicit rationale and
    /// compatibility assessment (FR-016).
    ///
    /// Returns a list of [`AmendmentIssue`] for any amendments missing
    /// required fields. An empty vec means all amendments are valid.
    #[must_use]
    pub fn validate_amendments(&self) -> Vec<AmendmentIssue> {
        let mut issues = Vec::new();
        for am in &self.amendments {
            if am.rationale.trim().is_empty() {
                issues.push(AmendmentIssue {
                    date: am.date.clone(),
                    article: am.article.clone(),
                    field: "rationale",
                    message: format!(
                        "amendment dated {} on {} is missing rationale",
                        am.date, am.article
                    ),
                });
            }
            if am.compatibility.trim().is_empty() {
                issues.push(AmendmentIssue {
                    date: am.date.clone(),
                    article: am.article.clone(),
                    field: "compatibility",
                    message: format!(
                        "amendment dated {} on {} is missing backward-compatibility assessment",
                        am.date, am.article
                    ),
                });
            }
            if am.date.trim().is_empty() {
                issues.push(AmendmentIssue {
                    date: am.date.clone(),
                    article: am.article.clone(),
                    field: "date",
                    message: format!("amendment on {} is missing a date", am.article),
                });
            }
        }
        issues
    }

    /// Append a new amendment row to the `## Amendment Log` table.
    ///
    /// If the section already exists, the row is inserted after the last
    /// table row. If no section exists, a new `## Amendment Log` section
    /// with header and separator is appended at the end.
    fn append_amendment_row(&self, request: &AmendmentRequest) -> String {
        let new_row = format!(
            "| {} | {} | {} | {} |",
            request.date, request.article, request.rationale, request.compatibility
        );

        let content = self.content.as_str();

        // Find the ## Amendment Log section
        let mut lines: Vec<&str> = content.lines().collect();
        let mut section_start: Option<usize> = None;
        let mut last_table_row: Option<usize> = None;
        let mut section_end: Option<usize> = None;

        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("## ") {
                if section_start.is_some() && section_end.is_none() {
                    section_end = Some(i);
                }
                if trimmed == "## Amendment Log" {
                    section_start = Some(i);
                } else if section_start.is_some() && section_end.is_none() {
                    section_end = Some(i);
                }
            }
            if section_start.is_some() && section_end.is_none() && trimmed.starts_with('|') {
                last_table_row = Some(i);
            }
        }

        match section_start {
            Some(_) => {
                // Section exists — insert after the last table row
                let insert_at = last_table_row.map_or(section_start.unwrap() + 1, |r| r + 1);
                lines.insert(insert_at, &new_row);
                lines.join("\n")
            }
            None => {
                // No Amendment Log section — create one at the end
                let mut result = content.to_string();
                if !result.is_empty() && !result.ends_with('\n') {
                    result.push('\n');
                }
                if !result.is_empty() && !result.ends_with("\n\n") {
                    result.push('\n');
                }
                result.push_str("## Amendment Log\n\n");
                result.push_str("| Date       | Article   | Rationale | Compatibility |\n");
                result.push_str("|------------|-----------|-----------|---------------|\n");
                result.push_str(&new_row);
                result.push('\n');
                result
            }
        }
    }
}

// ── Amendment Process (FR-016) ───────────────────────────────────────────

/// Input for a constitutional amendment (FR-016).
///
/// Used with [`Constitution::apply_amendment`] to produce updated
/// `CONSTITUTION.md` markdown with the amendment recorded in a dated
/// changelog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmendmentRequest {
    /// ISO-8601 date string (e.g., "2025-01-15").
    pub date: String,
    /// Which article was amended (e.g., "Article 3" or "Article 3: Simplicity").
    pub article: String,
    /// Required: explicit rationale for the amendment.
    pub rationale: String,
    /// Required: backward-compatibility assessment.
    pub compatibility: String,
}

impl AmendmentRequest {
    /// Create a new amendment request with all required fields.
    #[must_use]
    pub fn new(date: &str, article: &str, rationale: &str, compatibility: &str) -> Self {
        Self {
            date: date.to_string(),
            article: article.to_string(),
            rationale: rationale.to_string(),
            compatibility: compatibility.to_string(),
        }
    }
}

/// A validation issue with a constitutional amendment (FR-016).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmendmentIssue {
    /// Date of the amendment (may be empty if date is missing).
    pub date: String,
    /// Article reference of the amendment.
    pub article: String,
    /// Which field is problematic: "date", "rationale", or "compatibility".
    pub field: &'static str,
    /// Human-readable description of the issue.
    pub message: String,
}

/// Validate that a date string is in ISO-8601 `YYYY-MM-DD` format.
fn validate_amendment_date(date: &str) -> Result<(), SpecError> {
    let d = date.trim();
    if d.is_empty() {
        return Err(SpecError::AmendmentError(
            "amendment date is required".to_string(),
        ));
    }
    let parts: Vec<&str> = d.split('-').collect();
    if parts.len() != 3 {
        return Err(SpecError::AmendmentError(format!(
            "amendment date '{}' is not in YYYY-MM-DD format",
            d
        )));
    }
    let (year, month, day) = (parts[0], parts[1], parts[2]);
    if year.len() != 4 || !year.chars().all(|c| c.is_ascii_digit()) {
        return Err(SpecError::AmendmentError(format!(
            "amendment date '{}' has an invalid year (expected 4-digit YYYY)",
            d
        )));
    }
    if day.len() != 2 || !day.chars().all(|c| c.is_ascii_digit()) {
        return Err(SpecError::AmendmentError(format!(
            "amendment date '{}' has an invalid day (expected 2-digit DD)",
            d
        )));
    }
    if month.len() != 2 || !month.chars().all(|c| c.is_ascii_digit()) {
        return Err(SpecError::AmendmentError(format!(
            "amendment date '{}' has an invalid month (expected 2-digit MM)",
            d
        )));
    }
    let m: u32 = month.parse().unwrap_or(0);
    let dd: u32 = day.parse().unwrap_or(0);
    if m == 0 || m > 12 {
        return Err(SpecError::AmendmentError(format!(
            "amendment date '{}' has an invalid month (expected 01-12)",
            d
        )));
    }
    if dd == 0 || dd > 31 {
        return Err(SpecError::AmendmentError(format!(
            "amendment date '{}' has an invalid day (expected 01-31)",
            d
        )));
    }
    Ok(())
}

/// Parse a `CONSTITUTION.md` markdown string into a [`Constitution`].
///
/// Articles are extracted from headings matching `## Article <n>: <title>`
/// or `## Article <n> — <title>`. The body is all text between the heading
/// and the next `## ` heading.
///
/// If an `## Amendment Log` section is present, its markdown table is parsed
/// into [`Amendment`] entries.
///
/// # Example
///
/// ```
/// use ragent_specs::constitution::parse_constitution;
///
/// let md = "# Constitution\n\n## Article 1: Library-First\n\nPrefer small libraries.\n";
/// let c = parse_constitution(md);
/// assert_eq!(c.articles.len(), 1);
/// assert_eq!(c.articles[0].number, 1);
/// assert_eq!(c.articles[0].title, "Library-First");
/// assert_eq!(c.articles[0].body, "Prefer small libraries.");
/// ```
#[must_use]
pub fn parse_constitution(content: &str) -> Constitution {
    let articles = parse_articles(content);
    let amendments = parse_amendments(content);
    Constitution {
        content: content.to_string(),
        articles,
        amendments,
    }
}

/// Regex-free parser: extract articles from `## Article N: Title` headings.
fn parse_articles(content: &str) -> Vec<Article> {
    let mut articles = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i].trim_start();
        if let Some((number, title)) = parse_article_heading(line) {
            // Collect body lines until the next `## ` heading
            let mut body_lines = Vec::new();
            i += 1;
            while i < lines.len() {
                let next = lines[i].trim_start();
                if next.starts_with("## ") {
                    break;
                }
                body_lines.push(lines[i]);
                i += 1;
            }
            let body = body_lines.join("\n").trim().to_string();
            articles.push(Article {
                number,
                title,
                body,
            });
        } else {
            i += 1;
        }
    }
    articles
}

/// Parse a heading line like `## Article 3: Library-First` or
/// `## Article 3 — Library-First` into `(number, title)`.
/// Returns `None` if the line is not an article heading.
fn parse_article_heading(line: &str) -> Option<(u32, String)> {
    let rest = line.strip_prefix("## Article ")?;
    // The number is the leading run of digits, optionally followed by a
    // separator (`:` or `—` or `-`) and then the title.
    let sep_pos = rest.find([':', '—', '-']).unwrap_or(rest.len());
    let num_str = rest[..sep_pos].trim();
    let number: u32 = num_str.parse().ok()?;
    // Skip past the separator to get the title
    let title = if sep_pos < rest.len() {
        rest[sep_pos..]
            .chars()
            .next()
            .map(|c| rest[sep_pos + c.len_utf8()..].trim().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };
    Some((number, title))
}

/// Parse the `## Amendment Log` section's markdown table into amendments.
fn parse_amendments(content: &str) -> Vec<Amendment> {
    let mut amendments = Vec::new();
    let mut in_amendment_section = false;
    let mut in_table = false;
    let mut header_seen = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("## ") {
            in_amendment_section = trimmed == "## Amendment Log";
            in_table = false;
            header_seen = false;
            continue;
        }

        if !in_amendment_section {
            continue;
        }

        // Look for table rows
        if trimmed.starts_with('|') {
            if !in_table {
                // First pipe line is the header
                in_table = true;
                header_seen = false;
                continue;
            }
            if !header_seen {
                // Second pipe line is the separator (|---|---|)
                header_seen = true;
                continue;
            }
            // Data row
            if let Some(a) = parse_amendment_row(trimmed) {
                amendments.push(a);
            }
        } else if !trimmed.is_empty() {
            // Non-empty, non-table line ends the table
            in_table = false;
            header_seen = false;
        }
    }

    amendments
}

/// Parse a single markdown table row into an [`Amendment`].
/// Expected format: `| date | article | rationale | compatibility |`
fn parse_amendment_row(row: &str) -> Option<Amendment> {
    let cells: Vec<&str> = row.split('|').collect::<Vec<_>>();
    // Leading/trailing empty strings from the outer pipes
    if cells.len() < 6 {
        return None;
    }
    let date = cells[1].trim().to_string();
    let article = cells[2].trim().to_string();
    let rationale = cells[3].trim().to_string();
    let compatibility = cells[4].trim().to_string();
    Some(Amendment {
        date,
        article,
        rationale,
        compatibility,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_constitution() {
        let c = Constitution::empty();
        assert!(c.is_empty());
        assert!(c.articles.is_empty());
        assert!(c.amendments.is_empty());
    }

    #[test]
    fn test_parse_single_article() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nPrefer small libraries.\n";
        let c = parse_constitution(md);
        assert_eq!(c.articles.len(), 1);
        assert_eq!(c.articles[0].number, 1);
        assert_eq!(c.articles[0].title, "Library-First");
        assert_eq!(c.articles[0].body, "Prefer small libraries.");
    }

    #[test]
    fn test_parse_multiple_articles() {
        let md = "\
# Constitution

## Article 1: Library-First

Prefer small libraries over monoliths.

## Article 2: Simplicity

Keep it simple.

## Article 3: Anti-Abstraction

No premature abstractions.
";
        let c = parse_constitution(md);
        assert_eq!(c.articles.len(), 3);
        assert_eq!(c.articles[0].title, "Library-First");
        assert_eq!(c.articles[1].title, "Simplicity");
        assert_eq!(c.articles[2].title, "Anti-Abstraction");
    }

    #[test]
    fn test_article_by_number() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        assert_eq!(c.article_by_number(1).unwrap().title, "Library-First");
        assert!(c.article_by_number(99).is_none());
    }

    #[test]
    fn test_article_by_title() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        assert_eq!(c.article_by_title("Library-First").unwrap().number, 1);
        assert!(c.article_by_title("Nonexistent").is_none());
    }

    #[test]
    fn test_parse_em_dash_separator() {
        let md = "# Constitution\n\n## Article 1 — Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        assert_eq!(c.articles.len(), 1);
        assert_eq!(c.articles[0].title, "Library-First");
    }

    #[test]
    fn test_parse_no_articles() {
        let md = "# Constitution\n\nSome intro text.\n";
        let c = parse_constitution(md);
        assert!(c.articles.is_empty());
    }

    #[test]
    fn test_parse_amendment_log() {
        let md = "\
# Constitution

## Article 1: Library-First

Prefer small libraries.

## Amendment Log

| Date       | Article   | Rationale     | Compatibility |
|------------|-----------|---------------|---------------|
| 2025-01-15 | Article 3 | Updated scope | Backward OK   |
| 2025-02-01 | Article 1 | Clarified     | Breaking      |
";
        let c = parse_constitution(md);
        assert_eq!(c.amendments.len(), 2);
        assert_eq!(c.amendments[0].date, "2025-01-15");
        assert_eq!(c.amendments[0].article, "Article 3");
        assert_eq!(c.amendments[0].rationale, "Updated scope");
        assert_eq!(c.amendments[0].compatibility, "Backward OK");
        assert_eq!(c.amendments[1].date, "2025-02-01");
    }

    #[test]
    fn test_no_amendment_log() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        assert!(c.amendments.is_empty());
    }

    #[test]
    fn test_constitution_path() {
        let path = Constitution::path(Path::new("specs"));
        assert_eq!(path, PathBuf::from("specs/CONSTITUTION.md"));
    }

    #[test]
    fn test_article_body_multiline() {
        let md = "\
# Constitution

## Article 1: Library-First

Prefer composing from small libraries
over building monolithic subsystems.

This is a core principle.
";
        let c = parse_constitution(md);
        assert_eq!(c.articles.len(), 1);
        assert!(c.articles[0].body.contains("Prefer composing"));
        assert!(c.articles[0].body.contains("core principle"));
    }

    // ── Edge-case tests (T-041, NFR-004) ────────────────────────────────────

    #[test]
    fn test_parse_article_empty_body() {
        let md = "\
# Constitution

## Article 1: Library-First

## Article 2: Simplicity

Keep it simple.
";
        let c = parse_constitution(md);
        assert_eq!(c.articles.len(), 2);
        assert_eq!(c.articles[0].number, 1);
        assert_eq!(c.articles[0].title, "Library-First");
        assert!(
            c.articles[0].body.is_empty(),
            "article with no body text should have empty body"
        );
        assert_eq!(c.articles[1].title, "Simplicity");
        assert_eq!(c.articles[1].body, "Keep it simple.");
    }

    #[test]
    fn test_parse_preamble_before_articles() {
        let md = "\
# Constitution

This constitution defines the architectural principles
that govern all generated implementations.

## Article 1: Library-First

Prefer small libraries.
";
        let c = parse_constitution(md);
        assert_eq!(
            c.articles.len(),
            1,
            "preamble text should not create an article"
        );
        assert_eq!(c.articles[0].title, "Library-First");
        assert_eq!(c.articles[0].body, "Prefer small libraries.");
    }

    #[test]
    fn test_amendment_log_header_only_no_data_rows() {
        let md = "\
# Constitution

## Article 1: Library-First

Prefer small libraries.

## Amendment Log

| Date       | Article   | Rationale | Compatibility |
|------------|-----------|-----------|---------------|
";
        let c = parse_constitution(md);
        assert_eq!(c.articles.len(), 1);
        assert!(
            c.amendments.is_empty(),
            "amendment log with only a header row should produce no amendments"
        );
    }

    // ── Amendment Process tests (T-030, FR-016) ──────────────────────────

    #[test]
    fn test_apply_amendment_creates_new_section() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nPrefer small libraries.\n";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new(
            "2025-03-15",
            "Article 1",
            "Broadened to include wasm targets",
            "No breaking changes — existing code unaffected",
        );
        let updated = c.apply_amendment(&req).unwrap();
        assert!(updated.contains("## Amendment Log"));
        assert!(updated.contains("| 2025-03-15 | Article 1 | Broadened to include wasm targets | No breaking changes — existing code unaffected |"));
    }

    #[test]
    fn test_apply_amendment_appends_to_existing_section() {
        let md = "\
# Constitution

## Article 1: Library-First

Prefer small libraries.

## Amendment Log

| Date       | Article   | Rationale | Compatibility |
|------------|-----------|-----------|---------------|
| 2025-01-15 | Article 3 | Updated   | OK            |
";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new(
            "2025-06-01",
            "Article 1",
            "Clarified scope",
            "Backward compatible",
        );
        let updated = c.apply_amendment(&req).unwrap();
        assert!(updated.contains("| 2025-01-15 | Article 3 | Updated   | OK            |"));
        assert!(
            updated.contains("| 2025-06-01 | Article 1 | Clarified scope | Backward compatible |")
        );
    }

    #[test]
    fn test_apply_amendment_round_trip() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new("2025-07-20", "Article 1", "Reason X", "Compat Y");
        let updated = c.apply_amendment(&req).unwrap();
        let reparsed = parse_constitution(&updated);
        assert_eq!(reparsed.amendments.len(), 1);
        assert_eq!(reparsed.amendments[0].date, "2025-07-20");
        assert_eq!(reparsed.amendments[0].article, "Article 1");
        assert_eq!(reparsed.amendments[0].rationale, "Reason X");
        assert_eq!(reparsed.amendments[0].compatibility, "Compat Y");
    }

    #[test]
    fn test_apply_amendment_round_trip_with_existing_entries() {
        let md = "\
# Constitution

## Article 1: Library-First

Body.

## Amendment Log

| Date       | Article   | Rationale | Compatibility |
|------------|-----------|-----------|---------------|
| 2025-01-15 | Article 3 | Updated   | OK            |
";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new("2025-08-01", "Article 2", "New reason", "New compat");
        let updated = c.apply_amendment(&req).unwrap();
        let reparsed = parse_constitution(&updated);
        assert_eq!(reparsed.amendments.len(), 2);
        assert_eq!(reparsed.amendments[0].date, "2025-01-15");
        assert_eq!(reparsed.amendments[1].date, "2025-08-01");
        assert_eq!(reparsed.amendments[1].article, "Article 2");
    }

    #[test]
    fn test_apply_amendment_empty_rationale_rejected() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new("2025-03-15", "Article 1", "", "Compatible");
        let err = c.apply_amendment(&req).unwrap_err();
        assert!(err.to_string().contains("rationale"));
    }

    #[test]
    fn test_apply_amendment_whitespace_rationale_rejected() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new("2025-03-15", "Article 1", "   ", "Compatible");
        let err = c.apply_amendment(&req).unwrap_err();
        assert!(err.to_string().contains("rationale"));
    }

    #[test]
    fn test_apply_amendment_empty_compatibility_rejected() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new("2025-03-15", "Article 1", "Good reason", "");
        let err = c.apply_amendment(&req).unwrap_err();
        assert!(err.to_string().contains("compatibility"));
    }

    #[test]
    fn test_apply_amendment_empty_article_rejected() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new("2025-03-15", "", "Reason", "Compatible");
        let err = c.apply_amendment(&req).unwrap_err();
        assert!(err.to_string().contains("article"));
    }

    #[test]
    fn test_apply_amendment_empty_date_rejected() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new("", "Article 1", "Reason", "Compatible");
        let err = c.apply_amendment(&req).unwrap_err();
        assert!(err.to_string().contains("date"));
    }

    #[test]
    fn test_apply_amendment_invalid_date_format_rejected() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new("2025/03/15", "Article 1", "Reason", "Compatible");
        let err = c.apply_amendment(&req).unwrap_err();
        assert!(err.to_string().contains("YYYY-MM-DD"));
    }

    #[test]
    fn test_apply_amendment_invalid_month_rejected() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new("2025-13-15", "Article 1", "Reason", "Compatible");
        let err = c.apply_amendment(&req).unwrap_err();
        assert!(err.to_string().contains("month"));
    }

    #[test]
    fn test_apply_amendment_invalid_day_rejected() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new("2025-03-32", "Article 1", "Reason", "Compatible");
        let err = c.apply_amendment(&req).unwrap_err();
        assert!(err.to_string().contains("day"));
    }

    #[test]
    fn test_apply_amendment_two_digit_year_rejected() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        let req = AmendmentRequest::new("25-03-15", "Article 1", "Reason", "Compatible");
        let err = c.apply_amendment(&req).unwrap_err();
        assert!(err.to_string().contains("year"));
    }

    #[test]
    fn test_validate_amendments_all_valid() {
        let md = "\
## Amendment Log

| Date       | Article   | Rationale | Compatibility |
|------------|-----------|-----------|---------------|
| 2025-01-15 | Article 3 | Updated   | OK            |
| 2025-02-01 | Article 1 | Clarified | Breaking      |
";
        let c = parse_constitution(md);
        assert!(c.validate_amendments().is_empty());
    }

    #[test]
    fn test_validate_amendments_missing_rationale() {
        let md = "\
## Amendment Log

| Date       | Article   | Rationale | Compatibility |
|------------|-----------|-----------|---------------|
| 2025-01-15 | Article 3 |           | OK            |
";
        let c = parse_constitution(md);
        let issues = c.validate_amendments();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].field, "rationale");
        assert!(issues[0].message.contains("rationale"));
    }

    #[test]
    fn test_validate_amendments_missing_compatibility() {
        let md = "\
## Amendment Log

| Date       | Article   | Rationale | Compatibility |
|------------|-----------|-----------|---------------|
| 2025-01-15 | Article 3 | Updated   |               |
";
        let c = parse_constitution(md);
        let issues = c.validate_amendments();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].field, "compatibility");
    }

    #[test]
    fn test_validate_amendments_missing_both_fields() {
        let md = "\
## Amendment Log

| Date       | Article   | Rationale | Compatibility |
|------------|-----------|-----------|---------------|
| 2025-01-15 | Article 3 |           |               |
";
        let c = parse_constitution(md);
        let issues = c.validate_amendments();
        assert_eq!(issues.len(), 2);
    }

    #[test]
    fn test_validate_amendments_no_amendments() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);
        assert!(c.validate_amendments().is_empty());
    }

    #[test]
    fn test_validate_amendments_missing_date() {
        let md = "\
## Amendment Log

| Date       | Article   | Rationale | Compatibility |
|------------|-----------|-----------|---------------|
|            | Article 3 | Updated   | OK            |
";
        let c = parse_constitution(md);
        let issues = c.validate_amendments();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].field, "date");
    }

    #[test]
    fn test_amendment_request_new() {
        let req = AmendmentRequest::new("2025-01-01", "Article 1", "R", "C");
        assert_eq!(req.date, "2025-01-01");
        assert_eq!(req.article, "Article 1");
        assert_eq!(req.rationale, "R");
        assert_eq!(req.compatibility, "C");
    }

    #[test]
    fn test_apply_amendment_preserves_articles() {
        let md = "\
# Constitution

## Article 1: Library-First

Prefer small libraries.

## Article 2: Simplicity

Keep it simple.
";
        let c = parse_constitution(md);
        assert_eq!(c.articles.len(), 2);
        let req = AmendmentRequest::new("2025-03-15", "Article 2", "Broadened", "OK");
        let updated = c.apply_amendment(&req).unwrap();
        let reparsed = parse_constitution(&updated);
        assert_eq!(reparsed.articles.len(), 2);
        assert_eq!(reparsed.articles[0].title, "Library-First");
        assert_eq!(reparsed.articles[1].title, "Simplicity");
    }

    #[test]
    fn test_apply_multiple_amendments_sequentially() {
        let md = "# Constitution\n\n## Article 1: Library-First\n\nBody.\n";
        let c = parse_constitution(md);

        let req1 = AmendmentRequest::new("2025-01-01", "Article 1", "First", "OK1");
        let updated1 = c.apply_amendment(&req1).unwrap();
        let c1 = parse_constitution(&updated1);
        assert_eq!(c1.amendments.len(), 1);

        let req2 = AmendmentRequest::new("2025-02-01", "Article 1", "Second", "OK2");
        let updated2 = c1.apply_amendment(&req2).unwrap();
        let c2 = parse_constitution(&updated2);
        assert_eq!(c2.amendments.len(), 2);
        assert_eq!(c2.amendments[0].rationale, "First");
        assert_eq!(c2.amendments[1].rationale, "Second");
    }
}
