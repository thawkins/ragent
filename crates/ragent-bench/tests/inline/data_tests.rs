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

use super::{
    HumanevalPackRecord, build_humanevalpack_test_code, canonical_humanevalpack_language, fixture,
    humanevalpack_commands_for_language, humanevalpack_entry_class_for_language,
    humanevalpack_fixture_from_record, init_apps, init_bigcodebench, init_crosscodeeval,
    init_ds1000, init_livecodebench, init_multipl_e, init_repobench, init_swebench_lite,
    init_swebench_verified, require_fixture_language, rewrite_humanevalpack_go_test,
    rewrite_humanevalpack_go_test_setup,
};

fn manifest_for(files: Vec<&str>) -> BenchDataManifest {
    BenchDataManifest {
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
        files: files
            .into_iter()
            .map(|relative_path| super::BenchDataFile {
                relative_path: relative_path.to_string(),
                sha256: "x".to_string(),
                bytes: 1,
            })
            .collect(),
    }
}

fn safe_case(case_id: &str, extension: Option<&str>) -> super::BenchCaseFixture {
    super::BenchCaseFixture {
        case_id: case_id.to_string(),
        prompt: "p".to_string(),
        starter_code: None,
        reference: "r".to_string(),
        language: "python".to_string(),
        test_code: Some("assert True".to_string()),
        entry_point: None,
        entry_class: None,
        execution_commands: Vec::new(),
        execution_timeouts_secs: Vec::new(),
        source_extension: extension.map(str::to_string),
    }
}

#[test]
fn test_build_mbpp_test_code_returns_none_when_all_sections_blank() {
    assert_eq!(build_mbpp_test_code("   ", &[], &[]), None);
    assert_eq!(
        build_mbpp_test_code("", &["  ".to_string()], &[String::new()]),
        None
    );
}

#[test]
fn test_build_mbpp_test_code_includes_challenge_and_trims() {
    let payload = build_mbpp_test_code(
        "  setup  ",
        &["  assert a  ".to_string()],
        &["assert b".to_string()],
    )
    .expect("payload");
    assert_eq!(payload, "setup\nassert a\nassert b");
}

#[test]
fn test_canonical_humanevalpack_language_all_aliases_and_error() {
    assert_eq!(
        canonical_humanevalpack_language("cpp").expect("cpp"),
        ("cpp", "cpp")
    );
    assert_eq!(
        canonical_humanevalpack_language("Go").expect("go"),
        ("go", "go")
    );
    assert_eq!(
        canonical_humanevalpack_language("Java").expect("java"),
        ("java", "java")
    );
    assert_eq!(
        canonical_humanevalpack_language("js").expect("js"),
        ("javascript", "js")
    );
    assert_eq!(
        canonical_humanevalpack_language("Python").expect("python"),
        ("python", "python")
    );
    assert_eq!(
        canonical_humanevalpack_language("Rust").expect("rust"),
        ("rust", "rust")
    );
    assert!(
        canonical_humanevalpack_language("cobol")
            .expect_err("unsupported")
            .to_string()
            .contains("unsupported HumanEvalPack language")
    );
}

#[test]
fn test_humanevalpack_commands_cover_every_language() {
    let (cpp, ext) = humanevalpack_commands_for_language("cpp");
    assert_eq!(ext, "cpp");
    assert_eq!(cpp.len(), 2);
    assert_eq!(humanevalpack_commands_for_language("go").1, "go");
    assert_eq!(humanevalpack_commands_for_language("java").1, "java");
    assert_eq!(humanevalpack_commands_for_language("javascript").1, "js");
    let (python, py_ext) = humanevalpack_commands_for_language("python");
    assert_eq!(python, [] as [std::vec::Vec<std::string::String>; 0]);
    assert_eq!(py_ext, "py");
    assert_eq!(humanevalpack_commands_for_language("rust").1, "rs");
    assert_eq!(humanevalpack_commands_for_language("unknown").1, "txt");
    assert_eq!(
        humanevalpack_commands_for_language("unknown").0,
        [] as [std::vec::Vec<std::string::String>; 0]
    );
}

#[test]
fn test_humanevalpack_entry_class_only_for_java() {
    assert_eq!(
        humanevalpack_entry_class_for_language("java").as_deref(),
        Some("Solution")
    );
    assert!(humanevalpack_entry_class_for_language("rust").is_none());
}

