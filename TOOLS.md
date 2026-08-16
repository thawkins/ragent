# TOOLS.md — ragent Tool Reference

This document lists every built-in tool available to ragent agents, grouped by crate/category.
For each tool the name, permission category, description, JSON input schema, and output format are shown.

---

## Core tools (`ragent-tools-core`)

### `append_to_file` (file:write)
Append text to the end of a file. Creates the file and any missing parent directories if they do not exist. More efficient than a full rewrite when only adding content to the end.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the file to append to"
    },
    "content": {
      "type": "string",
      "description": "Text to append"
    }
  },
  "required": [
    "path",
    "content"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `bash` (shell:execute)
Execute a shell command and return stdout and stderr. Commands are run in the working directory.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "command": {
      "type": "string",
      "description": "Shell command to execute"
    },
    "timeout": {
      "type": "integer",
      "description": "Timeout in seconds (default: 120)"
    }
  },
  "required": [
    "command"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `bash_reset` (shell:execute)
Reset the persistent shell state (clears the saved working directory and environment variables). Use when the shell is in a bad state or you want to start fresh from the agent's working directory.

**Input schema:**
```json
{
  "type": "object",
  "properties": {}
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `calculator` (bash:execute)
Evaluate a mathematical expression and return the result. Supports Python arithmetic, the 'math' module (e.g. math.sqrt, math.pi), and integer/float/complex numbers. Examples: '2 ** 32', 'math.factorial(20)', '(3+4j) * 2'.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "expression": {
      "type": "string",
      "description": "Mathematical expression to evaluate (Python syntax)"
    }
  },
  "required": [
    "expression"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `copy_file` (file:write)
Copy a file to a new location. Creates the destination's parent directories if they do not exist. The source file is not modified.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "source": {
      "type": "string",
      "description": "Path to the source file"
    },
    "destination": {
      "type": "string",
      "description": "Destination path for the copy"
    }
  },
  "required": [
    "source",
    "destination"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `create` (file:write)
Create a new file with content. Truncates the file if it already exists. Creates parent directories if needed.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the file to create"
    },
    "content": {
      "type": "string",
      "description": "Content to write to the new file"
    }
  },
  "required": [
    "path",
    "content"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `diff_files` (file:read)
Show a unified diff between two files. Provide 'path_a' and 'path_b' to compare files on disk. Alternatively, provide 'text_a' and 'text_b' to diff inline strings. Returns a unified-diff style output.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path_a": {
      "type": "string",
      "description": "Path to the first file (left / old)"
    },
    "path_b": {
      "type": "string",
      "description": "Path to the second file (right / new)"
    },
    "text_a": {
      "type": "string",
      "description": "First text string (left / old), alternative to path_a"
    },
    "text_b": {
      "type": "string",
      "description": "Second text string (right / new), alternative to path_b"
    },
    "context_lines": {
      "type": "integer",
      "description": "Number of context lines around changes (default: 3)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `edit` (file:write)
Replace exactly one occurrence of old_string with new_string in a file. old_string must occur exactly once in the file, byte-for-byte — whitespace, indentation, and line endings must match precisely (no CRLF vs LF or trailing-space tolerance). Use an empty old_string with a non-existent file_path to create it; an empty new_string deletes the matched text. Include 3–5 lines of context around the change point so the match is unique. The result includes a line-numbered snippet of the edited region. Pass dry_run: true to preview the change without writing to disk.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "file_path": {
      "type": "string",
      "description": "Absolute path to the file to edit"
    },
    "old_string": {
      "type": "string",
              "description": "String to find and replace (must occur exactly once in the file, byte-for-byte; whitespace and line endings must match precisely). Empty string creates a new file."    },
    "new_string": {
      "type": "string",
      "description": "Replacement string. Empty string deletes the matched text."
    },
    "dry_run": {
      "type": "boolean",
      "description": "If true, resolve the match and return a preview snippet without writing the file."
    }
  },
  "required": [
    "file_path",
    "old_string",
    "new_string"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `file_info` (file:read)
Return metadata for a file or directory: size in bytes, last-modified time (UTC), file type (file/directory/symlink), and whether it exists.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the file or directory"
    }
  },
  "required": [
    "path"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `get_env` (none)
Read the value of one or more environment variables. Sensitive variables (containing KEY, SECRET, TOKEN, PASSWORD, etc.) are redacted. Use 'name' for a single variable or 'names' for a list.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "name": {
      "type": "string",
      "description": "Name of a single environment variable to read"
    },
    "names": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "List of environment variable names to read"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `glob` (file:read)
Find files matching a glob pattern. Recursively searches directories.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "pattern": {
      "type": "string",
      "description": "Glob pattern to match (e.g. '**/*.rs', 'src/**/*.ts')"
    },
    "path": {
      "type": "string",
      "description": "Base directory (default: working directory)"
    }
  },
  "required": [
    "pattern"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `grep` (file:read)
Search file contents for a regex pattern using ripgrep. Respects .gitignore rules. Returns matching lines with file path and line number. Supports regex, case-insensitive search, and file-type glob filtering.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "pattern": {
      "type": "string",
      "description": "Regex pattern to search for (Rust regex syntax)"
    },
    "path": {
      "type": "string",
      "description": "Directory or file to search in (default: working directory)"
    },
    "case_insensitive": {
      "type": "boolean",
      "description": "Case-insensitive matching (default: false)"
    },
    "include": {
      "type": "string",
      "description": "Glob pattern to restrict which files are searched (e.g. '*.rs', '**/*.ts')"
    },
    "exclude": {
      "type": "string",
      "description": "Glob pattern of files/directories to exclude"
    },
    "max_results": {
      "type": "integer",
      "description": "Maximum number of matches to return (default: 500, max: 500)"
    },
    "multiline": {
      "type": "boolean",
      "description": "Enable multiline mode — ^ and $ match line boundaries (default: false)"
    }
  },
  "required": [
    "pattern"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `list` (file:read)
List directory contents with tree-like output. Supports depth control.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Directory path to list (default: working directory)"
    },
    "depth": {
      "type": "integer",
      "description": "Maximum depth to recurse (default: 2)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `make_directory` (file:write)
Create a directory at the given path, including any missing parent directories (equivalent to `mkdir -p`). No-op if the directory already exists.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Directory path to create"
    }
  },
  "required": [
    "path"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `move_file` (file:write)
Move or rename a file or directory. Uses an atomic OS rename so the operation is instant on the same filesystem. Fails if source does not exist or destination's parent directory does not exist.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "source": {
      "type": "string",
      "description": "Path to the file or directory to move"
    },
    "destination": {
      "type": "string",
      "description": "Destination path (including new name)"
    }
  },
  "required": [
    "source",
    "destination"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `multi_edit` (file:write)
Apply multiple edits to one or more files atomically. Each edit replaces exactly one occurrence of old_string with new_string; matching is strict exact-byte (whitespace and line endings must match precisely). All edits are validated before any files are written — if any match fails, no files are modified. Edits to the same file are overlap-checked and applied highest-offset-first so input order does not matter. Each edit object uses file_path, old_string, and new_string.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "edits": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "file_path": {
            "type": "string"
          },
          "old_string": {
            "type": "string"
          },
          "new_string": {
            "type": "string"
          }
        },
        "required": [
          "file_path",
          "old_string",
          "new_string"
        ]
      },
      "description": "Array of edit operations to apply. Each edit is a single-instance exact replacement."
    }
  },
  "required": [
    "edits"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `patch` (file:write)
Apply a unified diff patch to one or more files. The patch must be in unified diff format (as produced by `diff -u` or `git diff`). All hunks are validated before any files are written.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "patch": {
      "type": "string",
      "description": "Unified diff content to apply"
    },
    "fuzz": {
      "type": "integer",
      "description": "Number of context lines that may be dropped from the top/bottom of each hunk when matching (default: 0)"
    },
    "path": {
      "type": "string",
      "description": "Optional: override the target file path (for single-file patches)"
    }
  },
  "required": [
    "patch"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `read` (file:read)
Read file contents. For large files (>100 lines) called without a line range, returns the first 100 lines plus a section map of the file's structure. Use start_line + num_lines to read a specific range. start_line is the 1-based absolute line number where reading begins. num_lines is the COUNT of lines to read from start_line (e.g. start_line=201, num_lines=100 reads lines 201-300). If both end_line and num_lines are provided, end_line takes precedence.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the file to read"
    },
    "start_line": {
      "type": "integer",
      "description": "PREFERRED: 1-based absolute line number where reading begins."
    },
    "num_lines": {
      "type": "integer",
      "description": "PREFERRED: how many lines to read from start_line."
    },
    "end_line": {
      "type": "integer",
      "description": "ADVANCED/LEGACY: 1-based absolute last line number to include."
    }
  },
  "required": [
    "path"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `rm` (file:write)
Delete a single file. Wildcards are not allowed. Fails if the file does not exist.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the file to delete. Must be a single file, no wildcards or glob patterns."
    }
  },
  "required": [
    "path"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `write` (file:write)
Write content to a file. Creates parent directories if needed.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the file to write"
    },
    "content": {
      "type": "string",
      "description": "Content to write to the file"
    }
  },
  "required": [
    "path",
    "content"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `apply_patch` (file:write)
Apply a Codex-style patch to one or more files. The patch must be wrapped in `*** Begin Patch` / `*** End Patch` and contain `*** Add File:`, `*** Delete File:`, or `*** Update File:` operations. Update operations use `@@` hunks with ` ` (context), `+` (add), and `-` (remove) lines. Hunk context lines must match the file byte-for-byte (no CRLF/trailing-whitespace tolerance). All operations are validated before any file is written.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "patch": {
      "type": "string",
      "description": "Codex-style patch content to apply"
    },
    "dry_run": {
      "type": "boolean",
      "description": "When true, validate the patch without writing any files (default: false)"
    },
    "path": {
      "type": "string",
      "description": "Optional: override the base directory for relative paths (default: working directory)"
    }
  },
  "required": [
    "patch"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

---

## Extended tools (`ragent-tools-extended`)

### `browser` (network:fetch)
Browser automation via Chrome DevTools Protocol (CDP). Can open URLs, snapshot pages, click elements, type text, fill forms, select options, wait for conditions, evaluate JavaScript, scroll, upload files, press keys, capture screenshots, check browser status, and launch a headless Chrome/Chromium instance. Requires a running Chrome/Chromium with --remote-debugging-port=9222 (use action=setup to launch one automatically).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "description": "Browser action to perform"
    },
    "url": {
      "type": "string",
      "description": "URL to navigate to (required for action=open)"
    },
    "selector": {
      "type": "string",
      "description": "CSS selector for the target element (click, type, select, upload, wait)"
    },
    "text": {
      "type": "string",
      "description": "Text to type (action=type) or key to press (action=press)"
    },
    "value": {
      "type": "string",
      "description": "Value for select option (action=select)"
    },
    "condition": {
      "type": "string",
      "description": "Wait condition (action=wait)"
    },
    "milliseconds": {
      "type": "integer",
      "description": "Wait duration in milliseconds (action=wait, condition=time)"
    },
    "expression": {
      "type": "string",
      "description": "JavaScript expression to evaluate (action=eval)"
    },
    "file_path": {
      "type": "string",
      "description": "Path to file to upload (action=upload)"
    },
    "full_page": {
      "type": "boolean",
      "description": "Capture full page screenshot (action=screenshot, default: false)"
    },
    "scroll_x": {
      "type": "integer",
      "description": "Horizontal scroll offset (action=scroll, default: 0)"
    },
    "scroll_y": {
      "type": "integer",
      "description": "Vertical scroll offset (action=scroll, default: 0)"
    },
    "headless": {
      "type": "boolean",
      "description": "Run browser in headless mode (action=setup, default: true)"
    },
    "port": {
      "type": "integer",
      "description": "CDP port for setup (action=setup, default: 9222)"
    },
    "wait": {
      "type": "boolean",
      "description": "Wait for page load after navigation (action=open, default: true)"
    },
    "css_selector": {
      "type": "string",
      "description": "CSS selector to narrow snapshot scope (action=snapshot)"
    },
    "fields": {
      "type": "object",
      "description": "Map of CSS selector to value for fill_form (action=fill_form)"
    }
  },
  "required": [
    "action"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `codeindex_dependencies` (codeindex:read)
Query file-level dependencies from the code index. Show what a file imports or what other files depend on it.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Relative file path to query dependencies for"
    },
    "direction": {
      "type": "string",
      "enum": [
        "imports",
        "dependents"
      ],
      "description": "Direction: 'imports' (what this file uses) or 'dependents' (what uses this file)"
    }
  },
  "required": [
    "path"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `codeindex_references` (codeindex:read)
Find all references to a symbol by name across the indexed codebase. Returns file locations grouped by file, with reference kind (call, type, field_access).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "symbol": {
      "type": "string",
      "description": "The symbol name to find references for"
    },
    "limit": {
      "type": "integer",
      "description": "Maximum references to return (default: 50, max: 200)"
    }
  },
  "required": [
    "symbol"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `codeindex_reindex` (codeindex:write)
Trigger a full re-index of the codebase. Scans all files, extracts symbols, and updates the search index.

**Input schema:**
```json
{
  "type": "object",
  "properties": {}
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `codeindex_search` (codeindex:read)
Search the codebase index for symbols, functions, types, and documentation. Uses full-text search with optional filters by kind, language, and file path.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "Search query — symbol name, keyword, or phrase to find in the codebase"
    },
    "file_pattern": {
      "type": "string",
      "description": "Filter by file path substring (e.g. 'src/parser' or '.rs')"
    },
    "kind": {
      "type": "string",
      "enum": [
        "function",
        "struct",
        "enum",
        "trait",
        "impl",
        "const",
        "static",
        "type_alias",
        "module",
        "macro",
        "field",
        "variant",
        "interface",
        "class",
        "method"
      ],
      "description": "Filter by symbol kind"
    },
    "language": {
      "type": "string",
      "description": "Filter by programming language (e.g. 'rust', 'python', 'typescript')"
    },
    "max_results": {
      "type": "integer",
      "description": "Maximum results to return (default: 20, max: 100)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `codeindex_status` (codeindex:read)
Show the current status and statistics of the codebase index — files indexed, symbols extracted, languages, index size, and timestamps.

**Input schema:**
```json
{
  "type": "object",
  "properties": {}
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `codeindex_symbols` (codeindex:read)
Query symbols (functions, structs, enums, traits) from the codebase index. Supports filtering by name, kind, file, language, and visibility.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "name": {
      "type": "string",
      "description": "Filter by symbol name (case-insensitive substring match)"
    },
    "kind": {
      "type": "string",
      "description": "Filter by symbol kind"
    },
    "file_path": {
      "type": "string",
      "description": "Filter by file path (substring match)"
    },
    "language": {
      "type": "string",
      "description": "Filter by language (e.g. 'rust')"
    },
    "visibility": {
      "type": "string",
      "description": "Filter by visibility"
    },
    "limit": {
      "type": "integer",
      "description": "Maximum results (default: 50, max: 200)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `http_request` (network:fetch)
Perform an HTTP request (GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS). Returns the response status code, selected response headers, and the response body (truncated at 1 MiB). For simple web page fetching prefer 'webfetch'; use this tool when you need full control over method/headers/body.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "url": {
      "type": "string",
      "description": "Full URL to request (including scheme, e.g. https://...)"
    },
    "method": {
      "type": "string",
      "description": "HTTP method (default: GET)"
    },
    "headers": {
      "type": "object",
      "description": "Additional request headers as a key-value map"
    },
    "body": {
      "type": "string",
      "description": "Request body (for POST/PUT/PATCH)"
    },
    "timeout": {
      "type": "integer",
      "description": "Timeout in seconds (default: 30)"
    }
  },
  "required": [
    "url"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `libre_info` (file:read)
Get information about a LibreOffice document (pages, words, etc.) without extracting its content.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the document"
    }
  },
  "required": [
    "path"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `libre_read` (file:read)
Read the text content of a LibreOffice document (ODT, ODP, ODS) using LibreOffice in headless mode.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the LibreOffice document"
    },
    "max_content_chars": {
      "type": "integer",
      "description": "Maximum content characters (default 40000)"
    }
  },
  "required": [
    "path"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `libre_write` (file:write)
Create or overwrite a LibreOffice document (ODT, ODP, ODS) with text content using LibreOffice in headless mode.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the document to create or overwrite"
    },
    "content": {
      "type": "string",
      "description": "Text content to write"
    }
  },
  "required": [
    "path",
    "content"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `mf_cache_clear` (network:fetch)
Clear the masterfetch content cache. When all=true, wipes all entries. When all=false (default), purges only expired entries.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "all": {
      "type": "boolean",
      "description": "If true, purge all entries. If false (default), purge only expired entries."
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `mf_crawl` (network:fetch)
Best-first same-domain crawl. Each page as markdown with content_ok + page_type. Supports discover_only, crawl_urls, focus, sitemap mode, and time + token caps.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "url": {
      "type": "string",
      "description": "Start URL for the crawl (http:// or https://)"
    },
    "focus": {
      "type": "string",
      "description": "Query string for scoring and filtering crawled pages"
    },
    "max_pages": {
      "type": "integer",
      "description": "Maximum pages to fetch (default: 10)"
    },
    "max_depth": {
      "type": "integer",
      "description": "Maximum crawl depth from start URL (default: 2)"
    },
    "max_total_chars": {
      "type": "integer",
      "description": "Total character budget across all pages (default: 200000)"
    },
    "deadline_ms": {
      "type": "integer",
      "description": "Time budget in milliseconds (default: 120000)"
    },
    "discover_only": {
      "type": "boolean",
      "description": "Return discovered URL map only without fetching content (default: false)"
    },
    "sitemap": {
      "type": "boolean",
      "description": "Sitemap mode: true = use sitemap, 'auto' = use if available, false = BFS (default: false)"
    },
    "respect_robots": {
      "type": "boolean",
      "description": "Check robots.txt before fetching (default: false)"
    }
  },
  "required": [
    "url"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `mf_fetch` (network:fetch)
Fetch any URL or PDF with automatic content extraction. HTTP first, auto-escalates to stealthy browser if blocked (browser tier not available in integrated Rust runtime — degrades honestly). Supports bulk fetch (urls array), css_selector narrowing, focus query filtering, pagination, and format selection (markdown/html/text/raw).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "url": {
      "type": "string",
      "description": "The URL to fetch (http:// or https://)"
    },
    "urls": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Array of URLs for parallel bulk fetch"
    },
    "format": {
      "type": "string",
      "enum": [
        "markdown",
        "html",
        "text",
        "raw"
      ],
      "description": "Output format (default: markdown)"
    },
    "css_selector": {
      "type": "string",
      "description": "CSS selector to narrow extraction scope"
    },
    "focus": {
      "type": "string",
      "description": "Query string for BM25-focused content filtering (post-extraction, no re-fetch)"
    },
    "max_content_chars": {
      "type": "integer",
      "description": "Maximum content characters (default: 40000, min: 500)"
    },
    "offset": {
      "type": "integer",
      "description": "Character offset to resume from for pagination"
    },
    "respect_robots": {
      "type": "boolean",
      "description": "Check robots.txt before fetching (default: false)"
    },
    "cache_ttl": {
      "type": "integer",
      "description": "Cache TTL in seconds (0 = bypass cache, default: 3600)"
    },
    "include_links": {
      "type": "boolean",
      "description": "Classify outgoing links into citations/navigation/external (default: false)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `mf_search` (network:fetch)
Local keyless web search. Multiple backends run in parallel (DuckDuckGo, Brave, and optional LangSearch / Tavily when their API keys are configured; use `langsearch_api_key` or `tavily_api_key` in config), merges + ranks with cross-engine consensus. Each result carries `relevance_score`, `fetch_relevance` (high/med/low), and `engines_consensus`. Supports `site`, `exclude_sites`, `freshness`, `max_results`, and `page` filters.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "The search query"
    },
    "max_results": {
      "type": "integer",
      "description": "Maximum results to return (1-50, default: 6)"
    },
    "page": {
      "type": "integer",
      "description": "Result page (0-10, default: 0)"
    },
    "site": {
      "type": "string",
      "description": "Restrict results to this domain"
    },
    "exclude_sites": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Domains to exclude from results"
    },
    "freshness": {
      "type": "string",
      "description": "Time filter for results"
    }
  },
  "required": [
    "query"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `mf_version` (network:fetch)
Return the masterfetch integration version, the ragent version, and a brief description of the integrated tool set. This tool does not make network calls.

**Input schema:**
```json
{
  "type": "object",
  "properties": {}
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `office_info` (file:read)
Get information about a Microsoft Office document (pages, words, etc.) without extracting its content.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the document"
    }
  },
  "required": [
    "path"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `office_read` (file:read)
Read the text content of a Microsoft Office document (DOCX, PPTX, XLSX) using python-docx / python-pptx / openpyxl.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the Office document"
    },
    "max_content_chars": {
      "type": "integer",
      "description": "Maximum content characters (default 40000)"
    }
  },
  "required": [
    "path"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `office_write` (file:write)
Create or overwrite a Microsoft Office document (DOCX, PPTX, XLSX) with text content using python-docx / python-pptx / openpyxl.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the document to create or overwrite"
    },
    "content": {
      "type": "string",
      "description": "Text content to write"
    }
  },
  "required": [
    "path",
    "content"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `open` (file:read)
Open or reveal a file, folder, or URL in the desktop environment. On Linux uses xdg-open, on macOS uses open, and on Windows uses start. The 'reveal' action opens the item's parent directory. URL schemes are validated against an allowlist before launching.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "target": {
      "type": "string",
      "description": "File path, folder path, or URL to open"
    },
    "action": {
      "type": "string",
      "description": "How to handle the target: open it (default), reveal its parent directory, or validate and open as a URL"
    }
  },
  "required": [
    "target"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `pdf_read` (file:read)
Read text and metadata from a PDF file using a headless browser or PDF extraction backend.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the PDF file"
    },
    "max_content_chars": {
      "type": "integer",
      "description": "Maximum content characters to return (default 40000)"
    },
    "pages": {
      "type": "string",
      "description": "Optional comma-separated page numbers or ranges, e.g. '1,3-5'"
    }
  },
  "required": [
    "path"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `pdf_write` (file:write)
Create a simple PDF file from text content using a headless browser or PDF generation backend.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the PDF file to create or overwrite"
    },
    "content": {
      "type": "string",
      "description": "Text or HTML content to render into PDF"
    }
  },
  "required": [
    "path",
    "content"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `question` (interactive)
Ask the user a question and wait for their typed response. Use this when you need clarification, prioritisation help, or confirmation before proceeding. When you need a choice from a fixed set, provide the optional `options` parameter as an array of strings, and the user will see a multiple-choice dialog.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "question": {
      "type": "string",
      "description": "The question to ask the user"
    },
    "options": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Optional multiple-choice options"
    }
  },
  "required": [
    "question"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `webfetch` (network:fetch)
Fetch the content of a URL via HTTP GET. HTML is automatically converted to plain text unless format is set to 'raw'.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "url": {
      "type": "string",
      "description": "The URL to fetch"
    },
    "format": {
      "type": "string",
      "enum": [
        "raw",
        "text"
      ],
      "description": "Output format: 'raw' (unchanged), 'text' (HTML→plain text). Default: 'text'"
    },
    "max_length": {
      "type": "integer",
      "description": "Maximum characters to return (default: 50000)"
    },
    "timeout": {
      "type": "integer",
      "description": "Request timeout in seconds (default: 30)"
    }
  },
  "required": [
    "url"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `websearch` (network:fetch)
Search the web and return results with titles, URLs, and snippets. Requires a TAVILY_API_KEY environment variable or 'tavily_api_key' in ragent.json config to be set.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "The search query"
    },
    "num_results": {
      "type": "integer",
      "description": "Number of results to return (default: 5, max: 20)"
    }
  },
  "required": [
    "query"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

---

## Agent runtime tools (`ragent-agent`)

### `bg` (shell:execute)
Manage background shell tasks: spawn, list, status, output, tail, cancel, wait, cleanup. Use this for long-running commands that should continue while the agent does other work.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "description": "Action to perform"
    },
    "command": {
      "type": "string",
      "description": "Shell command to spawn (required for action=spawn)"
    },
    "working_dir": {
      "type": "string",
      "description": "Working directory for spawn (default: session working directory)"
    },
    "task_id": {
      "type": "string",
      "description": "Task id (required for status, output, tail, cancel, wait)"
    },
    "lines": {
      "type": "integer",
      "description": "Number of lines to return for tail (default: 20)"
    },
    "timeout": {
      "type": "integer",
      "description": "Timeout in seconds for wait (default: 60)"
    },
    "completed_only": {
      "type": "boolean",
      "description": "Only cleanup completed/failed/cancelled tasks (default: true)"
    },
    "older_than_minutes": {
      "type": "integer",
      "description": "Cleanup tasks older than this many minutes (default: 60)"
    },
    "session_id": {
      "type": "string",
      "description": "Override session id for list/cleanup (default: current session)"
    },
    "status": {
      "type": "string",
      "description": "Filter by status for list (running/completed/failed/cancelled)"
    },
    "limit": {
      "type": "integer",
      "description": "Maximum number of tasks to return for list (default: 50)"
    }
  },
  "required": [
    "action"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `cancel_agent` (agent:control)
Cancel a running background sub-agent task.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "task_id": {
      "type": "string",
      "description": "Task id to cancel"
    }
  },
  "required": [
    "task_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `conversation_search` (file:read)
Search the current session conversation history. Modes: keyword (default), turn_range, stats. Returns ranked keyword matches with role and timestamp, a slice of messages by turn number, or a statistical summary including total/user/assistant/compaction counts.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "Search query for keyword mode"
    },
    "mode": {
      "type": "string",
      "description": "Search mode (default: keyword)"
    },
    "limit": {
      "type": "integer",
      "description": "Maximum number of results for keyword mode (default: 10)"
    },
    "context_turns": {
      "type": "integer",
      "description": "Number of surrounding turns to include around each keyword match (default: 0)"
    },
    "start_turn": {
      "type": "integer",
      "description": "First turn to include in turn_range mode (1-based, inclusive)"
    },
    "end_turn": {
      "type": "integer",
      "description": "Last turn to include in turn_range mode (1-based, inclusive)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gmail` (network:send)
Search, read, draft, and send Gmail messages using the Gmail REST API. Actions: search (query + max_results), read (id), draft (to, subject, body, cc, bcc), send (to, subject, body, cc, bcc), auth (store OAuth2 access_token or refresh_token + client credentials in encrypted local storage), status (check authentication), logout (remove stored tokens). Requires prior authentication via the auth action.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "description": "Operation to perform"
    },
    "query": {
      "type": "string",
      "description": "Gmail search query, e.g. \"from:ci@example.com is:unread\" (search)"
    },
    "max_results": {
      "type": "integer",
      "description": "Maximum messages to return for search (default 10, max 100)"
    },
    "id": {
      "type": "string",
      "description": "Message id (read)"
    },
    "to": {
      "type": "string",
      "description": "Recipient address (draft/send)"
    },
    "subject": {
      "type": "string",
      "description": "Subject line (draft/send)"
    },
    "body": {
      "type": "string",
      "description": "Plain-text message body (draft/send)"
    },
    "cc": {
      "type": "string",
      "description": "Optional Cc header (draft/send)"
    },
    "bcc": {
      "type": "string",
      "description": "Optional Bcc header (draft/send)"
    },
    "access_token": {
      "type": "string",
      "description": "OAuth2 access token to store (auth)"
    },
    "refresh_token": {
      "type": "string",
      "description": "OAuth2 refresh token to store (auth); enables automatic access-token refresh"
    },
    "client_id": {
      "type": "string",
      "description": "OAuth2 client id stored for refresh-token exchange (auth)"
    },
    "client_secret": {
      "type": "string",
      "description": "OAuth2 client secret stored for refresh-token exchange (auth)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `initiative` (storage:write)
Manage durable initiatives — long-lived project goals with milestones that persist across sessions and compaction. Use for multi-week efforts that should not be forgotten. Actions: create, read, update, checkpoint (record progress / complete a milestone), list, close.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "description": "Operation to perform"
    },
    "id": {
      "type": "string",
      "description": "Initiative id (required for read/update/checkpoint/close)"
    },
    "title": {
      "type": "string",
      "description": "Short goal title (required for create; optional for update)"
    },
    "description": {
      "type": "string",
      "description": "Detailed description / success criteria"
    },
    "milestones": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Milestone titles for create"
    },
    "milestone": {
      "type": "string",
      "description": "Milestone id to mark complete (checkpoint action)"
    },
    "note": {
      "type": "string",
      "description": "Free-text note recorded with a checkpoint"
    },
    "progress": {
      "type": "integer",
      "description": "Overall progress 0-100 (update/checkpoint)"
    },
    "status": {
      "type": "string",
      "description": "New status for update/close; filter for list (default: active)"
    },
    "limit": {
      "type": "integer",
      "description": "Maximum initiatives returned by list (default: 50)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `list_agents` (none)
List sub-agent tasks for the current session (running and completed).

**Input schema:**
```json
{
  "type": "object",
  "properties": {}
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `mcp_tool` (mcp)
Generic wrapper that invokes a tool on an external Model Context Protocol (MCP) server. The server and tool name are encoded in the dynamic tool name `mcp_{safe_server}_{safe_tool}`.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "arguments": {
      "type": "object",
      "description": "Arguments to pass to the MCP tool"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `memory_forget` (storage:write)
Delete structured memories by ID or by filter criteria. At least one criterion (id, older_than_days, max_confidence, category, or tags) is required.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "id": {
      "type": "integer",
      "description": "Delete a specific memory by its row ID"
    },
    "older_than_days": {
      "type": "integer",
      "description": "Delete memories not updated in this many days"
    },
    "max_confidence": {
      "type": "number",
      "description": "Delete memories with confidence at or below this value"
    },
    "category": {
      "type": "string",
      "description": "Delete memories in this category"
    },
    "tags": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Delete memories that have ALL of these tags"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `memory_recall` (file:read)
Search structured memories using full-text search with optional category, tag, and confidence filters.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "Full-text search query (space-separated terms, all must match)"
    },
    "categories": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Filter to these categories"
    },
    "tags": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Filter to memories that have ALL of these tags"
    },
    "min_confidence": {
      "type": "number",
      "description": "Minimum confidence threshold 0.0–1.0 (default: 0.5)"
    },
    "limit": {
      "type": "integer",
      "description": "Maximum results (default: 5)"
    }
  },
  "required": [
    "query"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `memory_store` (storage:write)
Store a structured memory with a category, tags, and confidence score. Categories: fact, pattern, preference, insight, error, workflow. Stored memories can be searched with memory_recall and deleted with memory_forget.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "content": {
      "type": "string",
      "description": "The memory content — a fact, pattern, insight, etc."
    },
    "category": {
      "type": "string",
      "description": "Category of the memory"
    },
    "tags": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Tags for filtering"
    },
    "confidence": {
      "type": "number",
      "description": "Confidence score 0.0–1.0 (default: 0.7)"
    },
    "source": {
      "type": "string",
      "description": "Source of the memory (e.g., 'manual', tool name)"
    }
  },
  "required": [
    "content",
    "category"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `new_agent` (agent:spawn)
Spawn a sub-agent to perform a focused task. Requires both 'agent' and 'task' parameters.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "agent": {
      "type": "string",
      "description": "Sub-agent type to spawn"
    },
    "task": {
      "type": "string",
      "description": "Task description"
    }
  },
  "required": [
    "agent",
    "task"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `plan_enter` (plan)
Delegate to the plan agent for read-only codebase analysis.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "request": {
      "type": "string",
      "description": "Planning request"
    }
  },
  "required": [
    "request"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `send_channel_message` (network:send)
Send a message to a configured external messaging channel (Telegram bot or Discord webhook). Channels are configured in ragent.json under the "channels" block.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "message": {
      "type": "string",
      "description": "Message text to deliver (required for send; max 4096 bytes)"
    },
    "action": {
      "type": "string",
      "description": "Operation to perform (default: send)"
    },
    "channel": {
      "type": "string",
      "description": "Channel targeting: telegram/discord/all/first configured"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `session_search` (file:read)
Search across all past sessions for messages matching a query. Supports filters for date range, working directory, role, per-session limits, and optional surrounding context.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "FTS5 keyword search"
    },
    "limit": {
      "type": "integer",
      "description": "Maximum total results to return (default: 10)"
    },
    "max_per_session": {
      "type": "integer",
      "description": "Maximum results to return from any single session"
    },
    "roles": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Filter to specific roles, e.g. [\"user\", \"assistant\"]"
    },
    "working_dir": {
      "type": "string",
      "description": "Restrict search to sessions created in this working directory"
    },
    "since": {
      "type": "string",
      "description": "ISO-8601 timestamp; only include messages created on or after this time"
    },
    "until": {
      "type": "string",
      "description": "ISO-8601 timestamp; only include messages created on or before this time"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `skill_manage` (skill:manage)
Manage the skill registry at runtime: list available skills, read a skill's prompt, load (discover + invoke) a skill by name, or reload all skills from disk.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "description": "Operation to perform"
    },
    "name": {
      "type": "string",
      "description": "Skill name (required for read/load)"
    },
    "arguments": {
      "type": "string",
      "description": "Arguments substituted into the skill body"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `spec_coverage` (spec:read)
Generate a requirement coverage report for a spec. Shows which requirements are linked to completed tasks.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "spec_id": {
      "type": "string",
      "description": "The spec identifier"
    }
  },
  "required": [
    "spec_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `spec_list` (spec:read)
List all specifications. Optionally filter by status.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "status": {
      "type": "string",
      "description": "Filter by spec status (optional)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `spec_read` (spec:read)
Read a specification by ID. Returns the full SPEC.md content, requirements, tasks, and current status.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "spec_id": {
      "type": "string",
      "description": "The spec identifier (e.g. 'testspec', 'auth-refactor')"
    }
  },
  "required": [
    "spec_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `spec_search` (spec:read)
Search all specifications by keyword. Returns matching specs with context snippets.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "Search query string"
    }
  },
  "required": [
    "query"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `spec_task_update` (spec:write)
Update the status of a task within a spec. Statuses: pending, in_progress, completed, blocked.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "spec_id": {
      "type": "string",
      "description": "The spec identifier"
    },
    "task_id": {
      "type": "string",
      "description": "The task identifier within the plan (e.g. 'T-001')"
    },
    "status": {
      "type": "string",
      "description": "New status: pending, in_progress, completed, blocked"
    }
  },
  "required": [
    "spec_id",
    "task_id",
    "status"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `agent_complete` (agent:control)
Signal that the current autonomous task is done; ends the session loop and returns control to the user.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "summary": {
      "type": "string",
      "description": "Summary of what was accomplished"
    }
  },
  "required": [
    "summary"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_approve_plan` (team:manage)
Approve a plan submitted by a teammate.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "teammate_name": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "teammate_name"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_assign_task` (team:manage)
Assign a task to a teammate.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "teammate_name": {
      "type": "string"
    },
    "task_id": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "teammate_name",
    "task_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_broadcast` (team:communicate)
Broadcast a message to all teammates in a team.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "message": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "message"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_cleanup` (team:manage)
Remove all completed teams and their data.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_create` (team:manage)
Create a new named team.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "goal": {
      "type": "string"
    },
    "context": {
      "type": "string",
      "description": "REQUIRED: The specific work context from the user's request — which files/directories to target, what to produce, where to write output. This is prepended to every teammate's spawn prompt so they know exactly what to work on."
    }
  },
  "required": [
    "team_name",
    "context"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_idle` (team:communicate)
Signal that the teammate is idle.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "teammate_name": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "teammate_name"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_memory_read` (team:communicate)
Read a memory entry from a team's shared memory.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "key": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "key"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_memory_write` (team:communicate)
Write a memory entry to a team's shared memory.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "key": {
      "type": "string"
    },
    "value": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "key",
    "value"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_message` (team:communicate)
Send a message to a teammate or the team mailbox.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "teammate_name": {
      "type": "string"
    },
    "message": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "message"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_read_messages` (team:communicate)
