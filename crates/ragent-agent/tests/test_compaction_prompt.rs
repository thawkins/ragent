#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-agent/src/compaction/prompt.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::compaction::prompt::*;

#[test]
fn test_build_prompt_new_summary() {
    let prompt = build_prompt(None, &["[User]: hello\n\n[Assistant]: hi"]);
    assert!(prompt.contains("Create a new anchored summary"));
    assert!(prompt.contains("## Objective"));
    assert!(prompt.contains("## Work State"));
    assert!(prompt.contains("[User]: hello"));
}

#[test]
fn test_build_prompt_update_summary() {
    let prompt = build_prompt(Some("## Objective\n- Old goal"), &["[User]: new info"]);
    assert!(prompt.contains("Update the anchored summary"));
    assert!(prompt.contains("## Objective"));
    assert!(prompt.contains("<previous-summary>"));
    assert!(prompt.contains("[User]: new info"));
}

#[test]
fn test_build_prompt_multiple_contexts() {
    let prompt = build_prompt(None, &["context A", "context B"]);
    let a_pos = prompt.find("context A").unwrap();
    let b_pos = prompt.find("context B").unwrap();
    assert!(b_pos > a_pos);
}
