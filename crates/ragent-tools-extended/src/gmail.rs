//! Gmail tool (`gmail`) — JCODEPLAN M7 (T-060).
//!
//! Search, read, draft, and send mail through the Gmail REST API v1. OAuth2
//! tokens are managed via an injectable [`TokenStore`] backend; the default
//! file-backed store keeps them in the `provider_auth` table of the ragent
//! `SQLite` database using the same machine-local encryption as provider
//! credentials — never in `ragent.json`.
//!
//! # Authentication model
//!
//! The tool supports two ways to become authenticated:
//!
//! 1. **Access token directly** — `gmail action="auth" access_token="ya29...."`.
//! 2. **Refresh token flow** — provide a refresh token obtained from the
//!    Google OAuth2 playground (scope `https://mail.google.com/`):
//!    `gmail action="auth" refresh_token="..." client_id="..." client_secret="..."`.
//!    The tool exchanges it for short-lived access tokens automatically when
//!    needed. Client credentials are read, in precedence order: auth-time
//!    arguments → stored credentials → `gmail.client_id` /
//!    `gmail.client_secret` in `ragent.json` → the `GMAIL_CLIENT_ID` /
//!    `GMAIL_CLIENT_SECRET` environment variables.
//!
//! # Actions
//!
//! | Action   | Description                                       |
//! |----------|---------------------------------------------------|
//! | `search` | List messages matching a Gmail search query       |
//! | `read`   | Fetch a single message (headers + decoded body)   |
//! | `draft`  | Create a draft (returns a draft id)               |
//! | `send`   | Send an email immediately                         |
//! | `auth`   | Store OAuth tokens in encrypted storage           |
//! | `status` | Report whether credentials are stored             |
//! | `logout` | Remove stored credentials                         |
//!
//! # Graceful degradation
//!
//! When unauthenticated, or when the credential store is unreachable, actions
//! return honest errors with actionable guidance.

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use ragent_storage::Storage;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use super::{Tool, ToolContext, ToolOutput};

/// Tool name used by the LLM.
pub const GMAIL_TOOL_NAME: &str = "gmail";

const DEFAULT_API_BASE: &str = "https://gmail.googleapis.com";
const DEFAULT_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_SEARCH_RESULTS: u64 = 100;
const DEFAULT_SEARCH_RESULTS: u64 = 10;
const MAX_BODY_SNIPPET: usize = 4000;

// ---------------------------------------------------------------------------
// Token store
// ---------------------------------------------------------------------------

/// Stored Gmail credentials held in encrypted storage.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GmailTokens {
    /// Short-lived OAuth2 access token for Gmail API calls.
    pub access_token: Option<String>,
    /// Long-lived OAuth2 refresh token used to obtain new access tokens.
    pub refresh_token: Option<String>,
    /// OAuth2 client id persisted at auth time (used for refresh).
    pub client_id: Option<String>,
    /// OAuth2 client secret persisted at auth time (used for refresh).
    pub client_secret: Option<String>,
}

/// Backend for persisting Gmail credentials.
///
/// The default implementation encrypts a JSON blob with the same v2
/// machine-local scheme used for provider credentials inside
/// [`Storage::set_provider_auth`]. Tests can substitute an in-memory store.
pub trait TokenStore: Send + Sync {
    /// Load the stored Gmail tokens (or `None` when absent).
    fn load(&self) -> Result<Option<GmailTokens>>;
    /// Persist Gmail tokens.
    fn save(&self, tokens: &GmailTokens) -> Result<()>;
    /// Remove Gmail tokens.
    fn clear(&self) -> Result<()>;
}

/// File-backed token store using the ragent `SQLite` credential table.
#[derive(Debug, Clone)]
pub struct SqliteTokenStore {
    db_path: PathBuf,
}

impl SqliteTokenStore {
    /// Create a store over the database at `db_path`.
    #[must_use]
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    /// Create a store over the shared ragent database
    /// (`<data_dir>/ragent/ragent.db`).
    pub fn shared() -> Self {
        let db_path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ragent")
            .join("ragent.db");
        Self::new(db_path)
    }

    fn open(&self) -> Result<Storage> {
        Storage::open(&self.db_path).with_context(|| {
            format!(
                "Failed to open credential store at {}",
                self.db_path.display()
            )
        })
    }
}

