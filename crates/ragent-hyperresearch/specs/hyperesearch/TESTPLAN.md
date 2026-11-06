---
status: draft
spec_id: hyperesearch
---

# Manual Test Plan — hyperresearch Deep-Research Engine

Manual, human-executable verification of the `ragent-hyperresearch` crate. No
automated test code is included here; every case is run by hand and judged
against its expected results.

## Prerequisites

1. **Toolchain** — Rust 1.85 or later (`rustc --version`), with `cargo` and
   `cargo-nextest` on `PATH`.
2. **Build the crate** — from the workspace root:

   ```
   cargo build -p ragent-hyperresearch
   ```

   The binary is at `target/debug/hyperresearch`. All commands below assume it
   is invoked as `./target/debug/hyperresearch` from the workspace root, or that
   `target/debug` is on `PATH` and the command is `hyperresearch`.
3. **Scratch vault** — every case works in a disposable vault, never the
   repository:

   ```
   mkdir -p target/temp/hr-manual/vault
   export HR_VAULT="$PWD/target/temp/hr-manual/vault"
   ```

4. **Offline-first configuration** — create
   `target/temp/hr-manual/config.toml` with:

   ```toml
   [vault]
   root = "target/temp/hr-manual/vault"

   [fetch]
   timeout_secs = 20
   max_body_bytes = 2000000
   respect_robots = true
   providers = ["builtin"]

   [research]
   profile = "standard"
   ```

   Point the CLI at it with `export HR_CONFIG="$PWD/target/temp/hr-manual/config.toml"`.
   Provider-credential cases are marked optional and require the stated key.
5. **Local sample files** — create these under `target/temp/hr-manual/`:

   - `sample.html` — a small HTML page whose body contains the text
     `<script>alert('xss')</script>` and the words `Sample research note body`.
   - `sample.pdf` — any 1–2 page text PDF (a printed-to-PDF document is fine).
   - `junk.html` — a page whose body is only `Access denied. Enable JavaScript.`
   - `notes/frontmatter.md` — a note with hand-written frontmatter using single
     quotes, keys in the order `id`, `title`, `tags`, and no trailing blank line.
6. **Optional provider keys** — for the credentialed cases set only the
   variable each case names (`EXA_API_KEY`, `TAVILY_API_KEY`, `SERPLY_API_KEY`).
   Cases that need one are marked **Optional**; skip them if no key is available.

## Test Cases

### TC-001: Note creation, listing, and byte-identical frontmatter

**Preconditions:** Prerequisites 1–5 complete; `HR_VAULT` and `HR_CONFIG` are
exported.

**Steps:**
1. Create a note:
   ```
   hyperresearch note add --id sample-note --title "Sample Research Note" --tag biology --tag methods
   ```
2. List the vault:
   ```
   hyperresearch note list
   ```
3. Read the created file directly:
   ```
   cat "$HR_VAULT/sample-note.md"
   ```
4. Copy the fixture note into the vault and re-render it unchanged:
   ```
   cp target/temp/hr-manual/notes/frontmatter.md "$HR_VAULT/frontmatter.md"
   hyperresearch index
   cat "$HR_VAULT/frontmatter.md"
   ```

**Test data:** id `sample-note`; title `Sample Research Note`; tags `biology`,
`methods`.

**Expected results:**
- Step 1 exits 0 and prints the created note path.
- Step 2 lists `sample-note` with its title and both tags.
- Step 3 shows YAML frontmatter delimited by `---` containing `id`,
  `title`, and `tags` followed by the Markdown body placeholder.
- Step 4 leaves `frontmatter.md` byte-identical to the fixture: same key order,
  same single-quoted strings, no added trailing blank line. Verify with
  `diff target/temp/hr-manual/notes/frontmatter.md "$HR_VAULT/frontmatter.md"`,
  which prints nothing.

### TC-002: Note id collision is refused, never overwritten

**Preconditions:** TC-001 complete, so `$HR_VAULT/sample-note.md` exists.

**Steps:**
1. Attempt to create a second note with the same id:
   ```
   hyperresearch note add --id sample-note --title "Different Title"
   ```
2. Print the exit status:
   ```
   echo "exit=$?"
   ```
