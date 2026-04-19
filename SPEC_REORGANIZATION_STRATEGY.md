# SPEC.md Structural Reorganization Strategy

**Date:** 2025-01-16  
**Status:** Recommended for Implementation  
**Scope:** 25+ sections, 7,514 lines  

---

## Executive Summary

The SPEC.md document has grown to 7,514 lines and currently suffers from poor logical organization. Sections are grouped by feature implementation rather than by user workflow or logical dependency. This causes:

1. **Numbering conflict** — Section 8 used twice (Memory System AND Teams)
2. **Scattered related content** — Storage systems split across 3 sections, integrations across 3 sections
3. **Poor navigation** — Users looking for related features must jump between distant sections
4. **Unclear flow** — No clear progression from foundational to advanced topics

**Proposed Solution:** Reorganize into 7 logical parts with clear progression and improved grouping.

---

## Current State Analysis

### Numbering Issues

**Critical Issue: Duplicate Section 8**
```
Line 2036: ## 8. Memory System
Line 2950: ## 8. Teams  ← WRONG! Should be 9
```

**Subsection Numbering Issues**
- Memory System subsections labeled "7.1", "7.2", etc. (should be "8.1", "8.2", etc.)
- This cascades errors through all subsequent sections

### Logical Grouping Problems

| Category | Current Locations | Problem |
|----------|------------------|---------|
| **Storage/Data Systems** | 6 (Code Index), 7 (AIWiki), 8 (Memory) | Spread across 3 sections; logically related but separated |
| **Integrations** | 19 (LSP), 20 (GitLab), 21 (MCP) | Good proximity but low in doc; users seeking integrations must scroll far |
| **Reference Materials** | 16, 17, 18, 25, 26 | Scattered (tools, office, CLI, testing); not grouped together |
| **Coordination** | 8 (Teams), 9 (Swarm) | Should include Orchestrator & Event Bus as they're related |
| **Architecture** | Sections 22-23 | Not grouped; treated as separate topics but are interdependent |

### Navigation Challenges

Users following these workflows encounter poor section organization:

**New User Workflow:** "I want to understand ragent"
- Current path: 1 (Overview) → 2 (Architecture) → 3 (Core Features) → 4 (TUI) → 5 (API) ✅ Good
- But then: 6 (Code Index) — This is advanced! Not where new users expect

**Developer Workflow:** "I want to build agents with custom skills and integrations"
- Current path: Must jump 15, 11, 12, 14, 19, 20, 21 (sections scattered)
- New path would be: 13 (Custom Agents) → 14 (Skills) → 15 (Prompt Opt) → 16 (Config) → 17-19 (Integrations)

**DevOps Workflow:** "I need comprehensive reference materials"
- Current path: Must find 16, 17, 18, 25, 26 (scattered across document)
- New path would be: 20 (Tools) → 21 (Office/PDF) → 22 (CLI) → 23 (Testing)

---

## Proposed Structure

### Part I: Foundation & Basics (Sections 1-5)
**Purpose:** Everything needed to understand ragent and start using it  
**User:** First-time users, decision makers  
**Flow:** Understand → Visualize → Use

| # | Section | Lines | Purpose |
|---|---------|-------|---------|
| 1 | Overview | 14 | What is ragent, key characteristics |
| 2 | Architecture | 42 | How it's built (5 crates, event bus, providers, tools) |
| 3 | Core Features | 354 | LLM providers (8), tool system (147+), agents, sessions |
| 4 | Terminal User Interface | 477 | Interactive TUI with 21 windows/overlays, commands, keybindings |
| 5 | HTTP Server & API | 167 | REST API (80+ endpoints), SSE streaming, authentication |

**Key Insight:** Sections 1-5 provide complete coverage from "What is ragent?" to "How do I use it?"

---

### Part II: Data & Knowledge Systems (Sections 6-8)
**Purpose:** All persistent storage, indexing, and knowledge management  
**User:** Developers leveraging advanced features  
**Relationships:** These three systems work together — Code Index finds code, Memory remembers context, AIWiki organizes knowledge

| # | Section | Lines | Purpose | Relationship |
|---|---------|-------|---------|--------------|
| 6 | Code Index | 589 | Codebase indexing (15+ languages, tree-sitter, Tantivy FTS) | Input: source code → Output: searchable symbols |
| 7 | Memory System | 914 | 3-tier persistence (blocks, SQLite store, embeddings) | Ingests: code knowledge, decisions → Enables: smart suggestions |
| 8 | AIWiki Knowledge Base | 253 | Project knowledge base (ingestion, web interface, analysis) | Inputs: documents, code → Outputs: knowledge graphs, Q&A |

