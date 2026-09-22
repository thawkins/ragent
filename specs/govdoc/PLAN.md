# Implementation Plan: `/spec govcreate` - Architecture Document to Spec + Project Scaffold

**Spec:** [SPEC.md](SPEC.md) · **Test plan:** [TESTPLAN.md](TESTPLAN.md)

## Approach

Add one new subcommand to the existing `/spec` surface and reuse three existing engines
rather than building new ones:

1. **Command surface** - extend `SpecCommand` in `crates/ragent-specs/src/commands.rs`
   with a `GovCreate` variant and a dedicated parser (`parse_govcreate`), register the
   subcommand in `USAGE_SUBCOMMANDS` and in `/spec help`, and add a `"govcreate"` case to
   the `/spec` dispatch arm in `crates/ragent-tui/src/app/slash.rs` (~line 6393).
2. **Content acquisition front end** - a new module family
   (`crates/ragent-tools-extended/src/archdoc/`) that classifies a content reference as
   URL or local path (pure function), crawls URLs through the existing masterfetch crawl
   engine and reads local files/folders through the existing file readers, then
   normalises both into one `GatheredCorpus { sources, text, stats }` shape. Budget
   logic (page/depth/char caps) is pure and unit-testable (NFR-002).
3. **Architecture extraction** - a prompt builder that instructs the configured model to
   produce the architecture structure from `GatheredCorpus`, and a deterministic
   fallback that emits a minimal structure (file/section inventory) when the model
   response cannot be parsed.
4. **Spec authoring** - delegate to a sibling of `SpecCommand::build_create_prompt`
   (`build_govcreate_prompt`) that takes the extracted structure and writes the same
   EARS `SPEC.md` + `PLAN.md` + `TESTPLAN.md` triple, including the FR-018 invocation
   frontmatter.
5. **Scaffold reuse** - call `project_scaffold::enforce_empty_directory_guard` and
   `project_scaffold::plan_and_emit` exactly as the `/new` path does, plus the hosting
   helpers, so there is a single scaffold implementation (FR-010).
6. **Orchestration** - a `govcreate` runner in the TUI app layer that chains
   validate -> guard -> scaffold -> acquire -> extract -> author -> report, streaming
   progress through the `/new`-style `ProgressLine` machinery, with staged failure
   containment (FR-013/FR-014) and cancellation-boundary handling (FR-019).
7. **CLI parity** - a `Commands::Spec` subcommand path in `src/cli.rs` mirroring the
   `ragent new` token-rebuild pattern so both surfaces share one parser.
8. **Docs** - `docs/howtos/spec.md` section plus autocomplete/help updates and CHANGELOG.

FR-006 is evaluated **before** the scaffold runs: an unreadable or empty content
reference terminates the run and leaves the target folder unchanged. The target folder
itself is created when it does not exist, then the empty-directory guard is evaluated
against it.

## Requirement Coverage Map

