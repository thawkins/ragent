# Tools — Browser Automation

Chrome DevTools Protocol (CDP) automation. Requires a running Chrome/Chromium
launched with `--remote-debugging-port=9222`; use `action: "setup"` to launch
one automatically.

| Tool | Description |
|------|-------------|
| `browser` | CDP automation: open, snapshot, click, type, fill forms, screenshot, etc. |

**Use cases:** interacting with dynamic web pages, form filling, screenshots,
uploading files into web forms.

**Visibility switch:** `browser` (see `docs/howtos/tool-visibility.md`).

---

## browser

Drive a Chrome/Chromium instance via the DevTools protocol.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `action` | enum | yes | One of: `open`, `snapshot`, `click`, `type`, `fill_form`, `select`, `wait`, `eval`, `scroll`, `upload`, `press`, `screenshot`, `status`, `setup` | `"open"` |
| `url` | string | yes for `open` | URL to navigate to | `"https://example.com"` |
| `selector` | string | action-dependent | CSS selector for `click`, `type`, `select`, `upload`, `wait` | `"#submit"` |
| `text` | string | action-dependent | Text to type (`type`) or key to press (`press`) | `"Enter"` |
| `value` | string | no | Value for select option (`select`) | `"Option1"` |
| `fields` | object | no | Map of CSS selector to value for `fill_form` | `{"#user":"me","#pass":"pw"}` |
| `expression` | string | yes for `eval` | JavaScript expression to evaluate | `"document.title"` |
| `file_path` | string | yes for `upload` | Path of the file to upload | `"assets/logo.png"` |
| `condition` | enum | no | Wait mode: `load`, `selector`, `time`, `none` | `"load"` |
| `milliseconds` | integer | no | Wait duration when `condition="time"` | `500` |
| `scroll_x` / `scroll_y` | integer | no | Scroll offsets for `scroll` | `0` / `600` |
| `css_selector` | string | no | Narrow `snapshot` scope | `"main"` |
| `full_page` | boolean | no | `screenshot`: capture the full scrollable page | `false` |
| `wait` | boolean | no | `open`: wait for page load | `true` |
| `headless` | boolean | no | `setup`: run browser headlessly | `true` |
| `port` | integer | no | `setup`: CDP port | `9222` |

**Example:**
```text
browser action="setup"
browser action="open" url="https://example.com"
browser action="snapshot"
browser action="click" selector="#login-button"
browser action="type" selector="#username" text="alice@example.com"
browser action="screenshot" full_page=true
```
