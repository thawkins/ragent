# AIWiki Example Workflow

This example demonstrates a complete AIWiki workflow from initialization to querying.

## Prerequisites

- ragent with AIWiki support
- A project directory

## Step-by-Step Workflow

### 1. Initialize AIWiki

```bash
cd your-project
ragent
```

In the TUI:
```
/aiwiki init
```

Expected output:
```
✅ AIWiki initialized and enabled!

Created directory structure:
• aiwiki/raw/ — place source documents here
• aiwiki/wiki/ — generated markdown pages
• aiwiki/static/ — web UI assets

The wiki is now active and ready.

Next steps:
• Add documents to aiwiki/raw/
• Run `/aiwiki sync` to process them
```

### 2. Add Documents

Create some sample content:

```bash
# Create a project README
cat > aiwiki/raw/README.md << 'EOF'
# My Project

This project uses Rust for high-performance systems.

## Architecture

We use a microservices architecture with the following components:

- **API Gateway** — Built with Axum
- **Auth Service** — Handles authentication
- **Data Service** — Manages data storage

## Technologies

- Rust (v1.75+)
- PostgreSQL
- Redis
- Docker
EOF

# Create an architecture document
cat > aiwiki/raw/architecture.md << 'EOF'
# System Architecture

## Overview

Our system is designed around Domain-Driven Design principles.

## Key Decisions

### Why Rust?

Rust provides:
- Memory safety without GC
- Zero-cost abstractions
- Excellent concurrency model

### Why Microservices?

- Independent deployment
- Technology flexibility
- Scalability

## Services

| Service | Language | Purpose |
|---------|----------|---------|
| API Gateway | Rust | Routing |
| Auth Service | Rust | Authentication |
| Data Service | Rust | Data access |
EOF
```

### 3. Sync the Wiki

```
/aiwiki sync
```

Expected output:
```
Sync complete: 2 new sources, 4 new pages

New pages:
• sources/README.md
• sources/architecture.md
• entities/API-Gateway
• entities/Auth-Service
• entities/Data-Service
• concepts/microservices
• concepts/Rust
• concepts/Domain-Driven-Design
```

### 4. Search

```
/aiwiki search rust architecture
```

Or use the tool:
```
/aiwiki ask "Why did we choose Rust?"
```

Expected output:
```
Based on the wiki:

Rust was chosen for the following reasons (from concepts/Rust.md):

- Memory safety without GC
- Zero-cost abstractions
- Excellent concurrency model

Sources:
- architecture.md (section: Why Rust?)
```

### 5. Browse

```
/aiwiki
```

Opens browser at `http://localhost:9100/aiwiki`

### 6. Generate Analysis

Create a comparison:

```
/aiwiki analyze "Microservices vs Monolith" sources/architecture.md
```

Expected output:
```
✅ Analysis generated: analyses/microservices-vs-monolith.md

This analysis compares microservices architecture (documented in the wiki)
with monolithic architecture, discussing trade-offs for your project context.
```

### 7. Export

Export for sharing:

```
/aiwiki export single_markdown my_project_wiki.md
```

Or for Obsidian:

```
/aiwiki export obsidian obsidian_vault/
```

### 8. Review Status

```
/aiwiki status
```

Expected output:
```
## AIWiki Status

**Status:** Enabled ✅

### Pages
- Total: 8
  - Entities: 3
  - Concepts: 3
  - Sources: 2
  - Analyses: 1

### Sources
- Raw files: 2
- Pending sync: 0

### Storage
- Wiki size: 45.2 KB
- Raw size: 12.8 KB

### Sync Status
- ✅ Wiki is up to date
- Last sync: 2026-01-15 14:32 UTC

### Configuration
- Auto-sync: Disabled (manual)
- Max file size: 10.0 MB
```

## Agent Integration Example

When working with agents, you can use AIWiki tools:

```
Agent: I need to understand the project architecture.

User: Use the wiki search tool to find information about our architecture.

Agent: [Uses aiwiki_search with query="architecture microservices"]

Found 3 relevant pages:
1. sources/architecture.md — System Architecture overview
2. concepts/microservices.md — Microservices concept
3. analyses/microservices-vs-monolith.md — Comparison

The system uses a microservices architecture with Domain-Driven Design...
```

## File Structure After Workflow

```
my-project/
├── aiwiki/
│   ├── config.json
│   ├── state.json
│   ├── raw/
│   │   ├── README.md
│   │   └── architecture.md
│   └── wiki/
│       ├── index.md
│       ├── entities/
│       │   ├── API-Gateway.md
│       │   ├── Auth-Service.md
│       │   └── Data-Service.md
│       ├── concepts/
│       │   ├── Rust.md
│       │   ├── microservices.md
│       │   └── Domain-Driven-Design.md
│       ├── sources/
│       │   ├── README.md
│       │   └── architecture.md
│       └── analyses/
│           └── microservices-vs-monolith.md
└── src/
    └── ...
```

## Next Steps

- Try ingesting PDF documentation: `/aiwiki ingest docs/api.pdf`
- Import external markdown: `/aiwiki import /path/to/docs`
- Generate more analyses with `/aiwiki analyze`
- Review contradictions: `/aiwiki review`

## See Also

- [AIWiki User Guide](../../docs/userdocs/aiwiki.md)
- [AIWIKIPLAN.md](../../AIWIKIPLAN.md)