| Requirement | Task(s) |
| ----------- | ------- |
| FR-001 Command availability | T-001, T-010, T-011 |
| FR-002 Argument acceptance and validation | T-002, T-003 |
| FR-003 Usage help | T-004, T-011 |
| FR-004 URL content reference acquisition | T-006 |
| FR-005 Local content reference acquisition | T-007 |
| FR-006 Unreadable or empty content reference | T-003, T-007, T-009, T-012 |
| FR-007 Architecture structure extraction | T-008 |
| FR-008 Structure-to-spec generation | T-009 |
| FR-009 Spec placement | T-009 |
| FR-010 Scaffold reuse | T-005 |
| FR-011 Empty-target guard | T-005, T-012 |
| FR-012 Ordering guarantee | T-012 |
| FR-013 Failure containment and partial-result report | T-012, T-013 |
| FR-014 Scaffold failure leaves no spec | T-012 |
| FR-015 Progress reporting | T-013, T-014 |
| FR-016 Bounded acquisition | T-006 |
| FR-017 Idempotent re-run with `--force` | T-003, T-009 |
| FR-018 Invocation summary | T-004, T-009 |
| FR-019 Cancellation | T-012, T-013 |
| FR-020 CLI parity | T-015 |
| NFR-001 Discoverability | T-010, T-016 |
| NFR-002 Testability | T-003, T-006, T-007, T-017 |
| NFR-003 Safety | T-003, T-006, T-007 |
| NFR-004 Performance | T-007, T-018 |
| NFR-005 Consistency with existing surfaces | T-004, T-011, T-013 |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Define `SpecCommand::GovCreate` variant and register the `govcreate` subcommand | FR-001 | S | High | completed | — |
| T-002 | Implement `parse_govcreate` argument parser (positionals, spec-ID validation, `/new` flags) | FR-002 | M | Critical | completed | T-001 |
| T-003 | Add pure content-reference classifier, path-safety and readability validator | FR-002, FR-006, FR-017, NFR-002, NFR-003 | M | Critical | completed | T-001 |
| T-004 | Add govcreate usage message, status/log strings, and invocation-frontmatter builder | FR-003, FR-018, NFR-005 | S | High | completed | T-001 |
| T-005 | Wire scaffold reuse (empty-dir guard, `plan_and_emit`, hosting helpers) into the govcreate runner | FR-010, FR-011 | M | Critical | completed | T-002 |
| T-006 | Implement bounded URL acquisition via the masterfetch crawl engine | FR-004, FR-016, NFR-002, NFR-003 | L | High | completed | T-003 |
| T-007 | Implement local file/folder acquisition with supported-format filtering | FR-005, FR-006, NFR-002, NFR-003, NFR-004 | M | High | completed | T-003 |
| T-008 | Build architecture-extraction prompt plus deterministic fallback structure | FR-007 | M | High | completed | T-006, T-007 |
| T-009 | Implement `build_govcreate_prompt` and spec writing to `<target>/specs/<specid>/` | FR-008, FR-009, FR-006, FR-017 | M | Critical | completed | T-004, T-008 |
| T-010 | Register the command in `SLASH_COMMANDS` and autocomplete metadata | FR-001, NFR-001 | S | Medium | completed | T-001 |
| T-011 | Add `"govcreate"` to the `/spec` dispatch arm and `/spec help` | FR-001, FR-003, NFR-005 | S | High | completed | T-001, T-004 |
| T-012 | Implement the govcreate orchestration runner (pre-scaffold validation, ordering, failure containment, cancellation) | FR-006, FR-011, FR-012, FR-013, FR-014, FR-019 | L | Critical | completed | T-005, T-009 |
| T-013 | Stream staged progress into a single in-place message and emit the terminal report | FR-015, FR-013, FR-019, NFR-005 | M | High | completed | T-012 |
| T-014 | Add TUI result polling and progress-message refresh for govcreate | FR-015 | M | Medium | completed | T-013 |
| T-015 | Add `ragent spec govcreate` CLI parity using the shared token parser | FR-020 | M | Medium | completed | T-002, T-012 |
| T-016 | Document the command in `docs/howtos/spec.md` and regenerate the how-to PDFs | NFR-001 | S | Medium | completed | T-011 |
| T-017 | Add unit tests for the parser, classifier, and acquisition budgets | FR-002, FR-003, FR-006, FR-016, FR-017, NFR-002 | M | High | completed | T-002, T-003, T-006, T-007 |
| T-018 | Verify acceptance criteria end to end, run fmt/clippy/test, and check NFR-004 timing | All, NFR-004 | M | High | completed | T-001, T-002, T-003, T-004, T-005, T-006, T-007, T-008, T-009, T-010, T-011, T-012, T-013, T-014, T-015, T-016, T-017 |
## Task Details

### T-001 - Define the `GovCreate` variant and register the subcommand

- Add `SpecCommand::GovCreate { spec_id, content_ref, target_folder, scaffold: ScaffoldRequest, force: bool }`
  to `crates/ragent-specs/src/commands.rs` alongside `Create` (line ~11) and `Jtbd`.
- Add `"govcreate"` to `USAGE_SUBCOMMANDS` (line ~194) so `is_usage_error` (line ~449)
  stays in sync.
- Keep the variant free of I/O; it carries parsed data only (NFR-002).

### T-002 - Implement `parse_govcreate`

- Add `parse_govcreate(rest: &str) -> SpecCommand` following the `parse_feature_with_research`
  token-scan style (line ~158). Extract three positionals in order: `specid`,
  `content-ref`, `target-folder`.
- Validate `specid` with the existing spec-ID rules (the same validator used by
  `/spec create` / `SpecId`), rejecting path-traversal or otherwise invalid spec IDs
  (FR-002).
- For the trailing flags, delegate to the existing `/new` parser
  `project_scaffold::flags::parse_flags` (line ~501) by rebuilding the flag token slice,
  so accepted values and error text cannot drift (mirrors `ScaffoldArgs::to_tokens`,
  `src/cli.rs:1056`).
- Return `Self::Unknown("govcreate")` for missing positionals, and surface
  `ScaffoldError` (hosting conflict, missing language/type, unknown flag) as the usage
  error path (FR-002).
- Accept and strip an optional `--force` before delegating (FR-017).

### T-003 - Content-reference classifier and safety validator

- New pure function `classify_content_ref(input: &str) -> ContentRef` returning
  `ContentRef::Url(Url)` for `http://`/`https://` prefixes and `ContentRef::Local(PathBuf)`
  otherwise; reject any other scheme (NFR-003).
