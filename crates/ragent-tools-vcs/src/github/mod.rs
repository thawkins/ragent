//! GitHub API client and GitHub-backed tools for ragent.

pub mod auth;
pub mod client;
pub mod github_actions;
pub mod github_issues;
pub mod github_prs;

pub use auth::{delete_token, device_flow_login, load_token, save_token};
pub use client::{
    GitHubClient, README_MAX_CHARS, RepoMetadata, build_reverse_prompt, classify_api_error,
    extract_download_url, extract_rate_limit_reset, format_reset_time, parse_root_tree,
};
pub use github_actions::GithubGetActionsTool;
pub use github_issues::{
    GithubCloseIssueTool, GithubCommentIssueTool, GithubCreateIssueTool, GithubGetIssueTool,
    GithubListIssuesTool,
};
pub use github_prs::{
    GithubCreatePrTool, GithubGetPrTool, GithubListPrsTool, GithubMergePrTool, GithubReviewPrTool,
};

pub use crate::{Tool, ToolContext, ToolOutput};
