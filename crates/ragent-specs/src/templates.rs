//! Default templates for SPEC.md, PLAN.md, CONSTITUTION.md, and FEEDBACK.md generation.

use crate::spec::SpecId;

/// Default template for a new SPEC.md file.
pub struct SpecTemplate;

impl SpecTemplate {
    /// Generate a SPEC.md from the EARS template with the given id and title.
    #[must_use]
    pub fn generate(id: &SpecId, title: &str) -> String {
        Self::generate_with_checklist(id, title, &[], false)
    }

    /// Generate a SPEC.md with a pre-populated `## Related Research` section
    /// listing the supplied research items (T-041). When `research_names` is
    /// empty, the `## Related Research` section is omitted entirely so the
    /// template matches the un-researched default.
    #[must_use]
    pub fn generate_with_research(id: &SpecId, title: &str, research_names: &[String]) -> String {
        Self::generate_with_checklist(id, title, research_names, false)
    }

    /// Generate a SPEC.md with an optional `## Quality Checklist` section
    /// (FR-006) and optional `## Related Research` section (T-041).
    ///
    /// When `include_checklist` is `false` the checklist section is omitted
    /// entirely, preserving the existing template output. When `true`, a
    /// self-review checklist covering requirement completeness, testability,
    /// and absence of speculative features is embedded.
    #[must_use]
    pub fn generate_with_checklist(
        id: &SpecId,
        title: &str,
        research_names: &[String],
        include_checklist: bool,
    ) -> String {
        let related_section = if research_names.is_empty() {
            String::new()
        } else {
            let mut s = String::from("\n## Related Research\n\n");
            s.push_str("This spec was informed by the following research items:\n\n");
            for name in research_names {
                s.push_str(&format!(
                    "- [`{name}`](../research/{name}/RESEARCH.md) — see the captured references for context.\n",
                ));
            }
            s.push('\n');
            s
        };
        let checklist_section = if include_checklist {
            "\n## Quality Checklist\n\nSelf-review before transitioning to `approved`:\n\n- [ ] Every functional and non-functional requirement uses valid EARS notation.\n- [ ] Each requirement is independently testable with a clear pass/fail criterion.\n- [ ] All features requested by the user are captured — no missing requirements.\n- [ ] No speculative features or gold-plating are included beyond the stated scope.\n- [ ] All `[NEEDS CLARIFICATION]` markers have been resolved or removed.\n\n"
                .to_string()
        } else {
            String::new()
        };

        let frontmatter = if research_names.is_empty() {
            format!("status: draft\nid: {}\n", id.as_str())
        } else {
            let yaml = research_names
                .iter()
                .map(|n| format!("\"{n}\""))
                .collect::<Vec<_>>()
                .join(", ");
            format!("status: draft\nid: {}\nresearch: [{}]\n", id.as_str(), yaml)
        };
        format!(
            r"---
{frontmatter}
---

# Specification: {title}

## Executive Summary

[Provide a brief summary of the feature or system being specified.]

## Scope & Objectives

### Scope

[Define what is in scope and what is out of scope.]

### Objectives

1. [Objective 1]
2. [Objective 2]
3. [Objective 3]

---

## Functional Requirements

### FR-001 — [Requirement Title]

`The <SYSTEM NAME> shall <SYSTEM RESPONSE>.`

[Additional context, if needed.]

### FR-002 — [Requirement Title]

`When <TRIGGER>, the <SYSTEM NAME> shall <SYSTEM RESPONSE>.`

### FR-003 — [Requirement Title]

`While <PRECONDITION>, the <SYSTEM NAME> shall <SYSTEM RESPONSE>.`

### FR-004 — [Requirement Title]

`Where <FEATURE> is included, the <SYSTEM NAME> shall <SYSTEM RESPONSE>.`

### FR-005 — [Requirement Title]

`If <TRIGGER>, the <SYSTEM NAME> shall <SYSTEM RESPONSE>.`

---

## Non-Functional Requirements

### NFR-001 — [Requirement Title]

`The <SYSTEM NAME> shall <SYSTEM RESPONSE>.`

---

## Constraints & Assumptions

### Constraints

1. [Constraint 1]
2. [Constraint 2]

### Assumptions

1. [Assumption 1]
2. [Assumption 2]

---

## Interfaces & Dependencies

### Internal Interfaces

| Component | Interface | Purpose |
|-----------|-----------|---------|
| [Component] | [Interface] | [Purpose] |

### External Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| [crate] | [version] | [Purpose] |

---
{related_section}{checklist_section}## Glossary
| Term | Definition |
|------|------------|
| **[Term]** | [Definition] |

---

*End of Specification*
",
        )
    }
}

/// Default template for a new PLAN.md file.
pub struct PlanTemplate;

impl PlanTemplate {
    /// Generate a PLAN.md from the standard implementation plan template.
    #[must_use]
    pub fn generate(id: &SpecId, title: &str) -> String {
        Self::generate_with_checklist(id, title, false)
    }

