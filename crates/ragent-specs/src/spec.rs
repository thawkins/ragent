use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// Unique, URL-safe identifier for a spec.
///
/// Used as the directory name under `specs/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpecId(String);

impl SpecId {
    /// Create a new SpecId from a string.
    ///
    /// Returns `None` if the string is empty or contains invalid characters.
    pub fn new(id: impl Into<String>) -> Option<Self> {
        let id = id.into();
        if id.is_empty() {
            return None;
        }
        // Only allow alphanumeric, hyphen, underscore
        if id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            Some(Self(id))
        } else {
            None
        }
    }

    /// Get the raw string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Get the directory name for this spec.
    pub fn dir_name(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SpecId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl AsRef<str> for SpecId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Lifecycle status of a spec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpecStatus {
    /// Initial draft state.
    #[serde(rename = "draft")]
    Draft,
    /// Under review by stakeholders.
    #[serde(rename = "in_review")]
    InReview,
    /// Approved and ready for implementation.
    #[serde(rename = "approved")]
    Approved,
    /// Implementation in progress.
    #[serde(rename = "in_progress")]
    InProgress,
    /// Code implementation complete.
    #[serde(rename = "implemented")]
    Implemented,
    /// Verified as correct and complete.
    #[serde(rename = "verified")]
    Verified,
    /// Archived, excluded from active queries.
    #[serde(rename = "archived")]
    Archived,
}

impl SpecStatus {
    /// All possible status values.
    pub const ALL: &[SpecStatus] = &[
        Self::Draft,
        Self::InReview,
        Self::Approved,
        Self::InProgress,
        Self::Implemented,
        Self::Verified,
        Self::Archived,
    ];

    /// Human-readable name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::InReview => "in_review",
            Self::Approved => "approved",
            Self::InProgress => "in_progress",
            Self::Implemented => "implemented",
            Self::Verified => "verified",
            Self::Archived => "archived",
        }
    }

    /// Parse a status from its string representation.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::Draft),
            "in_review" => Some(Self::InReview),
            "approved" => Some(Self::Approved),
            "in_progress" => Some(Self::InProgress),
            "implemented" => Some(Self::Implemented),
            "verified" => Some(Self::Verified),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

impl fmt::Display for SpecStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The type of EARS template a requirement follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EarsTemplate {
    /// Ubiquitous: `The <SYSTEM> shall <RESPONSE>.`
    #[serde(rename = "ubiquitous")]
    Ubiquitous,
    /// Event-driven: `When <TRIGGER>, the <SYSTEM> shall <RESPONSE>.`
    #[serde(rename = "event_driven")]
    EventDriven,
    /// State-driven: `While <PRECONDITION>, the <SYSTEM> shall <RESPONSE>.`
    #[serde(rename = "state_driven")]
    StateDriven,
    /// Optional: `Where <FEATURE> is included, the <SYSTEM> shall <RESPONSE>.`
    #[serde(rename = "optional")]
    Optional,
    /// Unwanted behaviour: `If <TRIGGER>, the <SYSTEM> shall <RESPONSE>.`
    #[serde(rename = "unwanted")]
    Unwanted,
}

impl EarsTemplate {
    /// Human-readable name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ubiquitous => "ubiquitous",
            Self::EventDriven => "event_driven",
            Self::StateDriven => "state_driven",
            Self::Optional => "optional",
            Self::Unwanted => "unwanted",
        }
    }
}

/// A single requirement within a spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirement {
    /// Unique identifier within the spec, e.g. "FR-007".
    pub id: String,
    /// The requirement text in EARS notation.
    pub text: String,
    /// The EARS template type.
    pub template: EarsTemplate,
    /// Whether this requirement has been implemented.
    pub implemented: bool,
}

/// Status of a task in a plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task not yet started.
    #[serde(rename = "pending")]
    Pending,
    /// Task actively being worked on.
    #[serde(rename = "in_progress")]
    InProgress,
    /// Task completed.
    #[serde(rename = "completed")]
    Completed,
    /// Task blocked or deferred.
    #[serde(rename = "blocked")]
    Blocked,
}

impl TaskStatus {
    /// Human-readable name.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Blocked => "blocked",
        }
    }

    /// Parse a status from its string representation.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "in_progress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            "blocked" => Some(Self::Blocked),
            _ => None,
        }
    }
}

