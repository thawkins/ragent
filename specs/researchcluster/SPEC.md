---
status: draft
audit:
  - { time: 1788098170, from: "none", to: "draft", actor: "system" }
---
---
status: draft
---
# Research Cluster Specification

## Overview

This specification defines the `cluster` extension to the `/research` slash command family. The `cluster` command analyzes the web-source documents stored under a completed research run's `sources/` folder, extracts the top 10 most important concepts using a fixed analysis prompt, and writes a `CONCEPTS.md` artifact into the same research folder.

## Requirements

### Functional Requirements

**FR-001 (Ubiquitous)**
The system shall provide a `/research cluster <research_id>` slash command in the TUI.

**FR-002 (State-driven)**
While a research run exists with a populated `sources/` folder, the system shall allow the user to invoke `cluster` on that run.

**FR-003 (Event-driven)**
When the user invokes `/research cluster <research_id>`, the system shall read every web-source document from `<research_folder>/sources/`.

**FR-004 (Ubiquitous)**
The system shall assemble the source documents into a single payload bounded by the active LLM provider's context window.

**FR-005 (Event-driven)**
When the source payload is assembled, the system shall send the fixed concept-extraction prompt to the currently selected LLM.

**FR-006 (Ubiquitous)**
The prompt shall instruct the model to identify up to 10 core concepts, each with a concise name, a short definition, and 1-2 brief evidence bullets.

an example prompt would be:

You are an expert data analyst and researcher. Your task is to analyze the provided set of documents and extract a comprehensive set of the most important concepts, themes, and ideas across them.

Please follow these steps:

1. Read through all the provided documents to understand the overall context

2. Identify the core concepts that appear frequently, hold significant weight, or tie the documents together.

3. For ach major concept identified, provide:

   - Concept Name: A concise label (2-4 words).

   - Definition/Description: A short explanation of what this concept means within the context of these documents.

   - Key Evidence/Context: 1-2 brief examples or bullet points showing how or where this concept appears in the text.



Structure your final output using clear markdown headings and bullet points. Avoid overlapping or repeating concepts. Focus on depth and relevance over sheer quantity.



Here are the documents:

[INSERT YOUR DOCUMENTS HERE]

**FR-007 (Event-driven)**
When the LLM returns a response, the system shall write the response as `CONCEPTS.md` into the same research folder as the source documents.

**FR-008 (Unwanted)**
The system shall not overwrite an existing `CONCEPTS.md` without explicit user confirmation.

**FR-009 (State-driven)**
If the requested research folder does not exist, the system shall display an error and refuse to proceed.

**FR-010 (State-driven)**
If the requested research folder exists but has no `sources/` folder, the system shall display an error and refuse to proceed.

**FR-011 (State-driven)**
If the `sources/` folder is empty, the system shall display an error and refuse to proceed.

**FR-012 (Optional)**
The user may supply an optional `--force` flag with `/research cluster <research_id> --force` to bypass the overwrite confirmation.

**FR-013 (Ubiquitous)**
The system shall display progress feedback in the TUI while documents are being read and the LLM call is in flight.

**FR-014 (Ubiquitous)**
The generated `CONCEPTS.md` shall use clear markdown headings and bullet points as instructed by the prompt.

### Non-Functional Requirements

**NFR-001 (Ubiquitous)**
The cluster command shall complete within the standard LLM call timeout used by the research system.

**NFR-002 (Ubiquitous)**
Source-document loading shall respect existing ragent file-size and context-window limits.

## Glossary

- **Research folder:** A directory under `research/<name>/` containing the artifacts for a single research run.
- **Sources folder:** The `sources/` sub-directory inside a research folder that holds the downloaded web-source documents.
- **CONCEPTS.md:** The output artifact produced by the cluster command.

## Assumptions and Constraints

- The cluster command relies on an existing research run that already downloaded web sources.
- The prompt text is fixed and not user-editable through the TUI in the first version.
- Concept extraction is performed by the currently selected LLM provider with its default model settings.