Read messages from the team mailbox.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    }
  },
  "required": [
    "team_name"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_shutdown_ack` (team:communicate)
Acknowledge a shutdown signal.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "teammate_name": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "teammate_name"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_shutdown_teammate` (team:manage)
Signal a teammate to shut down.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "teammate_name": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "teammate_name"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_spawn` (team:manage)
Spawn a new teammate in a team.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "teammate_name": {
      "type": "string"
    },
    "agent_type": {
      "type": "string"
    },
    "prompt": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "teammate_name",
    "agent_type",
    "prompt"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_status` (team:read)
Get status of a team.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    }
  },
  "required": [
    "team_name"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_submit_plan` (team:communicate)
Submit a plan for approval.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "teammate_name": {
      "type": "string"
    },
    "plan": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "teammate_name",
    "plan"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_task_claim` (team:tasks)
Claim a task from the team's shared task list.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "teammate_name": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "teammate_name"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_task_complete` (team:tasks)
Mark a team task as completed.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "task_id": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "task_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_task_create` (team:manage)
Create a new task in a team.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    },
    "description": {
      "type": "string"
    }
  },
  "required": [
    "team_name",
    "description"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_task_list` (team:read)
List tasks in a team.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    }
  },
  "required": [
    "team_name"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `team_wait` (agent:spawn)
Block until spawned teammates finish.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "team_name": {
      "type": "string"
    }
  },
  "required": [
    "team_name"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `think` (none)
Record a short reasoning note without changing project state.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "thought": {
      "type": "string",
      "description": "Short reasoning note"
    }
  },
  "required": [
    "thought"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `task_list` (none)
List all tasks for the current session, optionally filtered by status.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "status": {
      "type": "string",
      "description": "Filter by status: pending, in_progress, completed, or all (default: all)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `update_file` (file:write)
Alias for `write`. Write content to an existing file. Creates parent directories if needed.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "path": {
      "type": "string",
      "description": "Path to the file to write"
    },
    "content": {
      "type": "string",
      "description": "Content to write to the file"
    }
  },
  "required": [
    "path",
    "content"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `wait_agents` (agent:control)
Block until one or more background sub-agent tasks complete.

**Input schema:**
```json
{
  "type": "object",
  "properties": {}
}
```
**Output:** Human-readable result string (and optional structured metadata).

---

## VCS tools (`ragent-tools-vcs`)

### `git_add` (git:write)
Stage files for the next commit. Provide file paths or use 'all' to stage all changes. Use 'update' to stage changes only to tracked files.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "paths": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Files or directories to stage"
    },
    "all": {
      "type": "boolean",
      "description": "Stage all changes (git add -A). Overrides paths. (default: false)"
    },
    "update": {
      "type": "boolean",
      "description": "Stage changes only to tracked files (git add -u). Overrides paths. (default: false)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_branch` (git:read)
List branches. Shows current branch, local branches, and optionally remote branches with tracking info.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "all": {
      "type": "boolean",
      "description": "Include remote branches (default: true)"
    },
    "format": {
      "type": "string",
      "description": "Output format: 'short' or 'verbose' with tracking info (default: short)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_checkout` (git:write)
Switch branches or restore working tree files. Provide 'branch' to switch branches. Set 'create_branch' to create and switch to a new branch. Provide 'paths' with optional 'source' to restore specific files from a ref (default: HEAD).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "branch": {
      "type": "string",
      "description": "Branch name to switch to. Ignored when 'paths' is provided."
    },
    "create_branch": {
      "type": "boolean",
      "description": "Create and checkout a new branch (git checkout -b). Requires 'branch'. (default: false)"
    },
    "paths": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Files or directories to restore from 'source'."
    },
    "source": {
      "type": "string",
      "description": "Ref to restore files from (default: HEAD). Only used with 'paths'."
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_cherry_pick` (git:write)
Apply the changes introduced by specific commits onto the current branch. Provide commit hashes. Set 'no_commit' to apply changes without creating a commit.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "commits": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Commit hashes to cherry-pick (required)"
    },
    "no_commit": {
      "type": "boolean",
      "description": "Apply changes without committing (git cherry-pick -n) (default: false)"
    }
  },
  "required": [
    "commits"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_clone` (git:write)
Clone a git repository into a new directory. The clone is created inside the working directory. Optional: specify branch, shallow clone depth, or create a bare clone.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "url": {
      "type": "string",
      "description": "Repository URL to clone (required)"
    },
    "directory": {
      "type": "string",
      "description": "Target directory name (default: inferred from URL)"
    },
    "branch": {
      "type": "string",
      "description": "Branch to checkout after clone (--branch)"
    },
    "depth": {
      "type": "integer",
      "description": "Shallow clone depth (--depth)"
    },
    "bare": {
      "type": "boolean",
      "description": "Create a bare repository (--bare) (default: false)"
    }
  },
  "required": [
    "url"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_commit` (git:write)
Create a new commit. Requires a commit message. Use 'all' to stage all modified tracked files before committing. Use 'amend' to amend the previous commit. Use 'no_verify' to bypass pre-commit hooks.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "message": {
      "type": "string",
      "description": "Commit message (required)"
    },
    "all": {
      "type": "boolean",
      "description": "Stage all modified tracked files before committing (git commit -a) (default: false)"
    },
    "amend": {
      "type": "boolean",
      "description": "Amend the previous commit (git commit --amend) (default: false)"
    },
    "no_verify": {
      "type": "boolean",
      "description": "Bypass pre-commit hooks (git commit --no-verify) (default: false)"
    }
  },
  "required": [
    "message"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_diff` (git:read)
Show changes (diff) between working tree, staged index, or commits. Target: 'working' (default), 'staged', or a commit ref. Optional: path filter, stat summary.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "target": {
      "type": "string",
      "description": "What to diff: 'working' (default), 'staged', or a commit ref"
    },
    "path": {
      "type": "string",
      "description": "Limit diff to a specific file or directory"
    },
    "stat": {
      "type": "boolean",
      "description": "Show stat summary instead of full diff (default: false)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_fetch` (git:write)
Fetch from a remote repository without merging. Use 'prune' to remove deleted remote branches. Use 'all' to fetch all remotes.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "remote": {
      "type": "string",
      "description": "Remote name (default: origin)"
    },
    "branch": {
      "type": "string",
      "description": "Specific branch or ref to fetch"
    },
    "prune": {
      "type": "boolean",
      "description": "Prune deleted remote branches (git fetch --prune) (default: false)"
    },
    "all": {
      "type": "boolean",
      "description": "Fetch all remotes (git fetch --all) (default: false)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_log` (git:read)
Show the commit history. Optional: limit, branch, oneline, author, since.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "limit": {
      "type": "integer",
      "description": "Number of commits to show (default: 20)"
    },
    "branch": {
      "type": "string",
      "description": "Branch or ref to log (default: current branch)"
    },
    "oneline": {
      "type": "boolean",
      "description": "Use one-line format (default: true)"
    },
    "author": {
      "type": "string",
      "description": "Filter by author name or email"
    },
    "since": {
      "type": "string",
      "description": "Show commits newer than this date (e.g. 2024-01-01 or 1.week)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_merge` (git:write)
Merge a branch into the current branch. If there are conflicts, the tool reports the conflicted files and suggests running git_status next.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "branch": {
      "type": "string",
      "description": "Branch to merge into the current branch (required)"
    },
    "no_ff": {
      "type": "boolean",
      "description": "Create a merge commit even if fast-forward is possible (default: false)"
    },
    "ff_only": {
      "type": "boolean",
      "description": "Abort if not a fast-forward merge (default: false)"
    },
    "squash": {
      "type": "boolean",
      "description": "Squash all changes into a single commit (default: false)"
    },
    "message": {
      "type": "string",
      "description": "Custom merge commit message"
    }
  },
  "required": [
    "branch"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_pull` (git:write)
Fetch and integrate changes from a remote repository. Use 'rebase' to rebase instead of merging.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "remote": {
      "type": "string",
      "description": "Remote name (default: origin)"
    },
    "branch": {
      "type": "string",
      "description": "Branch to pull (default: current tracking branch)"
    },
    "rebase": {
      "type": "boolean",
      "description": "Rebase instead of merge (git pull --rebase) (default: false)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_push` (git:write)
Push branches and tags to a remote repository. CAUTION: force push can rewrite history. Use 'force' with care.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "remote": {
      "type": "string",
      "description": "Remote name (default: origin)"
    },
    "branch": {
      "type": "string",
      "description": "Branch to push (default: current branch)"
    },
    "force": {
      "type": "boolean",
      "description": "Force push with lease (git push --force-with-lease) (default: false)"
    },
    "tags": {
      "type": "boolean",
      "description": "Push all tags (git push --tags) (default: false)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_remote` (git:write)
List, add, remove, or update git remotes. Action: 'list' (default), 'add', 'remove', 'set-url'. Caution: 'add', 'remove', and 'set-url' modify repository configuration.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "enum": [
        "list",
        "add",
        "remove",
        "set-url"
      ],
      "description": "Action to perform (default: list)"
    },
    "name": {
      "type": "string",
      "description": "Remote name (for add/remove/set-url)"
    },
    "url": {
      "type": "string",
      "description": "Remote URL (for add/set-url)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_reset` (git:write)
Unstage files or reset the repository to a specific commit. CAUTION: mode 'hard' discards all local changes. Modes: 'soft' (keep changes staged), 'mixed' (keep changes unstaged, default), 'hard' (discard changes), 'keep' (discard changes but abort if overridden). Provide 'paths' to unstage specific files without resetting commits.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "mode": {
      "type": "string",
      "enum": [
        "soft",
        "mixed",
        "hard",
        "keep"
      ],
      "description": "Reset mode (default: mixed). Ignored when 'paths' is provided."
    },
    "target": {
      "type": "string",
      "description": "Commit ref to reset to (default: HEAD). Ignored when 'paths' is provided."
    },
    "paths": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Specific files to unstage. When provided, mode and target are ignored."
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_show` (git:read)
Show details of a commit, tag, or other git object. Shows author, date, message, and file statistics.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "ref": {
      "type": "string",
      "description": "Commit hash, tag, or ref to show (default: HEAD)"
    },
    "stat": {
      "type": "boolean",
      "description": "Include file change statistics (default: true)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_stash` (git:write)
Stash and unstash changes. Actions: 'push' (save changes, default), 'pop' (apply and remove latest), 'apply' (apply without removing), 'drop' (remove a stash), 'list' (show all stashes), 'clear' (remove all stashes).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "description": "Stash action to perform (default: push)"
    },
    "message": {
      "type": "string",
      "description": "Message for the stash (only used with 'push')"
    },
    "index": {
      "type": "integer",
      "description": "Stash index (for pop/apply/drop, default: 0)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_status` (git:read)
Show the working tree status: modified, staged, untracked, and conflicted files.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "branch": {
      "type": "boolean",
      "description": "Include branch name in output (default: true)"
    },
    "short": {
      "type": "boolean",
      "description": "Use short format (default: false)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `git_tag` (git:write)
List, show, create, or delete git tags. Action: 'list' (default), 'show', 'create', 'delete'. Caution: 'create' and 'delete' modify the repository.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "action": {
      "type": "string",
      "enum": [
        "list",
        "show",
        "create",
        "delete"
      ],
      "description": "Action to perform (default: list)"
    },
    "name": {
      "type": "string",
      "description": "Tag name (for show/create/delete)"
    },
    "ref": {
      "type": "string",
      "description": "Target commit or ref (for create, default: HEAD)"
    },
    "message": {
      "type": "string",
      "description": "Annotated tag message (for create)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `github_get_actions` (github:read)
List the last N GitHub Actions workflow runs for the current repository (default 1, max 30). Reports each run's status (OK/Failed). For failed runs, downloads the log archive and shows error context.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "limit": {
      "type": "integer",
      "description": "Number of recent workflow runs to inspect (default 1, max 30)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `github_get_issue` (github:read)
Get details of a specific GitHub issue including body, comments, and labels.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "number": {
      "type": "integer",
      "description": "Issue number"
    }
  },
  "required": [
    "number"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `github_get_pr` (github:read)
Get details of a specific GitHub pull request including body, comments, and checks.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "number": {
      "type": "integer",
      "description": "Pull request number"
    }
  },
  "required": [
    "number"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `github_list_issues` (github:read)
List GitHub issues for the current repository. Optional: state (open/closed/all), labels (comma-separated), limit (default 20).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "state": {
      "type": "string",
      "enum": [
        "open",
        "closed",
        "all"
      ],
      "description": "Filter by issue state (default: open)"
    },
    "labels": {
      "type": "string",
      "description": "Comma-separated label names to filter by"
    },
    "limit": {
      "type": "integer",
      "description": "Max issues to return (default 20, max 100)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `github_list_prs` (github:read)
List GitHub pull requests for the current repository. Optional: state (open/closed/all), limit (default 20).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "state": {
      "type": "string",
      "enum": [
        "open",
        "closed",
        "all"
      ],
      "description": "Filter by PR state (default: open)"
    },
    "limit": {
      "type": "integer",
      "description": "Max PRs to return (default 20, max 100)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `github_create_issue` (github:write)
Create a new GitHub issue in the current repository.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "title": {
      "type": "string",
      "description": "Issue title"
    },
    "body": {
      "type": "string",
      "description": "Issue body (markdown supported)"
    },
    "labels": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "description": "Label names to apply"
    }
  },
  "required": [
    "title"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `github_comment_issue` (github:write)
Add a comment to a GitHub issue.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "number": {
      "type": "integer",
      "description": "Issue number"
    },
    "body": {
      "type": "string",
      "description": "Comment body (markdown supported)"
    }
  },
  "required": [
    "number",
    "body"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `github_close_issue` (github:write)
Close a GitHub issue (optionally with a comment).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "number": {
      "type": "integer",
      "description": "Issue number"
    },
    "comment": {
      "type": "string",
      "description": "Optional closing comment"
    }
  },
  "required": [
    "number"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `github_create_pr` (github:write)
Create a new GitHub pull request in the current repository.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "title": {
      "type": "string",
      "description": "PR title"
    },
    "body": {
      "type": "string",
      "description": "PR body (markdown supported)"
    },
    "head": {
      "type": "string",
      "description": "Branch containing changes"
    },
    "base": {
      "type": "string",
      "description": "Branch to merge into (default: main)"
    },
    "draft": {
      "type": "boolean",
      "description": "Create as draft PR (default: false)"
    }
  },
  "required": [
    "title",
    "head"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `github_merge_pr` (github:write)
Merge a GitHub pull request.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "number": {
      "type": "integer",
      "description": "Pull request number"
    },
    "merge_method": {
      "type": "string",
      "enum": [
        "merge",
        "squash",
        "rebase"
      ],
      "description": "Merge method (default: merge)"
    }
  },
  "required": [
    "number"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `github_review_pr` (github:write)
Submit a review on a GitHub pull request.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "number": {
      "type": "integer",
      "description": "Pull request number"
    },
    "body": {
      "type": "string",
      "description": "Review body (markdown supported)"
    },
    "event": {
      "type": "string",
      "enum": [
        "APPROVE",
        "REQUEST_CHANGES",
        "COMMENT"
      ],
      "description": "Review event (default: COMMENT)"
    }
  },
  "required": [
    "number"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_approve_mr` (gitlab:write)
Approve a GitLab merge request.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "iid": {
      "type": "integer",
      "description": "Merge request IID"
    }
  },
  "required": [
    "iid"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_cancel_job` (gitlab:write)
Cancel a running or pending GitLab CI/CD job.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "job_id": {
      "type": "integer",
      "description": "The job ID to cancel"
    }
  },
  "required": [
    "job_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_cancel_pipeline` (gitlab:write)
Cancel a running GitLab CI/CD pipeline (cancels all pending and running jobs).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "pipeline_id": {
      "type": "integer",
      "description": "The pipeline ID to cancel"
    }
  },
  "required": [
    "pipeline_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_close_issue` (gitlab:write)
Close a GitLab issue (optionally with a note).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "iid": {
      "type": "integer",
      "description": "Issue IID"
    },
    "comment": {
      "type": "string",
      "description": "Optional closing note"
    }
  },
  "required": [
    "iid"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_comment_issue` (gitlab:write)
Add a note (comment) to a GitLab issue.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "iid": {
      "type": "integer",
      "description": "Issue IID"
    },
    "body": {
      "type": "string",
      "description": "Note body (markdown supported)"
    }
  },
  "required": [
    "iid",
    "body"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_create_issue` (gitlab:write)
Create a new GitLab issue in the current project.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "title": {
      "type": "string",
      "description": "Issue title"
    },
    "description": {
      "type": "string",
      "description": "Issue description (markdown supported)"
    },
    "labels": {
      "type": "string",
      "description": "Comma-separated label names"
    },
    "assignee_ids": {
      "type": "string",
      "description": "Comma-separated user IDs to assign"
    }
  },
  "required": [
    "title"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_create_mr` (gitlab:write)
Create a new GitLab merge request in the current project.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "title": {
      "type": "string",
      "description": "MR title"
    },
    "source_branch": {
      "type": "string",
      "description": "Branch containing changes"
    },
    "target_branch": {
      "type": "string",
      "description": "Branch to merge into (default: main)"
    },
    "description": {
      "type": "string",
      "description": "MR description (markdown supported)"
    }
  },
  "required": [
    "title",
    "source_branch"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_get_issue` (gitlab:read)
Get details of a specific GitLab issue including description, notes, and labels.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "iid": {
      "type": "integer",
      "description": "Issue IID (project-scoped number)"
    }
  },
  "required": [
    "iid"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_get_job` (gitlab:read)
Get details of a specific GitLab CI/CD job.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "job_id": {
      "type": "integer",
      "description": "The job ID"
    }
  },
  "required": [
    "job_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_get_job_log` (gitlab:read)
Download the log output of a GitLab CI/CD job. Returns the last N lines of the job trace (default 200, max 2000).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "job_id": {
      "type": "integer",
      "description": "The job ID"
    },
    "tail": {
      "type": "integer",
      "description": "Number of lines from the end to return (default 200, max 2000)"
    }
  },
  "required": [
    "job_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_get_mr` (gitlab:read)
Get details of a specific GitLab merge request including description, notes, and diff stats.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "iid": {
      "type": "integer",
      "description": "Merge request IID (project-scoped number)"
    }
  },
  "required": [
    "iid"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_get_pipeline` (gitlab:read)
Get details of a specific GitLab CI/CD pipeline.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "pipeline_id": {
      "type": "integer",
      "description": "The pipeline ID"
    }
  },
  "required": [
    "pipeline_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_list_issues` (gitlab:read)
List GitLab issues for the current project. Optional: state (opened/closed/all), labels (comma-separated), limit (default 20).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "state": {
      "type": "string",
      "enum": [
        "opened",
        "closed",
        "all"
      ],
      "description": "Filter by issue state (default: opened)"
    },
    "labels": {
      "type": "string",
      "description": "Comma-separated label names to filter by"
    },
    "limit": {
      "type": "integer",
      "description": "Max issues to return (default 20, max 100)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_list_jobs` (gitlab:read)
List CI/CD jobs for a GitLab pipeline.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "pipeline_id": {
      "type": "integer",
      "description": "The pipeline ID"
    }
  },
  "required": [
    "pipeline_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_list_mrs` (gitlab:read)
List GitLab merge requests for the current project. Optional: state (opened/closed/all), limit (default 20).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "state": {
      "type": "string",
      "enum": [
        "opened",
        "closed",
        "all"
      ],
      "description": "Filter by MR state (default: opened)"
    },
    "limit": {
      "type": "integer",
      "description": "Max MRs to return (default 20, max 100)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_list_pipelines` (gitlab:read)
List GitLab CI/CD pipelines for the current project. Optional: status filter, limit (default 20).

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "status": {
      "type": "string",
      "description": "Filter by pipeline status"
    },
    "limit": {
      "type": "integer",
      "description": "Max pipelines to return (default 20, max 100)"
    }
  }
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_merge_mr` (gitlab:write)
Merge a GitLab merge request.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "iid": {
      "type": "integer",
      "description": "Merge request IID"
    }
  },
  "required": [
    "iid"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_retry_job` (gitlab:write)
Retry a failed or cancelled GitLab CI/CD job.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "job_id": {
      "type": "integer",
      "description": "The job ID to retry"
    }
  },
  "required": [
    "job_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_retry_pipeline` (gitlab:write)
Retry all failed jobs in a GitLab CI/CD pipeline.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "pipeline_id": {
      "type": "integer",
      "description": "The pipeline ID to retry"
    }
  },
  "required": [
    "pipeline_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

### `gitlab_get_pipeline` (gitlab:read)
Get details of a specific GitLab CI/CD pipeline.

**Input schema:**
```json
{
  "type": "object",
  "properties": {
    "pipeline_id": {
      "type": "integer",
      "description": "The pipeline ID"
    }
  },
  "required": [
    "pipeline_id"
  ]
}
```
**Output:** Human-readable result string (and optional structured metadata).

---

## Tool aliases and dynamic tools

- `update_file` → alias for the core `write` tool.
- MCP tools are registered dynamically as `mcp_{safe_server}_{safe_tool}` and use the `mcp` permission category.
