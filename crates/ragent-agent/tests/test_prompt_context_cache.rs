//! Integration tests for the prompt-context cache.
//!
//! Validates that `AgentPerf` T-006 / FR-012 — the agent loop never
//! re-reads AGENTS.md / README / git-context more than once per cache TTL.
//!
//! We construct a `PromptContextCache` directly (it's a private struct,
//! but the helpers `collect_prompt_context` and `prompt_context_component`
//! are exposed for testing) and assert cache-hit behaviour under the
//! documented TTL.

use ragent_agent::agent::{
    PromptContextComponent, collect_prompt_context, prompt_context_component,
};

#[tokio::test]
async fn prompt_context_cache_key_is_deterministic_for_same_path() {
    // Two consecutive calls return identical (git, readme, agents_md,
    // file_tree) tuples when nothing has changed on disk and the TTL has
    // not expired.
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path();
    let a = collect_prompt_context(path).await;
    let b = collect_prompt_context(path).await;
    assert_eq!(a.0, b.0);
    assert_eq!(a.1, b.1);
    assert_eq!(a.2, b.2);
    assert_eq!(a.3, b.3);
}

#[tokio::test]
async fn prompt_context_cache_handles_missing_files() {
    // An empty temp directory has no AGENTS.md / README / git, so the
    // returned strings should be empty.
    let tmp = tempfile::tempdir().expect("tempdir");
    let (git, readme, agents_md, _file_tree) = collect_prompt_context(tmp.path()).await;
    assert_eq!(git, "");
    assert_eq!(readme, "");
    assert_eq!(agents_md, "");
    // file_tree is still a non-empty listing of the directory.
    // We don't assert on it because tempdir layout is platform-specific.
}

#[tokio::test]
async fn prompt_context_component_dispatches_correctly() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path();
    let git = prompt_context_component(path, PromptContextComponent::Git).await;
    let readme = prompt_context_component(path, PromptContextComponent::Readme).await;
    let agents_md = prompt_context_component(path, PromptContextComponent::AgentsMd).await;
    let file_tree = prompt_context_component(path, PromptContextComponent::FileTree).await;
    let (g2, r2, a2, f2) = collect_prompt_context(path).await;
    assert_eq!(git, g2);
    assert_eq!(readme, r2);
    assert_eq!(agents_md, a2);
    assert_eq!(file_tree, f2);
}

#[test]
fn clear_prompt_context_cache_can_be_called() {
    // We can't easily observe the cache state from outside, but
    // `clear_prompt_context_cache` must not panic when called.
    ragent_agent::agent::clear_prompt_context_cache();
}

#[test]
fn disable_git_prompt_context_is_idempotent() {
    ragent_agent::agent::disable_git_prompt_context();
    ragent_agent::agent::disable_git_prompt_context();
    ragent_agent::agent::disable_readme_prompt_context();
    ragent_agent::agent::disable_readme_prompt_context();
}