/// A single implementation task in a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier within the plan, e.g. "T-003".
    pub id: String,
    /// Human-readable title.
    pub title: String,
    /// Detailed description.
    pub description: String,
    /// Linked requirement IDs (e.g. ["FR-007"]).
    pub linked_requirements: Vec<String>,
    /// Current status.
    pub status: TaskStatus,
    /// Estimated effort: S, M, L.
    pub effort: String,
    /// Priority: Critical, High, Medium, Low.
    pub priority: String,
    /// Task IDs that must be completed before this one.
    pub dependencies: Vec<String>,
    /// Completion timestamp (Unix epoch seconds).
    pub completed_at: Option<u64>,
}

/// A spec, encompassing both the specification and its plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spec {
    /// Unique spec identifier.
    pub id: SpecId,
    /// Current lifecycle status.
    pub status: SpecStatus,
    /// Short title.
    pub title: String,
    /// Full specification text (raw markdown).
    pub spec_md: String,
    /// Implementation plan text (raw markdown).
    pub plan_md: String,
    /// Parsed requirements.
    pub requirements: Vec<Requirement>,
    /// Parsed plan tasks.
    pub tasks: Vec<Task>,
    /// Audit trail entries: (timestamp, old_status, new_status, actor).
    pub audit_trail: Vec<(u64, String, String, String)>,
    /// Optional reviewer assignments.
    pub reviewers: Vec<String>,
    /// Review comments text (contents of REVIEW.md if it exists).
    pub review_md: String,
    /// Last modified timestamp (Unix epoch seconds).
    pub modified_at: u64,
    /// Directory path where the spec lives.
    #[serde(skip)]
    pub path: Option<PathBuf>,
}

impl Spec {
    /// Create a new empty spec with the given id and title.
    pub fn new(id: SpecId, title: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id,
            status: SpecStatus::Draft,
            title: title.into(),
            spec_md: String::new(),
            plan_md: String::new(),
            requirements: Vec::new(),
            tasks: Vec::new(),
            audit_trail: vec![(now, "none".to_string(), "draft".to_string(), "system".to_string())],
            reviewers: Vec::new(),
            review_md: String::new(),
            modified_at: now,
            path: None,
        }
    }

    /// Get the path to the spec directory.
    pub fn dir_path(&self, specs_root: &Path) -> PathBuf {
        specs_root.join(self.id.dir_name())
    }

    /// Get the path to SPEC.md.
    pub fn spec_md_path(&self, specs_root: &Path) -> PathBuf {
        self.dir_path(specs_root).join("SPEC.md")
    }

    /// Get the path to PLAN.md.
    pub fn plan_md_path(&self, specs_root: &Path) -> PathBuf {
        self.dir_path(specs_root).join("PLAN.md")
    }

    /// Compute implementation coverage as a percentage.
    pub fn coverage_pct(&self) -> f64 {
        if self.requirements.is_empty() {
            return 0.0;
        }
        let implemented = self.requirements.iter().filter(|r| r.implemented).count();
        (implemented as f64 / self.requirements.len() as f64) * 100.0
    }

    /// Transition the spec to a new status, recording an audit entry.
    pub fn transition(&mut self, new_status: SpecStatus, actor: impl Into<String>) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let old = self.status.as_str().to_string();
        self.audit_trail.push((now, old, new_status.as_str().to_string(), actor.into()));
        self.status = new_status;
        self.modified_at = now;
    }
}

/// A plan document paired with a spec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan {
    /// The spec this plan belongs to.
    pub spec_id: SpecId,
    /// Plan title (usually mirrors spec title).
    pub title: String,
    /// Raw markdown content.
    pub content: String,
    /// Parsed tasks.
    pub tasks: Vec<Task>,
    /// Last modified timestamp.
    pub modified_at: u64,
}