3. Confirm the original note is untouched:
   ```
   head -5 "$HR_VAULT/sample-note.md"
   ```

**Test data:** id `sample-note`; title `Different Title`.

**Expected results:**
- Step 1 prints an error naming `sample-note` and the existing path
  `$HR_VAULT/sample-note.md`, and suggests choosing another id.
- Step 2 prints `exit=1` (any non-zero value).
- Step 3 still shows title `Sample Research Note`; the file was not modified and
  no `sample-note-1.md` was created. Confirm with
  `ls "$HR_VAULT"` — only one file matches `sample-note*`.

### TC-003: Bad input never panics

**Preconditions:** Prerequisites complete.

**Steps:**
1. Run note add with no arguments:
   ```
   hyperresearch note add
   echo "exit=$?"
   ```
2. Run a fetch with a syntactically invalid URL:
   ```
   hyperresearch fetch "ht!tp://not a url"
   echo "exit=$?"
   ```
3. Run an unknown subcommand:
   ```
   hyperresearch frobnicate
   echo "exit=$?"
   ```
4. Run search with a note id that does not exist:
   ```
   hyperresearch search --note does-not-exist
   echo "exit=$?"
   ```

**Test data:** as shown in each command.

**Expected results:**
- Every command prints a human-readable error and a usage hint where
  applicable.
- Every `echo "exit=$?"` prints a non-zero value.
- No output contains `panicked at`, `RUST_BACKTRACE`, or a thread-name panic
  trace.

### TC-004: Fetch pipeline with junk detection and PDF lane

**Preconditions:** Prerequisites complete, including `sample.html`,
`sample.pdf`, and `junk.html`.

**Steps:**
1. Fetch the local HTML sample:
   ```
   hyperresearch fetch "file://$PWD/target/temp/hr-manual/sample.html" --note fetch-html
   ```
2. Inspect the stored note body:
   ```
   sed -n '1,40p' "$HR_VAULT/fetch-html.md"
   ```
3. Fetch the junk page:
   ```
   hyperresearch fetch "file://$PWD/target/temp/hr-manual/junk.html" --note fetch-junk
   echo "exit=$?"
   ```
4. Fetch the PDF:
   ```
   hyperresearch fetch "file://$PWD/target/temp/hr-manual/sample.pdf" --note fetch-pdf
   ```
5. Inspect the PDF note:
   ```
   sed -n '1,40p' "$HR_VAULT/fetch-pdf.md"
   ```

**Test data:** the three local sample files above.

**Expected results:**
- Step 1 exits 0 and reports a successful fetch.
- Step 2 shows extracted text including `Sample research note body`, and the
  script text appears only as escaped or fenced content — no live
  `<script>alert('xss')</script>` tag reaches the body unfenced.
- Step 3 reports a junk-content outcome in its own wording (for example
  `junk` or `no usable content`), does not store a body of only
  `Access denied. Enable JavaScript.`, and exits non-zero.
- Step 4 exits 0 and reports the PDF lane was used.
- Step 5 shows extracted PDF text in the note body, with the source recorded in
  frontmatter.

### TC-005: Safe HTTP — certificate refusal and final-URL recheck

**Preconditions:** Prerequisites complete; a network connection is available for
the first two steps. For step 3, a host in `/etc/hosts` that maps a throwaway
name to `127.0.0.1` is set up and a local server is not running.

**Steps:**
1. Fetch a site with a deliberately broken certificate chain:
   ```
   hyperresearch fetch "https://self-signed.badssl.com/" --note tls-test
   echo "exit=$?"
   ```
2. Fetch a well-formed HTTPS page:
   ```
   hyperresearch fetch "https://example.com/" --note tls-ok
   echo "exit=$?"
   ```
3. Fetch `http://<throwaway-host>/` (the `127.0.0.1` mapping). The connection is
   refused; observe both the failure and the recorded attempt.

**Test data:** URLs as listed in each step.

**Expected results:**
- Step 1 fails with a certificate/TLS validation error and exits non-zero. It
  does not fetch the page, and there is no note `tls-test.md` created with a
  body.
