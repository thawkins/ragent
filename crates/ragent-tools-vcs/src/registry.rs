//! Tool registry for the extracted VCS tool set.

use std::sync::Arc;

use crate::ToolRegistry;
use crate::git::{
    GitAddTool, GitBranchTool, GitCheckoutTool, GitCherryPickTool, GitCloneTool, GitCommitTool,
    GitDiffTool, GitFetchTool, GitLogTool, GitMergeTool, GitPullTool, GitPushTool, GitRemoteTool,
    GitResetTool, GitShowTool, GitStashTool, GitStatusTool, GitTagTool,
};
use crate::github::{
    GithubCloseIssueTool, GithubCommentIssueTool, GithubCreateIssueTool, GithubCreatePrTool,
    GithubGetIssueTool, GithubGetPrTool, GithubListIssuesTool, GithubListPrsTool,
    GithubMergePrTool, GithubReviewPrTool,
};
use crate::gitlab::{
    GitlabApproveMrTool, GitlabCancelJobTool, GitlabCancelPipelineTool, GitlabCloseIssueTool,
    GitlabCommentIssueTool, GitlabCreateIssueTool, GitlabCreateMrTool, GitlabGetIssueTool,
    GitlabGetJobLogTool, GitlabGetJobTool, GitlabGetMrTool, GitlabGetPipelineTool,
    GitlabListIssuesTool, GitlabListJobsTool, GitlabListMrsTool, GitlabListPipelinesTool,
    GitlabMergeMrTool, GitlabRetryJobTool, GitlabRetryPipelineTool,
};

/// Create a registry with all extracted VCS tools registered.
#[must_use]
pub fn create_vcs_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();

    registry.register(Arc::new(GithubListIssuesTool));
    registry.register(Arc::new(GithubGetIssueTool));
    registry.register(Arc::new(GithubCreateIssueTool));
    registry.register(Arc::new(GithubCommentIssueTool));
    registry.register(Arc::new(GithubCloseIssueTool));
    registry.register(Arc::new(GithubListPrsTool));
    registry.register(Arc::new(GithubGetPrTool));
    registry.register(Arc::new(GithubCreatePrTool));
    registry.register(Arc::new(GithubMergePrTool));
    registry.register(Arc::new(GithubReviewPrTool));

    // --- Git local workspace tools (Milestone 1) ---
    registry.register(Arc::new(GitStatusTool));
    registry.register(Arc::new(GitLogTool));
    registry.register(Arc::new(GitDiffTool));
    registry.register(Arc::new(GitBranchTool));
    registry.register(Arc::new(GitShowTool));
    registry.register(Arc::new(GitRemoteTool));
    registry.register(Arc::new(GitTagTool));

    // --- Git local workspace tools (Milestone 2) ---
    registry.register(Arc::new(GitAddTool));
    registry.register(Arc::new(GitResetTool));
    registry.register(Arc::new(GitCheckoutTool));
    registry.register(Arc::new(GitCommitTool));
    registry.register(Arc::new(GitStashTool));
    registry.register(Arc::new(GitCherryPickTool));

    // --- Git local workspace tools (Milestone 3) ---
    registry.register(Arc::new(GitPushTool));
    registry.register(Arc::new(GitPullTool));
    registry.register(Arc::new(GitFetchTool));
    registry.register(Arc::new(GitCloneTool));

    // --- Git local workspace tools (Milestone 4) ---
    registry.register(Arc::new(GitMergeTool));

    registry.register(Arc::new(GitlabListIssuesTool));
    registry.register(Arc::new(GitlabGetIssueTool));
    registry.register(Arc::new(GitlabCreateIssueTool));
    registry.register(Arc::new(GitlabCommentIssueTool));
    registry.register(Arc::new(GitlabCloseIssueTool));
    registry.register(Arc::new(GitlabListMrsTool));
    registry.register(Arc::new(GitlabGetMrTool));
    registry.register(Arc::new(GitlabCreateMrTool));
    registry.register(Arc::new(GitlabMergeMrTool));
    registry.register(Arc::new(GitlabApproveMrTool));
    registry.register(Arc::new(GitlabListPipelinesTool));
    registry.register(Arc::new(GitlabGetPipelineTool));
    registry.register(Arc::new(GitlabListJobsTool));
    registry.register(Arc::new(GitlabGetJobTool));
    registry.register(Arc::new(GitlabGetJobLogTool));
    registry.register(Arc::new(GitlabRetryJobTool));
    registry.register(Arc::new(GitlabCancelJobTool));
    registry.register(Arc::new(GitlabRetryPipelineTool));
    registry.register(Arc::new(GitlabCancelPipelineTool));

    registry
}
