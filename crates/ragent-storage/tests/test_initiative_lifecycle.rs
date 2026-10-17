//! Isolation coverage for the initiative lifecycle
//! (`update_initiative`, `delete_initiative`) and the `progress` clamp.

use ragent_storage::storage::{InitiativeMilestone, Storage};

const PROJECT: &str = "/test/initiative-lifecycle";

fn ms(id: &str, title: &str, done: bool) -> InitiativeMilestone {
    InitiativeMilestone {
        id: id.to_string(),
        title: title.to_string(),
        done,
        completed_at: done.then(|| "2024-01-01T00:00:00+00:00".to_string()),
    }
}

fn storage() -> Storage {
    let s = Storage::open_in_memory().expect("storage");
    s.create_initiative(
        "g1",
        "Ship v2",
        "desc",
        &[ms("m1", "a", false)],
        PROJECT,
        "sess",
    )
    .expect("create");
    s
}

#[test]
fn update_initiative_sets_fields_and_progress() {
    let storage = storage();
    let changed = storage
        .update_initiative(
            "g1",
            PROJECT,
            Some("Ship v3"),
            Some("new desc"),
            Some(&[ms("m1", "a", true), ms("m2", "b", false)]),
            Some(40),
            None,
            None,
        )
        .expect("update");
    assert!(changed);

    let row = storage
        .get_initiative("g1", PROJECT)
        .expect("get")
        .expect("row");
    assert_eq!(row.title, "Ship v3");
    assert_eq!(row.description, "new desc");
    assert_eq!(row.progress, 40);
    let milestones = row.milestones();
    assert_eq!(milestones.len(), 2);
    assert!(milestones[0].done);
}

#[test]
fn update_initiative_completion_sets_and_clears_closed_at() {
    let storage = storage();
    storage
        .update_initiative(
            "g1",
            PROJECT,
            None,
            None,
            None,
            None,
            Some("completed"),
            None,
        )
        .expect("complete");
    let row = storage
        .get_initiative("g1", PROJECT)
        .expect("get")
        .expect("row");
    assert_eq!(row.status, "completed");
    assert!(row.closed_at.is_some(), "closed_at stamped on completion");

    // Re-opening clears the close timestamp.
    storage
        .update_initiative("g1", PROJECT, None, None, None, None, Some("active"), None)
        .expect("reopen");
    let row = storage
        .get_initiative("g1", PROJECT)
        .expect("get")
        .expect("row");
    assert_eq!(row.status, "active");
    assert!(row.closed_at.is_none(), "closed_at cleared on re-open");
}

#[test]
fn update_initiative_wrong_project_is_a_noop() {
    let storage = storage();
    let changed = storage
        .update_initiative(
            "g1",
            "/other/project",
            None,
            None,
            None,
            None,
            None,
            Some("x"),
        )
        .expect("update");
    assert!(!changed, "no row matches the wrong project");
}

#[test]
fn update_initiative_clamps_out_of_range_progress() {
    let storage = storage();
    // `progress` is a u8 so 200 is representable in the update; the reader
    // clamps it back into 0..=100 (FUNC-022).
    storage
        .update_initiative("g1", PROJECT, None, None, None, Some(200), None, None)
        .expect("update");
    let row = storage
        .get_initiative("g1", PROJECT)
        .expect("get")
        .expect("row");
    assert_eq!(row.progress, 100, "value above 100 is clamped on read");

    // A directly-seeded negative value reads back as 0.
    {
        let conn = storage.conn_lock_for_test().expect("lock");
        conn.execute("UPDATE initiatives SET progress = -5 WHERE id = 'g1'", [])
            .expect("seed negative");
    }
    let row = storage
        .get_initiative("g1", PROJECT)
        .expect("get")
        .expect("row");
    assert_eq!(row.progress, 0, "invalid progress reads as 0");
}

#[test]
fn delete_initiative_reports_row_removal() {
    let storage = storage();
    assert!(storage.delete_initiative("g1", PROJECT).expect("delete"));
    assert!(
        storage
            .get_initiative("g1", PROJECT)
            .expect("get")
            .is_none()
    );
    // A second delete finds nothing.
    assert!(
        !storage
            .delete_initiative("g1", PROJECT)
            .expect("delete again")
    );
    // Wrong project deletes nothing.
    storage
        .create_initiative("g2", "t", "", &[], PROJECT, "s")
        .expect("create g2");
    assert!(
        !storage
            .delete_initiative("g2", "/other")
            .expect("wrong project")
    );
}

#[test]
fn milestones_returns_empty_for_malformed_json() {
    let storage = storage();
    {
        let conn = storage.conn_lock_for_test().expect("lock");
        conn.execute(
            "UPDATE initiatives SET milestones_json = 'not json' WHERE id = 'g1'",
            [],
        )
        .expect("corrupt");
    }
    let row = storage
        .get_initiative("g1", PROJECT)
        .expect("get")
        .expect("row");
    assert!(
        row.milestones().is_empty(),
        "malformed milestone JSON reads as empty"
    );
}
