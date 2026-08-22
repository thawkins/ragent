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
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, None);

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
    let prompt = build_reverse_prompt(&md, &tree, None, None, None);

    assert!(prompt.contains("## README"));
    assert!(prompt.contains("(no README found)"));
}

// --- Empty README string ---

#[test]
fn test_build_readme_empty_string_shows_placeholder() {
    let md = sample_metadata();
    let tree = sample_tree();
    let prompt = build_reverse_prompt(&md, &tree, Some(""), None, None);

    assert!(prompt.contains("(no README found)"));
}

// --- Truncated README (>8000 chars) ---

#[test]
fn test_build_readme_truncated_at_8000_chars() {
    let md = sample_metadata();
    let tree = sample_tree();
    // Create a README with 10000 characters.
    let readme = "x".repeat(10_000);
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, None);

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
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, None);

    // Exactly at the limit — no truncation notice.
    assert!(!prompt.contains("[... README truncated"));
    assert!(prompt.contains("## README"));
}

#[test]
fn test_build_readme_just_over_8000_chars_truncated() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "z".repeat(README_MAX_CHARS + 1);
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, None);

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
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, None);

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
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), Some("Rust + Tokio"), None);

    assert!(prompt.contains("## Technology Stack Constraint"));
    assert!(prompt.contains("Rust + Tokio"));
}

#[test]
fn test_build_tech_stack_none_omitted() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, None);

    assert!(!prompt.contains("## Technology Stack Constraint"));
}

#[test]
fn test_build_tech_stack_empty_string_still_included() {
    // An empty tech string is still a constraint — it's Some("").
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), Some(""), None);

    assert!(prompt.contains("## Technology Stack Constraint"));
}

// --- Empty tree ---

#[test]
fn test_build_empty_tree_shows_placeholder() {
    let md = sample_metadata();
    let tree: Vec<String> = vec![];
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, None);

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
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, None);

    assert!(prompt.contains("Topics: (none)"));
}

// --- Section ordering ---

#[test]
fn test_build_section_ordering_metadata_before_tree_before_readme() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), Some("Tech"), None);

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
// --- Provider label (FR-016) ---

#[test]
fn test_build_provider_label_github_emits_source_section() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, Some("GitHub"));

    assert!(prompt.contains("## Repository Source"));
    assert!(prompt.contains("GitHub"));
}

#[test]
fn test_build_provider_label_gitlab_self_hosted_emits_source_section() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(
        &md,
        &tree,
        Some(&readme),
        None,
        Some("GitLab (gitlab.example.com)"),
    );

    assert!(prompt.contains("## Repository Source"));
    assert!(prompt.contains("GitLab (gitlab.example.com)"));
}

#[test]
fn test_build_provider_label_none_omits_source_section() {
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, None);

    assert!(!prompt.contains("## Repository Source"));
}

#[test]
fn test_build_provider_label_empty_string_still_emits_section() {
    // An empty provider label is still Some("") — the section header is emitted.
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, Some(""));

    assert!(prompt.contains("## Repository Source"));
}

#[test]
fn test_build_provider_label_section_before_metadata() {
    // FR-016: the ## Repository Source section must come BEFORE
    // ## Repository Metadata.
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, Some("GitHub"));

    let source_pos = prompt.find("## Repository Source").expect("source section");
    let meta_pos = prompt
        .find("## Repository Metadata")
        .expect("metadata section");

    assert!(
        source_pos < meta_pos,
        "source section should come before metadata"
    );
}

#[test]
fn test_build_provider_label_with_tech_stack_full_ordering() {
    // Full ordering: Source → Metadata → Tech → Tree → README.
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(
        &md,
        &tree,
        Some(&readme),
        Some("Rust"),
        Some("GitLab (gitlab.example.com)"),
    );

    let source_pos = prompt.find("## Repository Source").expect("source section");
    let meta_pos = prompt
        .find("## Repository Metadata")
        .expect("metadata section");
    let tech_pos = prompt
        .find("## Technology Stack Constraint")
        .expect("tech section");
    let tree_pos = prompt.find("## Root File Tree").expect("tree section");
    let readme_pos = prompt.find("## README").expect("readme section");

    assert!(source_pos < meta_pos, "source before metadata");
    assert!(meta_pos < tech_pos, "metadata before tech");
    assert!(tech_pos < tree_pos, "tech before tree");
    assert!(tree_pos < readme_pos, "tree before README");
}
// --- FR-015: reuse for both GitHub and GitLab ---

