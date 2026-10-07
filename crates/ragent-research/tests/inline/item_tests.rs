//! Inline tests for `item.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::research_name::ResearchName;

fn sample_name() -> ResearchName {
    ResearchName::new("rust-async").expect("name must validate")
}

#[test]
fn new_creates_item_in_draft_state() {
    let item = ResearchItem::new(sample_name(), "Rust Async Patterns", "async/await idioms");
    assert_eq!(item.status, ResearchStatus::Draft);
    assert_eq!(item.title, "Rust Async Patterns");
    assert_eq!(item.topic, "async/await idioms");
    assert_eq!(item.source_count(), 0);
    assert!(!item.has_sources());
    assert_eq!(item.created_at, item.modified_at);
}

#[test]
fn new_sets_both_timestamps_to_now() {
    let before = Utc::now();
    let item = ResearchItem::new(sample_name(), "t", "topic");
    let after = Utc::now();
    assert!(item.created_at >= before);
    assert!(item.created_at <= after);
    assert_eq!(item.created_at, item.modified_at);
}

#[test]
fn set_status_updates_modified_but_not_created() {
    let mut item = ResearchItem::new(sample_name(), "t", "topic");
    let created = item.created_at;
    // Tiny sleep so timestamps differ at millisecond resolution.
    std::thread::sleep(std::time::Duration::from_millis(2));
    item.set_status(ResearchStatus::InProgress);
    assert_eq!(item.status, ResearchStatus::InProgress);
    assert_eq!(item.created_at, created);
    assert!(item.modified_at >= created);
}

#[test]
fn set_title_updates_modified() {
    let mut item = ResearchItem::new(sample_name(), "Original", "topic");
    item.set_title("Updated");
    assert_eq!(item.title, "Updated");
}

#[test]
fn add_source_appends_in_order_and_bumps_modified() {
    let mut item = ResearchItem::new(sample_name(), "t", "topic");
    let s1 = Source::Other {
        label: "first".into(),
        captured_at: Utc::now(),
        body_path: "sources/other-01.md".into(),
        body: String::new(),
    };
    let s2 = Source::Other {
        label: "second".into(),
        captured_at: Utc::now(),
        body_path: "sources/other-02.md".into(),
        body: String::new(),
    };
    item.add_source(s1.clone()).add_source(s2.clone());
    assert_eq!(item.sources.len(), 2);
    assert_eq!(item.sources[0], s1);
    assert_eq!(item.sources[1], s2);
    assert_eq!(item.source_count(), 2);
    assert!(item.has_sources());
}

#[test]
fn touch_only_updates_modified() {
    let mut item = ResearchItem::new(sample_name(), "t", "topic");
    let created = item.created_at;
    let before_mod = item.modified_at;
    std::thread::sleep(std::time::Duration::from_millis(2));
    item.touch();
    assert_eq!(item.created_at, created);
    assert!(item.modified_at >= before_mod);
}

#[test]
fn render_frontmatter_contains_required_fields() {
    let item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    let fm = item.render_frontmatter();
    assert!(fm.starts_with("---\n"));
    assert!(fm.ends_with("---\n\n"));
    assert!(fm.contains("name: rust-async"));
    assert!(fm.contains("title: \"Rust Async\""));
    assert!(fm.contains("topic: \"topic\""));
    assert!(fm.contains("status: draft"));
    assert!(fm.contains("created: "));
    assert!(fm.contains("modified: "));
    assert!(fm.contains("sources: 0"));
    assert!(fm.contains("queries: []"));
}

#[test]
fn render_frontmatter_uses_plain_yaml_lines() {
    let item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    let fm = item.render_frontmatter();
    assert!(
        !fm.contains("**name:**"),
        "frontmatter should not use markdown bold labels"
    );
    assert!(
        fm.contains("name: rust-async\ntitle:"),
        "fields should be plain YAML key: value"
    );
}

