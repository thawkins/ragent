//! Inline tests for `reverse.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_parse_reverse_args_basic() {
    let args = parse_reverse_args("octocat/Hello-World").unwrap();
    assert_eq!(args.repo_input, "octocat/Hello-World");
    assert!(args.tech.is_none());
    assert!(args.create.is_none());
}

#[test]
fn test_parse_reverse_args_with_tech() {
    let args = parse_reverse_args("octocat/Hello-World --tech Rust").unwrap();
    assert_eq!(args.repo_input, "octocat/Hello-World");
    assert_eq!(args.tech.as_deref(), Some("Rust"));
    assert!(args.create.is_none());
}

#[test]
fn test_parse_reverse_args_with_quoted_tech_double_quotes() {
    let args = parse_reverse_args("octocat/Hello-World --tech \"Next.js + Rails\"").unwrap();
    assert_eq!(args.repo_input, "octocat/Hello-World");
    assert_eq!(args.tech.as_deref(), Some("Next.js + Rails"));
}

#[test]
fn test_parse_reverse_args_with_quoted_tech_single_quotes() {
    let args = parse_reverse_args("octocat/Hello-World --tech 'Vue + Vite'").unwrap();
    assert_eq!(args.tech.as_deref(), Some("Vue + Vite"));
}

#[test]
fn test_tokenize_reverse_args_preserves_inner_quotes() {
    let tokens = tokenize_reverse_args("repo --tech \"a 'b' c\" --depth 2");
    assert_eq!(
        tokens,
        vec![
            "repo".to_string(),
            "--tech".to_string(),
            "a 'b' c".to_string(),
            "--depth".to_string(),
            "2".to_string()
        ]
    );
}

#[test]
fn test_tokenize_reverse_args_empty_quoted_value_splits_nothing() {
    let tokens = tokenize_reverse_args("--tech \"\" Rust");
    assert_eq!(tokens, vec!["--tech".to_string(), "Rust".to_string(),]);
}

#[test]
fn test_parse_reverse_args_with_create() {
    let args = parse_reverse_args("octocat/Hello-World --create my-spec").unwrap();
    assert_eq!(args.repo_input, "octocat/Hello-World");
    assert_eq!(args.create.as_deref(), Some("my-spec"));
    assert!(args.tech.is_none());
}

#[test]
fn test_parse_reverse_args_with_both_flags() {
    let args =
        parse_reverse_args("octocat/Hello-World --tech \"Rust + Tokio\" --create spec1").unwrap();
    assert_eq!(args.repo_input, "octocat/Hello-World");
    assert!(args.tech.is_some());
    assert_eq!(args.create.as_deref(), Some("spec1"));
}

#[test]
fn test_parse_reverse_args_with_depth() {
    let args = parse_reverse_args("octocat/Hello-World --depth 3").unwrap();
    assert_eq!(args.repo_input, "octocat/Hello-World");
    assert_eq!(args.depth.as_deref(), Some("3"));
}

#[test]
fn test_parse_reverse_args_with_all_flags() {
    let args =
        parse_reverse_args("octocat/Hello-World --tech Rust --create spec1 --depth 5").unwrap();
    assert_eq!(args.repo_input, "octocat/Hello-World");
    assert_eq!(args.tech.as_deref(), Some("Rust"));
    assert_eq!(args.create.as_deref(), Some("spec1"));
    assert_eq!(args.depth.as_deref(), Some("5"));
}

#[test]
fn test_parse_reverse_args_url() {
    let args = parse_reverse_args("https://github.com/octocat/Hello-World").unwrap();
    assert_eq!(args.repo_input, "https://github.com/octocat/Hello-World");
}

#[test]
fn test_parse_reverse_args_empty() {
    assert!(parse_reverse_args("").is_none());
}

#[test]
fn test_parse_reverse_args_help() {
    assert!(parse_reverse_args("help").is_none());
}

#[test]
fn test_parse_reverse_args_missing_repo() {
    assert!(parse_reverse_args("--tech Rust").is_none());
}

#[test]
fn test_parse_reverse_args_missing_flag_value() {
    assert!(parse_reverse_args("octocat/Hello-World --tech").is_none());
}

#[test]
fn test_parse_reverse_args_missing_depth_value() {
    assert!(parse_reverse_args("octocat/Hello-World --depth").is_none());
}

#[test]
fn test_parse_reverse_args_flags_before_repo() {
    let args = parse_reverse_args("--tech Rust octocat/Hello-World").unwrap();
    assert_eq!(args.repo_input, "octocat/Hello-World");
    assert_eq!(args.tech.as_deref(), Some("Rust"));
}

