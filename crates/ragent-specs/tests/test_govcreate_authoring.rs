//! Tests for the govcreate spec-authoring stage: `build_govcreate_prompt`,
//! `build_architecture_structure_summary`, and `write_govcreate_spec`
//! (spec `govdoc` T-009; FR-008, FR-009, FR-006, FR-017).

use ragent_specs::SpecCommand;
use ragent_tools_extended::archdoc::{
    ArchitectureStructure, Component, DataStore, ExternalDependency, Interface, Relationship,
};
use ragent_tools_extended::project_scaffold::{ScaffoldRequest, parse_flags};

/// Build a validated scaffold request from `/new` flag tokens.
fn scaffold(flags: &[&str]) -> ScaffoldRequest {
    parse_flags(flags).expect("test fixture flags must be valid")
}

/// A fully-populated structure as a clean LLM extraction would produce.
fn full_structure() -> ArchitectureStructure {
    ArchitectureStructure {
        components: vec![
            Component {
                name: "Payment Gateway".to_string(),
                responsibilities: vec!["authorise payments".to_string()],
            },
            Component {
                name: "Ledger".to_string(),
                responsibilities: vec![
                    "record postings".to_string(),
                    "compute balances".to_string(),
                ],
            },
        ],
        interfaces: vec![Interface {
            name: "Payments API".to_string(),
            between: vec!["Payment Gateway".to_string(), "Ledger".to_string()],
            contract_desc: "REST over TLS".to_string(),
        }],
        data_stores: vec![DataStore {
            name: "Postgres".to_string(),
            kind: "database".to_string(),
            used_by: vec!["Ledger".to_string()],
        }],
        external_dependencies: vec![ExternalDependency {
            name: "Card Network".to_string(),
            kind: "external service".to_string(),
            used_by: vec!["Payment Gateway".to_string()],
        }],
        relationships: vec![Relationship {
            from: "Payment Gateway".to_string(),
            to: "Ledger".to_string(),
            kind: "writes".to_string(),
            detail: "settlement postings".to_string(),
        }],
        from_fallback: false,
    }
}

// ---------------------------------------------------------------------------
// build_govcreate_prompt (FR-008)
// ---------------------------------------------------------------------------

#[test]
fn test_govcreate_prompt_keeps_file_list_contract() {
    let prompt = SpecCommand::build_govcreate_prompt(
        "payments-arch",
        &full_structure(),
        "https://docs.example.gov/arch",
        "./payments-svc",
        &scaffold(&["--language", "rust", "--type", "cmdline"]),
    );
    // The identical file-list contract as build_create_prompt (FR-008).
    for needle in [
        "SPEC.md",
        "PLAN.md",
        "TESTPLAN.md",
        "EARS",
        "FR-001",
        "## Requirements",
        "T-001",
        "TC-001",
        "status: draft",
        "dependencies",
        "Critical",
        "Pending",
    ] {
        let lower = needle.to_lowercase();
        assert!(
            prompt.to_lowercase().contains(&lower),
            "prompt should contain {needle}: {prompt}"
        );
    }
}

#[test]
fn test_govcreate_prompt_targets_the_target_folder_specs_dir() {
    let prompt = SpecCommand::build_govcreate_prompt(
        "payments-arch",
        &full_structure(),
        "https://docs.example.gov/arch",
        "./payments-svc",
        &scaffold(&["--language", "rust", "--type", "cmdline"]),
    );
    for needle in [
        "./payments-svc/specs/payments-arch/SPEC.md",
        "./payments-svc/specs/payments-arch/PLAN.md",
        "./payments-svc/specs/payments-arch/TESTPLAN.md",
    ] {
        assert!(
            prompt.contains(needle),
            "prompt should target {needle}: {prompt}"
        );
    }
}

