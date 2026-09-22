---
status: draft
---

# OpenAlex Search Backend — Manual Test Plan

This is a **manual** test plan. It describes human-run verification steps
for the OpenAlex `mf_search` backend. It does **not** contain automated
test code.

## Prerequisites

1. A working build of ragent (`cargo build`).
2. Internet access (the OpenAlex API is at `https://api.openalex.org`).
3. No OpenAlex API key is required — the backend is keyless. An optional
   `OPENALEX_EMAIL` environment variable (or `openalex_email` config field)
   may be set to participate in the OpenAlex polite pool; this is
   recommended but not required for the tests below.
4. A shell with `curl` available, or the ragent TUI / CLI, to issue
   `mf_search` queries.
5. (Optional) `jq` for inspecting JSON responses in the terminal.

## Test Cases

### TC-001 — OpenAlex backend is always present in `mf_search`

**Title:** OpenAlex engine appears in the orchestrator without any
configuration.

**Preconditions:**
- ragent is built.
- No `OPENALEX_EMAIL`, `TAVILY_API_KEY`, `LANGSEARCH_API_KEY`, or
  `PERPLEXITY_API_KEY` environment variables are set.
- No `openalex_email` config field is set in `ragent.json`.

**Instructions:**
1. Open a terminal in the project root.
2. Run the ragent TUI: `ragent`.
3. At the prompt, type:
   ```
   mf_search query="machine learning" max_results=5
   ```
4. Press Enter to execute the tool call.

**Test data to enter:**
- Query: `machine learning`
- `max_results`: `5`

**Expected results:**
- The `mf_search` result metadata lists `openalex` among the engines used
  (e.g. `Engines: … (openalex) …`).
- At least one result has `source: openalex` and a scholarly title,
  publication year, or DOI visible in the snippet/URL.
- The result set is not empty.

---

### TC-002 — Scholarly metadata is present in OpenAlex results

**Title:** OpenAlex results surface academic metadata (DOI, publication
year, citation count, open-access URL).

**Preconditions:**
- ragent is built.
- Internet access is available.

**Instructions:**
1. Open a terminal in the project root.
2. Run: `ragent run "Use mf_search to search for 'graph neural networks' with max_results=3 and list each result's title, URL, and any scholarly metadata you can see in the snippet."`
3. Alternatively, use `curl` against a running server:
   ```
   curl -s -X POST http://localhost:9100/mf_search \
     -H "Content-Type: application/json" \
     -d '{"query":"graph neural networks","max_results":3}'
   ```

**Test data to enter:**
- Query: `graph neural networks`
- `max_results`: `3`

**Expected results:**
- At least one result originates from `openalex`.
- The OpenAlex result URL points to a scholarly landing page (e.g. an
  OpenAlex `https://openalex.org/W…` URI, a DOI `https://doi.org/…`, or a
  journal landing page).
- The snippet contains publication-year or citation-count information for
  the academic result.

---

### TC-003 — `site` filter scopes OpenAlex results to a publisher domain

**Title:** When a `site` filter is supplied, OpenAlex results are limited
to the specified publisher/repository.

**Preconditions:**
- ragent is built.
- Internet access is available.

**Instructions:**
1. Open a terminal in the project root.
2. Run the ragent TUI: `ragent`.
3. At the prompt, type:
   ```
   mf_search query="open access" site="nature.com" max_results=5
   ```
4. Press Enter.

**Test data to enter:**
- Query: `open access`
- `site`: `nature.com`
- `max_results`: `5`

**Expected results:**
- OpenAlex results (if any) reference the `nature.com` domain or a Nature
  journal source in their metadata/URL.
- Results from other engines may or may not honour the `site` filter; the
  test specifically checks that the OpenAlex-backed entries respect it.

---

### TC-004 — `freshness` filter translates to a publication-date range

**Title:** A `freshness=year` filter restricts OpenAlex results to works
published within the last 365 days.

**Preconditions:**
- ragent is built.
- Internet access is available.
- The current date is known (to verify the publication-year window).

