---
status: draft
---

# Specification: Reputation-Scored Web Source Relevance

## Overview

The `/research` command uses `WebGatherer` to collect candidate web sources for
a topic. Currently, each source is scored purely from local signals — the query,
the title, the snippet, and the URL. This specification adds a **site
reputation score** for every web source and folds that score into the relevance
calculation. The goal is to boost sources from well-known, high-trust domains
(e.g. official documentation, established encyclopedias, major universities,
government bodies, recognised news organisations) and reduce the ranking of
sources from domains that are frequently low-quality, spammy, or known to be
unreliable.

The reputation mechanism is intended to be:

1. **Transparent** — a developer can read the scoring table and understand why
   a source was boosted or penalised.
2. **Deterministic** — the score is computed from the URL host and a curated
   site table, with no extra LLM cost per source.
3. **Configurable** — project users can override, extend, or disable the
curated table via the configuration layer.
4. **Conservative** — reputation is a multiplier on the existing relevance
   signal, never a hard filter, so a highly relevant source from an unknown site
   can still outrank a weak source from a well-known site.

## Background

### Existing relevance scoring

`compute_relevance_label` in
`crates/ragent-research/src/web_gatherer/relevance.rs` returns a human-readable
label (`Very high`, `High`, `Medium-high`, `Medium`, `Low`, `Very low`) and a
boolean `retained` flag. The label is derived from the ratio of query terms that
appear in the title, snippet, and URL. The boolean flag is used in
`WebGatherer` to drop low-relevance hits before they are fetched, unless
`--use-low-relevance` is enabled.

### Existing source data

`Source::Web` carries `url`, `title`, `snippet`, `body`, `retrieved_at`, and the
computed `relevance` label. The new reputation data is added alongside that
label as a new field and a new numeric score.

### Reputation sources

The implementation uses three sources of reputation data, in order of
preference:

1. **Project-local override file** — `.ragent/reputation.json` or the path set
   in config (`research.reputation.override_path`).
2. **User-global override file** — `~/.config/ragent/reputation.json`.
3. **Bundled curated table** — a conservative default list compiled into the
   binary, covering the most common high-trust and known-low-trust domain
   patterns.

Each entry maps a **domain or domain suffix** (e.g. `docs.rs`, `.edu`,
`wikipedia.org`, `example-spam-site.com`) to a reputation **tier**. Tiers are
mapped to a numeric score in the closed interval `[0.0, 1.0]`:

| Tier      | Score range | Meaning |
| --------- | ----------- | ------- |
| excellent | 0.95–1.00   | Authoritative primary sources (official docs, standards bodies, major universities) |
| good      | 0.80–0.94   | Established publishers, recognised encyclopedias, reputable news |
| neutral   | 0.50–0.79   | Most of the web; no strong prior |
| poor      | 0.20–0.49   | Frequently unreliable, clickbait, anonymous aggregators |
| blocked   | 0.00–0.19   | Known spam, malware, disinformation, or user-blocked domains |

## Requirements

### FR-001 — Reputation score on every web source (ubiquitous)

Every web source captured by `WebGatherer` **shall** receive a numeric
`reputation_score` in the range `[0.0, 1.0]` before it is stored in the research
state.

> *Ubiquitous requirement — applies to all web sources regardless of
> configuration or topic.*

### FR-002 — Domain-based lookup (ubiquitous)

The `reputation_score` for a source **shall** be determined primarily from the
registered-domain-plus-suffix of the source URL, using the configured
reputation table.

### FR-003 — Tier-to-score mapping (ubiquitous)

The reputation table **shall** store entries as human-readable tiers; at lookup
time the tier **shall** be converted to a numeric score using a fixed mapping
that is documented and testable.

### FR-004 — Suffix matching for whole categories (state-driven)

When a domain matches both a specific entry and a suffix entry (e.g.
`cs.stanford.edu` matches both `stanford.edu` and `.edu`), the most specific
matching entry **shall** be used.

> *State-driven requirement — the selection depends on the current match
> specificity state.*

### FR-005 — Unknown domains are neutral (state-driven)

When a source URL's domain does not match any entry in the active reputation
table, the source **shall** receive the `neutral` score.

> *State-driven requirement — the default score is triggered by the absence of
> a match.*

### FR-006 — Relevance score adjusted by reputation (event-driven)

When the relevance label and reputation score are both available for a web
source, the gatherer **shall** compute an `adjusted_relevance` value that
combines the local relevance signal and the reputation score. Sources with a
reputation score above neutral are boosted; sources below neutral are reduced.

> *Event-driven requirement — triggered by the availability of both signals.*

### FR-007 — Boost is a multiplier, not a hard filter (ubiquitous)

The reputation adjustment **shall** be applied as a multiplier in the closed
interval `[0.5, 1.5]` derived from the reputation score, and then multiplied by
the existing numeric relevance. The adjustment **shall not** remove sources or
override the local relevance label on its own.

