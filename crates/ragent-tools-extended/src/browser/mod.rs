//! Browser automation tool (`browser`) — JCODEPLAN M4.
//!
//! Provides a single tool that can open URLs, snapshot pages, interact with
//! elements, evaluate JS, scroll, press keys, upload files, capture
//! screenshots, check status, and launch a headless browser. Uses the Chrome
//! DevTools Protocol (CDP) over WebSocket as the backend.
//!
//! # Actions
//!
//! | Action      | Description                                   |
//! |-------------|-----------------------------------------------|
//! | `open`      | Navigate to a URL                             |
//! | `snapshot`  | Get page content as text/markdown             |
//! | `click`     | Click an element by CSS selector              |
//! | `type`      | Type text into the focused element            |
//! | `fill_form` | Fill multiple form fields by selector         |
//! | `select`    | Select an option in a `<select>` element      |
//! | `wait`      | Wait for load, selector, or fixed time        |
//! | `eval`      | Evaluate a JavaScript expression              |
//! | `scroll`    | Scroll the page by relative offset            |
//! | `upload`    | Upload a file to an `<input type="file">`     |
//! | `press`     | Press a keyboard key                          |
//! | `screenshot`| Capture a screenshot as base64 PNG            |
//! | `status`    | Check browser connection status               |
//! | `setup`     | Launch a headless Chrome/Chromium instance    |
//!
//! # Configuration
//!
//! The CDP endpoint is configured in `ragent.json` under the `browser` key:
//!
//! ```json
//! {
//!   "browser": {
//!     "cdp_endpoint": "http://127.0.0.1:9222",
//!     "default_headless": true
//!   }
//! }
//! ```
//!
//! If not configured, the tool defaults to `http://127.0.0.1:9222`.
//!
//! # Graceful degradation
//!
//! When no browser is available (no CDP endpoint, no Chrome/Chromium
//! installed), the tool returns honest error messages with actionable
//! `next_action` guidance, similar to `mf_screenshot`.

pub mod actions;
pub mod cdp;
pub mod launch;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tracing::debug;

use crate::{Tool, ToolContext, ToolOutput};

/// Tool name used by the LLM.
pub const BROWSER_TOOL_NAME: &str = "browser";

/// Default CDP HTTP endpoint.
pub const DEFAULT_CDP_ENDPOINT: &str = "http://127.0.0.1:9222";

/// Browser automation tool.
///
/// Implements the `browser` tool with a CDP (Chrome DevTools Protocol) backend.
/// Connects to a headless Chrome/Chromium instance over WebSocket to perform
/// browser actions.
pub struct BrowserTool;

impl BrowserTool {
    /// Create a new `browser` tool instance.
    pub const fn new() -> Self {
        Self
    }

    /// Resolve the CDP HTTP endpoint from config or default.
    fn resolve_endpoint(ctx: &ToolContext) -> String {
        if let Some(config) = &ctx.config
            && let Some(endpoint) = config.browser.cdp_endpoint.as_ref()
            && !endpoint.is_empty()
        {
            return endpoint.clone();
        }
        DEFAULT_CDP_ENDPOINT.to_string()
    }
}

