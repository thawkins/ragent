//! Example demonstrating concurrent file reading and batch editing.
//!
//! This example shows how to use the [`ConcurrentFileReader`] to read multiple files
//! in parallel and [`apply_batch_edits`] to stage and commit changes.

use ragent_core::file_ops::{ConcurrentFileReader, apply_batch_edits};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Example: read a list of files, append a comment, and apply edits concurrently.
    let files: Vec<&str> = vec!["examples/parallel_edit.rs", "Cargo.toml"];

    let reader = ConcurrentFileReader::new().with_concurrency(4);
    let paths: Vec<PathBuf> = files.into_iter().map(PathBuf::from).collect();

    let reads = reader.read_paths(paths.clone()).await?;
    let mut pairs = Vec::new();
    for r in reads {
        if let Some(content) = r.content {
            let new_content = format!("{content}\n// Edited by parallel_edit example\n");
            pairs.push((r.path, new_content));
        }
    }

    let res = apply_batch_edits(pairs, 4, true).await?; // dry-run first
    println!(
        "Dry-run result: applied={}, conflicts={}, errors={}",
        res.applied.len(),
        res.conflicts.len(),
        res.errors.len()
    );

    // To actually write changes, call with dry_run=false
    // let res2 = apply_batch_edits(pairs, 4, false).await?;
    // println!("Applied: {}", res2.applied.len());

    Ok(())
}
