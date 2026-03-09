use ragent_core::permission::*;

// ── Last-match-wins semantics ────────────────────────────────────

#[test]
fn test_permission_last_match_wins() {
    let checker = PermissionChecker::new(vec![
        PermissionRule {
            permission: "file:write".into(),
            pattern: "**".to_string(),
            action: PermissionAction::Allow,
        },
        PermissionRule {
            permission: "file:write".into(),
            pattern: "/secret/**".to_string(),
            action: PermissionAction::Deny,
        },
    ]);

    assert_eq!(
        checker.check("file:write", "/home/user/file.txt"),
        PermissionAction::Allow,
    );
    assert_eq!(
        checker.check("file:write", "/secret/passwords.txt"),
        PermissionAction::Deny,
    );
}

// ── Deny overrides earlier Allow ─────────────────────────────────

#[test]
fn test_permission_deny_overrides_allow() {
    let checker = PermissionChecker::new(vec![
        PermissionRule {
            permission: "bash:execute".into(),
            pattern: "*".to_string(),
            action: PermissionAction::Allow,
        },
        PermissionRule {
            permission: "bash:execute".into(),
            pattern: "rm*".to_string(),
            action: PermissionAction::Deny,
        },
    ]);

    assert_eq!(
        checker.check("bash:execute", "ls -la"),
        PermissionAction::Allow,
    );
    assert_eq!(
        checker.check("bash:execute", "rm -rf /"),
        PermissionAction::Deny,
    );
}

// ── Wildcard permission matches all ──────────────────────────────

#[test]
fn test_permission_wildcard_matches_all() {
    let checker = PermissionChecker::new(vec![PermissionRule {
        permission: Permission::Custom("*".to_string()),
        pattern: "**".to_string(),
        action: PermissionAction::Allow,
    }]);

    assert_eq!(
        checker.check("file:read", "/any/path"),
        PermissionAction::Allow,
    );
    assert_eq!(
        checker.check("bash:execute", "anything"),
        PermissionAction::Allow,
    );
    assert_eq!(
        checker.check("custom_permission", "/foo"),
        PermissionAction::Allow,
    );
}

// ── Custom permission type ───────────────────────────────────────

#[test]
fn test_permission_custom_type() {
    let checker = PermissionChecker::new(vec![PermissionRule {
        permission: Permission::Custom("deploy".to_string()),
        pattern: "production/**".to_string(),
        action: PermissionAction::Deny,
    }]);

    assert_eq!(
        checker.check("deploy", "production/app.yaml"),
        PermissionAction::Deny,
    );
    assert_eq!(
        checker.check("deploy", "staging/app.yaml"),
        PermissionAction::Ask,
    );
}

// ── record_always overrides rules ────────────────────────────────

#[test]
fn test_permission_always_overrides_deny_rule() {
    let mut checker = PermissionChecker::new(vec![PermissionRule {
        permission: "file:write".into(),
        pattern: "**".to_string(),
        action: PermissionAction::Deny,
    }]);

    assert_eq!(
        checker.check("file:write", "/tmp/out.txt"),
        PermissionAction::Deny,
    );

    checker.record_always("file:write", "/tmp/**");

    assert_eq!(
        checker.check("file:write", "/tmp/out.txt"),
        PermissionAction::Allow,
    );
    // Non-matching paths still denied
    assert_eq!(
        checker.check("file:write", "/home/out.txt"),
        PermissionAction::Deny,
    );
}

// ── Multiple always grants ───────────────────────────────────────

#[test]
fn test_permission_multiple_always_grants() {
    let mut checker = PermissionChecker::new(vec![]);

    checker.record_always("file:read", "/src/**");
    checker.record_always("file:read", "/tests/**");

    assert_eq!(
        checker.check("file:read", "/src/main.rs"),
        PermissionAction::Allow,
    );
    assert_eq!(
        checker.check("file:read", "/tests/test.rs"),
        PermissionAction::Allow,
    );
    assert_eq!(
        checker.check("file:read", "/build/output"),
        PermissionAction::Ask,
    );
}

// ── No matching rule returns Ask ─────────────────────────────────

