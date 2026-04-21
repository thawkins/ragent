//! Test for permission dialog countdown timer display

use ragent_core::permission::PermissionRequest;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_permission_request_has_timeout_fields() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let request = PermissionRequest {
        id: "test-123".to_string(),
        session_id: "session-456".to_string(),
        permission: "bash".to_string(),
        patterns: vec!["ls".to_string()],
        metadata: serde_json::json!({
            "command": "ls -la",
            "created_at": now,
            "timeout_secs": 120_u64
        }),
        tool_call_id: None,
    };

    assert_eq!(
        request
            .metadata
            .get("timeout_secs")
            .and_then(|v| v.as_u64()),
        Some(120)
    );
    assert_eq!(
        request.metadata.get("created_at").and_then(|v| v.as_u64()),
        Some(now)
    );
}

#[test]
fn test_countdown_calculation_logic() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Simulate a request created 30 seconds ago
    let created_at = now - 30;
    let timeout_secs: u64 = 120;

    let elapsed = now.saturating_sub(created_at);
    let remaining = timeout_secs.saturating_sub(elapsed);

    assert_eq!(elapsed, 30);
    assert_eq!(remaining, 90);

    let remaining_mins = remaining / 60;
    let remaining_secs = remaining % 60;

    assert_eq!(remaining_mins, 1);
    assert_eq!(remaining_secs, 30);

    let title = format!(
        " Permission Required ({}:{:02} remaining) ",
        remaining_mins, remaining_secs
    );
    assert_eq!(title, " Permission Required (1:30 remaining) ");
}

#[test]
fn test_countdown_expired() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Simulate a request created 130 seconds ago (expired)
    let created_at = now - 130;
    let timeout_secs: u64 = 120;

    let elapsed = now.saturating_sub(created_at);
    let remaining = timeout_secs.saturating_sub(elapsed);

    assert_eq!(remaining, 0);

    let title = if remaining == 0 {
        " Permission Required (EXPIRED) ".to_string()
    } else {
        let remaining_mins = remaining / 60;
        let remaining_secs = remaining % 60;
        format!(
            " Permission Required ({}:{:02} remaining) ",
            remaining_mins, remaining_secs
        )
    };

    assert_eq!(title, " Permission Required (EXPIRED) ");
}

#[test]
fn test_countdown_formats_correctly() {
    // Test various time formats
    let test_cases = vec![
        (120, 2, 0, "2:00"), // 2 minutes
        (90, 1, 30, "1:30"), // 1:30
        (60, 1, 0, "1:00"),  // 1 minute
        (59, 0, 59, "0:59"), // 59 seconds
        (5, 0, 5, "0:05"),   // 5 seconds
        (0, 0, 0, "0:00"),   // expired
    ];

    for (remaining_secs, expected_mins, expected_secs, expected_str) in test_cases {
        let mins = remaining_secs / 60;
        let secs = remaining_secs % 60;

        assert_eq!(mins, expected_mins);
        assert_eq!(secs, expected_secs);

        let formatted = format!("{}:{:02}", mins, secs);
        assert_eq!(formatted, expected_str);
    }
}
