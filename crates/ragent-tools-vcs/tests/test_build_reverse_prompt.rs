//! Tests for `build_reverse_prompt` (FR-008, NFR-003).
//!
//! Covers: all fields present, missing README (None), empty README,
//! truncated README (>8000 chars), tech-stack injection, empty tree,
//! empty topics.

use ragent_tools_vcs::github::{README_MAX_CHARS, RepoMetadata, build_reverse_prompt};

fn sample_metadata() -> RepoMetadata {
    RepoMetadata {
        description: "A hello-world app".to_string(),
        language: "Rust".to_string(),
        topics: vec!["cli".to_string(), "ai".to_string(), "rust".to_string()],
        stargazers_count: 42,
        default_branch: "main".to_string(),
    }
}

fn sample_tree() -> Vec<String> {
    vec![
        "src".to_string(),
        "Cargo.toml".to_string(),
        "README.md".to_string(),
        ".gitignore".to_string(),
    ]
}

// --- All fields present ---

#[test]
fn test_build_all_fields_present() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello World\n\nThis is a test.".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None);

    assert!(prompt.contains("## Repository Metadata"));
    assert!(prompt.contains("Description: A hello-world app"));
    assert!(prompt.contains("Language: Rust"));
    assert!(prompt.contains("Stars: 42"));
    assert!(prompt.contains("Default branch: main"));
    assert!(prompt.contains("Topics: cli, ai, rust"));

    assert!(prompt.contains("## Root File Tree"));
    assert!(prompt.contains("src"));
    assert!(prompt.contains("Cargo.toml"));
    assert!(prompt.contains("README.md"));
    assert!(prompt.contains(".gitignore"));

    assert!(prompt.contains("## README"));
    assert!(prompt.contains("# Hello World"));
    assert!(prompt.contains("This is a test."));

    // No tech section when tech is None.
    assert!(!prompt.contains("## Technology Stack Constraint"));
}

// --- Missing README (None) ---

#[test]
fn test_build_readme_none_shows_placeholder() {
    let md = sample_metadata();
    let tree = sample_tree();
    let prompt = build_reverse_prompt(&md, &tree, None, None);

    assert!(prompt.contains("## README"));
    assert!(prompt.contains("(no README found)"));
}

// --- Empty README string ---

#[test]
fn test_build_readme_empty_string_shows_placeholder() {
    let md = sample_metadata();
    let tree = sample_tree();
    let prompt = build_reverse_prompt(&md, &tree, Some(""), None);

    assert!(prompt.contains("(no README found)"));
}

// --- Truncated README (>8000 chars) ---

#[test]
fn test_build_readme_truncated_at_8000_chars() {
    let md = sample_metadata();
    let tree = sample_tree();
    // Create a README with 10000 characters.
    let readme = "x".repeat(10_000);
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None);

    assert!(prompt.contains("## README"));
    assert!(prompt.contains("[... README truncated at 8000 characters ...]"));

    // The README content (before the truncation notice) should be exactly
    // README_MAX_CHARS characters of 'x'.
    let readme_section = prompt
        .split("## README\n")
        .nth(1)
        .expect("README section exists");
    let content = readme_section
        .split("\n\n[... README truncated")
        .next()
        .expect("truncation marker exists");
    assert_eq!(content.chars().count(), README_MAX_CHARS);
}

#[test]
fn test_build_readme_exactly_8000_chars_not_truncated() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "y".repeat(README_MAX_CHARS);
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None);

    // Exactly at the limit — no truncation notice.
    assert!(!prompt.contains("[... README truncated"));
    assert!(prompt.contains("## README"));
}

#[test]
fn test_build_readme_just_over_8000_chars_truncated() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "z".repeat(README_MAX_CHARS + 1);
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None);

    assert!(prompt.contains("[... README truncated at 8000 characters ...]"));
}

// --- Truncation works on character boundaries (not byte boundaries) ---

#[test]
fn test_build_readme_truncation_char_boundary_safe() {
    let md = sample_metadata();
    let tree = sample_tree();
    // Use multi-byte chars so we verify char-safe truncation.
    // '€' is 3 bytes in UTF-8. 4000 '€' = 12000 bytes, 4000 chars.
    // We need >8000 chars, so use 9000 '€'.
    let readme = "€".repeat(9000);
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None);

    assert!(prompt.contains("[... README truncated at 8000 characters ...]"));
    // The truncated content should be exactly 8000 '€' characters.
    let readme_section = prompt
        .split("## README\n")
        .nth(1)
        .expect("README section exists");
    let content = readme_section
        .split("\n\n[... README truncated")
        .next()
        .expect("truncation marker exists");
    assert_eq!(content.chars().count(), README_MAX_CHARS);
    assert_eq!(
        content.chars().filter(|c| *c == '€').count(),
        README_MAX_CHARS
    );
}

// --- Tech-stack injection ---

#[test]
fn test_build_tech_stack_included() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), Some("Rust + Tokio"));

    assert!(prompt.contains("## Technology Stack Constraint"));
    assert!(prompt.contains("Rust + Tokio"));
}

#[test]
fn test_build_tech_stack_none_omitted() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None);

    assert!(!prompt.contains("## Technology Stack Constraint"));
}

#[test]
fn test_build_tech_stack_empty_string_still_included() {
    // An empty tech string is still a constraint — it's Some("").
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), Some(""));

    assert!(prompt.contains("## Technology Stack Constraint"));
}

// --- Empty tree ---

#[test]
fn test_build_empty_tree_shows_placeholder() {
    let md = sample_metadata();
    let tree: Vec<String> = vec![];
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None);

    assert!(prompt.contains("## Root File Tree"));
    assert!(prompt.contains("(empty repository)"));
}

// --- Empty topics ---

#[test]
fn test_build_empty_topics_shows_none() {
    let md = RepoMetadata {
        description: "desc".to_string(),
        language: "Go".to_string(),
        topics: vec![],
        stargazers_count: 0,
        default_branch: "master".to_string(),
    };
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None);

    assert!(prompt.contains("Topics: (none)"));
}

// --- Section ordering ---

#[test]
fn test_build_section_ordering_metadata_before_tree_before_readme() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), Some("Tech"));

    let meta_pos = prompt
        .find("## Repository Metadata")
        .expect("metadata section");
    let tech_pos = prompt
        .find("## Technology Stack Constraint")
        .expect("tech section");
    let tree_pos = prompt.find("## Root File Tree").expect("tree section");
    let readme_pos = prompt.find("## README").expect("readme section");

    assert!(meta_pos < tech_pos, "metadata should come before tech");
    assert!(tech_pos < tree_pos, "tech should come before tree");
    assert!(tree_pos < readme_pos, "tree should come before README");
}
