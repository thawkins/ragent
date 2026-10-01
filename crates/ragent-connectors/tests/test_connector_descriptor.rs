//! Tests for connector descriptor validation and collision predicates
//! (spec `connectors` T-002; FR-002, FR-025).

use ragent_config::{McpTransport, trigger::McpNotificationMode};
use ragent_connectors::{
    ConnectorAuthShape, ConnectorDescriptor, ConnectorError, ConnectorId, ConnectorProvenance,
    ConnectorServer, UNSUP_COMMAND, UNSUP_SERVER_ID, UNSUP_TRANSPORT, UNSUP_URL, bridged_server_id,
    connector_id_collides, server_id_collides,
};

/// A minimal stdio server with a real command (expressible).
fn stdio_server(id: &str) -> ConnectorServer {
    ConnectorServer {
        id: id.to_string(),
        transport: "stdio".to_string(),
        command: Some("/usr/bin/mcp-echo".to_string()),
        args: Vec::new(),
        env: std::collections::BTreeMap::new(),
        url: None,
        headers: std::collections::BTreeMap::new(),
    }
}

/// A descriptor with one expressible stdio server and the given id.
fn descriptor_with_id(id: &str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("test id should be valid"),
        name: "Echo".to_string(),
        description: "echoes input".to_string(),
        category: "developer".to_string(),
        tags: vec!["test".to_string()],
        source: "catalogue:test".to_string(),
        provenance: ConnectorProvenance::Catalogue,
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![stdio_server("echo")],
        unsupported: Vec::new(),
    }
}

// --- ConnectorId -----------------------------------------------------------

#[test]
fn test_connector_id_accepts_valid_component() {
    let id = ConnectorId::new("google-drive").expect("valid id");
    assert_eq!(id.as_str(), "google-drive");
}

#[test]
fn test_connector_id_rejects_empty() {
    let err = ConnectorId::new("   ").expect_err("blank id must fail");
    assert!(matches!(err, ConnectorError::InvalidId { .. }));
}

#[test]
fn test_connector_id_rejects_path_separator() {
    assert!(ConnectorId::new("a/b").is_err());
    assert!(ConnectorId::new("a\\b").is_err());
}

#[test]
fn test_connector_id_rejects_parent_component() {
    assert!(ConnectorId::new("..").is_err());
    assert!(ConnectorId::new("../escape").is_err());
    assert!(ConnectorId::new("a..b").is_err());
}

#[test]
fn test_connector_id_rejects_absolute_and_whitespace() {
    assert!(ConnectorId::new("/etc/passwd").is_err());
    assert!(ConnectorId::new("has space").is_err());
}

#[test]
fn test_connector_id_rejects_overlong() {
    let long = "a".repeat(65);
    assert!(ConnectorId::new(&long).is_err());
    let max = "a".repeat(64);
    assert!(ConnectorId::new(&max).is_ok());
}

// --- Auth shape / provenance labels ---------------------------------------

#[test]
fn test_auth_shape_labels_and_credential_requirement() {
    assert_eq!(ConnectorAuthShape::None.label(), "none");
    assert!(!ConnectorAuthShape::None.requires_credential());
    for shape in [
        ConnectorAuthShape::Env,
        ConnectorAuthShape::Token,
        ConnectorAuthShape::Oauth,
        ConnectorAuthShape::Password,
    ] {
        assert!(shape.requires_credential(), "{shape:?} needs a credential");
    }
    assert_eq!(ConnectorAuthShape::Token.to_string(), "token");
}

#[test]
fn test_provenance_labels() {
    assert_eq!(ConnectorProvenance::Catalogue.label(), "catalogue");
    assert_eq!(ConnectorProvenance::Local.label(), "local");
    assert_eq!(ConnectorProvenance::Url.label(), "url");
    assert_eq!(ConnectorProvenance::Url.to_string(), "url");
}

// --- ConnectorServer supported shape --------------------------------------

#[test]
fn test_stdio_server_needs_a_command() {
    let mut server = stdio_server("echo");
    assert!(server.is_supported());
    assert_eq!(server.unsupported_label(), None);

    server.command = None;
    assert!(!server.is_supported());
    assert_eq!(server.unsupported_label(), Some(UNSUP_COMMAND));

    server.command = Some("   ".to_string());
    assert_eq!(server.unsupported_label(), Some(UNSUP_COMMAND));
}

#[test]
fn test_unknown_transport_is_unsupported() {
    let server = ConnectorServer {
        transport: "grpc".to_string(),
        ..stdio_server("echo")
    };
    assert!(!server.is_supported());
    assert_eq!(server.unsupported_label(), Some(UNSUP_TRANSPORT));
    assert_eq!(server.transport_kind(), None);
}

#[test]
fn test_http_and_sse_servers_need_a_url() {
    for transport in ["http", "sse"] {
        let mut server = ConnectorServer {
            transport: transport.to_string(),
            command: None,
            ..stdio_server("web")
        };
        assert_eq!(server.unsupported_label(), Some(UNSUP_URL));

        server.url = Some("https://example.org/mcp".to_string());
        assert!(server.is_supported());
        assert_eq!(server.unsupported_label(), None);
    }
}