#[test]
fn test_govcreate_prompt_embeds_the_architecture_structure() {
    let prompt = SpecCommand::build_govcreate_prompt(
        "payments-arch",
        &full_structure(),
        "https://docs.example.gov/arch",
        "./payments-svc",
        &scaffold(&["--language", "rust", "--type", "cmdline"]),
    );
    for needle in [
        "Payment Gateway",
        "authorise payments",
        "Ledger",
        "Payments API",
        "Postgres",
        "Card Network",
        "writes",
    ] {
        assert!(
            prompt.contains(needle),
            "prompt should surface {needle} from the structure: {prompt}"
        );
    }
}

#[test]
fn test_govcreate_prompt_embeds_fr018_invocation_frontmatter() {
    let prompt = SpecCommand::build_govcreate_prompt(
        "payments-arch",
        &full_structure(),
        "https://docs.example.gov/arch",
        "./payments-svc",
        &scaffold(&["--language", "rust", "--type", "cmdline", "--stack", "axum"]),
    );
    for needle in [
        "---\nstatus: draft",
        "id: payments-arch",
        "invocation:",
        "command: \"/spec govcreate\"",
        "content_ref: \"https://docs.example.gov/arch\"",
        "target_folder: \"./payments-svc\"",
        "language: rust",
        "stack: \"axum\"",
        "hosting: none",
    ] {
        assert!(
            prompt.contains(needle),
            "prompt should embed the invocation frontmatter line {needle}: {prompt}"
        );
    }
}

#[test]
fn test_govcreate_prompt_marks_fallback_extraction() {
    let mut structure = full_structure();
    structure.from_fallback = true;
    let prompt = SpecCommand::build_govcreate_prompt(
        "payments-arch",
        &structure,
        "https://docs.example.gov/arch",
        "./payments-svc",
        &scaffold(&["--language", "python", "--type", "library"]),
    );
    assert!(
        prompt.contains("fell back to a mechanical per-source structure"),
        "prompt should state the fallback ran: {prompt}"
    );
}

#[test]
fn test_govcreate_prompt_empty_structure_never_renders_blank_section() {
    let prompt = SpecCommand::build_govcreate_prompt(
        "empty-arch",
        &ArchitectureStructure::default(),
        "./docs/",
        "./empty-svc",
        &scaffold(&["--language", "rust", "--type", "library"]),
    );
    assert!(
        prompt.contains("No architecture structure was extracted"),
        "empty structure should render an explicit sentence: {prompt}"
    );
}

// ---------------------------------------------------------------------------
// build_architecture_structure_summary (FR-008 helper)
// ---------------------------------------------------------------------------

#[test]
fn test_structure_summary_renders_all_sections() {
    let summary = SpecCommand::build_architecture_structure_summary(&full_structure());
    for needle in [
        "### Components",
        "### Interfaces",
        "### Data stores",
        "### External dependencies",
        "### Relationships",
        "- **Payment Gateway**: authorise payments",
        "- **Ledger**: record postings; compute balances",
        "Payment Gateway <-> Ledger",
        "REST over TLS",
        "- **Postgres** (database) used by Ledger",
        "- **Card Network** (external service) used by Payment Gateway",
        "- Payment Gateway -[writes]-> Ledger: settlement postings",
    ] {
        assert!(
            summary.contains(needle),
            "summary should contain {needle}: {summary}"
        );
    }
}

#[test]
fn test_structure_summary_omits_empty_sections() {
    let structure = ArchitectureStructure {
        components: vec![Component {
            name: "Gateway".to_string(),
            responsibilities: vec![],
        }],
        ..ArchitectureStructure::default()
    };
    let summary = SpecCommand::build_architecture_structure_summary(&structure);
    assert!(summary.contains("### Components"), "{summary}");
    assert!(summary.contains("- **Gateway**"), "{summary}");
    assert!(!summary.contains("### Interfaces"), "{summary}");
    assert!(!summary.contains("### Data stores"), "{summary}");
    assert!(!summary.contains("### External dependencies"), "{summary}");
    assert!(!summary.contains("### Relationships"), "{summary}");
}