impl TokenStore for SqliteTokenStore {
    fn load(&self) -> Result<Option<GmailTokens>> {
        let raw: Option<String> = self.open()?.get_provider_auth("gmail")?;
        Ok(raw.and_then(|s| serde_json::from_str::<GmailTokens>(&s).ok()))
    }

    fn save(&self, tokens: &GmailTokens) -> Result<()> {
        let json = serde_json::to_string(tokens).context("Failed to serialise tokens")?;
        self.open()?.set_provider_auth("gmail", &json)
    }

    fn clear(&self) -> Result<()> {
        self.open()?.delete_provider_auth("gmail")
    }
}

// ---------------------------------------------------------------------------
// Config resolution
// ---------------------------------------------------------------------------

/// Resolved Gmail configuration (endpoint overrides + OAuth client credentials).
#[derive(Debug, Clone, Default)]
pub struct GmailResolvedConfig {
    /// HTTP base URL for the Gmail API.
    pub api_base: String,
    /// OAuth2 token endpoint.
    pub token_url: String,
    /// OAuth2 client id for the refresh-token exchange.
    pub client_id: Option<String>,
    /// OAuth2 client secret for the refresh-token exchange.
    pub client_secret: Option<String>,
}

// ---------------------------------------------------------------------------
// Gmail API response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GmailMessageRef {
    id: String,
}

#[derive(Debug, Deserialize)]
struct GmailListResponse {
    #[serde(default)]
    messages: Vec<GmailMessageRef>,
    #[serde(default)]
    result_size_estimate: u64,
    #[serde(default)]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailMessage {
    id: String,
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    label_ids: Vec<String>,
    #[serde(default)]
    snippet: String,
    #[serde(default)]
    payload: Option<GmailPayload>,
}

#[derive(Debug, Deserialize)]
struct GmailPayload {
    #[serde(default)]
    headers: Vec<GmailHeader>,
    #[serde(default)]
    body: Option<GmailBody>,
    #[serde(default)]
    parts: Vec<GmailPart>,
}

#[derive(Debug, Deserialize)]
struct GmailHeader {
    name: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct GmailBody {
    #[serde(default)]
    data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailPart {
    #[serde(default)]
    mime_type: Option<String>,
    #[serde(default)]
    body: Option<GmailBody>,
    #[serde(default)]
    parts: Vec<GmailPart>,
}

#[derive(Debug, Deserialize)]
struct GmailDraftResponse {
    id: String,
}

#[derive(Debug, Deserialize)]
struct TokenRefreshResponse {
    access_token: String,
}

// ---------------------------------------------------------------------------
// Tool
// ---------------------------------------------------------------------------

/// Gmail REST API tool.
///
/// Implements the `gmail` tool with OAuth2 bearer-token authentication backed
/// by an injectable [`TokenStore`].
#[derive(Clone)]
pub struct GmailTool {
    store: Arc<dyn TokenStore>,
}

impl GmailTool {
    /// Create a `gmail` tool backed by the shared ragent credential store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: Arc::new(SqliteTokenStore::shared()),
        }
    }

    /// Create a `gmail` tool with a custom token store (used by tests).
    #[must_use]
    pub fn with_store(store: Arc<dyn TokenStore>) -> Self {
        Self { store }
    }

    /// Resolve Gmail configuration (endpoint overrides + client credentials).
    ///
    /// Precedence: stored auth-time credentials → `ragent.json` `gmail.*`
    /// fields (with `env:` indirection) → `GMAIL_CLIENT_ID` /
    /// `GMAIL_CLIENT_SECRET` environment variables.
    pub fn resolved_config(ctx: &ToolContext, tokens: &GmailTokens) -> GmailResolvedConfig {
        let mut resolved = GmailResolvedConfig {
            api_base: DEFAULT_API_BASE.to_string(),
            token_url: DEFAULT_TOKEN_URL.to_string(),
            client_id: tokens.client_id.clone(),
            client_secret: tokens.client_secret.clone(),
        };
        if let Some(config) = &ctx.config {
            if let Some(base) = config.gmail.base_url.as_deref()
                && !base.is_empty()
            {
                resolved.api_base = base.trim_end_matches('/').to_string();
                // For local mock servers, point the token endpoint at the same host.
                if base.starts_with("http://") {
                    resolved.token_url = format!("{}/oauth2/v4/token", base.trim_end_matches('/'));
                }
            }
            if resolved.client_id.is_none() {
                resolved.client_id =
                    super::channels::resolve_secret(config.gmail.client_id.as_deref());
            }
            if resolved.client_secret.is_none() {
                resolved.client_secret =
                    super::channels::resolve_secret(config.gmail.client_secret.as_deref());
            }
        }
        if resolved.client_id.is_none() {
            resolved.client_id = std::env::var("GMAIL_CLIENT_ID")
                .ok()
                .filter(|v| !v.is_empty());
        }
        if resolved.client_secret.is_none() {
            resolved.client_secret = std::env::var("GMAIL_CLIENT_SECRET")
                .ok()
                .filter(|v| !v.is_empty());
        }
        resolved
    }

