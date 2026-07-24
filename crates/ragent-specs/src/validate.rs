//! EARS validation engine for spec management.
//!
//! Provides structural and syntax checking for `SPEC.md` files, including:
//! - Required section detection
//! - Requirement numbering and EARS template matching
//! - PLAN.md completeness checks
//! - Status value validation

use crate::spec::{EarsTemplate, Spec, SpecStatus};
use regex::Regex;
use std::fmt;
use std::sync::LazyLock;

// ── EARS regex patterns ───────────────────────────────────────────────────

static RE_UBIQUITOUS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^the\s+.+\s+shall\s+.+$").expect("ubiquitous regex should compile")
});

static RE_EVENT_DRIVEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^when\s+.+\s*,?\s*the\s+.+\s+shall\s+.+$")
        .expect("event-driven regex should compile")
});

static RE_STATE_DRIVEN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^while\s+.+\s*,?\s*the\s+.+\s+shall\s+.+$")
        .expect("state-driven regex should compile")
});

static RE_OPTIONAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^where\s+.+,?\s*the\s+.+\s+shall\s+.+$")
        .expect("optional regex should compile")
});

static RE_UNWANTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^if\s+.+\s*,?\s*the\s+.+\s+shall\s+.+$")
        .expect("unwanted regex should compile")
});

static RE_SECTION_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(#{2,4})\s+(.+)$").expect("section header regex should compile")
});

static RE_INLINE_REQUIREMENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(FR|NFR)-(\d+)\.\s+(.*)$").expect("inline requirement regex should compile")
});

static RE_REQUIREMENT_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(#{2,3})\s+(FR|NFR)-(\d+)\s*[-–—]\s*(.*)$")
        .expect("requirement header regex should compile")
});

static RE_STATUS_FRONTMATTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^status:\s*(\S+)$").expect("status frontmatter regex should compile")
});

static RE_TASK_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\|\s*(T-\d+)\s*\|").expect("task ID regex should compile"));

/// Severity of a validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Must be fixed; spec is non-compliant.
    Error,
    /// Should be fixed; spec is technically compliant but flawed.
    Warning,
    /// FYI; best-practice suggestion.
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Error => write!(f, "ERROR"),
            Self::Warning => write!(f, "WARNING"),
            Self::Info => write!(f, "INFO"),
        }
    }
}

/// Category of validation issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// Missing or malformed EARS requirement.
    EarsSyntax,
    /// Missing required section.
    MissingSection,
    /// Invalid or unknown status value.
    InvalidStatus,
    /// PLAN.md issue.
    Plan,
    /// Requirement numbering issue.
    Numbering,
    /// Structural/frontmatter issue.
    Structure,
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EarsSyntax => write!(f, "EARS Syntax"),
            Self::MissingSection => write!(f, "Missing Section"),
            Self::InvalidStatus => write!(f, "Invalid Status"),
            Self::Plan => write!(f, "Plan"),
            Self::Numbering => write!(f, "Numbering"),
            Self::Structure => write!(f, "Structure"),
        }
    }
}

/// A single issue found during validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Issue severity.
    pub severity: Severity,
    /// Issue category.
    pub category: Category,
    /// Human-readable message.
    pub message: String,
    /// Line number in the file (1-based, `None` for file-level issues).
    pub line: Option<usize>,
    /// Related requirement or task ID, if any.
    pub id: Option<String>,
}

impl Issue {
    /// Create a new issue.
    pub fn new(severity: Severity, category: Category, message: impl Into<String>) -> Self {
        Self {
            severity,
            category,
            message: message.into(),
            line: None,
            id: None,
        }
    }

    /// Set the line number.
    #[must_use]
    pub const fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    /// Set the related ID.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
}

/// Validation report for a spec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Report {
    /// Issues found (sorted by severity descending then line number).
    pub issues: Vec<Issue>,
    /// Total number of requirements parsed.
    pub requirement_count: usize,
    /// Number of EARS-valid requirements.
    pub valid_ears_count: usize,
    /// Number of requirements with detected templates.
    pub template_counts: std::collections::HashMap<EarsTemplate, usize>,
}

impl Report {
    /// Create an empty report.
    #[must_use]
    pub fn new() -> Self {
        Self {
            issues: Vec::new(),
            requirement_count: 0,
            valid_ears_count: 0,
            template_counts: std::collections::HashMap::new(),
        }
    }

    /// Add an issue to the report.
    pub fn add(&mut self, issue: Issue) {
        self.issues.push(issue);
    }

    /// Returns `true` if the report contains any [`Severity::Error`] issues.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    /// Returns `true` if the report contains any [`Severity::Warning`] issues.
    #[must_use]
    pub fn has_warnings(&self) -> bool {
        self.issues.iter().any(|i| i.severity == Severity::Warning)
    }