#[test]
fn test_structure_summary_empty_structure_is_one_sentence() {
    let summary =
        SpecCommand::build_architecture_structure_summary(&ArchitectureStructure::default());
    assert_eq!(
        summary,
        "No architecture structure was extracted from the source documentation."
    );
}

// ---------------------------------------------------------------------------
// write_govcreate_spec (FR-009, FR-017)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_write_govcreate_spec_creates_directory_and_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = SpecCommand::write_govcreate_spec(
        temp.path(),
        "payments-arch",
        "---\nstatus: draft\n---\n## Requirements\n",
        "## Tasks\n",
        "## Test Cases\n",
        false,
    )
    .await
    .expect("write should succeed");
    assert_eq!(dir, temp.path().join("specs").join("payments-arch"));
    assert!(dir.join("SPEC.md").is_file());
    assert!(dir.join("PLAN.md").is_file());
    assert!(dir.join("TESTPLAN.md").is_file());
    let spec_md = std::fs::read_to_string(dir.join("SPEC.md")).expect("read SPEC.md");
    assert!(spec_md.contains("status: draft"), "{spec_md}");
}

#[tokio::test]
async fn test_write_govcreate_spec_refuses_existing_without_force() {
    let temp = tempfile::tempdir().expect("tempdir");
    let existing = temp.path().join("specs").join("payments-arch");
    std::fs::create_dir_all(&existing).expect("create existing spec dir");
    std::fs::write(existing.join("SPEC.md"), "original").expect("seed SPEC.md");

    let result = SpecCommand::write_govcreate_spec(
        temp.path(),
        "payments-arch",
        "new SPEC",
        "new PLAN",
        "new TESTPLAN",
        false,
    )
    .await;
    let err = result.expect_err("existing spec without --force must refuse");
    let message = err.to_string();
    assert!(
        message.contains("already exists") && message.contains("--force"),
        "refusal should name the cause and the remedy: {message}"
    );
    assert!(
        message.contains("payments-arch"),
        "refusal should name the spec: {message}"
    );
    // The existing file is untouched.
    let spec_md = std::fs::read_to_string(existing.join("SPEC.md")).expect("read");
    assert_eq!(spec_md, "original");
}

#[tokio::test]
async fn test_write_govcreate_spec_overwrites_with_force() {
    let temp = tempfile::tempdir().expect("tempdir");
    let existing = temp.path().join("specs").join("payments-arch");
    std::fs::create_dir_all(&existing).expect("create existing spec dir");
    std::fs::write(existing.join("SPEC.md"), "original").expect("seed SPEC.md");

    SpecCommand::write_govcreate_spec(
        temp.path(),
        "payments-arch",
        "new SPEC",
        "new PLAN",
        "new TESTPLAN",
        true,
    )
    .await
    .expect("forced overwrite should succeed");
    let spec_md = std::fs::read_to_string(existing.join("SPEC.md")).expect("read");
    assert_eq!(spec_md, "new SPEC");
    let plan_md = std::fs::read_to_string(existing.join("PLAN.md")).expect("read");
    assert_eq!(plan_md, "new PLAN");
}

#[tokio::test]
async fn test_write_govcreate_spec_writes_atomically_without_tmp_leftovers() {
    let temp = tempfile::tempdir().expect("tempdir");
    let dir = SpecCommand::write_govcreate_spec(
        temp.path(),
        "clean-arch",
        "SPEC",
        "PLAN",
        "TESTPLAN",
        false,
    )
    .await
    .expect("write should succeed");
    // Atomic writes rename a unique `.NAME.seq.tmp` file; nothing temporary
    // may remain behind (NFR-004 crash-durability contract).
    let leftovers: Vec<_> = std::fs::read_dir(&dir)
        .expect("list spec dir")
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with('.'))
        .collect();
    assert!(
        leftovers.is_empty(),
        "no temp files should remain: {leftovers:?}"
    );
}
