# Tools — Office & PDF

Read and write Microsoft Office (DOCX/XLSX/PPTX), LibreOffice (ODT/ODS/ODP),
and PDF documents.

| Tool | Description |
|------|-------------|
| `office_read` | Read DOCX/XLSX/PPTX content. |
| `office_write` | Write Office documents. |
| `office_info` | Return Office document metadata. |
| `libre_read` | Read ODT/ODS/ODP content. |
| `libre_write` | Write LibreOffice documents. |
| `libre_info` | Return LibreOffice document metadata. |
| `pdf_read` | Extract text from a PDF. |
| `pdf_write` | Write a PDF document. |

**Visibility switch:** `office`. See `docs/howtos/office.md` for the full
manual.

---

## office_read / libre_read / pdf_read

Extract the text content of a document.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `path` | string | yes | Document path | `"report.docx"`, `"spec.pdf"` |
| `sheet` | string | no (office_read) | Specific worksheet for XLSX | `"Sheet1"` |

**Example:**
```text
office_read path="report.docx"
pdf_read path="spec.pdf"
```

---

## office_write / libre_write

Write a document. Format follows the file extension.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `path` | string | yes | Output path | `"out.docx"` |
| `content` | string | yes | Document content (plain text / markdown-ish structure) | — |

---

## office_info / libre_info

Return document metadata (title, author, page/sheet counts, etc.).

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `path` | string | yes | Document path |

---

## pdf_write

Write plain-text content as a PDF document.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `path` | string | yes | Output path | `"notes.pdf"` |
| `content` | string | yes | Text content to render | — |
