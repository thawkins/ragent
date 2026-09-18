# Tools — Web

Simple HTTP fetching and web search. For rich HTML-to-markdown extraction,
PDFs, bulk fetches, or envelope signals (`content_ok`, `is_official`,
`is_stale`), prefer the [MasterFetch](masterfetch.md) tools.

| Tool | Description |
|------|-------------|
| `webfetch` | Fetch a URL via HTTP GET; HTML converted to text. |
| `websearch` | Web search returning titles, URLs, snippets. |
| `http_request` | Full HTTP method/headers/body control. |

**Use cases:** fetching documentation pages, testing APIs, quick scraping.

---

## webfetch

Fetch the content of a URL via HTTP GET. HTML is converted to plain text
unless `format: "raw"` is requested.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `url` | string | yes | URL to fetch (HTTP/HTTPS only) | `"https://rust-lang.org"` |
| `format` | enum | no | `text` (HTML→text, default) or `raw` (unchanged) | `"text"` |
| `max_length` | integer | no | Maximum characters returned | `50000` |
| `timeout` | integer | no | Request timeout in seconds | `30` |

**Example:**
```text
webfetch url="https://rust-lang.org"
webfetch url="https://example.com/page.html" format="raw" max_length=10000
```

---

## websearch

Search the web and return results with titles, URLs, and snippets. Uses the
masterfetch multi-engine pipeline by default (keyless backends; optional
Tavily/LangSearch keys improve quality).

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `query` | string | yes | Search query | `"rust async patterns"` |
| `num_results` | integer | no | Number of results (default 5, max 20) | `10` |

**Example:**
```text
websearch query="tokio spawn_blocking" num_results=10
```

---

## http_request

Perform an HTTP request with full control over method, headers, and body.
Returns the status code, selected headers, and the body (truncated at 1 MiB).

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `url` | string | yes | Full URL including scheme | `"https://api.github.com/repos/rust-lang/rust"` |
| `method` | enum | no | `GET` (default), `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `OPTIONS` | `"GET"` |
| `headers` | object | no | Request headers as key/value map | `{"Authorization":"Bearer ..."}` |
| `body` | string | no | Request body (for `POST`/`PUT`/`PATCH`) | `"{\"name\":\"x\"}"` |
| `timeout` | integer | no | Timeout in seconds | `30` |

**Example:**
```text
http_request url="https://api.github.com/repos/rust-lang/rust" method="GET"
http_request url="https://httpbin.org/post" method="POST" body="{\"a\":1}"
```