- Step 2 succeeds and stores a note.
- Step 3 fails with a transport/connection error naming the host, exits
  non-zero, and no partial note body is written.
- In no case does the tool run with certificate verification disabled or print
  a message indicating verification was bypassed.

### TC-006: Batch fetch continues past a failing URL

**Preconditions:** Prerequisites complete; `sample.html` and `sample.pdf` exist.
The batch file below mixes two local files with one unreachable host.

**Steps:**
1. Create `target/temp/hr-manual/batch.txt` with exactly these three lines:
   ```
   file:///ABSOLUTE/PATH/target/temp/hr-manual/sample.html
   http://127.0.0.1:9/unreachable
   file:///ABSOLUTE/PATH/target/temp/hr-manual/sample.pdf
   ```
   Replace `ABSOLUTE/PATH` with the real absolute path to the workspace.
2. Run the batch:
   ```
   hyperresearch fetch-batch --file target/temp/hr-manual/batch.txt
   echo "exit=$?"
   ```
3. List the resulting notes:
   ```
   hyperresearch note list
   ```

**Test data:** the three URLs in `batch.txt`.

**Expected results:**
- Step 2 prints a per-URL result table: two successes and one failure naming
  `http://127.0.0.1:9/unreachable` with its cause.
- The batch does not stop at the failing URL; the third URL is still attempted.
- The final summary reports `2` succeeded and `1` failed.
- Step 3 lists notes for both local files; no note exists for the unreachable
  host.

### TC-007: Search and evidence filters

**Preconditions:** TC-001 and TC-004 complete, so notes `sample-note`,
`fetch-html`, and `fetch-pdf` exist in the vault. Run `hyperresearch index` once
first.

**Steps:**
1. Search the vault:
   ```
   hyperresearch search "sample research"
   ```
2. Search with an evidence filter that should exclude the HTML note:
   ```
   hyperresearch search "sample research" --min-quality 0.8 --year-from 2020
   ```
3. Search for a term that appears nowhere:
   ```
   hyperresearch search "zzzznotpresent"
   echo "exit=$?"
   ```

**Test data:** query `sample research`; filters `--min-quality 0.8`,
`--year-from 2020`; query `zzzznotpresent`.

**Expected results:**
- Step 1 returns hits for the notes containing `sample` and `research`, each
  hit showing the note id, title, and a highlighted snippet.
- Step 2 returns only hits whose recorded quality and publication year satisfy
  the filters; the HTML note (which has no publication year) is excluded and the
  output states the filters that were applied.
- Step 3 prints an empty-result message (not an error trace) and exits 0 or
  non-zero consistently with the documented empty-result behaviour.

### TC-008: Citation check reports unresolved, uncited, and unreachable citations

**Preconditions:** TC-001 and TC-004 complete. Create a report note with three
citations: one to an existing note, one to a non-existent note, and one to an
existing note whose stored source URL is unreachable.

**Steps:**
1. Create the report:
   ```
   hyperresearch note add --id report-cites --title "Citation Report"
   ```
2. Open `$HR_VAULT/report-cites.md` in an editor and, in the body, add exactly:
   ```
   Finding A [sample-note]. Finding B [missing-note]. Finding C [fetch-html].
   ```
   Save the file.
3. Run the citation check:
   ```
   hyperresearch citecheck --report report-cites
   ```
4. Run it again with JSON output:
   ```
   hyperresearch citecheck --report report-cites --format json
   ```

**Test data:** the three citation keys `sample-note`, `missing-note`,
`fetch-html`.

**Expected results:**
- Step 3 prints three sections: unresolved citations listing `missing-note`;
  uncited notes listing any vault note cited by nothing (including
  `report-cites` if nothing cites it); and unreachable sources listing
  `fetch-html` when its stored URL cannot be reached.
- `sample-note` appears in no problem section because it resolves.
- Step 4 emits valid JSON with the same three categories and the same entries.
  Pipe it through a JSON formatter to confirm it parses.

### TC-009: Graph edges and centrality ranking

**Preconditions:** TC-008 complete, so `report-cites` cites `sample-note` and
`fetch-html`.

**Steps:**
1. Build the graph:
   ```
   hyperresearch graph build
   ```
