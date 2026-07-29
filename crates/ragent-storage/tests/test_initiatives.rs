//! Tests for the durable `initiatives` table CRUD in `ragent-storage`
//! (JCODEPLAN M8 T-070).

use ragent_storage::storage::{InitiativeMilestone, Storage};

const PROJECT: &str = "/test/storage-m8";

fn ms(id: &str, title: &str, done: bool) -> InitiativeMilestone {
    InitiativeMilestone {
        id: id.to_string(),
        title: title.to_string(),
        done,
        completed_at: None,
    }
}

#[test]
fn test_initiative_table_created_by_migrate() {
    let storage = Storage::open_in_memory().expect("storage");
    // If the table doesn't exist, this insert fails with "no such table".
    storage
        .create_initiative("t1", "Title", "Desc", &[], PROJECT, "sess-1")
        .expect("table exists and insert works");
    assert!(
        storage
            .get_initiative("t1", PROJECT)
            .expect("get")
            .is_some()
    );
}

#[test]
fn test_initiative_round_trip_fields() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_initiative(
            "g1",
            "Ship v2",
            "long description",
            &[ms("ms-1", "design", false), ms("ms-2", "build", false)],
            PROJECT,
            "sess-9",
        )
        .expect("create");

    let row = storage
        .get_initiative("g1", PROJECT)
        .expect("get")
        .expect("row");
    assert_eq!(row.id, "g1");
    assert_eq!(row.title, "Ship v2");
    assert_eq!(row.description, "long description");
    assert_eq!(row.status, "active");
    assert_eq!(row.progress, 0);
    assert_eq!(row.project, PROJECT);
    assert_eq!(row.session_id, "sess-9");
    assert!(row.closed_at.is_none());
    assert!(!row.created_at.is_empty());
    assert!(!row.updated_at.is_empty());

    let milestones = row.milestones();
    assert_eq!(milestones.len(), 2);
    assert_eq!(milestones[0].id, "ms-1");
    assert_eq!(milestones[1].title, "build");
    assert!(!milestones.iter().any(|m| m.done));
}

#[test]
fn test_initiative_list_status_filter() {
    let storage = Storage::open_in_memory().expect("storage");
    for (id, status) in [
        ("a1", "active"),
        ("a2", "active"),
        ("p1", "paused"),
        ("c1", "completed"),
    ] {
        storage
            .create_initiative(id, id, "", &[], PROJECT, "s")
            .expect("create");
        if status != "active" {
            storage
                .update_initiative(id, PROJECT, None, None, None, None, Some(status), None)
                .expect("set status");
        }
    }

    let active = storage
        .list_initiatives(PROJECT, Some("active"))
        .expect("active");
    assert_eq!(active.len(), 2);
    let paused = storage
        .list_initiatives(PROJECT, Some("paused"))
        .expect("paused");
    assert_eq!(paused.len(), 1);
    let all = storage.list_initiatives(PROJECT, None).expect("all");
    assert_eq!(all.len(), 4);
    let all_kw = storage
        .list_initiatives(PROJECT, Some("all"))
        .expect("all kw");
    assert_eq!(all_kw.len(), 4);
}

#[test]
fn test_initiative_close_sets_closed_at() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_initiative("g1", "T", "", &[], PROJECT, "s")
        .expect("create");
    storage
        .update_initiative(
            "g1",
            PROJECT,
            None,
            None,
            None,
            Some(100),
            Some("completed"),
            None,
        )
        .expect("complete");
    let row = storage
        .get_initiative("g1", PROJECT)
        .expect("get")
        .expect("row");
    assert_eq!(row.status, "completed");
    assert!(row.closed_at.is_some(), "closed_at set on completion");

    // Re-opening clears closed_at.
    storage
        .update_initiative(
            "g1",
            PROJECT,
            None,
            None,
            None,
            Some(50),
            Some("active"),
            None,
        )
        .expect("reopen");
    let row = storage
        .get_initiative("g1", PROJECT)
        .expect("get")
        .expect("row");
    assert_eq!(row.status, "active");
    assert!(row.closed_at.is_none(), "closed_at cleared on re-open");
}

#[test]
fn test_initiative_abandoned_sets_closed_at() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_initiative("g1", "T", "", &[], PROJECT, "s")
        .expect("create");
    storage
        .update_initiative(
            "g1",
            PROJECT,
            None,
            None,
            None,
            None,
            Some("abandoned"),
            None,
        )
        .expect("abandon");
    let row = storage
        .get_initiative("g1", PROJECT)
        .expect("get")
        .expect("row");
    assert_eq!(row.status, "abandoned");
    assert!(row.closed_at.is_some(), "closed_at set on abandonment");
}

#[test]
fn test_initiative_update_nonexistent_returns_false() {
    let storage = Storage::open_in_memory().expect("storage");
    let changed = storage
        .update_initiative("nope", PROJECT, Some("x"), None, None, None, None, None)
        .expect("update");
    assert!(!changed, "updating a missing row reports false");
}

#[test]
fn test_initiative_milestone_malformed_json_safe() {
    // Direct DB write of malformed milestones_json shouldn't crash the decoder.
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_initiative("g1", "T", "", &[], PROJECT, "s")
        .expect("create");

    // Corrupt the JSON via a raw connection is not exposed; instead verify the
    // decoder's fallback on a hand-constructed row.
    let row = storage
        .get_initiative("g1", PROJECT)
        .expect("get")
        .expect("row");
    assert!(
        row.milestones().is_empty(),
        "empty default decodes to empty vec"
    );

    // Hand-built row with broken JSON hits the unwrap_or_default fallback.
    let broken = ragent_storage::InitiativeRow {
        milestones_json: "not-json{{{".to_string(),
        ..row
    };
    assert!(broken.milestones().is_empty());
}
