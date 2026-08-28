//! EARS validation engine for spec management.
//!
//! Provides structural and syntax checking for `SPEC.md` files, including:
//! - Required section detection
//! - Requirement numbering and EARS template matching
//! - PLAN.md completeness checks
//! - Status value validation
//!
//! SDD capability gating (FR-019) is handled by [`SddFlags`] and
//! [`validate_with_flags`]. When a flag is `false`, the corresponding
//! SDD-specific check is skipped so existing workflows are not disrupted.

use crate::plan_parser::PlanParser;
use crate::spec::{EarsTemplate, Spec, SpecStatus};
use regex::Regex;
use std::fmt;
use std::sync::LazyLock;

// ── SDD capability flags (FR-019) ──────────────────────────────────────────

/// Capability flags that gate SDD-specific validation checks and artifact
/// generation (FR-019).
///
/// This struct mirrors the relevant boolean fields of
/// `ragent_config::SddConfig` but lives in `ragent-specs` so the validation
/// engine can remain free of a `ragent-config` dependency. The TUI constructs
/// `SddFlags` from the loaded `SddConfig` at the call site.
///
/// All flags default to `false` (opt-in). Use [`SddFlags::all_enabled`] to
/// preserve the pre-gating behaviour of running every check.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SddFlags {
    /// Enable `[NEEDS CLARIFICATION]` marker detection and reporting (FR-002).
    pub clarification_markers: bool,
    /// Embed quality checklists in spec and plan templates (FR-006).
    pub quality_checklists: bool,
    /// Run ambiguity, contradiction, and gap consistency checks (FR-015).
    pub consistency_checks: bool,
    /// Enable Phase -1 pre-implementation gate validation (FR-008).
    pub phase_minus_one_gates: bool,
    /// Generate and parse `CONSTITUTION.md` artifact (FR-007).
    pub constitution: bool,
    /// Enable production feedback loop / `FEEDBACK.md` surfacing (FR-017).
    pub feedback_loop: bool,
}

impl SddFlags {
    /// Returns flags with every SDD capability enabled.
    ///
    /// Use this to preserve the pre-gating behaviour where all checks run
    /// unconditionally (backward compatibility for [`validate`]).
    #[must_use]
    pub const fn all_enabled() -> Self {
        Self {
            clarification_markers: true,
            quality_checklists: true,
            consistency_checks: true,
            phase_minus_one_gates: true,
            constitution: true,
            feedback_loop: true,
        }
    }

    /// Returns flags with every SDD capability disabled (the default opt-in
    /// state). No SDD-specific checks or artifacts are produced.
    #[must_use]
    pub const fn all_disabled() -> Self {
        Self {
            clarification_markers: false,
            quality_checklists: false,
            consistency_checks: false,
            phase_minus_one_gates: false,
            constitution: false,
            feedback_loop: false,
        }
    }

    /// Construct flags from individual boolean values, matching the field
    /// order of `ragent_config::SddConfig`.
    ///
    /// This convenience constructor lets the TUI build `SddFlags` from a
    /// loaded `SddConfig` without a direct type dependency:
    ///
    /// ```ignore
    /// let flags = SddFlags::from_bools(
    ///     cfg.clarification_markers,
    ///     cfg.quality_checklists,
    ///     cfg.consistency_checks,
    ///     cfg.phase_minus_one_gates,
    ///     cfg.constitution,
    ///     cfg.feedback_loop,
    /// );
    /// ```
    #[must_use]
    #[allow(clippy::fn_params_excessive_bools)]
    pub const fn from_bools(
        clarification_markers: bool,
        quality_checklists: bool,
        consistency_checks: bool,
        phase_minus_one_gates: bool,
        constitution: bool,
        feedback_loop: bool,
    ) -> Self {
        Self {
            clarification_markers,
            quality_checklists,
            consistency_checks,
            phase_minus_one_gates,
            constitution,
            feedback_loop,
        }
    }
}

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