2. List edges of the citation kind:
   ```
   hyperresearch graph list --kind cites
   ```
3. Rank notes by centrality:
   ```
   hyperresearch graph rank --top 5
   ```

**Test data:** none beyond the existing notes.

**Expected results:**
- Step 1 reports the number of nodes and edges created.
- Step 2 lists `report-cites → sample-note` and `report-cites → fetch-html` as
  `cites` edges, each row naming both endpoints and the edge kind.
- Step 3 prints at most five notes ordered by descending centrality, with the
  centrality value beside each id.

### TC-010: MCP server exposes tools and rejects malformed arguments

**Preconditions:** Prerequisites complete; a JSON-RPC client is available (for
example piping literal lines into the process).

**Steps:**
1. Send an initialize plus a tools/list request:
   ```
   printf '%s\n%s\n' \
     '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"manual","version":"0"}}}' \
     '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
     | hyperresearch mcp
   ```
2. Send a valid `create_note` call:
   ```
   printf '%s\n' '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_note","arguments":{"id":"mcp-note","title":"MCP Note","body":"Created over MCP"}}}' | hyperresearch mcp
   ```
3. Send a `create_note` call with a missing required argument:
   ```
   printf '%s\n' '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"create_note","arguments":{"title":"No Id"}}}' | hyperresearch mcp
   ```
4. Send a call naming a tool that does not exist:
   ```
   printf '%s\n' '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"no_such_tool","arguments":{}}}' | hyperresearch mcp
   ```

**Test data:** the literal JSON-RPC lines above.

**Expected results:**
- Step 1 returns an `initialize` result naming the server, then a `tools/list`
  result whose array includes at least `fetch_url`, `update_note`, `search`,
  `read_note`, and `create_note`, each with an input schema.
- Step 2 returns a success result for `create_note`, and
  `$HR_VAULT/mcp-note.md` now exists.
- Step 3 returns a JSON-RPC error result naming the missing `id` argument; the
  server process stays alive and no note is created.
- Step 4 returns a JSON-RPC error naming `no_such_tool`; the process stays alive
  and no panic trace appears on stderr.

### TC-011: HTTP serving escapes untrusted and XSS content

**Preconditions:** TC-001 and TC-004 complete, so `fetch-html.md` contains the
script text from `sample.html`.

**Steps:**
1. Start the server in the background:
   ```
   hyperresearch serve --bind 127.0.0.1:8099 &
   ```
2. Fetch the note page:
   ```
   curl -s http://127.0.0.1:8099/note/fetch-html
   ```
3. Fetch the index page:
   ```
   curl -s http://127.0.0.1:8099/
   ```
4. Search the returned HTML for a live script tag:
   ```
   curl -s http://127.0.0.1:8099/note/fetch-html | grep -c "<script>alert"
   ```
5. Stop the server:
   ```
   kill %1
   ```

**Test data:** the URLs above.

**Expected results:**
- Step 2 returns HTTP 200 HTML containing the note title and escaped body text.
- Step 3 returns an index page listing the note ids as links.
- Step 4 prints `0` — the payload appears as escaped text
  (`&lt;script&gt;alert...`), never as an executable `<script>` tag.
- Step 5 stops the server; a subsequent `curl` fails to connect.
- Requesting `/note/does-not-exist` returns a 404 page, not a stack trace.

### TC-012: Repair a drifted index

**Preconditions:** Prerequisites complete and at least one note exists.

**Steps:**
1. Delete the index state to simulate drift:
   ```
   rm -f "$HR_VAULT/.index.sqlite" "$HR_VAULT/INDEX.md"
   ```
2. Run the lint command:
   ```
   hyperresearch lint
   echo "exit=$?"
   ```
3. Run repair:
   ```
   hyperresearch repair
   ```
4. Run lint again:
   ```
   hyperresearch lint
   echo "exit=$?"
   ```

**Test data:** none.

**Expected results:**
- Step 2 reports the vault index is missing or drifted and exits non-zero.
- Step 3 rebuilds the index, printing each repaired item (at minimum the index
  build and `INDEX.md` regeneration).
- Step 4 reports a clean vault and exits 0.
- `$HR_VAULT/INDEX.md` exists and lists every note id with its title and
  last-updated timestamp, grouped by topic.

