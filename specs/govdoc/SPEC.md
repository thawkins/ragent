---
status: draft
audit:
  - { time: 1789663646, from: "none", to: "draft", actor: "system" }
---
# Specification: `/spec govcreate` - Government/Enterprise Architecture Document to Spec + Project Scaffold

## Overview

This specification defines a new TUI slash command, `/spec govcreate`, that turns an
existing **system architecture document** into an agent-ready project. The command
accepts:

1. a **spec ID** (`[specid]`),
2. a **content reference** - either an HTTP(S) URL pointing at a crawlable site, or a
   local file-system path (a single file or a folder tree),
3. a **target folder path** for the new project, if the path does not exist it is created and
4. the **same arguments accepted by the `/new` slash command**
   (`--language`, `--type`, `--stack`, `--github`/`--gitlab`).

The system reads the referenced content, extracts the architectural structure
(components, layers, interfaces, data stores, external dependencies, and the
relationships between them), uses that structure to generate a specification in the
same shape that `/spec create` produces (`SPEC.md`, `PLAN.md`, `TESTPLAN.md` under
EARS notation), and then scaffolds the new project at the target folder using the
existing `/new` scaffold engine so the generated project already contains the spec.

The folder containing the SPEC.md, PLAN.md and TESTPLAN.md files, is created under the spec
folder tree in the new project reffenced by the target folder path.

### Why this exists

`/spec create` starts from a free-text feature description. In government and
enterprise settings the source of truth is usually a *document*: a solution
architecture description, a design authority pack, a vendor architecture PDF, or a
documentation site. Today a user must read that document manually, distil it into a
prompt, run `/spec create`, and then separately run `/new` to scaffold a project.
`/spec govcreate` fuses those three steps into one command.

```text
/spec govcreate [specid] <content-ref> <target-folder> [--language <lang>] [--type <type>] [--stack <name>] [--github | --gitlab]
```

### Worked examples

```text
/spec govcreate payments-arch https://docs.example.gov/architecture/payments ./payments-svc --language rust --type cmdline --stack axum
/spec govcreate legacy-crm doc/architecture/legacy-crm-sad.pdf ./legacy-crm-migration --language python --type library
/spec govcreate datahub specs/govdoc/samples/datahub-docs ./datahub --language typescript --type tui
```

## Assumptions and interpretation (read before implementing)

The feature prompt leaves a small number of decisions open. This specification fixes
them explicitly so the work is testable; each is called out again under `## Open Questions` so a reviewer can overturn it deliberately rather than by accident.

- **A1 - Where the spec is written.** The generated `specs/<specid>/` directory is
  created **inside the target folder**, not in the invoking project. Rationale: the
  command's purpose is to bootstrap a *new* project that already carries its spec, and
  the `/new` engine already creates `specs/` in the scaffolded project. The invoking
  repository is left untouched.
- **A2 - Ordering of the two halves.** The scaffold runs **first** (so the target
  folder's empty-directory guard is evaluated against a genuinely empty directory and
  the `specs/` folder exists), then the spec is generated into
  `<target-folder>/specs/<specid>/`. If generation fails after a successful scaffold,
  the scaffolded project remains (FR-013 governs the report; FR-014 governs cleanup
  expectations).
- **A3 - Argument order is positional.** `specid`, then `content-ref`, then
  `target-folder`, then `/new` flags in any order. Paths that contain spaces must be
  quoted.
- **A4 - Extraction is LLM-driven, bounded by deterministic pre-processing.** The
  fetch/crawl and local reads are deterministic and bounded (page, depth, and character
  caps); the *structural* interpretation of the gathered text is performed by the
  configured model, matching how `/spec create` delegates authoring to the agent.
- **A5 - Reuse, not reinvention.** Content acquisition reuses the `mf_crawl` /
  `mf_fetch` tools and the local file readers; spec authoring reuses
  `SpecCommand::build_create_prompt` (or a sibling prompt builder); scaffolding reuses
  `project_scaffold::plan_and_emit` and the `ragent new` CLI path.

