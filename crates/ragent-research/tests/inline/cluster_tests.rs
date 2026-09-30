//! Inline tests for `cluster.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

fn tmp_root() -> (tempfile::TempDir, ResearchName) {
    let dir = tempfile::TempDir::new().expect("tempdir");
    let name = ResearchName::new("cluster-test").expect("valid name");
    (dir, name)
}

#[test]
fn estimate_max_payload_bytes_respects_reserve_and_bytes_per_token() {
    assert_eq!(
        estimate_max_payload_bytes(DEFAULT_CONTEXT_WINDOW_TOKENS),
        (DEFAULT_CONTEXT_WINDOW_TOKENS - PROMPT_RESERVE_TOKENS) * BYTES_PER_TOKEN_GUESS
    );
    assert_eq!(estimate_max_payload_bytes(0), 0);
    assert_eq!(estimate_max_payload_bytes(PROMPT_RESERVE_TOKENS / 2), 0);
}

#[test]
fn resolve_context_window_tokens_uses_registry() {
    let registry = ragent_llm::provider::create_default_registry();
    let ctx =
        resolve_context_window_tokens(Some("gemini"), Some("gemini-2.0-flash"), Some(&registry));
    assert_eq!(ctx, Some(1_048_576));
}

#[test]
fn resolve_context_window_tokens_returns_none_for_missing_model() {
    let registry = ragent_llm::provider::create_default_registry();
    assert!(
        resolve_context_window_tokens(Some("openai"), Some("missing"), Some(&registry)).is_none()
    );
    assert!(resolve_context_window_tokens(None, Some("gpt-4o"), Some(&registry)).is_none());
    assert!(resolve_context_window_tokens(Some("openai"), Some("gpt-4o"), None).is_none());
}

#[test]
fn build_payload_reads_and_concatenates_sources() {
    let (dir, name) = tmp_root();
    let sources = dir.path().join("cluster-test/sources");
    std::fs::create_dir_all(&sources).expect("create sources");
    std::fs::write(sources.join("web-01.md"), "First source body.").expect("write");
    std::fs::write(sources.join("web-02.md"), "Second source body.").expect("write");

    let payload = build_cluster_payload_sync(dir.path(), &name, Some(10_000)).expect("build");
    assert_eq!(payload.files, vec!["web-01.md", "web-02.md"]);
    assert!(payload.text.contains("--- web-01.md ---"));
    assert!(payload.text.contains("--- web-02.md ---"));
    assert!(payload.text.contains("First source body."));
    assert!(payload.text.contains("Second source body."));
    assert!(!payload.truncated);
}

#[test]
fn build_payload_truncates_when_total_exceeds_budget() {
    let (dir, name) = tmp_root();
    let sources = dir.path().join("cluster-test/sources");
    std::fs::create_dir_all(&sources).expect("create sources");
    // Two 3 kB files, each larger than the per-file cap derived from a
    // small context window.
    let big = "x".repeat(3_000);
    std::fs::write(sources.join("web-01.md"), &big).expect("write");
    std::fs::write(sources.join("web-02.md"), &big).expect("write");

    // 5k tokens -> available 904 tokens * 4 bytes/token = 3_616 bytes total,
    // so per-file cap is 1_808 bytes and both 3_000-byte files must be truncated.
    let payload = build_cluster_payload_sync(dir.path(), &name, Some(5_000)).expect("build");
    assert!(payload.truncated);
    assert!(payload.total_bytes <= payload.max_bytes + 200);
    assert!(payload.text.contains("truncated"));
}

#[test]
fn build_payload_is_empty_for_no_sources() {
    let (dir, name) = tmp_root();
    std::fs::create_dir_all(dir.path().join("cluster-test/sources")).expect("create sources");
    let payload = build_cluster_payload_sync(dir.path(), &name, None).expect("build");
    assert!(payload.text.is_empty());
    assert!(payload.files.is_empty());
    assert_eq!(
        payload.max_bytes,
        estimate_max_payload_bytes(DEFAULT_CONTEXT_WINDOW_TOKENS)
    );
}

#[test]
fn build_concept_extraction_prompt_includes_fixed_instructions_and_documents() {
    let payload = ClusterPayload {
        text: "\n\n--- web-01.md ---\n\nAlpha is important.\n\n--- web-02.md ---\n\nBeta ties the sources together.".to_string(),
        files: vec!["web-01.md".to_string(), "web-02.md".to_string()],
        total_bytes: 1,
        max_bytes: 2,
        truncated: false,
    };

    let prompt = build_concept_extraction_prompt(&payload);

    assert!(
        prompt.contains("You are an expert data analyst and researcher."),
        "prompt should contain the fixed persona/instructions"
    );
    assert!(
        prompt.contains("## N. Concept Name"),
        "prompt should request numbered level-2 concept headings"
    );
    assert!(
        prompt.contains("**Definition**"),
        "prompt should request bold definition label"
    );
    assert!(
        prompt.contains("**Key Evidence**"),
        "prompt should request bold evidence label"
    );
    assert!(
        prompt.contains("web-01"),
        "prompt should reference the document-header filename form"
    );
    assert!(
        prompt.contains("[#1]"),
        "prompt should ask the model to cite sources with [#N] markers"
    );
    assert!(
        prompt.contains("# Concepts"),
        "prompt should ask for a top-level Concepts heading"
    );
    assert!(
        prompt.contains(&format!(
            "up to {} core concepts",
            crate::limits::DEFAULT_MAX_CONCEPTS
        )),
        "prompt should inject the default concept cap"
    );
    assert!(
        !prompt.contains("[INSERT_DOCUMENTS_HERE]"),
        "placeholder should be replaced"
    );
    assert!(
        prompt.contains("--- web-01.md ---"),
        "prompt should contain the inserted document separators"
    );
    assert!(prompt.contains("Alpha is important."));
    assert!(prompt.contains("Beta ties the sources together."));
}

