//! Default templates for SPEC.md and PLAN.md generation.

use crate::spec::SpecId;

/// Default template for a new SPEC.md file.
pub struct SpecTemplate;

impl SpecTemplate {
    /// Generate a SPEC.md from the EARS template with the given id and title.
    #[must_use]
    pub fn generate(id: &SpecId, title: &str) -> String {
        Self::generate_with_research(id, title, &[])
    }

    /// Generate a SPEC.md with a pre-populated `## Related Research` section
    /// listing the supplied research items (T-041). When `research_names` is
    /// empty, the `## Related Research` section is omitted entirely so the
    /// template matches the un-researched default.
    #[must_use]
    pub fn generate_with_research(id: &SpecId, title: &str, research_names: &[String]) -> String {
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
{related_section}
## Glossary
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

| ID | Title | Requirement | Effort | Priority | Dependencies |
|----|-------|-------------|--------|----------|--------------|
      | T-001 | [Title] | FR-001 | S | Critical | pending | — |
---

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| [Risk] | [High/Medium/Low] | [High/Medium/Low] | [Mitigation] |

---

## Definition of Done

1. [Criterion 1]
2. [Criterion 2]
3. [Criterion 3]

---

*End of Implementation Plan*
",
            id = id.as_str(),
            title = title
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_template_contains_sections() {
        let id = SpecId::new("test").unwrap();
        let md = SpecTemplate::generate(&id, "Test Title");
        assert!(md.contains("# Specification: Test Title"));
        assert!(md.contains("## Executive Summary"));
        assert!(md.contains("## Functional Requirements"));
        assert!(md.contains("## Non-Functional Requirements"));
        assert!(md.contains("## Constraints & Assumptions"));
        assert!(md.contains("## Interfaces & Dependencies"));
        assert!(md.contains("## Glossary"));
        assert!(md.contains("status: draft"));
        assert!(md.contains("id: test"));
    }

    #[test]
    fn test_spec_template_with_research_emits_related_section() {
        let id = SpecId::new("test").unwrap();
        let md = SpecTemplate::generate_with_research(
            &id,
            "Test Title",
            &["rust-async".to_string(), "tokio-runtime".to_string()],
        );
        assert!(md.contains("## Related Research"));
        assert!(md.contains("`rust-async`"));
        assert!(md.contains("../research/rust-async/RESEARCH.md"));
        assert!(md.contains("`tokio-runtime`"));
        assert!(md.contains("research: [\"rust-async\", \"tokio-runtime\"]"));
    }

    #[test]
    fn test_spec_template_without_research_omits_related_section() {
        let id = SpecId::new("test").unwrap();
        let md = SpecTemplate::generate_with_research(&id, "Test Title", &[]);
        assert!(!md.contains("## Related Research"));
        assert!(!md.contains("research: ["));
    }

    #[test]
    fn test_spec_template_has_ears_examples() {
        let id = SpecId::new("test").unwrap();
        let md = SpecTemplate::generate(&id, "Test");
        assert!(md.contains("The <SYSTEM NAME> shall <SYSTEM RESPONSE>"));
        assert!(md.contains("When <TRIGGER>, the <SYSTEM NAME> shall"));
        assert!(md.contains("While <PRECONDITION>, the <SYSTEM NAME> shall"));
        assert!(md.contains("Where <FEATURE> is included, the <SYSTEM NAME> shall"));
        assert!(md.contains("If <TRIGGER>, the <SYSTEM NAME> shall"));
    }

    #[test]
    fn test_plan_template_contains_sections() {
        let id = SpecId::new("test").unwrap();
        let md = PlanTemplate::generate(&id, "Test Plan");
        assert!(md.contains("# Implementation Plan: Test Plan"));
        assert!(md.contains("## Overview"));
        assert!(md.contains("## Milestones"));
        assert!(md.contains("## Tasks"));
        assert!(md.contains("## Risks & Mitigations"));
        assert!(md.contains("## Definition of Done"));
        assert!(md.contains("spec_id: test"));
    }

    #[test]
    fn test_plan_template_has_task_table() {
        let id = SpecId::new("test").unwrap();
        let md = PlanTemplate::generate(&id, "Test Plan");
        assert!(md.contains("| ID | Title | Requirement | Effort | Priority | Dependencies |"));
        assert!(md.contains("| T-001 |"));
    }
}