#[test]
fn test_build_humanevalpack_test_code_python_and_native_paths() {
    let python = build_humanevalpack_test_code("python", "import x", "setup", "check()")
        .expect("python payload");
    assert_eq!(python, "import x\n\nsetup\n\ncheck()");

    let empty =
        build_humanevalpack_test_code("python", "", "", "").expect_err("empty python payload");
    assert!(empty.to_string().contains("was empty"));

    let cpp =
        build_humanevalpack_test_code("cpp", "#include", "", "test_body").expect("cpp payload");
    assert!(cpp.contains("PLACEHOLDER_CODE_BODY"));
    assert!(cpp.contains("test_body"));
}

#[test]
fn test_humanevalpack_go_rewrites_drop_testify_and_rename_assert() {
    let setup = rewrite_humanevalpack_go_test_setup(
        "\"github.com/stretchr/testify/assert\"\nimport (\n    \"testing\"\n)\n",
    );
    assert!(!setup.contains("stretchr/testify/assert"));
    assert!(setup.contains("func assertEqual("));
    assert!(setup.contains("\"reflect\""));

    let rewritten = rewrite_humanevalpack_go_test("assert := assert.New(t)\nassert.Equal(a, b)\n");
    assert!(!rewritten.contains("assert := assert.New(t)"));
    assert!(rewritten.contains("assertEqual(t, a, b)"));
    let crlf = rewrite_humanevalpack_go_test("assert := assert.New(t)\r\n");
    assert!(!crlf.contains("assert.New"));
}

#[test]
fn test_humanevalpack_fixture_from_record_prefers_prompt_then_declaration() {
    let record = HumanevalPackRecord {
        task_id: "HumanEvalPack/1".to_string(),
        prompt: "  ".to_string(),
        declaration: "def entry():".to_string(),
        canonical_solution: "return 1".to_string(),
        entry_point: "entry".to_string(),
        import: "import math".to_string(),
        test_setup: String::new(),
        test: "check(entry)".to_string(),
        instruction: "solve it".to_string(),
    };
    let fixture = humanevalpack_fixture_from_record("python", record).expect("fixture");
    assert_eq!(fixture.case_id, "HumanEvalPack/1");
    assert_eq!(fixture.starter_code.as_deref(), Some("def entry():"));
    assert_eq!(fixture.entry_point.as_deref(), Some("entry"));
    assert_eq!(fixture.source_extension.as_deref(), Some("py"));
    assert_eq!(
        fixture.execution_commands,
        [] as [std::vec::Vec<std::string::String>; 0]
    );
    assert_eq!(fixture.prompt, "solve it");
}

#[test]
fn test_humanevalpack_fixture_java_sets_entry_class_and_timeouts() {
    let record = HumanevalPackRecord {
        task_id: "HumanEvalPack/2".to_string(),
        prompt: "class Solution {}".to_string(),
        declaration: "unused".to_string(),
        canonical_solution: "return".to_string(),
        entry_point: "entry".to_string(),
        import: "import java.util.*;".to_string(),
        test_setup: "setup".to_string(),
        test: "test".to_string(),
        instruction: "instruction".to_string(),
    };
    let fixture = humanevalpack_fixture_from_record("java", record).expect("fixture");
    assert_eq!(fixture.entry_class.as_deref(), Some("Solution"));
    assert_eq!(fixture.execution_commands.len(), 2);
    assert_eq!(fixture.execution_timeouts_secs.len(), 2);
    assert_eq!(
        fixture.execution_timeouts_secs,
        vec![crate::exec_guard::FIXTURE_DEFAULT_TIMEOUT_SECS; 2]
    );
}

#[test]
fn test_canonical_mbpp_language_covers_all_and_errors() {
    for (input, expected) in [
        ("Python", "python"),
        ("C++", "cpp"),
        ("CSharp", "csharp"),
        ("Dart", "dart"),
        ("Go", "go"),
        ("Haskell", "haskell"),
        ("Java", "java"),
        ("Javascript", "javascript"),
        ("Julia", "julia"),
        ("Kotlin", "kotlin"),
        ("Lua", "lua"),
        ("PHP", "php"),
        ("R", "r"),
        ("Rust", "rust"),
        ("Scala", "scala"),
        ("TypeScript", "typescript"),
    ] {
        assert_eq!(canonical_mbpp_language(input).expect(input), expected);
    }
    assert!(
        canonical_mbpp_language("cobol")
            .expect_err("unsupported")
            .to_string()
            .contains("unsupported BC-MBPP language")
    );
}