#[test]
fn test_quote_arg_wraps_values_with_whitespace() {
    assert_eq!(quote_arg("rust"), "rust");
    assert_eq!(
        quote_arg("next: rust; stack: axum"),
        "\"next: rust; stack: axum\""
    );
    assert_eq!(quote_arg("say \"hi\" now"), "\"say 'hi' now\"");
}

#[test]
fn test_build_spec_reverse_args_folds_scaffold_into_tech() {
    // The validated `/new` flags are rendered as the free-form `--tech`
    // constraint `/reverse` already understands.
    let scaffold = ragent_tools_extended::project_scaffold::parse_flags(&[
        "--language",
        "rust",
        "--type",
        "gui",
        "--stack",
        "gtk4",
    ])
    .expect("valid scaffold flags");
    let args = build_spec_reverse_args(
        "octocat/Hello-World".to_string(),
        Some("my-spec".to_string()),
        Some("3".to_string()),
        Some(scaffold),
        Some("./out".to_string()),
    );
    assert_eq!(args.repo, "octocat/Hello-World");
    assert_eq!(args.create.as_deref(), Some("my-spec"));
    assert_eq!(args.depth.as_deref(), Some("3"));
    assert_eq!(
        args.tech.as_deref(),
        Some("language: rust; type: gui; stack: gtk4")
    );
}

#[test]
fn test_run_spec_reverse_renders_parseable_args() {
    // The rendered argument string must survive the `/reverse` tokenizer
    // unchanged so a scaffolded invocation reaches the same parser.
    let scaffold = ragent_tools_extended::project_scaffold::parse_flags(&[
        "--language",
        "rust",
        "--type",
        "cmdline",
    ])
    .expect("valid scaffold flags");
    let args = build_spec_reverse_args(
        "octocat/Hello-World".to_string(),
        None,
        None,
        Some(scaffold),
        None,
    );
    let rendered = format!(
        "{} --tech {}",
        quote_arg(&args.repo),
        quote_arg(args.tech.as_deref().unwrap_or_default())
    );
    let parsed = parse_reverse_args(&rendered).expect("rendered args parse");
    assert_eq!(parsed.repo_input, "octocat/Hello-World");
    assert_eq!(
        parsed.tech.as_deref(),
        Some("language: rust; type: cmdline")
    );
}

#[test]
fn test_build_llm_task_includes_context() {
    let context = "## Repository Metadata\nDescription: test";
    let task = build_llm_task(context, None);
    assert!(task.contains("reverse-engineer"));
    assert!(task.contains(context));
    assert!(task.contains("Output only the prompt text"));
}

#[test]
fn test_build_llm_task_with_tech() {
    let context = "## Repository Metadata\nDescription: test";
    let task = build_llm_task(context, Some("Rust + Tokio"));
    assert!(task.contains("technology stack: Rust + Tokio"));
}

#[test]
fn test_build_llm_task_without_tech() {
    let context = "test";
    let task = build_llm_task(context, None);
    assert!(!task.contains("technology stack"));
}

#[test]
fn test_provider_label_and_id_github() {
    let provider = VcsProvider::GitHub {
        owner: "octocat".to_string(),
        repo: "Hello-World".to_string(),
    };
    let (repo_id, label) = provider_label_and_id(&provider);
    assert_eq!(repo_id, "octocat/Hello-World");
    assert_eq!(label, "GitHub");
}

#[test]
fn test_provider_label_and_id_gitlab_default() {
    let provider = VcsProvider::GitLab {
        host: None,
        project_path: "group/project".to_string(),
    };
    let (repo_id, label) = provider_label_and_id(&provider);
    assert_eq!(repo_id, "group/project");
    assert_eq!(label, "GitLab");
}

#[test]
fn test_provider_label_and_id_gitlab_self_hosted() {
    let provider = VcsProvider::GitLab {
        host: Some("https://gitlab.example.com".to_string()),
        project_path: "group/project".to_string(),
    };
    let (repo_id, label) = provider_label_and_id(&provider);
    assert_eq!(repo_id, "group/project");
    assert_eq!(label, "GitLab (gitlab.example.com)");
}

#[test]
fn test_reverse_help_message_content() {
    let msg = reverse_help_message();
    assert!(msg.contains("Usage:"));
    assert!(msg.contains("--tech"));
    assert!(msg.contains("--create"));
    assert!(msg.contains("--depth"));
    assert!(msg.contains("/github login"));
    assert!(msg.contains("/gitlab setup"));
    assert!(msg.contains("gitlab:"));
}

#[test]
fn test_reverse_help_message_lists_all_formats() {
    let msg = reverse_help_message();
    assert!(msg.contains("owner/repo"));
    assert!(msg.contains("github:"));
    assert!(msg.contains("gitlab:"));
    assert!(msg.contains("https://github.com"));
    assert!(msg.contains("https://gitlab.com"));
    assert!(msg.contains("git@github.com"));
    assert!(msg.contains("git@gitlab.com"));
}
