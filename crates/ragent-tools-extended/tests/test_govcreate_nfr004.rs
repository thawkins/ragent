//! NFR-004 timing test: acquiring a local folder of 100 supported documents
//! must complete well within the 30-second bound (excluding model latency).
//!
//! The bound in `SPEC.md` is a ceiling, not an expectation; on a developer
//! workstation `acquire_local` completes in well under a second for this
//! corpus. We assert < 5s so the test catches a regression (an accidental
//! O(n^2) walk, a per-file syscall storm, or similar) without being flaky.

use std::time::Instant;

use ragent_tools_extended::archdoc::{LocalAcquisitionBudget, acquire_local};

#[test]
fn test_nfr004_local_acquisition_of_100_files_is_within_budget() {
    let dir = tempfile::Builder::new()
        .prefix("govcreate-nfr004")
        .tempdir()
        .expect("tempdir");
    let src = dir.path().join("src");
    std::fs::create_dir(&src).expect("mkdir");
    for i in 0..100 {
        let content =
            format!("# Design doc {i}\n\nComponent {i} responsibilities and interfaces. ")
                .repeat(60);
        std::fs::write(src.join(format!("doc-{i:03}.md")), content).expect("write");
    }
    assert_eq!(
        std::fs::read_dir(&src).expect("read_dir").count(),
        100,
        "precondition: 100 files staged"
    );

    let started = Instant::now();
    let corpus = acquire_local(&src, &LocalAcquisitionBudget::default());
    let elapsed = started.elapsed();

    // 100 files at ~3.6KB each; the default char cap is 200KB so roughly
    // the first ~55 files alphabetically fill the corpus.
    assert!(
        corpus.sources.len() >= 50,
        "at least half the docs land in the corpus: got {}",
        corpus.sources.len()
    );
    assert!(
        elapsed.as_secs_f64() < 5.0,
        "acquiring 100 local docs must be well under the 30s NFR-004 bound \
         (excluding model latency); took {elapsed:?} for {} sources",
        corpus.sources.len()
    );
}
