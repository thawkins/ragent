//! Tests for the `/research cluster` CONCEPTS.md formatting upgrades:
//! sequential concept numbering and `**WebSources:**` reference blocks in the
//! same style as the `## Findings` sources lists in `RESEARCH.md`.

use ragent_research::{WebSourceMeta, format_concepts_md_with_sources, load_web_source_metadata};

fn meta(index: usize, url: &str, title: &str, author: &str, published: &str) -> WebSourceMeta {
    WebSourceMeta {
        index,
        file: format!("web-{index:02}.md"),
        url: url.to_string(),
        title: title.to_string(),
        author: author.to_string(),
        published: published.to_string(),
    }
}

#[test]
fn test_format_concepts_md_with_sources_renumbers_headings_sequentially() {
    let raw = "# Concepts\n\n## 3. Out of Order\n\n**Definition:** one.\n\n## Miscounted\n\n**Definition:** two.\n\n## 7 - Skipped Ahead\n\n**Definition:** three.\n\n## 4: Colon Style\n\n**Definition:** four.\n";
    let out = format_concepts_md_with_sources(raw, &[]);
    assert!(out.contains("## 1. Out of Order"), "{out}");
    assert!(out.contains("## 2. Miscounted"), "{out}");
    assert!(out.contains("## 3. Skipped Ahead"), "{out}");
    assert!(out.contains("## 4. Colon Style"), "{out}");
    assert!(!out.contains("## 3. Out of Order"), "{out}");
    assert!(!out.contains("## 7"), "{out}");
}

#[test]
fn test_format_concepts_md_with_sources_leaves_numbered_input_clean() {
    let raw = "# Concepts\n\n## 1. First Concept\n\n**Definition:** one.\n\n## 2. Second Concept\n\n**Definition:** two.\n";
    let out = format_concepts_md_with_sources(raw, &[]);
    assert!(out.contains("## 1. First Concept"), "{out}");
    assert!(out.contains("## 2. Second Concept"), "{out}");
}

#[test]
fn test_format_concepts_md_with_sources_appends_web_sources_block() {
    let raw = "# Concepts\n\n## 1. Semantic Search\n\n**Definition:** Retrieval over scholarly corpora.\n\n**Key Evidence:**\n- hybrid retrieval across 200M+ papers (web-01)\n- understands meaning, not keywords (web-02)\n";
    let sources = vec![
        meta(
            1,
            "https://example.com/one",
            "Guide to AI Research Tools",
            "Jane Doe",
            "2025-05-26",
        ),
        meta(2, "https://example.com/two", "Second Source", "", ""),
    ];
    let out = format_concepts_md_with_sources(raw, &sources);
    assert!(out.contains("**WebSources:**"), "{out}");
    // Inline `web-NN` citations are rewritten to the `[#N]` RESEARCH.md style.
    assert!(
        out.contains("hybrid retrieval across 200M+ papers ([#1])"),
        "{out}"
    );
    assert!(!out.contains("web-01"), "{out}");
    assert!(!out.contains("web-02"), "{out}");
    // Bullet style must match RESEARCH.md render_finding_sources:
    // - [N] Title [Author] — URL (published YYYY-MM-DD), URL linkified.
    assert!(
        out.contains(
            "- [1] Guide to AI Research Tools [Jane Doe] — [https://example.com/one](https://example.com/one) (published 2025-05-26)"
        ),
        "{out}"
    );
    assert!(
        out.contains("- [2] Second Source — [https://example.com/two](https://example.com/two)"),
        "{out}"
    );
}

#[test]
fn test_format_concepts_md_with_sources_accepts_hash_citation_style() {
    // Models following the updated prompt emit `[#N]` directly; they must be
    // left untouched and still produce the WebSources block.
    let raw = "# Concepts\n\n## 1. Semantic Search\n\n**Key Evidence:**\n- hybrid retrieval across 200M+ papers ([#1])\n";
    let sources = vec![meta(1, "https://example.com/one", "Guide", "", "")];
    let out = format_concepts_md_with_sources(raw, &sources);
    assert!(out.contains("([#1])"), "{out}");
    assert!(out.contains("**WebSources:**"), "{out}");
}

