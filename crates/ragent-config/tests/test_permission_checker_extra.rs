//! Extra coverage for `ragent_config::permission`: the `Permission` display
//! round-trip, the `PermissionRule`/`PermissionRequest` shapes, the
//! `PermissionChecker` wildcard and `record_always` paths, and the SEC-023
//! fail-closed handling of a malformed deny glob.

use ragent_config::permission::{Permission, PermissionAction, PermissionChecker, PermissionRule};

fn rule(permission: Permission, pattern: &str, action: PermissionAction) -> PermissionRule {
    PermissionRule {
        permission,
        pattern: Some(pattern.to_string()),
        action,
    }
}

#[test]
fn test_permission_display_round_trips_through_from() {
    let cases = [
        (Permission::Read, "read"),
        (Permission::Edit, "edit"),
        (Permission::Bash, "bash"),
        (Permission::Web, "web"),
        (Permission::Question, "question"),
        (Permission::PlanEnter, "plan_enter"),
        (Permission::PlanExit, "plan_exit"),
        (Permission::Task, "task"),
        (Permission::ExternalDirectory, "external_directory"),
        (Permission::DoomLoop, "doom_loop"),
    ];
    for (variant, label) in cases {
        assert_eq!(variant.to_string(), label);
        assert_eq!(Permission::from(label), variant);
    }
    // A custom permission displays its literal name and round-trips.
    let custom = Permission::Custom("deploy:prod".to_string());
    assert_eq!(custom.to_string(), "deploy:prod");
    assert_eq!(Permission::from("deploy:prod"), custom);
}

#[test]
fn test_permission_serde_round_trip() {
    for variant in [
        Permission::Read,
        Permission::Web,
        Permission::PlanExit,
        Permission::Custom("widget".to_string()),
    ] {
        let json = serde_json::to_string(&variant).expect("serialise");
        let back: Permission = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back, variant, "round trip for {variant:?}");
    }
}

#[test]
fn test_permission_action_display_and_serde() {
    assert_eq!(PermissionAction::Allow.to_string(), "allow");
    assert_eq!(PermissionAction::Deny.to_string(), "deny");
    assert_eq!(PermissionAction::Ask.to_string(), "ask");
    for (action, wire) in [
        (PermissionAction::Allow, "\"allow\""),
        (PermissionAction::Deny, "\"deny\""),
        (PermissionAction::Ask, "\"ask\""),
    ] {
        assert_eq!(serde_json::to_string(&action).expect("serialise"), wire);
        let back: PermissionAction = serde_json::from_str(wire).expect("deserialise");
        assert_eq!(back, action);
    }
}

#[test]
fn test_permission_request_serde_shape() {
    let request: ragent_config::PermissionRequest = serde_json::from_str(
        r#"{
            "id": "req-1",
            "session_id": "sess-1",
            "permission": "bash",
            "patterns": ["rm -rf /"],
            "metadata": { "tool": "bash" },
            "tool_call_id": "call-7"
        }"#,
    )
    .expect("parse request");
    assert_eq!(request.permission, "bash");
    assert_eq!(request.patterns, vec!["rm -rf /".to_string()]);
    assert_eq!(request.tool_call_id.as_deref(), Some("call-7"));
}

#[test]
fn test_permission_rule_pattern_is_optional_and_serialises_without_it() {
    let rule = PermissionRule {
        permission: Permission::Read,
        pattern: None,
        action: PermissionAction::Ask,
    };
    let json = serde_json::to_string(&rule).expect("serialise");
    assert!(
        !json.contains("pattern"),
        "an absent pattern is omitted: {json}"
    );
    let back: PermissionRule = serde_json::from_str(&json).expect("deserialise");
    assert!(back.pattern.is_none());
    assert_eq!(back.action, PermissionAction::Ask);
}

#[test]
fn test_checker_last_match_wins() {
    let checker = PermissionChecker::new(vec![
        rule(Permission::Read, "**", PermissionAction::Deny),
        rule(Permission::Read, "src/**", PermissionAction::Allow),
    ]);
    assert_eq!(
        checker.check("read", "src/main.rs"),
        PermissionAction::Allow
    );
    assert_eq!(checker.check("read", "etc/passwd"), PermissionAction::Deny);
    // No matching rule for an unrelated path -> Ask.
    let ask = PermissionChecker::new(vec![rule(
        Permission::Read,
        "src/**",
        PermissionAction::Allow,
    )]);
    assert_eq!(ask.check("read", "docs/readme.md"), PermissionAction::Ask);
}