**Key Insight:** These three systems form the "knowledge layer" that makes ragent intelligent. They're independent but complementary.

---

### Part III: Multi-Agent Coordination (Sections 9-12)
**Purpose:** Team operations, decomposition, autonomous execution, orchestration  
**User:** Advanced users managing complex tasks  
**Flow:** Teams → Decomposition → Autonomous Loops → Orchestration
**Dependency Chain:** Teams (basic) → Swarm (decomposition) → Autopilot (autonomous) → Orchestrator (management)

| # | Section | Lines | Purpose | Builds On |
|---|---------|-------|---------|-----------|
| 9 | Teams & Multi-Agent Coordination | 702 | Named teammates, tasks, messaging | Core foundation |
| 10 | Swarm Mode & Decomposition | 488 | Auto-decompose goals into parallel tasks | Teams |
| 11 | Autopilot Mode | 31 | Autonomous iteration limits, permission auto-approval | Swarm |
| 12 | Orchestrator & Event Bus | 334 | Multi-agent coordination, pub/sub system | All of above |

**Key Insight:** These four sections should be together because they build progressively: Teams → Swarm decomposition → Autopilot execution → Orchestrator management.

---

### Part IV: Customization & Extension (Sections 13-16)
**Purpose:** Tailoring ragent to your needs  
**User:** Users customizing behavior  
**Flow:** Agents → Skills → Optimization → Configuration

| # | Section | Lines | Purpose |
|---|---------|-------|---------|
| 13 | Custom Agents & Agent System | ~200 | OASF-based agent definitions, templates, permissions |
| 14 | Skills System | 395 | Loadable skill packs (tools, prompts, context) |
| 15 | Prompt Optimization | 148 | 12 transformation methods (CO-STAR, CRISPE, CoT, etc.) |
| 16 | Configuration & Setup | 61 | ragent.json schema, environment variables |

**Key Insight:** This part is about personalization — adapting the base tool to your specific workflow.

---

### Part V: External Integrations (Sections 17-19)
**Purpose:** Connecting ragent to external systems  
**User:** Developers integrating with existing tools  
**Type:** All optional; all expand capabilities via external connections

| # | Section | Lines | Purpose |
|---|---------|-------|---------|
| 17 | LSP Integration | 205 | Language Server Protocol (hover, definition, references) |
| 18 | GitLab Integration | 137 | Issues, merge requests, pipelines, jobs |
| 19 | MCP Integration | 179 | Model Context Protocol (extend via external servers) |

**Key Insight:** Group all integrations together for discoverability.

---

### Part VI: Reference Materials (Sections 20-23)
**Purpose:** Comprehensive lookup documentation  
**User:** Developers, operators needing specific information  
**Type:** All are reference/lookup oriented, not narrative

| # | Section | Lines | Purpose |
|---|---------|-------|---------|
| 20 | Tool Reference | 147+ | 147+ tools organized by category |
| 21 | Office, LibreOffice, and PDF Tools | 530 | Office document read/write operations |
| 22 | CLI Command Reference | 127 | All CLI commands with usage |
| 23 | Testing & CI/CD | 127 | Test organization, benchmarks, CI workflow |

**Key Insight:** These are all "when you need to look something up" materials, not narrative flow.

---

### Part VII: Security & Operations (Sections 24-25)
**Purpose:** Cross-cutting operational concerns  
**User:** DevOps, security, operators  
**Nature:** Critical for all other sections but logically separate

| # | Section | Lines | Purpose |
|---|---------|-------|---------|
| 24 | Security & Permissions | 676 | Permission system, 7-layer bash safety, secret redaction |
| 25 | Auto-Update Mechanism | 84 | Self-updating binary, version management |

**Key Insight:** These affect all other systems but are operational concerns rather than feature documentation.

---

### Appendices
| Letter | Section | Purpose |
|--------|---------|---------|
| A | Version History | Release notes and migration guides |
| B | Documentation | Links to reference materials |
| C | Project Contact | GitHub, license, authors |
| D | Changelog (2025-01-16) | What changed in this revision |

---

## Section Movement Map

### Moves Needed

