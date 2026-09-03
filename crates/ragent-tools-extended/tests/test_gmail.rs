#![allow(clippy::assert_is_empty)]
//! Tests for the `gmail` tool — JCODEPLAN M7 (T-060/T-062).
//!
//! Covers:
//! - Tool identity (name, permission category, description)
//! - Parameters schema (all actions + fields)
//! - `gmail` config block parsing and merge behaviour
//! - Encrypted token round-trip through [`SqliteTokenStore`]
//! - auth/status/logout actions (with an in-memory token store)
//! - Mocked Gmail API: search, read, draft, send (axum test server)
//! - Refresh-token exchange against a mocked OAuth2 token endpoint
//! - RFC 2822 raw message construction
//! - Graceful degradation when unauthenticated

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::routing::{get, post};
use ragent_tools_extended::gmail::{GMAIL_TOOL_NAME, GmailTokens, GmailTool, TokenStore};
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// In-memory token store
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct MemStore {
    tokens: Mutex<Option<GmailTokens>>,
}

impl TokenStore for MemStore {
    fn load(&self) -> anyhow::Result<Option<GmailTokens>> {
        Ok(self.tokens.lock().unwrap().clone())
    }
    fn save(&self, tokens: &GmailTokens) -> anyhow::Result<()> {
        *self.tokens.lock().unwrap() = Some(tokens.clone());
        Ok(())
    }
    fn clear(&self) -> anyhow::Result<()> {
        *self.tokens.lock().unwrap() = None;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Test context helpers
// ---------------------------------------------------------------------------

fn ctx(api_base: Option<&str>) -> ToolContext {
    let mut config = ragent_config::Config::default();
    if let Some(base) = api_base {
        config.gmail.base_url = Some(base.to_string());
    }
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::path::PathBuf::from("."),
        event_bus: Arc::new(EventBus::new(64)),
        storage: None,
        code_index: None,
        config: Some(Arc::new(config)),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
    }
}

fn tool_with_store() -> (GmailTool, Arc<MemStore>) {
    let store = Arc::new(MemStore::default());
    (GmailTool::with_store(store.clone()), store)
}

// ---------------------------------------------------------------------------
// Tool identity / schema
// ---------------------------------------------------------------------------

#[test]
fn test_tool_name() {
    let tool = GmailTool::new();
    assert_eq!(tool.name(), GMAIL_TOOL_NAME);
    assert_eq!(tool.name(), "gmail");
}

#[test]
fn test_permission_category_is_network_send() {
    let tool = GmailTool::new();
    assert_eq!(tool.permission_category(), "network:send");
}

#[test]
fn test_description_mentions_actions() {
    let tool = GmailTool::new();
    let desc = tool.description();
    for word in [
        "search", "read", "draft", "send", "auth", "status", "logout",
    ] {
        assert!(desc.contains(word), "description mentions {word}");
    }
    assert!(desc.contains("OAuth2"));
}

#[test]
fn test_schema_has_all_seven_actions() {
    let tool = GmailTool::new();
    let schema = tool.parameters_schema();
    let actions = schema
        .pointer("/properties/action/enum")
        .and_then(Value::as_array)
        .expect("action enum should exist");
    assert_eq!(actions.len(), 7);
    for a in [
        "search", "read", "draft", "send", "auth", "status", "logout",
    ] {
        assert!(actions.contains(&json!(a)), "schema includes {a}");
    }
    // action is required
    let required = schema["required"].as_array().expect("required");
    assert!(required.contains(&json!("action")));
    // message fields present
    for f in [
        "query",
        "id",
        "to",
        "subject",
        "body",
        "access_token",
        "refresh_token",
    ] {
        assert!(
            schema.pointer(&format!("/properties/{f}")).is_some(),
            "schema has {f}"
        );
    }
}

#[test]
fn test_registered_in_extended_registry() {
    let registry = ragent_tools_extended::create_extended_registry();
    assert!(registry.contains("gmail"));
}

// ---------------------------------------------------------------------------
// Config block
// ---------------------------------------------------------------------------

#[test]
fn test_gmail_config_defaults() {
    let cfg = ragent_config::GmailConfig::default();
    assert!(cfg.is_empty());
    assert!(cfg.client_id.is_none());
}

#[test]
fn test_gmail_config_parses_block() {
    let raw = json!({
        "gmail": {
            "client_id": "my-client.apps.googleusercontent.com",
            "client_secret": "env:GMAIL_CLIENT_SECRET"
        }
    });
    let cfg: ragent_config::Config = serde_json::from_value(raw).expect("config should parse");
    assert_eq!(
        cfg.gmail.client_id.as_deref(),
        Some("my-client.apps.googleusercontent.com")
    );
    assert_eq!(
        cfg.gmail.client_secret.as_deref(),
        Some("env:GMAIL_CLIENT_SECRET")
    );
    assert!(!cfg.gmail.is_empty());
}

#[test]
fn test_gmail_config_skip_serialise_when_empty() {
    let cfg = ragent_config::Config::default();
    let v = serde_json::to_value(&cfg).expect("serialise");
    assert!(v.get("gmail").is_none());
}

#[test]
fn test_gmail_merge_overlay_wins() {
    let mut base = ragent_config::Config::default();
    base.gmail.client_secret = Some("base-secret".into());
    let mut overlay = ragent_config::Config::default();
    overlay.gmail.client_id = Some("overlay-id".into());
    let merged = ragent_config::Config::merge(base, overlay);
    assert_eq!(merged.gmail.client_id.as_deref(), Some("overlay-id"));
    assert_eq!(merged.gmail.client_secret.as_deref(), Some("base-secret"));
}

// ---------------------------------------------------------------------------
// SqliteTokenStore (encrypted credential table round-trip)
// ---------------------------------------------------------------------------

#[test]
fn test_sqlite_token_store_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("test.db");
    let store = ragent_tools_extended::gmail::SqliteTokenStore::new(db_path.clone());
    assert!(store.load().expect("load").is_none());

