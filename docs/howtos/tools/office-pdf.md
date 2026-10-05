# Tools — PDF

Read and write PDF documents.

| Tool | Description |
|------|-------------|
| `pdf_read` | Extract text or metadata from a PDF. |
| `pdf_write` | Write a PDF document. |

**Visibility switch:** none — the PDF family is registered and visible by
default. See `docs/howtos/office.md` for the full manual.

---

## pdf_read

Extract text, metadata, or structured per-page JSON from a PDF.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `path` | string | yes | PDF path | `"spec.pdf"` |
| `start_page` | integer | no | First page (1-based, inclusive) | `1` |
| `end_page` | integer | no | Last page (1-based, inclusive) | `10` |
| `format` | string | no | `text` (default), `metadata`, or `json` | `"metadata"` |

**Example:**
```text
pdf_read path="spec.pdf"
pdf_read path="paper.pdf" start_page=3 end_page=7
pdf_read path="report.pdf" format="metadata"
```

---

## pdf_write

Write a PDF document from structured content.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `path` | string | yes | Output path | `"notes.pdf"` |
| `content` | object | yes | Document content: optional `title` and an `elements` array of `paragraph`/`heading`/`table`/`image` items | — |

**Example:**
```text
pdf_write path="report.pdf" content={"title":"Report","elements":[{"type":"heading","level":1,"text":"Summary"},{"type":"paragraph","text":"All checks pass."}]}
```
