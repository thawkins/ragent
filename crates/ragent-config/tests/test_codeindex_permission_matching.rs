//! Permission-system tests for codeindex tool categories.

use ragent_config::permission::{Permission, PermissionAction, PermissionChecker, PermissionRule};

#[test]
fn test_codeindex_read_rules_are_not_normalized_to_custom_permissions() {
    let checker = PermissionChecker::new(vec![PermissionRule {
        permission: Permission::Custom("tool:codeindex_search".to_string()),
        pattern: Some("*".to_string()),
        action: PermissionAction::Allow,
    }]);

    assert_eq!(
        checker.check("codeindex:read", "anything"),
        PermissionAction::Ask,
        "custom tool:codeindex_search rule does not match codeindex:read"
    );
}

#[test]
fn test_codeindex_read_and_write_can_be_hardwired_with_explicit_custom_rules() {
    let checker = PermissionChecker::new(vec![
        PermissionRule {
            permission: Permission::Custom("codeindex:read".to_string()),
            pattern: Some("*".to_string()),
            action: PermissionAction::Allow,
        },
        PermissionRule {
            permission: Permission::Custom("codeindex:write".to_string()),
            pattern: Some("*".to_string()),
            action: PermissionAction::Allow,
        },
    ]);

    assert_eq!(
        checker.check("codeindex:read", "anything"),
        PermissionAction::Allow
    );
    assert_eq!(
        checker.check("codeindex:write", "anything"),
        PermissionAction::Allow
    );
}
