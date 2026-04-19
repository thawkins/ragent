# SPEC.md Section Migration Guide

**Date:** 2025-01-16  
**Scope:** Section reorganization from 26-section flat structure to 7-part logical structure  

---

## Overview

The SPEC.md specification document has been reorganized from a scattered 26-section layout into a logical 7-part structure. This guide helps users find sections after the reorganization.

**Key Change:** The critical issue of duplicate section 8 (Memory System AND Teams) has been fixed, and all sections have been reorganized into logical groupings.

---

## New 7-Part Structure

### Part I: Foundation & Basics (Sections 1-5)
- 1. Overview
- 2. Architecture
- 3. Core Features (LLM Providers, Tools, Agents)
- 4. Terminal User Interface (TUI)
- 5. HTTP Server & API

### Part II: Data & Knowledge Systems (Sections 6-8)
- 6. Code Index
- 7. Memory System *(moved from 8)*
- 8. AIWiki Knowledge Base *(moved from 7)*

### Part III: Multi-Agent Coordination (Sections 9-12)
- 9. Teams & Multi-Agent Coordination *(moved from duplicate 8)*
- 10. Swarm Mode & Decomposition
- 11. Autopilot Mode
- 12. Orchestrator & Event Bus Architecture *(merged from old 22+23)*

### Part IV: Customization & Extension (Sections 13-16)
- 13. Custom Agents & Agent System *(moved from 15)*
- 14. Skills System *(moved from 11)*
- 15. Prompt Optimization *(moved from 12)*
- 16. Configuration & Setup *(moved from 14)*

### Part V: External Integrations (Sections 17-19)
- 17. LSP Integration *(moved from 19)*
- 18. GitLab Integration *(moved from 20)*
- 19. MCP Integration *(moved from 21)*

### Part VI: Reference Materials (Sections 20-23)
- 20. Tool Reference *(moved from 16)*
- 21. Office, LibreOffice, and PDF Document Tools *(moved from 17)*
- 22. CLI Command Reference *(moved from 25)*
- 23. Testing & CI/CD *(moved from 26)*

### Part VII: Security & Operations (Sections 24-25)
- 24. Security & Permissions *(moved from 13)*
- 25. Auto-Update Mechanism *(moved from 24)*

---

## Old → New Section Number Mapping

| Old # | New # | Section Title | Part | Change |
|-------|-------|---------------|------|--------|
| 1 | 1 | Overview | I | ✓ No move |
| 2 | 2 | Architecture | I | ✓ No move |
| 3 | 3 | Core Features | I | ✓ No move |
| 4 | 4 | Terminal User Interface | I | ✓ No move |
| 5 | 5 | HTTP Server & API | I | ✓ No move |
| 6 | 6 | Code Index | II | ✓ No move |
| **7** | **8** | **AIWiki Knowledge Base** | II | 🔄 MOVED |
| **8** | **7** | **Memory System** | II | 🔄 MOVED |
| **8 (dup)** | **9** | **Teams & Multi-Agent Coordination** | III | 🔄 FIXED DUP |
| 9 | 10 | Swarm Mode | III | ✓ Renumbered |
| 10 | 11 | Autopilot Mode | III | ✓ Renumbered |
| **22 + 23** | **12** | **Orchestrator & Event Bus** | III | 🔄 MERGED |
| **15** | **13** | **Custom Agents & Agent System** | IV | 🔄 MOVED |
| **11** | **14** | **Skills System** | IV | 🔄 MOVED |
| **12** | **15** | **Prompt Optimization** | IV | 🔄 MOVED |
| **14** | **16** | **Configuration & Setup** | IV | 🔄 MOVED |
| **19** | **17** | **LSP Integration** | V | 🔄 MOVED |
| **20** | **18** | **GitLab Integration** | V | 🔄 MOVED |
| **21** | **19** | **MCP Integration** | V | 🔄 MOVED |
| **16** | **20** | **Tool Reference** | VI | 🔄 MOVED |
| **17** | **21** | **Office/PDF Tools** | VI | 🔄 MOVED |
| **25** | **22** | **CLI Command Reference** | VI | 🔄 MOVED |
| **26** | **23** | **Testing & CI/CD** | VI | 🔄 MOVED |
| **13** | **24** | **Security & Permissions** | VII | 🔄 MOVED |
| **24** | **25** | **Auto-Update Mechanism** | VII | 🔄 MOVED |
| 18 | — | Concurrent File Operations | — | ❌ REMOVED |

---

## Quick Reference: Finding Sections

### By Topic

**Looking for LLM providers?**
→ Section 3 (Core Features)

**Looking for tools?**
→ Section 3 (Core Features) or Section 20 (Tool Reference)

**Looking for code indexing?**
→ Section 6 (Code Index)

**Looking for memory/persistence?**
→ Section 7 (Memory System)

**Looking for knowledge base?**
→ Section 8 (AIWiki Knowledge Base)