#[test]
fn render_frontmatter_escapes_quotes_in_title_and_topic() {
    let item = ResearchItem::new(sample_name(), "Has \"quotes\"", "also \"quoted\"");
    let fm = item.render_frontmatter();
    assert!(fm.contains("title: \"Has \\\"quotes\\\"\""));
    assert!(fm.contains("topic: \"also \\\"quoted\\\"\""));
}

#[test]
fn render_frontmatter_round_trips_through_parse() {
    let item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    let fm = item.render_frontmatter();
    let parsed = ResearchItem::from_frontmatter(&fm).expect("frontmatter must parse");
    assert_eq!(parsed.name, item.name);
    assert_eq!(parsed.title, item.title);
    assert_eq!(parsed.topic, item.topic);
    assert_eq!(parsed.status, item.status);
    assert_eq!(parsed.created_at, item.created_at);
    assert_eq!(parsed.modified_at, item.modified_at);
    assert!(parsed.sources.is_empty());
}

#[test]
fn render_frontmatter_round_trips_with_special_chars_in_title() {
    let mut item = ResearchItem::new(sample_name(), "t", "topic");
    item.set_title("Has: colon");
    let fm = item.render_frontmatter();
    let parsed = ResearchItem::from_frontmatter(&fm).expect("frontmatter must parse");
    assert_eq!(parsed.title, "Has: colon");
}

#[test]
fn render_frontmatter_round_trips_with_queries() {
    let mut item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    item.set_queries(vec!["first query".into(), "second query".into()]);
    let fm = item.render_frontmatter();
    let parsed = ResearchItem::from_frontmatter(&fm).expect("frontmatter must parse");
    assert_eq!(parsed.queries, vec!["first query", "second query"]);
}

#[test]
fn render_frontmatter_round_trips_with_model() {
    let mut item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    item.model = Some("anthropic/claude-sonnet-4".into());
    let fm = item.render_frontmatter();
    assert!(
        fm.contains("Model: \"anthropic/claude-sonnet-4\""),
        "frontmatter should contain a Model: line; got:\n{fm}"
    );
    let parsed = ResearchItem::from_frontmatter(&fm).expect("frontmatter must parse");
    assert_eq!(parsed.model.as_deref(), Some("anthropic/claude-sonnet-4"));
}

#[test]
fn render_frontmatter_omits_model_line_when_none() {
    let item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    let fm = item.render_frontmatter();
    assert!(
        !fm.contains("Model:"),
        "frontmatter must omit Model: when no model is set; got:\n{fm}"
    );
}

#[test]
fn render_frontmatter_round_trips_with_quote_in_query() {
    let mut item = ResearchItem::new(sample_name(), "t", "topic");
    item.set_queries(vec!["query with \"quotes\"".into()]);
    let fm = item.render_frontmatter();
    let parsed = ResearchItem::from_frontmatter(&fm).expect("frontmatter must parse");
    assert_eq!(parsed.queries, vec!["query with \"quotes\""]);
}

#[test]
fn from_frontmatter_parses_bold_labeled_format() {
    let block = "---\n\n**name:** rust-async\n\n**title:** Rust Async\n\n**topic:** async/await\n\n**status:** draft\n\n---\n\n";
    let item = ResearchItem::from_frontmatter(block).unwrap();
    assert_eq!(item.name.as_str(), "rust-async");
    assert_eq!(item.title, "Rust Async");
    assert_eq!(item.topic, "async/await");
    assert_eq!(item.status, ResearchStatus::Draft);
}

#[test]
fn from_frontmatter_still_parses_legacy_plain_yaml() {
    let block =
        "---\nname: rust-async\ntitle: Rust Async\ntopic: async/await\nstatus: draft\n---\n";
    let item = ResearchItem::from_frontmatter(block).unwrap();
    assert_eq!(item.name.as_str(), "rust-async");
    assert_eq!(item.title, "Rust Async");
    assert_eq!(item.topic, "async/await");
    assert_eq!(item.status, ResearchStatus::Draft);
}
#[test]
fn from_frontmatter_fails_on_missing_name() {
    let block = "---\n\n**title:** foo\n\n---\n\n";
    let err = ResearchItem::from_frontmatter(block).unwrap_err();
    assert!(matches!(err, ResearchItemError::MissingField(_)));
}

