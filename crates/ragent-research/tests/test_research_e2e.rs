//! End-to-end test (T-054): run research → spec → verify linkage.
//!
//! This is the highest-level integration test: it creates a real research
//! item on disk, then builds a SPEC.md via `SpecTemplate::generate_with_research`
//! to simulate the `/spec create --from-research rust-async` flow, and
//! asserts the resulting frontmatter and `## Related Research` section
//! correctly reference the captured research.

use ragent_research::ResearchManager;
use ragent_specs::{SpecCommand, SpecTemplate};
use tempfile::TempDir;

#[test]
fn end_to_end_research_then_spec() {
    let tmp = TempDir::new().unwrap();
    let mgr = ResearchManager::new(tmp.path().join("research"));

    // Phase 1: create a research item.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        mgr.create("rust-async", "Rust Async", "async/await idioms")
            .await
            .unwrap();
    });

    // Phase 2: build a SPEC.md with the --from-research frontmatter.
    let spec_id = ragent_specs::spec::SpecId::new("async-await").unwrap();
    let spec_md = SpecTemplate::generate_with_research(
        &spec_id,
        "Async/Await ergonomics",
        &["rust-async".to_string()],
    );
    assert!(
        spec_md.contains("research: [\"rust-async\"]"),
        "expected frontmatter to record the dependency; got:\n{spec_md}"
    );
    assert!(
        spec_md.contains("## Related Research"),
        "expected Related Research section; got:\n{spec_md}"
    );
    assert!(
        spec_md.contains("../research/rust-async/RESEARCH.md"),
        "expected cross-link to the captured research; got:\n{spec_md}"
    );

    // Phase 3: verify the spec command parser accepts the same input a
    // TUI user would type.
    let cmd = SpecCommand::parse("create async-await Add async/await ergonomics");
    match cmd {
        SpecCommand::Create {
            specname,
            feature,
            from_research: _,
        } => {
            assert_eq!(specname, "async-await");
            assert!(feature.contains("async/await ergonomics"));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}