**Instructions:**
1. Open a terminal in the project root.
2. Run the ragent TUI: `ragent`.
3. At the prompt, type:
   ```
   mf_search query="covid vaccine efficacy" freshness=year max_results=5
   ```
4. Press Enter.

**Test data to enter:**
- Query: `covid vaccine efficacy`
- `freshness`: `year`
- `max_results`: `5`

**Expected results:**
- OpenAlex results have a publication year equal to or later than
  `(current year − 1)`.
- No OpenAlex result has a publication year older than the freshness
  window.

---

### TC-005 — OpenAlex graceful degradation when the API is unreachable

**Title:** If the OpenAlex API returns an error or is blocked, `mf_search`
still returns results from the other backends.

**Preconditions:**
- ragent is built.
- A way to simulate an OpenAlex failure (e.g. temporarily block
  `api.openalex.org` via `/etc/hosts` pointing to `127.0.0.1`, or
  disconnect from the internet).

**Instructions:**
1. Simulate the OpenAlex outage (e.g. add `127.0.0.1 api.openalex.org`
   to `/etc/hosts`, or disable network).
2. Run the ragent TUI: `ragent`.
3. At the prompt, type:
   ```
   mf_search query="quantum computing" max_results=5
   ```
4. Press Enter.
5. Restore network/`/etc/hosts` after the test.

**Test data to enter:**
- Query: `quantum computing`
- `max_results`: `5`

**Expected results:**
- `mf_search` returns results (from DuckDuckGo and/or Brave).
- The metadata indicates `openalex` is `blocked` or absent from the
  contributing engines.
- The tool does not crash or return an empty result set solely due to the
  OpenAlex failure.

---

### TC-006 — Optional polite-pool email is sent and never logged in plain text

**Title:** When `OPENALEX_EMAIL` is set, the OpenAlex request includes a
`mailto=` parameter, and the email is masked in any diagnostics.

**Preconditions:**
- ragent is built.
- Internet access is available.
- Log level is set to show `debug`/`trace` output
  (e.g. `--log-level debug`).

**Instructions:**
1. Set the environment variable:
   ```
   export OPENALEX_EMAIL="tester@example.org"
   ```
2. Run the ragent TUI with debug logging:
   ```
   ragent --log-level debug
   ```
3. At the prompt, type:
   ```
   mf_search query="bibliometrics" max_results=3
   ```
4. Press Enter.
5. Inspect the debug logs for the OpenAlex request URL.

**Test data to enter:**
- Query: `bibliometrics`
- `max_results`: `3`
- `OPENALEX_EMAIL`: `tester@example.org`

**Expected results:**
- The debug log shows the OpenAlex request URL contains `mailto=…`.
- The email address is **masked** in the logs (e.g. `te****@example.org`
  or equivalent); the full plain-text email does not appear in any log
  line.
- `mf_search` returns OpenAlex results normally.

---

### TC-007 — `max_results` is clamped to OpenAlex's per-page limit

**Title:** Requesting more than 200 results does not cause an OpenAlex API
error; the engine clamps `per_page` to 200 and truncates to the requested
count.

**Preconditions:**
- ragent is built.
- Internet access is available.

**Instructions:**
1. Open a terminal in the project root.
2. Run the ragent TUI: `ragent`.
3. At the prompt, type:
   ```
   mf_search query="climate change" max_results=250
   ```
4. Press Enter.

**Test data to enter:**
- Query: `climate change`
- `max_results`: `250`

**Expected results:**
- No OpenAlex API error is reported (the engine clamps `per_page` to 200).
- The returned result set is capped at `max_results` (250) across all
  engines, with OpenAlex contributing up to 200 of them.
- The tool completes without a panic or `Err`.

## Cleanup

1. Unset any environment variables set for the tests:
   ```
   unset OPENALEX_EMAIL
   ```
2. Restore `/etc/hosts` if it was modified for TC-005 (remove the
   `127.0.0.1 api.openalex.org` entry).
3. Reconnect the network if it was disabled for TC-005.
4. Exit the ragent TUI / CLI session.