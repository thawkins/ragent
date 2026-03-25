//! Tests for test_permission.rs

use ragent_core::permission::*;

#[test]
fn test_permission_default_action_is_ask() {
    let checker = PermissionChecker::new(vec![]);
    assert_eq!(
        checker.check("file:read", "/some/path"),
        PermissionAction::Ask
    );
}

#[test]
fn test_permission_allow_rule_matches() {
    let checker = PermissionChecker::new(vec![PermissionRule {
        permission: "file:read".into(),
        pattern: "/home/**".into(),
        action: PermissionAction::Allow,
    }]);
    assert_eq!(
        checker.check("file:read", "/home/user/file.txt"),
        PermissionAction::Allow
    );
}

#[test]
fn test_permission_always_grant_overrides() {
    let mut checker = PermissionChecker::new(vec![]);
    checker.record_always("file:write", "/tmp/**");
    assert_eq!(
        checker.check("file:write", "/tmp/output.txt"),
        PermissionAction::Allow
    );
}
