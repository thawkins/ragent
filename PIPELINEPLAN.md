# PIPELINEPLAN.md — Research & Design Tool Pipeline Capabilities

## Purpose

This document analyzes ragent's three research and design tools — `/research`,
`/reverse`, and `/spec` — and proposes a set of capabilities that compose them
into reusable, multi-stage **pipelines**. Each proposed capability is written as
a project description suitable for consumption by the `/spec create` function,
which will generate a formal `SPEC.md` (EARS requirements), `PLAN.md`
(implementation plan), and `TESTPLAN.md` (manual test plan) from it.

The goal is to move ragent from a set of powerful but individually-invoked tools
toward a **declarative pipeline system** where research, reverse-engineering,
and specification-driven development flow into each other — and into new
applications — with minimal manual hand-off.

---

## 1. Tool Property Analysis

### 1.1 `/research` — Structured Information Gathering

| Property | Value |
|----------|-------|
| **Input** | Topic string, seed URL, seed file (PDF/DOCX/ODT/etc.), flags |
| **Output** | `RESEARCH.md` with citations, contradiction graph, source tensions |
| **Search** | DuckDuckGo, Brave, OpenAlex (scholarly), Wikipedia, optional API engines |
| **Local integration** | `--use-local` (codebase), `--use-specs` (existing specs) |
| **Quality pipeline** | 16-step adversarial pipeline (full/dissertation tiers) |
| **Output formats** | report, executive-summary, comparison-table, source-bibliography, imrad |
| **Resumability** | SQLite source vault; re-run with same name skips re-fetch |
| **Interfaces** | TUI slash command, CLI (`ragent research`), HTTP API (`GET/POST/DELETE /research`) |
| **Cross-tool linkage** | `--from-research` flag links research artifacts into specs |
| **Tiers** | light, full, dissertation |
| **Depth** | shallow, standard, deep |

**Strengths:** Self-contained, citable, traceable, resumable, API-exposed.
**Limitations:** One-shot per invocation; no chaining to other tools beyond
the `--from-research` spec link; no scheduled/recurring execution; no output
diffing across runs.

### 1.2 `/reverse` — Repository Reverse-Engineering

| Property | Value |
|----------|-------|
| **Input** | `github:<owner/repo>` or `gitlab:<owner/repo>` (plus bare `owner/repo` and full URLs as legacy forms), `--tech <stack>`, `--create <name>` |
| **Output** | Synthetic creation prompt (in chat), optional `SPEC.md` via `--create` |
| **Data sources** | GitHub REST API (public github.com and GitHub Enterprise) or GitLab REST API (public gitlab.com and self-hosted instances): repository metadata, root file tree, README |
| **LLM role** | Generates a prompt describing how to recreate the repo from scratch |
| **Constraints** | Does NOT clone; does NOT read individual source files |
| **Cross-tool linkage** | `--create` chains into `/spec create` |
| **Interfaces** | TUI slash command, CLI (`ragent reverse`) |

