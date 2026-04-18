# AIWiki Implementation Plan

## Overview

AIWiki is an embedded, project-scoped knowledge base system for ragent, inspired by [axiom-wiki](https://github.com/abubakarsiddik31/axiom-wiki). Unlike RAG systems that re-derive answers from raw sources on every query, AIWiki **compiles** knowledge into an interconnected wiki of markdown pages with automatic cross-linking and incremental updates.

## Key Differences from axiom-wiki

| Feature | axiom-wiki | ragent AIWiki |
|---------|-----------|---------------|
| Storage | `axiom/` directory | `[PROJECT_ROOT]/aiwiki/` |
| CLI | Standalone tool | Embedded slash commands (`/aiwiki`) |
| Integration | External process | Native HTTP server + slash commands |
| LLM | Configurable provider | Uses ragent's configured provider |
| Sync | Manual (`axiom-wiki sync`) | Automatic on file changes |
| Browse | Terminal UI | Web interface at `http://localhost:9100/aiwiki` |

## Directory Structure

```
[PROJECT_ROOT]/
├── aiwiki/
│   ├── config.json          # Wiki configuration
│   ├── state.json           # Source file hashes for incremental updates
│   ├── raw/                 # Source documents (PDFs, MD, images, etc.)
│   │   ├── README.pdf
│   │   ├── api-spec.md
│   │   └── architecture.png
│   ├── wiki/                # Generated markdown pages (Git-tracked)
│   │   ├── index.md         # Auto-generated page catalog
│   │   ├── entities/        # People, places, organizations
│   │   │   ├── john-doe.md
│   │   │   └── acme-corp.md
│   │   ├── concepts/        # Ideas, topics, theories
│   │   │   ├── rust-lifetimes.md
│   │   │   └── microservices.md
│   │   ├── sources/         # One summary per source file
│   │   │   ├── readme-pdf.md
│   │   │   └── api-spec-md.md
│   │   ├── analyses/        # Derived content (comparisons, Q&A)
│   │   │   └── rust-vs-go.md
│   │   └── log.md           # Operation history
│   ├── static/              # Web UI assets (CSS, JS, images)
│   │   ├── css/
│   │   │   └── aiwiki.css
│   │   ├── js/
│   │   │   └── aiwiki.js
│   │   └── favicon.ico
│   └── .gitignore           # Ignores raw/ (large files), tracks wiki/ and static/
```

## Design Principles

1. **Project-Scoped**: Each wiki is tied to a specific project/directory
2. **Markdown-Native**: Plain markdown with YAML frontmatter, editable in any editor
3. **Incremental**: SHA-256 hashing tracks source changes; only new/modified files processed
4. **Cross-Linked**: AI extracts entities and auto-links between pages
5. **Git-Friendly**: `wiki/` and `static/` are tracked; `raw/` is gitignored (contains binary/large files)
6. **Web-First**: Browse, search, and edit via HTTP server; slash commands trigger actions

## Slash Commands

| Command | Description |
|---------|-------------|
| `/aiwiki` | Open AIWiki in browser (launches default browser to `http://localhost:9100/aiwiki`) |
| `/aiwiki init` | Initialize aiwiki/ directory structure (auto-enables AIWiki) |
| `/aiwiki on` | Enable AIWiki system |
| `/aiwiki off` | Disable AIWiki system (no indexing, no performance impact) |
| `/aiwiki ingest [path]` | Ingest documents: file, directory, or scan raw/ folder |
| `/aiwiki sync` | Update stale wiki pages, process new sources |
| `/aiwiki search <query>` | Full-text search across all wiki pages (opens browser with results) |
| `/aiwiki status` | Show wiki statistics in TUI |
| `/aiwiki config` | Show/edit wiki configuration |
| `/aiwiki help` | Show detailed help and examples |

**Ingest Command Details:**
- `/aiwiki ingest` - Scans `aiwiki/raw/` directory for new/modified files
- `/aiwiki ingest /path/to/file.pdf` - Ingests a single file into `aiwiki/raw/`
- `/aiwiki ingest /path/to/folder` - Recursively ingests all supported files from directory

**Supported File Types:**
- Markdown (.md, .markdown)
- Plain Text (.txt)
- PDF (.pdf) - text extraction supported
- Word Documents (.docx) - stub support
- OpenDocument Text (.odt) - stub support

### AIWiki Enable/Disable State

The AIWiki system has an `enabled` flag stored in `config.json` that controls whether the wiki is active:

- **`enabled: true`** (default after `/aiwiki init`): AIWiki is active, indexing runs, sync commands work, all slash commands functional
- **`enabled: false`** (after `/aiwiki off`): AIWiki is completely disabled with zero performance impact

**Behavior when disabled (`/aiwiki off`):**
- No file watching or indexing occurs
- No background sync processes run
- All `/aiwiki` slash commands except `/aiwiki on` print: "AIWiki is currently disabled. Run `/aiwiki on` to enable."
- No wiki pages are accessible via web interface
- No memory overhead from AIWiki structures
- Existing wiki files remain on disk but are not processed

**Re-enabling (`/aiwiki on`):**
- Restores full AIWiki functionality
- Resumes from previous state (state.json preserved)
- Runs incremental sync if sync_mode is not Manual

**Implementation Notes:**
- The `enabled` field is stored in `aiwiki/config.json` and persists across sessions
- ragent-server checks `enabled` flag before starting any AIWiki background tasks
- TUI slash command handlers check `Aiwiki::is_enabled()` before executing
- When disabled, the system behaves as if AIWiki does not exist (no hooks, no watchers)

## Web Interface (ragent-server)

The AIWiki web interface is served by ragent-server at `http://localhost:9100/aiwiki`.

### HTTP Routes

```
GET  /aiwiki              → Home page (index.md rendered)
GET  /aiwiki/pages        → List all pages (JSON)
GET  /aiwiki/page/:path   → Render specific page (HTML)
GET  /aiwiki/edit/:path   → Edit page (HTML form)
POST /aiwiki/edit/:path   → Save page edits
GET  /aiwiki/search?q=... → Search results page
GET  /aiwiki/api/search?q=... → Search API (JSON)
GET  /aiwiki/graph        → Graph visualization (interactive D3/SVG)
GET  /aiwiki/api/graph    → Page graph data (JSON)
GET  /aiwiki/sync         → Trigger sync (redirects to home)
GET  /aiwiki/status       → Wiki statistics page
GET  /aiwiki/static/*     → Static assets (CSS, JS, images)
```

### Web UI Features

- **Home Page**: Wiki index with quick links, recent changes, search bar
- **Page Viewer**: Rendered markdown with syntax highlighting, navigation sidebar
- **Page Editor**: In-browser editing for wiki pages (saves to `.md` files)
- **Search**: Full-text search with highlighted results, faceted by type (entity, concept, source)
- **Graph View**: Interactive force-directed graph of page relationships
- **Status Dashboard**: Sync status, storage usage, recent activity log

## Data Models

### Wiki Configuration (config.json)
```json
{
  "version": "1.0.0",
  "created_at": "2026-01-15T10:30:00Z",
  "last_sync": "2026-01-15T14:20:00Z",
  "settings": {
    "auto_sync": true,
    "auto_sync_interval_secs": 300,
    "max_file_size_mb": 50,
    "excluded_patterns": ["*.tmp", "*.log", "node_modules/**"],
    "web_port": 9100,
    "web_theme": "light"
  }
}
```

### State Tracking (state.json)
```json
{
  "sources": {
    "raw/README.pdf": {
      "hash": "sha256:abc123...",
      "ingested_at": "2026-01-15T10:30:00Z",
      "pages": ["sources/readme-pdf.md"]
    },
    "raw/api-spec.md": {
      "hash": "sha256:def456...",
      "ingested_at": "2026-01-15T11:00:00Z",
      "pages": ["sources/api-spec-md.md", "concepts/rest-api-design.md"]
    }
  }
}
```

### Wiki Page Schema (YAML Frontmatter)
```yaml
---
id: "entity-john-doe-001"
type: "entity"  # entity | concept | source | analysis
title: "John Doe"
created: "2026-01-15T10:30:00Z"
updated: "2026-01-15T10:30:00Z"
source: "raw/team-roster.pdf"
tags: ["team", "engineering", "leadership"]
related: ["entity-acme-corp", "concept-microservices"]
---

# John Doe

Engineering lead at Acme Corp...

## Relationships

- Works at [[Acme Corp]]
- Expert in [[microservices]] architecture
```

## Milestones

### Milestone 1: Foundation (Core Infrastructure) ✅ COMPLETE
**Goal**: Basic directory structure, configuration, and state management
**Status**: All tasks completed

#### Tasks

1. ✅ **T1.1 - Create aiwiki crate structure**
   - Create `crates/aiwiki/` with Cargo.toml
   - Define public API module structure
   - Add to workspace Cargo.toml

2. ✅ **T1.2 - Implement directory initialization**
   - `aiwiki init` command
   - Create aiwiki/, aiwiki/raw/, aiwiki/wiki/ subdirectories
   - Generate config.json with defaults
   - Generate .gitignore for raw/

3. ✅ **T1.3 - Implement configuration management**
   - Load/save config.json
   - Config validation
   - Default settings

4. ✅ **T1.4 - Implement state tracking**
   - SHA-256 file hashing
   - Load/save state.json
   - Detect new/modified/deleted sources

5. ✅ **T1.5 - Add slash command skeleton**
   - `/aiwiki init` integration in TUI
   - Basic status messages

6. ✅ **T1.6 - Implement enable/disable system**
   - Add `enabled` field to config.json
   - `/aiwiki on` command - enable AIWiki
   - `/aiwiki off` command - disable AIWiki
   - `/aiwiki init` auto-enables (sets enabled=true)
   - When disabled, all commands except `/aiwiki on` show disabled message
   - Zero performance impact when disabled (no indexing, no watchers)
   - Update TUI handlers to check enabled state

**Acceptance Criteria**:
- ✅ `/aiwiki init` creates proper directory structure
- ✅ Config and state files are valid JSON
- ✅ Hash detection works correctly
- 🔄 `/aiwiki on/off` toggle works and persists in config
- 🔄 When disabled, only `/aiwiki on` works
- 🔄 When disabled, no background processes run

---

### Milestone 2: Ingestion Pipeline
**Goal**: File ingestion with LLM-powered extraction

#### Tasks

6. **T2.1 - Implement file readers**
   - Text files (.md, .txt, .rs, .py, etc.)
   - PDF support (via pdf_extract or similar)
   - Image OCR (optional, future)
   - Size limits and exclusion patterns

7. **T2.2 - Implement LLM extraction prompt**
   - Design extraction prompt for entities/concepts
   - Support different content types
   - Token budgeting

8. **T2.3 - Implement source summary generation**
   - Generate sources/<filename>.md for each ingested file
   - Extract key points, entities, concepts
   - Link to extracted pages

9. **T2.4 - Implement entity/concept extraction**
   - Generate entities/<slug>.md for people, orgs, places
   - Generate concepts/<slug>.md for ideas, topics
   - Auto-link related pages

10. **T2.5 - Implement incremental ingestion**
    - Skip unchanged files (hash check)
    - Update modified files
    - Remove deleted file entries

11. **T2.6 - Add ingestion slash commands**
    - `/aiwiki ingest <file>` - single file
    - `/aiwiki ingest` - scan raw/ directory
    - Progress reporting in TUI

**Acceptance Criteria**:
- Text and PDF files can be ingested
- Source summaries are generated with YAML frontmatter
- Entities and concepts are extracted
- Incremental updates work (only changed files processed)

---

### Milestone 3: Sync & Auto-Update ✅ COMPLETE
**Goal**: Automatic wiki maintenance and synchronization
**Status**: Core sync implementation complete

#### Tasks

12. ✅ **T3.1 - Implement stale page detection**
    - Compare source hashes to detect outdated wiki pages
    - Flag pages needing refresh

13. ✅ **T3.2 - Implement sync orchestration**
    - Re-extract from updated sources
    - Update existing wiki pages
    - Preserve manual edits (merge strategy)

14. ✅ **T3.3 - Implement cross-link validation**
    - Detect broken internal links
    - Suggest new links from content

15. ✅ **T3.4 - Implement auto-sync file watcher**
    - Watch raw/ directory for changes
    - Debounced ingestion trigger
    - Configurable interval

16. ✅ **T3.5 - Add sync slash commands**
    - `/aiwiki sync` - manual sync trigger
    - Show sync report (updated, created, removed counts)

**Acceptance Criteria**:
- ✅ `/aiwiki sync` updates stale pages
- ✅ File watcher detects changes in raw/
- ✅ Cross-links are validated

---

### Milestone 4: Web Interface (ragent-server Integration)
**Goal**: Full-featured web-based wiki browsing, editing, and search served by ragent-server

#### Tasks

17. **T4.1 - Design web UI HTML templates**
    - Base layout template with navigation
    - Page viewer template with markdown rendering
    - Page editor template (textarea with preview)
    - Search results template
    - Graph visualization container
    - Status dashboard template

18. **T4.2 - Implement static asset serving**
    - CSS styles (light/dark themes)
    - JavaScript for interactivity
    - Favicon and icons
    - Serve from `/aiwiki/static/*`

19. **T4.3 - Implement page serving routes**
    - `GET /aiwiki` - Home page (renders index.md)
    - `GET /aiwiki/page/:path` - Render markdown page to HTML
    - `GET /aiwiki/pages` - JSON API for page list
    - Wiki-link resolution (`[[Page Name]]` → actual URL)
    - Syntax highlighting for code blocks

20. **T4.4 - Implement page editing**
    - `GET /aiwiki/edit/:path` - Edit form with current content
    - `POST /aiwiki/edit/:path` - Save edits to .md file
    - Preserve YAML frontmatter on edit
    - Auto-update `updated` timestamp

21. **T4.5 - Implement full-text search (web)**
    - `GET /aiwiki/search?q=...` - Search results page (HTML)
    - `GET /aiwiki/api/search?q=...` - Search API (JSON)
    - Tantivy-based indexing
    - Highlight matching terms
    - Filter by type (entity, concept, source, analysis)

22. **T4.6 - Implement interactive graph visualization**
    - `GET /aiwiki/graph` - Graph page with D3.js/SVG
    - `GET /aiwiki/api/graph` - Page graph data (JSON nodes + edges)
    - Force-directed layout
    - Click to navigate, zoom, pan
    - Filter by page type

23. **T4.7 - Implement web status dashboard**
    - `GET /aiwiki/status` - Web-based statistics
    - Source count, page count, last sync
    - Storage usage
    - Recent activity log
    - Sync trigger button

24. **T4.8 - Update slash commands for web**
    - `/aiwiki` - Open browser to `http://localhost:9100/aiwiki`
    - `/aiwiki search <query>` - Open browser to search results
    - `/aiwiki status` - Show brief status in TUI + "Open in browser" link

25. **T4.9 - Implement browser launching**
    - Detect default browser (xdg-open, open, start)
    - Launch browser on `/aiwiki` command
    - Configurable browser preference

**Acceptance Criteria**:
- `/aiwiki` opens wiki in default browser
- Pages render with proper markdown formatting
- Wiki-links navigate between pages
- Search returns ranked results with highlighting
- Graph shows interactive page relationships
- Editing saves changes to .md files
- Static assets load correctly

---

### Milestone 5: Analysis & Derived Content ✅ COMPLETE
**Goal**: AI-powered analysis and Q&A on wiki content
**Status**: Core analysis features implemented

#### Tasks

26. ✅ **T5.1 - Implement analysis generation**
    - Compare multiple sources (e.g., "Rust vs Go")
    - Generate analysis/<slug>.md pages
    - Track source provenance

27. ✅ **T5.2 - Implement wiki Q&A**
    - Query wiki content via LLM
    - Ground responses in wiki sources
    - Cite source pages

28. ✅ **T5.3 - Implement contradiction detection**
    - Compare statements across pages
    - Flag potential contradictions
    - Suggest resolutions

29. 🔄 **T5.4 - Add analysis slash commands**
    - `/aiwiki analyze <topic>` - generate analysis (API ready, TUI help updated)
    - `/aiwiki ask <question>` - Q&A on wiki (API ready, TUI help updated)
    - `/aiwiki review` - find contradictions (API ready, TUI help updated)

**Acceptance Criteria**:
- ✅ Analysis pages are generated with proper attribution
- ✅ Q&A cites wiki sources
- ✅ Contradictions are detected and flagged

---

### Milestone 6: Integration & Polish ✅ COMPLETE (except T6.1)
**Goal**: Seamless integration with ragent workflows

#### Tasks

30. **T6.1 - Implement session context injection** ⏳ PENDING
    - Auto-include relevant wiki pages in LLM context
    - Based on current conversation topics
    - Note: This requires session processor modifications and is a future enhancement

31. **T6.2 - Implement wiki-aware tool calls** ✅ COMPLETE
    - `aiwiki_search` tool for agents
    - `aiwiki_ingest` tool for agents  
    - `aiwiki_status` tool for agents

32. **T6.3 - Implement export/import** ✅ COMPLETE
    - Export wiki as single markdown file
    - Import external markdown into wiki
    - Obsidian-compatible vault export
    - Additional: `aiwiki_export` and `aiwiki_import` tools for agents

33. **T6.4 - Add status monitoring** ✅ COMPLETE
    - `/aiwiki status` shows detailed stats
    - Token usage tracking (in state)
    - Storage usage display

34. **T6.5 - Documentation & examples** ✅ COMPLETE
    - User guide in docs/userdocs/aiwiki.md
    - Example workflow in examples/aiwiki/
    - Update QUICKSTART.md with AIWiki section

**Acceptance Criteria**:
- Wiki content can be used in agent conversations
- Tools expose wiki functionality to agents
- Status shows comprehensive statistics
- Documentation is complete

---

## Technical Architecture

### Crate Structure

```
crates/aiwiki/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── config.rs           # Configuration management
│   ├── state.rs            # State tracking (hashes)
│   ├── ingest.rs           # File ingestion pipeline
│   ├── extract/            # LLM extraction
│   │   ├── mod.rs
│   │   ├── entities.rs
│   │   ├── concepts.rs
│   │   └── sources.rs
│   ├── sync.rs             # Synchronization logic
│   ├── search.rs           # Full-text search indexing (tantivy)
│   ├── graph.rs            # Page relationship graph
│   ├── analysis.rs         # Derived content generation
│   ├── markdown.rs         # Markdown parsing and rendering
│   └── web/                # Web interface (NEW)
│       ├── mod.rs          # Module exports
│       ├── routes.rs       # HTTP route handlers
│       ├── templates.rs    # HTML template definitions
│       ├── static/         # Static assets (embedded or filesystem)
│       │   ├── css/
│       │   └── js/
│       └── render.rs       # Markdown to HTML rendering
└── tests/
    ├── integration_tests.rs
    └── fixtures/
```

### Dependencies

```toml
[dependencies]
# Core
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
thiserror = "1.0"

# File handling
walkdir = "2"
sha2 = "0.10"
hex = "0.4"

# PDF extraction
pdf-extract = "0.7"

# Search
tantivy = "0.21"

# Markdown
pulldown-cmark = "0.9"
pulldown-cmark-to-cmark = "11"
comrak = "0.24"  # GitHub-flavored markdown rendering

# Frontmatter
serde_yaml = "0.9"

# Web (axum - same as ragent-server)
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["fs", "trace"] }
askama = "0.12"  # HTML templates

# Browser launching
webbrowser = "1.0"

# LLM (reuse ragent-core)
ragent-core = { path = "../ragent-core" }
```

### Integration Points

1. **HTTP Server Integration** (`crates/ragent-server/src/`)
   - Add `aiwiki` module to server routes
   - Mount at `/aiwiki/*` 
   - Serve static files, pages, search, graph

2. **TUI Integration** (`crates/ragent-tui/src/`)
   - Slash commands: `/aiwiki`, `/aiwiki init`, `/aiwiki ingest`, `/aiwiki sync`, `/aiwiki search`, `/aiwiki status`
   - Browser launching on `/aiwiki` command

3. **Tool Integration** (`crates/ragent-core/src/tool/`)
   - New tools: `aiwiki_search`, `aiwiki_ingest`, `aiwiki_status`

4. **Static Assets**
   - Option A: Embed with `rust-embed` crate
   - Option B: Serve from filesystem at `aiwiki/static/`
   - Option C: Generate on first run

## Web Interface Design

### Page Layout

```
+----------------------------------------------------------+
| 🧠 AIWiki  [Home] [Pages] [Graph] [Search...] [Status]   |
+----------------------------------------------------------+
|                                                          |
|  +------------------+  +------------------------------+   |
|  | Navigation       |  | Content                      |   |
|  | - Entities (12)  |  |                              |   |
|  | - Concepts (8)   |  |  # Page Title                |   |
|  | - Sources (5)    |  |                              |   |
|  | - Analyses (2)   |  |  Content here...             |   |
|  |                  |  |  - bullet                    |   |
|  | Recent Changes     |  |  - bullet                    |   |
|  | - page 1           |  |                              |   |
|  | - page 2           |  |  [[Linked Page]]             |   |
|  +------------------+  +------------------------------+   |
|                                                          |
+----------------------------------------------------------+
| Last synced: 5 minutes ago | [Edit] [Delete]           |
+----------------------------------------------------------+
```

### Search Results Page

```
+----------------------------------------------------------+
| Search: "rust async"                      [Search]      |
+----------------------------------------------------------+
| Found 12 results in 0.15s                                  |
|                                                          |
|  [Sources] [Entities] [Concepts] [Analyses]              |
|                                                          |
|  ## Sources (3)                                          |
|  1. **Async Rust Book** (sources/async-rust-book.md)     |
|     ...async programming in Rust enables...              |
|     [View] [Related: Tokio, Futures]                     |
|                                                          |
|  ## Concepts (2)                                         |
|  1. **Async/Await** (concepts/async-await.md)            |
|     Programming pattern for non-blocking...              |
|                                                          |
|  ## Entities (1)                                         |
|  1. **Tokio Runtime** (entities/tokio-runtime.md)        |
|     Async runtime for Rust...                            |
|                                                          |
+----------------------------------------------------------+
```

### Graph Visualization

```
+----------------------------------------------------------+
| AIWiki Graph | [Filter: All ▼] [Reset View]              |
+----------------------------------------------------------+
|                                                          |
|     +----------+                                         |
|     | Rust     |                                         |
|     +----+-----+                                         |
|          |                                               |
|     +----v-----+     +----------+                       |
|     | Lifetimes|-----| Ownership |                       |
|     +----------+     +----+-----+                       |
|                             |                           |
|     +----------+     +------v--+                       |
|     | Borrow   |-----| Checker |                       |
|     +----------+     +---------+                       |
|                                                          |
+----------------------------------------------------------+
| Click to navigate • Scroll to zoom • Drag to pan         |
+----------------------------------------------------------+
```

## LLM Prompts

### Source Summary Extraction

```
Analyze the following document content and create a structured summary.

Content:
{content}

Extract:
1. A brief summary (2-3 paragraphs)
2. Key entities mentioned (people, organizations, places)
3. Key concepts/topics discussed
4. Important dates or milestones

Format your response as YAML frontmatter followed by markdown content.
```

### Entity Extraction

```
Extract all entities from the following text.

Text:
{text}

For each entity, provide:
- name: Full name
- type: person | organization | place | product | other
- description: Brief description
- relationships: List of related entities mentioned

Output as a YAML list.
```

### Concept Extraction

```
Extract all concepts and topics from the following text.

Text:
{text}

For each concept, provide:
- name: Concept name
- description: Brief explanation
- related_concepts: List of related concepts mentioned

Output as a YAML list.
```

## Success Metrics

1. **Ingestion Performance**: Process 100 pages in <30 seconds
2. **Search Latency**: Full-text query returns in <100ms
3. **Web Latency**: Page loads in <200ms
4. **Storage Efficiency**: Wiki size <20% of raw source size
5. **Accuracy**: Entity extraction precision >80%
6. **Usability**: Slash command discoverable via `/help`
7. **Accessibility**: WCAG 2.1 AA compliant web interface

## Future Enhancements

- Web clipping (`/aiwiki clip <url>`)
- Image OCR for diagrams
- Collaborative wiki sync (multi-user)
- Vector semantic search
- MCP server integration
- Mobile app companion
- Real-time sync with WebSocket updates
- Page templates for common types
- Wiki page versioning (Git integration)

---

**Document Version**: 1.1.0  
**Last Updated**: 2026-01-15  
**Status**: Design Complete - Ready for Implementation

## Changelog

### v1.1.0 (2026-01-15)
- Changed browsing mechanism from TUI panel to web interface served by ragent-server
- Added HTTP routes for `/aiwiki/*` endpoints
- Added static asset serving (CSS, JS, images)
- Added interactive graph visualization with D3.js
- Added in-browser page editing
- Added browser launching from TUI slash commands
- Updated milestone 4 from "TUI Panel" to "Web Interface"
- Updated technical architecture with web module
