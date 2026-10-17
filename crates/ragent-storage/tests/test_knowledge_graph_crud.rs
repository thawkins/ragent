//! Isolation coverage for the knowledge-graph CRUD
//! (`upsert_entity`, `create_relationship`, `list_entities`,
//! `list_relationships`, `query_entity_neighbours`).
//!
//! The KG stores entities (unique by name+type) and typed relationships between
//! them. These tests pin the mention-count increment on re-upsert, the
//! confidence-merge (`MAX`) on a repeated relationship, the descending orderings,
//! and the 1-hop neighbour expansion.

use ragent_storage::storage::Storage;

fn storage() -> Storage {
    Storage::open_in_memory().expect("storage")
}

#[test]
fn upsert_entity_creates_then_increments_mention_count() {
    let storage = storage();
    let id = storage
        .upsert_entity("Rust", "language", 1)
        .expect("first upsert");
    let again = storage
        .upsert_entity("Rust", "language", 1)
        .expect("second upsert");
    assert_eq!(id, again, "same name+type resolves to the same row");

    // A different type is a distinct entity.
    let other = storage
        .upsert_entity("Rust", "project", 2)
        .expect("other type");
    assert_ne!(id, other);

    let entities = storage.list_entities().expect("list");
    assert_eq!(entities.len(), 2);
    let rust_lang = entities
        .iter()
        .find(|e| e.name == "Rust" && e.entity_type == "language")
        .expect("present");
    assert_eq!(rust_lang.mention_count, 2, "mention count incremented");
    assert_ne!(rust_lang.created_at, "");
    assert_ne!(rust_lang.updated_at, "");
}

#[test]
fn relationships_merge_confidence_and_are_listed_by_confidence() {
    let storage = storage();
    let a = storage.upsert_entity("A", "concept", 1).expect("a");
    let b = storage.upsert_entity("B", "concept", 2).expect("b");

    let r1 = storage
        .create_relationship(a, b, "uses", 0.4, Some(2))
        .expect("rel 1");
    // A repeated relationship raises the confidence to the max.
    let _ = storage
        .create_relationship(a, b, "uses", 0.9, Some(2))
        .expect("rel 2");
    // A distinct relation type is a separate row.
    let _ = storage
        .create_relationship(b, a, "depends_on", 0.7, None)
        .expect("rel 3");

    let rels = storage.list_relationships().expect("list");
    assert_eq!(rels.len(), 2, "same (src,tgt,type) collapses to one row");

    let uses = rels
        .iter()
        .find(|r| r.relation_type == "uses")
        .expect("uses rel");
    assert_eq!(uses.id, r1, "the original row id is preserved on merge");
    assert!((uses.confidence - 0.9).abs() < f64::EPSILON);
    assert_eq!(uses.source_memory_id, Some(2));

    // Ordered by confidence descending: uses (0.9) before depends_on (0.7).
    assert_eq!(rels[0].relation_type, "uses");
    assert_eq!(rels[1].relation_type, "depends_on");
}

#[test]
fn query_entity_neighbours_returns_self_and_connected() {
    let storage = storage();
    let a = storage.upsert_entity("A", "concept", 1).expect("a");
    let b = storage.upsert_entity("B", "concept", 1).expect("b");
    let c = storage.upsert_entity("C", "concept", 1).expect("c");
    let d = storage.upsert_entity("D", "concept", 1).expect("d");

    // A -> B, C -> A; D is disconnected.
    storage
        .create_relationship(a, b, "uses", 0.8, None)
        .expect("a->b");
    storage
        .create_relationship(c, a, "related_to", 0.5, None)
        .expect("c->a");

    let (entities, relationships) = storage.query_entity_neighbours(a).expect("neighbours");
    let ids: Vec<i64> = entities.iter().map(|e| e.id).collect();
    assert!(ids.contains(&a) && ids.contains(&b) && ids.contains(&c));
    assert!(!ids.contains(&d), "disconnected entity excluded");
    assert_eq!(relationships.len(), 2, "both touching edges returned");

    // A leaf with no edges returns only itself.
    let (only_d, no_rels) = storage.query_entity_neighbours(d).expect("leaf");
    assert_eq!(only_d.len(), 1);
    assert_eq!(only_d[0].id, d);
    assert!(no_rels.is_empty());
}

#[test]
fn list_entities_orders_by_mention_count_descending() {
    let storage = storage();
    storage.upsert_entity("low", "concept", 1).expect("low");
    let high = storage.upsert_entity("high", "concept", 1).expect("high");
    // Bump `high` three more times.
    for _ in 0..3 {
        storage.upsert_entity("high", "concept", 1).expect("bump");
    }
    let entities = storage.list_entities().expect("list");
    assert_eq!(entities[0].id, high);
    assert_eq!(entities[0].mention_count, 4);
}