- `validate_local_path(path, invoking_root)` ensures the canonicalised path stays inside
  the invoking directory tree (NFR-003) and returns the specific readability cause -
  path not found, permission denied, unsupported format, or empty corpus - so FR-006 can
  be reported before any scaffold write (FR-006).
- `is_forced_overwrite(target_spec_dir, force)` decides the FR-017 branch as a pure
  predicate.

### T-004 - Usage message, status/log strings, invocation frontmatter

- `build_govcreate_help_message()` returning an ASCII-only usage block with the argument
  order, both content-reference forms, and the accepted `/new` values (FR-003,
  NFR-005). Derive the language/type lists from
  `project_scaffold::flags::language_value_list` / `app_type_value_list`.
- `build_govcreate_status`, `build_govcreate_message`, `build_govcreate_log` following
  `build_create_status`/`build_create_message`/`build_create_log` (lines ~483, ~492,
  ~506) and using ASCII markers only.
- `build_govcreate_frontmatter(...)` emitting the FR-018 invocation record
  (`spec_id`, `content_ref`, `target_folder`, `language`, `type`, `stack`, hosting).

### T-005 - Wire scaffold reuse into the runner

- In the govcreate runner, create the target folder when it does not exist, then call
  `project_scaffold::enforce_empty_directory_guard(&target)` (guard.rs:61) before any
  other action (FR-011), then `project_scaffold::plan_and_emit(...)` (mod.rs:94) with the
  parsed `ScaffoldRequest`.
- Reuse the hosting helpers used by `run_new_scaffold` (`src/cli.rs:1140`) - git init and
  optional GitHub/GitLab remote - rather than reimplementing them.
- Do not duplicate any recipe, language, or stack logic (FR-010).

### T-006 - Bounded URL acquisition

- New `crates/ragent-tools-extended/src/archdoc/url_source.rs` that invokes the
  masterfetch crawl engine with a bounded budget (`max_pages`, `max_depth`,
  `max_total_chars`, `deadline_ms`), reusing the `pub const` defaults in
  `masterfetch/crawl/orchestrator.rs` (DEFAULT_MAX_PAGES, DEFAULT_MAX_DEPTH,
  DEFAULT_MAX_TOTAL_CHARS, DEFAULT_DEADLINE_MS) rather than re-literalising them.
- Default `respect_robots: true` (NFR-003). Record fetched pages, total characters, and
  per-page exclusion/failure reasons by reading the engine's existing stats.
- Pure budget function `budget_exhausted(stats, limits) -> Option<BudgetReason>` for
  unit testing (FR-016, NFR-002).

### T-007 - Local file/folder acquisition

- New `crates/ragent-tools-extended/src/archdoc/local_source.rs`: for a file, read it
  through the existing document readers; for a directory, walk subdirectories and gather
  supported text/markup/PDF/office types.
- Sort the walk deterministically and cap total characters using the same budget shape as
  T-006.
- Enforce the T-003 path-safety check and the NFR-004 performance target (100 files /
  under 30 s).
- Return the same `GatheredCorpus` shape as T-006 so extraction is source-agnostic, and
  surface the FR-006 cause when the reference is unreadable or yields no usable text.

### T-008 - Architecture extraction

- New prompt builder `build_arch_extraction_prompt(corpus: &GatheredCorpus) -> String`
  asking for named components, responsibilities, interfaces/contracts, data stores,
  external dependencies, and relationships (FR-007).
- Define `ArchitectureStructure` as a typed struct with a serde-deserialisable JSON
  contract so the model response can be parsed and validated.
- Deterministic fallback: when the response does not parse, emit a minimal structure
  derived from the source inventory (page titles, headings, file names) so authoring
  still proceeds and never panics (mirrors the research mechanical fallback precedent).

### T-009 - Spec authoring and placement

- `build_govcreate_prompt(spec_id, structure, invocation) -> String` as a sibling of
  `build_create_prompt` (commands.rs:520), keeping the identical file-list contract
  (SPEC.md EARS with `status: draft` frontmatter, PLAN.md task table, TESTPLAN.md manual
  cases) plus the FR-018 invocation frontmatter.
- Write to `<target-folder>/specs/<specid>/` (FR-009), creating the directory when the
  scaffold step did not. Honour `--force` via the T-003 predicate (FR-017), otherwise
  refuse and report the existing spec.
- Report a clear cause when the corpus was empty or the model produced nothing usable
  (FR-006).

### T-010 - Command registry and autocomplete

- Add a `SlashCommandDef { trigger: "govcreate", description: ... }` entry (the
  `"spec"` descriptor at state.rs:868 is the sibling model) and update the `"spec"`
  description to mention `govcreate` (FR-001, NFR-001).