    /// Generate a PLAN.md with an optional `## Quality Checklist` section
    /// (FR-006) and a `## File Creation Order` section (FR-014).
    ///
    /// When `include_checklist` is `false` the checklist section is omitted
    /// entirely, preserving the existing template output. When `true`, a
    /// self-review checklist covering requirement traceability, testability,
    /// and absence of speculative tasks is embedded.
    ///
    /// The `## File Creation Order` section is always included — it documents
    /// the test-first ordering (contracts → tests → source) that the
    /// implementation runner checks advisory (FR-014).
    #[must_use]
    pub fn generate_with_checklist(id: &SpecId, title: &str, include_checklist: bool) -> String {
        let checklist_section = if include_checklist {
            "\n## Quality Checklist\n\nSelf-review before transitioning to `approved`:\n\n- [ ] Every task references at least one requirement (FR/NFR) from the SPEC.md.\n- [ ] Each task has a clear acceptance criterion or test that verifies completion.\n- [ ] All requirements from the SPEC.md are covered by at least one task.\n- [ ] No speculative tasks or gold-plating are included beyond the stated scope.\n- [ ] Task dependencies are acyclic and all referenced task IDs exist.\n\n"
                .to_string()
        } else {
            String::new()
        };
        format!(
            r"---
spec_id: {id}
---

# Implementation Plan: {title}

## Overview

[Summarise the implementation approach.]

---

## Milestones

### Milestone 1: [Name]
**Deliverable:** [What is delivered.]

- [Task 1]
- [Task 2]

### Milestone 2: [Name]
**Deliverable:** [What is delivered.]

- [Task 1]
- [Task 2]

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | [Title] | FR-001 | S | Critical | Pending | — |
---

## File Creation Order

Tasks should create files in the following order to maintain test-first
discipline. The implementation runner emits an advisory warning if tasks
violate this ordering (FR-014).

1. **Contracts** — API specs, schema definitions, and interface descriptions
   in `contracts/` before any tests or source files.
2. **Contract tests** — tests that validate contract conformance.
3. **Integration tests** — tests that verify cross-component interactions.
4. **End-to-end (e2e) tests** — tests that validate full user-facing flows.
5. **Unit tests** — tests for individual functions and modules.
6. **Source files** — implementation code that satisfies the tests above.

---

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| [Risk] | [High/Medium/Low] | [High/Medium/Low] | [Mitigation] |

---
{checklist_section}
## Definition of Done

1. [Criterion 1]
2. [Criterion 2]
3. [Criterion 3]

---

*End of Implementation Plan*
",
            id = id.as_str(),
            title = title,
            checklist_section = checklist_section,
        )
    }
}

/// Default template for a new FEEDBACK.md file (FR-017, T-031).
///
/// Production feedback notes inform the next plan regeneration.
pub struct FeedbackTemplate;

impl FeedbackTemplate {
    /// Generate a FEEDBACK.md template for the given spec title.
    #[must_use]
    pub fn generate(title: &str) -> String {
        format!(
            r"# Feedback: {title}

Production metrics, incident reports, and user feedback that should
inform the next plan regeneration.

## Feedback Notes

| Date | Source | Note |
|------|--------|------|
| [YYYY-MM-DD] | [metric/incident/user] | [Feedback note] |

---

*Notes in this file are advisory — they are surfaced during `/spec plan`
regeneration but do not block validation or status transitions.*
",
            title = title,
        )
    }
}

/// Default template for a new CONSTITUTION.md file (FR-007, T-014).
///
/// The constitution defines immutable architectural principles that govern
/// generated implementations. This template emits the standard set of
/// default articles in the markdown format that `parse_constitution` can
/// read back.
pub struct ConstitutionTemplate;

impl ConstitutionTemplate {
    /// Generate a CONSTITUTION.md from the default articles (FR-007).
    ///
    /// The output is a markdown document with a `# Constitution` header,
    /// an introductory paragraph, and one `## Article N: <Title>` section
    /// per default article. The result is round-trip parseable by
    /// `parse_constitution`.
    #[must_use]
    pub fn generate() -> String {
        const ARTICLES: &[(&str, &str)] = &[
            (
                "Library-First",
                "Depend on libraries, not frameworks. Avoid vendor lock-in. \
                 Prefer composing from small libraries over building monolithic subsystems.",
            ),
            (
                "Simplicity",
                "Do the simplest thing that works. No speculative abstractions. \
                 If a senior engineer would say it is overcomplicated, simplify.",
            ),
            (
                "Anti-Abstraction",
                "Abstract only when you have three concrete examples. \
                 No premature abstraction for single-use code.",
            ),
            (
                "Integration-First Testing",
                "Test through public interfaces. Test the integration, \
                 not the implementation. Write tests before source files.",
            ),
            (
                "Constitutional Amendment Process",
                "Amendments require a dated changelog entry and version increment. \
                 Existing articles are immutable unless amended through this process.",
            ),
        ];
        let mut body = String::new();
        for (i, &(title, principle)) in ARTICLES.iter().enumerate() {
            let n = i + 1;
            body.push_str(&format!("## Article {n}: {title}\n\n{principle}\n\n"));
        }
        format!(
            "# Constitution\n\n\
             Immutable architectural principles that govern generated implementations.\n\
             These articles constrain how plans and tasks are structured.\n\n\
             {body}\
             ---\n\n\
             *Amendments require a dated changelog entry and version increment.*\n"
        )
    }
}
