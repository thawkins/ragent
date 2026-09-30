//! Inline tests for `cdp.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_version_info_deserialises() {
    let json = r#"{
        "Browser": "Chrome/131.0.6778.85",
        "V8": "13.1.201.7",
        "WebKit": "537.36",
        "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/browser/abc-123",
        "User-Agent": "Mozilla/5.0"
    }"#;
    let info: VersionInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.browser, "Chrome/131.0.6778.85");
    assert_eq!(
        info.web_socket_debugger_url,
        "ws://127.0.0.1:9222/devtools/browser/abc-123"
    );
}

#[test]
fn test_target_info_deserialises() {
    let json = r#"{
        "id": "target-1",
        "type": "page",
        "title": "Example",
        "url": "https://example.com",
        "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/page/target-1",
        "attached": false
    }"#;
    let target: TargetInfo = serde_json::from_str(json).unwrap();
    assert_eq!(target.id, "target-1");
    assert_eq!(target.target_type, "page");
    assert_eq!(target.url, "https://example.com");
}

#[test]
fn test_first_page_target_finds_page() {
    let targets = vec![
        TargetInfo {
            id: "bg".to_string(),
            target_type: "background_page".to_string(),
            title: String::new(),
            url: String::new(),
            web_socket_debugger_url: String::new(),
            attached: false,
        },
        TargetInfo {
            id: "page1".to_string(),
            target_type: "page".to_string(),
            title: "Test".to_string(),
            url: "https://example.com".to_string(),
            web_socket_debugger_url: "ws://127.0.0.1:9222/devtools/page/page1".to_string(),
            attached: false,
        },
    ];
    let result = first_page_target(&targets).unwrap();
    assert_eq!(result.id, "page1");
}

#[test]
fn test_first_page_target_no_page() {
    let targets = vec![TargetInfo {
        id: "bg".to_string(),
        target_type: "background_page".to_string(),
        title: String::new(),
        url: String::new(),
        web_socket_debugger_url: String::new(),
        attached: false,
    }];
    let result = first_page_target(&targets);
    assert!(result.is_err());
}

#[test]
fn test_cdp_error_display() {
    let err = CdpError::CommandError {
        code: -32000,
        message: "Cannot navigate to invalid URL".to_string(),
    };
    assert!(err.to_string().contains("-32000"));
    assert!(err.to_string().contains("Cannot navigate"));
}