    fn client() -> Result<reqwest::Client> {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .build()
            .context("Failed to build HTTP client")
    }

    /// Return a valid access token, refreshing via the stored refresh token
    /// when no access token is present (or when `force_refresh` is set).
    async fn access_token(
        &self,
        tokens: &mut GmailTokens,
        cfg: &GmailResolvedConfig,
        force_refresh: bool,
    ) -> Result<String> {
        if !force_refresh && let Some(token) = tokens.access_token.as_ref() {
            return Ok(token.clone());
        }
        let refresh = tokens.refresh_token.clone().ok_or_else(|| {
            anyhow::anyhow!(
                "No Gmail access token stored. Run: gmail action=\"auth\" access_token=\"...\" \
                 (or supply refresh_token + client_id + client_secret for automatic refresh)."
            )
        })?;
        let client_id = cfg.client_id.clone().ok_or_else(|| {
            anyhow::anyhow!(
                "Gmail refresh requires an OAuth client id: pass client_id to the auth action, \
                 set gmail.client_id in ragent.json, or export GMAIL_CLIENT_ID."
            )
        })?;
        let client_secret = cfg.client_secret.clone().ok_or_else(|| {
            anyhow::anyhow!(
                "Gmail refresh requires an OAuth client secret: pass client_secret to the auth \
                 action, set gmail.client_secret in ragent.json, or export GMAIL_CLIENT_SECRET."
            )
        })?;

        let resp = Self::client()?
            .post(&cfg.token_url)
            .form(&[
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("refresh_token", refresh.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .context("Token refresh request failed")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            bail!("Gmail token refresh failed (HTTP {status}): {body}");
        }
        let refreshed: TokenRefreshResponse = resp
            .json()
            .await
            .context("Invalid token refresh response")?;
        tokens.access_token = Some(refreshed.access_token.clone());
        // Persist the new access token alongside the refresh token.
        self.store.save(tokens)?;
        Ok(refreshed.access_token)
    }

    /// Execute an authenticated request against the Gmail API.
    ///
    /// Retries once with a token refresh when the API answers `401`.
    async fn api_request(
        &self,
        cfg: &GmailResolvedConfig,
        method: reqwest::Method,
        url: &str,
        body: Option<Value>,
    ) -> Result<Value> {
        let client = Self::client()?;
        let mut tokens = self.store.load()?.ok_or_else(|| {
            anyhow::anyhow!(
                "Not authenticated with Gmail. Run gmail action=\"auth\" access_token=\"...\" first."
            )
        })?;
        for attempt in 0..2 {
            let token = self.access_token(&mut tokens, cfg, attempt > 0).await?;
            let mut req = client.request(method.clone(), url).bearer_auth(token);
            if let Some(b) = &body {
                req = req.json(b);
            }
            let resp = req.send().await.context("Gmail API request failed")?;
            if resp.status() == reqwest::StatusCode::UNAUTHORIZED && attempt == 0 {
                // Drop the access token and retry via the refresh token.
                tokens.access_token = None;
                continue;
            }
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                bail!("Gmail API error (HTTP {status}): {body}");
            }
            if resp.status() == reqwest::StatusCode::NO_CONTENT {
                return Ok(Value::Null);
            }
            return resp.json().await.context("Invalid Gmail API response");
        }
        unreachable!("loop performs at most 2 iterations and returns inside")
    }

    /// Extract envelope headers (`from`, `to`, `cc`, `subject`, `date`).
    fn header_map(payload: &GmailPayload) -> Value {
        let interesting = ["from", "to", "cc", "subject", "date"];
        let mut map = serde_json::Map::new();
        for h in &payload.headers {
            let name = h.name.to_lowercase();
            if interesting.contains(&name.as_str()) {
                map.insert(name, Value::String(h.value.clone()));
            }
        }
        Value::Object(map)
    }

    /// Decode a URL-safe base64 Gmail body blob to UTF-8 text.
    fn decode_body(data: &str) -> Option<String> {
        let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(data)
            .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(data))
            .ok()?;
        String::from_utf8(decoded).ok()
    }