#[test]
fn test_permission_no_match_returns_ask() {
    let checker = PermissionChecker::new(vec![PermissionRule {
        permission: "file:read".into(),
        pattern: "/specific/path/**".to_string(),
        action: PermissionAction::Allow,
    }]);

    assert_eq!(
        checker.check("file:write", "/specific/path/foo"),
        PermissionAction::Ask,
    );
    assert_eq!(
        checker.check("file:read", "/other/path/foo"),
        PermissionAction::Ask,
    );
}

// ── Complex glob patterns ────────────────────────────────────────

#[test]
fn test_permission_complex_glob_patterns() {
    let checker = PermissionChecker::new(vec![
        PermissionRule {
            permission: "file:read".into(),
            pattern: "**/*.rs".to_string(),
            action: PermissionAction::Allow,
        },
        PermissionRule {
            permission: "file:read".into(),
            pattern: "**/target/**".to_string(),
            action: PermissionAction::Deny,
        },
    ]);

    assert_eq!(
        checker.check("file:read", "src/main.rs"),
        PermissionAction::Allow,
    );
    assert_eq!(
        checker.check("file:read", "target/debug/ragent.rs"),
        PermissionAction::Deny,
    );
    assert_eq!(
        checker.check("file:read", "README.md"),
        PermissionAction::Ask,
    );
}

// ── Permission enum round-trip ───────────────────────────────────

#[test]
fn test_permission_enum_from_str() {
    let known = vec![
        ("read", Permission::Read),
        ("edit", Permission::Edit),
        ("bash", Permission::Bash),
        ("web", Permission::Web),
        ("question", Permission::Question),
        ("plan_enter", Permission::PlanEnter),
        ("plan_exit", Permission::PlanExit),
        ("todo", Permission::Todo),
        ("external_directory", Permission::ExternalDirectory),
        ("doom_loop", Permission::DoomLoop),
    ];
    for (s, expected) in &known {
        let parsed = Permission::from(*s);
        assert_eq!(&parsed, expected, "Failed for: {}", s);
    }

    let custom = Permission::from("custom_thing");
    assert_eq!(custom, Permission::Custom("custom_thing".to_string()));
}

// ── Permission Display ───────────────────────────────────────────

#[test]
fn test_permission_display() {
    assert_eq!(Permission::Read.to_string(), "read");
    assert_eq!(Permission::Edit.to_string(), "edit");
    assert_eq!(Permission::Bash.to_string(), "bash");
    assert_eq!(Permission::Custom("deploy".into()).to_string(), "deploy");
}

// ── PermissionAction Display ─────────────────────────────────────

#[test]
fn test_permission_action_display() {
    assert_eq!(PermissionAction::Allow.to_string(), "allow");
    assert_eq!(PermissionAction::Deny.to_string(), "deny");
    assert_eq!(PermissionAction::Ask.to_string(), "ask");
}

// ── PermissionAction serde ───────────────────────────────────────

#[test]
fn test_permission_action_serde() {
    for action in &[PermissionAction::Allow, PermissionAction::Deny, PermissionAction::Ask] {
        let json = serde_json::to_string(action).unwrap();
        let deserialized: PermissionAction = serde_json::from_str(&json).unwrap();
        assert_eq!(&deserialized, action);
    }
}

// ── PermissionDecision serde ─────────────────────────────────────

#[test]
fn test_permission_decision_serde() {
    for decision in &[PermissionDecision::Once, PermissionDecision::Always, PermissionDecision::Deny] {
        let json = serde_json::to_string(decision).unwrap();
        let deserialized: PermissionDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(&deserialized, decision);
    }
}

// ── Empty ruleset ────────────────────────────────────────────────

#[test]
fn test_permission_empty_ruleset_always_asks() {
    let checker = PermissionChecker::new(vec![]);
    assert_eq!(checker.check("file:read", "/any"), PermissionAction::Ask);
    assert_eq!(checker.check("bash:execute", "ls"), PermissionAction::Ask);
    assert_eq!(checker.check("anything", "whatever"), PermissionAction::Ask);
}