## Background - existing machinery to reuse

- **Slash-command surface.** Commands are declared as static `SlashCommandDef` entries
  in `crates/ragent-tui/src/app/state.rs` (`SLASH_COMMANDS`, the `"spec"` entry is at
  line ~868) and dispatched in the `/spec` arm of
  `crates/ragent-tui/src/app/slash.rs` (line ~6393). `SpecCommand::parse` lives in
  `crates/ragent-specs/src/commands.rs` (line ~204) with the `"create"` arm at
  ~211-223. `/new` is dispatched at `slash.rs:6388` into
  `crates/ragent-tui/src/app/newproj.rs:98` (`handle_new_command`).
- **Spec authoring prompt.** `SpecCommand::build_create_prompt`
  (`crates/ragent-specs/src/commands.rs:520`) produces the EARS `SPEC.md` + `PLAN.md` +
  `TESTPLAN.md` prompt; the `/spec create` arm spawns the general agent with
  `processor.process_message` (`slash.rs:6446`). `/spec specify`
  (`build_specify_prompt`, `commands.rs:604`) is the SPEC-only precedent.
- **Scaffold engine.** `crates/ragent-tools-extended/src/project_scaffold/` provides the
  shared pipeline `plan_and_emit` (`mod.rs:94`), the empty-directory guard
  `enforce_empty_directory_guard` (`guard.rs:61`), and the flag parser `parse_flags`
  (`flags.rs:501`) over `ScaffoldRequest` (`flags.rs:283`), `Language` (`flags.rs:25`),
  `AppType` (`flags.rs:234`), `HostingTarget` (`flags.rs:270`). The CLI parity surface is
  `ScaffoldArgs` in `src/cli.rs:1030` and `run_new_scaffold` (`cli.rs:1140`).
- **Content acquisition.** `mf_crawl` (crawl tool with `max_pages`, `max_depth`,
  `max_total_chars`, `deadline_ms`, `sitemap`, `focus`, `respect_robots`) and
  `mf_fetch` live under `crates/ragent-tools-extended/src/masterfetch/tools/`; the
  legacy `webfetch` tool is `crates/ragent-tools-extended/src/webfetch.rs`.
- **Progress streaming.** `newproj.rs` demonstrates the streamed progress pattern
  (`ProgressLine`, `push_progress`, `poll_newproj_result`, `refresh_newproj_progress_message`);
  `/research` demonstrates the same pattern for long, multi-stage runs.

The new work is therefore: one argument parser, one content-acquisition front end that
normalises URL and local sources into the same gathered-text shape, one architecture
extraction prompt/hand-off, and the orchestration that chains scaffold -> extract ->
author.

## Definitions

| Term                             | Meaning                                                                                                                                                                                        |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Content reference**      | The second positional argument: an`http://` or `https://` URL, or a local path to a file or folder.                                                                                        |
| **Architecture structure** | The extracted model of the system: named components, their responsibilities, layers/tiers, interfaces/contracts, data stores, external dependencies, and the relationships/flows between them. |
| **Gathered text**          | The normalised, bounded plain-text corpus produced from a content reference before LLM interpretation.                                                                                         |
| **Target folder**          | The third positional argument: the directory in which the new project is scaffolded.                                                                                                           |
| **govcreate run**          | One invocation of the command, from parse to terminal report.                                                                                                                                  |

## Requirements

### FR-001 - Command availability (ubiquitous)

The system shall provide a `/spec govcreate` slash command, registered in the
`SLASH_COMMANDS` registry and reachable through the `/spec` dispatch arm, that is
listed in `/spec help` and in the slash-command autocomplete menu.

### FR-002 - Argument acceptance and validation (ubiquitous)

