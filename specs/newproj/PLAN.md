# Implementation Plan: `/new` — New Project Scaffolding Command

**Spec:** [SPEC.md](SPEC.md) · **Test plan:** [TESTPLAN.md](TESTPLAN.md)

## Approach

Add a self-contained `project-scaffold` module family plus `/new` command wiring:

1. **Scaffold engine** — a new module set in `crates/ragent-tools-extended`
   (`project_scaffold/`) holding: flag parsing/validation, the language registry
   (`rust`/`python`/`go`/`typescript`), per-type layout templates (library/cmdline/tui/gui/webapp),
   stack overlay snippets, and the ragent workspace initialiser (`.ragent/`, `specs/`,
   `log/`, `.gitignore`, `AGENTS.md`). Pure functions — no I/O at the decision layer —
   so recipes are unit-testable as data.
2. **Command surface** — register `new` as a static trigger in
   `crates/ragent-tui/src/app/state.rs`, dispatch in `slash.rs`, mirroring the
   `/reverse` flag-parsing pattern (`--language`, `--type`, `--stack`, `--github`,
   `--gitlab`, `help`).
3. **Hosting integration** — reuse `crates/ragent-tools-vcs`: local `git` init/commit via
   the existing git tool family; remote creation + push via the GitHub/GitLab clients,
   with failure containment (FR-010) and idempotent retry (FR-015).
4. **Help surface** — `/new help` renders a detailed help page whose accepted-value
   lists are derived at render time from the same language/type/stack registries the
   scaffolder uses (FR-018, NFR-001).
5. **Documentation scaffold** — every scaffold also emits `README.md`, `QUICKSTART.md`,
   `STATS.md`, and a `docs/` folder, rendered from the same per-language recipe data
   that produces the code artifacts (FR-019, NFR-002).
6. **Docs** — `docs/howtos/newproject.md` following the existing how-to structure, plus
   README/SPEC.md/QUICKSTART touchpoints per project convention.

## Requirement Coverage Map

| Requirement | Task(s) |
| ----------- | ------- |
| FR-001 Command availability | T-011 |
| FR-002 Empty-directory guard | T-001, T-006 |
| FR-003 Required flag validation | T-001 |
| FR-004 ragent workspace initialisation | T-005 |
| FR-005 Hello-world generation | T-002 |
| FR-006 App-type mapping | T-003 |
| FR-007 Stack layering | T-004 |
| FR-008 Remote-initialisation flags | T-008, T-009, T-010 |
| FR-009 Mutual exclusion of hosting flags | T-001 |
| FR-010 Remote-init failure containment | T-009, T-010 |
| FR-011 Summary report | T-008 |
| FR-012 Help surface | T-011 |
| FR-013 CLI parity | T-012 |
| FR-014 Foreground execution | T-013 |
| FR-015 Idempotent remote retry | T-009, T-010 |
| FR-016 No silent overwrite | T-007 |
| FR-017 Supported language registry | T-002 |
| FR-018 `/new help` detailed help | T-016 |
| FR-019 Project documentation scaffold | T-017 |
| NFR-001 Help content consistency | T-016 |
| NFR-002 Documentation template consistency | T-017 |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Define scaffold flag types, validation, and error enum (pure logic) | FR-002, FR-003, FR-009 | M | Critical | completed | — |
| T-002 | Build language registry with per-language recipes (rust/python/go/typescript) | FR-005, FR-017 | L | Critical | completed | T-001 |
| T-003 | Implement app-type layout mapping (library/cmdline/tui/gui/webapp) per language | FR-006 | L | Critical | completed | T-002 |
| T-004 | Implement stack overlay system with known-stack snippets and unknown-stack warning | FR-007 | M | High | completed | T-003 |
| T-005 | Implement ragent workspace initialiser (`.ragent/`, `specs/`, `log/`, `.gitignore`, `AGENTS.md`) | FR-004 | S | Critical | completed | T-001 |
| T-006 | Implement empty-directory guard with artifact allowlist | FR-002 | S | Critical | completed | T-001 |
| T-007 | Implement no-silent-overwrite file emission (report existing files, leave untouched) | FR-016 | S | High | completed | T-005 |
| T-008 | Local git init + initial commit + summary plumbing | FR-008, FR-011 | M | High | completed | T-005 |
| T-009 | GitHub remote create + push integration with failure containment | FR-008, FR-010, FR-015 | M | High | completed | T-008 |
| T-010 | GitLab remote create + push integration with failure containment | FR-008, FR-010, FR-015 | M | High | completed | T-008 |
| T-011 | Register `/new` trigger + slash dispatch + `help` surface in TUI | FR-001, FR-012 | M | Critical | completed | T-001, T-005 |
| T-012 | CLI parity surface for non-TUI mode | FR-013 | S | Medium | completed | T-011 |
| T-013 | Foreground progress streaming in message window | FR-014 | S | Medium | completed | T-011 |
| T-014 | End-to-end manual test pass against TESTPLAN.md | All | M | High | completed | T-009, T-010, T-011, T-013, T-016, T-017 |
| T-015 | Documentation: how-to manual, README, SPEC.md, QUICKSTART.md updates | FR-012 | S | Low | completed | T-011 |
| T-016 | Implement `/new help` detailed help renderer (purpose, per-argument docs, registry-derived values, worked examples) | FR-018, NFR-001 | S | High | completed | T-002, T-004, T-011 |
| T-017 | Generate README.md, QUICKSTART.md, STATS.md, and docs/ folder from language recipe data | FR-019, NFR-002 | M | High | completed | T-002, T-005 |
| T-018 | Fix GitHub token resolution (app-token downgrade to `gh` CLI) and the CLI `--github` runtime-drop panic | FR-008, FR-010 | S | High | completed | T-009 |
| T-019 | Fix stack overlay duplicating the entry point: replace the base `main` with the framework starter's single entry point | FR-007 | S | High | completed | T-004 |

