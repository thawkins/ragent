//! Browser action implementations.
//!
//! Each function corresponds to one `browser` tool action. They take a
//! [`super::cdp::CdpConnection`] and action parameters, execute the
//! appropriate CDP commands, and return a [`super::BrowserActionOutput`].
//!
//! # Actions
//!
//! | Action     | CDP domains used                         |
//! |------------|------------------------------------------|
//! | `open`     | Page.navigate                            |
//! | `snapshot` | DOM.getDocument + DOM.getOuterHTML       |
//! | `click`    | DOM.querySelector + DOM.resolveNode + Input.dispatchMouseEvent |
//! | `type`     | Input.insertText (or dispatchKeyEvent per char) |
//! | `fill_form`| Runtime.evaluate (batch querySelector + value set) |
//! | `select`   | DOM.querySelector + DOM.setAttributeValue |
//! | `wait`     | Page.loadEventFired or fixed sleep       |
//! | `eval`     | Runtime.evaluate                         |
//! | `scroll`   | Runtime.evaluate (window.scrollBy)       |
//! | `upload`   | DOM.setFileInputFiles                    |
//! | `press`    | Input.dispatchKeyEvent                   |
//! | `screenshot`| Page.captureScreenshot                  |
//! | `status`   | GET /json/version + /json                |
//! | `setup`    | Launch Chrome/Chromium subprocess        |

use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tracing::debug;

use super::cdp::CdpConnection;

/// Maximum content length for snapshot/screenshot responses (1 MiB).
const MAX_CONTENT_BYTES: usize = 1024 * 1024;

/// Ensure required CDP domains are enabled.
///
/// Called once after connecting to a target. Enables `Page`, `DOM`, and
/// `Runtime` domains so subsequent actions work.
///
/// # Errors
///
/// Returns an error if any domain-enable command fails.
pub async fn enable_domains(conn: &CdpConnection) -> Result<()> {
    conn.command_default("Page.enable", None).await?;
    conn.command_default("DOM.enable", None).await?;
    conn.command_default("Runtime.enable", None).await?;
    Ok(())
}