impl Default for BrowserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &'static str {
        BROWSER_TOOL_NAME
    }

    fn description(&self) -> &'static str {
        "Browser automation via Chrome DevTools Protocol (CDP). Required parameter: \
         'action' (one of open, snapshot, click, type, fill_form, select, wait, eval, \
         scroll, upload, press, screenshot, status, setup). Other parameters depend on \
         the action, e.g. 'url' for open, 'selector' for click/type/select/upload/wait, \
         'expression' for eval, 'file_path' for upload, and 'port'/'headless' for setup. \
         Requires a running Chrome/Chromium with --remote-debugging-port=9222 (use \
         action=setup to launch one automatically)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "open", "snapshot", "click", "type", "fill_form",
                        "select", "wait", "eval", "scroll", "upload",
                        "press", "screenshot", "status", "setup"
                    ],
                    "description": "Browser action to perform"
                },
                "url": {
                    "type": "string",
                    "description": "URL to navigate to (required for action=open)"
                },
                "selector": {
                    "type": "string",
                    "description": "CSS selector for the target element (click, type, select, upload, wait)"
                },
                "text": {
                    "type": "string",
                    "description": "Text to type (action=type) or key to press (action=press)"
                },
                "value": {
                    "type": "string",
                    "description": "Value for select option (action=select)"
                },
                "fields": {
                    "type": "object",
                    "description": "Map of CSS selector to value for fill_form",
                    "additionalProperties": { "type": "string" }
                },
                "expression": {
                    "type": "string",
                    "description": "JavaScript expression to evaluate (action=eval)"
                },
                "wait": {
                    "type": "boolean",
                    "description": "Wait for page load after navigation (action=open, default: true)"
                },
                "condition": {
                    "type": "string",
                    "enum": ["load", "selector", "time", "none"],
                    "description": "Wait condition (action=wait)"
                },
                "milliseconds": {
                    "type": "integer",
                    "description": "Wait duration in milliseconds (action=wait, condition=time)"
                },
                "scroll_x": {
                    "type": "integer",
                    "description": "Horizontal scroll offset (action=scroll, default: 0)"
                },
                "scroll_y": {
                    "type": "integer",
                    "description": "Vertical scroll offset (action=scroll, default: 0)"
                },
                "file_path": {
                    "type": "string",
                    "description": "Path to file to upload (action=upload)"
                },
                "full_page": {
                    "type": "boolean",
                    "description": "Capture full page screenshot (action=screenshot, default: false)"
                },
                "css_selector": {
                    "type": "string",
                    "description": "CSS selector to narrow snapshot scope (action=snapshot)"
                },
                "port": {
                    "type": "integer",
                    "description": "CDP port for setup (action=setup, default: 9222)"
                },
                "headless": {
                    "type": "boolean",
                    "description": "Run browser in headless mode (action=setup, default: true)"
                }
            },
            "required": ["action"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "web"
    }

    /// # Errors
    ///
    /// Returns an error if the action fails, the browser is not available,
    /// or required parameters are missing. When no CDP endpoint is reachable,
    /// returns an honest error with `next_action` guidance instead of hanging.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let action = input
            .get("action")
            .and_then(Value::as_str)
            .context("Missing required 'action' parameter")?;

        debug!(action, "browser tool: executing action");

        let result = match action {
            "setup" => self.handle_setup(&input).await,
            "status" => self.handle_status(ctx).await,
            _ => self.handle_browser_action(action, &input, ctx).await,
        };

        match result {
            Ok(value) => {
                let content = format_browser_result(action, &value);
                Ok(ToolOutput {
                    content,
                    metadata: Some(value),
                })
            }
            Err(e) => {
                // Check if this is a connection error — provide actionable guidance.
                let err_str = e.to_string();
                if err_str.contains("ConnectionFailed")
                    || err_str.contains("Connection refused")
                    || err_str.contains("ConnectionClosed")
                    || err_str.contains("No CDP target")
                    || err_str.contains("endpoint")
                    || err_str.contains("error sending request")
                    || err_str.contains("tcp connect error")
                    || err_str.contains("Connect")
                    || err_str.contains("connect error")
                {
                    let endpoint = Self::resolve_endpoint(ctx);
                    let content = format!(
                        "Browser is not available at {endpoint}.\n\n\
                         Error: {err_str}\n\n\
                         next_action: use action=\"setup\" to launch a headless \
                         Chrome/Chromium instance, or start Chrome manually with:\n\
                         google-chrome --remote-debugging-port=9222 --headless=new"
                    );
                    return Ok(ToolOutput {
                        content,
                        metadata: Some(json!({
                            "action": action,
                            "available": false,
                            "endpoint": endpoint,
                            "error": err_str,
                            "next_action": "use action=setup to launch a browser",
                        })),
                    });
                }
                Err(e)
            }
        }
    }
}

