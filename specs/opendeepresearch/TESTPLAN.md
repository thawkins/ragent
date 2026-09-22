---
status: draft
---
# TESTPLAN: Open Deep Research feature-gap integration into ragent

## Scope

This manual test plan verifies the requirements defined in `specs/opendeepresearch/SPEC.md`. It covers scope clarification, supervisor/researcher multi-agent mode, competitive analysis mode, the comparison-table format, webpage summarization and vault persistence, self-evaluation, TUI integration, and HTTP API integration.

## Test Environment

- A local ragent build running in a project directory with an LLM provider configured (e.g., Ollama for local runs or an API key for cloud providers).
- Network access so web search and fetch tools can retrieve live sources.
- A clean working directory where generated `RESEARCH.md` files will be written.

## Assumptions

- The build under test has completed tasks T-001 through T-018 from `PLAN.md`.
- The configured provider can answer short prompts within the default timeouts.
- `mf_search` and `mf_fetch` are enabled and reachable.

## Test Cases

### TC-001: Scope clarification asks one question before web searches

**Title:** Ambiguous scope triggers a single clarifying question.

**Requirements covered:** FR-005, FR-017.

**Preconditions:**
- ragent TUI or CLI is running and connected to a provider.
- No previous `/research` run is active in the same session.

**Steps:**
1. Enter or send the prompt: `/research "Research the inference market"`.
2. Wait for the agent's first response.

**Test data to enter:**
- Prompt: `/research "Research the inference market"`

**Expected result:**
- The system responds with exactly one clarifying question such as "Which market segment do you want to focus on — cloud LLM APIs, on-device inference, or hardware accelerators?".
- No web search tool calls appear in the log before the user answers the question.
- After a brief answer is supplied, the run proceeds to research.

---

### TC-002: Supervisor mode runs parallel researchers and synthesizes a report

**Title:** Supervisor/researcher graph mode produces a single synthesized report from parallel sub-researchers.

**Requirements covered:** FR-001, FR-007, FR-009.

**Preconditions:**
- T-005, T-006, T-007, and T-008 are implemented.
- A provider is configured.

**Steps:**
1. Enter or send the prompt: `/research --mode supervisor "Explain Rust async runtimes"`.
2. Wait for the run to complete.
3. Open the generated `RESEARCH.md`.

**Test data to enter:**
- Prompt: `/research --mode supervisor "Explain Rust async runtimes"`

**Expected result:**
- The system delegates multiple sub-topics (e.g., Tokio, async-std, smol, embassy) in parallel up to the configured concurrency limit.
- Each researcher returns compressed notes visible in the step log.
- The final `RESEARCH.md` contains a single coherent report that combines the sub-researcher findings.
- The report cites sources using the existing ragent citation format.

---

### TC-003: Competitive mode infers entities and produces per-entity profiles

**Title:** Competitive analysis mode identifies comparable entities and produces per-entity profiles.

**Requirements covered:** FR-001, FR-006, FR-009, FR-011.

**Preconditions:**
- T-009 and T-010 are implemented.
- A provider is configured.

**Steps:**
1. Enter or send the prompt: `/research --mode competitive "Compare Fireworks AI, Together.ai, and Groq for LLM inference"`.
2. Wait for the run to complete.
3. Open the generated `RESEARCH.md`.

**Test data to enter:**
- Prompt: `/research --mode competitive "Compare Fireworks AI, Together.ai, and Groq for LLM inference"`

**Expected result:**
- The system extracts three entities: Fireworks AI, Together.ai, and Groq.
- One parallel researcher is delegated per entity.
- The final report contains a dedicated per-entity profile section for each provider.
- The profiles include pricing, latency, model catalog, and API roughness indicators if publicly available.

---

### TC-004: Comparison-table format emits only the table and profiles

**Title:** `--format comparison-table` produces a compact artifact containing only the comparison table and entity profiles.

**Requirements covered:** FR-014, FR-016.

**Preconditions:**
- T-011 is implemented.

**Steps:**
1. Enter or send the prompt: `/research --mode competitive --format comparison-table "Compare Fireworks AI, Together.ai, and Groq for LLM inference"`.
2. Wait for the run to complete.
3. Open the generated `RESEARCH.md`.

**Test data to enter:**
- Prompt: `/research --mode competitive --format comparison-table "Compare Fireworks AI, Together.ai, and Groq for LLM inference"`

**Expected result:**
- The generated file is shorter than the full-report version of the same topic.
- It contains a Markdown comparison table with explicit comparison criteria as column headers (e.g., Pricing, Latency, Model Catalog, API Style).
- It contains per-entity profiles.
- It does not contain a long narrative synthesis beyond the table and profiles.