#[test]
fn from_frontmatter_fails_on_invalid_name() {
    let block = "---\n\n**name:** ..\n\n**title:** foo\n\n---\n\n";
    let err = ResearchItem::from_frontmatter(block).unwrap_err();
    assert!(matches!(err, ResearchItemError::InvalidName(_)));
}

// -- derive_title -------------------------------------------------------

#[test]
fn derive_title_uses_full_topic_not_first_word() {
    // Regression: the title must summarise the topic, not be truncated to
    // the first word.
    let title = derive_title("async/await idioms in stable Rust", None);
    assert_eq!(title, "async/await idioms in stable Rust");
}

#[test]
fn derive_title_trims_surrounding_whitespace() {
    let title = derive_title("   rust async runtimes   ", None);
    assert_eq!(title, "rust async runtimes");
}

#[test]
fn derive_title_falls_back_to_from_url_when_topic_empty() {
    let title = derive_title("", Some("https://example.com/article"));
    assert_eq!(title, "https://example.com/article");
}
#[test]
fn derive_title_falls_back_to_research_when_both_empty() {
    assert_eq!(derive_title("", None), "Research");
    assert_eq!(derive_title("   ", Some("")), "Research");
}

#[test]
fn derive_title_files_falls_back_to_first_path() {
    assert_eq!(
        derive_title_files("", None, &["docs/notes.md".to_string()]),
        "docs/notes.md"
    );
}

#[test]
fn derive_title_files_prefers_topic_then_url() {
    assert_eq!(
        derive_title_files(
            "real topic",
            Some("https://example.com"),
            &["docs/notes.md".to_string()]
        ),
        "real topic"
    );
    assert_eq!(
        derive_title_files(
            "",
            Some("https://example.com"),
            &["docs/notes.md".to_string()]
        ),
        "https://example.com"
    );
}

#[test]
fn derive_title_caps_long_topics_on_a_word_boundary() {
    let long = "word ".repeat(40); // 200 chars, each "word " is 5 chars
    let title = derive_title(&long, None);
    assert!(title.chars().count() <= DERIVED_TITLE_MAX_CHARS + 3); // +3 ellipsis
    assert!(title.ends_with("..."));
    // Must not split a word in half.
    assert!(!title.trim_end_matches("...").ends_with("wo"));
    assert!(!title.trim_end_matches("...").ends_with('r'));
}

#[test]
fn derive_title_caps_single_overlong_word() {
    let huge = "a".repeat(300);
    let title = derive_title(&huge, None);
    // No whitespace to break on, so it is hard-truncated at the limit.
    assert!(title.chars().count() <= DERIVED_TITLE_MAX_CHARS + 3);
    assert!(title.ends_with("..."));
}

#[test]
fn from_frontmatter_fails_on_invalid_status() {
    let block = "---\n\n**name:** rust-async\n\n**title:** foo\n\n**status:** nope\n\n---\n\n";
    let err = ResearchItem::from_frontmatter(block).unwrap_err();
    assert!(matches!(err, ResearchItemError::InvalidStatus(_)));
}

#[test]
fn from_frontmatter_fails_on_invalid_timestamp() {
    let block =
        "---\n\n**name:** rust-async\n\n**title:** foo\n\n**created:** not-a-date\n\n---\n\n";
    let err = ResearchItem::from_frontmatter(block).unwrap_err();
    assert!(matches!(err, ResearchItemError::InvalidTimestamp { .. }));
}