- Add any autocomplete/parameter-hint metadata the `/spec` family uses.

### T-011 - Dispatch arm and `/spec help`

- Add the `SpecCommand::GovCreate { .. }` case to the `/spec` dispatch arm
  (slash.rs:6393) that spawns the runner, mirroring the `Create` arm's
  `select_general_agent` + `processor.process_message` structure but pointing at the
  govcreate orchestration.
- Add a `govcreate` row to `build_help_message()` (commands.rs:~461) with the worked
  examples (FR-003, NFR-005).

### T-012 - Orchestration runner

- Implement `run_govcreate(...)` chaining validate -> guard -> scaffold -> acquire ->
  extract -> author -> report with the explicit ordering of FR-012 (no acquisition before
  the guard passes).
- Validate the content reference (existence, readability, path safety) before the
  scaffold step so an unreadable or empty reference terminates the run and leaves the
  target folder unchanged (FR-006).
- Scaffold failure short-circuits before acquisition (FR-014); post-scaffold failure
  preserves the project and reports the partial result (FR-013).
- Model the stage state as an enum so the report can name completed vs failed stages.
- Check the existing cancellation flag between stages and stop at the boundary (FR-019).

### T-013 - Progress streaming and report

- Emit one progress line per stage (guard, scaffold, acquisition with counts, extraction,
  authoring, completion) using the `ProgressLine` pattern (newproj.rs:56).
- Update one in-place progress message rather than stacking (FR-015).
- Emit the terminal summary with the `From: /spec govcreate` prefix and `[ok]`/`[err]`
  markers (NFR-005).

### T-014 - TUI polling and refresh

- Add a `poll_govcreate_result` equivalent to `poll_newproj_result` (newproj.rs:246) and a
  `refresh_govcreate_progress_message` equivalent, so repeated polls update a single
  message (FR-015).

### T-015 - CLI parity

- Add a `ragent spec govcreate` subcommand to the spec command enum in `src/cli.rs`,
  rebuilding the token slice from typed args (the `ScaffoldArgs::to_tokens` pattern at
  cli.rs:1056) and calling the shared parser + runner.
- Print progress and the summary to stdout; exit non-zero on a guard or acquisition
  failure (FR-020).

### T-016 - Documentation and PDF regeneration

- Add a `/spec govcreate` section to `docs/howtos/spec.md` with the worked examples, then
  regenerate the how-to PDFs with the pandoc/xelatex pipeline (NFR-001).

### T-017 - Unit tests

- Parser tests: positional extraction, missing positionals, invalid spec ID,
  `--github`/`--gitlab` conflict, `--force` handling (FR-002, FR-003, FR-017).
- Classifier tests: URL vs local classification, scheme rejection, path-escape rejection
  (NFR-003).
- Budget tests: page/depth/char cap exhaustion and the returned reason (FR-016, NFR-002).
- Local-acquisition tests over a fixture folder including nested subdirectories and an
  unsupported file type (FR-005, FR-006).

### T-018 - Acceptance verification

- Walk the six acceptance criteria in SPEC.md against a built binary.
- Run `cargo fmt --all --check`, `cargo clippy --workspace -- -D warnings`, and
  `cargo test --workspace`.
- Time a local folder acquisition of 100 files against the NFR-004 bound.

## Risks and Mitigations

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| LLM extraction returns unparseable output | Authoring stalls | T-008 provides a deterministic file/section-inventory fallback so authoring always proceeds |
| URL crawl exceeds budget or hangs | Slow run, unbounded memory | T-006 enforces page/depth/char/deadline caps and reports `BudgetReason` |
| Local path escapes the invoking tree | Data exposure | T-003 validates the canonicalised path before any read (NFR-003) |
| Invalid spec ID (path traversal) writes outside `specs/` | File-system escape | T-002 validates `specid` with the existing spec-ID rules (FR-002) |
| Two scaffold implementations drift | Inconsistent projects | T-005 delegates entirely to `project_scaffold::plan_and_emit` (FR-010) |
| Spec written into the wrong root | Project polluted | A1 fixes placement to `<target-folder>/specs/<specid>/` (FR-009, Q1) |

## Notes on Assumptions

- A1 (spec placement) is implemented by T-009 and exercised by TC-011; Q1 may overturn it.
- A2 (scaffold before acquire) applies only to a valid reference: FR-006 terminates the
  run before the scaffold step when the content reference is unreadable or empty, so
  validation (T-003, T-012) is evaluated up front.
- A3 (positional argument order) is implemented by T-002.
- A4 (LLM-driven extraction bounded by deterministic pre-processing) is split across
  T-006/T-007 (deterministic) and T-008 (LLM).
- A5 (reuse) is enforced by T-005, T-006, T-007 and T-009.