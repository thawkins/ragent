---
status: draft
---
# PLAN: Open Deep Research feature-gap integration into ragent

## Overview

This plan implements the requirements defined in `specs/opendeepresearch/SPEC.md`. Work is organized into configuration and request plumbing, scope clarification and brief generation, supervisor/researcher graph execution, competitive analysis mode, summarization and multi-model support, self-evaluation, and UI/API integration. All tasks are initially set to **Pending**.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `--mode` CLI/TUI/HTTP option and configuration schema | FR-001, FR-009 | S | High | completed | none |
| T-002 | Extend `ResearchRunRequest` and `build_session_config` for mode, research brief, and multi-model settings | FR-001, FR-013 | M | High | completed | T-001 |
| T-003 | Implement scope clarification prompt and single-question loop | FR-005, FR-017 | M | High | completed | none |
| T-004 | Add research-brief generator that turns user prompt into detailed mission statement | FR-004 (brief context), FR-009 | M | High | completed | T-003 |
| T-005 | Build supervisor agent state machine and `supervisor` node | FR-001, FR-007, FR-009 | L | High | completed | T-002 |
| T-006 | Build researcher agent node with tool loop and note capture | FR-001, FR-007 | L | High | completed | T-005 |
| T-007 | Implement compressed-research synthesis step per researcher | FR-003, FR-004 | M | High | completed | T-006 |
| T-008 | Wire supervisor/researcher graph into `ResearchSession` and tier router | FR-009 | M | High | completed | T-005, T-006, T-007 |
| T-009 | Implement competitive-analysis entity extractor | FR-006, FR-011 | M | Critical | completed | T-004 |
| T-010 | Implement competitive-analysis delegation: one researcher per entity | FR-006, FR-007 | M | Critical | completed | T-006, T-009 |
| T-011 | Implement comparison-table synthesis and dedicated `comparison-table` format rendering | FR-006, FR-014, FR-016 | L | Critical | completed | T-010 |
| T-012 | Add per-page webpage summarization using `--summarization-model` | FR-002, FR-003, FR-010 | M | High | completed | none |
| T-013 | Persist summarized sources to the research vault with original URL and timestamp | FR-003, FR-004, FR-018 | S | High | completed | T-012 |
| T-014 | Support distinct `research_model`, `compression_model`, and `final_report_model` in `ragent.json` and CLI | FR-002, FR-013 | M | Medium | completed | T-002 |
| T-015 | Add self-evaluation prompt/scorecard for quality, relevance, groundedness, completeness, structure | FR-008, FR-015, FR-019 | L | Medium | completed | T-011 |
| T-016 | Add `--evaluate` CLI flag and `research.evaluate.enabled` config | FR-015 | S | Medium | completed | T-015 |
| T-017 | Update TUI slash-command completer for `/research --mode` and new flags | FR-001, FR-012, FR-014 | S | Medium | completed | T-001 |
| T-018 | Update HTTP server research routes to accept `mode`, `summarization_model`, and `evaluate` | FR-001, FR-014, FR-015 | M | Medium | completed | T-002, T-016 |
| T-019 | Write crate-level tests for clarification, brief generation, and entity extraction | FR-005, FR-011 | M | High | completed | T-003, T-004, T-009 |
| T-020 | Write integration test for competitive-analysis end-to-end run | FR-006, FR-016 | L | Critical | completed | T-011, T-013 |
| T-021 | Document new `/research` modes and flags in `QUICKSTART.md` and `SPEC.md` | all | S | Low | completed | T-011, T-018 |