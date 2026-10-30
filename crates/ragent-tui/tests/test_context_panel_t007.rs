//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-007**: Compute skills, memory and AGENTS.md partition sizes.
//!   Also covers the FR-005 system-prompt estimate used by the partitions.

mod support;

#[test]
fn test_system_prompt_token_count_is_positive() {
    // FR-005: the assembled system prompt is never empty (the builder always
    // appends the reasoning-tool guidance), so the estimate must be positive.
    let app = support::make_app();
    let count = app.system_prompt_token_count();
    assert!(
        count > 0,
        "system prompt token count should be positive; got {count}"
    );
}

#[test]
fn test_agents_md_token_count_is_non_negative() {
    // FR-009: the AGENTS.md partition reports the byte size of the guideline
    // block; zero is valid when no guideline file exists, but it must not
    // be absurdly large relative to a real file.
    let app = support::make_app();
    let count = app.agents_md_token_count();
    assert!(
        count < 1_000_000,
        "AGENTS.md partition implausibly large: {count}"
    );
}

#[test]
fn test_memory_injection_token_count_is_non_negative() {
    // FR-009: memory injections may legitimately be zero (no memories for the
    // working directory), but the count must be a bounded slice, never a
    // wild value from an underflow.
    let app = support::make_app();
    let count = app.memory_injection_token_count();
    assert!(
        count < 1_000_000,
        "memory injection partition implausibly large: {count}"
    );
    // It is a slice of the whole prompt, so it can never exceed it.
    assert!(
        count <= app.system_prompt_token_count(),
        "memory injection partition larger than the system prompt"
    );
}

#[test]
fn test_skills_token_count_is_non_negative() {
    // FR-009: the skills partition is zero when no agent-invocable skills
    // resolve, positive otherwise; either way it must be a bounded slice.
    let app = support::make_app();
    let count = app.skills_token_count();
    assert!(
        count < 1_000_000,
        "skills partition implausibly large: {count}"
    );
    assert!(
        count <= app.system_prompt_token_count(),
        "skills partition larger than the system prompt"
    );
}

#[test]
fn test_prompt_sub_partitions_fit_inside_system_prompt() {
    // FR-009: each sub-partition is a slice of the assembled system prompt,
    // so individually they cannot exceed the whole prompt.
    let app = support::make_app();
    let total = app.system_prompt_token_count();
    assert!(
        app.agents_md_token_count() <= total,
        "AGENTS.md partition larger than system prompt"
    );
    assert!(
        app.memory_injection_token_count() <= total,
        "memory partition larger than system prompt"
    );
    assert!(
        app.skills_token_count() <= total,
        "skills partition larger than system prompt"
    );
}
