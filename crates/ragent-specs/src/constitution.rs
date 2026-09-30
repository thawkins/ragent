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
//! `## Article <n> - <title>` headings. The amendment log is a markdown table
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
/// returns a value with no articles - callers should treat this as "no
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
                // Any subsequent section heading closes the Amendment Log
                // section. The former inner `else if` duplicate of this
                // assignment was unreachable and has been removed.
                if section_start.is_some() && section_end.is_none() {
                    section_end = Some(i);
                }
                if trimmed == "## Amendment Log" {
                    section_start = Some(i);
                }
            }
            if section_start.is_some() && section_end.is_none() && trimmed.starts_with('|') {
                last_table_row = Some(i);
            }
        }

        match section_start {
            Some(start) => {
                // Section exists - insert after the last table row
                let insert_at = last_table_row.map_or(start + 1, |r| r + 1);
                lines.insert(insert_at, &new_row);
                lines.join("\n")
            }
            None => {
                // No Amendment Log section - create one at the end
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
/// or `## Article <n> - <title>`. The body is all text between the heading
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
/// `## Article 3 - Library-First` into `(number, title)`.
/// Returns `None` if the line is not an article heading.
fn parse_article_heading(line: &str) -> Option<(u32, String)> {
    let rest = line.strip_prefix("## Article ")?;
    // The number is the leading run of digits, optionally followed by a
    // separator (`:` or `-` or `-`) and then the title.
    let sep_pos = rest.find([':', '-', '-']).unwrap_or(rest.len());
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
    // Leading/trailing empty strings from the outer pipes; a well-formed row
    // has 6 cells. When the rationale itself contains a literal `|`, extra
    // cells appear between article and compatibility - rejoin them so the
    // row is not silently mis-parsed.
    if cells.len() < 6 {
        return None;
    }
    let date = cells[1].trim().to_string();
    let article = cells[2].trim().to_string();
    let rationale = if cells.len() > 6 {
        cells[3..cells.len() - 2].join("|")
    } else {
        cells[3].to_string()
    }
    .trim()
    .to_string();
    let compatibility = cells[cells.len() - 2].trim().to_string();
    Some(Amendment {
        date,
        article,
        rationale,
        compatibility,
    })
}

#[cfg(test)]
#[path = "../tests/inline/constitution_tests.rs"]
mod tests;
