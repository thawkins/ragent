//! Relevance-scoring benchmark (PERF-068).
//!
//! Compares the old per-candidate cost (rebuild the normalised query and its
//! morphological variants for every candidate) against the shipped behaviour
//! (build the `PreparedQuery` once per sub-query, reuse it across candidates).

#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use ragent_research::web_gatherer::PreparedQuery;

const QUERY: &str = "how to configure and operate agentic AI loops";

fn candidates() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        (
            "Designing agentic loops",
            "Patterns for structuring autonomous agent systems.",
            "https://simonwillison.net/2025/Sep/30/designing-agentic-loops",
        ),
        (
            "What is an agentic loop? (And how to build one)",
            "A practical guide to building agentic loops in production.",
            "https://www.inngest.com/blog/agent-loop-architecture",
        ),
        (
            "Semantic Role Labeling: A Systematical Survey",
            "We survey semantic role labeling methods.",
            "https://arxiv.org/html/2502.08660v1",
        ),
        (
            "Agentic AI engineering patterns",
            "How to operate AI agents reliably at scale.",
            "https://example.com/agentic-ai-engineering",
        ),
    ]
}

fn bench_relevance(c: &mut Criterion) {
    let hits = candidates();

    c.bench_function("relevance_rebuild_query_per_candidate", |b| {
        b.iter(|| {
            for (title, snippet, url) in &hits {
                // Old behaviour: query normalised inside every call.
                let prepared = PreparedQuery::new(QUERY);
                let _ = prepared.label(title, snippet, url);
            }
        });
    });

    let prepared = PreparedQuery::new(QUERY);
    c.bench_function("relevance_prepared_query", |b| {
        b.iter(|| {
            for (title, snippet, url) in &hits {
                // Shipped behaviour: query prepared once, reused per candidate.
                let _ = prepared.label(title, snippet, url);
            }
        });
    });
}

criterion_group!(benches, bench_relevance);
criterion_main!(benches);
