---
title: "Ragent-Core Performance Benchmark Guide"
source: "benchmark-guide"
type: source
tags: [benchmarking, performance, ragent-core, cargo, criterion, profiling, flamegraph, CI/CD, testing]
generated: "2026-04-18T15:13:06.922691261+00:00"
---

# Ragent-Core Performance Benchmark Guide

This document provides instructions for running and interpreting performance benchmarks for the ragent-core library. It covers how to execute all five benchmark suites using cargo bench, run individual benchmark functions, and generate flame graphs for profiling on Linux. The five benchmark suites measure different aspects of the system: file operations with EditStaging::commit, orchestrator coordination and job submission overhead, snapshot operations (take, restore, and incremental save), team communication via mailboxes, and tool execution performance for glob and read operations with LRU caching. Results are stored in HTML format under target/criterion/ for interactive visualization, and machine-readable JSON estimates are available for CI integration and regression detection.

## Related

### Entities

- [ragent-core](../entities/ragent-core.md) — product
- [Criterion](../entities/criterion.md) — technology
- [cargo bench](../entities/cargo-bench.md) — technology
- [cargo flamegraph](../entities/cargo-flamegraph.md) — technology
- [perf](../entities/perf.md) — technology
- [EditStaging](../entities/editstaging.md) — technology
- [Coordinator](../entities/coordinator.md) — technology
- [GlobTool](../entities/globtool.md) — technology
- [ReadTool](../entities/readtool.md) — technology
- [Mailbox](../entities/mailbox.md) — technology

### Concepts

- [Performance Benchmarking](../concepts/performance-benchmarking.md)
- [Flame Graph](../concepts/flame-graph.md)
- [Regression Budget](../concepts/regression-budget.md)
- [LRU Caching](../concepts/lru-caching.md)
- [Concurrency Levels](../concepts/concurrency-levels.md)
- [Incremental Save](../concepts/incremental-save.md)
- [Metrics Snapshot](../concepts/metrics-snapshot.md)