static RE_NEEDS_CLARIFICATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\[NEEDS CLARIFICATION:\s*(.+?)\]")
        .expect("needs-clarification regex should compile")
});

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
    /// Unresolved `[NEEDS CLARIFICATION]` marker.
    Clarification,
    /// Ambiguous language (vague terms, undefined references).
    Ambiguity,
    /// Contradiction between two requirements.
    Contradiction,
    /// Requirement lacks acceptance criteria.
    Gap,
    /// Phase -1 pre-implementation gate not checked (FR-008).
    PhaseMinusOneGate,
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
            Self::Clarification => write!(f, "Clarification"),
            Self::Ambiguity => write!(f, "Ambiguity"),
            Self::Contradiction => write!(f, "Contradiction"),
            Self::Gap => write!(f, "Gap"),
            Self::PhaseMinusOneGate => write!(f, "Phase -1 Gate"),
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
    #[must_use]
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

    /// Returns `true` if the report contains any clarification-marker issues
    /// (FR-002).
    #[must_use]
    pub fn has_clarifications(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.category == Category::Clarification)
    }

    /// Count clarification-marker issues in the report (FR-002).
    #[must_use]
    pub fn clarification_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.category == Category::Clarification)
            .count()
    }

    /// Returns `true` if the report contains any consistency-check issues
    /// (ambiguity, contradiction, or gap) (FR-015).
    #[must_use]
    pub fn has_consistency_issues(&self) -> bool {
        self.issues.iter().any(|i| {
            i.category == Category::Ambiguity
                || i.category == Category::Contradiction
                || i.category == Category::Gap
        })
    }

    /// Count consistency-check issues in the report (FR-015).
    ///
    /// Includes ambiguity, contradiction, and gap issues.
    #[must_use]
    pub fn consistency_issue_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| {
                i.category == Category::Ambiguity
                    || i.category == Category::Contradiction
                    || i.category == Category::Gap
            })
            .count()
    }

    /// Count issues by category.
    #[must_use]
    pub fn count_by_category(&self, category: Category) -> usize {
        self.issues
            .iter()
            .filter(|i| i.category == category)
            .count()
    }

    /// Returns `true` if the report contains any Phase -1 gate issues
    /// (FR-008).
    #[must_use]
    pub fn has_phase_gate_issues(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.category == Category::PhaseMinusOneGate)
    }

    /// Count Phase -1 gate issues in the report (FR-008).
    #[must_use]
    pub fn phase_gate_issue_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.category == Category::PhaseMinusOneGate)
            .count()
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

        // FR-002: clarification-marker summary
        let clarifications = self.clarification_count();
        if clarifications > 0 {
            lines.push(format!(
                "Clarification markers: {clarifications} unresolved — see [Clarification] issues below"
            ));
        }

        // FR-015: consistency-check summary
        let consistency = self.consistency_issue_count();
        if consistency > 0 {
            let ambiguity = self.count_by_category(Category::Ambiguity);
            let contradictions = self.count_by_category(Category::Contradiction);
            let gaps = self.count_by_category(Category::Gap);
            lines.push(format!(
                "Consistency issues: {consistency} (ambiguity: {ambiguity}, contradictions: {contradictions}, gaps: {gaps})"
            ));
        }

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

/// A `[NEEDS CLARIFICATION: <question>]` marker found in a SPEC.md.
///
/// Produced by [`detect_clarification_markers`]. The marker is not an error
/// by itself — it signals an unresolved ambiguity that the author should
/// resolve before the spec can transition to `approved` (see FR-003).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClarificationMarker {
    /// 1-based line number where the marker appears.
    pub line: usize,
    /// The question text captured inside the brackets.
    pub question: String,
}

/// Detect all `[NEEDS CLARIFICATION: <question>]` markers in SPEC.md content.
///
/// Matching is case-insensitive. Each marker is returned with its 1-based
/// line number and the captured question text. Markers are **not** reported
/// as validation errors by this function — callers (e.g. T-008) decide
/// whether to add them to the [`Report`].
///
/// # Examples
///
/// ```
/// # use ragent_specs::validate::detect_clarification_markers;
/// let content = "Some text [NEEDS CLARIFICATION: what scale?] more text";
/// let markers = detect_clarification_markers(content);
/// assert_eq!(markers.len(), 1);
/// assert_eq!(markers[0].question, "what scale?");
/// assert_eq!(markers[0].line, 1);
/// ```
pub fn detect_clarification_markers(content: &str) -> Vec<ClarificationMarker> {
    let mut markers = Vec::new();
    for (i, line) in content.lines().enumerate() {
        for caps in RE_NEEDS_CLARIFICATION.captures_iter(line) {
            markers.push(ClarificationMarker {
                line: i + 1,
                question: caps[1].trim().to_string(),
            });
        }
    }
    markers
}

