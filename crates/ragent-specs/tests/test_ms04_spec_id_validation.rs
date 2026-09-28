//! MS-04 (SECTASKS T-065) spec-id validation guards.

use ragent_specs::spec::SpecId;

#[test]
fn spec_id_new_rejects_traversal_shapes() {
    for bad in ["..", "../..", "/etc", "a/b", "", "a b", "a\\b"] {
        assert!(
            SpecId::new(bad).is_none(),
            "SpecId::new must reject {bad:?}"
        );
    }
}

#[test]
fn spec_id_new_accepts_slug_shapes() {
    for good in ["testspec", "auth-refactor", "a_b-1"] {
        assert!(
            SpecId::new(good).is_some(),
            "SpecId::new must accept {good:?}"
        );
    }
}

#[test]
fn spec_id_deserialization_routes_through_the_constructor() {
    let ok: Result<SpecId, _> = serde_json::from_str("\"valid-id\"");
    assert_eq!(ok.expect("valid id must deserialize").as_str(), "valid-id");

    let bad: Result<SpecId, _> = serde_json::from_str("\"../../x\"");
    assert!(bad.is_err(), "a traversal id must not deserialize");
}
