use anyhow::Result;
use std::path::PathBuf;

#[tokio::test]
async fn test_stress_many_files_commit() -> Result<()> {
    // Create a temporary directory with many small files to simulate a bulk edit.
    let dir = tempfile::tempdir()?;
    let mut paths: Vec<PathBuf> = Vec::new();
    let count = 200; // stress level for CI-friendly run

    for i in 0..count {
        let p = dir.path().join(format!("file_{:03}.txt", i));
        let content = format!("original {}", i);
        tokio::fs::write(&p, content).await?;
        paths.push(p);
    }

    // Stage edits for all files
    let mut staging = ragent_core::file_ops::EditStaging::new(false);
    for p in &paths {
        let new = format!("modified {}", p.file_name().unwrap().to_string_lossy());
        staging.stage_edit(p, new).await?;
    }

    // Commit with reasonable concurrency
    let res = staging.commit_all(8).await?;

    // Expect all applied with no conflicts/errors
    assert!(res.errors.is_empty(), "errors: {:?}", res.errors);
    assert!(res.conflicts.is_empty(), "conflicts: {:?}", res.conflicts);
    assert_eq!(res.applied.len(), paths.len());

    // Validate contents on disk
    for p in &paths {
        let got = tokio::fs::read_to_string(p).await?;
        assert!(
            got.starts_with("modified"),
            "unexpected content for {:?}: {}",
            p,
            got
        );
    }

    Ok(())
}
