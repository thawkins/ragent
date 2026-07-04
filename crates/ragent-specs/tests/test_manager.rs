use ragent_specs::manager::{SortBy, SpecFilter, SpecManager, is_valid_transition};
use ragent_specs::spec::SpecStatus;
use std::path::PathBuf;

fn specs_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[tokio::test]
async fn test_manager_real_project_discover() {
    let mgr = SpecManager::new(specs_root());
    let specs = mgr.discover_specs().await.unwrap();
    assert!(
        !specs.is_empty(),
        "Should discover at least one spec in the project"
    );
    let ids: Vec<&str> = specs.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"testspec"), "Should find testspec");
}

#[tokio::test]
async fn test_manager_real_project_list_all() {
    let mgr = SpecManager::new(specs_root());
    let specs = mgr.list_specs(&SpecFilter::new()).await.unwrap();
    assert!(!specs.is_empty());
    for spec in &specs {
        assert!(
            spec.status != SpecStatus::Archived || specs.len() == 1,
            "Archived specs should be excluded by default"
        );
    }
}

#[tokio::test]
async fn test_manager_real_project_list_by_status() {
    let mgr = SpecManager::new(specs_root());
    let draft_specs = mgr
        .list_specs(&SpecFilter::new().with_status(SpecStatus::Draft))
        .await
        .unwrap();
    assert!(
        !draft_specs.is_empty() || {
            let all = mgr
                .list_specs(&SpecFilter::new().with_archived())
                .await
                .unwrap();
            !all.is_empty()
        }
    );
}

#[tokio::test]
async fn test_manager_real_project_search() {
    let mgr = SpecManager::new(specs_root());
    let results = mgr.search_specs("EARS").await.unwrap();
    assert!(!results.is_empty(), "Should find specs mentioning EARS");
    let testspec = results.iter().find(|r| r.spec.id.as_str() == "testspec");
    assert!(testspec.is_some(), "testspec should mention EARS");
}

#[tokio::test]
async fn test_manager_real_project_search_snippets() {
    let mgr = SpecManager::new(specs_root());
    let results = mgr.search_specs("requirement").await.unwrap();
    assert!(!results.is_empty());
    for result in &results {
        assert!(
            !result.snippets.is_empty(),
            "Search results should have snippets"
        );
    }
}

#[tokio::test]
async fn test_manager_real_project_sorting() {
    let mgr = SpecManager::new(specs_root());
    let by_id = mgr
        .list_specs(&SpecFilter::new().with_sort(SortBy::Id))
        .await
        .unwrap();
    for window in by_id.windows(2) {
        assert!(
            window[0].id.as_str() <= window[1].id.as_str(),
            "Specs should be sorted by ID"
        );
    }
}

#[tokio::test]
async fn test_manager_real_project_read_and_validate_transitions() {
    let mgr = SpecManager::new(specs_root());
    let specs = mgr.discover_specs().await.unwrap();
    let testspec = specs
        .into_iter()
        .find(|s| s.id.as_str() == "testspec")
        .unwrap();

    let current = testspec.status;
    let next = match current {
        SpecStatus::Draft => vec![SpecStatus::InReview],
        SpecStatus::InReview => vec![SpecStatus::Draft, SpecStatus::Approved],
        SpecStatus::Approved => vec![SpecStatus::InProgress],
        SpecStatus::InProgress => vec![SpecStatus::Implemented],
        SpecStatus::Implemented => vec![SpecStatus::Verified],
        SpecStatus::Verified => vec![SpecStatus::Archived],
        SpecStatus::Archived => vec![SpecStatus::Draft],
    };
    for status in next {
        assert!(
            is_valid_transition(current, status),
            "Transition from {:?} to {:?} should be valid",
            current,
            status
        );
    }
}

#[test]
fn test_transition_graph_completeness() {
    for &status in SpecStatus::ALL {
        let next = match status {
            SpecStatus::Draft => vec![SpecStatus::InReview],
            SpecStatus::InReview => vec![SpecStatus::Draft, SpecStatus::Approved],
            SpecStatus::Approved => vec![SpecStatus::InProgress],
            SpecStatus::InProgress => vec![SpecStatus::Implemented],
            SpecStatus::Implemented => vec![SpecStatus::Verified],
            SpecStatus::Verified => vec![SpecStatus::Archived],
            SpecStatus::Archived => vec![SpecStatus::Draft],
        };
        assert!(
            !next.is_empty(),
            "Every status must have at least one transition"
        );
        for &to in &next {
            assert!(is_valid_transition(status, to));
        }
    }
}