    /// Count issues by severity.
    #[must_use]
    pub fn count_by_severity(&self, severity: Severity) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == severity)
            .count()
    }

    /// Sort issues by severity (errors first) then by line number.
    pub fn sort(&mut self) {
        self.issues.sort_by(|a, b| {
            let sev_order = |s: Severity| match s {
                Severity::Error => 0,
                Severity::Warning => 1,
                Severity::Info => 2,
            };
            let sev_cmp = sev_order(a.severity).cmp(&sev_order(b.severity));
            if sev_cmp != std::cmp::Ordering::Equal {
                return sev_cmp;
            }
            match (a.line, b.line) {
                (Some(al), Some(bl)) => al.cmp(&bl),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
    }

    /// Format the report as a human-readable multi-line string.
    #[must_use]
    pub fn format(&self, spec_id: &str) -> String {
        let mut lines = vec![format!("Validation Report for `{}`", spec_id)];
        lines.push(format!(
            "Requirements: {} total, {} EARS-valid",
            self.requirement_count, self.valid_ears_count
        ));

        for (template, count) in &self.template_counts {
            lines.push(format!("  {}: {}", template.as_str(), count));
        }

        let errors = self.count_by_severity(Severity::Error);
        let warnings = self.count_by_severity(Severity::Warning);
        let infos = self.count_by_severity(Severity::Info);
        lines.push(format!(
            "Issues: {errors} error(s), {warnings} warning(s), {infos} info(s)"
        ));
        lines.push(String::new());

        for issue in &self.issues {
            let loc = issue
                .line
                .map_or_else(|| "file".to_string(), |l| format!("line {l}"));
            let id = issue
                .id
                .as_ref()
                .map_or_else(String::new, |id| format!(" [{id}]"));
            lines.push(format!(
                "  [{}] {} — {}{} (at {})",
                issue.severity, issue.category, issue.message, id, loc
            ));
        }

        lines.join("\n")
    }
}

impl Default for Report {
    fn default() -> Self {
        Self::new()
    }
}

// ── EARS template detection ─────────────────────────────────────────────────

/// Detect the EARS template type from a requirement text string.
///
/// Returns the matching template, or `None` if no template matches.
pub fn detect_ears_template(text: &str) -> Option<EarsTemplate> {
    let trimmed = text.trim();
    if RE_EVENT_DRIVEN.is_match(trimmed) {
        return Some(EarsTemplate::EventDriven);
    }
    if RE_STATE_DRIVEN.is_match(trimmed) {
        return Some(EarsTemplate::StateDriven);
    }
    if RE_OPTIONAL.is_match(trimmed) {
        return Some(EarsTemplate::Optional);
    }
    if RE_UNWANTED.is_match(trimmed) {
        return Some(EarsTemplate::Unwanted);
    }
    if RE_UBIQUITOUS.is_match(trimmed) {
        return Some(EarsTemplate::Ubiquitous);
    }
    None
}

// ── Requirement parser ────────────────────────────────────────────────────

/// A parsed requirement from a SPEC.md file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRequirement {
    /// Requirement ID, e.g. "FR-001".
    pub id: String,
    /// Requirement title from the header.
    pub title: String,
    /// Line number of the requirement header.
    pub header_line: usize,
    /// The EARS text (inside backticks).
    pub ears_text: String,
    /// Line number of the EARS text.
    pub ears_line: usize,
}

/// Parse requirements from SPEC.md content.
///
/// Extracts FR-### and NFR-### headers and the first backtick-enclosed
/// text block following each header.
pub fn parse_requirements(content: &str) -> Vec<ParsedRequirement> {
    let mut reqs = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        if let Some(caps) = RE_REQUIREMENT_HEADER.captures(lines[i]) {
            let id = format!("{}-{}", &caps[2], &caps[3]);
            let title = caps[4].trim().to_string();
            let header_line = i + 1;

            // Look for the first backtick-enclosed text after this header
            let mut ears_text = String::new();
            let mut ears_line = 0;
            let mut j = i + 1;
            while j < lines.len() {
                let line = lines[j];
                if RE_REQUIREMENT_HEADER.is_match(line) || line.starts_with("## ") {
                    break;
                }
                if line.starts_with('`') && line.ends_with('`') {
                    ears_text = line.trim_matches('`').to_string();
                    ears_line = j + 1;
                    break;
                }
                j += 1;
            }

            reqs.push(ParsedRequirement {
                id,
                title,
                header_line,
                ears_text,
                ears_line,
            });
        } else if let Some(caps) = RE_INLINE_REQUIREMENT.captures(lines[i]) {
            // Alternative format: "FR-###.  The system shall ..." as a plain paragraph.
            let id = format!("{}-{}", &caps[1], &caps[2]);
            let ears_text = caps[3].trim().to_string();
            let title = if ears_text.len() > 50 {
                format!("{ears_text:.47}...")
            } else {
                ears_text.clone()
            };
            reqs.push(ParsedRequirement {
                id,
                title,
                header_line: i + 1,
                ears_text,
                ears_line: i + 1,
            });
        }
        i += 1;
    }

    reqs
}
/// Extract required section headers from a SPEC.md.
pub fn extract_sections(content: &str) -> Vec<(usize, String, usize)> {
    let mut sections = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if let Some(caps) = RE_SECTION_HEADER.captures(line) {
            let level = caps[1].len();
            let name = caps[2].trim().to_string();
            sections.push((level, name, i + 1));
        }
    }
    sections
}

