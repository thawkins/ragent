//! Integration tests for `ragent-config` permission parsing & checking.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/permission.rs`
//! (T-005 of the testconsolidate spec).

use ragent_config::{Permission, PermissionAction, PermissionChecker, PermissionRule};

#[test]
fn test_permission_from_flat_names() {
    assert_eq!(Permission::from("read"), Permission::Read);
    assert_eq!(Permission::from("edit"), Permission::Edit);
    assert_eq!(Permission::from("bash"), Permission::Bash);
    assert_eq!(Permission::from("web"), Permission::Web);
}

#[test]
fn test_permission_from_namespaced_categories() {
    // file:read → Read
    assert_eq!(Permission::from("file:read"), Permission::Read);
    // file:write → Edit
    assert_eq!(Permission::from("file:write"), Permission::Edit);
    // bash:execute → Bash
    assert_eq!(Permission::from("bash:execute"), Permission::Bash);
    // network:fetch → Web
    assert_eq!(Permission::from("network:fetch"), Permission::Web);
}

#[test]
fn test_permission_from_case_insensitive() {
    assert_eq!(Permission::from("READ"), Permission::Read);
    assert_eq!(Permission::from("File:Read"), Permission::Read);
    assert_eq!(Permission::from("BASH:EXECUTE"), Permission::Bash);
}

#[test]
fn test_permission_from_aliases() {
    // "write" is an alias for Edit
    assert_eq!(Permission::from("write"), Permission::Edit);
    // "execute" is an alias for Bash
    assert_eq!(Permission::from("execute"), Permission::Bash);
    // "fetch" is an alias for Web
    assert_eq!(Permission::from("fetch"), Permission::Web);
    // "plan" is an alias for PlanEnter
    assert_eq!(Permission::from("plan"), Permission::PlanEnter);
}

#[test]
fn test_permission_checker_with_namespaced_categories() {
    let rules = vec![PermissionRule {
        permission: Permission::Read,
        pattern: Some("**".to_string()),
        action: PermissionAction::Allow,
    }];
    let checker = PermissionChecker::new(rules);

    // Should match even with namespaced category
    assert_eq!(
        checker.check("file:read", "src/main.rs"),
        PermissionAction::Allow
    );
}

#[test]
fn test_permission_checker_with_bash_execute() {
    let rules = vec![PermissionRule {
        permission: Permission::Bash,
        pattern: Some("**".to_string()),
        action: PermissionAction::Deny,
    }];
    let checker = PermissionChecker::new(rules);
    // Should match bash:execute → Bash
    assert_eq!(
        checker.check("bash:execute", "ls -la"),
        PermissionAction::Deny
    );
}