// ── Ambiguity detection (T-026, FR-015) ─────────────────────────────────────

/// Regex matching common vague/ambiguous terms in requirement text.
/// Matches whole words only (case-insensitive).
static RE_VAGUE_TERMS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(fast|efficient|scalable|robust|flexible|user-friendly|intuitive|simple|easy|appropriate|reasonable|adequate|sufficient|several|various|some|many|few|minimal|significant|substantial|high.?performance|low.?latency|high.?quality|as\s+needed|as\s+appropriate|etc\.?)\b")
        .expect("vague-terms regex should compile")
});

/// Regex matching potential acronyms: 2–6 consecutive uppercase ASCII letters.
static RE_ACRONYM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b([A-Z]{2,6})\b").expect("acronym regex should compile"));

/// Regex matching acronym *definitions* — patterns like
/// "Full Name (ACRONYM)", "ACRONYM (Full Name)",
/// "ACRONYM — Full Name", or "ACRONYM: Full Name".
static RE_ACRONYM_DEFINED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\(([A-Z]{2,6})\)|\b([A-Z]{2,6})\b\s*(?:\(|\u2014\u2014?|:))")
        .expect("acronym-definition regex should compile")
});

/// Kind of ambiguity detected (FR-015).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbiguityKind {
    /// A vague or unmeasurable term (e.g. "fast", "scalable").
    VagueTerm,
    /// An acronym used in requirements that is never defined in the spec.
    UndefinedAcronym,
}

impl fmt::Display for AmbiguityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::VagueTerm => write!(f, "vague term"),
            Self::UndefinedAcronym => write!(f, "undefined acronym"),
        }
    }
}

/// A single ambiguity issue detected in a SPEC.md (FR-015, T-026).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbiguityIssue {
    /// 1-based line number where the ambiguous text appears.
    pub line: usize,
    /// Requirement ID (e.g. "FR-003") nearest to the issue, if identifiable.
    pub requirement_id: Option<String>,
    /// Kind of ambiguity.
    pub kind: AmbiguityKind,
    /// The vague term or undefined acronym that was found.
    pub term: String,
    /// Optional human-readable suggestion for resolving the ambiguity.
    pub suggestion: String,
}

/// Detect ambiguity in SPEC.md content: vague terms and undefined acronyms
/// (FR-015, T-026).
///
/// This function scans the EARS text of every parsed requirement for:
///
/// - **Vague terms** — unmeasurable language like "fast", "scalable", or
///   "user-friendly" that lacks acceptance criteria.
/// - **Undefined acronyms** — uppercase abbreviations (2–6 letters) used in
///   requirement text but never defined elsewhere in the spec via a
///   `Full Name (ACRONYM)` or `ACRONYM (Full Name)` pattern.
///
/// Issues are returned as [`AmbiguityIssue`] entries. They are **not**
/// automatically added to a [`Report`] — the caller (T-029) decides whether to
/// incorporate them as warnings.
///
/// # Examples
///
/// ```
/// # use ragent_specs::validate::{detect_ambiguity, AmbiguityKind};
/// let content = "## Functional Requirements\n\n### FR-001 - Performance\n\n`The system shall be fast and scalable.`\n";
/// let issues = detect_ambiguity(content);
/// assert!(issues.iter().any(|i| i.term == "fast"));
/// assert!(issues.iter().any(|i| i.term == "scalable"));
/// ```
#[must_use]
pub fn detect_ambiguity(content: &str) -> Vec<AmbiguityIssue> {
    let reqs = parse_requirements(content);
    let mut issues = Vec::new();

    // Collect all acronyms that are defined somewhere in the spec.
    let defined_acronyms = collect_defined_acronyms(content);

    for req in &reqs {
        if req.ears_text.is_empty() {
            continue;
        }

        // Vague term detection
        for m in RE_VAGUE_TERMS.find_iter(&req.ears_text) {
            let term = m.as_str().trim().to_lowercase();
            let term = term.replace(['\n', '\r'], " ");
            let term = term.split_whitespace().collect::<Vec<_>>().join(" ");
            issues.push(AmbiguityIssue {
                line: req.ears_line,
                requirement_id: Some(req.id.clone()),
                kind: AmbiguityKind::VagueTerm,
                term,
                suggestion: "replace with a measurable criterion".to_string(),
            });
        }

        // Undefined acronym detection
        for m in RE_ACRONYM.find_iter(&req.ears_text) {
            let acronym = m.as_str().to_string();
            if defined_acronyms.contains(&acronym) {
                continue;
            }
            issues.push(AmbiguityIssue {
                line: req.ears_line,
                requirement_id: Some(req.id.clone()),
                kind: AmbiguityKind::UndefinedAcronym,
                term: acronym.clone(),
                suggestion: format!(
                    "define \"{acronym}\" on first use, e.g. 'Full Name ({acronym})'"
                ),
            });
        }
    }

    issues
}

