use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    pub files: HashMap<PathBuf, Vec<u8>>,
    pub created_at: DateTime<Utc>,
}

/// Take a snapshot of the specified files by reading them into memory.
pub fn take_snapshot(
    session_id: &str,
    message_id: &str,
    files: &[PathBuf],
) -> Result<Snapshot> {
    let mut file_contents = HashMap::new();

    for path in files {
        if path.exists() && path.is_file() {
            let content = std::fs::read(path)?;
            file_contents.insert(path.clone(), content);
        }
    }

    Ok(Snapshot {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        message_id: message_id.to_string(),
        files: file_contents,
        created_at: Utc::now(),
    })
}

/// Restore a snapshot by writing all files back to disk.
pub fn restore_snapshot(snapshot: &Snapshot) -> Result<()> {
    for (path, content) in &snapshot.files {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_snapshot_roundtrip() {
        let dir = std::env::temp_dir().join("ragent_snapshot_test");
        std::fs::create_dir_all(&dir).unwrap();

        let file_path = dir.join("test.txt");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(b"hello snapshot").unwrap();

        let snapshot =
            take_snapshot("session1", "msg1", &[file_path.clone()]).unwrap();
        assert_eq!(snapshot.files.len(), 1);
        assert_eq!(
            snapshot.files.get(&file_path).unwrap(),
            b"hello snapshot"
        );

        // Modify file
        std::fs::write(&file_path, "modified").unwrap();

        // Restore
        restore_snapshot(&snapshot).unwrap();
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello snapshot");

        // Cleanup
        std::fs::remove_dir_all(&dir).ok();
    }
}
