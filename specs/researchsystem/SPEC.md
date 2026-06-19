---
status: draft
audit:
  - { time: 1781907407, from: "none", to: "draft", actor: "system" }
---
# Specification: Research System

## Executive Summary

This document specifies a **Research System** for ragent that allows users to gather, organize, and persist research on any topic directly from the TUI. The system provides a `/research` slash command (modeled after the existing `/spec` system), creates a structured `research/<topic>/` directory hierarchy for each research item, gathers information via web search and/or local file reading, cross-references findings with the current project's resources, and produces a self-contained `RESEARCH.md` document with an embedded references index. The Research System is designed to be the first step in the ragent workflow: research the problem space, then write a spec, then implement.

## Scope & Objectives

### Scope

The Research System covers:
- **Slash command** — `/research <topic>` (and supporting sub-commands: `list`, `open`, `search`, `delete`) in the TUI, and equivalent CLI sub-commands under `ragent research`
- **Directory conventions** — standardised `research/<research-name>/` folder structure parallel to `specs/`
- **Information gathering** — web search (via existing `websearch` tool) and local file reading (via existing `read`, `grep`, `glob` tools) orchestrated into a research session
- **Cross-referencing** — automatic identification of in-project files, types, modules, and prior specs relevant to the topic
- **Output document** — a single `RESEARCH.md` containing the gathered content plus a references index block that lists every supporting file/web source with a one-line summary
- **Supporting files** — raw search results, downloaded pages, and excerpts stored as siblings of `RESEARCH.md` for auditability
- **Naming** — research names are URL-safe, lowercase, hyphen-separated, and must be unique

### Out of Scope

- AI-driven synthesis of gathered information (the LLM agent already performs synthesis; the Research System just structures inputs and outputs)
- Real-time collaborative editing of research items
- Publishing research to external knowledge bases
- Citation/bibliography management (BibTeX, CSL) — the references index is a lightweight markdown list
- Versioning of research items beyond what VCS provides automatically
- Automatic re-runs of research on a schedule

### Objectives

1. Provide a low-friction way to start any ragent session with structured, reproducible research
2. Eliminate ad-hoc "I read 10 tabs and lost the links" workflows by persisting every source
3. Make prior research discoverable via `list` and `search` sub-commands
4. Enable downstream spec creation that links back to the research that informed it
5. Keep research items self-contained so they can be shared, archived, or opened months later

---

## Functional Requirements

### FR-001 — Research Directory Structure

The ragent Research System shall enforce a standard directory layout for research items under `research/` at the project root.

`The <ragent Research System> shall <enforce that every research item resides in a subdirectory of research/ named after a URL-safe research-name, containing a RESEARCH.md file>.`

`When <a research item is created>, the <ragent Research System> shall <generate the directory research/<research-name>/, write an empty RESEARCH.md skeleton, and register the item in the global research index>.`

`If <a directory research/<name> already exists>, the <ragent Research System> shall <refuse to create a duplicate and suggest /research open <name> instead>.`

### FR-002 — Research Name Validation

`The <ragent Research System> shall <validate that every research-name consists only of lowercase ASCII letters, digits, and hyphens, starts with a letter, and is between 3 and 64 characters long>.`

`While <a research-name is being entered by the user>, the <ragent Research System> shall <provide live feedback indicating validity and uniqueness>.`

`If <the user supplies an invalid research-name>, the <ragent Research System> shall <reject the input and display the validation rule that was violated>.`

### FR-003 — `/research` Slash Command

`The <ragent Research System> shall <register a /research slash command in the TUI, parallel to the existing /spec slash command>.`

`When <the user types "/research <research-name> <topic description>">, the <ragent Research System> shall <create the research directory, launch an information-gathering session scoped to the topic, and write the resulting RESEARCH.md before returning control to the prompt>.`

`Where <the user provides only "/research" with no arguments>, the <ragent Research System> shall <display the research help text listing all sub-commands and their arguments>.`

### FR-004 — Research Sub-Commands

`The <ragent Research System> shall <support the following sub-commands after the base /research command: list, open <name>, search <query>, show <name>, delete <name>>.`

