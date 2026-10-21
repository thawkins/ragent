# Code Index — Module / Function / 1000-Line Counting Specification

**Version:** 1.0  
**Status:** Active — source-of-truth for any scan, report, or remediation that references "a 1000-line function" or "a 1000-line module"  
**Owner:** ragent team  
**Applies to:** `function_analyzer`, code-index scanner, `FUNCPLAN`, `PERFPLAN`, and any ad-hoc audit script

---

## 1. Definitions

### 1.1 Module

A **module** is a single source file that compiles as one translation unit. One file = one module. Never a package, directory, or build target.

| Language | Module = | File extensions |
|----------|----------|----------------|
| Python | one `.py` file | `.py` |
| JavaScript / TypeScript | one `.js`/`.ts`/`.jsx`/`.tsx`/`.mjs`/`.cjs`/`.mts`/`.cts` file | `.js`, `.ts`, `.jsx`, `.tsx`, `.mjs`, `.cjs`, `.mts`, `.cts` |
| Go | one `.go` file | `.go` |
| Java | one `.java` file | `.java` |
| Rust | one `.rs` file | `.rs` |
| Kotlin | one `.kt`/`.kts` file | `.kt`, `.kts` |
| Ruby | one `.rb` file | `.rb` |
| PHP | one `.php` file | `.php` |
| C | one `.c`/`.h` file | `.c`, `.h` |
| C++ | one `.cc`/`.cpp`/`.cxx`/`.hpp`/`.hh`/`.h` file | `.cc`, `.cpp`, `.cxx`, `.hpp`, `.hh`, `.h` |
| C# | one `.cs` file | `.cs` |
| Swift | one `.swift` file | `.swift` |
| Shell | one `.sh`/`.bash`/`.zsh` file | `.sh`, `.bash`, `.zsh` |
| SQL / Terraform / HCL | one file of that language | `.sql`, `.tf`, `.hcl` |

Notes:

- A `.d.ts` declaration-only TypeScript file is **not** a module (it contains no executable code). Skip it (see §5).
- A `__init__.py` that contains only re-exports still counts as a module; it just rarely violates the threshold.
- Generated interface/header files that live in `build/` or `target/` are excluded by path (see §5), not by extension.

### 1.2 Function

A **function** is any named callable with a body that the language's grammar recognises as a function-like declaration. Anonymous/inline callables are excluded.

| Language | Counts as function | Does NOT count |
|----------|--------------------|----------------|
| **All** | Top-level `fn`/`func`/`def`/`function` declarations | — |
| **All** | Methods (associated functions on a type/class/struct/impl/trait) | — |
| **All** | Constructors (`__init__`, `constructor`, `new()`, `init`) | — |
| **All OOP** | Getters, setters, operator overloads | — |
| **JS/TS** | `function` declarations, `class` methods, object-literal methods | Arrow functions assigned to `const` (`const f = () => {}`), inline callbacks |
| **Python** | `def` and `async def` at any nesting level | `lambda` expressions |
| **Rust** | `fn` (free, associated, trait default) | Closures (`\|x\| {}`) |
| **Go** | `func` (top-level and receiver methods) | Anonymous func literals, closures |
| **Java/Kotlin/C#** | Methods, constructors | Lambdas, anonymous classes |
| **Ruby/PHP** | Method definitions (`def`, `function`) | Blocks, procs, arrow-function equivalents |

Rationale: lambdas/arrow callbacks/closures are typically 1-10 lines and are noise for a "1000-line" signal. The threshold is meant to catch decomposable units of work (named functions/methods), not to flag `array.map(x => x*2)`.

Anonymous functions bound to a named variable (`const handler = async (req) => { ... }` in JS, `handler = lambda x: ...` in Python with a multi-line body) are a grey area. Rule: if the binding name is recoverable from the AST (variable assignment whose RHS is a function expression), count it as a function under that name. Bare inline callbacks passed as arguments never count.

### 1.3 Lines — how to count

For both modules and functions:

1. **Metric:** physical lines (newline-delimited), **inclusive** of the opening signature line through the closing delimiter line.
   - For a function: from the `def`/`fn`/`func`/`function` keyword line to the matching closing `}` / `end` / dedent line, inclusive.
   - For a module: total lines in the file (`wc -l` semantics) after exclusions in §1.3 bullet 4.
2. **Blank lines:** **excluded.** A line that is empty or contains only whitespace (`^\s*$`) contributes 0 to the count.
3. **Comments:** **excluded.** Strip before counting:
   - Line comments: `//`, `#`, `--`, `;` (language-dependent).
   - Block comments: `/* ... */`, `"""..."""` (Python docstring blocks), `<!-- ... -->`, etc.
   - Docstrings / JSDoc / Rust `///` / `//!` doc comments: treated as comments, excluded.
   - A line that is *only* a comment (after trimming) contributes 0.
   - A line that has code followed by a trailing comment counts as 1 (it is a code line).
4. **Effective formula:** `LOC = total physical lines − blank lines − comment-only lines`, scoped to the entity (file for module, body span for function).

Implementation note: for accuracy, parse with tree-sitter / language-aware tooling rather than regex. A line-count helper MUST accept both the raw text and a way to identify comment/blank lines; a reference implementation is `scripts/function_analyzer.py`.

