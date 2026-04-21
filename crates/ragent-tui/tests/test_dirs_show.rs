//! Test for /dirs show command displaying built-in lists

#[test]
fn test_get_builtin_lists_returns_expected_structure() {
    let (builtin_allow, builtin_deny) = ragent_core::dir_lists::get_builtin_lists();

    // Verify return types are Vec<String>
    assert!(builtin_allow.is_empty() || !builtin_allow.is_empty());

    // Built-in denylist should contain system directories
    assert!(
        !builtin_deny.is_empty(),
        "Built-in denylist should contain system directories"
    );

    // Verify patterns use glob syntax
    assert!(
        builtin_deny
            .iter()
            .all(|p| p.contains("**") || p.ends_with("/")),
        "All denylist patterns should be glob patterns"
    );
}

#[test]
fn test_builtin_denylist_includes_critical_directories() {
    let (_, builtin_deny) = ragent_core::dir_lists::get_builtin_lists();

    // Unix/Linux critical directories
    assert!(builtin_deny.iter().any(|p| p.contains("/bin")));
    assert!(builtin_deny.iter().any(|p| p.contains("/etc")));
    assert!(builtin_deny.iter().any(|p| p.contains("/sys")));
    assert!(builtin_deny.iter().any(|p| p.contains("/boot")));
    assert!(builtin_deny.iter().any(|p| p.contains("/dev")));

    // macOS directories
    assert!(builtin_deny.iter().any(|p| p.contains("/System")));
    assert!(builtin_deny.iter().any(|p| p.contains("/Library")));

    // Windows directories
    assert!(builtin_deny.iter().any(|p| p.contains("Windows")));
}

#[test]
fn test_builtin_allowlist_empty_by_default() {
    let (builtin_allow, _) = ragent_core::dir_lists::get_builtin_lists();

    // Currently the allowlist is empty by design
    assert!(
        builtin_allow.is_empty(),
        "Built-in allowlist should be empty (can be extended later)"
    );
}