// ── Core validators ────────────────────────────────────────────────────────

/// Validate a spec and return a [`Report`].
///
/// This runs structural, EARS, and PLAN.md validation in sequence.
#[must_use]
pub fn validate(spec: &Spec) -> Report {
    let mut report = Report::new();

    validate_structure(spec, &mut report);
    validate_ears(spec, &mut report);
    validate_plan(spec, &mut report);

    report.sort();
    report
}

/// Validate structural aspects: required sections, frontmatter status.
pub fn validate_structure(spec: &Spec, report: &mut Report) {
    let sections = extract_sections(&spec.spec_md);
    let section_names: Vec<String> = sections.iter().map(|(_, n, _)| n.clone()).collect();

    // Check required top-level sections
    let required = [
        "Executive Summary",
        "Scope & Objectives",
        "Functional Requirements",
        "Non-Functional Requirements",
        "Constraints & Assumptions",
    ];
    for req in &required {
        if !section_names.iter().any(|n| n.eq_ignore_ascii_case(req)) {
            report.add(Issue::new(
                Severity::Error,
                Category::MissingSection,
                format!("Missing required section: {req}"),
            ));
        }
    }

    // Check frontmatter status validity
    for (i, line) in spec.spec_md.lines().enumerate() {
        if line.trim() == "---" {
            // Look for status in the next few lines
            for j in (i + 1)..(i + 10) {
                if let Some(l) = spec.spec_md.lines().nth(j) {
                    if l.trim() == "---" {
                        break;
                    }
                    if let Some(caps) = RE_STATUS_FRONTMATTER.captures(l.trim()) {
                        let status_str = &caps[1];
                        if SpecStatus::parse(status_str).is_none() {
                            report.add(
                                Issue::new(
                                    Severity::Error,
                                    Category::InvalidStatus,
                                    format!("Unknown status value: {status_str}"),
                                )
                                .with_line(i + j + 2),
                            );
                        }
                    }
                }
            }
            break;
        }
    }

    // Check that spec status matches frontmatter status
    if let Some(fm_status) = extract_frontmatter_status(&spec.spec_md) {
        let parsed = SpecStatus::parse(&fm_status);
        if parsed.is_some_and(|s| s != spec.status) {
            report.add(Issue::new(
                Severity::Warning,
                Category::Structure,
                format!(
                    "Frontmatter status '{}' does not match spec.status '{:?}'",
                    fm_status, spec.status
                ),
            ));
        }
    }
}

fn extract_frontmatter_status(content: &str) -> Option<String> {
    if !content.starts_with("---") {
        return None;
    }
    let end = content[3..].find("---")?;
    let frontmatter = &content[3..3 + end];
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("status:") {
            return Some(val.trim().to_string());
        }
    }
    None
}