#[test]
fn test_checker_patternless_rule_is_skipped() {
    // A rule with no pattern is dropped during construction and never matches.
    let checker = PermissionChecker::new(vec![PermissionRule {
        permission: Permission::Read,
        pattern: None,
        action: PermissionAction::Allow,
    }]);
    assert_eq!(checker.check("read", "anything"), PermissionAction::Ask);
}

#[test]
fn test_checker_wildcard_rule_applies_to_every_permission() {
    let checker = PermissionChecker::new(vec![rule(
        Permission::Custom("*".to_string()),
        "**/secrets/**",
        PermissionAction::Deny,
    )]);
    assert_eq!(
        checker.check("read", "a/secrets/key"),
        PermissionAction::Deny
    );
    assert_eq!(
        checker.check("bash", "a/secrets/key"),
        PermissionAction::Deny
    );
    assert_eq!(checker.check("read", "src/main.rs"), PermissionAction::Ask);
}

#[test]
fn test_checker_record_always_takes_precedence_over_deny() {
    let mut checker =
        PermissionChecker::new(vec![rule(Permission::Edit, "**", PermissionAction::Ask)]);
    assert_eq!(checker.check("edit", "src/lib.rs"), PermissionAction::Ask);

    // A recorded permanent grant short-circuits the ruleset for a match.
    checker.record_always("edit", "src/**");
    assert_eq!(checker.check("edit", "src/lib.rs"), PermissionAction::Allow);
    assert_eq!(
        checker.check("edit", "vendor/lib.rs"),
        PermissionAction::Ask
    );

    // A malformed pattern passed to `record_always` is simply ignored.
    checker.record_always("edit", "[unterminated");
    assert_eq!(
        checker.check("edit", "vendor/lib.rs"),
        PermissionAction::Ask
    );

    // Namespaced forms resolve to the same grant bucket.
    checker.record_always("file:write", "gen/**");
    assert_eq!(checker.check("edit", "gen/x.rs"), PermissionAction::Allow);
}

#[test]
fn test_checker_malformed_deny_rule_fails_closed() {
    // SEC-023: an invalid deny glob must not be dropped; it becomes a catch-all
    // deny for its permission so the protection is never lost to a typo.
    let checker = PermissionChecker::new(vec![rule(
        Permission::Bash,
        "[unterminated",
        PermissionAction::Deny,
    )]);
    assert_eq!(checker.check("bash", "ls"), PermissionAction::Deny);
    assert_eq!(checker.check("bash", "/etc/passwd"), PermissionAction::Deny);
    // Another permission is unaffected.
    assert_eq!(checker.check("read", "src/main.rs"), PermissionAction::Ask);
}

#[test]
fn test_checker_malformed_deny_wildcard_fails_closed() {
    let checker = PermissionChecker::new(vec![rule(
        Permission::Custom("*".to_string()),
        "[unterminated",
        PermissionAction::Deny,
    )]);
    assert_eq!(checker.check("read", "anything"), PermissionAction::Deny);
    assert_eq!(checker.check("bash", "ls"), PermissionAction::Deny);
}

#[test]
fn test_checker_malformed_allow_rule_is_only_logged() {
    // A malformed allow rule is dropped, leaving the default Ask.
    let checker = PermissionChecker::new(vec![rule(
        Permission::Read,
        "[unterminated",
        PermissionAction::Allow,
    )]);
    assert_eq!(checker.check("read", "src/main.rs"), PermissionAction::Ask);
}

#[test]
fn test_checker_custom_exact_and_normalized_buckets_both_apply() {
    // A rule keyed on the literal custom string matches when the lookup uses
    // that exact permission, while a rule keyed on the normalized variant also
    // matches a namespaced lookup.
    let checker = PermissionChecker::new(vec![
        rule(
            Permission::Custom("file:read".to_string()),
            "**",
            PermissionAction::Allow,
        ),
        rule(Permission::Read, "src/**", PermissionAction::Allow),
    ]);
    assert_eq!(
        checker.check("file:read", "docs/x.md"),
        PermissionAction::Allow
    );
    assert_eq!(
        checker.check("file:read", "src/x.rs"),
        PermissionAction::Allow
    );
    // An unnamespaced `read` still hits the normalized `Read` bucket.
    assert_eq!(checker.check("read", "src/x.rs"), PermissionAction::Allow);
    assert_eq!(checker.check("read", "docs/x.md"), PermissionAction::Ask);
}
