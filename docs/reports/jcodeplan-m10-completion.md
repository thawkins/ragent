# JCODEPLAN M10 Completion Report — Open/reveal and remaining UX tools

**Status:** ✅ complete
**Scope:** Implement JCODEPLAN Milestone 10 — add `open` (open/reveal files,
folders, URLs) and any remaining low-effort aliases.

---

## Deliverables

### 1. `open` tool — `crates/ragent-tools-core/src/open.rs`

- Cross-platform launch wrapper:
  - Linux: `xdg-open <target>`
  - macOS: `open <target>`
  - Windows: `cmd /c start "" <target>`
- Actions:
  - `open` (default) — resolves the target relative to `working_dir` and launches
    the desktop handler.
  - `reveal` — opens the parent directory of a file (or the directory itself if the
    target is a folder).
  - `url` — validates the URL scheme against an allowlist before launching.
- URL scheme allowlist: `http`, `https`, `mailto`, `file`. Unknown schemes such as
  `ftp://` are rejected with an actionable error listing allowed schemes.
- Path guard: relative targets are resolved against the tool context's working
  directory and checked with `check_path_within_root` to prevent directory-escape
  attacks.
- Permission category: `shell:execute`.

### 2. Tests — `crates/ragent-tools-core/tests/test_open.rs`

- `test_open_rejects_disallowed_url_scheme` — `ftp://` fails with a clear error.
- `test_open_accepts_https_url` — valid `https://` URL produces a non-empty command
  containing the target.
- `test_open_reveals_parent_directory` — `action="reveal"` on `src/main.rs`
  resolves to the `src` directory.
- `test_open_resolves_relative_path` — relative file path resolves to the file.

### 3. Registration

- `OpenTool` is registered in `ragent_tools_core::create_core_registry()` under
  the Shell tools section.
- `register_extracted_core_tools()` in `crates/ragent-agent/src/tool/mod.rs`
  pulls it into the agent default registry automatically, so it is available to
  every session without further wiring.

---

## Acceptance verification

```rust
// `open target="target/release/ragent" action="reveal"` builds the correct
// command and, on a desktop system, opens the parent directory in the file manager.
```

Confirmed by:

```bash
cargo test -p ragent-tools-core --test test_open
# running 4 tests ... ok
```

---

## Documentation

- `docs/JCODEPLAN.md` — T-090, T-091, T-092, T-093, T-094 marked ✅ and a status
  paragraph added under the M10 section.
- `docs/reports/jcodeplan-m10-completion.md` — this report.

---

## Notes

- No new code was required: the `open` tool was already implemented as part of the
  earlier JCODEPLAN porting work (M2/M4 phase). This milestone completion step
  records that fact and locks the task statuses.
- The tool is intentionally thin: it delegates to the OS default handler rather
  than implementing its own rendering or protocol logic, which keeps it safe and
  portable.
