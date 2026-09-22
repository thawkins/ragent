---
status: draft
---

# Wikipedia Search Backend — Manual Test Plan

This is a **manual** test plan. It describes human-run verification steps
for the Wikipedia `mf_search` backend. It does **not** contain automated
test code.

## Prerequisites

1. A working build of ragent (`cargo build`).
2. Internet access (the Wikipedia REST API is at
   `https://en.wikipedia.org/api/rest_v1/` and the MediaWiki Action API
   is at `https://en.wikipedia.org/w/api.php`).
3. No Wikipedia API key is required — the backend is keyless. No
   environment variables or config fields are needed for the tests below.
4. A shell with `curl` available, or the ragent TUI / CLI, to issue
   `mf_search` queries.
5. (Optional) `jq` for inspecting JSON responses in the terminal.
6. Confirm the build includes the `wikipedia` engine:
   ```
   ./target/debug/ragent run "Run mf_search with engine=\"wikipedia\" query=\"Rust programming language\" max_results=2 and print the engines used and each result's title, URL, and snippet."
   ```
   The output should mention `wikipedia` among the engines.

## Test Cases

### TC-001 — Wikipedia backend is always present in `mf_search`

**Title:** Wikipedia engine appears in the orchestrator without any
configuration.

**Preconditions:**
- ragent is built.
- No `TAVILY_API_KEY`, `LANGSEARCH_API_KEY`, or `PERPLEXITY_API_KEY`
  environment variables are set.
- No `ragent.json` config file is required.

**Instructions:**
1. Open a terminal in the project root.
2. Run the ragent TUI:
   ```
   ./target/debug/ragent
   ```
3. At the prompt, type:
   ```
   mf_search query="Albert Einstein" max_results=5
   ```
4. Press **Enter** to execute the tool call.
5. Observe the tool output in the log panel.

**Test data to enter:**
- Query: `Albert Einstein`
- `max_results`: `5`

**Expected results:**
- The `mf_search` result metadata lists `wikipedia` among the engines
  used (e.g. `Engines: … (wikipedia) …`).
- At least one result has `source: wikipedia`.
- The Wikipedia result URL points to `https://en.wikipedia.org/wiki/…`.
- The result set is not empty.

---

### TC-002 — Encyclopedia extract and metadata are present in Wikipedia results

**Title:** Wikipedia results surface a plain-text extract, short
description, and canonical article URL.

**Preconditions:**
- ragent is built.
- Internet access is available.

**Instructions:**
1. Open a terminal in the project root.
2. Run a one-shot prompt:
   ```
   ./target/debug/ragent run "Use mf_search to search for 'photosynthesis' with max_results=3 and list each result's title, URL, and full snippet."
   ```
3. Alternatively, use `curl` against a running server:
   ```
   curl -s -X POST http://localhost:9100/mf_search \
     -H "Content-Type: application/json" \
     -d '{"query":"photosynthesis","max_results":3}'
   ```

**Test data to enter:**
- Query: `photosynthesis`
- `max_results`: `3`

**Expected results:**
- At least one result originates from `wikipedia`.
- The Wikipedia result URL is
  `https://en.wikipedia.org/wiki/Photosynthesis` (or a redirect/canonical
  variant).
- The snippet contains a concise plain-text extract describing
  photosynthesis (not raw HTML).
- The snippet begins with or contains the short description (e.g.
  "Biological process") when the Wikipedia page provides one.

---

### TC-003 — `engine="wikipedia"` restricts the search to the Wikipedia backend

**Title:** When the `engine` parameter is set to `wikipedia`, only
Wikipedia results are returned.

**Preconditions:**
- ragent is built.
- Internet access is available.

**Instructions:**
1. Open a terminal in the project root.
2. Run a one-shot prompt:
   ```
   ./target/debug/ragent run "Run mf_search with engine=\"wikipedia\" query=\"quantum entanglement\" max_results=3 and print the source of each result."
   ```
3. Observe the result sources.

**Test data to enter:**
- `engine`: `wikipedia`
- Query: `quantum entanglement`
- `max_results`: `3`

**Expected results:**
- Every returned result has `source: wikipedia`.
- No results from `duckduckgo`, `brave`, `openalex`, or other backends
  appear.
- At least one result is returned (Wikipedia has an article on the
  topic).

---

### TC-004 — Ambiguous query resolves to the best-matching Wikipedia page

**Title:** A common-word or ambiguous query resolves to the most
relevant Wikipedia page title via the Action API search step.