/// Navigate to a URL.
///
/// Sends `Page.navigate` and optionally waits for `Page.loadEventFired`.
///
/// # Arguments
///
/// * `conn` — the CDP connection.
/// * `url` — the URL to navigate to.
/// * `wait` — if `true`, wait for the page load event (up to 30s).
///
/// # Errors
///
/// Returns an error if navigation fails or the load event times out.
pub async fn action_open(conn: &CdpConnection, url: &str, wait: bool) -> Result<Value> {
    debug!(url, wait, "browser: open");

    let params = json!({ "url": url });
    let result = conn
        .command("Page.navigate", Some(params), Duration::from_secs(60))
        .await?;

    // Check for navigation error.
    if let Some(err_text) = result.get("errorText").and_then(Value::as_str)
        && !err_text.is_empty()
    {
        bail!("Navigation failed: {err_text}");
    }

    let frame_id = result
        .get("frameId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let loader_id = result
        .get("loaderId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    if wait {
        // Subscribe to events and wait for Page.loadEventFired.
        let mut events = conn.subscribe();
        let _ = tokio::time::timeout(Duration::from_secs(30), async {
            while let Ok(event) = events.recv().await {
                if event.method == "Page.loadEventFired" {
                    break;
                }
            }
        })
        .await
        .context("page load event timeout");
    }

    Ok(json!({
        "url": url,
        "frameId": frame_id,
        "loaderId": loader_id,
        "loaded": wait,
    }))
}

/// Capture the current page content as markdown/text.
///
/// Retrieves the full DOM tree and extracts the text content.
///
/// # Errors
///
/// Returns an error if DOM commands fail.
pub async fn action_snapshot(conn: &CdpConnection, css_selector: Option<&str>) -> Result<Value> {
    debug!(css_selector, "browser: snapshot");

    // Get the document root.
    let doc = conn
        .command_default(
            "DOM.getDocument",
            Some(json!({ "depth": -1, "pierce": true })),
        )
        .await?;

    let root_node_id = doc
        .pointer("/root/nodeId")
        .and_then(Value::as_u64)
        .context("DOM.getDocument did not return root nodeId")?;

    // If a CSS selector is provided, query for it.
    let (node_id, html) = if let Some(selector) = css_selector {
        let query_result = conn
            .command_default(
                "DOM.querySelector",
                Some(json!({ "nodeId": root_node_id, "selector": selector })),
            )
            .await?;

        let target_node_id = query_result
            .get("nodeId")
            .and_then(Value::as_u64)
            .context("querySelector returned no nodeId")?;

        if target_node_id == 0 {
            bail!("No element found matching selector: {selector}");
        }

        let outer_html = conn
            .command_default(
                "DOM.getOuterHTML",
                Some(json!({ "nodeId": target_node_id })),
            )
            .await?;

        (
            target_node_id,
            outer_html
                .get("outerHTML")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        )
    } else {
        // Get the full document outer HTML.
        let outer_html = conn
            .command_default("DOM.getOuterHTML", Some(json!({ "nodeId": root_node_id })))
            .await?;

        let html = outer_html
            .get("outerHTML")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        (root_node_id, html)
    };

    // Convert HTML to text using the existing extractor.
    let text = html_to_text(&html);

    // Extract the page title.
    let title_result = conn
        .command_default(
            "Runtime.evaluate",
            Some(json!({ "expression": "document.title" })),
        )
        .await?;
    let title = title_result
        .pointer("/result/value")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Extract the current URL.
    let url_result = conn
        .command_default(
            "Runtime.evaluate",
            Some(json!({ "expression": "window.location.href" })),
        )
        .await?;
    let url = url_result
        .pointer("/result/value")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    Ok(json!({
        "url": url,
        "title": title,
        "text": text,
        "html_length": html.len(),
        "node_id": node_id,
    }))
}

/// Click an element matching a CSS selector.
///
/// Uses `DOM.querySelector` to find the element, then dispatches a mouse
/// press + release at its center.
///
/// # Errors
///
/// Returns an error if the selector doesn't match or the click fails.
pub async fn action_click(conn: &CdpConnection, selector: &str) -> Result<Value> {
    debug!(selector, "browser: click");

    let node_info = resolve_selector(conn, selector).await?;
    let node_id = node_info
        .get("nodeId")
        .and_then(Value::as_u64)
        .context("resolved node has no nodeId")?;

    // Get the box model to find the center coordinates.
    let box_model = conn
        .command_default("DOM.getBoxModel", Some(json!({ "nodeId": node_id })))
        .await?;

    let quad = box_model
        .pointer("/model/border")
        .and_then(Value::as_array)
        .context("getBoxModel returned no border quad");

    let quad = quad?;
    if quad.len() < 8 {
        bail!("box model quad has insufficient points");
    }

    let coords: Vec<f64> = quad.iter().filter_map(Value::as_f64).collect();
    if coords.len() < 8 {
        bail!("could not parse box model coordinates");
    }

    let x = (coords[0] + coords[2] + coords[4] + coords[6]) / 4.0;
    let y = (coords[1] + coords[3] + coords[5] + coords[7]) / 4.0;

    // Dispatch mouse pressed.
    conn.command_default(
        "Input.dispatchMouseEvent",
        Some(json!({
            "type": "mousePressed",
            "x": x,
            "y": y,
            "button": "left",
            "clickCount": 1,
        })),
    )
    .await?;

    // Dispatch mouse released.
    conn.command_default(
        "Input.dispatchMouseEvent",
        Some(json!({
            "type": "mouseReleased",
            "x": x,
            "y": y,
            "button": "left",
            "clickCount": 1,
        })),
    )
    .await?;

    Ok(json!({
        "selector": selector,
        "clicked": true,
        "x": x,
        "y": y,
    }))
}

/// Type text into the currently focused element (or an element matching a
/// selector).
///
/// # Errors
///
/// Returns an error if focusing or typing fails.
pub async fn action_type(
    conn: &CdpConnection,
    text: &str,
    selector: Option<&str>,
) -> Result<Value> {
    debug!(text, selector, "browser: type");

    // Optionally focus the element first.
    if let Some(selector) = selector {
        let node_info = resolve_selector(conn, selector).await?;
        let node_id = node_info
            .get("nodeId")
            .and_then(Value::as_u64)
            .context("resolved node has no nodeId")?;

        conn.command_default("DOM.focus", Some(json!({ "nodeId": node_id })))
            .await?;
    }

    // Use Input.insertText for efficient bulk text entry.
    conn.command_default("Input.insertText", Some(json!({ "text": text })))
        .await?;

    Ok(json!({
        "text": text,
        "selector": selector,
        "typed": true,
    }))
}

/// Fill multiple form fields in one action.
///
/// # Arguments
///
/// * `fields` — a map of CSS selector → value pairs.
///
/// # Errors
///
/// Returns an error if any field cannot be found or filled.
pub async fn action_fill_form(
    conn: &CdpConnection,
    fields: &serde_json::Map<String, Value>,
) -> Result<Value> {
    debug!(field_count = fields.len(), "browser: fill_form");

    let mut results = Vec::new();

    for (selector, value) in fields {
        let value_str = match value {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };

        // Use Runtime.evaluate to set the value directly.
        let escaped_selector = json!(selector);
        let escaped_value = json!(value_str);
        let expr = format!(
            "(function() {{ var el = document.querySelector({escaped_selector}); \
             if (!el) return {{ success: false, error: 'not found' }}; \
             el.value = {escaped_value}; \
             el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
             el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
             return {{ success: true }}; }})()"
        );

        let result = conn
            .command_default("Runtime.evaluate", Some(json!({ "expression": expr })))
            .await?;

        let success = result
            .pointer("/result/value/success")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        results.push(json!({
            "selector": selector,
            "value": value_str,
            "success": success,
        }));
    }

    let all_success = results
        .iter()
        .all(|r| r.get("success").and_then(Value::as_bool).unwrap_or(false));

    Ok(json!({
        "fields": results,
        "all_success": all_success,
    }))
}

