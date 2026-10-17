//! Isolation coverage for the structured-memory CRUD helpers not exercised by
//! the transaction tests: `list_memories`, `list_memories_for_project`,
//! `list_all_memories`, `count_memories`, `count_memories_for_project`,
//! `update_memory_content`, `set_memory_tags`, `increment_memory_access`,
//! `update_memory_confidence`, `clear_memories_for_project`.

use ragent_storage::storage::Storage;
use std::path::Path;

fn storage() -> Storage {
    Storage::open_in_memory().expect("storage")
}

fn add(storage: &Storage, content: &str, project: &str) -> i64 {
    storage
        .create_memory(content, "fact", "test", 0.8, project, "s1", &[])
        .expect("create")
}

#[test]
fn list_and_count_memories_by_project() {
    let storage = storage();
    add(&storage, "alpha one", "/work/proj");
    add(&storage, "alpha two", "/work/proj");
    add(&storage, "beta one", "/other/proj");

    let proj = storage.list_memories("/work/proj", 10).expect("list proj");
    assert_eq!(proj.len(), 2);
    assert!(proj.iter().all(|m| m.project == "/work/proj"));

    let all = storage.list_all_memories(10).expect("list all");
    assert_eq!(all.len(), 3);
    // Limit truncates.
    assert_eq!(storage.list_all_memories(2).expect("limited").len(), 2);

    assert_eq!(storage.count_memories().expect("count"), 3);
}

#[test]
fn project_scope_matches_full_path_and_basename() {
    let storage = storage();
    // Current format stores the full path; legacy rows stored the basename only.
    add(&storage, "full path row", "/work/myproj");
    add(&storage, "basename row", "myproj");

    let dir = Path::new("/work/myproj");
    assert_eq!(
        storage.count_memories_for_project(dir).expect("count"),
        2,
        "both the full-path and basename keys are matched"
    );

    let listed = storage.list_memories_for_project(dir, 10).expect("list");
    assert_eq!(listed.len(), 2);

    // A directory whose basename collides with nothing matches its full path.
    let other = Path::new("/work/elsewhere");
    assert_eq!(
        storage
            .count_memories_for_project(other)
            .expect("count other"),
        0
    );
    assert!(
        storage
            .list_memories_for_project(other, 10)
            .expect("list other")
            .is_empty()
    );
}

#[test]
fn project_scope_with_empty_basename_uses_full_path_only() {
    let storage = storage();
    add(&storage, "root row", "/");
    // `Path::new("/")` has no final component, so only the full-path key applies.
    let dir = Path::new("/");
    assert_eq!(storage.count_memories_for_project(dir).expect("count"), 1);
    assert_eq!(
        storage
            .list_memories_for_project(dir, 10)
            .expect("list")
            .len(),
        1
    );
}

#[test]
fn update_content_tags_confidence_and_access() {
    let storage = storage();
    let id = add(&storage, "original content", "/p");

    assert!(
        storage
            .update_memory_content(id, "rewritten content")
            .expect("content")
    );
    let row = storage.get_memory(id).expect("get").expect("row");
    assert_eq!(row.content, "rewritten content");

    storage
        .set_memory_tags(id, &["rust".to_string(), "sqlite".to_string()])
        .expect("set tags");
    let mut tags = storage.get_memory_tags(id).expect("tags");
    tags.sort();
    assert_eq!(tags, vec!["rust".to_string(), "sqlite".to_string()]);

    // Replacing clears the previous set.
    storage
        .set_memory_tags(id, &["only".to_string()])
        .expect("replace tags");
    assert_eq!(
        storage.get_memory_tags(id).expect("tags2"),
        vec!["only".to_string()]
    );

    assert!(
        storage
            .update_memory_confidence(id, 0.42)
            .expect("confidence")
    );
    assert!(
        (storage
            .get_memory(id)
            .expect("get")
            .expect("row")
            .confidence
            - 0.42)
            .abs()
            < 1e-9
    );

    assert!(storage.increment_memory_access(id).expect("access"));
    let row = storage.get_memory(id).expect("get").expect("row");
    assert_eq!(row.access_count, 1);
    assert!(row.last_accessed.is_some());

    // Updates against a missing id report false.
    assert!(
        !storage
            .update_memory_content(9999, "x")
            .expect("missing content")
    );
    assert!(
        !storage
            .update_memory_confidence(9999, 0.1)
            .expect("missing conf")
    );
    assert!(
        !storage
            .increment_memory_access(9999)
            .expect("missing access")
    );
}

#[test]
fn clear_memories_for_project_removes_only_that_project() {
    let storage = storage();
    add(&storage, "p1 memory", "/clear/target");
    add(&storage, "p2 memory", "/keep/other");
    // A legacy basename row for the same project.
    add(&storage, "legacy", "target");

    let removed = storage
        .clear_memories_for_project(Path::new("/clear/target"))
        .expect("clear");
    assert_eq!(removed, 2, "full-path and basename rows both removed");
    assert_eq!(storage.count_memories().expect("count"), 1);
    assert_eq!(
        storage
            .list_memories("/keep/other", 10)
            .expect("keep")
            .len(),
        1
    );
}

#[test]
fn get_all_memory_tags_batches_every_row() {
    let storage = storage();
    let a = add(&storage, "a", "/p");
    let b = add(&storage, "b", "/p");
    storage
        .set_memory_tags(a, &["x".to_string()])
        .expect("a tags");
    storage
        .set_memory_tags(b, &["y".to_string(), "z".to_string()])
        .expect("b tags");

    let all = storage.get_all_memory_tags().expect("all tags");
    assert_eq!(
        all.get(&a).map(Vec::as_slice),
        Some(["x".to_string()].as_slice())
    );
    assert_eq!(all.get(&b).map(Vec::len), Some(2));

    let batched = storage.get_memory_tags_batched(&[a]).expect("batched");
    assert_eq!(batched.len(), 1);
    assert!(batched.contains_key(&a));
    assert!(
        storage
            .get_memory_tags_batched(&[])
            .expect("empty")
            .is_empty()
    );
}