    /// Walk the MIME tree looking for a `text/plain` body, falling back to
    /// `text/html` (tags stripped) when that is all that exists.
    fn extract_body(payload: &GmailPayload) -> Option<String> {
        let mut plain: Option<String> = None;
        let mut html: Option<String> = None;
        let mut stack: Vec<&GmailPart> = payload.parts.iter().collect();
        while let Some(part) = stack.pop() {
            if let Some(body) = part.body.as_ref().and_then(|b| b.data.as_deref())
                && let Some(text) = Self::decode_body(body)
            {
                match part.mime_type.as_deref() {
                    Some("text/plain") if plain.is_none() => plain = Some(text),
                    Some("text/html") if html.is_none() => html = Some(text),
                    _ => {}
                }
            }
            for child in &part.parts {
                stack.push(child);
            }
        }
        if plain.is_none()
            && let Some(top) = payload.body.as_ref().and_then(|b| b.data.as_deref())
        {
            plain = Self::decode_body(top);
        }
        if let Some(text) = plain {
            return Some(text);
        }
        html.map(|h| {
            let mut out = String::with_capacity(h.len());
            let mut in_tag = false;
            for ch in h.chars() {
                match ch {
                    '<' => in_tag = true,
                    '>' => in_tag = false,
                    _ if !in_tag => out.push(ch),
                    _ => {}
                }
            }
            out
        })
    }

    fn truncate(s: &str, max: usize) -> String {
        if s.len() <= max {
            s.to_string()
        } else {
            let mut cut = s
                .char_indices()
                .take_while(|(i, _)| *i <= max)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(0);
            if cut == 0 {
                cut = s.len().min(max);
            }
            format!("{}… (truncated)", &s[..cut])
        }
    }

    /// Build a minimal RFC 2822 message and return it URL-safe-base64-encoded.
    ///
    /// Public so tests can verify the wire format without hitting the network.
    pub fn build_raw_message(
        to: &str,
        subject: &str,
        body: &str,
        cc: Option<&str>,
        bcc: Option<&str>,
    ) -> String {
        let mut raw = String::with_capacity(256 + body.len());
        raw.push_str(&format!("To: {to}\r\n"));
        if let Some(cc) = cc {
            raw.push_str(&format!("Cc: {cc}\r\n"));
        }
        if let Some(bcc) = bcc {
            raw.push_str(&format!("Bcc: {bcc}\r\n"));
        }
        raw.push_str(&format!("Subject: {subject}\r\n"));
        raw.push_str("Content-Type: text/plain; charset=UTF-8\r\n\r\n");
        raw.push_str(body);
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw.as_bytes())
    }
}

impl Default for GmailTool {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tool implementation
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl Tool for GmailTool {
    fn name(&self) -> &'static str {
        GMAIL_TOOL_NAME
    }

