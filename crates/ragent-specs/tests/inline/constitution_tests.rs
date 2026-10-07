//! Inline tests for `constitution.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_empty_constitution() {
    let c = Constitution::empty();
    assert!(c.is_empty(), "constitution should be empty");
    assert!(c.articles.is_empty(), "articles should be empty");
    assert!(c.amendments.is_empty(), "amendments should be empty");
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
    let md = "# Constitution\n\n## Article 1 - Library-First\n\nBody.\n";
    let c = parse_constitution(md);
    assert_eq!(c.articles.len(), 1);
    assert_eq!(c.articles[0].title, "Library-First");
}

#[test]
fn test_parse_no_articles() {
    let md = "# Constitution\n\nSome intro text.\n";
    let c = parse_constitution(md);
    assert!(c.articles.is_empty(), "articles should be empty");
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
    assert!(c.amendments.is_empty(), "amendments should be empty");
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

// -- Edge-case tests (T-041, NFR-004) ------------------------------------

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

// -- Amendment Process tests (T-030, FR-016) --------------------------

#[test]
fn test_apply_amendment_creates_new_section() {
    let md = "# Constitution\n\n## Article 1: Library-First\n\nPrefer small libraries.\n";
    let c = parse_constitution(md);
    let req = AmendmentRequest::new(
        "2025-03-15",
        "Article 1",
        "Broadened to include wasm targets",
        "No breaking changes - existing code unaffected",
    );
    let updated = c.apply_amendment(&req).unwrap();
    assert!(updated.contains("## Amendment Log"));
    assert!(updated.contains("| 2025-03-15 | Article 1 | Broadened to include wasm targets | No breaking changes - existing code unaffected |"));
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
    assert!(updated.contains("| 2025-06-01 | Article 1 | Clarified scope | Backward compatible |"));
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
    assert!(
        c.validate_amendments().is_empty(),
        "amendment validation should be empty"
    );
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
    assert!(
        c.validate_amendments().is_empty(),
        "amendment validation should be empty"
    );
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
