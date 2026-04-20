---
title: "Defensive Web Scraping"
type: concept
generated: "2026-04-19T19:53:40.458711088+00:00"
---

# Defensive Web Scraping

### From: webfetch

Defensive web scraping encompasses programming practices that anticipate and gracefully handle the diverse failure modes of automated web content retrieval. Unlike browser-based interaction where users can adapt to errors, automated tools must implement comprehensive safeguards against network failures, malformed content, security restrictions, and resource exhaustion. WebFetchTool demonstrates several defensive patterns: URL scheme validation, timeout enforcement, redirect limiting, response size management, and parsing fallback strategies.

The security model begins with input validation at the boundary. WebFetchTool explicitly rejects non-HTTP/HTTPS URLs, preventing potential file system access through `file://` schemes or unexpected protocol handlers. This scheme whitelist approach is a fundamental security control for URL-based tools. The implementation then configures the HTTP client with resource limits: a 30-second timeout prevents indefinite blocking on slow or hung connections, while the 5-redirect limit prevents unbounded chains that could indicate loops or tracking systems. These limits protect both the agent's responsiveness and the target server's resources.

Content handling implements multiple layers of defense. The success status check ensures only valid HTTP responses proceed to processing, with clear error messages incorporating canonical status descriptions. Body reading includes context for failure attribution. The content length limit (50,000 characters default, configurable) prevents memory exhaustion from unexpectedly large responses—a real concern with modern web applications generating megabytes of JavaScript-heavy HTML. Processing includes character-boundary-aware truncation to avoid string corruption. Finally, the HTML parsing pipeline provides graceful degradation: if the sophisticated html2text library fails, the simple tag stripper produces usable output. This defense-in-depth ensures the tool remains functional across the wide spectrum of web content quality.

## External Resources

- [HTTP response status codes reference](https://developer.mozilla.org/en-US/docs/Web/HTTP/Status) - HTTP response status codes reference
- [RFC 7231 HTTP redirect handling specification](https://datatracker.ietf.org/doc/html/rfc7231#section-6.4) - RFC 7231 HTTP redirect handling specification

## Sources

- [webfetch](../sources/webfetch.md)