`When <the user runs "/research list">, the <ragent Research System> shall <display a tabular summary of all research items in research/, sorted by last-modified descending, showing name, title, source count, and last-modified date>.`

`When <the user runs "/research open <name>">, the <ragent Research System> shall <open the RESEARCH.md of the named item in the TUI's markdown viewer>.`

`When <the user runs "/research search <query>">, the <ragent Research System> shall <perform a full-text search across all RESEARCH.md files in research/ and display matching snippets with file paths>.`

`When <the user runs "/research show <name>">, the <ragent Research System> shall <display the metadata of the named research item: name, title, created, modified, source list, and references index summary>.`

`If <the user runs "/research delete <name>">, the <ragent Research System> shall <prompt for confirmation and, on approval, remove the research/<name> directory and unregister the item from the index>.`

### FR-005 — Research Title and Metadata

`The <ragent Research System> shall <record a human-readable title for each research item, derived from the topic description or supplied via "/research <name> --title <title>">.`

`When <a research item is created>, the <ragent Research System> shall <write a YAML frontmatter block at the top of RESEARCH.md containing at least: name, title, created (ISO 8601 UTC), modified (ISO 8601 UTC), and status fields>.`

`While <a research item is being updated>, the <ragent Research System> shall <update the modified timestamp in the frontmatter on every write>.`

### FR-006 — Information Gathering Session

`The <ragent Research System> shall <orchestrate an information-gathering session that combines web search and local file reading for a research topic>.`

`When <a research session starts>, the <ragent Research System> shall <issue at least one web search via the existing websearch tool and at least one local file scan via the existing read/grep/glob tools before producing the RESEARCH.md>.`

`If <the web search tool is unavailable or returns no results>, the <ragent Research System> shall <continue with local file research only and note the absence of web sources in the references index>.`

`If <the local file scan finds no relevant files>, the <ragent Research System> shall <continue with web research only and note the absence of local sources in the references index>.`

### FR-007 — Web Source Capture

`The <ragent Research System> shall <persist every web source consulted during a research session as a numbered supporting file in research/<name>/sources/web-<n>.md, where <n> is a zero-padded sequence starting at 01>.`

`When <a web search result is captured>, the <ragent Research System> shall <store the source URL, title, fetch timestamp, and full text content in the supporting file>.`

`Where <the source is fetchable via HTTP>, the <ragent Research System> shall <prefer fetching the full page via webfetch and storing the rendered text>.`

### FR-008 — Local Source Capture

`The <ragent Research System> shall <persist excerpts of any local file consulted during a research session as a numbered supporting file in research/<name>/sources/local-<n>.md>.`

`When <a local file is cross-referenced>, the <ragent Research System> shall <include the relative path from the project root, the matching lines or excerpt, and a one-line note explaining its relevance>.`

### FR-009 — Cross-Referencing with Project Resources

`The <ragent Research System> shall <cross-reference the research topic with resources in the current project, including: source files, prior specs under specs/, AGENTS.md, README.md, and any explicitly-named related directories>.`

`When <cross-referencing is performed>, the <ragent Research System> shall <identify at least three relevant project resources where available and include each in the references index with its path and a one-line relevance summary>.`

`If <fewer than three relevant project resources are found>, the <ragent Research System> shall <include all matches it did find and explicitly state that no further matches were identified>.`

### FR-010 — RESEARCH.md Document Structure

`The <ragent Research System> shall <write a RESEARCH.md file for each research item with the following sections, in order: frontmatter, Title, Topic, Summary, Findings, In-Project Cross-References, Open Questions, References Index>.`

`The <ragent Research System> shall <render the References Index as a markdown table with columns: #, Type (web|local|spec|other), Path/URL, Title, Relevance, Captured (ISO 8601)>.`

`While <the RESEARCH.md is being written>, the <ragent Research System> shall <number every reference sequentially starting at 1 and use the same number when citing the reference elsewhere in the document>.`

### FR-011 — References Index Block

`The <ragent Research System> shall <include a References Index block at the bottom of every RESEARCH.md that enumerates every supporting file written under research/<name>/sources/ and every web URL captured>.`