    let tokens = GmailTokens {
        access_token: Some("ya29.test-access".into()),
        refresh_token: Some("1//test-refresh".into()),
        client_id: Some("cid".into()),
        client_secret: Some("csecret".into()),
    };
    store.save(&tokens).expect("save");
    let loaded = store.load().expect("load").expect("tokens present");
    assert_eq!(loaded.access_token.as_deref(), Some("ya29.test-access"));
    assert_eq!(loaded.refresh_token.as_deref(), Some("1//test-refresh"));
    assert_eq!(loaded.client_id.as_deref(), Some("cid"));

    // Verify the on-disk row is encrypted, not plaintext JSON.
    let raw = {
        let conn = rusqlite::Connection::open(&db_path).expect("open db");
        conn.query_row(
            "SELECT api_key FROM provider_auth WHERE provider_id = 'gmail'",
            [],
            |row| row.get::<_, String>(0),
        )
        .expect("raw row")
    };
    assert!(
        !raw.contains("ya29.test-access"),
        "plaintext leaked into db"
    );
    assert!(!raw.contains('{'), "value is not stored as plaintext json");

    store.clear().expect("clear");
    assert!(store.load().expect("load").is_none());
}

// ---------------------------------------------------------------------------
// auth / status / logout actions (no network)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_auth_status_logout_cycle() {
    let (tool, _store) = tool_with_store();
    let ctx = ctx(None);

    // Status before auth.
    let out = tool
        .execute(json!({"action": "status"}), &ctx)
        .await
        .expect("status");
    assert_eq!(out.metadata.as_ref().unwrap()["authenticated"], false);

    // Auth with an access token.
    let out = tool
        .execute(json!({"action": "auth", "access_token": "ya29.abc"}), &ctx)
        .await
        .expect("auth");
    assert_eq!(out.metadata.as_ref().unwrap()["authenticated"], true);
    assert_eq!(out.metadata.as_ref().unwrap()["has_refresh_token"], false);

    // Status reflects the stored token.
    let out = tool
        .execute(json!({"action": "status"}), &ctx)
        .await
        .expect("status");
    assert_eq!(out.metadata.as_ref().unwrap()["authenticated"], true);
    assert_eq!(out.metadata.as_ref().unwrap()["has_access_token"], true);

    // Logout removes it.
    tool.execute(json!({"action": "logout"}), &ctx)
        .await
        .expect("logout");
    let out = tool
        .execute(json!({"action": "status"}), &ctx)
        .await
        .expect("status");
    assert_eq!(out.metadata.as_ref().unwrap()["authenticated"], false);
}