The system shall parse `/spec govcreate` arguments as `[specid] <content-ref> <target-folder> [--language <lang>] [--type <type>] [--stack <name>] [--github | --gitlab]`, shall validate `specid` with the existing spec-ID rules, shall require
`--language` and `--type` exactly as `/new` does, and shall reject `--github` together
with `--gitlab`.

### FR-003 - Usage help (event-driven)

**When** `/spec govcreate` is invoked with no arguments, with `help`, or with an
unparseable argument list, the system shall display a usage message that documents the
argument order, the two content-reference forms, the target-folder requirement, and the
accepted `/new` flags and values, and shall create no files.

### FR-004 - URL content reference acquisition (optional)

**Where** the content reference begins with `http://` or `https://`, the system shall
acquire the referenced content by crawling the site with a bounded budget, and shall
record the set of pages fetched, the total characters gathered, and any pages that were
excluded or failed together with the reason.

### FR-005 - Local content reference acquisition (optional)

**Where** the content reference is an existing local file, the system shall read that
file; **where** it is an existing local directory, the system shall read the directory
tree, including nested subdirectories, and shall gather the textual content of the
supported document types found within it.

### FR-006 - Unreadable or empty content reference (unwanted)

**If** the content reference file or directory does not exist, is unreadable, or yields
no usable text, **then** the system shall terminate the run before scaffolding, shall
report the specific cause (path not found, permission denied, unsupported format, or
empty corpus), and shall leave the target folder unchanged.

### FR-007 - Architecture structure extraction (event-driven)

**When** a non-empty gathered corpus has been produced, the system shall extract the
architecture structure from it, identifying at minimum named components, their
responsibilities, interfaces or contracts, data stores, external dependencies, and the
relationships between components.

### FR-008 - Structure-to-spec generation (ubiquitous)

The system shall generate `SPEC.md`, `PLAN.md`, and `TESTPLAN.md` for `specid` from the
extracted architecture structure, in the same EARS-notation shape and with the same
section, frontmatter, and task-table contracts that `/spec create` produces.

### FR-009 - Spec placement (ubiquitous)

The system shall write the generated spec to `<target-folder>/specs/<specid>/`, creating
the directory if the scaffold step has not already created it.

### FR-010 - Scaffold reuse (ubiquitous)

The system shall scaffold the new project at the target folder by delegating to the
existing project-scaffold engine with the supplied `--language`, `--type`, `--stack`,
and hosting flags, and shall not implement a second scaffold path.

### FR-011 - Empty-target guard (unwanted)

**If** the target folder is not empty (ignoring ragent's own artefact allowlist:
`.ragent`, `log`, `target`), **then** the system shall refuse to scaffold, shall report
which entries block the operation, and shall not extract, generate, or write anything.

### FR-012 - Ordering guarantee (state-driven)

**While** a govcreate run is executing, the system shall perform the steps in the order
guard, scaffold, acquire, extract, author, and report, and shall not begin content
acquisition before the target-directory guard has passed.

### FR-013 - Failure containment and partial-result report (event-driven)

**When** any stage fails after the scaffold step has completed, the system shall
preserve the scaffolded project, report the completed stages and the failed stage with
its cause, and state that no spec was generated.

### FR-014 - Scaffold failure leaves no spec (unwanted)

**If** the scaffold step fails, **then** the system shall not run acquisition or
authoring, shall report the scaffold error, and shall not write a partial spec.

### FR-015 - Progress reporting (state-driven)

**While** a govcreate run is in progress, the system shall stream staged progress into
the message window - at least one line each for guard, scaffold, acquisition (with the
count of pages or files gathered), extraction, spec authoring, and completion - and
shall update a single progress message in place rather than stacking new messages per
stage.

### FR-016 - Bounded acquisition (state-driven)

**While** acquiring a URL content reference, the system shall enforce a page cap, a
depth cap, and a total-character budget, and shall stop acquiring once any budget is
reached, reporting that the budget was reached.

