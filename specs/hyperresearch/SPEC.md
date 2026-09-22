---
status: draft
audit:
  - { time: 1786739021, from: "none", to: "draft", actor: "system" }
---
# SPEC: Hyperresearch integration for ragent /research

## Context

Hyperresearch (https://github.com/jordan-gibbs/hyperresearch) is a Claude Code plugin that turns a single prompt into a tier-adaptive, multi-step deep research pipeline. It produces adversarially-audited reports with full source provenance, a persistent searchable source vault, open-access recovery for paywalled papers, contradiction graphs, critic-driven gap filling, and surgical patch-based drafting. ragent already has a `/research` slash command and `ragent research` CLI that performs web search and local-file cross-referencing and writes a self-contained `RESEARCH.md`. This specification adds the most valuable Hyperresearch capabilities — tiered pipeline depth, persistent source vaults, adversarial audit, contradiction tracking, open-access recovery, and resumable runs — into ragent's research tooling while keeping it Rust-native and compatible with ragent's existing provider model.

## Goals

1. Let ragent users run research at three depth tiers matching Hyperresearch's light / full / dissertation model.
2. Persist every gathered source into a searchable vault so future research reuses it before fetching anything new.
3. Verify citations and detect contradictions before the report ships.
4. Recover legal open-access copies of paywalled scholarly sources.
5. Support resumable research runs via run manifests.

## Non-goals

1. Re-implement Hyperresearch's Python subagent roster verbatim or depend on its Python package.
2. Add a separate paid subscription model.
3. Provide a fully-autonomous "no human oversight" mode beyond existing autopilot.

## Requirements

### Ubiquitous requirements

**FR-001** While the `/research` command is available, the system SHALL support a `--tier light|full|dissertation` option that selects the research depth. The system should default to "full".

**FR-002** While a research run is in progress, the system SHALL write every acquired source into a persistent, searchable vault under `.ragent/research_vault/<run_tag>/`.

**FR-003** While a source is added to the vault, the system SHALL record source metadata including URL, title, fetch timestamp, search engine provenance, media type, and raw content path.

**FR-004** While the final report is generated, the system SHALL use only vault-stored source content for citations and direct quotations.

### Event-driven requirements

**FR-005** When the user invokes `/research --tier full`, the system SHALL execute the full 16-step equivalent pipeline: decompose, width sweep, contradiction graph, loci analysis, depth investigation, cross-locus reconcile, source tensions, corpus critic, evidence digest, triple draft, synthesize, critics, gap-fetch, patcher, cite-check, polish, readability audit.

**FR-006** When the cite-check step detects a citation that is not supported by its source, the system SHALL flag the sentence as `CITATION_VERIFICATION_FAILED` and require human approval before shipping the report.

**FR-007** When a research run crashes or is interrupted, the system SHALL, upon the next `/research resume <run_tag>` invocation, read the run manifest and continue from the last completed step.

### State-driven requirements

**FR-008** If the run is in the `light` tier, the system SHALL skip contradiction graph, loci analysis, depth investigation, cross-locus reconcile, source tensions, corpus critic, evidence digest, critics, gap-fetch, and readability audit steps.

**FR-009** If the source vault already contains sources relevant to the current query, the system SHALL reuse those sources before issuing new web search requests.

**FR-010** If open-access recovery is enabled and a fetched scholarly source returns fewer than `oa_min_full_text_chars` characters, the system SHALL query Unpaywall and Europe PMC for a legal open-access full-text copy.

### Optional requirements

**FR-011** The user MAY configure `research.open_access_recovery` in `ragent.json` to enable or disable open-access recovery (default: false).

**FR-012** The user MAY configure `research.contact_email` in `ragent.json` to satisfy Unpaywall's terms of service.

**FR-013** The user MAY supply a `--chapter-count N` option with `--tier dissertation` to partition the research into N chapters.

### Unwanted requirements

**FR-014** The system SHALL NOT ship a citation whose source content cannot be found in the vault or whose source text does not support the cited claim.

**FR-015** The system SHALL NOT present a recovered open-access copy as if it were read from the original paywalled URL without disclosing the recovery source and version in the report frontmatter, CLI output, and source note.

**FR-016** The system SHALL NOT fetch new sources when the vault already contains sufficient sources to answer the query at the requested tier.

## Definitions

- **run_tag**: a stable, URL-safe identifier derived from the query and timestamp that names a single research run directory.
- **vault**: the on-disk source store at `.ragent/research_vault/<run_tag>/` plus an embedded SQLite index.
- **tier**: one of `light`, `full`, or `dissertation`.
- **open-access recovery**: the process of locating a legal, open-access copy of a scholarly work via Unpaywall or Europe PMC when the original URL is blocked or truncated.
- **cite-check**: a verification step that confirms each cited claim is supported by the cited source text.
- **contradiction graph**: a ranked set of source pairs that make mutually incompatible claims.

## Acceptance criteria

- `/research --tier full "impact of GLP-1 drugs on cardiovascular outcomes"` produces a `RESEARCH.md` with a contradiction graph, source tensions, and a cite-check summary.
- `/research resume <run_tag>` continues an interrupted run without re-fetching already-vaulted sources.
- A paywalled DOI with a legal open-access copy is recovered and disclosed correctly.
- `light` tier completes in under 45 minutes on a typical broadband connection and skips adversarial steps.

## Notes

- The existing `ragent-research` crate already has `WebGatherer`, `Source`, `Document`, and CLI modules. The new features should extend those modules rather than replacing them.
- The existing `mf_search` engine already supports DuckDuckGo, Brave, OpenAlex, Wikipedia, Exa, and optional LangSearch / Tavily / Perplexity. The width sweep should leverage `mf_search` for parallel fetching.
- The existing SQLite storage in `ragent-storage` can host the vault index.
