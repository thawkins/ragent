//! Tool registry for the extracted VCS tool set.

use std::sync::Arc;

use crate::ToolRegistry;
use crate::git::{
    GitAddTool, GitBranchTool, GitCheckoutTool, GitCherryPickTool, GitCloneTool, GitCommitTool,
    GitDiffTool, GitFetchTool, GitLogTool, GitMergeTool, GitPullTool, GitPushTool, GitRemoteTool,
    GitResetTool, GitShowTool, GitStashTool, GitStatusTool, GitTagTool,
};
use crate::github::{
    GitHubCloseIssueTool, GitHubCommentIssueTool, GitHubCreateIssueTool, GitHubCreatePrTool,
    GitHubGetActionsTool, GitHubGetIssueTool, GitHubGetPrTool, GitHubListIssuesTool,
    GitHubListPrsTool, GitHubMergePrTool, GitHubReviewPrTool,
};
use crate::gitlab::{
    GitLabApproveMrTool, GitLabCancelJobTool, GitLabCancelPipelineTool, GitLabCloseIssueTool,
    GitLabCommentIssueTool, GitLabCreateIssueTool, GitLabCreateMrTool, GitLabGetIssueTool,
    GitLabGetJobLogTool, GitLabGetJobTool, GitLabGetMrTool, GitLabGetPipelineTool,
    GitLabListIssuesTool, GitLabListJobsTool, GitLabListMrsTool, GitLabListPipelinesTool,
    GitLabMergeMrTool, GitLabRetryJobTool, GitLabRetryPipelineTool,
};

/// Register each listed tool type on `$registry` as an `Arc<dyn Tool>`.
macro_rules! register_tools {
    ($registry:expr; $($tool:path),+ $(,)?) => {
        $( $registry.register(Arc::new($tool)); )+
    };
}

/// Create a registry with all extracted VCS tools registered.
#[must_use]
pub fn create_vcs_registry() -> ToolRegistry {
    let registry = ToolRegistry::new();

    register_tools! { registry;
        GitHubListIssuesTool,
        GitHubGetIssueTool,
        GitHubCreateIssueTool,
        GitHubCommentIssueTool,
        GitHubCloseIssueTool,
        GitHubListPrsTool,
        GitHubGetPrTool,
        GitHubCreatePrTool,
        GitHubMergePrTool,
        GitHubReviewPrTool,
        GitHubGetActionsTool,
        // --- Git local workspace tools (Milestone 1) ---
        GitStatusTool,
        GitLogTool,
        GitDiffTool,
        GitBranchTool,
        GitShowTool,
        GitRemoteTool,
        GitTagTool,
        // --- Git local workspace tools (Milestone 2) ---
        GitAddTool,
        GitResetTool,
        GitCheckoutTool,
        GitCommitTool,
        GitStashTool,
        GitCherryPickTool,
        // --- Git local workspace tools (Milestone 3) ---
        GitPushTool,
        GitPullTool,
        GitFetchTool,
        GitCloneTool,
        // --- Git local workspace tools (Milestone 4) ---
        GitMergeTool,
        GitLabListIssuesTool,
        GitLabGetIssueTool,
        GitLabCreateIssueTool,
        GitLabCommentIssueTool,
        GitLabCloseIssueTool,
        GitLabListMrsTool,
        GitLabGetMrTool,
        GitLabCreateMrTool,
        GitLabMergeMrTool,
        GitLabApproveMrTool,
        GitLabListPipelinesTool,
        GitLabGetPipelineTool,
        GitLabListJobsTool,
        GitLabGetJobTool,
        GitLabGetJobLogTool,
        GitLabRetryJobTool,
        GitLabCancelJobTool,
        GitLabRetryPipelineTool,
        GitLabCancelPipelineTool,
    }

    registry
}