**Strengths:** Fast orientation for unfamiliar repos; clean spec chaining;
supports both GitHub and GitLab (public and self-hosted).
**Limitations:** Shallow — only root tree + README; no deep source analysis;
no comparison across multiple repos; no integration with the code index graph;
no way to feed reverse output into research (e.g. "research this repo's
domain, then reverse-engineer it").

### 1.3 `/spec` — Specification-Driven Development

| Property | Value |
|----------|-------|
| **Input** | Description string, `--from-research <name>`, `--from-reverse` (via `--create`) |
| **Output** | `SPEC.md` (EARS), `PLAN.md` (task table), `TESTPLAN.md` |
| **Notation** | EARS (5 template types), FR/NFR numbering |
| **Lifecycle** | draft → in_review → approved → in_progress → implemented → verified → archived |
| **Orchestration** | `/spec impl` injects per-task prompts into agent session in dependency order |
| **Analysis** | JTBD analysis, requirement coverage, compliance validation |
| **Feedback** | `FEEDBACK.md` advisory notes surfaced during plan regeneration |
| **SDD artifacts** | `data-model.md`, `contracts/` (optional) |
| **Cross-tool linkage** | `--from-research` consumes research artifacts; reverse `--create` feeds it |
| **Interfaces** | TUI slash command, CLI (`ragent spec`) |

**Strengths:** Formal, validated, lifecycle-managed, implementation-orchestrating.
**Limitations:** Consumes research/reverse output but does not feed forward
into automated implementation verification or CI gating; no multi-spec
dependency graph; `/spec impl` is interactive and session-bound.

### 1.4 Existing Composition Patterns

```
/reverse github:<owner/repo> --create <name>   ──▶  /spec create <name>  ──▶  /spec impl <name>
/reverse gitlab:<owner/repo> --create <name>   ──▶  /spec create <name>  ──▶  /spec impl <name>
/research create <name> <topic>                ──▶  /spec create <name> --from-research <r>
```

These two chains exist today. They are **pairwise and manual**: the user
invokes each stage separately and the artifacts are passed by naming
convention, not by a typed pipeline contract. The `github:` and `gitlab:`
prefixes (with self-hosted GitLab instances supported via configured base
URL) select the VCS provider for the `/reverse` stage.

---

## 2. Gap Analysis

| Gap | Affects | Impact |
|-----|---------|--------|
| No declarative pipeline definition | All tools | Users must remember the exact sequence and flags for every multi-stage workflow |
| No typed artifact passing | research → spec, reverse → spec | Stage coupling is by file-path convention; no schema validation between stages |
| No deep reverse (source-level) | `/reverse` | Reverse is limited to README + root tree; misses architecture inferred from actual code |
| No reverse-to-research bridge | reverse ↔ research | Cannot research a repo's domain then reverse-engineer it, or vice versa |
| No multi-repo comparison | `/reverse`, `/research` | Cannot reverse several repos and produce a comparison research report |
| Provider-coupled reverse | `/reverse` | Reverse is hard-wired to GitHub; no GitLab (public gitlab.com or self-hosted) support, blocking pipelines that target GitLab-hosted repos |
| No scheduled/recurring research | `/research` | Cannot watch a topic over time; no drift detection between runs |
| No research diffing | `/research` | Re-running the same topic overwrites; no structured "what changed" output |
| No spec-to-implementation verification gate | `/spec` | `/spec impl` is interactive; no automated "did the implementation satisfy the spec" check |
| No pipeline persistence/resumability | All tools | A multi-stage run that fails midway must be restarted from scratch |
| No pipeline templates | All tools | Common workflows (due diligence, migration, literature review) must be re-specified each time |

---

## 3. Design Principles for Pipeline Capabilities

1. **Declarative, not imperative** — a pipeline is a data structure (YAML/JSON),
   not a script. The engine interprets it.
2. **Typed artifacts** — each stage declares its input and output artifact
   types; the engine validates compatibility before running.
3. **Composable with existing tools** — pipelines reuse `/research`,
   `/reverse`, `/spec`, and the code index as stages, not reimplement them.
   Repository references use provider-prefixed identifiers (`github:`,
   `gitlab:`) so a single pipeline definition works across VCS providers,
   including self-hosted GitLab instances.
4. **Resumable** — pipeline state persists; a failed stage can be retried
   without re-running completed stages.
5. **Observable** — every stage emits progress events to the event bus;
   pipelines can be monitored in the TUI and via HTTP.
6. **Non-interactive by default** — pipelines run to completion without
   user prompts; interactive gating is an opt-in per stage.
7. **Templateable** — common pipeline shapes are saved as reusable templates
   with parameter substitution.

---

## 4. Proposed Pipeline Capabilities (Project Descriptions)

Each description below is formatted for direct use with `/spec create`. Run:

```text
/spec create <spec-id> "<description text>"
```

The descriptions are written to be self-contained — the explore agent that
generates the spec does not need additional context beyond the description
and the tool documentation it can discover in the codebase.

---

### 4.1 Pipeline Engine — Declarative Multi-Stage Workflow Orchestration

**Spec ID:** `pipeline-engine`

**Description:**

Add a declarative pipeline engine to ragent that lets users define and run
multi-stage workflows composing the research, reverse-engineering, and spec
management tools (and future tools) as typed stages with automatic artifact
passing. A pipeline is defined in a YAML or JSON file (`pipeline.yaml`)
listing ordered stages, where each stage declares: a tool to invoke
(research, reverse, spec, codeindex, bash, or a custom agent), the input
artifact reference, the expected output artifact type, and optional
parameters. The engine validates that each stage's input type matches the
preceding stage's output type before execution, persists intermediate
artifacts to a pipeline run directory under `pipelines/<run-id>/`, emits
progress events to the event bus for TUI and HTTP monitoring, and supports
resumability — a failed or interrupted pipeline can be re-run from the last
completed stage. Provide a `/pipeline run <file>` slash command and a
`ragent pipeline run <file>` CLI subcommand. Ship built-in templates for the
common workflows described in the rest of this document (due diligence,
migration, competitive analysis, literature review, knowledge-base builder).
The system shall validate pipeline definitions against a schema before
execution and reject definitions with type-mismatched stage connections. The
system shall persist pipeline run state so that interrupted runs can resume
from the last successful stage. The system shall emit progress events for
every stage transition visible in both the TUI and the HTTP API. Where a
stage fails, the system shall halt the pipeline and report the failing
stage, its inputs, and a diagnostic message. The system shall support a
`--dry-run` flag that validates the pipeline definition and prints the
execution plan without running any stage.

---

### 4.2 Pipeline Artifact Bus — Typed Artifact Passing Between Stages

**Spec ID:** `pipeline-artifact-bus`

**Description:**

Add a typed artifact bus that underpins the pipeline engine by defining a
standard set of artifact types and a registry that pipeline stages use to
publish and consume intermediate outputs. Define artifact types for the
existing tool outputs: `ResearchReport` (a `RESEARCH.md` path with
frontmatter and source count), `ReversePrompt` (a synthetic creation prompt
string), `SpecBundle` (a `specs/<id>/` directory containing `SPEC.md`,
`PLAN.md`, `TESTPLAN.md`), `CodeIndexGraph` (a codebase index snapshot
reference), `SourceVault` (a research source vault reference), and
`MarkdownDocument` (a generic markdown file path). Each artifact type
specifies a schema (required fields, optional fields, validation rules).
The bus stores artifacts in `pipelines/<run-id>/artifacts/<stage>/` and
maintains a manifest mapping stage outputs to artifact paths and types.
Stages declare their consumed artifact type; the engine resolves the
reference and injects the artifact data. The system shall reject a pipeline
whose stage consumes an artifact type that no preceding stage produces. The
system shall validate every artifact against its type schema at stage
completion and fail the pipeline on schema violation. The system shall
allow a stage to declare multiple output artifacts (e.g. a research stage
produces both a `ResearchReport` and a `SourceVault`). The artifact bus
shall be queryable via HTTP so external systems can inspect pipeline
intermediates.

---

### 4.3 Deep Reverse — Full-Source Repository Analysis

**Spec ID:** `deep-reverse`

**Description:**

Extend the `/reverse` tool to optionally perform deep source-level
analysis beyond the current root-tree-and-README fetch. The repository
argument uses a provider-prefixed identifier (`github:<owner/repo>` or
`gitlab:<owner/repo>`, with self-hosted GitLab instances addressed as
`gitlab:<host>/<owner/repo>` or via a configured GitLab base URL). Add a
`--deep` flag that, when set, clones the repository to a temporary directory
(authenticating with `GITHUB_TOKEN` for private GitHub repos and
`GITLAB_TOKEN` plus the configured GitLab base URL for private or self-hosted
GitLab repos), runs the code index tree-sitter parser
over the cloned sources to build a symbol graph, extracts the top-N
most-connected symbols (god nodes) and community structure, and feeds the
symbol graph, community labels, and god-node list into the LLM prompt
alongside the existing metadata, root tree, and README. The resulting
synthetic creation prompt shall include an "Inferred Architecture" section
derived from the code graph rather than only from the README. Add a
`--deep-limit <N>` flag to cap the number of source files indexed (default
500) to bound runtime. Add a `--deep-graph` flag that, when set with
`--create`, embeds the code graph as a `graph.md` artifact in the generated
spec directory for later reference. The system shall clean up the temporary
clone after generating the prompt unless `--keep-clone` is set. The system
shall report progress (cloning, indexing, graphing, prompting) to the event
bus. The deep analysis shall be resumable — if the clone and index succeed
but the LLM call fails, re-running with the same repo reuses the cached graph.
This capability composes with the pipeline engine as a `reverse` stage with
`deep: true` parameters, producing both a `ReversePrompt` and a
`CodeIndexGraph` artifact.

---

### 4.4 Research-to-Reverse Bridge — Domain Research Before Reverse-Engineering

**Spec ID:** `research-reverse-bridge`

**Description:**

Add a `/reverse` flag `--research-first <topic>` that, before generating the
synthetic creation prompt, runs a `/research create` session on the given
topic (defaulting to the repository's primary language and description) at
`light` tier, then feeds the research executive summary and top findings
into the reverse-engineering LLM prompt as domain context. The research
session is stored under `research/<repo-name>-context/` and linked from the
generated spec's frontmatter when `--create` is also used. The repository
argument to `/reverse` uses the provider-prefixed form (`github:` or
`gitlab:`, the latter supporting both public gitlab.com and self-hosted
GitLab instances via configured base URL). Add a corresponding
`--reverse-after` flag to `/research create` that, after writing
`RESEARCH.md`, scans the report for repository URLs and matches them to a
VCS provider (github.com → `github:`, gitlab.com or a known self-hosted
GitLab host → `gitlab:`), then offers to reverse-engineer the most-cited
repository. The system shall deduplicate repository URLs found in research
sources and present at most five candidates. When `--reverse-after --create
<name>` is used, the system shall chain directly from research into reverse
into spec creation in a single invocation. The system shall record the
research-to-reverse linkage in both the research frontmatter and the spec
frontmatter so the provenance chain is traceable. This bridge enables the
pipeline pattern: research a domain → identify a reference implementation →
reverse-engineer it → spec a port or clone.

---

### 4.5 Competitive Analysis Pipeline — Multi-Repo Reverse + Comparison Research

**Spec ID:** `competitive-analysis-pipeline`

**Description:**

Add a `/compare` slash command and pipeline template that reverse-engineers
multiple repositories in the same domain and produces a structured
comparison research report. The command accepts a space-separated list of
provider-prefixed repository identifiers (`github:<owner/repo>` and/or
`gitlab:<owner/repo>`, with self-hosted GitLab instances supported via
configured base URL, so a single comparison can span GitHub and GitLab)
or a topic string that triggers a `/research` session to discover candidate
repos. For each repo, the pipeline runs a
`/reverse` stage (with optional `--deep`) producing a `ReversePrompt`
artifact, then runs a synthesis stage that feeds all reverse prompts into a
single `/research create` session with `--format comparison-table` to
produce a `RESEARCH.md` comparing the repositories across architecture,
technology stack, module structure, and design decisions. The comparison
report shall include a summary matrix table, per-repo findings, and a
recommendation section. The system shall support a `--spec-winner <name>`
flag that, after producing the comparison, runs `/spec create` using the
highest-recommended repository's reverse prompt. The system shall cap the
number of repos at eight to bound runtime and LLM cost. The pipeline shall
be resumable — if the comparison research stage fails, the individual
reverse artifacts are retained and only the synthesis re-runs. Provide a
pipeline template `competitive-analysis.yaml` with parameters for repo list
and comparison axes. This composes the pipeline engine (4.1), the artifact
bus (4.2), and the research-to-reverse bridge (4.4).

---

### 4.6 Due-Diligence Pipeline — Security and Quality Audit of a Repository

**Spec ID:** `due-diligence-pipeline`

**Description:**

Add a due-diligence pipeline template and
`/duediligence github:<owner/repo>` or `/duediligence gitlab:<owner/repo>`
slash command that assesses a public GitHub or GitLab repository (including
self-hosted GitLab instances) for security posture, code quality, and
maintenance health before adoption or contribution. The pipeline runs five
stages: (1) `/reverse --deep` to
understand architecture and produce a `ReversePrompt` plus
`CodeIndexGraph`; (2) `/research create` with `--use-pdf` and topic
"<repo> vulnerability CVE security advisory" at `full` tier to find
known security issues; (3) a `bash` stage running `cargo audit` (for
Rust repos) or `npm audit` (for Node repos) against the cloned sources;
(4) a code-index god-node analysis stage identifying the most
connection-dense modules as maintenance risk hotspots; (5) a synthesis
stage that assembles all artifacts into a `DUE-DILIGENCE.md` report with
sections for Architecture, Security Findings, Dependency Audit, Risk
Hotspots, and a Go/No-Go recommendation. The system shall aggregate
findings from all stages and compute a simple risk score (0–100) from the
number and severity of security findings, audit vulnerabilities, and
god-node concentration. The report shall be written to
`research/<repo>-due-diligence/DUE-DILIGENCE.md`. The system shall support
a `--spec-remediation <name>` flag that, if the risk score exceeds a
threshold (default 60), runs `/spec create` with a remediation description
derived from the findings. Provide a pipeline template
`due-diligence.yaml`. This composes deep-reverse (4.3), research, bash,
codeindex, and spec creation.

---

### 4.7 Migration Pipeline — Reverse Source Stack, Spec Target Stack, Implement

**Spec ID:** `migration-pipeline`

**Description:**

Add a migration pipeline template and `/migrate github:<owner/repo> --to
<stack>` or `/migrate gitlab:<owner/repo> --to <stack>` slash command that
automates planning a technology-stack migration for an existing repository
on GitHub or GitLab (including self-hosted GitLab instances). The pipeline
runs four stages: (1) `/reverse --deep
--tech <source-stack>` to produce a `ReversePrompt` and `CodeIndexGraph`
capturing the original architecture; (2) a transformation stage that feeds
the reverse prompt into an LLM with a migration directive, producing a
`MigrationPlan` artifact that maps each original module to a target-stack
equivalent and flags modules with no direct equivalent; (3) `/spec create`
using the migration plan, constrained to the target stack via `--tech`, to
produce a `SpecBundle` with EARS requirements for the ported system; (4)
`/spec impl --dry-run` to produce a task list and effort estimate without
starting implementation. The system shall write the migration plan to
`migrations/<repo>-to-<stack>/MIGRATION.md` with a module mapping table,
a risk assessment for modules with no direct equivalent, and an effort
estimate derived from the spec plan's task table. The system shall support
a `--go` flag that proceeds past the dry-run to actual `/spec impl`
implementation. The pipeline shall be resumable at each stage boundary.
Provide a pipeline template `migration.yaml` with parameters for source
repo, source stack, and target stack. This composes deep-reverse (4.3),
spec creation, and spec implementation orchestration.

---

### 4.8 Literature Review Pipeline — Dissertation-Tier Research to Replication Spec

**Spec ID:** `literature-review-pipeline`

**Description:**

Add a literature-review pipeline template and `/literature <topic>`
slash command that produces a dissertation-tier research report and
optionally generates a spec for replicating a cited study. The pipeline
runs three stages: (1) `/research create` with `--tier dissertation
--depth deep --use-pdf --format imrad` to produce a `ResearchReport`
in IMRaD format with full adversarial quality pipeline, contradiction
graph, and source tensions; (2) a citation-extraction stage that parses
the `RESEARCH.md` references index and identifies the most-cited primary
source (paper, dataset, or tool); (3) an optional `--replicate <name>`
flag that, if the top source is a software artifact with a repository URL
(github.com, gitlab.com, or a known self-hosted GitLab host),
runs `/reverse --deep` on it (using the provider-prefixed form) and
`/spec create` to produce a
replication spec that reproduces the study's methodology. The system
shall write the literature review to `research/<topic>-literature/`
and any replication spec to `specs/<name>/`. The system shall include
the research frontmatter (source count, tier, contradiction count) in
the replication spec's `## Related Research` section. The pipeline
shall support a `--since <year>` flag passed through to the research
stage to limit source recency. Provide a pipeline template
`literature-review.yaml` with parameters for topic, year filter, and
replication flag. This composes research (dissertation tier) with
deep-reverse and spec creation, targeting academic and scientific
use cases.

---

### 4.9 Scheduled Research Watch — Recurring Topic Monitoring with Drift Detection

**Spec ID:** `scheduled-research-watch`

**Description:**

Add a scheduled research capability that runs `/research create` on a
recurring schedule and produces a structured diff against the prior run.
Integrate with the existing cron system (`cron_add`) so users can register
a research watch via `/research watch <name> <topic> --every <duration>`
(e.g. `--every 1w`). Each scheduled run writes `RESEARCH.md` to
`research/<name>/` and archives the previous run to
`research/<name>/archive/<timestamp>/`. After each run, a diff stage
compares the new and prior `RESEARCH.md` findings sections and writes a
`DELTA.md` to `research/<name>/` listing: new findings not present in the
prior run, findings that dropped out, sources newly cited, and sources no
longer cited. The system shall send a notification (via the configured
channel — Telegram or Discord) summarising the delta when changes exceed
a threshold (default: any new finding or any dropped finding). The system
shall cap archive retention to a configurable number of runs (default 12)
to bound disk usage. The system shall support a `--alert-keywords`
flag that triggers a notification only when specific keywords appear in
new findings. Provide a pipeline template `research-watch.yaml`. This
extends research with scheduling, archiving, diffing, and notification,
enabling ongoing intelligence gathering on evolving topics.

---

### 4.10 Knowledge-Base Builder Pipeline — Research + Spec + Code Index Fusion

**Spec ID:** `knowledge-base-pipeline`

**Description:**

Add a knowledge-base builder pipeline template and `/kb build <name>
<topic>` slash command that fuses web research, local codebase
knowledge, and existing specs into a unified, citable knowledge base
document for a project or domain. The pipeline runs four stages: (1)
`/research create` with `--use-local --use-specs --depth deep` to
produce a `ResearchReport` that cross-references the local codebase and
existing specs; (2) a code-index stage that builds (or refreshes) the
codebase graph and extracts god nodes, communities, and the top
dependency paths, producing a `CodeIndexGraph` artifact and a
`graph.md` summary; (3) a spec-scan stage that reads all
`specs/<id>/SPEC.md` files and produces a `SpecInventory` artifact with
requirement counts, lifecycle statuses, and coverage summaries; (4) a
synthesis stage that merges the research report, graph summary, and
spec inventory into a `KNOWLEDGE-BASE.md` with sections for Domain
Overview (from research), Architecture (from code graph), Specified
Capabilities (from specs), Gaps (requirements not covered by code),
and Open Questions (from research). The system shall write the output to
`knowledge/<name>/KNOWLEDGE-BASE.md` and link all source artifacts in a
`## Provenance` section. The system shall support a `--refresh` flag
that re-runs only the stages whose inputs have changed (research
sources, code mtime, spec mtime) based on stored timestamps. Provide a
pipeline template `knowledge-base.yaml`. This composes research, the
code index, and spec inventory into a single living document useful for
onboarding, audits, and architectural reviews.

---

### 4.11 Spec-to-Implementation Verification Gate — Automated Spec Compliance Checking

**Spec ID:** `spec-verification-gate`

**Description:**

Add an automated spec compliance verification gate that checks whether an
implementation produced by `/spec impl` actually satisfies the spec's EARS
requirements. After `/spec impl` completes (or at any point via
`/spec verify <name>`), the system runs a verification stage that: (1)
re-reads `specs/<name>/SPEC.md` and extracts every FR/NFR requirement with
its EARS template type; (2) for each requirement, constructs a verification
prompt asking the LLM to assess the current codebase state against the
requirement, providing the code-index symbol graph and the diff of files
changed during `/spec impl`; (3) produces a per-requirement verdict
(satisfied, partially-satisfied, not-satisfied, unverifiable) with
citations to specific code locations; (4) writes a `VERIFICATION.md` to
`specs/<name>/` with a compliance matrix and an overall pass/fail. The
system shall integrate with the spec lifecycle so that a spec can only
transition from `implemented` to `verified` when the verification gate
passes (all requirements satisfied or partially-satisfied with accepted
rationale). The system shall support a `--strict` flag that requires all
requirements to be fully satisfied. The system shall write failing
verification results to `FEEDBACK.md` so the next `/spec plan` regeneration
incorporates them. This closes the loop from spec → implementation →
verified, making the spec lifecycle enforcement meaningful rather than
manual. This composes spec management with the code index and the
existing FEEDBACK.md mechanism.

---

### 4.12 RFP/Tender Response Pipeline — Requirements Research to Spec

**Spec ID:** `rfp-response-pipeline`

**Description:**

Add an RFP (Request for Proposal) / tender response pipeline template and
`/rfp <source>` slash command that ingests an RFP document (via
`--from-file` for PDF/DOCX or `--from-url` for a web-published RFP),
extracts the formal requirements, researches the problem domain, and
produces a spec that defines a compliant solution. The pipeline runs four
stages: (1) a document-ingest stage that extracts text from the RFP source
and produces a `RfpDocument` artifact with raw text and metadata; (2) a
requirement-extraction stage that feeds the document to an LLM with a
prompt to extract numbered requirements, evaluation criteria, and
constraints, producing a `RequirementList` artifact; (3) `/research
create` with `--from-file <rfp>` and topic derived from the RFP domain to
produce a `ResearchReport` establishing domain context and identifying
relevant prior art and risks; (4) `/spec create` using the requirement
list as the description and `--from-research` to link the research, to
produce a `SpecBundle` whose EARS requirements map to the RFP's
requirements. The system shall produce a `COMPLIANCE-MATRIX.md` mapping each
RFP requirement to the spec's FR/NFR IDs, indicating compliant,
partially-compliant, and non-compliant items. The system shall write
outputs to `rfp/<name>/`. Provide a pipeline template `rfp-response.yaml`
with parameters for source path, domain, and compliance threshold. This
composes document ingestion, research, and spec creation into a
bid/proposal-generation workflow.

---

## 5. Composition Map

The diagram below shows how the proposed capabilities compose the
existing tools. Solid arrows are existing flows; dashed arrows are
proposed new flows enabled by this plan.

```
                    ┌─────────────────────────────────────────────────┐
                    │            PIPELINE ENGINE (4.1)                 │
                    │  declarative YAML · typed stages · resumable     │
                    └───────────────┬─────────────────────────────────┘
                                    │ drives
                    ┌───────────────▼─────────────────────────────────┐
                    │         ARTIFACT BUS (4.2)                       │
                    │  ResearchReport · ReversePrompt · SpecBundle     │
                    │  CodeIndexGraph · SourceVault · MarkdownDocument │
                    └───┬───────┬───────┬───────┬───────┬─────────────┘
                        │       │       │       │       │
            ┌───────────▼┐ ┌───▼────┐ ┌─▼─────┐ ┌▼──────┐ ┌▼──────────┐
            │ /research   │ │/reverse│ │/spec  │ │codeidx│ │ bash/agent │
            │ (existing)  │ │(exist.)│ │(exist)│ │(exist)│ │ (existing) │
            └─────���┬──────┘ └───┬───┘ └───┬───┘ └───┬───┘ └─────┬─────┘
                   │            │         │         │           │
                   │  ┌─────────▼────┐    │         │           │
                   │  │ DEEP REVERSE │    │         │           │
                   │  │ (4.3)        │────┼─────────┘           │
                   │  └──────┬───────┘    │                     │
                   │         │            │                     │
              ┌────▼─────────▼────┐  ┌───▼──────────────┐       │
              │ RESEARCH-REVERSE   │  │ SPEC VERIFICATION│       │
              │ BRIDGE (4.4)       │  │ GATE (4.11)       │       │
              └────────┬───────────┘  └──────────────────┘       │
                       │                                        │
   ┌───────────────────┼──────────────────────────���─┐            │
   │                   │                            │            │
┌──▼──────────┐  ┌────▼─────────────┐  ┌───────────▼──────────┐ │
│ COMPETITIVE │  │ DUE DILIGENCE    │  │ MIGRATION PIPELINE   │ │
│ ANALYSIS    │  │ PIPELINE (4.6)   │  │ (4.7)                │ │
│ (4.5)       │  └──────────────────┘  └──────────────────────┘ │
└─────────────┘                                                 │
                                                                  │
┌──────────────────────┐  ┌────────────────────┐  ┌──────────────▼──────┐
│ LITERATURE REVIEW    │  │ SCHEDULED RESEARCH  │  │ KNOWLEDGE BASE       │
│ PIPELINE (4.8)       │  │ WATCH (4.9)         │  │ BUILDER (4.10)       │
└──────────────────────┘  └────────────────────┘  └────────────────────���┘

┌──────────────────────────────────────────────────────────────────────┐
│ RFP / TENDER RESPONSE PIPELINE (4.12)                                │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 6. Recommended Implementation Order

The capabilities have dependencies. Suggested order for `/spec create` +
implementation:

| Order | Spec ID | Rationale |
|-------|---------|-----------|
| 1 | `pipeline-artifact-bus` | Foundation: defines the typed artifact contracts all other pipelines use |
| 2 | `pipeline-engine` | Foundation: the engine that runs stages and consumes the artifact bus |
| 3 | `deep-reverse` | Enhances an existing tool; produces `CodeIndexGraph` artifact for downstream pipelines |
| 4 | `research-reverse-bridge` | First end-to-end pipeline using the engine; validates the architecture |
| 5 | `spec-verification-gate` | Closes the spec lifecycle loop; independent of the pipeline engine |
| 6 | `competitive-analysis-pipeline` | First multi-repo pipeline; exercises parallel stages |
| 7 | `due-diligence-pipeline` | Composes deep-reverse + research + bash + codeindex |
| 8 | `migration-pipeline` | Composes deep-reverse + spec + impl |
| 9 | `literature-review-pipeline` | Exercises dissertation-tier research + replication |
| 10 | `scheduled-research-watch` | Adds scheduling, diffing, notification |
| 11 | `knowledge-base-pipeline` | Fusion of all three tools + code index |
| 12 | `rfp-response-pipeline` | Document ingestion + research + spec; broadest composition |

---

## 7. How to Use This Document

To create a spec from any of the project descriptions above, run in the
ragent TUI:

```text
/spec create <spec-id> "<paste the Description section text>"
```

For example, to create the pipeline-engine spec:

```text
/spec create pipeline-engine "Add a declarative pipeline engine to ragent that lets users define and run multi-stage workflows composing the research, reverse-engineering, and spec management tools..."
```

Then validate and plan:

```text
/spec validate pipeline-engine
/spec plan pipeline-engine
/spec tasks pipeline-engine
```

Each description is written to produce a self-contained spec with EARS
requirements, a dependency-ordered plan, and a manual test plan. The
descriptions intentionally reference existing tool behavior (research tiers,
spec lifecycle, reverse flags) so the explore agent grounds the generated
requirements in the actual codebase.

---

*End of PIPELINEPLAN.md*