/// Select an option in a `<select>` element.
///
/// # Errors
///
/// Returns an error if the element or option cannot be found.
pub async fn action_select(conn: &CdpConnection, selector: &str, value: &str) -> Result<Value> {
    debug!(selector, value, "browser: select");

    let escaped_selector = json!(selector);
    let escaped_value = json!(value);
    let expr = format!(
        "(function() {{ var el = document.querySelector({escaped_selector}); \
         if (!el) return {{ success: false, error: 'element not found' }}; \
         el.value = {escaped_value}; \
         el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
         return {{ success: true }}; }})()"
    );

    let result = conn
        .command_default("Runtime.evaluate", Some(json!({ "expression": expr })))
        .await?;

    let success = result
        .pointer("/result/value/success")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if !success {
        let error = result
            .pointer("/result/value/error")
            .and_then(Value::as_str)
            .unwrap_or("unknown error");
        bail!("select failed: {error}");
    }

    Ok(json!({
        "selector": selector,
        "value": value,
        "selected": true,
    }))
}

/// Wait for a condition or a fixed duration.
///
/// # Arguments
///
/// * `condition` — `"load"` (wait for page load), `"selector"` (wait for
///   element), `"time"` (fixed sleep), or `"none"` (no wait).
/// * `selector` — CSS selector (required for `condition=selector`).
/// * `milliseconds` — duration for `condition=time` (default 1000).
///
/// # Errors
///
/// Returns an error if a wait condition times out.
pub async fn action_wait(
    conn: &CdpConnection,
    condition: &str,
    selector: Option<&str>,
    milliseconds: Option<u64>,
) -> Result<Value> {
    debug!(condition, selector, milliseconds, "browser: wait");

    match condition {
        "load" => {
            let mut events = conn.subscribe();
            match tokio::time::timeout(Duration::from_secs(30), async {
                while let Ok(event) = events.recv().await {
                    if event.method == "Page.loadEventFired" {
                        return Ok(());
                    }
                }
                bail!("connection closed while waiting for load event");
            })
            .await
            {
                Ok(Ok(())) => Ok(json!({ "condition": "load", "waited": true })),
                Ok(Err(e)) => Err(e),
                Err(_) => bail!("page load event timeout (30s)"),
            }
        }
        "selector" => {
            let selector = selector.context("wait condition=selector requires a selector")?;
            let escaped_selector = json!(selector);
            // Poll for the element every 100ms, up to 30s.
            let deadline = std::time::Instant::now() + Duration::from_secs(30);
            loop {
                let expr = format!("document.querySelector({escaped_selector}) !== null");
                let result = conn
                    .command_default("Runtime.evaluate", Some(json!({ "expression": expr })))
                    .await?;
                let found = result
                    .pointer("/result/value")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if found {
                    return Ok(json!({
                        "condition": "selector",
                        "selector": selector,
                        "waited": true,
                    }));
                }
                if std::time::Instant::now() > deadline {
                    bail!("selector wait timeout (30s): {selector}");
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        "time" => {
            let ms = milliseconds.unwrap_or(1000);
            tokio::time::sleep(Duration::from_millis(ms)).await;
            Ok(json!({ "condition": "time", "milliseconds": ms, "waited": true }))
        }
        _ => {
            // "none" or unknown — no wait.
            Ok(json!({ "condition": condition, "waited": false }))
        }
    }
}

/// Evaluate a JavaScript expression and return the result.
///
/// # Errors
///
/// Returns an error if the evaluation throws or the result cannot be
/// serialised.
pub async fn action_eval(conn: &CdpConnection, expression: &str) -> Result<Value> {
    debug!(expr_len = expression.len(), "browser: eval");

    let result = conn
        .command_default(
            "Runtime.evaluate",
            Some(json!({
                "expression": expression,
                "returnByValue": true,
                "awaitPromise": true,
                "userGesture": true,
            })),
        )
        .await?;

    // Check for evaluation exception.
    if let Some(exception) = result.get("exceptionDetails") {
        let text = exception
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or("evaluation error");
        let description = exception
            .pointer("/exception/description")
            .and_then(Value::as_str)
            .unwrap_or("");
        bail!("JS evaluation error: {text} {description}");
    }

    let value = result
        .pointer("/result/value")
        .cloned()
        .unwrap_or(Value::Null);

    Ok(json!({
        "expression": expression,
        "result": value,
    }))
}

/// Scroll the page by a relative offset.
///
/// # Errors
///
/// Returns an error if the scroll command fails.
pub async fn action_scroll(conn: &CdpConnection, x: i64, y: i64) -> Result<Value> {
    debug!(x, y, "browser: scroll");

    let expr = format!("window.scrollBy({x}, {y})");
    conn.command_default("Runtime.evaluate", Some(json!({ "expression": expr })))
        .await?;

    // Get the new scroll position.
    let pos_result = conn
        .command_default(
            "Runtime.evaluate",
            Some(json!({
                "expression": "JSON.stringify({x: window.scrollX, y: window.scrollY})",
                "returnByValue": true,
            })),
        )
        .await?;

    let position = pos_result
        .pointer("/result/value")
        .cloned()
        .unwrap_or(Value::Null);

    Ok(json!({
        "scroll_x": x,
        "scroll_y": y,
        "position": position,
    }))
}

/// Upload a file to a file input element.
///
/// # Arguments
///
/// * `selector` — CSS selector for the `<input type="file">` element.
/// * `file_path` — absolute path to the file to upload.
///
/// # Errors
///
/// Returns an error if the element cannot be found or the file doesn't exist.
pub async fn action_upload(conn: &CdpConnection, selector: &str, file_path: &str) -> Result<Value> {
    debug!(selector, file_path, "browser: upload");

    // Verify the file exists.
    if !std::path::Path::new(file_path).exists() {
        bail!("file not found: {file_path}");
    }

    let node_info = resolve_selector(conn, selector).await?;
    let node_id = node_info
        .get("nodeId")
        .and_then(Value::as_u64)
        .context("resolved node has no nodeId")?;

    conn.command_default(
        "DOM.setFileInputFiles",
        Some(json!({
            "nodeId": node_id,
            "files": [file_path],
        })),
    )
    .await?;

    Ok(json!({
        "selector": selector,
        "file": file_path,
        "uploaded": true,
    }))
}

/// Press a keyboard key.
///
/// # Arguments
///
/// * `key` — the key to press (e.g. `"Enter"`, `"Tab"`, `"Escape"`).
///
/// # Errors
///
/// Returns an error if the key event dispatch fails.
pub async fn action_press(conn: &CdpConnection, key: &str) -> Result<Value> {
    debug!(key, "browser: press");

    // Key down.
    conn.command_default(
        "Input.dispatchKeyEvent",
        Some(json!({
            "type": "keyDown",
            "key": key,
            "code": key_code_for(key),
            "windowsVirtualKeyCode": virtual_key_code_for(key),
        })),
    )
    .await?;

    // Key up.
    conn.command_default(
        "Input.dispatchKeyEvent",
        Some(json!({
            "type": "keyUp",
            "key": key,
            "code": key_code_for(key),
            "windowsVirtualKeyCode": virtual_key_code_for(key),
        })),
    )
    .await?;

    Ok(json!({
        "key": key,
        "pressed": true,
    }))
}

/// Capture a screenshot as base64-encoded PNG.
///
/// # Errors
///
/// Returns an error if the screenshot capture fails.
pub async fn action_screenshot(conn: &CdpConnection, full_page: bool) -> Result<Value> {
    debug!(full_page, "browser: screenshot");

    let result = conn
        .command_default(
            "Page.captureScreenshot",
            Some(json!({
                "format": "png",
                "captureBeyondViewport": full_page,
            })),
        )
        .await?;

    let data = result
        .get("data")
        .and_then(Value::as_str)
        .context("captureScreenshot returned no data")?;

    // Truncate if too large.
    let truncated = data.len() > MAX_CONTENT_BYTES;
    let data = if truncated {
        &data[..MAX_CONTENT_BYTES]
    } else {
        data
    };

    Ok(json!({
        "format": "png",
        "data": data,
        "data_length": data.len(),
        "truncated": truncated,
        "full_page": full_page,
    }))
}

// ── Helper functions ─────────────────────────────────────────────��────────

/// Resolve a CSS selector to a DOM node.
///
/// Uses `DOM.querySelector` on the document root.
///
/// # Errors
///
/// Returns an error if the document root cannot be obtained or the selector
/// doesn't match any element.
async fn resolve_selector(conn: &CdpConnection, selector: &str) -> Result<Value> {
    let doc = conn
        .command_default("DOM.getDocument", Some(json!({ "depth": 0 })))
        .await?;

    let root_node_id = doc
        .pointer("/root/nodeId")
        .and_then(Value::as_u64)
        .context("DOM.getDocument did not return root nodeId")?;

    let query_result = conn
        .command_default(
            "DOM.querySelector",
            Some(json!({ "nodeId": root_node_id, "selector": selector })),
        )
        .await?;

    let node_id = query_result
        .get("nodeId")
        .and_then(Value::as_u64)
        .context("querySelector returned no nodeId")?;

    if node_id == 0 {
        bail!("No element found matching selector: {selector}");
    }

    Ok(query_result)
}

/// Convert HTML to plain text using a lightweight tag-stripping approach.
///
/// For richer extraction, the masterfetch extractor could be used, but this
/// keeps the browser module self-contained.
fn html_to_text(html: &str) -> String {
    // Simple tag stripping: remove everything between < and >.
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => text.push(ch),
            _ => {}
        }
    }

    // Collapse whitespace.
    let mut result = String::with_capacity(text.len());
    let mut prev_ws = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_ws {
                result.push(' ');
                prev_ws = true;
            }
        } else {
            result.push(ch);
            prev_ws = false;
        }
    }

    // Decode common HTML entities.
    result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