#[test]
fn test_bc_mbpp_fixture_keeps_python_solution_but_blanks_other_languages() {
    let python = bc_mbpp_fixture_from_record(BcMbppFullRecord {
        qid: "9".to_string(),
        language: "Python".to_string(),
        extension: "py".to_string(),
        commands: Vec::new(),
        timeouts: Vec::new(),
        entry_cls_name: String::new(),
        entry_fn_name: "answer".to_string(),
        signature_with_docstring: String::new(),
        text: "Return the answer.".to_string(),
        test_code: "assert answer() == 42".to_string(),
        solution_python: "def answer():\n    return 42".to_string(),
    })
    .expect("python fixture");
    assert_eq!(python.prompt, "Return the answer.");
    assert_eq!(python.reference, "def answer():\n    return 42");
    assert!(python.entry_class.is_none());
    assert_eq!(python.entry_point.as_deref(), Some("answer"));
    assert_eq!(python.source_extension.as_deref(), Some("py"));

    let rust = bc_mbpp_fixture_from_record(BcMbppFullRecord {
        qid: "10".to_string(),
        language: "Rust".to_string(),
        extension: "rs".to_string(),
        commands: Vec::new(),
        timeouts: Vec::new(),
        entry_cls_name: "Solution".to_string(),
        entry_fn_name: String::new(),
        signature_with_docstring: "pub fn answer() -> i32 {".to_string(),
        text: "fallback".to_string(),
        test_code: "test".to_string(),
        solution_python: "ignored".to_string(),
    })
    .expect("rust fixture");
    assert_eq!(rust.reference, "");
    assert_eq!(rust.entry_class.as_deref(), Some("Solution"));
    assert!(rust.entry_point.is_none());
    assert_eq!(rust.prompt, "pub fn answer() -> i32 {");
}

#[test]
fn test_validate_case_fixtures_accepts_safe_and_rejects_unsafe_extension() {
    let manifest = manifest_for(vec!["dataset/cases.jsonl"]);
    let good = vec![safe_case("mbpp-1", Some("rs"))];
    validate_case_fixtures("mbpp", &manifest, &good).expect("safe extension accepted");

    let bad = vec![safe_case("mbpp-2", Some("../etc/passwd"))];
    let error =
        validate_case_fixtures("mbpp", &manifest, &bad).expect_err("unsafe extension rejected");
    assert!(error.to_string().contains("unsafe source extension"));
}

#[test]
fn test_validate_case_fixtures_stale_hint_names_full_rebuild_when_raw_present() {
    let manifest = manifest_for(vec!["dataset/raw/mbpp.jsonl"]);
    let stale = vec![super::BenchCaseFixture {
        test_code: None,
        ..safe_case("mbpp-1", None)
    }];
    let error =
        validate_case_fixtures("mbpp", &manifest, &stale).expect_err("stale fixtures rejected");
    assert!(
        error
            .to_string()
            .contains("/bench init mbpp --full --force-download")
    );
}

#[test]
fn test_require_fixture_language_accepts_match_and_rejects_mismatch() {
    assert_eq!(
        require_fixture_language("apps", "python", "python").expect("match"),
        "python"
    );
    let error = require_fixture_language("apps", "rust", "python").expect_err("mismatch");
    assert!(error.to_string().contains("does not provide init fixtures"));
}

#[test]
fn test_sample_fixture_builders_return_one_case_each() {
    let builders: Vec<(
        &str,
        Box<dyn Fn() -> anyhow::Result<Vec<super::BenchCaseFixture>>>,
    )> = vec![
        ("apps", Box::new(|| init_apps("python"))),
        ("ds1000", Box::new(|| init_ds1000("python"))),
        ("multipl-e-python", Box::new(|| init_multipl_e("python"))),
        ("multipl-e-rust", Box::new(|| init_multipl_e("rust"))),
        ("repobench", Box::new(|| init_repobench("python"))),
        ("crosscodeeval", Box::new(|| init_crosscodeeval("python"))),
        ("swebench-lite", Box::new(|| init_swebench_lite("diff"))),
        (
            "swebench-verified",
            Box::new(|| init_swebench_verified("diff")),
        ),
        ("livecodebench", Box::new(|| init_livecodebench("python"))),
        ("bigcodebench", Box::new(|| init_bigcodebench("python"))),
    ];
    for (label, build) in builders {
        let fixtures = build().unwrap_or_else(|error| panic!("{label}: {error}"));
        assert_eq!(fixtures.len(), 1, "{label} should produce one fixture");
        assert!(!fixtures[0].case_id.is_empty(), "{label} case id");
    }

    // Unsupported language path.
    assert!(init_multipl_e("cobol").is_err());
}

