//! Inline tests for `source.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

fn dt() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2024-01-15T10:30:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

fn web_with_oa_recovery() -> Source {
    Source::Web {
        published_at: None,
        url: "https://doi.org/10.1234/example".into(),
        title: "Example paper".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "full text".into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: Some(Box::new(crate::open_access::RecoveredOpenAccess {
            url: "https://pmc.ncbi.nlm.nih.gov/articles/PMC123456/".into(),
            source: crate::open_access::RecoverySource::EuropePmc,
            license: Some("CC-BY-4.0".into()),
            version: Some("publishedVersion".into()),
        })),
        author: None,
    }
}

#[test]
fn oa_recovery_note_includes_source_url_version_and_license() {
    let source = web_with_oa_recovery();
    let note = source.oa_recovery_note().expect("should have note");
    assert!(
        note.contains("europepmc"),
        "note should name service: {note}"
    );
    assert!(
        note.contains("pmc.ncbi.nlm.nih.gov/articles/PMC123456/"),
        "note should include recovered URL: {note}"
    );
    assert!(
        note.contains("publishedVersion"),
        "note should include version: {note}"
    );
    assert!(
        note.contains("CC-BY-4.0"),
        "note should include license: {note}"
    );
}

#[test]
fn oa_recovery_note_is_none_without_recovery() {
    let web = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "page text".into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert!(web.oa_recovery_note().is_none());
}