### TC-013: Embedded skill pipeline and golden stability

**Preconditions:** Prerequisites complete.

**Steps:**
1. List the embedded skills:
   ```
   hyperresearch template list --kind skill
   ```
2. Print the decompose stage:
   ```
   hyperresearch template show hyperresearch-1-decompose
   ```
3. Print the umbrella skill:
   ```
   hyperresearch template show hyperresearch
   ```
4. Compare the human-facing skill file with the published copy:
   ```
   head -20 crates/ragent-hyperresearch/skills/deep-research/SKILL.md
   ```

**Test data:** skill names `hyperresearch-1-decompose` and `hyperresearch`.

**Expected results:**
- Step 1 lists all 18 numbered stages by name, from
  `hyperresearch-1-decompose` through
  `hyperresearch-18-*`-equivalent names, plus the umbrella `hyperresearch`
  skill.
- Step 2 prints the decompose instructions verbatim, with no template
  placeholders left unsubstituted.
- Step 3 prints the umbrella skill describing the collect, search, synthesise
  loop.
- Step 4 shows `skills/deep-research/SKILL.md` declaring `name` and
  `description` in its frontmatter, matching the umbrella skill's text.

### TC-014: Export a report in Markdown and JSON

**Preconditions:** TC-008 complete, so `report-cites` exists with citations.

**Steps:**
1. Export as Markdown:
   ```
   hyperresearch export --note report-cites --format markdown --out target/temp/hr-manual/report.md
   ```
2. Read the output:
   ```
   head -40 target/temp/hr-manual/report.md
   ```
3. Export as JSON:
   ```
   hyperresearch export --note report-cites --format json --out target/temp/hr-manual/report.json
   ```
4. Validate the JSON:
   ```
   python3 -m json.tool target/temp/hr-manual/report.json > /dev/null && echo valid
   ```

**Test data:** note id `report-cites`; formats `markdown` and `json`.

**Expected results:**
- Step 1 exits 0 and names the written path.
- Step 2 shows the note title as a heading, the body, and a references section
  listing `sample-note` and `fetch-html`.
- Step 3 exits 0 and writes the file.
- Step 4 prints `valid`; the JSON contains a `notes` array, a `citations` array,
  and an `runs` or `metadata` object.

### TC-015: Research run records its profile and effort controls

**Preconditions:** Prerequisites complete. Optional: a provider key is
configured. This case is **Optional** if no research provider is available; in
that case run step 1 only and verify the refusal path.

**Steps:**
1. Start a small research run with an explicit effort setting:
   ```
   hyperresearch research "effects of sleep deprivation on memory consolidation" --profile dissertation --width 3
   echo "exit=$?"
   ```
2. Inspect the recorded run:
   ```
   hyperresearch config show --section runs
   ```
3. Request an unsupported profile:
   ```
   hyperresearch research "any topic" --profile no-such-profile
   echo "exit=$?"
   ```

**Test data:** query text as shown; profile `dissertation`; width `3`; invalid
profile `no-such-profile`.

**Expected results:**
- Step 1 runs, streams progress per stage, and exits 0 (or, without a provider,
  exits non-zero with a clear "no provider configured" message and writes no
  partial run record).
- Step 2 lists a run record containing the query, `profile = dissertation`,
  `width = 3`, the providers used, and counts of sources fetched and claims
  extracted.
- Step 3 exits non-zero and lists the available profiles, without starting a
  run.

## Cleanup

1. Stop any server still running from TC-011:
   ```
   jobs
   kill %1 2>/dev/null || true
   ```
2. Unset the manual-test environment variables:
   ```
   unset HR_VAULT HR_CONFIG
   ```
3. Remove the scratch tree:
   ```
   rm -rf target/temp/hr-manual
   ```
4. If any provider key was exported for the optional cases, unset it:
   ```
   unset EXA_API_KEY TAVILY_API_KEY SERPLY_API_KEY
   ```
5. Confirm the repository is clean, proving the manual tests wrote nothing into
   the checked-in tree:
   ```
   git status --porcelain
   ```
   The only permissible entries are files that existed before testing began.
