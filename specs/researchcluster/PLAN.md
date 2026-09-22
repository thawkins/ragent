---
status: draft
---

# Research Cluster Implementation Plan

## Overview

This plan describes the implementation of the `/research cluster` slash-command extension, which analyzes web-source documents from a completed research run and produces a `CONCEPTS.md` artifact using a fixed concept-extraction prompt.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `cluster` slash-command parsing to `/research` handler | FR-001, FR-002 | S | High | completed | — |
| T-002 | Locate research folder and validate `sources/` folder exists | FR-009, FR-010, FR-011 | S | High | completed | — |
| T-003 | Read and concatenate web-source documents with context-window guard | FR-003, FR-004, NFR-002 | M | High | completed | T-002 |
| T-004 | Build fixed concept-extraction prompt with documents inserted | FR-005, FR-006, FR-014 | S | High | completed | T-003 |
| T-005 | Dispatch prompt to active LLM and stream/await response | FR-005, NFR-001 | M | High | completed | T-004 |
| T-006 | Write `CONCEPTS.md` to research folder with overwrite guard | FR-007, FR-008 | S | High | completed | T-005 |
| T-007 | Implement optional `--force` flag for overwrite bypass | FR-012 | S | Medium | completed | T-001 |
| T-008 | Add TUI progress feedback during read and LLM phases | FR-013 | S | Medium | completed | T-003, T-005 |
| T-009 | Wire command into research CLI runner if CLI path is supported | FR-001 | S | Medium | completed | T-001 |
| T-010 | Review generated artifact formatting and prompt wording | FR-014 | S | Low | completed | T-006 |