---

## 2. Threshold Rule

```
violation  ⇔  effective_LOC  >  1000       (strictly greater than)
clean      ⇔  effective_LOC  ≤  1000
```

- `1000` itself is **not** a violation. Only `1001` and above violate.
- Do **not** use `>=`.
- Report the exact effective LOC alongside the verdict so consumers can verify.

---

## 3. Exclusions — paths and file kinds never scanned

Exclude a file if **any** rule matches. Exclusions are evaluated before counting.

### 3.1 Directory / path patterns (substring match on the repo-relative path)

```
node_modules/
.venv/  venv/  env/  .env/
build/  dist/  out/  target/  _build/
vendor/  vendored/  third_party/  third-party/
generated/  codegen/  .generated/
.next/  .nuxt/  .output/
coverage/  .coverage/
.mypy_cache/  .pytest_cache/  __pycache__/
.git/  .git/
.hg/  .svn/
```

### 3.2 File-name patterns

```
*.min.js  *.min.css  *.bundle.js  *.chunk.js
*.generated.*  *.pb.go  *.pb.cc  *.pb.h
*.g.dart  *.freezed.dart
package-lock.json  yarn.lock  pnpm-lock.yaml  Cargo.lock  poetry.lock
*.map  *.snap  *.sum
```

### 3.3 Test files (excluded from violation counts)

```
**/test_*.py  **/*_test.py  **/*_test.go  **/*.test.js  **/*.test.ts
**/*.spec.js  **/*.spec.ts  **/*.spec.tsx  **/__tests__/**
**/tests/** (when the directory is conventionally test-only; crate-internal
             integration tests under crates/*/tests/ are source, not excluded —
             they exercise the product)
```

Refinement: in this repo, `crates/*/tests/` files ARE product-test code. They are excluded from the "refactor long functions" action list but may still appear in a full inventory with a `kind: test` tag.

### 3.4 Vendored / generated code

- Any file under a path matching §3.1 counts as vendored/generated.
- Any file whose first 5 lines contain `DO NOT EDIT`, `auto-generated`, `generated by`, `Code generated`, or `@generated` is treated as generated even outside excluded directories.

### 3.5 Minified files

Heuristic: if any single line exceeds **500 characters** and the file has fewer than 5 lines per 1000 characters on average, treat as minified and exclude. Explicit `*.min.*` already covers the common case.

---

## 4. Supported Languages for Initial Scan

The initial scan MUST support at minimum:

| # | Language | Parser hint |
|---|----------|-------------|
| 1 | Python | tree-sitter-python |
| 2 | JavaScript | tree-sitter-javascript |
| 3 | TypeScript (incl. TSX) | tree-sitter-typescript |
| 4 | Go | tree-sitter-go |
| 5 | Java | tree-sitter-java |
| 6 | Rust | tree-sitter-rust |

Additional languages (Kotlin, Ruby, PHP, C/C++, C#, Swift, Shell) SHOULD be supported where the scanner already has a grammar (see code-index: 14 grammars, 50 language IDs) but are not required for the initial pass.

A file whose extension does not map to a supported language is reported with `language: unknown` and `status: skipped` — it never counts as a violation.

---

## 5. Output Contract

Every scan tool that implements this spec MUST emit one JSON object per violation with at least:

```json
{
  "path": "crates/ragent-agent/src/session/processor.rs",
  "language": "rust",
  "kind": "function",
  "name": "SessionProcessor::run_turn",
  "span": { "start_line": 142, "end_line": 1189 },
  "loc": 1047,
  "threshold": 1000,
  "violates": true
}
```

For modules, `kind` is `"module"` and `name` is the file's repo-relative path; `span` covers the whole file.

Optional enrichment: `raw_lines` (before blank/comment stripping), `blank_lines`, `comment_lines`.

---

## 6. Worked Examples

- A Python function whose raw body span is 1200 physical lines but contains 150 blank lines and 80 comment-only lines has `LOC = 1200 − 150 − 80 = 970` → **clean** (not a violation).
- A Rust file with 1000 effective LOC is **clean**. At 1001 it becomes a violation.
- `const onClick = (e) => setState(e)` in a `.tsx` file is a one-line arrow assigned to a variable — counts as a function **only** if its body span exceeds the threshold on its own (unlikely); inline `arr.map(x => x+1)` never counts.
- `vendor/highlight.js/highlight.min.js` is excluded by both `vendor/` and `*.min.js`.
- `crates/ragent-tui/tests/test_layout_deep.rs` is a test file — excluded from the violation list.

---

## 7. Conformance

A scan is conformant iff:

1. It uses effective LOC per §1.3 (blank + comment-only lines removed).
2. It applies the strict `> 1000` rule per §2.
3. It respects all exclusions in §3.
4. It distinguishes modules vs functions per §1.1-1.2.
5. It supports the 6 required languages per §4.
6. Its output includes the fields in §5.

Reference checker: `cargo run --bin ragent -- code-audit` (future) / `scripts/function_analyzer.py --spec-check <report.json>`.

---

*End of spec — v1.0*