impl Plan {
    /// Create a new empty plan for the given spec.
    pub fn new(spec_id: SpecId, title: impl Into<String>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            spec_id,
            title: title.into(),
            content: String::new(),
            tasks: Vec::new(),
            modified_at: now,
        }
    }

    /// Get the path to PLAN.md.
    pub fn path(&self, specs_root: &Path) -> PathBuf {
        specs_root.join(self.spec_id.dir_name()).join("PLAN.md")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_id_valid() {
        assert!(SpecId::new("my-spec-1").is_some());
        assert!(SpecId::new("testspec").is_some());
        assert!(SpecId::new("spec_mgt_v1").is_some());
    }

    #[test]
    fn test_spec_id_invalid() {
        assert!(SpecId::new("").is_none());
        assert!(SpecId::new("my spec").is_none());
        assert!(SpecId::new("my/spec").is_none());
        assert!(SpecId::new("my.spec").is_none());
    }

    #[test]
    fn test_spec_status_parse() {
        assert_eq!(SpecStatus::parse("draft"), Some(SpecStatus::Draft));
        assert_eq!(SpecStatus::parse("in_review"), Some(SpecStatus::InReview));
        assert_eq!(SpecStatus::parse("approved"), Some(SpecStatus::Approved));
        assert_eq!(SpecStatus::parse("in_progress"), Some(SpecStatus::InProgress));
        assert_eq!(SpecStatus::parse("implemented"), Some(SpecStatus::Implemented));
        assert_eq!(SpecStatus::parse("verified"), Some(SpecStatus::Verified));
        assert_eq!(SpecStatus::parse("archived"), Some(SpecStatus::Archived));
        assert_eq!(SpecStatus::parse("unknown"), None);
    }

    #[test]
    fn test_spec_status_display() {
        assert_eq!(SpecStatus::Draft.to_string(), "draft");
        assert_eq!(SpecStatus::InReview.to_string(), "in_review");
    }

    #[test]
    fn test_spec_new() {
        let id = SpecId::new("test").unwrap();
        let spec = Spec::new(id.clone(), "Test Spec");
        assert_eq!(spec.id.as_str(), "test");
        assert_eq!(spec.status, SpecStatus::Draft);
        assert_eq!(spec.title, "Test Spec");
        assert_eq!(spec.coverage_pct(), 0.0);
    }

    #[test]
    fn test_spec_paths() {
        let id = SpecId::new("testspec").unwrap();
        let spec = Spec::new(id, "Test");
        let root = Path::new("specs");
        assert_eq!(spec.dir_path(root), Path::new("specs/testspec"));
        assert_eq!(spec.spec_md_path(root), Path::new("specs/testspec/SPEC.md"));
        assert_eq!(spec.plan_md_path(root), Path::new("specs/testspec/PLAN.md"));
    }

    #[test]
    fn test_spec_transition() {
        let id = SpecId::new("test").unwrap();
        let mut spec = Spec::new(id, "Test");
        spec.transition(SpecStatus::InReview, "alice");
        assert_eq!(spec.status, SpecStatus::InReview);
        assert_eq!(spec.audit_trail.len(), 2);
        assert_eq!(spec.audit_trail[1].1, "draft");
        assert_eq!(spec.audit_trail[1].2, "in_review");
        assert_eq!(spec.audit_trail[1].3, "alice");
    }

    #[test]
    fn test_spec_coverage() {
        let id = SpecId::new("test").unwrap();
        let mut spec = Spec::new(id, "Test");
        spec.requirements = vec![
            Requirement {
                id: "FR-001".to_string(),
                text: "The system shall do X.".to_string(),
                template: EarsTemplate::Ubiquitous,
                implemented: true,
            },
            Requirement {
                id: "FR-002".to_string(),
                text: "The system shall do Y.".to_string(),
                template: EarsTemplate::Ubiquitous,
                implemented: false,
            },
        ];
        assert!((spec.coverage_pct() - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_task_status() {
        assert_eq!(TaskStatus::Pending.as_str(), "pending");
        assert_eq!(TaskStatus::Completed.as_str(), "completed");
    }

    #[test]
    fn test_ears_template_as_str() {
        assert_eq!(EarsTemplate::Ubiquitous.as_str(), "ubiquitous");
        assert_eq!(EarsTemplate::EventDriven.as_str(), "event_driven");
        assert_eq!(EarsTemplate::StateDriven.as_str(), "state_driven");
        assert_eq!(EarsTemplate::Optional.as_str(), "optional");
        assert_eq!(EarsTemplate::Unwanted.as_str(), "unwanted");
    }

    #[test]
    fn test_plan_new() {
        let id = SpecId::new("test").unwrap();
        let plan = Plan::new(id, "Test Plan");
        assert_eq!(plan.title, "Test Plan");
        assert!(plan.tasks.is_empty());
    }

    #[test]
    fn test_spec_status_all() {
        assert_eq!(SpecStatus::ALL.len(), 7);
        assert!(SpecStatus::ALL.contains(&SpecStatus::Draft));
        assert!(SpecStatus::ALL.contains(&SpecStatus::Archived));
    }
}
