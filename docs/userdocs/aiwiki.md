# AIWiki User Guide

AIWiki is an embedded, project-scoped knowledge base system for ragent. It compiles knowledge into an interconnected wiki of markdown pages with automatic cross-linking and incremental updates.

## Table of Contents

- [Quick Start](#quick-start)
- [Concepts](#concepts)
- [Slash Commands](#slash-commands)
- [Agent Tools](#agent-tools)
- [Web Interface](#web-interface)
- [Import/Export](#importexport)
- [Best Practices](#best-practices)
- [Troubleshooting](#troubleshooting)

## Quick Start

### 1. Initialize AIWiki

```
/aiwiki init
```

This creates the `aiwiki/` directory structure:
- `aiwiki/raw/` — Place source documents here
- `aiwiki/wiki/` — Generated markdown pages
- `aiwiki/static/` — Web UI assets
- `aiwiki/config.json` — Wiki configuration
- `aiwiki/state.json` — File tracking state

### 2. Add Documents

Place documents in `aiwiki/raw/`:
- Markdown files (`.md`)
- Plain text (`.txt`)
- PDF files (`.pdf`)
- Word documents (`.docx`)
- OpenDocument text (`.odt`)

### 3. Sync

```
/aiwiki sync
```

This processes your documents into wiki pages with extracted entities and concepts.

### 4. Browse

```
/aiwiki
```

Opens the web interface at `http://localhost:9100/aiwiki`

## Concepts

### Pages

AIWiki generates four types of pages:

- **Entities** — People, organizations, places (e.g., `entities/john-doe.md`)
- **Concepts** — Ideas, topics, theories (e.g., `concepts/rust-lifetimes.md`)
- **Sources** — One summary per source document (e.g., `sources/readme-pdf.md`)
- **Analyses** — Derived content like comparisons (e.g., `analyses/rust-vs-go.md`)

### Cross-Linking

Pages are automatically linked:
- Entity mentions in source documents link to entity pages
- Concept references link to concept pages
- Bidirectional links between related pages

### Incremental Updates

AIWiki tracks file hashes in `state.json`. Only new or modified files are processed during sync.

## Slash Commands

| Command | Description |
|---------|-------------|
| `/aiwiki show` | Open AIWiki in browser |
| `/aiwiki init` | Initialize wiki structure |
| `/aiwiki on` | Enable AIWiki |
| `/aiwiki off` | Disable AIWiki (zero performance impact) |
| `/aiwiki ingest [path]` | Ingest file, directory, or scan raw/ |
| `/aiwiki sync [--force]` | Sync wiki with changes |
| `/aiwiki status` | Show statistics |
| `/aiwiki search <query>` | Search wiki content |
| `/aiwiki ask <question>` | Query wiki with citations |
| `/aiwiki analyze <topic> <sources...>` | Generate analysis |
| `/aiwiki review` | Find contradictions |
| `/aiwiki help` | Show help |

## Agent Tools

Agents can use these tools to interact with AIWiki:

### `aiwiki_search`

Search the wiki for pages matching a query.

**Parameters:**
- `query` (required) — Search terms
- `page_type` — Filter by: entities, concepts, sources, analyses
- `max_results` — Default 10, max 50

**Example:**
```json
{
  "query": "rust memory safety",
  "page_type": "concepts",
  "max_results": 5
}
```

### `aiwiki_ingest`

Ingest documents into the knowledge base.

**Parameters:**
- `path` — File or directory to ingest (optional, scans raw/ if omitted)
- `move_file` — Move instead of copy (default: false)
- `subdirectory` — Store in subdirectory within raw/

**Example:**
```json
{
  "path": "docs/api-spec.md",
  "move_file": false
}
```

### `aiwiki_status`

Show wiki statistics and configuration.

**Parameters:** None

### `aiwiki_export`

Export wiki to various formats.

**Parameters:**
- `format` — "single_markdown" or "obsidian"
- `output_path` — Output file or directory

**Example:**
```json
{
  "format": "obsidian",
  "output_path": "my_vault"
}
```

### `aiwiki_import`

Import external markdown into wiki.

**Parameters:**
- `path` (required) — File or directory to import
- `target_subdir` — Place in subdirectory (default: "imports")

**Example:**
```json
{
  "path": "external_docs/",
  "target_subdir": "external"
}
```

## Web Interface

Access at `http://localhost:9100/aiwiki`

### Features

- **Browse** — Navigate pages by category
- **Search** — Full-text search with excerpts
- **Edit** — Modify pages in-browser
- **Graph** — Visualize page relationships
- **Status** — View statistics

### Pages

| URL | Description |
|-----|-------------|
| `/aiwiki` | Home/index page |
| `/aiwiki/page/*path` | View specific page |
| `/aiwiki/edit/*path` | Edit page |
| `/aiwiki/search?q=query` | Search results |
| `/aiwiki/graph` | Relationship graph |
| `/aiwiki/status` | Statistics |

## Import/Export

### Export to Single Markdown

```
/aiwiki export single_markdown my_wiki.md
```

Creates one combined markdown file with all pages.

### Export to Obsidian

```
/aiwiki export obsidian my_vault/
```

Creates an Obsidian-compatible vault:
- `.obsidian/` directory with settings
- All wiki pages as `.md` files
- Proper frontmatter for Obsidian

### Import External Markdown

```
/aiwiki import /path/to/docs
```

Imports markdown files into `aiwiki/wiki/imports/`.

## Best Practices

### File Organization

```
aiwiki/
├── raw/
│   ├── README.pdf          # Project docs
│   ├── api/
│   │   └── openapi.yaml    # API specs
│   └── research/
│       └── paper.pdf       # Research papers
└── wiki/                   # Auto-generated
    ├── entities/           # People, orgs
    ├── concepts/           # Topics
    ├── sources/            # Doc summaries
    └── analyses/           # Comparisons
```

### Naming

- Use descriptive file names
- Prefer kebab-case: `api-overview.md` not `API Overview.md`
- Group related files in subdirectories

### Sync Workflow

1. Add documents to `aiwiki/raw/`
2. Run `/aiwiki sync` to process
3. Review generated pages
4. Edit pages for clarity if needed
5. Use `/aiwiki ask` to query

### Performance

- Disable when not needed: `/aiwiki off`
- Sync is incremental (only changed files)
- Large files (>10MB) are skipped
- Use subdirectories to organize

## Troubleshooting

### "AIWiki not initialized"

Run `/aiwiki init` first.

### "AIWiki is disabled"

Run `/aiwiki on` to enable.

### Sync shows no changes

Check that files are in `aiwiki/raw/` and are supported formats.

### Web interface not loading

Ensure the ragent server is running on port 9100.

### Broken links

Run `/aiwiki sync` to regenerate cross-links.

### Out of sync

Use `/aiwiki sync --force` to re-process all files.

## See Also

- [AIWIKIPLAN.md](../AIWIKIPLAN.md) — Implementation plan
- Examples: `examples/aiwiki/`
