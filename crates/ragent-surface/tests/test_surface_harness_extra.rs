//! Additional coverage for `ragent-surface::harness` (the schema sample
//! generator's top-level fallbacks and `schema_type` tolerance) and for the
//! shared `ragent-surface::scan::ScanFailure` row.

use serde_json::json;

use ragent_surface::harness::{sample_for_schema, schema_type, truncate};
use ragent_surface::scan::ScanFailure;

#[test]
fn truncate_respects_a_multibyte_char_boundary() {
    // 5 chars, requested max of 3: the ellipsis is appended after the char,
    // not a byte, boundary.
    assert_eq!(truncate("abcde", 3), "abc...");
    assert_eq!(truncate("aé", 1), "a...");
    // Exactly at the limit is untouched.
    assert_eq!(truncate("abc", 3), "abc");
    // A zero limit truncates everything visible.
    assert_eq!(truncate("abc", 0), "...");
}

#[test]
fn sample_for_schema_defaults_for_a_top_level_non_object_value() {
    // A top-level JSON boolean/number/string/array/null (not a schema object)
    // degrades to a typed zero sample.
    assert_eq!(sample_for_schema(&json!(true)), json!(false));
    assert_eq!(sample_for_schema(&json!(7)), json!(0));
    assert_eq!(sample_for_schema(&json!("hello")), json!("sample"));
    assert_eq!(sample_for_schema(&json!([1, 2, 3])), json!([]));
    assert_eq!(sample_for_schema(&json!(null)), json!(null));
}

#[test]
fn sample_for_schema_honours_examples_before_enum() {
    assert_eq!(
        sample_for_schema(&json!({"type": "string", "examples": ["first", "second"]})),
        json!("first")
    );
    // An empty examples array falls through to the typed default.
    assert_eq!(
        sample_for_schema(&json!({"type": "string", "examples": []})),
        json!("sample")
    );
}

#[test]
fn sample_for_schema_degrades_an_unknown_type_to_null() {
    assert_eq!(
        sample_for_schema(&json!({"type": "totally-unknown"})),
        json!(null)
    );
    // Untyped object with no properties yields an empty object.
    assert_eq!(sample_for_schema(&json!({"properties": {}})), json!({}));
}

#[test]
fn schema_type_tolerates_array_and_absent_forms() {
    let string_form = json!({"type": "integer"});
    assert_eq!(
        schema_type(string_form.as_object().expect("obj")),
        "integer"
    );

    let array_form = json!({"type": ["string", "null"]});
    assert_eq!(schema_type(array_form.as_object().expect("obj")), "string");

    // An array with no string entries falls back to "object".
    let no_strings = json!({"type": [1, 2]});
    assert_eq!(schema_type(no_strings.as_object().expect("obj")), "object");

    let absent = json!({});
    assert_eq!(schema_type(absent.as_object().expect("obj")), "object");

    let non_string_non_array = json!({"type": 42});
    assert_eq!(
        schema_type(non_string_non_array.as_object().expect("obj")),
        "object"
    );
}

#[test]
fn scan_failure_rows_carry_the_crate_own_error() {
    let failure = ScanFailure {
        error: "descriptor missing".to_string(),
    };
    assert_eq!(failure.error, "descriptor missing");

    let typed = ScanFailure { error: 42_u32 };
    assert_eq!(typed.error, 42);
}
