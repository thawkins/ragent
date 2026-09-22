---
status: draft
audit:
  - { time: 1787956543, from: "none", to: "draft", actor: "system" }
---
# SPEC: Source-Quality Scoreboard for /research Output

## 1. Summary

When ragent produces a research document (`research/<name>/RESEARCH.md`), a reader
currently has to hunt through the document — the Data Quality Summary sits after
"Top 10 Implications" in the report layout — to learn whether the sources behind
the findings are trustworthy. This spec defines a **simple, deterministic,
LLM-free "Corpus Quality Scoreboard"**: a compact, at-a-glance block rendered
near the top of every research document that summarizes the quality of the
source corpus using indicators the pipeline already computes.

The design goal is legibility first: one score, one ASCII meter bar, one grade
band, and a handful of hard facts — no new scoring algorithms, no LLM calls,
no non-ASCII glyphs.

## 2. Survey of Available Corpus-Quality Indicators

Before choosing the display mechanism, the following indicators already exist
in `crates/ragent-research` and can be consumed directly:

| Indicator | Symbol | Type / Range | Source |
|---|---|---|---|
| Overall corpus score | `CorpusCriticReport.score` | `u32`, 0-100 | `corpus_critic.rs` |
| Coverage subscore | `CorpusCriticReport.coverage_score` | `u32`, 0-100 | `corpus_critic.rs` |
| Evidence-depth subscore | `CorpusCriticReport.evidence_score` | `u32`, 0-100 | `corpus_critic.rs` |
| Balance subscore | `CorpusCriticReport.balance_score` | `u32`, 0-100 | `corpus_critic.rs` |
| Tension subscore | `CorpusCriticReport.tension_score` | `u32`, 40-100 | `corpus_critic.rs` |
| Critic pass gate | `CorpusCriticReport.passed` | `bool` (`score >= 60`) | `corpus_critic.rs` |
| Synthesis audit score | `SynthesisAudit.overall_score` | `u32`, 0-100 | `synthesis.rs` |
| Audit recommendation | `SynthesisAudit.recommendation` | `str`: proceed / caution / revise | `synthesis.rs` |
| Contradiction edges | `ContradictionGraph.edges.len()` | `usize` | `contradiction.rs` |
| Strongest edge strength | `ContradictionEdge.strength` | `u8`, 0-100 | `contradiction.rs` |
| Citation check | `CitationCheckResult.passed` / `.gate_open` | `bool` | `cite_checker.rs` |
| Per-source relevance | `Source::relevance_rank()` | `u8`, 1-8 (5 = unknown) | `source.rs` |
| Full body captured | `Source::has_body()` | `bool` (fetch-success proxy) | `source.rs` |
| Publication date | `Source::published_at()` | `Option<DateTime<Utc>>` | `source.rs` |
| Source kind | `Source::type_str()` | web / local / extra-local / spec / other | `source.rs` |
| Provenance | `Source::search_engine()`, `media_type()` | `&str` | `source.rs` |

Gaps deliberately **not** filled by this spec (kept simple): per-domain
diversity scoring, freshness/staleness flags, and any per-source numeric score.
The scoreboard reports raw facts (distinct-domain count, full-text count, date
span) instead of derived judgements for these.

## 3. Chosen Mechanism

A `## Corpus Quality Scoreboard` section, rendered immediately after the
document frontmatter/title and before the Abstract (IMRaD) or Executive
Summary body (report layout), containing:

1. **Score line** — `Quality: 74/100 - Grade B (Good)`.
2. **ASCII meter bar** — a 20-cell proportional bar inside a fenced code block
   (fenced so the TUI research bypass renders it verbatim):
   `[###############-----]  74/100`.
3. **Subscore line** — coverage / evidence / balance / tension when the corpus
   critic is available.
4. **Source facts line(s)** — gathered vs cited counts, full-text count,
   distinct-domain count, average relevance rank, cited date span.
5. **Tension/citation line** — contradiction edge count with strongest strength,
   citation-check status.

Grade bands (aligned with the existing synthesis-audit thresholds at 80/50):

| Band | Range | Meaning |
|---|---|---|
| A | 80-100 | Excellent — proceed |
| B | 65-79 | Good |
| C | 50-64 | Adequate — caution |
| D | 0-49 | Weak — revise |

Score precedence: corpus critic score when present; otherwise synthesis-audit
overall score; otherwise the document is **Not graded** and only the source
facts render.

