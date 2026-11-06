//! Tests for the deck enrichment stage (spec `diffdeck`: FR-023, FR-024).
//!
//! Every case is a pure in-memory assertion: the report is a string literal,
//! the model is a fake that records the prompt it was handed and returns a
//! fixed reply, and no fixture is written to disk. A failure is therefore
//! always an enrichment rule and never an environment artefact.
//!
//! The cases are grouped by the requirement they pin:
//!
//! - FR-023 - each finding's `**Observation:**` / `**Analysis:**` prose becomes
//!   one paragraph under the finding's headline;
//! - FR-024 - each Top Implication lists the findings ranked most relevant to
//!   it;
//! - NFR-002 - with no model wired in, the enrichment is deterministic and
//!   spends no tokens;
//! - NFR-003 - a model that fails degrades to the deterministic values rather
//!   than failing the run.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::NaiveDate;
use ragent_research::deck::{
    DeckElement, DeckRenderOptions, ParsedReport, diff_reports, parse_report, render_deck,
};
use ragent_research::deck_enrich::{
    DeckEnrichment, DeckModel, MAX_FINDINGS_PER_CONCEPT, clean_summary, deterministic_enrichment,
    enrich_deck, parse_relevance_reply, parse_summary_reply,
};

/// Parse a report that is expected to succeed.
fn parse_ok(text: &str) -> ParsedReport {
    match parse_report(text) {
        Ok(parsed) => parsed,
        Err(err) => panic!("expected the report to parse, got: {err}"),
    }
}

/// The generation date every render case uses, so output is comparable.
fn generated_on() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 4, 1).expect("2026-04-01 is a real date")
}

/// A report carrying two concepts and two findings that share vocabulary with
/// one concept each.
const TWO_FINDINGS_REPORT: &str = "\
# Title: Cost and Governance

## Executive Summary

The report surveys inference cost and agent governance.

## Concepts

### 1. Token Cost Per Outcome

