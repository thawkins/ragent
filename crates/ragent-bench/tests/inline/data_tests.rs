//! Inline tests for `data.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::{
    BcMbppFullRecord, BenchCaseFixture, BenchDataManifest, BenchDataSource, BenchInitMode,
    HumanEvalFullRecord, MANIFEST_VERSION, bc_mbpp_fixture_from_record, build_mbpp_test_code,
    canonical_mbpp_language, effective_init_mode_for_suite, humaneval_fixture_from_record,
    validate_case_fixtures,
};

#[test]
fn test_humaneval_full_record_preserves_hidden_tests() {
    let fixture = humaneval_fixture_from_record(HumanEvalFullRecord {
        task_id: "HumanEval/0".to_string(),
        prompt: "def add(a, b):\n".to_string(),
        canonical_solution: "    return a + b\n".to_string(),
        test: "def check(candidate):\n    assert candidate(1, 2) == 3\n".to_string(),
        entry_point: "add".to_string(),
    });

    assert_eq!(fixture.case_id, "HumanEval/0");
    assert_eq!(fixture.entry_point.as_deref(), Some("add"));
    assert!(
        fixture
            .test_code
            .as_deref()
            .is_some_and(|test| test.contains("check(candidate)"))
    );
}

#[test]
fn test_build_mbpp_test_code_preserves_setup_and_assertions() {
    let payload = build_mbpp_test_code(
        "import math",
        &["assert solve(1) == 2".to_string()],
        &["assert solve(2) == 3".to_string()],
    )
    .expect("mbpp test payload");

    assert!(payload.contains("import math"));
    assert!(payload.contains("assert solve(1) == 2"));
    assert!(payload.contains("assert solve(2) == 3"));
}

#[test]
fn test_bc_mbpp_language_aliases_resolve_to_cli_slugs() {
    assert_eq!(canonical_mbpp_language("Rust").expect("rust slug"), "rust");
    assert_eq!(canonical_mbpp_language("C++").expect("cpp slug"), "cpp");
    assert_eq!(
        canonical_mbpp_language("TypeScript").expect("typescript slug"),
        "typescript"
    );
}

#[test]
fn test_bc_mbpp_full_record_builds_native_fixture() {
    let fixture = bc_mbpp_fixture_from_record(BcMbppFullRecord {
        qid: "7".to_string(),
        language: "Rust".to_string(),
        extension: "rs".to_string(),
        commands: vec![
            vec!["rustc".to_string(), "__FILENAME__".to_string()],
            vec!["./__FILENAME__.exe".to_string()],
        ],
        timeouts: vec![10, 10],
        entry_cls_name: "Solution".to_string(),
        entry_fn_name: "answer".to_string(),
        signature_with_docstring: "pub fn answer() -> i32 {".to_string(),
        text: "Return the answer.".to_string(),
        test_code: "PLACEHOLDER_CODE_BODY\nfn main() { println!(\"TEST-0...PASSED\"); }"
            .to_string(),
        solution_python: "def answer():\n    return 42".to_string(),
    })
    .expect("native bc-mbpp fixture");

    assert_eq!(fixture.case_id, "mbpp-7");
    assert_eq!(fixture.language, "rust");
    assert_eq!(fixture.entry_point.as_deref(), Some("answer"));
    assert_eq!(fixture.source_extension.as_deref(), Some("rs"));
    assert_eq!(fixture.execution_commands.len(), 2);
    assert_eq!(fixture.reference, "");
    assert!(
        fixture
            .test_code
            .as_deref()
            .is_some_and(|tests| tests.contains("PLACEHOLDER_CODE_BODY"))
    );
}

#[test]
fn test_validate_case_fixtures_rejects_stale_mbpp_cases() {
    let manifest = BenchDataManifest {
        bench_name: "mbpp".to_string(),
        display_name: "MBPP".to_string(),
        language: "python".to_string(),
        revision: "BC-MBPP-1.0".to_string(),
        sources: vec![BenchDataSource {
            kind: "dataset".to_string(),
            url: "https://example.invalid/mbpp".to_string(),
        }],
        initialized_at_utc: "2026-05-02T00:00:00Z".to_string(),
        dataset_dir: "dataset".to_string(),
        case_file: "dataset/cases.jsonl".to_string(),
        case_count: 1,
        status: "ready".to_string(),
        manifest_version: MANIFEST_VERSION,
        files: vec![],
    };
    let cases = vec![BenchCaseFixture {
        case_id: "mbpp-1".to_string(),
        prompt: "Return True if a string is a palindrome.".to_string(),
        starter_code: None,
        reference: "def is_palindrome(s):\n    return s == s[::-1]".to_string(),
        language: "python".to_string(),
        test_code: None,
        entry_point: None,
        entry_class: None,
        execution_commands: Vec::new(),
        execution_timeouts_secs: Vec::new(),
        source_extension: None,
    }];

    let error = validate_case_fixtures("mbpp", &manifest, &cases)
        .expect_err("stale mbpp fixtures should be rejected");
    assert!(
        error
            .to_string()
            .contains("stale and missing bundled tests")
    );
}

#[test]
fn test_effective_init_mode_for_all_full_falls_back_for_unsupported_suites() {
    assert_eq!(
        effective_init_mode_for_suite("humaneval", BenchInitMode::Full),
        BenchInitMode::Full
    );
    assert_eq!(
        effective_init_mode_for_suite("mbpp", BenchInitMode::Full),
        BenchInitMode::Full
    );
    assert_eq!(
        effective_init_mode_for_suite("apps", BenchInitMode::Full),
        BenchInitMode::Sample
    );
    assert_eq!(
        effective_init_mode_for_suite("swebench-lite", BenchInitMode::Full),
        BenchInitMode::Sample
    );
}