/// Map a key name to its `code` value for CDP `Input.dispatchKeyEvent`.
fn key_code_for(key: &str) -> &'static str {
    match key {
        "Enter" => "Enter",
        "Tab" => "Tab",
        "Escape" => "Escape",
        "Backspace" => "Backspace",
        "Delete" => "Delete",
        "ArrowUp" => "ArrowUp",
        "ArrowDown" => "ArrowDown",
        "ArrowLeft" => "ArrowLeft",
        "ArrowRight" => "ArrowRight",
        "Home" => "Home",
        "End" => "End",
        "PageUp" => "PageUp",
        "PageDown" => "PageDown",
        " " => "Space",
        _ => "Unidentified",
    }
}

/// Map a key name to its Windows virtual key code.
fn virtual_key_code_for(key: &str) -> u32 {
    match key {
        "Enter" => 0x0D,
        "Tab" => 0x09,
        "Escape" => 0x1B,
        "Backspace" => 0x08,
        "Delete" => 0x2E,
        "ArrowUp" => 0x26,
        "ArrowDown" => 0x28,
        "ArrowLeft" => 0x25,
        "ArrowRight" => 0x27,
        "Home" => 0x24,
        "End" => 0x23,
        "PageUp" => 0x21,
        "PageDown" => 0x22,
        " " => 0x20,
        _ => 0,
    }
}