| From | Current Pos | New Pos | Section | Reason |
|------|-----------|---------|---------|--------|
| No change | 1 | 1 | Overview | Foundation |
| No change | 2 | 2 | Architecture | Foundation |
| No change | 3 | 3 | Core Features | Foundation |
| No change | 4 | 4 | TUI | Foundation |
| No change | 5 | 5 | HTTP Server | Foundation |
| No change | 6 | 6 | Code Index | Part II |
| **MOVE** | 8 | 7 | Memory System | Part II (was mislabeled as 7.x) |
| **MOVE** | 7 | 8 | AIWiki | Part II (reorder for logical flow) |
| **MOVE** | 8 (dup) | 9 | Teams | Part III (fix numbering) |
| No change | 9 | 10 | Swarm | Part III |
| No change | 10 | 11 | Autopilot | Part III |
| **MOVE+MERGE** | 22,23 | 12 | Orchestrator & Event Bus | Part III (group coordination) |
| **MOVE** | 15 | 13 | Custom Agents | Part IV (customization) |
| **MOVE** | 11 | 14 | Skills | Part IV |
| **MOVE** | 12 | 15 | Prompt Optimization | Part IV |
| **MOVE** | 14 | 16 | Configuration | Part IV |
| **MOVE** | 19 | 17 | LSP Integration | Part V |
| **MOVE** | 20 | 18 | GitLab Integration | Part V |
| **MOVE** | 21 | 19 | MCP Integration | Part V |
| **MOVE** | 16 | 20 | Tool Reference | Part VI |
| **MOVE** | 17 | 21 | Office/PDF Tools | Part VI |
| **DELETE/DEPRECATE** | 18 | — | Concurrent File Operations | Marked "Planned", incomplete |
| **MOVE** | 25 | 22 | CLI Reference | Part VI |
| **MOVE** | 26 | 23 | Testing & CI/CD | Part VI |
| **MOVE** | 13 | 24 | Security & Permissions | Part VII |
| **MOVE** | 24 | 25 | Auto-Update | Part VII |

### Numbering Updates Required

| Section | Current Subsections | New Subsections | Issue |
|---------|------------------|-----------------|--------|
| Memory System | 7.1-7.18 | 7.1-7.18 | Correct! (now in position 7) |
| Teams | 8.1-8.11 | 9.1-9.11 | Renumber all |
| Swarm | 9.1-9.x | 10.1-10.x | Renumber all |
| Autopilot | 10.1-10.x | 11.1-11.x | Renumber all |
| Orchestrator | 22.1-22.x, 23.1-23.x | 12.1-12.x | Merge and renumber |
| Custom Agents | 15.1-15.x | 13.1-13.x | Renumber all |
| Skills | 11.1-11.x | 14.1-14.x | Renumber all |
| Prompt Opt | 12.1-12.x | 15.1-15.x | Renumber all |
| Configuration | 14.1-14.x | 16.1-16.x | Renumber all |
| LSP | 19.1-19.x | 17.1-17.x | Renumber all |
| GitLab | 20.1-20.x | 18.1-18.x | Renumber all |
| MCP | 21.1-21.x | 19.1-19.x | Renumber all |
| Tool Ref | 16.1-16.x | 20.1-20.x | Renumber all |
| Office/PDF | 17.1-17.x | 21.1-21.x | Renumber all |
| CLI | 25.1-25.x | 22.1-22.x | Renumber all |
| Testing | 26.1-26.x | 23.1-23.x | Renumber all |
| Security | 13.1-13.x | 24.1-24.x | Renumber all |
| Auto-Update | 24.1-24.x | 25.1-25.x | Renumber all |

---

## Benefits of This Organization

### 1. Fixes Critical Issues ✅
- **Eliminates duplicate section 8** — Removes ambiguity and reference confusion
- **Corrects subsection numbering** — Memory System subsections now correctly labeled 7.1-7.18
- **Enables proper referencing** — Section numbers are now stable and unique

### 2. Improved Navigation ✅
- **Part headers** — Users understand the document structure at a glance
- **Logical progression** — Foundation → Data → Execution → Customization → Integration → Reference
- **Clear relationships** — Related sections are adjacent, not scattered

### 3. Better User Workflows ✅
- **New users** follow sections 1-5 (Foundation)
- **Data-focused developers** find 6-8 together (Data & Knowledge)
- **Team leads** find 9-12 together (Coordination)
- **Customizers** find 13-16 together (Customization)
- **Integrators** find 17-19 together (Integrations)
- **Reference seekers** find 20-23 together (Reference)
- **Operators** find 24-25 together (Security & Ops)

### 4. Improved Discoverability ✅
- **Related features** are now in adjacent sections
- **Dependencies** are clearer (Swarm depends on Teams, Autopilot depends on Swarm, etc.)
- **Cross-cutting concerns** (Security, Orchestrator) are grouped with similar concerns