### FR-017 - Idempotent re-run with `--force` (optional)

**Where** a spec directory already exists at `<target-folder>/specs/<specid>/` and the
user supplies `--force`, the system shall overwrite the generated spec files; **where**
`--force` is omitted, the system shall refuse and report the existing spec.

### FR-018 - Invocation summary (ubiquitous)

The system shall record the invocation (spec ID, content reference, target folder, and
all `/new` flags) in the spec's YAML frontmatter so a later command can replay or audit
how the spec was produced.

### FR-019 - Cancellation (unwanted)

**If** the user cancels a govcreate run while it is in progress, **then** the system
shall stop at the next stage boundary, report the completed stages, and shall not write
spec files after the cancellation point.

### FR-020 - CLI parity (optional)

**Where** the ragent binary is invoked as `ragent spec govcreate ...`, the system shall
perform the same run as the TUI slash command, reporting progress and the summary to
stdout.

### NFR-001 - Discoverability

The command shall appear in `/spec help`, in the autocomplete menu, and in
`docs/howtos/spec.md` with the same worked examples shown in this specification.

### NFR-002 - Testability

The argument parser, path/URL classification, and bounded-acquisition budget logic shall
be pure functions with unit-testable inputs and outputs, independent of network access.

### NFR-003 - Safety

Acquisition shall refuse local-content references that escape the invoking directory
tree, shall honour `respect_robots` for URL references by default, and shall not follow
URL references to non-HTTP(S) schemes.

### NFR-004 - Performance

A run on a local folder reference of up to 100 files shall complete acquisition in under
30 seconds on a developer workstation, excluding model latency.

### NFR-005 - Consistency with existing surfaces

Error text, status strings, and summary formatting shall follow the existing `/spec` and
`/new` conventions (ASCII-only output, `[err]`/`[ok]` markers, `From: /spec govcreate`
message prefix).

## Scope

In scope:

- New argument parsing and dispatch for `/spec govcreate` (TUI + CLI parity).
- A content-acquisition front end normalising URL crawl results and local file/folder
  reads into one gathered-text shape.
- An architecture-extraction prompt and the hand-off to the authoring prompt.
- Orchestration chaining scaffold -> acquire -> extract -> author -> report.
- Docs, help text, autocomplete, and CHANGELOG entries.

Out of scope:

- Changes to the `/new` scaffold engine's recipes, languages, or stacks.
- Changes to the `/spec create` prompt contract beyond adding a sibling builder if
  needed.
- Diagram/image understanding (PDF figures, PNG architecture diagrams) in the first
  iteration.
- Remote repository *analysis* (that is `/reverse`); this feature reads *documents*.
- Automated scheduled/batch govcreate runs.

## Acceptance criteria

1. `/spec govcreate` with a URL reference, a target folder, `--language rust --type cmdline`, scaffolds the project, writes `specs/<specid>/SPEC.md|PLAN.md|TESTPLAN.md`,
   and reports the summary.
2. The same command against a local folder of Markdown and PDF architecture documents
   produces an equivalent spec.
3. A non-empty target folder produces a refusal that names the blocking entries and
   writes nothing.
4. A bad content reference produces a refusal before any scaffold write.
5. `/spec govcreate help` lists every accepted flag and both content-reference forms.
6. `ragent spec govcreate ...` performs the same run from the CLI.

## Open Questions

- **Q1** Should the spec be written into the target folder (A1) or into the invoking
  project's `specs/`? This spec assumes the target folder; confirm with the requester.
- **Q2** Should `--force` also permit scaffolding into a non-empty directory that
  already contains a scaffolded project (re-scaffold), or only overwrite the spec?
  This spec scopes `--force` to the spec only (FR-017).
- **Q3** Should URL acquisition default to `respect_robots: true` (NFR-003 assumes yes)
  with an opt-out flag `--no-robots`?