**Definition:** Token cost per outcome is the spend of a completed agent workload rather than its price per token [#4].

### 2. Runtime Governance

**Definition:** Runtime governance is enforcement of agent policy in the request path, with audit evidence for every decision [#7].

## Findings

### **Finding 1** - Token cost per outcome drives agent margin

**Observation:**
Token prices collapsed while agent bills rose, because reasoning workloads consume far more tokens per outcome than chat.

**Analysis:**
A second sentence of analysis that must not survive the first-sentence fallback. The unit economics only hold when spend is attributed to completed work.

**Implication:**
Meter internally and price externally.

**Sources:**
- [4] Cost notes - [https://example.com/cost](https://example.com/cost) (published 2026-03-01)

**Source date range:** 2026-03-01..2026-03-01 (1 of 1 cited web sources dated)


### **Finding 2** - Runtime governance closes the approval gap

**Observation:**
Paper approvals leave the gateway without context, so a policy document can approve an agent nobody can audit.

**Analysis:**
Enforcement in the request path generates audit evidence automatically at the moment of action.

**Sources:**
- [7] Governance notes - [https://example.com/gov](https://example.com/gov) (published 2026-03-02)

**Source date range:** 2026-03-02..2026-03-02 (1 of 1 cited web sources dated)
";

/// A fake model that records every call and returns canned replies.
#[derive(Default)]
struct FakeModel {
    summary_reply: Option<String>,
    relevance_reply: Option<String>,
    fail: bool,
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl DeckModel for FakeModel {
    async fn summarize_finding(
        &self,
        headline: &str,
        blocks: &[String],
        _max_chars: usize,
    ) -> anyhow::Result<String> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(format!("summary:{headline}:{}", blocks.len()));
        }
        if self.fail {
            anyhow::bail!("provider unavailable");
        }
        Ok(self
            .summary_reply
            .clone()
            .unwrap_or_else(|| "A model-written paragraph.".to_string()))
    }

    async fn rank_concept_findings(
        &self,
        concepts: &[(String, String)],
        findings: &[(String, String)],
    ) -> anyhow::Result<String> {
        if let Ok(mut calls) = self.calls.lock() {
            calls.push(format!("rank:{}:{}", concepts.len(), findings.len()));
        }
        if self.fail {
            anyhow::bail!("provider unavailable");
        }
        Ok(self
            .relevance_reply
            .clone()
            .unwrap_or_else(|| "1: 1\n2: 2".to_string()))
    }
}

// -- FR-023: the finding observation and analysis become one paragraph --------

#[test]
fn test_parse_finding_detail_carries_observation_analysis_and_implication() {
    let parsed = parse_ok(TWO_FINDINGS_REPORT);
    let findings = parsed.items(DeckElement::Findings);
    assert_eq!(findings.len(), 2);
    let first = findings.first().expect("the report declares two findings");
    assert_eq!(first.detail.len(), 3, "detail: {:?}", first.detail);
    assert!(
        first.detail[0].starts_with("Token prices collapsed"),
        "observation first: {:?}",
        first.detail
    );
    assert!(
        first.detail[1].starts_with("A second sentence of analysis"),
        "analysis second: {:?}",
        first.detail
    );
    assert!(
        first.detail[2].starts_with("Meter internally"),
        "implication third: {:?}",
        first.detail
    );
}

#[test]
fn test_parse_finding_detail_excludes_the_sources_bullets() {
    let parsed = parse_ok(TWO_FINDINGS_REPORT);
    let first = parsed
        .items(DeckElement::Findings)
        .first()
        .expect("the report declares two findings");
    for block in &first.detail {
        assert!(
            !block.contains("example.com"),
            "a sources bullet must not become summary prose: {block}"
        );
        assert!(
            !block.contains("Source date range"),
            "the date-range note must not become summary prose: {block}"
        );
    }
}

#[test]
fn test_deterministic_summary_joins_the_first_sentence_of_each_block() {
    let parsed = parse_ok(TWO_FINDINGS_REPORT);
    let first = parsed
        .items(DeckElement::Findings)
        .first()
        .expect("the report declares two findings");
    let summary = first.deterministic_summary();
    assert!(
        summary.contains(
            "Token prices collapsed while agent bills rose, because reasoning workloads consume \
             far more tokens per outcome than chat."
        ),
        "summary: {summary}"
    );
    assert!(
        summary.contains(
            "A second sentence of analysis that must not survive the first-sentence fallback."
        ),
        "summary: {summary}"
    );
    assert!(
        !summary.contains("The unit economics only hold"),
        "only the first sentence of a block survives: {summary}"
    );
    assert!(
        !summary.contains('\n'),
        "the paragraph is one line: {summary:?}"
    );
}

#[test]
fn test_deterministic_summary_is_empty_for_an_item_with_no_detail() {
    let parsed = parse_ok("## Executive Summary\n\nOne paragraph only.\n");
    let item = parsed
        .items(DeckElement::ExecutiveSummary)
        .first()
        .expect("the summary parsed");
    assert_eq!(item.deterministic_summary(), "");
}

// -- FR-024/FR-026: concepts own their findings -------------------------------

#[test]
fn test_deterministic_enrichment_groups_the_related_finding_under_each_concept() {
    let newer = parse_ok(TWO_FINDINGS_REPORT);
    let older = newer.clone();
    let enrichment = deterministic_enrichment(&newer, &older);
    assert!(!enrichment.used_model, "no model was consulted");

    let concepts = newer.items(DeckElement::Concepts);
    let findings = newer.items(DeckElement::Findings);
    assert_eq!(concepts.len(), 2);
    assert_eq!(findings.len(), 2);

    let cost_finding = findings.first().expect("first finding");
    let gov_finding = findings.get(1).expect("second finding");

    let cost_ranked = enrichment.findings_for(&concepts[0], findings);
    assert_eq!(
        cost_ranked.first().map(|finding| &finding.key),
        Some(&cost_finding.key),
        "the token-cost concept groups the token-cost finding first"
    );

    let gov_ranked = enrichment.findings_for(&concepts[1], findings);
    assert_eq!(
        gov_ranked.first().map(|finding| &finding.key),
        Some(&gov_finding.key),
        "the governance concept groups the governance finding first"
    );

    let claimed = enrichment.claimed_finding_keys();
    assert!(claimed.contains(cost_finding.key.as_str()), "{claimed:?}");
    assert!(claimed.contains(gov_finding.key.as_str()), "{claimed:?}");
}

#[test]
fn test_deterministic_enrichment_lists_no_finding_for_an_unrelated_concept() {
    let report = "\
## Concepts

### 1. Technology And Financing Horizons

**Definition:** Debt service and productivity gains are measured on different horizons.

## Findings

### **Finding 1** - Runtime governance closes the approval gap

**Observation:**
Paper approvals leave the gateway without context.

**Analysis:**
Request-path enforcement generates audit evidence.
";
    let newer = parse_ok(report);
    let enrichment = deterministic_enrichment(&newer, &newer);
    let concepts = newer.items(DeckElement::Concepts);
    let findings = newer.items(DeckElement::Findings);
    assert!(
        enrichment.findings_for(&concepts[0], findings).is_empty(),
        "an unrelated finding is not grouped under the concept"
    );
    assert!(
        enrichment.claimed_finding_keys().is_empty(),
        "an ungrouped finding stays available to the Other Findings slide"
    );
}

#[test]
fn test_deterministic_enrichment_caps_findings_per_concept() {
    let mut report = String::from(
        "## Concepts\n\n### 1. Token Cost Per Outcome\n\n**Definition:** Token cost per outcome is the spend of a completed workload.\n\n## Findings\n\n",
    );
    for index in 1..=6 {
        report.push_str(&format!(
            "### **Finding {index}** - Token cost per outcome number {index}\n\n\
             **Observation:**\nToken cost per outcome drives the margin of agent workload {index}.\n\n\
             **Analysis:**\nToken spend per outcome is the metric that matters.\n\n"
        ));
    }
    let newer = parse_ok(&report);
    let enrichment = deterministic_enrichment(&newer, &newer);
    let concepts = newer.items(DeckElement::Concepts);
    let findings = newer.items(DeckElement::Findings);
    let ranked = enrichment.findings_for(&concepts[0], findings);
    assert_eq!(ranked.len(), MAX_FINDINGS_PER_CONCEPT);
}

// -- The model path ---------------------------------------------------------

#[test]
fn test_parse_summary_reply_strips_a_label_and_caps_the_length() {
    let reply = "Summary: \"A model paragraph that runs on and on.\"";
    let cleaned = parse_summary_reply(reply, 200).expect("a usable paragraph");
    assert_eq!(cleaned, "A model paragraph that runs on and on.");

    let long = "word ".repeat(400);
    let capped = parse_summary_reply(&long, 60).expect("a usable paragraph");
    assert!(capped.chars().count() <= 63, "capped: {capped:?}");
    assert!(capped.ends_with("..."), "capped: {capped:?}");
}

#[test]
fn test_clean_summary_rejects_an_empty_reply() {
    assert_eq!(clean_summary("", 100), None);
    assert_eq!(clean_summary("   \"\"  ", 100), None);
    assert_eq!(clean_summary("***", 100), None);
}

#[test]
fn test_clean_summary_escapes_markdown_that_would_break_the_slide() {
    let cleaned =
        clean_summary("A pipe | and a `tick` and a #hash", 200).expect("a usable paragraph");
    assert!(cleaned.contains("\\|"), "escaped pipe: {cleaned}");
    assert!(cleaned.contains("\\`"), "escaped backtick: {cleaned}");
}

#[test]
fn test_parse_relevance_reply_accepts_the_loose_formats_a_model_emits() {
    let reply = "\
1: 1, 2
2) Finding 3
**3**: 4; 5
- implication 4 : 6
not a mapping
0: 1
";
    assert_eq!(
        parse_relevance_reply(reply),
        vec![(1, 1), (1, 2), (2, 3), (3, 4), (3, 5), (4, 6)]
    );
}

#[tokio::test]
async fn test_enrich_deck_uses_the_model_summary_and_ranking() {
    let newer = parse_ok(TWO_FINDINGS_REPORT);
    let model = FakeModel {
        summary_reply: Some("A model paragraph per finding.".to_string()),
        relevance_reply: Some("1: 1\n2: 2".to_string()),
        fail: false,
        calls: Arc::new(Mutex::new(Vec::new())),
    };
    let enrichment = enrich_deck(&newer, newer.clone(), Some(&model)).await;
    assert!(enrichment.used_model);

    let findings = newer.items(DeckElement::Findings);
    let first = findings.first().expect("first finding");
    assert_eq!(
        enrichment.summary_for(first).as_deref(),
        Some("A model paragraph per finding.")
    );

    // Two summary calls (one per finding) and one ranking call.
    let calls = model.calls.lock().expect("the fake records its calls");
    assert_eq!(
        calls
            .iter()
            .filter(|call| call.starts_with("summary:"))
            .count(),
        2,
        "calls: {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call == "rank:2:2"),
        "the ranking call names both lists: {calls:?}"
    );
}

#[tokio::test]
async fn test_enrich_deck_without_a_model_does_not_call_anything() {
    let newer = parse_ok(TWO_FINDINGS_REPORT);
    let enrichment = enrich_deck(&newer, newer.clone(), None).await;
    assert!(!enrichment.used_model);
    assert!(enrichment.summaries.is_empty());
    assert!(!enrichment.concept_findings.is_empty(), "still grouped");
}

#[tokio::test]
async fn test_enrich_deck_falls_back_when_the_model_fails() {
    let newer = parse_ok(TWO_FINDINGS_REPORT);
    let model = FakeModel {
        summary_reply: None,
        relevance_reply: None,
        fail: true,
        calls: Arc::new(Mutex::new(Vec::new())),
    };
    let enrichment = enrich_deck(&newer, newer.clone(), Some(&model)).await;
    assert!(!enrichment.used_model, "a failed call is not a used model");

    let findings = newer.items(DeckElement::Findings);
    let first = findings.first().expect("first finding");
    // The fallback paragraph still describes the finding.
    let summary = enrichment
        .summary_for(first)
        .expect("the deterministic paragraph");
    assert!(summary.starts_with("Token prices collapsed"), "{summary}");

    // The deterministic grouping is still in place.
    let concepts = newer.items(DeckElement::Concepts);
    assert!(
        !enrichment.findings_for(&concepts[0], findings).is_empty(),
        "the term-overlap grouping survives a failed call"
    );
}

#[tokio::test]
async fn test_enrich_deck_keeps_the_deterministic_ranking_when_the_reply_names_nothing() {
    let newer = parse_ok(TWO_FINDINGS_REPORT);
    let model = FakeModel {
        summary_reply: None,
        relevance_reply: Some("no numbers here at all".to_string()),
        fail: false,
        calls: Arc::new(Mutex::new(Vec::new())),
    };
    let enrichment = enrich_deck(&newer, newer.clone(), Some(&model)).await;
    let concepts = newer.items(DeckElement::Concepts);
    let findings = newer.items(DeckElement::Findings);
    assert!(
        !enrichment.findings_for(&concepts[0], findings).is_empty(),
        "an unusable reply must not blank the grouped findings"
    );
}

// -- Rendering ----------------------------------------------------------------

/// The render options for an empty diff of `newer`, with `enrichment` applied.
fn render_with(newer: &ParsedReport, enrichment: DeckEnrichment) -> String {
    let diff = diff_reports(newer, newer);
    let pair = ragent_research::deck::ReportSeries::classify(&[
        "RESEARCH-2026-01-01.md".to_string(),
        "RESEARCH-2026-02-01.md".to_string(),
    ])
    .select_pair("deck", None, None)
    .expect("two dated reports select a pair");
    // An enriched deck always carries the newer report's findings, which is
    // what the concept slides list; `DeckRenderOptions::new` does this for a
    // real run, but a test-built option set has to say so explicitly.
    let findings = newer.items(DeckElement::Findings).to_vec();
    let options = DeckRenderOptions::new("deck", generated_on(), &pair, newer, newer)
        .with_include_old(true)
        .with_findings(findings)
        .with_enrichment(enrichment);
    render_deck(&diff, &options).markdown
}

/// The `(concept name, slide body)` pairs of a rendered deck.
///
/// A concept slide is a `## <name>` section of the Concepts element; the helper
/// splits the deck on those headings and keeps the sections that carry a
/// concept's status line, so a case can assert which findings sit under which
/// concept without depending on the surrounding slides.
fn concept_layout(deck: &str) -> Vec<(String, String)> {
    let mut layout = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    for line in deck.lines() {
        if let Some(name) = line.strip_prefix("## ") {
            if let Some((name, body)) = current.take() {
                layout.push((name, body));
            }
            current = Some((name.to_string(), Vec::new()));
            continue;
        }
        if let Some((_, body)) = current.as_mut() {
            body.push(line);
        }
    }
    if let Some((name, body)) = current {
        layout.push((name, body));
    }
    layout
        .into_iter()
        .filter(|(_, body)| {
            body.iter()
                .any(|line| line.starts_with("_`") && line.ends_with("`_"))
        })
        .map(|(name, body)| (name, body.join("\n")))
        .collect()
}

#[test]
fn test_render_puts_a_paragraph_under_each_finding_headline() {
    let newer = parse_ok(TWO_FINDINGS_REPORT);
    let deck = render_with(&newer, DeckEnrichment::default());
    assert!(
        deck.contains("### `OLD` Token cost per outcome drives agent margin"),
        "finding headline as a sub-heading:\n{deck}"
    );
    assert!(
        deck.contains("Token prices collapsed while agent bills rose"),
        "the observation sentence is under the headline:\n{deck}"
    );
    assert!(
        deck.contains("A second sentence of analysis"),
        "the analysis sentence is under the headline:\n{deck}"
    );
}

#[test]
fn test_render_prefers_the_model_summary_over_the_fallback() {
    let newer = parse_ok(TWO_FINDINGS_REPORT);
    let findings = newer.items(DeckElement::Findings);
    let mut enrichment = DeckEnrichment::default();
    let key = findings.first().expect("first finding").key.clone();
    enrichment
        .summaries
        .insert(key, "Written by the model.".to_string());
    let deck = render_with(&newer, enrichment);
    assert!(deck.contains("Written by the model."), "{deck}");
    assert!(
        !deck.contains("Token prices collapsed while agent bills rose"),
        "the fallback is not rendered when a model summary exists:\n{deck}"
    );
}

#[test]
fn test_render_groups_each_finding_under_its_concept() {
    let newer = parse_ok(TWO_FINDINGS_REPORT);
    let deck = render_with(&newer, deterministic_enrichment(&newer, &newer));
    let layout = concept_layout(&deck);
    assert_eq!(layout.len(), 2, "one slide per concept:\n{deck}");
    assert_eq!(
        layout[0].0, "Token Cost Per Outcome",
        "the first concept leads:\n{deck}"
    );
    assert!(
        layout[0]
            .1
            .contains("Token cost per outcome drives agent margin"),
        "the token-cost finding sits under the token-cost concept:\n{deck}"
    );
    assert!(
        !layout[0]
            .1
            .contains("Runtime governance closes the approval gap"),
        "a finding is grouped under one concept only:\n{deck}"
    );
    assert_eq!(layout[1].0, "Runtime Governance", "{deck}");
    assert!(
        layout[1]
            .1
            .contains("Runtime governance closes the approval gap"),
        "the governance finding sits under the governance concept:\n{deck}"
    );
}

#[test]
fn test_render_puts_a_concepts_definition_and_status_above_its_findings() {
    let newer = parse_ok(TWO_FINDINGS_REPORT);
    let deck = render_with(&newer, deterministic_enrichment(&newer, &newer));
    assert!(
        deck.contains("_`OLD`_"),
        "a concept slide carries its status tag:\n{deck}"
    );
    assert!(
        deck.contains("Token cost per outcome is the spend of a completed agent workload"),
        "a concept slide carries its definition:\n{deck}"
    );
}

#[test]
fn test_render_marks_a_concept_with_no_finding() {
    let report = "\
## Concepts

### 1. An Orphan Concept

**Definition:** Nothing in the findings speaks to this.

## Findings

### **Finding 1** - An unrelated finding about gardening
";
    let newer = parse_ok(report);
    let deck = render_with(&newer, deterministic_enrichment(&newer, &newer));
    assert!(
        deck.contains("_No finding maps to this concept._"),
        "an empty concept says so:\n{deck}"
    );
}

#[test]
fn test_render_keeps_an_unclaimed_finding_on_the_other_findings_slide() {
    let report = "\
## Concepts

### 1. Token Cost Per Outcome

**Definition:** Token cost per outcome is the spend of a completed workload.

## Findings

### **Finding 1** - Token cost per outcome drives agent margin

**Observation:**
Token cost per outcome rose while prices fell.

**Analysis:**
Spend per completed work is the metric.

### **Finding 2** - Warehouse-native observability economics

**Observation:**
Hyperscaler bundling pressures standalone observability vendors.

**Analysis:**
A 34 percent CAGR does not imply the standalone vendors keep it.
";
    let newer = parse_ok(report);
    let deck = render_with(&newer, deterministic_enrichment(&newer, &newer));
    assert!(
        deck.contains("## Other Findings"),
        "the residual findings keep a slide:\n{deck}"
    );
    let other = deck
        .split("## Other Findings")
        .nth(1)
        .expect("the Other Findings slide exists");
    assert!(
        other.contains("Warehouse-native observability economics"),
        "the ungrouped finding is on it:\n{deck}"
    );
    assert!(
        !other.contains("Token cost per outcome drives agent margin"),
        "a grouped finding is not repeated there:\n{deck}"
    );
}

#[test]
fn test_render_never_emits_an_implications_or_open_questions_slide() {
    let report = "\
## Top 10 Implications

1. An implication that must not reach the deck.

## Open Questions

- A question that must not reach the deck?

## Concepts

### 1. A Concept

**Definition:** A definition.

## Findings

### **Finding 1** - A finding about a concept
";
    let newer = parse_ok(report);
    let deck = render_with(&newer, deterministic_enrichment(&newer, &newer));
    assert!(!deck.contains("Top Implications"), "{deck}");
    assert!(!deck.contains("Open Questions"), "{deck}");
    assert!(!deck.contains("must not reach the deck"), "{deck}");
}

#[test]
fn test_render_keeps_a_headline_only_finding_as_a_bullet() {
    let report = "\
## Findings

### **Finding 1** - A headline with no prose at all
";
    let newer = parse_ok(report);
    let deck = render_with(&newer, DeckEnrichment::default());
    assert!(
        deck.contains("`OLD` A headline with no prose at all"),
        "a prose-less finding keeps the bullet form:\n{deck}"
    );
}