The existing detailed `render_data_quality_summary` section remains untouched;

- the scoreboard is an at-a-glance summary, not a replacement.

### Example rendering

```markdown
## Corpus Quality Scoreboard

Quality: **74/100** - Grade B (Good)

```
[##############--------]  74/100
```

- Critic: pass (coverage 60 | evidence 80 | balance 70 | tension 100)
- Sources: 42 gathered | 17 cited | 35 full text | 21 distinct domains
- Relevance: 6.4/8 average | Cited date span: 2019-2025 (3 undated)
- Contradictions: 2 edges (strongest 78/100) | Citation check: passed
```

## 4. Requirements (EARS)

EARS templates used: [Ubiquitous], [Event-driven], [State-driven],
[Optional], [Unwanted].

### Ubiquitous

**FR-001** — The system shall render a Corpus Quality Scoreboard section into
every generated research document when at least one quality-evaluation artifact
(corpus critic report, synthesis audit, or the gathered source list) is
available.

**FR-002** — The system shall display the overall corpus quality score as an
integer in the range 0-100 together with its grade band (A 80-100, B 65-79,
C 50-64, D 0-49).

**FR-003** — The system shall render a proportional ASCII meter bar, exactly
20 cells wide, using `#` for filled cells and `-` for empty cells, wrapped in
square brackets and emitted inside a fenced code block.

**FR-004** — The system shall display the source-facts line containing: number
of sources gathered, number cited, number with a captured body, number of
distinct domains (web sources only), average relevance rank (one decimal,
x/8), and the cited publication-date span when any cited source has a
publication date.

**FR-005** — The system shall display the critic subscores (coverage, evidence,
balance, tension) when a corpus critic report is present.

### Event-driven

**FR-006** — When a corpus critic report is absent but a synthesis audit is
present, the system shall derive the displayed score from the synthesis-audit
overall score and derive the displayed grade band from that score.

**FR-007** — When neither a corpus critic report nor a synthesis audit is
present but at least one source was gathered, the system shall display the
score line as `Quality: Not graded` and render only the source-facts and
meter-free parts of the scoreboard.

**FR-008** — When the rendered document is opened via `/research open <name>`
in the TUI overlay, the system shall display the scoreboard content identically
to its in-file rendering.

**FR-009** — When the corpus critic reports unresolved contradiction edges, the
system shall display the contradiction count and the strongest edge strength
on the tension/citation line of the scoreboard.

**FR-010** — When a citation check result is present, the system shall display
its status (`passed` / `failed`) on the tension/citation line.

### State-driven

**FR-011** — While a scoreboard is rendered, the system shall place it
immediately after the document title/frontmatter and before the Abstract
(IMRaD layout) or the first body section (report layout).

**FR-012** — While a scoreboard is rendered, the system shall leave the
existing Data Quality Summary section, its placement, and its content
unchanged.

### Optional

**FR-013** — Where the output format is an abbreviated format
(`executive-summary`, `comparison-table`, or `source-bibliography`), the
system may omit the critic-subscore and tension/citation lines and render only
the score line, meter bar, and source-facts line.

**FR-014** — Where the number of gathered web sources is zero (local-only
runs), the system may omit the distinct-domain count and average relevance
figures from the source-facts line.

### Unwanted

**FR-015** — The system shall not modify any scoring computation of the corpus
critic, synthesis audit, contradiction graph, or citation checker; the
scoreboard is read-only aggregation and rendering.

**FR-016** — The system shall not emit non-ASCII characters (emoji, box-drawing
or block glyphs) in the scoreboard; the meter bar uses only `#`, `-`, `[`, `]`,
spaces, and the grade/score digits.

## 5. Non-Goals

- No new per-source numeric scoring algorithm (relevance, body, date are
  reported as raw facts only).
- No new LLM calls; the scoreboard is fully deterministic.
- No changes to HTTP API payloads; JSON consumers continue to deserialize
  `CorpusCriticReport` / `SynthesisAudit` directly.
- No interactive UI component; the scoreboard is plain Markdown in the
  document.

## 6. Acceptance

- A report-layout and an IMRaD-layout document each show the scoreboard above
  the body with score, grade band, and meter bar.
- Fallback paths (synthesis-audit-only, not-graded) render without panics or
  blank sections.
- The rendered scoreboard block contains only ASCII characters.
- `cargo fmt --check` and `cargo clippy` pass with no new warnings.