### FR-008 — Transparent label includes reputation (ubiquitous)

The `relevance` label stored on each web source **shall** include the
reputation tier in parentheses, e.g. `High — title matches query (good)` or
`Medium — partial query match (poor)`, so users can see why the adjusted score
differs from the local-only score.

### FR-009 — Configurable override table (event-driven)

When a project-local or user-global reputation override file is present and
valid, the gatherer **shall** load it at startup and merge it on top of the
bundled table. Entries in the override file **shall** replace bundled entries
with the same key; new entries **shall** be added.

> *Event-driven requirement — triggered by the presence of an override file.*

### FR-010 — Override file schema validation (event-driven)

When an override file is present but contains invalid JSON, unknown tiers, or
malformed scores, the system **shall** log a clear warning, fall back to the
bundled table, and continue gathering.

> *Event-driven requirement — triggered by parsing/validation failure of the
> override file.*

### FR-011 — Reputation can be disabled (event-driven)

When the configuration flag `research.reputation.enabled` is set to `false`, the
gatherer **shall** assign every web source the neutral score and omit the
reputation parenthetical from the label.

> *Event-driven requirement — triggered by the disabled configuration state.*

### FR-012 — No new network calls per source (ubiquitous)

The reputation lookup **shall not** require any network I/O per web source. All
necessary data **shall** be available from the bundled table or the local
override file.

### FR-013 — Hard-block list honoured (event-driven)

When a source URL matches a `blocked` tier entry and the source would
otherwise be retained, the gatherer **shall** treat the source as low relevance
and drop it, regardless of the local relevance label, unless
`--use-low-relevance` is enabled.

> *Event-driven requirement — triggered by a `blocked` reputation match.*

### FR-014 — CLI flag to show raw versus adjusted score (optional)

The research CLI **may** support a `--show-reputation-breakdown` flag that emits
both the raw local relevance label and the adjusted relevance label for each
source in the `RESEARCH.md` references section.

> *Optional requirement — improves debuggability but is not required for the
> core feature.*

### FR-015 — Unknown or malformed URL (unwanted)

The gatherer **shall not** panic or fail a research run when a source URL has no
parseable host. Such sources **shall** receive the neutral score.

> *Unwanted requirement — prevents a malformed URL from aborting the gather
> pass.*

### FR-016 — Protected domains cannot be demoted below neutral by override (unwanted)

An override file **shall not** be allowed to mark a bundled `excellent` or
`good` domain as `blocked` or `poor` without an explicit `force: true` flag on
that entry. This prevents accidental global demotion of widely trusted domains.

> *Unwanted requirement — protects the integrity of the default trust list.*

## Non-functional Requirements

### NFR-001 — Performance

Reputation lookup for a single source **shall** complete in O(log n) time or
better, where n is the number of table entries, and shall add less than 1 ms of
wall-clock time per source on typical hardware.

### NFR-002 — Determinism

Given the same URL, query, title, snippet, and active reputation table, the
adjusted relevance score **shall** be deterministic across runs.

### NFR-003 — Testability

The reputation lookup and score combination logic **shall** be pure functions
exposed through a module-level API so they can be unit-tested without network
access, configuration files, or a running research session.

### NFR-004 — Backwards compatibility

When reputation scoring is disabled or absent, the existing relevance labels
and retention behaviour **shall** remain unchanged.

## Scope

### In scope

- New `reputation` module in `crates/ragent-research`.
- Bundled curated domain tier table.
- Override-file loading and merging logic.
- Integration of reputation lookup into `compute_relevance_label` or a new
  `adjust_relevance_with_reputation` helper.
- Updated `Source::Web` fields to store `reputation_score` and adjusted label.
- Configuration surface for `research.reputation.enabled` and
  `research.reputation.override_path`.
- Unit tests for lookup, scoring, override merging, and edge cases.
- Integration tests verifying adjusted labels appear in gathered sources and in
  `RESEARCH.md`.

### Out of scope

- Live reputation services or third-party reputation APIs.
- Per-page quality estimation based on content analysis (e.g. LLM-based
  authority scoring).
- Browser history or user feedback loops that mutate the table over time.
- GUI visualisations of reputation scores beyond the parenthetical label.
- Changing the low-relevance pre-filter thresholds; only the label and the
  adjusted score change.

## Risks and Assumptions

- The curated table will be biased toward English-language and technology-focused
  sources. Project-local overrides are the primary mechanism for correcting that
  bias.
- We assume that simple domain/suffix matching is sufficient for the first
  iteration; sub-path matching is out of scope.
- The score mapping may need tuning after user feedback; the constants are
  chosen to be conservative.

## Related Work

- Existing `WebGatherer` and `compute_relevance_label` in
  `crates/ragent-research/src/web_gatherer/relevance.rs`.
- `Source::Web` definition in `crates/ragent-research/src/source.rs`.
- Configuration parsing in `crates/ragent-config`.
- `--use-low-relevance` flag and the `keep_low_relevance` gatherer option.