#[test]
fn test_fixture_helper_populates_expected_fields() {
    let fixtures = fixture("id-1", "prompt", "reference", "python");
    assert_eq!(fixtures.len(), 1);
    let case = &fixtures[0];
    assert_eq!(case.case_id, "id-1");
    assert_eq!(case.prompt, "prompt");
    assert_eq!(case.reference, "reference");
    assert_eq!(case.language, "python");
    assert!(case.starter_code.is_none());
    assert!(case.test_code.is_none());
}

use super::{
    BenchDataFile, contained_join, download_url_bytes_cached,
    ensure_full_init_supported_for_target, manifest_file_for_path, prepare_suite_dataset,
    sha256_file, suite_fixtures, supports_full_init, verify_manifest_file, write_cases,
    write_manifest,
};
use crate::command::BenchInitTarget;
use std::path::PathBuf;

fn tmp_root(tag: &str) -> PathBuf {
    let dir = std::env::current_dir()
        .expect("cwd")
        .join("target")
        .join("temp")
        .join(format!(
            "inline-data-{tag}-{}",
            uuid::Uuid::new_v4().simple()
        ));
    std::fs::create_dir_all(&dir).expect("create tmp root");
    dir
}

#[test]
fn test_supports_full_init_only_for_humaneval_and_mbpp() {
    assert!(supports_full_init("humaneval"));
    assert!(supports_full_init("mbpp"));
    assert!(!supports_full_init("apps"));
    assert!(!supports_full_init("swebench-lite"));
}

#[test]
fn test_ensure_full_init_supported_ok_for_sample_and_errors_for_full() {
    // Sample mode / non-Full target is a no-op.
    ensure_full_init_supported_for_target(&BenchInitTarget::All, BenchInitMode::Sample)
        .expect("sample mode is fine");

    // Full target bails naming the missing suites.
    let error = ensure_full_init_supported_for_target(&BenchInitTarget::Full, BenchInitMode::Full)
        .expect_err("full target gated");
    assert!(error.to_string().contains("full"));
    assert!(error.to_string().contains("apps"));

    // Mode==Full on a Suite target is also gated.
    let suite_error = ensure_full_init_supported_for_target(
        &BenchInitTarget::Suite("apps".to_string()),
        BenchInitMode::Full,
    )
    .expect_err("suite full gated");
    assert!(suite_error.to_string().contains("full dataset ingestion"));
}

#[test]
fn test_suite_fixtures_routes_known_suites_and_rejects_unknown() {
    assert_eq!(
        suite_fixtures("humaneval", "python")
            .expect("humaneval")
            .len(),
        1
    );
    assert_eq!(suite_fixtures("mbpp", "python").expect("mbpp").len(), 1);
    assert_eq!(suite_fixtures("apps", "python").expect("apps").len(), 1);
    assert_eq!(suite_fixtures("ds1000", "python").expect("ds1000").len(), 1);
    assert_eq!(
        suite_fixtures("multipl-e", "rust")
            .expect("multipl-e")
            .len(),
        1
    );
    assert_eq!(
        suite_fixtures("repobench", "python")
            .expect("repobench")
            .len(),
        1
    );
    assert_eq!(
        suite_fixtures("crosscodeeval", "python")
            .expect("crosscodeeval")
            .len(),
        1
    );
    assert_eq!(
        suite_fixtures("swebench-lite", "diff")
            .expect("swebench-lite")
            .len(),
        1
    );
    assert_eq!(
        suite_fixtures("swebench-verified", "diff")
            .expect("swebench-verified")
            .len(),
        1
    );
    assert_eq!(
        suite_fixtures("livecodebench", "python")
            .expect("lcb")
            .len(),
        1
    );
    assert_eq!(
        suite_fixtures("bigcodebench", "python").expect("bcb").len(),
        1
    );

    assert!(
        suite_fixtures("nope", "python")
            .expect_err("unknown suite")
            .to_string()
            .contains("unknown benchmark suite")
    );
}

#[test]
fn test_prepare_suite_dataset_sample_mode_produces_fixtures_without_tracked_files() {
    let root = tmp_root("prepare-sample");
    let prepared =
        prepare_suite_dataset(&root, "humaneval", "python", BenchInitMode::Sample, false)
            .expect("prepare sample");
    assert_eq!(prepared.fixtures.len(), 1);
    assert_eq!(prepared.tracked_paths, [] as [std::path::PathBuf; 0]);
}