### 5. Scalability ✅
- **Clear part structure** makes it easy to add new sections
- **Numbered parts** enable subsection headers (e.g., "Part II: Data & Knowledge Systems")
- **Logical grouping** provides template for future reorganizations

---

## Implementation Approach

### Phase 1: Planning ✅ COMPLETE
- [x] Identify current issues
- [x] Design new structure
- [x] Map all movements
- [x] Document dependencies

### Phase 2: Extraction
- [ ] Extract each section to individual files
- [ ] Preserve all content exactly
- [ ] Note all cross-references

### Phase 3: Reorganization
- [ ] Rebuild SPEC.md in new order
- [ ] Update all section numbers (## 1, ## 2, etc.)
- [ ] Update all subsection numbers (### 1.1, ### 1.2, etc.)
- [ ] Rebuild Table of Contents

### Phase 4: Cross-Reference Updates
- [ ] Find all internal cross-references (e.g., "See section 8...")
- [ ] Update all references to new numbers
- [ ] Verify all links point correctly

### Phase 5: Verification
- [ ] Check all subsection numbering is consistent
- [ ] Verify no duplicate section numbers
- [ ] Test all internal cross-references
- [ ] Review for readability

### Phase 6: Enhancement
- [ ] Add part headers and descriptions
- [ ] Add section dependency notes where relevant
- [ ] Update Executive Summary to reflect new flow
- [ ] Update Table of Contents formatting

### Phase 7: Documentation
- [ ] Document all changes made
- [ ] Update CHANGELOG
- [ ] Create migration guide (old section # → new section #)

---

## Deprecation Note

### Section to Remove: "Concurrent File Operations (Planned)"
**Current:** Section 18  
**New:** Deprecated (remove or move to appendix)  
**Reason:**
- Marked "Planned" — not yet implemented
- Seems outdated (section has minimal content)
- Not referenced in other sections
- Would clutter Part VI reference materials

**Options:**
1. Remove entirely
2. Move to appendix with deprecation notice
3. Keep in appendix under "Future Features"

**Recommendation:** Remove entirely (file ops are already comprehensively documented in Core Features)

---

## Documentation of Changes

This restructuring should be documented with:

1. **Migration Guide** — Old section # → New section #
2. **Change Log Entry** — Major structural reorganization
3. **Section Dependency Graph** — Visual showing relationships
4. **Table of Contents Index** — Quick lookup by topic

---

## Estimated Effort

| Phase | Task | Effort | Notes |
|-------|------|--------|-------|
| Extraction | Extract 25 sections to files | 30 min | Mostly automated |
| Reorganization | Rebuild in new order | 1 hour | Careful ordering |
| Numbering | Update all section/subsection numbers | 2 hours | Must be precise |
| Cross-References | Find and update all references | 2 hours | Find → verify → update |
| Verification | Check for errors, consistency | 1 hour | Thorough review |
| Enhancement | Add headers, notes, TOC | 1 hour | Polish |
| Documentation | Update CHANGELOG, migration guide | 30 min | Clear communication |
| **Total** | | **~7.5 hours** | With careful, methodical approach |

---

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-----------|--------|-----------|
| Cross-reference errors | Medium | High | Automated grep to find all refs; manual verification |
| Subsection numbering mistakes | Medium | High | Validation script; careful review |
| Content accidentally deleted | Low | Critical | Keep backups; use git commits |
| Readers can't find old content | Low | Medium | Provide migration guide; TOC clearly shows flow |

---

## Success Criteria

✅ All sections are uniquely numbered (no duplicates)  
✅ All subsections are correctly numbered  
✅ Related sections are adjacent (integrations together, reference together, etc.)  
✅ All internal cross-references work correctly  
✅ New Table of Contents reflects structure with part groupings  
✅ No content is lost or corrupted  
✅ Document is more navigable and logical  
✅ Users can easily find related information  

---

## Recommendation

**Status:** READY FOR IMPLEMENTATION

This reorganization is important for document quality and user experience. The current state with duplicate section numbers and scattered related content undermines the specification's authority and clarity.

**Next Steps:**
1. Approve this plan
2. Proceed with Phase 2: Extraction
3. Create git branch for changes
4. Execute reorganization systematically
5. Extensive testing before merge
6. Announce changes to users via CHANGELOG

---

**Document Created:** 2025-01-16  
**Last Updated:** 2025-01-16  
**Status:** Ready for Review