impl BrowserTool {
    /// Handle the `setup` action — launch a headless browser.
    async fn handle_setup(&self, input: &Value) -> Result<Value> {
        let port = input.get("port").and_then(Value::as_u64).map(|p| p as u16);
        let headless = input
            .get("headless")
            .and_then(Value::as_bool)
            .unwrap_or(true);

        launch::action_setup(port, headless).await
    }

    /// Handle the `status` action — check browser connectivity.
    async fn handle_status(&self, ctx: &ToolContext) -> Result<Value> {
        let endpoint = Self::resolve_endpoint(ctx);
        actions::action_status_raw(&endpoint).await
    }

    /// Handle all browser actions that require a CDP connection.
    async fn handle_browser_action(
        &self,
        action: &str,
        input: &Value,
        ctx: &ToolContext,
    ) -> Result<Value> {
        let endpoint = Self::resolve_endpoint(ctx);

        // Discover targets and find a page target.
        let targets = cdp::list_targets(&endpoint).await?;
        let page_target = cdp::first_page_target(&targets)?;

        // Connect to the target's WebSocket.
        let conn = cdp::CdpConnection::connect(&page_target.web_socket_debugger_url).await?;

        // Enable required domains.
        actions::enable_domains(&conn).await?;

        // Dispatch to the appropriate action handler.
        let result = match action {
            "open" => {
                let url = input
                    .get("url")
                    .and_then(Value::as_str)
                    .context("Missing required 'url' parameter for action=open")?;
                let wait = input.get("wait").and_then(Value::as_bool).unwrap_or(true);
                actions::action_open(&conn, url, wait).await
            }
            "snapshot" => {
                let css_selector = input
                    .get("css_selector")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty());
                actions::action_snapshot(&conn, css_selector).await
            }
            "click" => {
                let selector = input
                    .get("selector")
                    .and_then(Value::as_str)
                    .context("Missing required 'selector' parameter for action=click")?;
                actions::action_click(&conn, selector).await
            }
            "type" => {
                let text = input
                    .get("text")
                    .and_then(Value::as_str)
                    .context("Missing required 'text' parameter for action=type")?;
                let selector = input
                    .get("selector")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty());
                actions::action_type(&conn, text, selector).await
            }
            "fill_form" => {
                let fields = input
                    .get("fields")
                    .and_then(Value::as_object)
                    .context("Missing required 'fields' parameter for action=fill_form")?;
                actions::action_fill_form(&conn, fields).await
            }
            "select" => {
                let selector = input
                    .get("selector")
                    .and_then(Value::as_str)
                    .context("Missing required 'selector' parameter for action=select")?;
                let value = input
                    .get("value")
                    .and_then(Value::as_str)
                    .context("Missing required 'value' parameter for action=select")?;
                actions::action_select(&conn, selector, value).await
            }
            "wait" => {
                let condition = input
                    .get("condition")
                    .and_then(Value::as_str)
                    .unwrap_or("none");
                let selector = input
                    .get("selector")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty());
                let ms = input.get("milliseconds").and_then(Value::as_u64);
                actions::action_wait(&conn, condition, selector, ms).await
            }
            "eval" => {
                let expression = input
                    .get("expression")
                    .and_then(Value::as_str)
                    .context("Missing required 'expression' parameter for action=eval")?;
                actions::action_eval(&conn, expression).await
            }
            "scroll" => {
                let x = input.get("scroll_x").and_then(Value::as_i64).unwrap_or(0);
                let y = input.get("scroll_y").and_then(Value::as_i64).unwrap_or(0);
                actions::action_scroll(&conn, x, y).await
            }
            "upload" => {
                let selector = input
                    .get("selector")
                    .and_then(Value::as_str)
                    .context("Missing required 'selector' parameter for action=upload")?;
                let file_path = input
                    .get("file_path")
                    .and_then(Value::as_str)
                    .context("Missing required 'file_path' parameter for action=upload")?;
                actions::action_upload(&conn, selector, file_path).await
            }
            "press" => {
                let key = input
                    .get("text")
                    .or_else(|| input.get("key"))
                    .and_then(Value::as_str)
                    .context("Missing required 'text' or 'key' parameter for action=press")?;
                actions::action_press(&conn, key).await
            }
            "screenshot" => {
                let full_page = input
                    .get("full_page")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                actions::action_screenshot(&conn, full_page).await
            }
            _ => bail!("Unknown browser action: {action}"),
        };

        // Close the connection when done.
        conn.close();

        result
    }
}