#[test]
fn test_build_reuse_for_github_metadata() {
    // FR-015: build_reverse_prompt works identically with data sourced from
    // a GitHub repository. The RepoMetadata, tree, and README have the same
    // structure regardless of provider.
    let md = RepoMetadata {
        description: "A GitHub repo".to_string(),
        language: "TypeScript".to_string(),
        topics: vec!["node".to_string(), "express".to_string()],
        stargazers_count: 100,
        default_branch: "main".to_string(),
    };
    let tree = vec!["src".to_string(), "package.json".to_string()];
    let readme = "# GitHub Project".to_string();

    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, Some("GitHub"));

    assert!(prompt.contains("## Repository Source\nGitHub"));
    assert!(prompt.contains("Description: A GitHub repo"));
    assert!(prompt.contains("Language: TypeScript"));
    assert!(prompt.contains("Stars: 100"));
    assert!(prompt.contains("package.json"));
    assert!(prompt.contains("# GitHub Project"));
}

#[test]
fn test_build_reuse_for_gitlab_metadata() {
    // FR-015: build_reverse_prompt works identically with data sourced from
    // a GitLab repository. The same RepoMetadata struct is populated from
    // either provider's API.
    let md = RepoMetadata {
        description: "A GitLab repo".to_string(),
        language: "Rust".to_string(),
        topics: vec!["cli".to_string(), "terminal".to_string()],
        stargazers_count: 25,
        default_branch: "master".to_string(),
    };
    let tree = vec!["src".to_string(), "Cargo.toml".to_string()];
    let readme = "# GitLab Project".to_string();

    let prompt = build_reverse_prompt(
        &md,
        &tree,
        Some(&readme),
        None,
        Some("GitLab (gitlab.example.com)"),
    );

    assert!(prompt.contains("## Repository Source\nGitLab (gitlab.example.com)"));
    assert!(prompt.contains("Description: A GitLab repo"));
    assert!(prompt.contains("Language: Rust"));
    assert!(prompt.contains("Stars: 25"));
    assert!(prompt.contains("Default branch: master"));
    assert!(prompt.contains("Cargo.toml"));
    assert!(prompt.contains("# GitLab Project"));
}

#[test]
fn test_build_reuse_same_metadata_different_provider_label() {
    // FR-015: the same metadata/tree/readme can be paired with different
    // provider labels. Only the ## Repository Source section changes.
    let md = RepoMetadata {
        description: "Shared repo".to_string(),
        language: "Go".to_string(),
        topics: vec![],
        stargazers_count: 0,
        default_branch: "main".to_string(),
    };
    let tree = vec!["main.go".to_string()];
    let readme = "# Shared".to_string();

    let github_prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, Some("GitHub"));
    let gitlab_prompt =
        build_reverse_prompt(&md, &tree, Some(&readme), None, Some("GitLab (gitlab.com)"));

    // Both prompts contain the same metadata/tree/readme content.
    assert!(github_prompt.contains("Description: Shared repo"));
    assert!(gitlab_prompt.contains("Description: Shared repo"));
    assert!(github_prompt.contains("main.go"));
    assert!(gitlab_prompt.contains("main.go"));

    // Only the source section differs.
    assert!(github_prompt.contains("## Repository Source\nGitHub"));
    assert!(gitlab_prompt.contains("## Repository Source\nGitLab (gitlab.com)"));
    assert!(!github_prompt.contains("GitLab"));
    assert!(!gitlab_prompt.contains("GitHub"));
}

