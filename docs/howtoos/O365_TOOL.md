# O365 Tool — Office Document Read/Write Support

## Overview

Add a new tool to ragent that enables the AI agent to read and write Microsoft
Office documents (Word `.docx`, Excel `.xlsx`, and PowerPoint `.pptx` files).
The tool will be registered in the standard `ToolRegistry` alongside existing
tools (`read`, `write`, `edit`, `bash`, etc.) and will be available to all LLM
providers via function-calling.

## Goals

- **Read** content from `.docx`, `.xlsx`, and `.pptx` files and return it as
  structured plain text that the LLM can reason about.
- **Write** content to `.docx`, `.xlsx`, and `.pptx` files from structured
  text/JSON input provided by the LLM.
- Integrate cleanly with the existing `Tool` trait, `ToolContext`, and
  permission system.
- Keep dependencies minimal and avoid pulling in large C/C++ bindings.

## Recommended Rust Crates

| Format     | Crate              | Purpose              | Notes                                 |
|------------|--------------------|----------------------|---------------------------------------|
| Word docx  | `docx-rust`        | Read & write `.docx` | Mature, good API coverage             |
| Excel xlsx | `calamine`         | Read `.xlsx`/`.xls`  | Proven reader, supports many formats  |
| Excel xlsx | `rust_xlsxwriter`  | Write `.xlsx`        | Feature-rich writer                   |
| PowerPoint | `ooxmlsdk`         | Read & write `.pptx` | Open XML SDK port, covers all formats |

**Alternative (unified):** `ooxmlsdk` covers all three formats but is lower-level
and requires more boilerplate. The per-format crate approach gives better ergonomics
for each file type.

## Architecture

### Tool Registration

Three new tools registered in `create_default_registry()`:

| Tool Name      | Permission      | Description                                |
|----------------|-----------------|--------------------------------------------|
| `office_read`  | `file:read`     | Read content from Word, Excel, or PowerPoint files |
| `office_write` | `file:write`    | Write content to Word, Excel, or PowerPoint files  |
| `office_info`  | `file:read`     | Get metadata/structure info about an Office file    |

### File Structure

```
crates/ragent-core/src/tool/
├── mod.rs                  # Updated: register new tools
├── office_read.rs          # OfficeReadTool implementation
├── office_write.rs         # OfficeWriteTool implementation
├── office_info.rs          # OfficeInfoTool implementation
└── office_common.rs        # Shared helpers (format detection, text extraction)
```

### Tool Schemas

#### `office_read`

```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the Office document to read"
    },
    "sheet": {
      "type": "string",
      "description": "For Excel: sheet name or index (default: first sheet)"
    },
    "range": {
      "type": "string",
      "description": "For Excel: cell range e.g. 'A1:D10' (default: all data)"
    },
    "slide": {
      "type": "integer",
      "description": "For PowerPoint: specific slide number (default: all slides)"
    },
    "format": {
      "type": "string",
      "enum": ["text", "markdown", "json"],
      "description": "Output format (default: markdown)"
    }
  },
  "required": ["path"]
}
```

#### `office_write`

```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to write the Office document"
    },
    "type": {
      "type": "string",
      "enum": ["docx", "xlsx", "pptx"],
      "description": "Document type (auto-detected from extension if omitted)"
    },
    "content": {
      "type": "object",
      "description": "Document content (structure depends on type)"
    }
  },
  "required": ["path", "content"]
}
```

**Content structure per type:**

- **docx**: `{ "paragraphs": [{ "text": "...", "style": "Heading1" }, ...] }`
- **xlsx**: `{ "sheets": [{ "name": "Sheet1", "rows": [["A1", "B1"], ["A2", "B2"]] }] }`
- **pptx**: `{ "slides": [{ "title": "...", "body": "...", "notes": "..." }] }`

#### `office_info`

```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the Office document"
    }
  },
  "required": ["path"]
}
```

Returns: file type, page/sheet/slide count, author, title, creation date,
word count (docx), sheet names (xlsx), slide titles (pptx).

## Implementation Plan

### TASK-001: Add dependencies and create module skeleton

- Add `docx-rust`, `calamine`, `rust_xlsxwriter`, and `ooxmlsdk` to
  `[workspace.dependencies]` in root `Cargo.toml`
- Add workspace references to `crates/ragent-core/Cargo.toml`
- Create empty module files: `office_common.rs`, `office_read.rs`,
  `office_write.rs`, `office_info.rs`
- Add `mod office_common; mod office_read; mod office_write; mod office_info;`
  to `tool/mod.rs`
- Verify: `cargo check`

### TASK-002: Implement format detection and shared helpers

File: `office_common.rs`

- `detect_format(path) -> Result<OfficeFormat>` — detect docx/xlsx/pptx from
  extension and magic bytes
- `OfficeFormat` enum: `Docx`, `Xlsx`, `Pptx`
- Path resolution helper using `ToolContext::working_dir`
- Write unit tests in `crates/ragent-core/tests/tool/`

### TASK-003: Implement `office_read` for Word (.docx)

File: `office_read.rs`

- Parse `.docx` using `docx-rust`
- Extract paragraphs with style information (headings, body, lists)
- Extract tables as markdown tables
- Output as plain text, markdown, or JSON
- Truncate output if exceeding 100KB (consistent with other tools)
- Write integration tests with sample `.docx` fixture files