/// Collect every acronym that appears in a definition pattern within `content`.
///
/// Recognised definition patterns:
/// - `Full Name (ACRONYM)` — captures the acronym inside parentheses.
/// - `ACRONYM (Full Name)` — captures the acronym before the parenthetical.
/// - `ACRONYM — Full Name` — captures the acronym before an em-dash.
/// - `ACRONYM: Full Name` — captures the acronym before a colon.
fn collect_defined_acronyms(content: &str) -> Vec<String> {
    let mut defined = Vec::new();
    for caps in RE_ACRONYM_DEFINED.captures_iter(content) {
        // The regex has two capture groups; one of them will be non-empty.
        let acronym = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        if !acronym.is_empty() && !defined.contains(&acronym) {
            defined.push(acronym);
        }
    }
    defined
}

// ── Contradiction detection (T-027, FR-015) ────────────────────────────────

/// Kind of contradiction detected between two requirements (FR-015, T-027).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContradictionKind {
    /// One requirement mandates an action that another explicitly forbids
    /// (e.g. "shall store" vs "shall not store").
    NegationConflict,
    /// Two requirements use mutually exclusive verbs for the same subject
    /// (e.g. "shall accept" vs "shall reject").
    OppositeAction,
}

impl fmt::Display for ContradictionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NegationConflict => write!(f, "negation conflict"),
            Self::OppositeAction => write!(f, "opposite action"),
        }
    }
}

/// A contradiction detected between two requirements (FR-015, T-027).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContradictionIssue {
    /// First requirement ID in the conflicting pair (e.g. "FR-001").
    pub req_a: String,
    /// Second requirement ID in the conflicting pair (e.g. "FR-002").
    pub req_b: String,
    /// Line number of the first requirement's EARS text.
    pub line_a: usize,
    /// Line number of the second requirement's EARS text.
    pub line_b: usize,
    /// Kind of contradiction.
    pub kind: ContradictionKind,
    /// The conflicting action or term found in both requirements.
    pub term: String,
    /// Human-readable description of the contradiction.
    pub description: String,
}

/// Pairs of verbs that are semantically opposite (antonyms).
///
/// Each tuple is stored in both orders so lookup is symmetric.
const OPPOSITE_VERBS: &[(&str, &str)] = &[
    ("accept", "reject"),
    ("reject", "accept"),
    ("enable", "disable"),
    ("disable", "enable"),
    ("allow", "prevent"),
    ("prevent", "allow"),
    ("allow", "deny"),
    ("deny", "allow"),
    ("allow", "block"),
    ("block", "allow"),
    ("permit", "forbid"),
    ("forbid", "permit"),
    ("include", "exclude"),
    ("exclude", "include"),
    ("start", "stop"),
    ("stop", "start"),
    ("create", "delete"),
    ("delete", "create"),
    ("add", "remove"),
    ("remove", "add"),
    ("open", "close"),
    ("close", "open"),
    ("begin", "end"),
    ("end", "begin"),
    ("require", "forbid"),
    ("forbid", "require"),
    ("store", "delete"),
    ("delete", "store"),
    ("retain", "purge"),
    ("purge", "retain"),
    ("encrypt", "decrypt"),
    ("decrypt", "encrypt"),
    ("grant", "revoke"),
    ("revoke", "grant"),
    ("activate", "deactivate"),
    ("deactivate", "activate"),
    ("connect", "disconnect"),
    ("disconnect", "connect"),
    ("show", "hide"),
    ("hide", "show"),
];