#[test]
fn from_frontmatter_defaults_optional_fields() {
    let block = "---\n\n**name:** rust-async\n\n**title:** foo\n\n---\n\n";
    let item = ResearchItem::from_frontmatter(block).unwrap();
    assert_eq!(item.status, ResearchStatus::Draft);
    assert!(item.topic.is_empty());
}

#[test]
fn from_frontmatter_tolerates_unknown_fields() {
    let block = "---\n\n**name:** rust-async\n\n**title:** foo\n\n**extra:** stuff\n\n**another:** 42\n\n---\n\n";
    let item = ResearchItem::from_frontmatter(block).expect("unknown fields must not error");
    assert_eq!(item.title, "foo");
}

#[test]
fn serde_round_trip_preserves_all_fields() {
    let mut item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    item.set_status(ResearchStatus::Complete);
    item.add_source(Source::Other {
        label: "example".into(),
        captured_at: Utc::now(),
        body_path: "sources/other-01.md".into(),
        body: String::new(),
    });
    let json = serde_json::to_string(&item).unwrap();
    let back: ResearchItem = serde_json::from_str(&json).unwrap();
    assert_eq!(item, back);
}

#[test]
fn render_frontmatter_includes_open_access_recovery_when_true() {
    let mut item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    item.open_access_recovery = true;
    let fm = item.render_frontmatter();
    assert!(
        fm.contains("open_access_recovery: true"),
        "frontmatter should disclose OA recovery when enabled; got:\n{fm}"
    );
}

#[test]
fn render_frontmatter_omits_open_access_recovery_when_false() {
    let item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    let fm = item.render_frontmatter();
    assert!(
        !fm.contains("open_access_recovery"),
        "frontmatter should omit OA recovery line when disabled; got:\n{fm}"
    );
}

#[test]
fn from_frontmatter_parses_open_access_recovery_true() {
    let block =
        "---\nname: rust-async\ntitle: Rust Async\ntopic: topic\nopen_access_recovery: true\n---\n";
    let item = ResearchItem::from_frontmatter(block).expect("must parse");
    assert!(item.open_access_recovery);
}

#[test]
fn from_frontmatter_defaults_open_access_recovery_false() {
    let block = "---\nname: rust-async\ntitle: Rust Async\ntopic: topic\n---\n";
    let item = ResearchItem::from_frontmatter(block).expect("must parse");
    assert!(!item.open_access_recovery);
}

#[test]
fn serde_round_trip_preserves_open_access_recovery() {
    let mut item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    item.open_access_recovery = true;
    let json = serde_json::to_string(&item).unwrap();
    let back: ResearchItem = serde_json::from_str(&json).unwrap();
    assert!(back.open_access_recovery);
}

#[test]
fn render_frontmatter_includes_invocation_when_set() {
    let mut item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    item.invocation =
        Some("ragent research create --name rust-async \"topic\" --tier full".to_string());
    let fm = item.render_frontmatter();
    assert!(
        fm.contains(
            "invocation: \"ragent research create --name rust-async \\\"topic\\\" --tier full\""
        ),
        "frontmatter should record the verbatim invocation; got:\n{fm}"
    );
}

#[test]
fn render_frontmatter_omits_invocation_when_none() {
    let item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    let fm = item.render_frontmatter();
    assert!(
        !fm.contains("invocation:"),
        "frontmatter should omit invocation line when unset; got:\n{fm}"
    );
}

#[test]
fn from_frontmatter_parses_invocation() {
    let block = "---\nname: rust-async\ntitle: Rust Async\ntopic: topic\ninvocation: \"ragent research create --name rust-async \\\"topic\\\"\"\n---\n";
    let item = ResearchItem::from_frontmatter(block).expect("must parse");
    assert_eq!(
        item.invocation.as_deref(),
        Some("ragent research create --name rust-async \"topic\"")
    );
}

#[test]
fn from_frontmatter_defaults_invocation_to_none() {
    let block = "---\nname: rust-async\ntitle: Rust Async\ntopic: topic\n---\n";
    let item = ResearchItem::from_frontmatter(block).expect("must parse");
    assert!(item.invocation.is_none());
}