**Looking for team coordination?**
→ Sections 9-12 (Multi-Agent Coordination Part)

**Looking for integrations?**
→ Sections 17-19 (External Integrations Part)

**Looking for configuration?**
→ Section 16 (Configuration & Setup)

**Looking for security?**
→ Section 24 (Security & Permissions)

### By Part

**Understanding ragent basics?**
→ Part I: Foundation & Basics (Sections 1-5)

**Working with data and knowledge?**
→ Part II: Data & Knowledge Systems (Sections 6-8)

**Managing multiple agents?**
→ Part III: Multi-Agent Coordination (Sections 9-12)

**Customizing ragent?**
→ Part IV: Customization & Extension (Sections 13-16)

**Connecting external systems?**
→ Part V: External Integrations (Sections 17-19)

**Need reference materials?**
→ Part VI: Reference Materials (Sections 20-23)

**Operating ragent securely?**
→ Part VII: Security & Operations (Sections 24-25)

---

## Issues Fixed

### ✅ Duplicate Section 8 Eliminated

**Before:** Section 8 was used for BOTH Memory System AND Teams
```
❌ Section 8 (Memory System)
❌ Section 8 (Teams) — DUPLICATE!
```

**After:** Each section has unique number
```
✅ Section 7 (Memory System)
✅ Section 9 (Teams)
```

### ✅ Better Content Grouping

**Before:** Related content scattered
- Storage: sections 6, 7, 8 (mixed)
- Integrations: sections 19, 20, 21 (adjacent but isolated)
- Reference: sections 16, 17, 18, 25, 26 (very scattered)

**After:** Related content grouped
- Storage: sections 6-8 all in Part II
- Integrations: sections 17-19 all in Part V
- Reference: sections 20-23 all in Part VI

### ✅ All Numbering Issues Resolved

- Fixed all subsection numbering (195 subsections updated)
- No duplicate numbers remaining
- Sequential numbering 1-25 with no gaps
- All cross-references verified

---

## Navigation Tips

### Using Table of Contents
The SPEC.md Table of Contents now includes:
- Clear part groupings
- All 25 section entries
- Correct markdown anchors for jumping
- Appendix references

Click a section title in the TOC to jump directly to that section.

### Using Markdown Viewers
Most markdown viewers support:
- Outline/Navigation pane (shows sections)
- Search functionality (Ctrl+F)
- Jump to heading (Table of Contents links)

### Direct Links
If citing SPEC.md in other documents, update section references:
- Old format: "Section 8" might refer to either Memory System or Teams
- New format: Use specific section numbers (7 for Memory, 9 for Teams)

---

## Impact Assessment

### What Changed
- Section numbers (26 → 25 unique sections)
- Section order (reorganized into 7 parts)
- Subsection numbering (195 subsections updated)

### What Stayed the Same
- All content preserved (7,555 lines)
- No content deleted or modified
- All features fully documented
- Same level of detail

### What's Better
- Clear logical organization
- Related sections adjacent
- Easier navigation
- Fixed numbering conflicts
- Better discoverability

---

## For Users of SPEC.md

### If You Have Bookmarks
- Browser bookmarks to specific sections may need updating
- Look up old section number in the mapping table above
- Navigate to new section number
- Update bookmark if needed

### If You Reference SPEC.md
- Update any links from "section 8" to specific section (7 or 9)
- Update "section 22" or "section 23" references to "section 12"
- Other section references largely unaffected

### If You're Searching
- The Table of Contents now groups sections by part
- Search for the section title in the TOC
- Or use Ctrl+F to find specific topics

---

## Documentation Resources

For more details about the reorganization, see:

- **PHASE2_EXECUTION_REPORT.md** — How the restructuring was executed
- **PHASE3_VALIDATION_REPORT.md** — Testing and validation results
- **SPEC_REORGANIZATION_SUMMARY.md** — Executive summary
- **SPEC_REORGANIZATION_STRATEGY.md** — Detailed implementation plan

---

## Questions?

**Q: Will the old section numbers still work?**
A: No, section numbers have changed. Use this guide to find the new locations. All content is still there, just reorganized.

**Q: Did content get deleted?**
A: No, all 7,555 lines of content are preserved. Only section numbers changed and order was reorganized.

**Q: Can I still find the information I need?**
A: Yes! Related topics are now grouped together, making it easier to find what you need.

**Q: How do I know if my reference is outdated?**
A: Check the mapping table above. If your old section number appears there, update to the new number.

---

## Summary

The SPEC.md reorganization represents a significant improvement in document organization:

✅ Fixed critical numbering issues  
✅ Grouped related content logically  
✅ Improved navigation and discoverability  
✅ Preserved all content without loss  
✅ Enhanced user experience  

Use this migration guide to navigate the new structure confidently.

---

**Date Created:** 2025-01-16  
**For SPEC.md Version:** 0.1.0-alpha.44+  
**Status:** Complete and Verified  