#[test]
fn test_manifest_file_for_path_hashes_and_sizes_a_file() {
    let root = tmp_root("manifest-file");
    let relative = std::path::Path::new("dataset/cases.jsonl");
    std::fs::create_dir_all(root.join("dataset")).expect("mkdir");
    std::fs::write(root.join(relative), "hello\n").expect("write");

    let file = manifest_file_for_path(&root, relative).expect("manifest file");
    assert_eq!(file.relative_path, "dataset/cases.jsonl");
    assert_eq!(file.bytes, 6);
    assert_eq!(file.sha256.len(), 64);

    // A missing file errors with context.
    let missing =
        manifest_file_for_path(&root, std::path::Path::new("nope.txt")).expect_err("missing file");
    assert!(missing.to_string().contains("stat benchmark file"));
}

#[test]
fn test_sha256_file_matches_a_known_digest() {
    let root = tmp_root("sha256");
    let path = root.join("data.bin");
    std::fs::write(&path, b"hello\n").expect("write");
    // sha256("hello\n")
    assert_eq!(
        sha256_file(&path).expect("sha"),
        "5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03"
    );
    assert!(
        sha256_file(&root.join("missing"))
            .expect_err("missing")
            .to_string()
            .contains("read benchmark file")
    );
}

#[test]
fn test_verify_manifest_file_accepts_matching_and_rejects_tampering() {
    let root = tmp_root("verify-file");
    let relative = "dataset/cases.jsonl";
    std::fs::create_dir_all(root.join("dataset")).expect("mkdir");
    std::fs::write(root.join(relative), "hello\n").expect("write");
    let file =
        manifest_file_for_path(&root, std::path::Path::new(relative)).expect("file metadata");
    verify_manifest_file(&root, &file).expect("matching file verifies");

    // Size mismatch.
    let wrong_size = BenchDataFile {
        bytes: file.bytes + 1,
        ..file.clone()
    };
    assert!(
        verify_manifest_file(&root, &wrong_size)
            .expect_err("size mismatch")
            .to_string()
            .contains("size mismatch")
    );

    // Checksum mismatch.
    let wrong_digest = BenchDataFile {
        sha256: "0".repeat(64),
        ..file
    };
    assert!(
        verify_manifest_file(&root, &wrong_digest)
            .expect_err("checksum mismatch")
            .to_string()
            .contains("checksum mismatch")
    );
}

#[test]
fn test_write_then_load_manifest_and_cases_roundtrip() {
    let root = tmp_root("write-roundtrip");
    let manifest = manifest_for(vec!["dataset/cases.jsonl"]);
    write_manifest(&root, &manifest).expect("write manifest");
    let loaded = super::load_manifest(&root).expect("load manifest");
    assert_eq!(loaded, manifest);

    let case = safe_case("c-1", Some("py"));
    std::fs::create_dir_all(root.join("dataset")).expect("mkdir dataset");
    write_cases(
        &root,
        std::path::Path::new("dataset/cases.jsonl"),
        std::slice::from_ref(&case),
    )
    .expect("write cases");
    let cases = super::load_cases(&root).expect("load cases");
    assert_eq!(cases, vec![case]);

    // Writing cases into a missing directory errors with context.
    assert!(
        write_cases(&root, std::path::Path::new("missing/cases.jsonl"), &[])
            .expect_err("missing dir")
            .to_string()
            .contains("create case file")
    );
}

#[test]
fn test_contained_join_rejects_escapes_and_allows_relative() {
    let root = tmp_root("contained-join");
    let ok = contained_join(&root, "dataset/cases.jsonl").expect("relative ok");
    assert!(ok.starts_with(&root));
    assert!(
        contained_join(&root, "../escape")
            .expect_err("dotdot rejected")
            .to_string()
            .contains("benchmark manifest path")
    );
    assert!(contained_join(&root, "/abs/path").is_err());
    assert!(contained_join(&root, "").is_err());
}

#[test]
fn test_download_url_bytes_cached_reuses_existing_cache_file() {
    let cache_dir = std::env::current_dir()
        .expect("cwd")
        .join("target")
        .join("temp")
        .join("bench-downloads");
    std::fs::create_dir_all(&cache_dir).expect("create cache dir");
    let cache_name = format!("unit-cache-{}.bin", uuid::Uuid::new_v4().simple());
    let cache_path = cache_dir.join(&cache_name);
    std::fs::write(&cache_path, b"cached-bytes").expect("seed cache");

    // A cache hit must return the seeded bytes without any network access.
    let bytes = download_url_bytes_cached("https://example.invalid/never", &cache_name, false)
        .expect("cache hit");
    assert_eq!(bytes, b"cached-bytes");
    let _ = std::fs::remove_file(&cache_path);
}
