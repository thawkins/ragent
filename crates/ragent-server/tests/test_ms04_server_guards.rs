//! MS-04 (SECTASKS T-064) server guards: generic internal errors and a
//! credential-free event rendering for logs.

use ragent_agent::event::Event;
use ragent_server::routes::internal_error_response;
use ragent_server::sse::redacted_event_debug;

#[test]
fn internal_error_response_hides_the_detail() {
    let (status, body) = internal_error_response(
        "get_session",
        "unable to open database file: /home/victim/.local/share/ragent/ragent.db",
    );
    assert_eq!(status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    let text = body.0.to_string();
    assert_eq!(text, r#"{"error":"internal server error"}"#);
    assert!(
        !text.contains("ragent.db"),
        "path must not be echoed: {text}"
    );
}

#[test]
fn redacted_event_debug_masks_device_code_and_token() {
    let start = Event::CopilotDeviceFlowStartResult {
        user_code: Some("ABCD-1234".to_string()),
        verification_uri: Some("https://github.com/login/device".to_string()),
        device_code: Some("super-secret-device-code".to_string()),
        interval: Some(5),
        error: None,
    };
    let rendered = redacted_event_debug(&start);
    assert!(!rendered.contains("super-secret-device-code"), "{rendered}");
    assert!(rendered.contains("device_code_present: true"), "{rendered}");

    let complete = Event::CopilotDeviceFlowComplete {
        token: "gho_supersecrettoken".to_string(),
        api_base: "https://api.githubcopilot.com".to_string(),
    };
    let rendered = redacted_event_debug(&complete);
    assert!(!rendered.contains("gho_supersecrettoken"), "{rendered}");
    assert!(rendered.contains("token_present: true"), "{rendered}");
}