    fn description(&self) -> &'static str {
        "Search, read, draft, and send Gmail messages using the Gmail REST API. \
             Required parameter: 'action' (search, read, draft, send, auth, status, logout). \
             'query' is required for search; 'id' for read; 'to', 'subject', and 'body' for \
             draft/send; 'access_token' or 'refresh_token' (with client_id/secret) for auth. \
             Requires prior OAuth2 authentication via the auth action."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation to perform",
                    "enum": ["search", "read", "draft", "send", "auth", "status", "logout"]
                },
                "query": {
                    "type": "string",
                    "description": "Gmail search query, e.g. \"from:ci@example.com is:unread\" (search)"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum messages to return for search (default 10, max 100)"
                },
                "id": {
                    "type": "string",
                    "description": "Message id (read)"
                },
                "to": {
                    "type": "string",
                    "description": "Recipient address (draft/send)"
                },
                "subject": {
                    "type": "string",
                    "description": "Subject line (draft/send)"
                },
                "body": {
                    "type": "string",
                    "description": "Plain-text message body (draft/send)"
                },
                "cc": {
                    "type": "string",
                    "description": "Optional Cc header (draft/send)"
                },
                "bcc": {
                    "type": "string",
                    "description": "Optional Bcc header (draft/send)"
                },
                "access_token": {
                    "type": "string",
                    "description": "OAuth2 access token to store (auth)"
                },
                "refresh_token": {
                    "type": "string",
                    "description": "OAuth2 refresh token to store (auth); enables automatic access-token refresh"
                },
                "client_id": {
                    "type": "string",
                    "description": "OAuth2 client id stored for refresh-token exchange (auth)"
                },
                "client_secret": {
                    "type": "string",
                    "description": "OAuth2 client secret stored for refresh-token exchange (auth)"
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "network:send"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let action = input["action"]
            .as_str()
            .context("Missing required 'action' parameter")?;

        match action {
            // ------------------------------------------------------------ auth
            "auth" => {
                let access_token = input["access_token"].as_str().map(str::to_string);
                let refresh_token = input["refresh_token"].as_str().map(str::to_string);
                if access_token.is_none() && refresh_token.is_none() {
                    bail!(
                        "auth requires 'access_token' and/or 'refresh_token'. \
                         Obtain them from https://developers.google.com/oauthplayground \
                         using scope https://mail.google.com/."
                    );
                }
                let mut tokens = self.store.load()?.unwrap_or_default();
                if access_token.is_some() {
                    tokens.access_token = access_token;
                }
                if let Some(rt) = refresh_token {
                    tokens.refresh_token = Some(rt);
                }
                // Client credentials may also be supplied at auth time; they are
                // written into the encrypted credential blob alongside tokens so
                // refresh works later without config changes.
                if let Some(cid) = input["client_id"].as_str() {
                    tokens.client_id = Some(cid.to_string());
                }
                if let Some(csec) = input["client_secret"].as_str() {
                    tokens.client_secret = Some(csec.to_string());
                }
                self.store.save(&tokens)?;
                let has_refresh = tokens.refresh_token.is_some();
                Ok(ToolOutput {
                    content: "Gmail credentials stored (encrypted) in the ragent credential store."
                        .to_string(),
                    metadata: Some(json!({
                        "authenticated": true,
                        "has_refresh_token": has_refresh,
                    })),
                })
            }
            // ------------------------------------------------------------ status
            "status" => match self.store.load() {
                Ok(tokens) => {
                    let (has_access, has_refresh) = match &tokens {
                        Some(t) => (t.access_token.is_some(), t.refresh_token.is_some()),
                        None => (false, false),
                    };
                    let authenticated = has_access || has_refresh;
                    Ok(ToolOutput {
                        content: format!(
                            "gmail: authenticated={authenticated}, access_token={has_access}, refresh_token={has_refresh}"
                        ),
                        metadata: Some(json!({
                            "authenticated": authenticated,
                            "has_access_token": has_access,
                            "has_refresh_token": has_refresh,
                        })),
                    })
                }
                Err(e) => Ok(ToolOutput {
                    content: format!("gmail: UNAVAILABLE — credential store unreachable: {e}"),
                    metadata: Some(json!({
                        "authenticated": false,
                        "next_action": "Ensure the ragent data directory is writable."
                    })),
                }),
            },
            // ------------------------------------------------------------ logout
            "logout" => {
                self.store.clear()?;
                Ok(ToolOutput {
                    content: "Gmail credentials removed.".to_string(),
                    metadata: Some(json!({ "authenticated": false })),
                })
            }
            // ---------------------------------------------------------- search
            "search" => {
                let query = input["query"]
                    .as_str()
                    .context("Missing required 'query' parameter for search action")?;
                let max_results = input["max_results"]
                    .as_u64()
                    .unwrap_or(DEFAULT_SEARCH_RESULTS)
                    .clamp(1, MAX_SEARCH_RESULTS);
                let tokens = self.store.load()?.unwrap_or_default();
                let cfg = Self::resolved_config(ctx, &tokens);

                let url = format!(
                    "{}/gmail/v1/users/me/messages?q={}&maxResults={max_results}",
                    cfg.api_base,
                    urlencode(query),
                );
                let list: GmailListResponse = serde_json::from_value(
                    self.api_request(&cfg, reqwest::Method::GET, &url, None)
                        .await?,
                )
                .context("Invalid search response")?;

                let mut out = Vec::new();
                for mref in &list.messages {
                    let detail_url = format!(
                        "{}/gmail/v1/users/me/messages/{}?format=metadata\
                         &metadataHeaders=From&metadataHeaders=To\
                         &metadataHeaders=Subject&metadataHeaders=Date",
                        cfg.api_base, mref.id
                    );
                    let detail: GmailMessage = serde_json::from_value(
                        self.api_request(&cfg, reqwest::Method::GET, &detail_url, None)
                            .await?,
                    )
                    .context("Invalid message metadata response")?;
                    let headers = detail
                        .payload
                        .as_ref()
                        .map(Self::header_map)
                        .unwrap_or(Value::Null);
                    out.push(json!({
                        "id": detail.id,
                        "thread_id": detail.thread_id,
                        "labels": detail.label_ids,
                        "snippet": detail.snippet,
                        "headers": headers,
                    }));
                }

                let content = format!(
                    "{} message(s) matched (estimate {}).",
                    out.len(),
                    list.result_size_estimate
                );
                Ok(ToolOutput {
                    content,
                    metadata: Some(json!({
                        "count": out.len(),
                        "result_size_estimate": list.result_size_estimate,
                        "next_page_token": list.next_page_token,
                        "messages": out,
                    })),
                })
            }
            // ------------------------------------------------------------ read
            "read" => {
                let id = input["id"]
                    .as_str()
                    .context("Missing required 'id' parameter for read action")?;
                let tokens = self.store.load()?.unwrap_or_default();
                let cfg = Self::resolved_config(ctx, &tokens);
                let url = format!(
                    "{}/gmail/v1/users/me/messages/{}?format=full",
                    cfg.api_base, id
                );
                let msg: GmailMessage = serde_json::from_value(
                    self.api_request(&cfg, reqwest::Method::GET, &url, None)
                        .await?,
                )
                .context("Invalid message response")?;

                let headers = msg
                    .payload
                    .as_ref()
                    .map(Self::header_map)
                    .unwrap_or(Value::Null);
                let body = msg
                    .payload
                    .as_ref()
                    .and_then(Self::extract_body)
                    .map(|b| Self::truncate(&b, MAX_BODY_SNIPPET))
                    .unwrap_or_else(|| msg.snippet.clone());

                let subject = headers["subject"].as_str().unwrap_or("(no subject)");
                let from = headers["from"].as_str().unwrap_or("?");
                Ok(ToolOutput {
                    content: format!("From: {from}\nSubject: {subject}\n\n{body}"),
                    metadata: Some(json!({
                        "id": msg.id,
                        "thread_id": msg.thread_id,
                        "labels": msg.label_ids,
                        "headers": headers,
                        "body": body,
                    })),
                })
            }
            // ---------------------------------------------------- draft | send
            "draft" | "send" => {
                let to = input["to"]
                    .as_str()
                    .context("Missing required 'to' parameter")?;
                let subject = input["subject"].as_str().unwrap_or("(no subject)");
                let body = input["body"].as_str().unwrap_or("");
                let cc = input["cc"].as_str();
                let bcc = input["bcc"].as_str();

                let raw = Self::build_raw_message(to, subject, body, cc, bcc);
                let tokens = self.store.load()?.unwrap_or_default();
                let cfg = Self::resolved_config(ctx, &tokens);

                let (url, req_body) = if action == "draft" {
                    (
                        format!("{}/gmail/v1/users/me/drafts", cfg.api_base),
                        json!({ "message": { "raw": raw } }),
                    )
                } else {
                    (
                        format!("{}/gmail/v1/users/me/messages/send", cfg.api_base),
                        json!({ "raw": raw }),
                    )
                };

                let value = self
                    .api_request(&cfg, reqwest::Method::POST, &url, Some(req_body))
                    .await?;

                if action == "draft" {
                    let draft: GmailDraftResponse =
                        serde_json::from_value(value).context("Invalid draft response")?;
                    Ok(ToolOutput {
                        content: format!("Draft created: {}", draft.id),
                        metadata: Some(json!({ "draft_id": draft.id })),
                    })
                } else {
                    let message_id = value
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string();
                    Ok(ToolOutput {
                        content: format!("Message sent: {message_id}"),
                        metadata: Some(json!({ "message_id": message_id })),
                    })
                }
            }
            other => bail!(
                "Unknown action: '{other}'. Supported actions: search, read, draft, send, \
                 auth, status, logout"
            ),
        }
    }
}

/// Minimal percent-encoding for query values (RFC 3986 unreserved kept).
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(char::from(b));
            }
            _ => {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}