/// Parsed action from an EARS requirement: polarity + verb + object.
struct ParsedAction {
    is_negative: bool,
    verb: String,
    object: String,
}

/// Extract the action phrase from EARS text, separating the polarity
/// (positive vs negative) and the base verb.
///
/// Returns `Some(ParsedAction)` if a "shall" clause is found, `None` otherwise.
fn extract_action(ears_text: &str) -> Option<ParsedAction> {
    let lower = ears_text.to_lowercase();
    let shall_pos = lower.find("shall")?;
    let after_shall = lower[shall_pos + 5..].trim_start().to_string();

    // Check for negation: "shall not" or "shall never"
    let (is_negative, rest) = if let Some(r) = after_shall.strip_prefix("not ") {
        (true, r.trim_start().to_string())
    } else if let Some(r) = after_shall.strip_prefix("never ") {
        (true, r.trim_start().to_string())
    } else {
        (false, after_shall)
    };

    // Extract the first word as the verb
    let mut parts = rest.splitn(2, char::is_whitespace);
    let verb = parts.next()?.trim().to_string();
    if verb.is_empty() {
        return None;
    }
    let object = parts.next().unwrap_or("").trim().to_string();
    Some(ParsedAction {
        is_negative,
        verb,
        object,
    })
}

/// Detect contradictions between requirements in a SPEC.md (FR-015, T-027).
///
/// This function compares every pair of parsed requirements and checks for:
///
/// - **Negation conflicts** — one requirement says "shall \<verb\> \<object\>"
///   while another says "shall not \<verb\> \<object\>" for the same verb and
///   object.
/// - **Opposite actions** — two requirements use antonymous verbs (e.g.
///   "shall accept" vs "shall reject") with a similar object phrase.
///
/// Issues are returned as [`ContradictionIssue`] entries. They are **not**
/// automatically added to a [`Report`] — the caller (T-029) decides whether
/// to incorporate them as warnings.
///
/// # Examples
///
/// ```
/// # use ragent_specs::validate::{detect_contradictions, ContradictionKind};
/// let content = "\
/// ## Functional Requirements
///
/// ### FR-001 - Store Data
///
/// `The system shall store all user data.`
///
/// ### FR-002 - No Storage
///
/// `The system shall not store all user data.`
/// ";
/// let issues = detect_contradictions(content);
/// assert!(issues.iter().any(|i| i.kind == ContradictionKind::NegationConflict));
/// ```
#[must_use]
pub fn detect_contradictions(content: &str) -> Vec<ContradictionIssue> {
    let reqs = parse_requirements(content);
    let mut issues = Vec::new();

    // Extract actions for all requirements up front
    let actions: Vec<(&ParsedRequirement, Option<ParsedAction>)> = reqs
        .iter()
        .map(|r| (r, extract_action(&r.ears_text)))
        .collect();

    for i in 0..actions.len() {
        let (req_a, act_a_opt) = &actions[i];
        let act_a = match act_a_opt {
            Some(a) => a,
            None => continue,
        };

        for (req_b, act_b_opt) in actions.iter().skip(i + 1) {
            let act_b = match act_b_opt {
                Some(b) => b,
                None => continue,
            };

            // Check negation conflict: same verb, same object, opposite polarity
            if act_a.verb == act_b.verb
                && act_a.object == act_b.object
                && act_a.is_negative != act_b.is_negative
                && !act_a.verb.is_empty()
            {
                let positive_req = if !act_a.is_negative { req_a } else { req_b };
                let negative_req = if act_a.is_negative { req_a } else { req_b };
                issues.push(ContradictionIssue {
                    req_a: positive_req.id.clone(),
                    req_b: negative_req.id.clone(),
                    line_a: positive_req.ears_line,
                    line_b: negative_req.ears_line,
                    kind: ContradictionKind::NegationConflict,
                    term: act_a.verb.clone(),
                    description: format!(
                        "{} requires \"shall {} {}\" but {} requires \"shall not {} {}\"",
                        positive_req.id,
                        act_a.verb,
                        act_a.object,
                        negative_req.id,
                        act_a.verb,
                        act_a.object
                    ),
                });
                continue;
            }

            // Check opposite-action conflict: antonymous verbs, similar object
            if act_a.is_negative == act_b.is_negative {
                // Only check when both are same polarity (both positive or both negative)
                for (v1, v2) in OPPOSITE_VERBS {
                    if act_a.verb == *v1 && act_b.verb == *v2 {
                        // Check that the object phrases share at least one
                        // significant word (to reduce false positives)
                        if objects_overlap(&act_a.object, &act_b.object) {
                            issues.push(ContradictionIssue {
                                req_a: req_a.id.clone(),
                                req_b: req_b.id.clone(),
                                line_a: req_a.ears_line,
                                line_b: req_b.ears_line,
                                kind: ContradictionKind::OppositeAction,
                                term: format!("{v1}/{v2}"),
                                description: format!(
                                    "{} uses \"shall {} {}\" but {} uses \"shall {} {}\" — \
                                     these verbs are semantically opposite",
                                    req_a.id,
                                    act_a.verb,
                                    act_a.object,
                                    req_b.id,
                                    act_b.verb,
                                    act_b.object
                                ),
                            });
                            break;
                        }
                    }
                }
            }
        }
    }

    issues
}