/// Format a browser action result as human-readable text.
fn format_browser_result(action: &str, value: &Value) -> String {
    match action {
        "open" => {
            let url = value.get("url").and_then(Value::as_str).unwrap_or("?");
            let loaded = value
                .get("loaded")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            format!("Navigated to {url}\nPage loaded: {loaded}")
        }
        "snapshot" => {
            let title = value.get("title").and_then(Value::as_str).unwrap_or("");
            let url = value.get("url").and_then(Value::as_str).unwrap_or("");
            let text = value.get("text").and_then(Value::as_str).unwrap_or("");
            let header = if title.is_empty() {
                format!("URL: {url}\n\n")
            } else {
                format!("Title: {title}\nURL: {url}\n\n")
            };
            // Truncate very long snapshots.
            if text.len() > 50_000 {
                format!(
                    "{header}{}\n\n[truncated — {}/{} chars shown]",
                    &text[..50_000],
                    50_000,
                    text.len()
                )
            } else {
                format!("{header}{text}")
            }
        }
        "click" => {
            let selector = value.get("selector").and_then(Value::as_str).unwrap_or("?");
            format!("Clicked element: {selector}")
        }
        "type" => {
            let typed = value.get("typed").and_then(Value::as_bool).unwrap_or(false);
            format!("Text typed: {typed}")
        }
        "fill_form" => {
            let all_success = value
                .get("all_success")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            format!("Form fill complete (all fields: {all_success})")
        }
        "select" => {
            let selector = value.get("selector").and_then(Value::as_str).unwrap_or("?");
            let val = value.get("value").and_then(Value::as_str).unwrap_or("?");
            format!("Selected '{val}' in {selector}")
        }
        "wait" => {
            let condition = value
                .get("condition")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let waited = value
                .get("waited")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            format!("Wait condition '{condition}': {waited}")
        }
        "eval" => {
            let result = value.get("result").cloned().unwrap_or(Value::Null);
            let result_str =
                serde_json::to_string_pretty(&result).unwrap_or_else(|_| "null".to_string());
            // Truncate very long results.
            if result_str.len() > 10_000 {
                format!("Result (truncated):\n{}", &result_str[..10_000])
            } else {
                format!("Result:\n{result_str}")
            }
        }
        "scroll" => {
            format!(
                "Scrolled to: {}",
                value.get("position").cloned().unwrap_or(Value::Null)
            )
        }
        "upload" => {
            let file = value.get("file").and_then(Value::as_str).unwrap_or("?");
            format!("File uploaded: {file}")
        }
        "press" => {
            let key = value.get("key").and_then(Value::as_str).unwrap_or("?");
            format!("Key pressed: {key}")
        }
        "screenshot" => {
            let data_len = value
                .get("data_length")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let full_page = value
                .get("full_page")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            format!("Screenshot captured ({data_len} bytes base64 PNG, full_page={full_page})")
        }
        "status" => {
            let browser = value
                .get("browser")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let pages = value
                .get("page_targets")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let available = value
                .get("available")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if available {
                format!("Browser: {browser}\nPage targets: {pages}\nStatus: available")
            } else {
                "Status: not available".to_string()
            }
        }
        "setup" => {
            let status = value
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let endpoint = value
                .get("http_endpoint")
                .and_then(Value::as_str)
                .unwrap_or("?");
            let browser = value.get("browser").and_then(Value::as_str).unwrap_or("?");
            format!("Setup: {status}\nEndpoint: {endpoint}\nBrowser: {browser}")
        }
        _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| "unknown action".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_tool_name() {
        let tool = BrowserTool;
        assert_eq!(tool.name(), "browser");
    }

    #[test]
    fn test_browser_tool_permission_category() {
        let tool = BrowserTool;
        assert_eq!(tool.permission_category(), "web");
    }

    #[test]
    fn test_parameters_schema_has_all_actions() {
        let tool = BrowserTool;
        let schema = tool.parameters_schema();
        let actions = schema
            .pointer("/properties/action/enum")
            .and_then(Value::as_array)
            .expect("action enum should exist");

        let action_names: Vec<&str> = actions.iter().filter_map(Value::as_str).collect();

        assert!(action_names.contains(&"open"));
        assert!(action_names.contains(&"snapshot"));
        assert!(action_names.contains(&"click"));
        assert!(action_names.contains(&"type"));
        assert!(action_names.contains(&"fill_form"));
        assert!(action_names.contains(&"select"));
        assert!(action_names.contains(&"wait"));
        assert!(action_names.contains(&"eval"));
        assert!(action_names.contains(&"scroll"));
        assert!(action_names.contains(&"upload"));
        assert!(action_names.contains(&"press"));
        assert!(action_names.contains(&"screenshot"));
        assert!(action_names.contains(&"status"));
        assert!(action_names.contains(&"setup"));
    }

    #[test]
    fn test_parameters_schema_requires_action() {
        let tool = BrowserTool;
        let schema = tool.parameters_schema();
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .expect("required should exist");
        assert!(required.iter().any(|v| v.as_str() == Some("action")));
    }

    #[test]
    fn test_default_cdp_endpoint() {
        assert_eq!(DEFAULT_CDP_ENDPOINT, "http://127.0.0.1:9222");
    }

    #[test]
    fn test_format_browser_result_snapshot() {
        let value = json!({
            "title": "Example",
            "url": "https://example.com",
            "text": "Hello world",
            "html_length": 100,
            "node_id": 1,
        });
        let text = format_browser_result("snapshot", &value);
        assert!(text.contains("Example"));
        assert!(text.contains("Hello world"));
    }

    #[test]
    fn test_format_browser_result_open() {
        let value = json!({
            "url": "https://example.com",
            "loaded": true,
        });
        let text = format_browser_result("open", &value);
        assert!(text.contains("https://example.com"));
        assert!(text.contains("true"));
    }

    #[test]
    fn test_format_browser_result_click() {
        let value = json!({ "selector": "#button", "clicked": true });
        let text = format_browser_result("click", &value);
        assert!(text.contains("#button"));
    }

    #[test]
    fn test_format_browser_result_eval() {
        let value = json!({ "expression": "1+1", "result": 2 });
        let text = format_browser_result("eval", &value);
        assert!(text.contains('2'));
    }

    #[test]
    fn test_format_browser_result_screenshot() {
        let value = json!({
            "data_length": 1024,
            "full_page": true,
            "format": "png",
        });
        let text = format_browser_result("screenshot", &value);
        assert!(text.contains("1024"));
        assert!(text.contains("true"));
    }

    #[test]
    fn test_format_browser_result_status_available() {
        let value = json!({
            "browser": "Chrome/131.0",
            "page_targets": 2,
            "available": true,
        });
        let text = format_browser_result("status", &value);
        assert!(text.contains("Chrome/131.0"));
        assert!(text.contains("available"));
    }

    #[test]
    fn test_format_browser_result_setup() {
        let value = json!({
            "status": "launched",
            "http_endpoint": "http://127.0.0.1:9222",
            "browser": "Chrome/131.0",
        });
        let text = format_browser_result("setup", &value);
        assert!(text.contains("launched"));
        assert!(text.contains("Chrome/131.0"));
    }
}