#[test]
fn test_build_reuse_with_tech_constraint_both_providers() {
    // FR-015: the tech constraint works identically for both providers.
    let md = RepoMetadata {
        description: "desc".to_string(),
        language: "Python".to_string(),
        topics: vec!["ml".to_string()],
        stargazers_count: 5,
        default_branch: "develop".to_string(),
    };
    let tree = vec!["train.py".to_string()];
    let readme = "# ML".to_string();

    let github_prompt =
        build_reverse_prompt(&md, &tree, Some(&readme), Some("PyTorch"), Some("GitHub"));
    let gitlab_prompt =
        build_reverse_prompt(&md, &tree, Some(&readme), Some("PyTorch"), Some("GitLab"));

    assert!(github_prompt.contains("## Technology Stack Constraint\nPyTorch"));
    assert!(gitlab_prompt.contains("## Technology Stack Constraint\nPyTorch"));
}

// --- FR-016: provider label edge cases ---

#[test]
fn test_build_provider_label_gitlab_default_instance() {
    // FR-016: "GitLab" without a host suffix (configured instance).
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, Some("GitLab"));

    assert!(prompt.contains("## Repository Source\nGitLab"));
    // Ensure the label is exactly "GitLab" not "GitLab (".
    let source_section = prompt
        .split("## Repository Source\n")
        .nth(1)
        .expect("source section");
    let label = source_section.split("\n\n").next().expect("label");
    assert_eq!(label, "GitLab");
}

#[test]
fn test_build_provider_label_gitlab_self_hosted_with_port() {
    // FR-016: self-hosted GitLab with a port in the label.
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(
        &md,
        &tree,
        Some(&readme),
        None,
        Some("GitLab (gitlab.corp.local:8443)"),
    );

    assert!(prompt.contains("## Repository Source\nGitLab (gitlab.corp.local:8443)"));
}

#[test]
fn test_build_provider_label_does_not_appear_in_other_sections() {
    // FR-016: the provider label should only appear in the ## Repository
    // Source section, not leaked into metadata, tree, or README sections.
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(
        &md,
        &tree,
        Some(&readme),
        None,
        Some("GitLab (unique-marker-xyz)"),
    );

    // The marker should appear exactly once (in the source section).
    let count = prompt.matches("unique-marker-xyz").count();
    assert_eq!(count, 1, "provider label should appear exactly once");
}

#[test]
fn test_build_provider_label_section_is_first() {
    // FR-016: when a provider label is supplied, the ## Repository Source
    // section is the very first section in the output.
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), Some("Rust"), Some("GitHub"));

    assert!(
        prompt.starts_with("## Repository Source\n"),
        "prompt should start with the Repository Source section"
    );
}

#[test]
fn test_build_no_provider_label_metadata_is_first() {
    // FR-016: when no provider label is supplied, the ## Repository Metadata
    // section is the first section (backward compat).
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, None);

    assert!(
        prompt.starts_with("## Repository Metadata\n"),
        "prompt should start with the Repository Metadata section when no label"
    );
    assert!(!prompt.contains("## Repository Source"));
}

#[test]
fn test_build_provider_label_with_empty_tree_and_no_readme() {
    // FR-016: the source section is emitted even when tree is empty and
    // README is missing.
    let md = sample_metadata();
    let tree: Vec<String> = vec![];
    let prompt = build_reverse_prompt(&md, &tree, None, None, Some("GitHub"));

    assert!(prompt.contains("## Repository Source\nGitHub"));
    assert!(prompt.contains("(empty repository)"));
    assert!(prompt.contains("(no README found)"));
}

#[test]
fn test_build_provider_label_multiline_label_preserved() {
    // FR-016: a label containing newlines is preserved as-is in the section.
    let md = sample_metadata();
    let tree = sample_tree();
    let readme = "# Hello".to_string();
    let label = "GitLab\nSelf-hosted instance";
    let prompt = build_reverse_prompt(&md, &tree, Some(&readme), None, Some(label));

    assert!(prompt.contains("## Repository Source\nGitLab\nSelf-hosted instance"));
}