/// Check if two object phrases share at least one significant word
/// (length > 2, not a stop word).
fn objects_overlap(a: &str, b: &str) -> bool {
    const STOP_WORDS: &[&str] = &[
        "the", "all", "any", "and", "for", "with", "from", "into", "that", "this", "data", "user",
    ];
    let words_a: std::collections::HashSet<&str> = a
        .split_whitespace()
        .filter(|w| w.len() > 2 && !STOP_WORDS.contains(w))
        .collect();
    let words_b: std::collections::HashSet<&str> = b
        .split_whitespace()
        .filter(|w| w.len() > 2 && !STOP_WORDS.contains(w))
        .collect();
    words_a.iter().any(|w| words_b.contains(w))
}

// ── Gap detection (T-028, FR-015) ──────────────────────────────────────────

/// Kind of acceptance-criteria gap detected in a requirement (FR-015, T-028).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GapKind {
    /// The EARS text contains no numeric value or measurable quantity.
    NoMeasurableCriterion,
    /// The requirement uses a copular verb ("shall be …") without specifying
    /// a testable state or observable outcome.
    VagueOutcome,
}

impl fmt::Display for GapKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoMeasurableCriterion => write!(f, "no measurable criterion"),
            Self::VagueOutcome => write!(f, "vague outcome"),
        }
    }
}

/// A gap detected in a requirement that lacks acceptance criteria (FR-015, T-028).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GapIssue {
    /// Requirement ID (e.g. "FR-003").
    pub requirement_id: String,
    /// 1-based line number of the requirement's EARS text.
    pub line: usize,
    /// Kind of gap.
    pub kind: GapKind,
    /// The EARS text that was analysed.
    pub ears_text: String,
    /// Human-readable suggestion for adding acceptance criteria.
    pub suggestion: String,
}

/// Regex matching a digit (0–9) anywhere in the text.
static RE_DIGIT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d").expect("digit regex should compile"));

/// Verbs that produce directly testable, observable outcomes.
const TESTABLE_VERBS: &[&str] = &[
    "store",
    "save",
    "write",
    "read",
    "delete",
    "remove",
    "create",
    "generate",
    "send",
    "receive",
    "display",
    "show",
    "hide",
    "log",
    "record",
    "return",
    "output",
    "print",
    "reject",
    "accept",
    "enable",
    "disable",
    "start",
    "stop",
    "connect",
    "disconnect",
    "encrypt",
    "decrypt",
    "validate",
    "verify",
    "check",
    "parse",
    "convert",
    "transform",
    "export",
    "import",
    "upload",
    "download",
    "render",
    "update",
    "insert",
    "query",
    "authenticate",
    "authorize",
    "grant",
    "revoke",
    "notify",
    "alert",
    "block",
    "allow",
    "process",
    "execute",
    "run",
    "compile",
    "build",
    "deploy",
    "rollback",
    "snapshot",
];