`When <a supporting file is written>, the <ragent Research System> shall <append a row to the References Index describing the file's path, type, capture timestamp, and one-line summary>.`

`If <no sources were captured during the session>, the <ragent Research System> shall <write a single row in the References Index stating "No sources captured">.`

### FR-012 — Research Index

`The <ragent Research System> shall <maintain a research/INDEX.md file that lists every research item with its name, title, status, created, and modified timestamps>.`

`When <a research item is created, updated, or deleted>, the <ragent Research System> shall <update research/INDEX.md to reflect the change before returning to the user>.`

`While <the research/ directory is being scanned>, the <ragent Research System> shall <derive the index from the on-disk state of each RESEARCH.md frontmatter, treating INDEX.md as a derived cache>.`

### FR-013 — Status Tracking

`The <ragent Research System> shall <track a status field for each research item with at least the values: draft, in-progress, complete, archived>.`

`When <a research item is created>, the <ragent Research System> shall <set its status to draft>.`

`While <the RESEARCH.md is being generated>, the <ragent Research System> shall <set the status to in-progress>.`

`When <the RESEARCH.md is fully written>, the <ragent Research System> shall <set the status to complete>.`

`If <the user runs "/research archive <name>">, the <ragent Research System> shall <set the status to archived and exclude the item from default list output unless --all is supplied>.`

### FR-014 — CLI Sub-Commands

`The <ragent Research System> shall <provide equivalent CLI sub-commands under "ragent research", matching the TUI /research sub-commands: create, list, open, search, show, delete, archive>.`

`When <the user runs "ragent research create <name> <topic>">, the <ragent Research System> shall <perform the same workflow as the TUI /research slash command but emit machine-readable progress to stdout>.`

### FR-015 — Integration with Spec Workflow

`The <ragent Research System> shall <allow a spec under specs/ to declare a research dependency in its PLAN.md via a line of the form "research: <research-name>">.`

`When <the user runs "/spec create <id> --from-research <name>">, the <ragent Research System> shall <pre-populate the new SPEC.md with a "## Related Research" section linking to research/<name>/RESEARCH.md>.`

`Where <a spec has a research dependency>, the <ragent Research System> shall <include the research name in /spec list output and link to the research directory>.`

### FR-016 — Unwanted Behaviour — Duplicate Creation

`If <the user attempts to create a research item with a name that already exists>, the <ragent Research System> shall <refuse the create operation, return a non-zero exit code in CLI mode, and suggest "/research open <name>" to view the existing item>.`

### FR-017 — Unwanted Behaviour — Path Traversal

`If <a research-name contains path traversal sequences such as "..", "/", or a leading dot>, the <ragent Research System> shall <reject the input as invalid per FR-002 and refuse to create or open the item>.`

### FR-018 — Unwanted Behaviour — Missing Item

`If <the user runs "/research open", "/research show", or "/research delete" with a name that does not exist>, the <ragent Research System> shall <return a clear "research item not found" error and list the names of the three closest existing items by edit distance>.`

### FR-019 — Optional — Additional Sources Directory

`Where <the user supplies "/research <name> <topic> --sources-dir <path>">, the <ragent Research System> shall <additionally scan the supplied directory during cross-referencing and include matching files in the references index with the "extra-local" type>.`

### FR-020 — Optional — Research Templates

`Where <a template file research/_templates/<name>.md exists>, the <ragent Research System> shall <use it as the skeleton for the new RESEARCH.md, replacing placeholder variables ({{title}}, {{topic}}, {{date}}) before writing>.`

### FR-021 — State-Driven — Concurrent Edit Detection

`While <a research item is open in the TUI viewer>, the <ragent Research System> shall <detect external file-system changes to RESEARCH.md by comparing mtime on each render and reload the file if it has changed since the viewer opened>.`

### FR-022 — Event-Driven — Autocomplete

`When <the user is typing "/research" in the TUI input>, the <ragent Research System> shall <display an autocomplete dropdown showing all valid sub-commands (create, list, open, search, show, delete, archive) and, after a sub-command, the existing research names>.`

---

## Non-Functional Requirements

### NFR-001 — Performance