#[test]
fn format_concepts_md_normalizes_output() {
    let raw = "\n\n  \n## Alpha\n\n**Definition:** first concept.\n\n**Key Evidence:**\n- appears here\n\n\n\n## Beta\n\n**Definition:** second concept.\n\n  ";
    let out = format_concepts_md(raw);
    assert!(
        out.starts_with("# Concepts\n"),
        "output should begin with # Concepts: {out:?}"
    );
    assert!(out.contains("## Alpha\n"));
    assert!(out.contains("## Beta\n"));
    assert!(out.contains("**Definition:**"));
    assert!(out.contains("- appears here"));
    assert!(
        !out.contains("\n\n\n"),
        "excessive blank lines should be collapsed: {out:?}"
    );
    assert!(out.ends_with('\n'));
    assert!(!out.ends_with("\n\n"));
}

#[test]
fn format_concepts_md_adds_heading_when_missing() {
    let raw = "## Alpha\n\nDefinition of alpha.\n";
    let out = format_concepts_md(raw);
    assert_eq!(out.lines().next().unwrap(), "# Concepts");
    assert!(out.contains("## Alpha"));
}

#[test]
fn format_concepts_md_handles_empty_input() {
    assert_eq!(
        format_concepts_md(""),
        "# Concepts\n\nNo concepts were extracted.\n"
    );
    assert_eq!(
        format_concepts_md("   \n  \n  "),
        "# Concepts\n\nNo concepts were extracted.\n"
    );
}

#[test]
fn build_concept_extraction_prompt_repeats_payload_independently() {
    let payload = ClusterPayload {
        text: "first".to_string(),
        files: vec![],
        total_bytes: 0,
        max_bytes: 0,
        truncated: false,
    };
    let p1 = build_concept_extraction_prompt(&payload);
    let p2 = build_concept_extraction_prompt(&payload);
    assert_eq!(p1, p2, "same payload must produce deterministic prompt");
    assert_eq!(
        p1.matches("first").count(),
        1,
        "payload inserted exactly once"
    );
}

#[test]
fn build_concept_extraction_prompt_for_limit_injects_the_limit() {
    let prompt = build_concept_extraction_prompt_for_limit("body", 3);
    assert!(
        prompt.contains("up to 3 core concepts"),
        "explicit limit must be injected"
    );
    assert!(
        !prompt.contains(CONCEPT_COUNT_INSTRUCTION_PLACEHOLDER),
        "count placeholder must be replaced"
    );
    assert!(!prompt.contains("[INSERT_DOCUMENTS_HERE]"));
    assert!(prompt.contains("body"));
}

#[test]
fn build_concept_extraction_prompt_for_zero_limit_is_unbounded() {
    let prompt = build_concept_extraction_prompt_for_limit("body", 0);
    assert!(
        prompt.contains("Identify every core concept"),
        "zero limit must ask for every concept"
    );
    assert!(
        !prompt.contains("up to 0 core concepts"),
        "zero must not be rendered as a numeric cap"
    );
}

#[test]
fn build_concept_extraction_prompt_uses_the_default_limit() {
    let payload = ClusterPayload {
        text: "payload".to_string(),
        files: vec![],
        total_bytes: 0,
        max_bytes: 0,
        truncated: false,
    };
    let prompt = build_concept_extraction_prompt(&payload);
    assert!(prompt.contains(&format!(
        "up to {} core concepts",
        crate::limits::DEFAULT_MAX_CONCEPTS
    )));
}

#[tokio::test]
async fn write_concepts_md_creates_file() {
    let (dir, name) = tmp_root();
    std::fs::create_dir_all(dir.path().join("cluster-test")).expect("create item dir");
    let path = write_concepts_md(dir.path(), &name, "# Concepts\n\n- Alpha\n")
        .await
        .expect("write");
    assert_eq!(path, dir.path().join("cluster-test/CONCEPTS.md"));
    let content = std::fs::read_to_string(&path).expect("read");
    assert_eq!(content, "# Concepts\n\n- Alpha\n");
}

#[tokio::test]
async fn write_concepts_md_overwrites_existing_file() {
    let (dir, name) = tmp_root();
    let item_dir = dir.path().join("cluster-test");
    std::fs::create_dir_all(&item_dir).expect("create item dir");
    let path = item_dir.join("CONCEPTS.md");
    std::fs::write(&path, "old content").expect("seed old file");
    let returned = write_concepts_md(dir.path(), &name, "new content")
        .await
        .expect("overwrite");
    assert_eq!(returned, path);
    let content = std::fs::read_to_string(&path).expect("read");
    assert_eq!(content, "# Concepts\nnew content\n");
}

#[tokio::test]
async fn write_concepts_md_normalizes_content() {
    let (dir, name) = tmp_root();
    std::fs::create_dir_all(dir.path().join("cluster-test")).expect("create item dir");
    let path = write_concepts_md(
        dir.path(),
        &name,
        "\n\n  \n## Gamma\n\n**Definition:** third concept.\n\n\n\n",
    )
    .await
    .expect("write");
    let content = std::fs::read_to_string(&path).expect("read");
    assert!(content.starts_with("# Concepts\n"));
    // Headings are renumbered sequentially by format_concepts_md_with_sources.
    assert!(content.contains("## 1. Gamma"));
    assert!(!content.contains("\n\n\n"));
    assert!(content.ends_with('\n'));
}