/// Detect gaps where requirements lack acceptance criteria (FR-015, T-028).
///
/// This function scans every parsed requirement's EARS text for signals that
/// acceptance criteria may be missing:
///
/// - **No measurable criterion** — the EARS text contains no digit (0–9),
///   implying there is no quantifiable threshold, count, time bound, or
///   percentage to verify against.
/// - **Vague outcome** — the verb after "shall" is "be" (a copular verb),
///   meaning the requirement describes a state rather than a testable action.
///   Only flagged when the requirement also lacks a digit.
///
/// Requirements that use a testable action verb (e.g. "shall store", "shall
/// display") are considered to have implicit acceptance criteria even without
/// a numeric threshold, so `NoMeasurableCriterion` is not reported for them.
///
/// Issues are returned as [`GapIssue`] entries. They are **not** automatically
/// added to a [`Report`] — the caller (T-029) decides whether to incorporate
/// them as warnings.
///
/// # Examples
///
/// ```
/// # use ragent_specs::validate::{detect_gaps, GapKind};
/// let content = "\
/// ## Functional Requirements
///
/// ### FR-001 - Quality
///
/// `The system shall be robust.`
/// ";
/// let issues = detect_gaps(content);
/// assert!(issues.iter().any(|i| i.kind == GapKind::VagueOutcome));
/// ```
#[must_use]
pub fn detect_gaps(content: &str) -> Vec<GapIssue> {
    let reqs = parse_requirements(content);
    let mut issues = Vec::new();

    for req in &reqs {
        if req.ears_text.is_empty() {
            continue;
        }

        let ears = &req.ears_text;
        let has_digit = RE_DIGIT.is_match(ears);
        let action = extract_action(ears);

        // Check for vague outcome: "shall be …" with no further testable action
        if let Some(act) = &action
            && act.verb == "be"
        {
            issues.push(GapIssue {
                requirement_id: req.id.clone(),
                line: req.ears_line,
                kind: GapKind::VagueOutcome,
                ears_text: ears.to_string(),
                suggestion: "replace \"shall be …\" with a testable action \
                             (e.g. \"shall display\", \"shall return\") or add a \
                             measurable criterion"
                    .to_string(),
            });
            continue;
        }

        // Check for no measurable criterion — but only if the verb is not
        // a recognised testable verb (those have implicit acceptance criteria).
        if !has_digit {
            let verb_is_testable = action
                .as_ref()
                .map(|a| TESTABLE_VERBS.contains(&a.verb.as_str()))
                .unwrap_or(false);

            if !verb_is_testable {
                issues.push(GapIssue {
                    requirement_id: req.id.clone(),
                    line: req.ears_line,
                    kind: GapKind::NoMeasurableCriterion,
                    ears_text: ears.to_string(),
                    suggestion: "add a measurable acceptance criterion \
                                 (e.g. a time bound, count, or percentage)"
                        .to_string(),
                });
            }
        }
    }

    issues
}

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
/// This is the backward-compatible entry point that runs every check
/// unconditionally (equivalent to [`validate_with_flags`] with
/// [`SddFlags::all_enabled`]). Use [`validate_with_flags`] when you need to
/// gate SDD capabilities based on configuration (FR-019).
#[must_use]
pub fn validate(spec: &Spec) -> Report {
    validate_with_flags(spec, &SddFlags::all_enabled())
}