---

### TC-005: Per-page summarization persists sources to the vault with original URLs and timestamps

**Title:** Webpage summarization stores summarized sources in the research vault with original URLs and timestamps.

**Requirements covered:** FR-002, FR-003, FR-010, FR-018.

**Preconditions:**
- T-012 and T-013 are implemented.
- The research vault path is writable.

**Steps:**
1. Enter or send the prompt: `/research --mode supervisor --summarization-model openai:gpt-4.1-mini "Explain Rust async runtimes"`.
2. Wait for the run to complete.
3. Inspect the research vault for the run (either via the session log or the vault directory).

**Test data to enter:**
- Prompt: `/research --mode supervisor --summarization-model openai:gpt-4.1-mini "Explain Rust async runtimes"`

**Expected result:**
- Fetched webpages are summarized using the configured lightweight model before synthesis.
- Each summarized source is stored in the vault with a `url` field containing the original URL.
- Each summarized source includes a `summary_timestamp` or equivalent field.
- Citations in the final report reference the original URLs, not the summaries alone.

---

### TC-006: Self-evaluation appends a scorecard to the report

**Title:** `--evaluate` appends a quality scorecard with the five required dimensions.

**Requirements covered:** FR-008, FR-015, FR-019.

**Preconditions:**
- T-015 and T-016 are implemented.

**Steps:**
1. Enter or send the prompt: `/research --mode supervisor --evaluate "Explain Rust async runtimes"`.
2. Wait for the run to complete.
3. Open the generated `RESEARCH.md`.

**Test data to enter:**
- Prompt: `/research --mode supervisor --evaluate "Explain Rust async runtimes"`

**Expected result:**
- A scorecard is appended to the report frontmatter or endmatter.
- The scorecard contains scores for quality, relevance, groundedness, completeness, and structure.
- If the evaluator fails, the failure is logged and the scorecard shows an error note rather than being silently omitted.

---

### TC-007: TUI slash command supports new research flags

**Title:** The TUI `/research` slash-command completer exposes `--mode`, `--summarization-model`, and `--evaluate`.

**Requirements covered:** FR-001, FR-012, FR-014.

**Preconditions:**
- T-017 is implemented.
- The TUI is running.

**Steps:**
1. Open the TUI input prompt.
2. Type `/research --` and inspect the autocomplete suggestions.
3. Type `/research --mode ` and inspect the autocomplete suggestions.
4. Select `--mode competitive` and submit a topic.

**Test data to enter:**
- Input: `/research --mode competitive "Compare Fireworks AI, Together.ai, and Groq"`

**Expected result:**
- After `/research --`, suggestions include `--mode`, `--summarization-model`, and `--evaluate`.
- After `/research --mode `, suggestions include `tiered`, `supervisor`, and `competitive`.
- The selected competitive mode starts a research run and eventually produces a comparison report.

---

### TC-008: HTTP API accepts new research options

**Title:** The HTTP research endpoints accept `mode`, `summarization_model`, and `evaluate`.

**Requirements covered:** FR-001, FR-014, FR-015.

**Preconditions:**
- T-018 is implemented.
- The ragent HTTP server is running and authenticated.
- `curl` or an equivalent HTTP client is available.

**Steps:**
1. Send a POST request to the `/research` endpoint with the body:
   ```json
   {
     "query": "Compare Fireworks AI, Together.ai, and Groq for LLM inference",
     "mode": "competitive",
     "format": "comparison-table",
     "summarization_model": "openai:gpt-4.1-mini",
     "evaluate": true
   }
   ```
2. Follow the returned `Location` header or poll the status until completion.
3. Download and open the resulting `RESEARCH.md`.

**Test data to enter:**
- HTTP body as shown above.

**Expected result:**
- The server returns `202 Accepted` with a `Location` header pointing at the run status.
- The run completes without error.
- The resulting `RESEARCH.md` contains a comparison table, per-entity profiles, and a self-evaluation scorecard.
- The event stream includes step events for entity extraction, parallel researchers, and synthesis.

---

## Cleanup

After each test case:
1. Delete or archive the generated `RESEARCH.md` so the next test starts from a clean output state.
2. Optionally truncate the research vault directory used by the test if disk usage is a concern.
3. Close any extra TUI sessions or HTTP client connections.

## Sign-off

| Role | Name | Date | Result |
|------|------|------|--------|
| Tester | | | |
| Reviewer | | | |