#[test]
fn test_blank_server_id_is_unsupported() {
    let server = ConnectorServer {
        id: String::new(),
        ..stdio_server("echo")
    };
    assert_eq!(server.unsupported_label(), Some(UNSUP_SERVER_ID));
}

#[test]
fn test_server_converts_to_mcp_config() {
    let server = stdio_server("echo");
    let config = server.to_mcp_config();
    assert_eq!(config.type_, McpTransport::Stdio);
    assert_eq!(config.command.as_deref(), Some("/usr/bin/mcp-echo"));
    assert_eq!(config.notification, McpNotificationMode::None);
}

// --- Descriptor validation -------------------------------------------------

#[test]
fn test_descriptor_with_no_servers_fails() {
    let mut descriptor = descriptor_with_id("echo");
    descriptor.servers.clear();
    let err = descriptor.validate().expect_err("no servers must fail");
    match err {
        ConnectorError::NoServers { id } => assert_eq!(id, "echo"),
        other => panic!("expected NoServers, got {other:?}"),
    }
}

#[test]
fn test_descriptor_with_unknown_transport_fails_with_reason() {
    let mut descriptor = descriptor_with_id("unsupported");
    descriptor.servers = vec![ConnectorServer {
        transport: "grpc".to_string(),
        ..stdio_server("echo")
    }];

    let err = descriptor
        .validate()
        .expect_err("no supported servers must fail");
    match err {
        ConnectorError::NoSupportedServers { id, reason } => {
            assert_eq!(id, "unsupported");
            assert!(reason.contains(UNSUP_TRANSPORT), "reason was {reason}");
        }
        other => panic!("expected NoSupportedServers, got {other:?}"),
    }
    // FR-025: the label is retained, not dropped.
    assert_eq!(descriptor.unsupported.len(), 1);
    assert!(descriptor.unsupported[0].contains(UNSUP_TRANSPORT));
}

#[test]
fn test_descriptor_with_one_bad_server_keeps_label_and_one_good() {
    let mut descriptor = descriptor_with_id("mixed");
    descriptor.servers = vec![
        stdio_server("echo"),
        ConnectorServer {
            transport: "grpc".to_string(),
            ..stdio_server("bad")
        },
    ];

    descriptor.validate().expect("one good server is enough");
    assert_eq!(descriptor.unsupported.len(), 1);
    assert!(descriptor.unsupported[0].contains("bad"));
    assert!(descriptor.unsupported[0].contains(UNSUP_TRANSPORT));
    assert_eq!(descriptor.supported_servers().len(), 1);
    assert!(descriptor.has_supported_server());
}

#[test]
fn test_valid_descriptor_has_no_unsupported_labels() {
    let mut descriptor = descriptor_with_id("echo");
    descriptor.validate().expect("valid descriptor");
    assert!(descriptor.unsupported.is_empty());
}

// --- Bridged ids and collision predicates ---------------------------------

#[test]
fn test_bridged_server_id_prefixes_connector() {
    assert_eq!(
        bridged_server_id("google-drive", "drive"),
        "google-drive.drive"
    );
    let descriptor = descriptor_with_id("echo");
    assert_eq!(descriptor.bridged_id("echo"), "echo.echo");
}

#[test]
fn test_connector_id_collision() {
    assert!(connector_id_collides("echo", ["echo", "other"]));
    assert!(!connector_id_collides("new", ["echo", "other"]));
    assert!(!connector_id_collides("echo", [] as [&str; 0]));
}

#[test]
fn test_server_id_collision() {
    // A bridged id that clashes with a plain ragent.json mcp key (FR-033).
    assert!(server_id_collides(
        "echo.echo",
        ["filesystem", "echo.echo", "git"]
    ));
    assert!(!server_id_collides("echo.echo", ["filesystem", "git"]));
}

// --- Serde round trip ------------------------------------------------------

#[test]
fn test_descriptor_serde_round_trip() {
    let mut descriptor = descriptor_with_id("echo");
    descriptor.validate().expect("valid");
    let json = serde_json::to_string(&descriptor).expect("serialise");
    let back: ConnectorDescriptor = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(back, descriptor);
}

#[test]
fn test_descriptor_json_uses_lowercase_enums_and_skips_empty() {
    let descriptor = descriptor_with_id("echo");
    let value: serde_json::Value = serde_json::to_value(&descriptor).expect("to_value");
    assert_eq!(value["auth"], "none");
    assert_eq!(value["provenance"], "catalogue");
    assert_eq!(value["id"], "echo");
    // An empty server list would still serialise, but env/headers on the server
    // are omitted when empty.
    let server = &value["servers"][0];
    assert!(server.get("env").is_none());
    assert!(server.get("headers").is_none());
}

#[test]
fn test_descriptor_deserialises_with_defaults() {
    let descriptor: ConnectorDescriptor = serde_json::from_str(
        r#"{
            "id": "echo",
            "name": "Echo",
            "servers": [ { "id": "echo", "command": "/usr/bin/mcp-echo" } ]
        }"#,
    )
    .expect("minimal descriptor should parse");
    assert_eq!(descriptor.id.as_str(), "echo");
    assert_eq!(descriptor.auth, ConnectorAuthShape::None);
    assert_eq!(descriptor.provenance, ConnectorProvenance::Catalogue);
    assert_eq!(descriptor.servers[0].transport, "stdio");
}