`The <ragent Research System> shall <complete a /research create operation that gathers 10 web sources and 5 local sources and writes the RESEARCH.md in under 60 seconds on a typical broadband connection>.`

### NFR-002 — Reliability

`The <ragent Research System> shall <write all files atomically using write-then-rename so that a crash mid-write never leaves a partial RESEARCH.md visible>.`

### NFR-003 — Usability

`The <ragent Research System> shall <require no flags for the common case "/research <name> <topic>" and shall not prompt the user for optional inputs unless a flag is explicitly given>.`

### NFR-004 — Observability

`The <ragent Research System> shall <log each research session's start, each source captured (with URL or path), and the final write of RESEARCH.md at info level via the existing tracing infrastructure>.`

### NFR-005 — Portability

`The <ragent Research System> shall <produce paths in RESEARCH.md that are relative to the project root, using forward slashes, so that the file is portable across operating systems>.`

### NFR-006 — Security

`The <ragent Research System> shall <not execute any code from captured web sources; web content is treated as untrusted text and is escaped or fenced before being embedded in RESEARCH.md>.`

### NFR-007 — Maintainability

`The <ragent Research System> shall <be implemented in a new ragent-research crate that depends only on ragent-types, ragent-config, ragent-llm (for the agent context), and standard crates for HTTP/Markdown>.`

---

## Constraints & Assumptions

### Constraints

- The Research System must coexist with the existing `/spec` system; names under `specs/` and `research/` are independent namespaces
- The Research System must use the existing `websearch`, `webfetch`, `read`, `grep`, and `glob` tools rather than re-implementing their functionality
- The Research System must respect the existing permission system — fetching external URLs and writing files both go through normal permission checks
- All file paths in the `research/` directory must be relative to the project root

### Assumptions

- The LLM agent driving the research session has access to the same tool surface as a normal ragent session
- Web search and fetch have a working API key (Tavily or similar) configured by the user
- A `research/` directory at the project root is acceptable to version control
- Most research items will contain fewer than 50 sources; performance targets assume this scale

---

## Interfaces & Dependencies

### Internal Interfaces

- `ResearchName` (newtype over `String`) — validated per FR-002
- `ResearchItem` struct — `name`, `title`, `status`, `created`, `modified`, `sources: Vec<Source>`
- `Source` enum — `Web { url, title, captured_at, body_path }`, `Local { path, captured_at, body_path, relevance }`, `Spec { path, captured_at }`, `Other { label, captured_at, body_path }`
- `ResearchManager` — public API mirroring `SpecManager`; `create()`, `list()`, `open()`, `search()`, `show()`, `delete()`, `archive()`
- `ResearchIndex` — derived cache written to `research/INDEX.md`

### External Dependencies

- Existing `websearch` tool (crates/ragent-tools-extended) — for web discovery
- Existing `webfetch` tool — for full-page retrieval
- Existing `read`, `grep`, `glob` tools — for local cross-referencing
- Existing `tracing` infrastructure — for NFR-004
- `tokio` — for async orchestration of the research session
- `chrono` — for ISO 8601 timestamps (already in use)
- `similar` — for fuzzy matching research names in FR-018 (already a dependency)

### Dependencies on Existing ragent Crates

- `ragent-types` — for IDs, events, errors
- `ragent-config` — for resolving the project root
- `ragent-agent` — for the tool registry used during the research session
- `ragent-tui` — for the `/research` slash command, autocomplete, and viewer
- `ragent-server` — for the `POST /research` HTTP endpoint
- `ragent-specs` — for the spec-integration in FR-015

---

## Glossary

- **Research item** — A single `research/<name>/` directory containing a `RESEARCH.md` and optional supporting files
- **Research name** — The URL-safe identifier used as the directory name; validated per FR-002
- **Research session** — The act of running `/research create`; orchestrates web and local gathering
- **Source** — A single piece of captured evidence, either a web URL or a local file
- **References index** — The final section of `RESEARCH.md` listing every captured source
- **Cross-reference** — A local file, prior spec, or project document that is relevant to the topic
- **Skeleton** — The initial `RESEARCH.md` written before content is filled in