/// Check the browser status by querying the HTTP discovery endpoints.
///
/// Returns version info and target count.
///
/// # Errors
///
/// Returns an error if the HTTP endpoints are unreachable.
pub async fn action_status_raw(http_endpoint: &str) -> Result<Value> {
    use super::cdp::{discover_version, list_targets};

    let version = discover_version(http_endpoint).await?;
    let targets = list_targets(http_endpoint).await?;
    let page_count = targets.iter().filter(|t| t.target_type == "page").count();

    Ok(json!({
        "browser": version.browser,
        "user_agent": version.user_agent,
        "web_socket_debugger_url": version.web_socket_debugger_url,
        "targets": targets.len(),
        "page_targets": page_count,
        "available": true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_text_strips_tags() {
        let html = "<html><body><h1>Title</h1><p>Hello &amp; world</p></body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Title"));
        assert!(text.contains("Hello & world"));
        assert!(!text.contains('<'));
    }

    #[test]
    fn test_html_to_text_collapses_whitespace() {
        let html = "<div>  multiple   spaces  </div>";
        let text = html_to_text(html);
        assert!(!text.contains("  "));
    }

    #[test]
    fn test_key_code_for_enter() {
        assert_eq!(key_code_for("Enter"), "Enter");
        assert_eq!(key_code_for("Tab"), "Tab");
        assert_eq!(key_code_for(" "), "Space");
    }

    #[test]
    fn test_virtual_key_code_for_enter() {
        assert_eq!(virtual_key_code_for("Enter"), 0x0D);
        assert_eq!(virtual_key_code_for("Tab"), 0x09);
        assert_eq!(virtual_key_code_for("Escape"), 0x1B);
    }
}