## Task Notes

- **T-001** — Pure types module: `ScaffoldRequest`, `AppType`, `HostingTarget`,
  `ScaffoldError`; validation short-circuits before any filesystem writes
  (FR-002/FR-003 abort with zero mutations).
- **T-003** — Library type generates no binary entrypoint; cmdline generates console
  entry; tui/gui generate language-appropriate starters.
- **T-009/T-010** — Remote failure must leave local scaffold intact and print actionable
  remediation (FR-010); retry path tolerates an existing local repo without remote
  (FR-015).
- **T-011** — Static trigger entry (`trigger: "new"`) in the slash-command registry and
  dispatch arm in `slash.rs`; `/new help` text mirrors the flag table from the spec.
- **T-016** — The help page is assembled at render time from the language/type/stack
  registries (NFR-001) rather than a hardcoded value list, combined with a fixed prose
  block explaining the command purpose, argument optionality/defaults, mutual
  exclusion of `--github`/`--gitlab`, and two worked examples (FR-018).
- **T-017** — README/QUICKSTART build-and-run instructions and STATS.md statistics are
  rendered from the per-language recipe data (NFR-002: e.g. QUICKSTART run command for
  Rust cmdline is `cargo run`); STATS.md records UTC timestamp, language, type, stack,
  file count, and the ragent version; `docs/` is created as part of the FR-004 layout
  emission step. The files flow through the T-007 no-silent-overwrite emitter and are
  reported in the FR-011 summary; when `--github`/`--gitlab` is used they are included
  in the initial commit (T-008/T-009/T-010).
- **T-014** — The end-to-end pass covers the documentation scaffold (TC-016) and the
  detailed help page (TC-015) in addition to the core/hosting flows, hence the added
  dependencies on T-016 and T-017.
- **T-018** — GitHub credential resolution is a single shared chain in
  `ragent_config::github::resolve_from_stored` (env → stored file → `gh` CLI, with a
  GitHub-App-token downgrade). The `/new` CLI scaffold runs the remote half inside a
  runtime-owning worker so the `reqwest::blocking` client never drops a runtime from an
  async context; the TUI path already runs on a worker `std::thread`. Coverage: the
  `crates/ragent-config/tests/test_github_token.rs` unit suite plus the live
  `--github` CLI run.- **T-019** — `StackRecipe::overlay_source` now runs the base source through
  `strip_entry_point`, which removes the single `fn main { … }` function
  (brace-balanced scan, so nested braces do not end it early) before the
  framework starter is appended. Every FR-007 starter declares its own `main`,
  so the emitted binary source has exactly one entry point and the scaffolded
  project builds and runs as generated; a base source with no `fn main` (a
  library body) is left intact. Covered by the new
  `test_stack_overlay_on_binary_yields_single_main` regression test across all
  five Rust stacks plus the strengthened
  `test_stack_overlay_layers_import_and_starter_on_source` single-`main` assert.