#[test]
fn type_str_for_each_variant() {
    let web = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "page text".into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(web.type_str(), "web");

    let local = Source::Local {
        path: "src/lib.rs".into(),
        kind: LocalSourceKind::InProject,
        captured_at: dt(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: "Main library entry".into(),
        body: "fn main() {}".into(),
    };
    assert_eq!(local.type_str(), "local");

    let extra = Source::Local {
        path: "extra/notes.md".into(),
        kind: LocalSourceKind::Extra,
        captured_at: dt(),
        body_path: PathBuf::from("sources/local-02.md"),
        relevance: "External notes".into(),
        body: String::new(),
    };
    assert_eq!(extra.type_str(), "extra-local");

    let spec = Source::Spec {
        spec_id: "researchsystem".into(),
        captured_at: dt(),
        relevance: "The spec under design".into(),
    };
    assert_eq!(spec.type_str(), "spec");

    let other = Source::Other {
        label: "Interview transcript".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/other-01.md"),
        body: "Q: ... A: ...".into(),
    };
    assert_eq!(other.type_str(), "other");
}

#[test]
fn title_and_path_or_url_for_each_variant() {
    let web = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: String::new(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(web.title(), "Example");
    assert_eq!(web.path_or_url(), "https://example.com");

    let local = Source::Local {
        path: "src/lib.rs".into(),
        kind: LocalSourceKind::InProject,
        captured_at: dt(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: "Main library entry".into(),
        body: String::new(),
    };
    assert_eq!(local.title(), "src/lib.rs");
    assert_eq!(local.path_or_url(), "src/lib.rs");

    let spec = Source::Spec {
        spec_id: "researchsystem".into(),
        captured_at: dt(),
        relevance: "The spec under design".into(),
    };
    assert_eq!(spec.title(), "researchsystem");
    assert_eq!(spec.path_or_url(), "researchsystem");
}

#[test]
fn captured_at_is_accessible_for_each_variant() {
    let now = dt();
    let web = Source::Web {
        published_at: None,
        url: "u".into(),
        title: "t".into(),
        captured_at: now,
        body_path: PathBuf::from("x"),
        body: String::new(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(web.captured_at(), now);
}

#[test]
fn serde_round_trip_web() {
    let s = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "page text".into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: Source = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}

#[test]
fn web_author_accessor_returns_extracted_name() {
    let with_author = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "page text".into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: Some("Jane Doe".into()),
    };
    assert_eq!(with_author.author(), Some("Jane Doe"));

    let without_author = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "page text".into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(without_author.author(), None);

    let local = Source::Local {
        path: "src/lib.rs".into(),
        kind: LocalSourceKind::InProject,
        captured_at: dt(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: String::new(),
        body: "code".into(),
    };
    assert_eq!(local.author(), None);
}

#[test]
fn serde_round_trip_web_backward_compatible_without_body() {
    let json = r#"{
        "source_type": "web",
        "url": "https://example.com",
        "title": "Example",
        "captured_at": "2024-01-15T10:30:00Z",
        "body_path": "sources/web-01.md"
    }"#;
    let s: Source = serde_json::from_str(json).unwrap();
    if let Source::Web { body, .. } = s {
        assert_eq!(body, "");
    } else {
        panic!("expected Web variant");
    }
}

#[test]
fn serde_round_trip_local_with_default_kind() {
    let json = r#"{
        "source_type": "local",
        "path": "src/lib.rs",
        "captured_at": "2024-01-15T10:30:00Z",
        "body_path": "sources/local-01.md",
        "relevance": "Entry point"
    }"#;
    let s: Source = serde_json::from_str(json).unwrap();
    if let Source::Local { kind, body, .. } = s {
        assert_eq!(kind, LocalSourceKind::InProject);
        assert_eq!(body, "");
    } else {
        panic!("expected Local variant");
    }
}

#[test]
fn serde_round_trip_spec() {
    let s = Source::Spec {
        spec_id: "foo".into(),
        captured_at: dt(),
        relevance: "Related".into(),
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: Source = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}

#[test]
fn serde_round_trip_other() {
    let s = Source::Other {
        label: "PDF".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/other-01.md"),
        body: "transcript".into(),
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: Source = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
}

#[test]
fn serde_round_trip_other_backward_compatible_without_body() {
    let json = r#"{
        "source_type": "other",
        "label": "PDF",
        "captured_at": "2024-01-15T10:30:00Z",
        "body_path": "sources/other-01.md"
    }"#;
    let s: Source = serde_json::from_str(json).unwrap();
    if let Source::Other { body, .. } = s {
        assert_eq!(body, "");
    } else {
        panic!("expected Other variant");
    }
}

#[test]
fn body_and_has_body_for_each_variant() {
    let web = Source::Web {
        published_at: None,
        url: "u".into(),
        title: "t".into(),
        captured_at: dt(),
        body_path: PathBuf::from("x"),
        body: "hello".into(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(web.body(), Some("hello"));
    assert!(web.has_body());

    let empty = Source::Web {
        published_at: None,
        url: "u".into(),
        title: "t".into(),
        captured_at: dt(),
        body_path: PathBuf::from("x"),
        body: String::new(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(empty.body(), Some(""));
    assert!(!empty.has_body());

    let spec = Source::Spec {
        spec_id: "s".into(),
        captured_at: dt(),
        relevance: String::new(),
    };
    assert_eq!(spec.body(), None);
    assert!(!spec.has_body());
}

#[test]
fn local_source_kind_default() {
    assert_eq!(LocalSourceKind::default(), LocalSourceKind::InProject);
}

#[test]
fn serde_round_trip_web_with_relevance() {
    let s = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "page text".into(),
        relevance: "High - title matches query terms".into(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: Source = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s);
    assert_eq!(back.relevance(), Some("High - title matches query terms"));
}

#[test]
fn web_relevance_returns_empty_as_some() {
    let s = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: String::new(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(s.relevance(), Some(""));
}

// ── Milestone E-003: relevance_rank tests ─────────────────────────────

#[test]
fn relevance_rank_very_high_is_8() {
    let s = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "text".into(),
        relevance: "Very high - exact title match".into(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(s.relevance_rank(), 8);
}

#[test]
fn relevance_rank_high_is_7() {
    let s = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "text".into(),
        relevance: "High - title matches query".into(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(s.relevance_rank(), 7);
}

#[test]
fn relevance_rank_medium_is_5() {
    let s = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "text".into(),
        relevance: "Medium - partial query match".into(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(s.relevance_rank(), 5);
}

#[test]
fn relevance_rank_low_is_3() {
    let s = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "text".into(),
        relevance: "Low - weak query match".into(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(s.relevance_rank(), 3);
}

#[test]
fn relevance_rank_very_low_is_1() {
    let s = Source::Web {
        published_at: None,
        url: "https://example.com".into(),
        title: "Example".into(),
        captured_at: dt(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "text".into(),
        relevance: "Very low - no clear query match".into(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    };
    assert_eq!(s.relevance_rank(), 1);
}

#[test]
fn relevance_rank_no_relevance_defaults_to_5() {
    let s = Source::Local {
        path: "src/lib.rs".into(),
        kind: LocalSourceKind::InProject,
        captured_at: dt(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: String::new(),
        body: "fn main() {}".into(),
    };
    assert_eq!(s.relevance_rank(), 5);
}