/// Validate EARS syntax for all requirements.
pub fn validate_ears(spec: &Spec, report: &mut Report) {
    let reqs = parse_requirements(&spec.spec_md);
    report.requirement_count = reqs.len();

    if reqs.is_empty() {
        report.add(Issue::new(
            Severity::Error,
            Category::EarsSyntax,
            "No requirements found (expected FR-### or NFR-### headers)",
        ));
        return;
    }

    // Check for duplicate requirement IDs
    let mut seen_ids: Vec<String> = Vec::new();
    for req in &reqs {
        if seen_ids.contains(&req.id) {
            report.add(
                Issue::new(
                    Severity::Error,
                    Category::Numbering,
                    format!("Duplicate requirement ID: {}", req.id),
                )
                .with_line(req.header_line)
                .with_id(&req.id),
            );
        }
        seen_ids.push(req.id.clone());
    }

    // Check numbering gaps
    let mut fr_numbers: Vec<u32> = Vec::new();
    let mut nfr_numbers: Vec<u32> = Vec::new();
    let re = Regex::new(r"^(\D+)-(\d+)$").unwrap();
    for req in &reqs {
        if let Some(caps) = re.captures(&req.id) {
            let prefix = &caps[1];
            let num: u32 = caps[2].parse().unwrap_or(0);
            match prefix {
                "FR" => fr_numbers.push(num),
                "NFR" => nfr_numbers.push(num),
                _ => {}
            }
        }
    }
    fr_numbers.sort_unstable();
    nfr_numbers.sort_unstable();
    check_numbering_gaps(&fr_numbers, "FR", report);
    check_numbering_gaps(&nfr_numbers, "NFR", report);

    // Validate each requirement's EARS syntax
    for req in &reqs {
        if req.ears_text.is_empty() {
            report.add(
                Issue::new(
                    Severity::Error,
                    Category::EarsSyntax,
                    format!(
                        "Requirement {} has no EARS text (expected backtick-enclosed sentence)",
                        req.id
                    ),
                )
                .with_line(req.header_line)
                .with_id(&req.id),
            );
            continue;
        }

        if let Some(template) = detect_ears_template(&req.ears_text) {
            report.valid_ears_count += 1;
            *report.template_counts.entry(template).or_insert(0) += 1;
        } else {
            report.add(
                Issue::new(
                    Severity::Error,
                    Category::EarsSyntax,
                    format!("Requirement {} does not match any EARS template", req.id),
                )
                .with_line(req.ears_line)
                .with_id(&req.id),
            );
        }
    }
}

fn check_numbering_gaps(numbers: &[u32], prefix: &str, report: &mut Report) {
    if numbers.len() < 2 {
        return;
    }
    for window in numbers.windows(2) {
        let gap = window[1] - window[0];
        if gap > 1 {
            for missing in (window[0] + 1)..window[1] {
                report.add(Issue::new(
                    Severity::Warning,
                    Category::Numbering,
                    format!("Gap in numbering: {prefix}-{missing} is missing"),
                ));
            }
        }
    }
}

/// Validate PLAN.md completeness: existence, task IDs, requirement linkage.
pub fn validate_plan(spec: &Spec, report: &mut Report) {
    if spec.plan_md.is_empty() {
        report.add(Issue::new(
            Severity::Error,
            Category::Plan,
            "PLAN.md is empty or missing",
        ));
        return;
    }

    // Check for required PLAN.md sections
    let plan_sections = extract_sections(&spec.plan_md);
    let section_names: Vec<String> = plan_sections.iter().map(|(_, n, _)| n.clone()).collect();

    let plan_required = ["Overview", "Milestones", "Tasks"];
    for req in &plan_required {
        if !section_names.iter().any(|n| n.eq_ignore_ascii_case(req)) {
            report.add(Issue::new(
                Severity::Warning,
                Category::Plan,
                format!("PLAN.md missing section: {req}"),
            ));
        }
    }

    // Extract task IDs from PLAN.md (looking for table rows with T-###)
    let mut task_ids: Vec<String> = Vec::new();
    for line in spec.plan_md.lines() {
        if let Some(caps) = RE_TASK_ID.captures(line.trim()) {
            task_ids.push(caps[1].to_string());
        }
    }

    // Extract requirement IDs from SPEC.md
    let reqs = parse_requirements(&spec.spec_md);
    let req_ids: Vec<String> = reqs.iter().map(|r| r.id.clone()).collect();

    // Check that each task references a valid requirement
    let re = Regex::new(r"(FR-\d+|NFR-\d+)").unwrap();
    for line in spec.plan_md.lines() {
        for cap in re.find_iter(line) {
            let ref_id = cap.as_str();
            if !req_ids.iter().any(|id| id.eq_ignore_ascii_case(ref_id)) {
                report.add(Issue::new(
                    Severity::Warning,
                    Category::Plan,
                    format!("PLAN.md references unknown requirement {ref_id}"),
                ));
            }
        }
    }

    // Check for duplicate task IDs
    let mut seen_tasks: Vec<String> = Vec::new();
    for task_id in &task_ids {
        if seen_tasks.contains(task_id) {
            report.add(Issue::new(
                Severity::Warning,
                Category::Plan,
                format!("Duplicate task ID: {task_id}"),
            ));
        }
        seen_tasks.push(task_id.clone());
    }
}

// ── Convenience API ────────────────────────────────────────────────────────

// NOTE: Async filesystem validation is a Milestone-4 integration task.
// The `validate` function above is sufficient for the validation engine.

#[cfg(test)]
#[path = "../tests/inline/validate.rs"]
mod tests_tests;