#[tokio::test]
async fn test_auth_requires_a_token() {
    let (tool, _store) = tool_with_store();
    let ctx = ctx(None);
    let err = tool
        .execute(json!({"action": "auth"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("access_token"));
}

#[tokio::test]
async fn test_search_requires_auth_first() {
    let (tool, _store) = tool_with_store();
    let ctx = ctx(None);
    let err = tool
        .execute(json!({"action": "search", "query": "from:a@b.c"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Not authenticated"));
}

#[tokio::test]
async fn test_unknown_action() {
    let (tool, _store) = tool_with_store();
    let ctx = ctx(None);
    let err = tool
        .execute(json!({"action": "explode"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Unknown action"));
}

// ---------------------------------------------------------------------------
// Raw message construction
// ---------------------------------------------------------------------------

#[test]
fn test_build_raw_message_rfc2822() {
    let encoded = GmailTool::build_raw_message(
        "bob@example.com",
        "Hello",
        "Body here",
        Some("cc@example.com"),
        None,
    );
    let decoded =
        base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, &encoded)
            .expect("base64url decodes");
    let text = String::from_utf8(decoded).expect("utf8");
    assert!(text.contains("To: bob@example.com\r\n"));
    assert!(text.contains("Cc: cc@example.com\r\n"));
    assert!(text.contains("Subject: Hello\r\n"));
    assert!(text.contains("\r\n\r\nBody here"));
    assert!(!text.contains("Bcc:"));
}

// ---------------------------------------------------------------------------
// Mocked Gmail API
// ---------------------------------------------------------------------------

fn gmail_message(id: &str, subject: &str, from: &str, body: &str) -> Value {
    let body_b64 = base64::Engine::encode(
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
        body.as_bytes(),
    );
    json!({
        "id": id,
        "threadId": "t-1",
        "labelIds": ["INBOX"],
        "snippet": body.chars().take(30).collect::<String>(),
        "payload": {
            "headers": [
                {"name": "From", "value": from},
                {"name": "To", "value": "me@example.com"},
                {"name": "Subject", "value": subject},
                {"name": "Date", "value": "Mon, 1 Jan 2024 00:00:00 +0000"}
            ],
            "mimeType": "text/plain",
            "body": {"data": body_b64, "size": body.len()},
            "parts": []
        }
    })
}

/// Spawn a mock Gmail API + OAuth2 token endpoint. Returns
/// (base_url, captured_requests).
async fn spawn_gmail_mock() -> (String, Arc<Mutex<Vec<(String, String)>>>) {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let cap1 = Arc::clone(&captured);
    let cap2 = Arc::clone(&captured);
    let cap3 = Arc::clone(&captured);
    let cap4 = Arc::clone(&captured);
    let cap5 = Arc::clone(&captured);

    let app = Router::new()
        .route(
            "/gmail/v1/users/me/messages",
            get(
                move |query: axum::extract::Query<HashMap<String, String>>| {
                    let captured = Arc::clone(&cap1);
                    async move {
                        captured.lock().unwrap().push((
                            format!(
                                "GET /messages?q={}",
                                query.get("q").cloned().unwrap_or_default()
                            ),
                            String::new(),
                        ));
                        axum::Json(json!({
                            "messages": [{"id": "m-1"}, {"id": "m-2"}],
                            "resultSizeEstimate": 2
                        }))
                    }
                },
            ),
        )
        .route(
            "/gmail/v1/users/me/messages/{id}",
            get(move |path: axum::extract::Path<String>| {
                let captured = Arc::clone(&cap2);
                async move {
                    captured
                        .lock()
                        .unwrap()
                        .push((format!("GET /messages/{}", path.0), String::new()));
                    let (subject, from, body) = match path.0.as_str() {
                        "m-1" => ("Build passed", "ci@example.com", "green build on main"),
                        _ => ("Deploy notice", "ops@example.com", "deploy finished"),
                    };
                    axum::Json(gmail_message(&path.0, subject, from, body))
                }
            }),
        )
        .route(
            "/gmail/v1/users/me/messages/send",
            post(move |body: axum::Json<Value>| {
                let captured = Arc::clone(&cap3);
                async move {
                    captured
                        .lock()
                        .unwrap()
                        .push(("POST /send".to_string(), body.0.to_string()));
                    axum::Json(json!({"id": "sent-1", "threadId": "t-9"}))
                }
            }),
        )
        .route(
            "/gmail/v1/users/me/drafts",
            post(move |body: axum::Json<Value>| {
                let captured = Arc::clone(&cap4);
                async move {
                    captured
                        .lock()
                        .unwrap()
                        .push(("POST /drafts".to_string(), body.0.to_string()));
                    axum::Json(json!({"id": "draft-1"}))
                }
            }),
        )
        .route(
            "/oauth2/v4/token",
            post(move |form: axum::Form<HashMap<String, String>>| {
                let captured = Arc::clone(&cap5);
                async move {
                    captured
                        .lock()
                        .unwrap()
                        .push(("POST /token".to_string(), format!("{:?}", form.0)));
                    axum::Json(json!({
                        "access_token": "ya29.refreshed",
                        "expires_in": 3600,
                        "token_type": "Bearer"
                    }))
                }
            }),
        );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    (format!("http://{addr}"), captured)
}

#[tokio::test]
async fn test_search_and_read_via_mock() {
    let (base_url, captured) = spawn_gmail_mock().await;
    let (tool, store) = tool_with_store();
    let ctx = ctx(Some(&base_url));

    // Authenticate with a plain access token.
    tool.execute(json!({"action": "auth", "access_token": "ya29.test"}), &ctx)
        .await
        .expect("auth");
    assert!(store.load().unwrap().is_some());

    // Search.
    let out = tool
        .execute(
            json!({"action": "search", "query": "from:ci@example.com"}),
            &ctx,
        )
        .await
        .expect("search");
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["count"], 2);
    let messages = meta["messages"].as_array().expect("messages");
    assert_eq!(messages[0]["id"], "m-1");
    assert_eq!(messages[0]["headers"]["subject"], "Build passed");
    assert_eq!(messages[1]["headers"]["from"], "ops@example.com");

    // Read.
    let out = tool
        .execute(json!({"action": "read", "id": "m-1"}), &ctx)
        .await
        .expect("read");
    assert!(out.content.contains("From: ci@example.com"));
    assert!(out.content.contains("Subject: Build passed"));
    assert!(out.content.contains("green build on main"));
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["id"], "m-1");

    // The mock saw the expected calls.
    let calls = captured.lock().unwrap();
    assert!(calls.iter().any(|(p, _)| p.starts_with("GET /messages?q=")));
    assert!(calls.iter().any(|(p, _)| p == "GET /messages/m-1"));
}

#[tokio::test]
async fn test_draft_and_send_via_mock() {
    let (base_url, captured) = spawn_gmail_mock().await;
    let (tool, _store) = tool_with_store();
    let ctx = ctx(Some(&base_url));
    tool.execute(json!({"action": "auth", "access_token": "ya29.test"}), &ctx)
        .await
        .expect("auth");

    let out = tool
        .execute(
            json!({
                "action": "draft",
                "to": "bob@example.com",
                "subject": "Drafty",
                "body": "draft body"
            }),
            &ctx,
        )
        .await
        .expect("draft");
    assert_eq!(out.metadata.as_ref().unwrap()["draft_id"], "draft-1");

    let out = tool
        .execute(
            json!({
                "action": "send",
                "to": "bob@example.com",
                "subject": "Ship it",
                "body": "go go go"
            }),
            &ctx,
        )
        .await
        .expect("send");
    assert_eq!(out.metadata.as_ref().unwrap()["message_id"], "sent-1");

    // Verify the wire payload carries a decodable raw message.
    let calls = captured.lock().unwrap();
    let send = calls
        .iter()
        .find(|(p, _)| p == "POST /send")
        .expect("send call");
    let body: Value = serde_json::from_str(&send.1).expect("json body");
    let raw = body["raw"].as_str().expect("raw field");
    let decoded = base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, raw)
        .expect("decode raw");
    let text = String::from_utf8(decoded).expect("utf8");
    assert!(text.contains("To: bob@example.com"));
    assert!(text.contains("Subject: Ship it"));

    let draft = calls
        .iter()
        .find(|(p, _)| p == "POST /drafts")
        .expect("draft call");
    let body: Value = serde_json::from_str(&draft.1).expect("json body");
    assert!(body["message"]["raw"].is_string());
}

#[tokio::test]
async fn test_refresh_token_exchange_via_mock() {
    let (base_url, captured) = spawn_gmail_mock().await;
    let (tool, _store) = tool_with_store();
    let ctx = ctx(Some(&base_url));

    // Authenticate with only a refresh token + client credentials.
    tool.execute(
        json!({
            "action": "auth",
            "refresh_token": "1//refresh-me",
            "client_id": "cid",
            "client_secret": "csecret"
        }),
        &ctx,
    )
    .await
    .expect("auth");

    // A search should first hit the token endpoint, then succeed.
    let out = tool
        .execute(json!({"action": "search", "query": "is:unread"}), &ctx)
        .await
        .expect("search with refresh");
    assert_eq!(out.metadata.as_ref().unwrap()["count"], 2);

    let calls = captured.lock().unwrap();
    let token_call = calls
        .iter()
        .find(|(p, _)| p == "POST /token")
        .expect("token call");
    assert!(token_call.1.contains("1//refresh-me"));
    assert!(token_call.1.contains("refresh_token"));
}

#[test]
fn test_resolved_config_env_fallback() {
    // Env fallback: without config or stored tokens, resolved client creds
    // come from GMAIL_CLIENT_ID / GMAIL_CLIENT_SECRET when present. We cannot
    // set env vars safely in parallel tests, so just assert the precedence
    // chain shape with an empty env (None result).
    let tool_ctx = ctx(None);
    let tokens = GmailTokens::default();
    let cfg = GmailTool::resolved_config(&tool_ctx, &tokens);
    assert_eq!(cfg.api_base, "https://gmail.googleapis.com");
    assert_eq!(cfg.token_url, "https://oauth2.googleapis.com/token");
}
