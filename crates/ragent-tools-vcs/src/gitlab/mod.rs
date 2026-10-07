//! GitLab API client and GitLab-backed tools for ragent.

pub mod auth;
pub mod client;
pub mod gitlab_issues;
pub mod gitlab_mrs;
pub mod gitlab_pipelines;
pub(crate) mod helpers;

pub use auth::{
    GitLabConfig, delete_config, delete_token, load_config, load_token, load_token_checked,
    migrate_legacy_files, save_config, save_token,
};
pub use client::GitLabClient;
pub use gitlab_issues::{
    GitLabCloseIssueTool, GitLabCommentIssueTool, GitLabCreateIssueTool, GitLabGetIssueTool,
    GitLabListIssuesTool,
};
pub use gitlab_mrs::{
    GitLabApproveMrTool, GitLabCreateMrTool, GitLabGetMrTool, GitLabListMrsTool, GitLabMergeMrTool,
};
pub use gitlab_pipelines::{
    GitLabCancelJobTool, GitLabCancelPipelineTool, GitLabGetJobLogTool, GitLabGetJobTool,
    GitLabGetPipelineTool, GitLabListJobsTool, GitLabListPipelinesTool, GitLabRetryJobTool,
    GitLabRetryPipelineTool,
};

pub use crate::{Tool, ToolContext, ToolOutput};