#[test]
fn frontmatter_invocation_round_trips() {
    let mut item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    item.invocation =
        Some("ragent research create --name rust-async \"a: b\" --tier full".to_string());
    let fm = item.render_frontmatter();
    let back = ResearchItem::from_frontmatter(&fm).expect("must parse");
    assert_eq!(back.invocation, item.invocation);
}

#[test]
fn serde_round_trip_preserves_invocation() {
    let mut item = ResearchItem::new(sample_name(), "Rust Async", "topic");
    item.invocation = Some("ragent research create --name rust-async \"topic\"".to_string());
    let json = serde_json::to_string(&item).unwrap();
    let back: ResearchItem = serde_json::from_str(&json).unwrap();
    assert_eq!(back.invocation, item.invocation);
}

#[test]
fn error_display_messages_are_useful() {
    let err = ResearchItemError::MissingField("title".to_string());
    assert!(err.to_string().contains("title"));
    let err = ResearchItemError::InvalidStatus("weird".to_string());
    assert!(err.to_string().contains("weird"));
    let err = ResearchItemError::InvalidTimestamp {
        field: "created".to_string(),
        source: "parse error".to_string(),
    };
    assert!(err.to_string().contains("created"));
    assert!(err.to_string().contains("parse error"));
}

// -- strip_control_chars / frontmatter sanitization --------------------

#[test]
fn strip_control_chars_preserves_newlines_and_tabs() {
    let input = "line1\nline2\ttabbed";
    assert_eq!(strip_control_chars(input), input);
}

#[test]
fn strip_control_chars_replaces_cr_with_space() {
    let input = "line1\r\nline2";
    assert_eq!(strip_control_chars(input), "line1 \nline2");
}

#[test]
fn strip_control_chars_drops_c0_control_chars() {
    // 0x00 NUL, 0x01 SOH, 0x07 BEL, 0x08 BS, 0x0B VT, 0x0C FF, 0x0E SO, 0x1F US
    let input = "a\x00b\x01c\x07d\x08e\x0Bf\x0Cg\x0Eh\x1Fi";
    assert_eq!(strip_control_chars(input), "abcdefghi");
}

#[test]
fn strip_control_chars_drops_c1_control_chars() {
    // 0x7F DEL, 0x80, 0x9F - use char literals since \x80/\x9F exceed
    // the \x00-\x7F range allowed in byte/string escapes.
    let input = format!("a{}b{}c{}d", '\u{7F}', '\u{80}', '\u{9F}');
    assert_eq!(strip_control_chars(&input), "abcd");
}

#[test]
fn render_frontmatter_strips_control_chars_from_title() {
    let item = ResearchItem::new(sample_name(), "Title\x01with\x02ctrl", "topic");
    let fm = item.render_frontmatter();
    assert!(
        !fm.contains('\x01') && !fm.contains('\x02'),
        "frontmatter must not contain control chars, got: {fm:?}"
    );
    assert!(fm.contains("Titlewithctrl"));
}

#[test]
fn render_frontmatter_strips_control_chars_from_topic() {
    let item = ResearchItem::new(sample_name(), "title", "topic\x07with\x08ctrl");
    let fm = item.render_frontmatter();
    assert!(
        !fm.contains('\x07') && !fm.contains('\x08'),
        "frontmatter must not contain control chars, got: {fm:?}"
    );
    assert!(fm.contains("topicwithctrl"));
}

#[test]
fn render_frontmatter_strips_control_chars_from_queries() {
    let mut item = ResearchItem::new(sample_name(), "title", "topic");
    item.set_queries(vec!["query\x01with\x02ctrl".into()]);
    let fm = item.render_frontmatter();
    assert!(
        !fm.contains('\x01') && !fm.contains('\x02'),
        "frontmatter must not contain control chars in queries, got: {fm:?}"
    );
    assert!(fm.contains("querywithctrl"));
}
