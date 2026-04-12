//! Tests for concurrent_file_ops.rs

use anyhow::Result;
use std::path::PathBuf;

#[tokio::test]
async fn test_concurrent_reader_reads_files() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let a = dir.path().join("a.txt");
    let b = dir.path().join("b.txt");
    tokio::fs::write(&a, "hello a").await?;
    tokio::fs::write(&b, "hello b").await?;

    let paths = vec![a.clone(), b.clone()];
    let reader = ragent_core::file_ops::ConcurrentFileReader::new().with_concurrency(2);
    let results = reader.read_paths(paths).await?;
    let mut found: std::collections::HashMap<PathBuf, String> = std::collections::HashMap::new();
    for r in results {
        if let Some(c) = r.content {
            found.insert(r.path, c);
        }
    }

    assert_eq!(found.get(&a).map(|s| s.as_str()), Some("hello a"));
    assert_eq!(found.get(&b).map(|s| s.as_str()), Some("hello b"));
    Ok(())
}

#[tokio::test]
async fn test_edit_staging_dry_run() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let f = dir.path().join("file.txt");
    tokio::fs::write(&f, "original").await?;

    let mut staging = ragent_core::file_ops::EditStaging::new(true);
    staging.stage_edit(&f, "modified".to_string()).await?;
    let res = staging.commit_all(2).await?;

    // dry run should report applied paths but not write files
    assert!(res.errors.is_empty());
    assert!(res.conflicts.is_empty());
    assert_eq!(res.applied.len(), 1);

    let content = tokio::fs::read_to_string(&f).await?;
    assert_eq!(content, "original");
    Ok(())
}

#[tokio::test]
async fn test_edit_staging_conflict_detection() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let f = dir.path().join("conflict.txt");
    tokio::fs::write(&f, "orig").await?;

    let mut staging = ragent_core::file_ops::EditStaging::new(false);
    staging.stage_edit(&f, "ours".to_string()).await?;

    // Simulate external change before commit
    tokio::fs::write(&f, "theirs").await?;

    let res = staging.commit_all(2).await?;
    assert!(!res.conflicts.is_empty());
    assert!(res.applied.is_empty());
    Ok(())
}
