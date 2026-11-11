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

Each `content.elements` item:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `type` | enum | yes | `paragraph`, `heading`, `table`, `image` |
| `text` | string | no | Text content (`paragraph`, `heading`) |
| `level` | integer | no | Heading level 1-3 (`heading`, default 1) |
| `headers` | array | no | Column headers (`table`) |
| `rows` | array | no | Rows, each an array of cell strings (`table`) |
| `image_path` | string | yes, for `image` | PNG or JPEG file. An `image` element without it fails the whole `pdf_write` call |
| `width_mm` | number | no | Display width in mm (`image`, default: fit page width) |
| `caption` | string | no | Caption below the image (`image`) |

**Example:**
```text
pdf_write path="report.pdf" content={"title":"Report","elements":[{"type":"heading","level":1,"text":"Summary"},{"type":"paragraph","text":"All checks pass."}]}
```
