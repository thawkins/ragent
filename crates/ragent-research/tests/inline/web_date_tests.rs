//! Inline tests for `web_date.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

fn web_dt(y: i32, m: u32, d: u32) -> DateTime<Utc> {
    NaiveDate::from_ymd_opt(y, m, d)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
}

// Small chrono arithmetic helpers for readable test assertions.
trait DtArith {
    fn plus_hours(self, h: u32) -> DateTime<Utc>;
    fn plus_mins(self, m: u32) -> DateTime<Utc>;
}
impl DtArith for DateTime<Utc> {
    fn plus_hours(self, h: u32) -> DateTime<Utc> {
        self + chrono::Duration::hours(i64::from(h))
    }
    fn plus_mins(self, m: u32) -> DateTime<Utc> {
        self + chrono::Duration::minutes(i64::from(m))
    }
}

#[test]
fn test_extracts_from_article_published_time_meta() {
    let html = r#"<html><head>
        <meta property="article:published_time" content="2024-03-22T10:30:00Z">
        </head><body>hi</body></html>"#;
    assert_eq!(
        extract_published_at(html),
        Some(web_dt(2024, 3, 22).plus_hours(10).plus_mins(30))
    );
}

#[test]
fn test_extracts_from_json_ld_date_published() {
    let html = r#"<html><head>
        <script type="application/ld+json">
        {"@type":"Article","datePublished":"2023-11-05T08:00:00+00:00"}
        </script>
        </head><body>x</body></html>"#;
    assert_eq!(
        extract_published_at(html),
        Some(web_dt(2023, 11, 5).plus_hours(8))
    );
}

#[test]
fn test_extracts_from_json_ld_array() {
    let html = r#"<html><head>
        <script type="application/ld+json">
        [{"@type":"NewsArticle"},{"datePublished":"2022-01-10"}]
        </script>
        </head><body>x</body></html>"#;
    assert_eq!(extract_published_at(html), Some(web_dt(2022, 1, 10)));
}

#[test]
fn test_extracts_from_time_element() {
    let html = r#"<html><body>
        <time datetime="2021-06-07T12:00:00Z">June 7, 2021</time>
        </body></html>"#;
    assert_eq!(
        extract_published_at(html),
        Some(web_dt(2021, 6, 7).plus_hours(12))
    );
}

#[test]
fn test_falls_back_to_visible_iso_date() {
    let html = "<html><body><p>Published on 2020-09-01 by example.</p></body></html>";
    assert_eq!(extract_published_at(html), Some(web_dt(2020, 9, 1)));
}

#[test]
fn test_falls_back_to_human_date() {
    let html = "<html><body><h1>Report</h1><p>January 15, 2019 - summary.</p></body></html>";
    assert_eq!(extract_published_at(html), Some(web_dt(2019, 1, 15)));
}

#[test]
fn test_returns_none_when_no_date_present() {
    let html = "<html><body><p>No dates here at all.</p></body></html>";
    assert_eq!(extract_published_at(html), None);
}

#[test]
fn test_returns_none_for_malformed_meta_content() {
    let html = r#"<meta property="article:published_time" content="not-a-date">"#;
    assert_eq!(extract_published_at(html), None);
}

#[test]
fn test_handles_pubdate_meta_name() {
    let html = r#"<meta name="pubdate" content="2018-04-02T05:00:00Z">"#;
    assert_eq!(
        extract_published_at(html),
        Some(web_dt(2018, 4, 2).plus_hours(5))
    );
}

#[test]
fn test_bare_iso_date_without_time() {
    assert_eq!(parse_date_string("2024-12-25"), Some(web_dt(2024, 12, 25)));
}

#[test]
fn test_slash_and_dot_iso_variants() {
    assert_eq!(parse_date_string("2024/12/25"), Some(web_dt(2024, 12, 25)));
    assert_eq!(parse_date_string("2024.12.25"), Some(web_dt(2024, 12, 25)));
}

#[test]
fn test_prefers_json_ld_over_meta() {
    // JSON-LD is checked first.
    let html = r#"<html><head>
        <meta property="article:published_time" content="2099-01-01">
        <script type="application/ld+json">{"datePublished":"2023-05-05"}</script>
        </head></html>"#;
    assert_eq!(extract_published_at(html), Some(web_dt(2023, 5, 5)));
}
