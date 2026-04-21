//! GitLab API client and GitLab-backed tools for ragent.

pub mod auth;
pub mod client;
pub mod gitlab_issues;
pub mod gitlab_mrs;
pub mod gitlab_pipelines;

pub use auth::{
    GitLabConfig, delete_config, delete_token, load_config, load_token, migrate_legacy_files,
    save_config, save_token,
};
pub use client::GitLabClient;
pub use gitlab_issues::{
    GitlabCloseIssueTool, GitlabCommentIssueTool, GitlabCreateIssueTool, GitlabGetIssueTool,
    GitlabListIssuesTool,
};
pub use gitlab_mrs::{
    GitlabApproveMrTool, GitlabCreateMrTool, GitlabGetMrTool, GitlabListMrsTool, GitlabMergeMrTool,
};
pub use gitlab_pipelines::{
    GitlabCancelJobTool, GitlabCancelPipelineTool, GitlabGetJobLogTool, GitlabGetJobTool,
    GitlabGetPipelineTool, GitlabListJobsTool, GitlabListPipelinesTool, GitlabRetryJobTool,
    GitlabRetryPipelineTool,
};

pub use crate::{Tool, ToolContext, ToolOutput};
