#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-agent/src/updater/mod.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::updater::*;

#[test]
fn test_is_newer_same() {
    assert!(!is_newer("0.1.0-alpha.21", "0.1.0-alpha.21"));
}

#[test]
fn test_is_newer_higher_prerelease() {
    assert!(is_newer("0.1.0-alpha.22", "0.1.0-alpha.21"));
}

#[test]
fn test_is_newer_higher_patch() {
    assert!(is_newer("0.1.1", "0.1.0"));
}

#[test]
fn test_is_newer_lower() {
    assert!(!is_newer("0.1.0-alpha.20", "0.1.0-alpha.21"));
}

#[test]
fn test_is_newer_with_v_prefix() {
    assert!(is_newer("v0.2.0", "0.1.0"));
}