#[test]
fn test_format_concepts_md_with_sources_skips_unknown_and_duplicate_refs() {
    let raw = "# Concepts\n\n## 1. Known Only\n\n**Definition:** cites web-01, web-99, and web-01 again.\n";
    let sources = vec![meta(1, "https://example.com/one", "Known Source", "", "")];
    let out = format_concepts_md_with_sources(raw, &sources);
    // Inline citations are rewritten to `[#N]` even when the source is unknown
    // (web-99 has no captured metadata, so it gets no WebSources bullet).
    assert!(out.contains("cites [#1], [#99], and [#1] again"), "{out}");
    assert!(out.contains("**WebSources:**"), "{out}");
    assert_eq!(out.matches("- [1] Known Source").count(), 1, "{out}");
    assert!(!out.contains("- [99]"), "{out}");
}

#[test]
fn test_format_concepts_md_with_sources_no_refs_no_block() {
    let raw = "# Concepts\n\n## 1. No Citations\n\n**Definition:** plain concept.\n";
    let sources = vec![meta(1, "https://example.com/one", "Known Source", "", "")];
    let out = format_concepts_md_with_sources(raw, &sources);
    assert!(!out.contains("**WebSources:**"), "{out}");
}

#[test]
fn test_format_concepts_md_with_sources_empty_input_unchanged() {
    assert_eq!(
        format_concepts_md_with_sources("", &[]),
        "# Concepts\n\nNo concepts were extracted.\n"
    );
}

#[test]
fn test_load_web_source_metadata_parses_header_fields() {
    let dir = tempfile::TempDir::new().unwrap();
    let name = ragent_research::ResearchName::new("cluster-meta").unwrap();
    let sources = dir.path().join("cluster-meta/sources");
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::write(
        sources.join("web-02.md"),
        "# Web source\n\n\
         - URL: https://example.com/two\n\
         - Title: Second Source\n\
         - Author(s): —\n\
         - Language: English\n\
         - Published (UTC): 2025-05-26T00:00:00+00:00\n\
         - Captured (UTC): 2026-09-02T07:14:23+00:00\n\
         - Relevance: Medium\n\n\
         ```text\nbody\n```\n",
    )
    .unwrap();
    std::fs::write(
        sources.join("web-01.md"),
        "# Web source\n\n\
         - URL: https://example.com/one\n\
         - Title: First Source\n\
         - Author(s): Jane Doe\n\
         - Published (UTC): —\n\n\
         ```text\nbody\n```\n",
    )
    .unwrap();
    std::fs::write(sources.join("local-01.md"), "not a web source").unwrap();

    let metas = load_web_source_metadata(dir.path(), &name).unwrap();
    assert_eq!(metas.len(), 2, "{metas:?}");
    assert_eq!(metas[0].index, 1);
    assert_eq!(metas[0].title, "First Source");
    assert_eq!(metas[0].author, "Jane Doe");
    assert_eq!(metas[0].published, "", "unknown date renders as -");
    assert_eq!(metas[1].index, 2);
    assert_eq!(metas[1].url, "https://example.com/two");
    assert_eq!(metas[1].author, "", "author - placeholder maps to empty");
    assert_eq!(metas[1].published, "2025-05-26");
}

#[test]
fn test_load_web_source_metadata_missing_dir_is_error() {
    let dir = tempfile::TempDir::new().unwrap();
    let name = ragent_research::ResearchName::new("cluster-absent").unwrap();
    assert!(load_web_source_metadata(dir.path(), &name).is_err());
}

#[tokio::test]
async fn test_write_concepts_md_end_to_end_numbered_and_sourced() {
    let dir = tempfile::TempDir::new().unwrap();
    let name = ragent_research::ResearchName::new("cluster-e2e").unwrap();
    let sources = dir.path().join("cluster-e2e/sources");
    std::fs::create_dir_all(&sources).unwrap();
    std::fs::write(
        sources.join("web-01.md"),
        "# Web source\n\n\
         - URL: https://example.com/one\n\
         - Title: First Source\n\
         - Author(s): Jane Doe\n\
         - Published (UTC): 2025-05-26T00:00:00+00:00\n\n\
         ```text\nbody\n```\n",
    )
    .unwrap();

    let raw = "# Concepts\n\n## AI Research Assistants\n\n**Definition:** tools that accelerate research.\n\n**Key Evidence:**\n- top-8 platform ranking (web-01)\n";
    let path = ragent_research::write_concepts_md(dir.path(), &name, raw)
        .await
        .unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(
        content.contains("## 1. AI Research Assistants"),
        "{content}"
    );
    assert!(content.contains("([#1])"), "{content}");
    assert!(!content.contains("web-01"), "{content}");
    assert!(content.contains("**WebSources:**"), "{content}");
    assert!(
        content.contains(
            "- [1] First Source [Jane Doe] — [https://example.com/one](https://example.com/one) (published 2025-05-26)"
        ),
        "{content}"
    );
}