/// Validate a spec with SDD capability flags gating which checks run
/// (FR-019).
///
/// Core checks — structural sections, EARS syntax, and PLAN.md completeness —
/// always run regardless of flags. SDD-specific checks are gated:
///
/// - `flags.clarification_markers` → [`validate_clarifications`] (FR-002)
/// - `flags.consistency_checks` → ambiguity, contradiction, and gap detection
///   (FR-015)
///
/// When a flag is `false`, the corresponding check is skipped entirely,
/// preserving existing workflows for users who have not opted in.
#[must_use]
pub fn validate_with_flags(spec: &Spec, flags: &SddFlags) -> Report {
    let mut report = Report::new();

    // Core checks — always run.
    validate_structure(spec, &mut report);
    validate_ears(spec, &mut report);
    validate_plan(spec, &mut report);

    // SDD-gated checks (FR-019).
    if flags.clarification_markers {
        validate_clarifications(spec, &mut report);
    }
    if flags.consistency_checks {
        validate_consistency(spec, &mut report);
    }
    if flags.phase_minus_one_gates {
        validate_phase_minus_one_gates(spec, &mut report);
    }

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

/// Validate clarification markers: detect `[NEEDS CLARIFICATION: <question>]`
/// markers in SPEC.md and add them as warning issues to the report (FR-002).
///
/// Markers are reported as [`Severity::Warning`] with [`Category::Clarification`]
/// so the author knows which ambiguities remain unresolved. The spec is not
/// rejected (no errors), but the markers are surfaced during validation.
pub fn validate_clarifications(spec: &Spec, report: &mut Report) {
    let markers = detect_clarification_markers(&spec.spec_md);
    for marker in markers {
        report.add(
            Issue::new(
                Severity::Warning,
                Category::Clarification,
                format!(
                    "Unresolved clarification marker: [NEEDS CLARIFICATION: {}]",
                    marker.question
                ),
            )
            .with_line(marker.line),
        );
    }
}

/// Run consistency checks (ambiguity, contradiction, gap) and add issues to
/// the report as warnings (FR-015).
///
/// This function aggregates the three [`detect_ambiguity`],
/// [`detect_contradictions`], and [`detect_gaps`] detectors, converting their
/// findings into [`Issue`] entries with [`Severity::Warning`] and the
/// appropriate [`Category`]. It is gated by `flags.consistency_checks` in
/// [`validate_with_flags`] (FR-019).
pub fn validate_consistency(spec: &Spec, report: &mut Report) {
    // Ambiguity issues (FR-015: vague terms, undefined acronyms)
    for issue in detect_ambiguity(&spec.spec_md) {
        let id = issue.requirement_id.as_deref().unwrap_or("");
        report.add(
            Issue::new(
                Severity::Warning,
                Category::Ambiguity,
                format!("{}: \"{}\" — {}", issue.kind, issue.term, issue.suggestion),
            )
            .with_line(issue.line)
            .with_id(id),
        );
    }

    // Contradiction issues (FR-015: negation conflicts, opposite actions)
    for issue in detect_contradictions(&spec.spec_md) {
        report.add(
            Issue::new(
                Severity::Warning,
                Category::Contradiction,
                format!(
                    "{} between {} and {}: {}",
                    issue.kind, issue.req_a, issue.req_b, issue.description
                ),
            )
            .with_line(issue.line_a),
        );
    }

    // Gap issues (FR-015: missing acceptance criteria)
    for issue in detect_gaps(&spec.spec_md) {
        report.add(
            Issue::new(
                Severity::Warning,
                Category::Gap,
                format!(
                    "{} ({}): {}",
                    issue.kind, issue.requirement_id, issue.suggestion
                ),
            )
            .with_line(issue.line)
            .with_id(&issue.requirement_id),
        );
    }
}

/// Validate Phase -1 pre-implementation gates in PLAN.md (FR-008, T-016).
///
/// Parses the `## Phase -1 Gates` section from the PLAN.md and checks that
/// all three required gates (Simplicity, Anti-Abstraction, Integration-First)
/// are present and checked. Issues are reported as [`Severity::Warning`] —
/// they are advisory during validation. The actual transition blocking is
/// handled separately (T-017).
///
/// - If the PLAN.md is empty, this check is skipped (`validate_plan` already
///   reports that error).
/// - If the `## Phase -1 Gates` section is entirely absent, a single warning
///   is emitted noting the missing section.
/// - If the section is present but some required gates are unchecked or
///   missing, a warning is emitted for each unchecked gate.
/// - If all required gates are checked, no issues are added.
pub fn validate_phase_minus_one_gates(spec: &Spec, report: &mut Report) {
    if spec.plan_md.is_empty() {
        return;
    }

    let gates = PlanParser::parse_phase_minus_one_gates(&spec.plan_md);

    if gates.is_empty() {
        report.add(Issue::new(
            Severity::Warning,
            Category::PhaseMinusOneGate,
            "PLAN.md missing '## Phase -1 Gates' section — required gates \
             (Simplicity, Anti-Abstraction, Integration-First) are not \
             documented (FR-008)",
        ));
        return;
    }

    for gate_name in gates.unchecked_required_gates() {
        report.add(Issue::new(
            Severity::Warning,
            Category::PhaseMinusOneGate,
            format!(
                "Phase -1 gate '{gate_name}' is unchecked — must be \
                 acknowledged before implementation (FR-008)"
            ),
        ));
    }
}

// ── Convenience API ────────────────────────────────────────────────────────

// NOTE: Async filesystem validation is a Milestone-4 integration task.
// The `validate` function above is sufficient for the validation engine.

#[cfg(test)]
#[path = "../tests/inline/validate.rs"]
mod tests_tests;
