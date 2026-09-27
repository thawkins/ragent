# Corpus Analysis Companion (CORPA.md)

Quality-assurance companion document for `Rust stabilized async/.await in version 1.39 but ships no async runtime in its…`. Generated together with `RESEARCH.md`; the `[#N]` source indices reference the Sources Reference table at the bottom of this file.

## Contradiction Graph

_(no contradictions detected among the gathered sources)_

## Loci Analysis

| Locus | Sources | Mentions | Representative Snippets |
|-------|---------|----------|-------------------------|
| Performance | #1, #3, #5, #6 | 4 | se programming language that emphasizes performance, type safety, concurrency, and memory s; aling multi-threaded schedu |
| Accessibility | #2 | 1 | five values — stability, ergonomics, accessibility, integration, and speed — with highli |
| Cost | #5 | 1 | -like API, simplicity, and low learning cost, with crates like tide, surf, and async |
| Quality | #6 | 1 | rtise, community support, documentation quality, and performance benchmarks—citing th |
| Safety | #1 | 1 | guage that emphasizes performance, type safety, concurrency, and memory safety. (Sourc |
| Scalability | #6 | 1 | w 25% annually. The article also covers scalability planning (efficient data structures red |

## Depth Investigation

| Locus | Depth | Sources | Note |
|-------|-------|---------|------|
| Performance | deep | #1, #3, #5 | Detected in 4 sources (depth: deep). |
| Accessibility | surface | #2 | Detected in 1 source (depth: surface). |
| Cost | surface | #5 | Detected in 1 source (depth: surface). |
| Quality | surface | #6 | Detected in 1 source (depth: surface). |
| Safety | surface | #1 | Detected in 1 source (depth: surface). |
| Scalability | surface | #6 | Detected in 1 source (depth: surface). |

## Source Tensions

| Kind | Label | Sources | Note |
|------|-------|---------|------|
| shallow evidence | Accessibility | #2 | surface evidence: only 1 source(s) mention this dimension. |
| shallow evidence | Cost | #5 | surface evidence: only 1 source(s) mention this dimension. |
| shallow evidence | Quality | #6 | surface evidence: only 1 source(s) mention this dimension. |
| shallow evidence | Safety | #1 | surface evidence: only 1 source(s) mention this dimension. |
| shallow evidence | Scalability | #6 | surface evidence: only 1 source(s) mention this dimension. |
| isolated source | Accessibility | #2 | Source #2 only supports one dimension and may represent an outlier or niche view. |
| isolated source | Performance | #3 | Source #3 only supports one dimension and may represent an outlier or niche view. |

## Synthesis Audit

**Overall score:** 95/100

**Recommendation:** Proceed — the synthesis passes the deterministic 4-critic audit.

Synthesis audit for 'Comparison of Rust async runtimes tokio vs async-std --no-tui --yes --depth shallow' scored 95/100 across critics [coverage=83 logic=100 evidence=100 readability=100]; 7/7 sources cited.

| Critic | Score | Status | Issue / Gap Summary |
|--------|-------|--------|---------------------|
| coverage | 83 | pass | Dimension 'Scalability' is not addressed in the synthesis findings or implications |
| logic | 100 | pass | No contradictions detected; no logic conflicts to resolve. |
| evidence | 100 | pass | none |
| readability | 100 | pass | Finding 1 contains a paragraph longer than 1200 characters |

## Corpus Critic

**Overall score:** 54/100 (review)

**Subscores:** coverage 60 | evidence 16 | balance 67 | tension 100

**Issues:**
- Dimension 'Accessibility' has only surface-level support (1 source(s))
- Dimension 'Cost' has only surface-level support (1 source(s))
- Dimension 'Quality' has only surface-level support (1 source(s))
- Dimension 'Safety' has only surface-level support (1 source(s))
- Dimension 'Scalability' has only surface-level support (1 source(s))
- 2 source(s) only support a single dimension and may be outliers

**Evidence gaps:**
- Find additional evidence on 'Accessibility' for 'Comparison of Rust async runtimes tokio vs async-std --no-tui --yes --depth shallow'
- Find additional evidence on 'Cost' for 'Comparison of Rust async runtimes tokio vs async-std --no-tui --yes --depth shallow'
- Find additional evidence on 'Quality' for 'Comparison of Rust async runtimes tokio vs async-std --no-tui --yes --depth shallow'
- Find additional evidence on 'Safety' for 'Comparison of Rust async runtimes tokio vs async-std --no-tui --yes --depth shallow'
- Find additional evidence on 'Scalability' for 'Comparison of Rust async runtimes tokio vs async-std --no-tui --yes --depth shallow'

**Shallow dimensions:** Accessibility, Cost, Quality, Safety, Scalability

**Isolated sources:** #2, #3

## Sources Reference

| # | Type | Media | Language | Path/URL | Title | Author | Published | Relevance | Search tool | Engine | Captured |
|---|------|-------|----------|----------|-------|--------|-----------|-----------|-------------|--------|----------|
| 1 | web | page | English | [https://en.wikipedia.org/wiki/Rust_(programming_language)](https://en.wikipedia.org/wiki/Rust_(programming_language)) | Rust (programming language) | — | — | Encyclopedia — engine-ranked summary | mf_search | wikipedia | 2026-09-07T01:06:56.665031836+00:00 |
| 2 | web | page | English | [https://async.rs/blog](https://async.rs/blog) | async-std - Blog | — | — | Medium — partial query match | mf_search | langsearch | 2026-09-07T01:07:18.984248958+00:00 |
| 3 | web | page | Portuguese | [https://rustlang.com.br/artigos/tokio-vs-async-std](https://rustlang.com.br/artigos/tokio-vs-async-std) | Tokio vs Async-std: Qual Runtime Rust Usar? \| Rust Brasil | [Equipe Rust Brasil] | 2026-02-23 | High — title + snippet match query | mf_search | langsearch | 2026-09-07T01:07:07.005067848+00:00 |
| 4 | web | page | English | [https://www.toolsku.com/en/blog/rust-async-runtime-comparison-2026](https://www.toolsku.com/en/blog/rust-async-runtime-comparison-2026) | Rust Async Runtime Comparison in 2026: Tokio vs… \| Tools Ku | [David Liu] | 2026-04-30 | High — title + snippet match query | mf_search | langsearch | 2026-09-07T01:07:11.570251482+00:00 |
| 5 | web | page | Japanese | [https://qiita.com/Aqua-218/items/b953029e6f1095f53c17](https://qiita.com/Aqua-218/items/b953029e6f1095f53c17) | tokio vs async-std、どっちを選ぶべきか本気で考えた - Qiita | [@aqua_developer] | 2025-12-10 | High — title + snippet match query | mf_search | langsearch | 2026-09-07T01:07:03.287817815+00:00 |
| 6 | web | page | English | [https://moldstud.com/articles/p-rust-asynchronous-programming-comparing-tokio-with-other-libraries](https://moldstud.com/articles/p-rust-asynchronous-programming-comparing-tokio-with-other-libraries) | Rust Asynchronous Programming - Comparing Tokio with Other Libraries | [Ana Crudu] | — | Medium — partial query match | mf_search | langsearch | 2026-09-07T01:07:26.763230981+00:00 |
| 7 | web | page | English | [http://arxiv.org/abs/2202.04431](http://arxiv.org/abs/2202.04431) | Assessing the alignment between the information needs of developers and the documentation of programming languages: A… | [Filipe R. Cogo, Xin Xia, Ahmed E. Hassan] | — | Scholarly — engine-ranked abstract | mf_search | openalex | 2026-09-07T01:06:58.299551068+00:00 |