**Preconditions:**
- ragent is built.
- Internet access is available.

**Instructions:**
1. Open a terminal in the project root.
2. Run a one-shot prompt:
   ```
   ./target/debug/ragent run "Run mf_search with engine=\"wikipedia\" query=\"jaguar\" max_results=3 and print each result's title and URL."
   ```
3. Observe the result titles and URLs.

**Test data to enter:**
- `engine`: `wikipedia`
- Query: `jaguar`
- `max_results`: `3`

**Expected results:**
- The top result is the Wikipedia article for "Jaguar" (the animal or
  the disambiguation/primary topic, as ranked by the MediaWiki search).
- Each result URL is of the form
  `https://en.wikipedia.org/wiki/<Title>`.
- No result is empty or missing a title.

---

### TC-005 — Wikipedia graceful degradation when the API is unreachable

**Title:** When the Wikipedia API returns an error or is unreachable,
the remaining `mf_search` backends still return results.

**Preconditions:**
- ragent is built.
- The Wikipedia host can be made unreachable (e.g. by disconnecting the
  network, adding a firewall rule, or pointing DNS to an invalid host in
  a test build).
- Internet access to DuckDuckGo / Brave / OpenAlex is available.

**Instructions:**
1. Temporarily block or break access to `en.wikipedia.org`.
2. Open a terminal in the project root.
3. Run a one-shot prompt:
   ```
   ./target/debug/ragent run "Run mf_search with query=\"artificial intelligence\" max_results=5 and print the engines used and any engine-blocked signals."
   ```
4. Observe the result metadata.
5. Restore access to `en.wikipedia.org`.

**Test data to enter:**
- Query: `artificial intelligence`
- `max_results`: `5`

**Expected results:**
- The `wikipedia` engine is reported as `blocked` (or with an error
  message) in the engine metadata.
- The overall result set is not empty — results from DuckDuckGo,
  Brave, and/or OpenAlex still appear.
- No panic, no `unwrap` error, no `Err` propagation to the agent.

---

### TC-006 — A descriptive User-Agent is sent (no HTTP 403)

**Title:** The Wikipedia backend includes a descriptive `User-Agent`
header so requests are not rejected with HTTP 403.

**Preconditions:**
- ragent is built.
- Internet access is available.
- (Optional) A network inspection tool (`tcpdump`, `mitmproxy`, or a
  logging proxy) to observe outbound headers.

**Instructions:**
1. (Optional) Start a logging proxy or packet capture targeting
   `en.wikipedia.org:443`.
2. Open a terminal in the project root.
3. Run a one-shot prompt:
   ```
   ./target/debug/ragent run "Run mf_search with engine=\"wikipedia\" query=\"Rust programming language\" max_results=2."
   ```
4. Observe the result set and (if capturing) the outbound request
   headers.

**Test data to enter:**
- `engine`: `wikipedia`
- Query: `Rust programming language`
- `max_results`: `2`

**Expected results:**
- The Wikipedia result for "Rust (programming language)" is returned
  successfully (HTTP 200 path).
- No `blocked: 403` or `blocked: missing user-agent` signal appears in
  the engine metadata.
- (If capturing) the outbound `User-Agent` header is a descriptive
  string identifying ragent (e.g. `ragent/<version> (…)`), not a default
  or empty value.

---

### TC-007 — `max_results` caps the number of Wikipedia summaries fetched

**Title:** The backend fetches at most `max_results` summaries and
returns at most that many Wikipedia results.

**Preconditions:**
- ragent is built.
- Internet access is available.

**Instructions:**
1. Open a terminal in the project root.
2. Run a one-shot prompt:
   ```
   ./target/debug/ragent run "Run mf_search with engine=\"wikipedia\" query=\"computer science\" max_results=4 and print the count of wikipedia results returned."
   ```
3. Observe the number of Wikipedia results.

**Test data to enter:**
- `engine`: `wikipedia`
- Query: `computer science`
- `max_results`: `4`

**Expected results:**
- The number of `source: wikipedia` results is ≤ 4.
- No more than 4 summaries were fetched (observable via logs if debug
  logging is enabled).

## Cleanup

1. Restore network access to `en.wikipedia.org` if TC-005 blocked it.
2. Stop any logging proxy / packet capture started for TC-006.
3. Remove any temporary firewall rules added for the tests.
4. No ragent configuration files were created or modified by these
   tests; no further teardown is required.