### TASK-004: Implement `office_read` for Excel (.xlsx)

File: `office_read.rs`

- Parse `.xlsx` using `calamine`
- Support reading specific sheets by name or index
- Support cell range selection (e.g. `A1:D10`)
- Format cells as strings (handle dates, numbers, formulas)
- Output as markdown table, plain CSV, or JSON
- Include sheet metadata in output (row count, column count)
- Write integration tests with sample `.xlsx` fixture files

### TASK-005: Implement `office_read` for PowerPoint (.pptx)

File: `office_read.rs`

- Parse `.pptx` using `ooxmlsdk`
- Extract slide titles, body text, and speaker notes
- Support reading specific slides by number
- Output as structured markdown (one section per slide)
- Write integration tests with sample `.pptx` fixture files

### TASK-006: Implement `office_write` for Word (.docx)

File: `office_write.rs`

- Create new `.docx` documents using `docx-rust`
- Support paragraph styles: `Normal`, `Heading1`–`Heading6`, `ListBullet`,
  `ListNumber`, `Code`
- Support basic inline formatting via markdown-like syntax (`**bold**`,
  `*italic*`, `` `code` ``)
- Create parent directories if needed (consistent with WriteTool)
- Write integration tests (round-trip: write then read back)

### TASK-007: Implement `office_write` for Excel (.xlsx)

File: `office_write.rs`

- Create new `.xlsx` workbooks using `rust_xlsxwriter`
- Support multiple named sheets
- Support typed cell values (string, number, boolean, date)
- Support basic column width auto-sizing
- Write integration tests (round-trip: write then read back)

### TASK-008: Implement `office_write` for PowerPoint (.pptx)

File: `office_write.rs`

- Create new `.pptx` presentations using `ooxmlsdk`
- Support title + body layout per slide
- Support speaker notes
- Write integration tests (round-trip: write then read back)

### TASK-009: Implement `office_info`

File: `office_info.rs`

- Extract metadata from all three formats:
  - **Common**: file size, format, created/modified dates, author, title
  - **docx**: page count (estimated), paragraph count, word count
  - **xlsx**: sheet names and row/column counts per sheet
  - **pptx**: slide count, slide titles
- Return as structured text with optional JSON metadata
- Write integration tests

### TASK-010: Register tools and add integration tests

- Register `OfficeReadTool`, `OfficeWriteTool`, `OfficeInfoTool` in
  `create_default_registry()`
- Create sample fixture files in `tests/fixtures/office/`:
  - `sample.docx` — multi-paragraph document with headings and tables
  - `sample.xlsx` — multi-sheet workbook with mixed data types
  - `sample.pptx` — multi-slide presentation with notes
- Write end-to-end integration tests exercising the full tool pipeline
- Verify all existing tests still pass
- Run `cargo clippy` and `cargo fmt`

### TASK-011: Documentation

- Add doc comments with `# Examples` to all public APIs
- Update `DOC_INVENTRY.md` with new public items
- Update `README.md` with office tool documentation
- Update `SPEC.md` if tool specifications are tracked there

## Output Format Examples

### Reading a Word document

```
$ /tool office_read {"path": "report.docx"}

# Quarterly Report

## Introduction

This report covers Q4 2025 performance metrics...

## Financial Summary

| Metric   | Q3 2025 | Q4 2025 | Change |
|----------|---------|---------|--------|
| Revenue  | $1.2M   | $1.5M   | +25%   |
| Expenses | $800K   | $850K   | +6%    |

## Conclusion

Overall performance exceeded expectations...
```

### Reading an Excel spreadsheet

```
$ /tool office_read {"path": "data.xlsx", "sheet": "Sales", "range": "A1:C5"}

Sheet: Sales (5 rows × 3 columns)

| Product | Units | Revenue  |
|---------|-------|----------|
| Widget  | 150   | $4,500   |
| Gadget  | 89    | $2,670   |
| Doohick | 210   | $6,300   |
| Thingam | 45    | $1,350   |
```

### Writing a Word document

```json
{
  "path": "output.docx",
  "content": {
    "paragraphs": [
      { "text": "Meeting Notes", "style": "Heading1" },
      { "text": "Date: 2026-03-10", "style": "Normal" },
      { "text": "Action items:", "style": "Heading2" },
      { "text": "Complete the quarterly review", "style": "ListBullet" },
      { "text": "Update the project timeline", "style": "ListBullet" }
    ]
  }
}
```

## Risks and Mitigations

| Risk | Mitigation |
|------|-----------|
| `ooxmlsdk` is relatively new and may have gaps | Fall back to raw XML manipulation for unsupported features; wrap in error handling |
| Large Office files could overwhelm LLM context | Enforce output truncation (100KB limit), support range/slide selection |
| Binary file fixtures bloat the repository | Keep fixtures minimal; generate programmatically in tests where possible |
| Format detection edge cases (`.doc`, `.xls`) | Only support modern OOXML formats; return clear error for legacy formats |
| Complex formatting lost in text extraction | Focus on content extraction; document that formatting fidelity is best-effort |

## Dependencies Summary

```toml
# Workspace Cargo.toml additions
docx-rust = "0.5"
calamine = "0.26"
rust_xlsxwriter = "0.82"
ooxmlsdk = "0.3"
```

*Note: Pin to latest stable versions at implementation time. Version numbers
above are illustrative.*
