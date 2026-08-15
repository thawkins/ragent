//! Portable session archive export (spec `piegap` FR-006).
//!
//! This module provides functionality to export a session into a portable
//! archive file containing:
//! - A manifest with SHA-256 checksums of all files
//! - The session transcript (messages)
//! - Session-scoped automation sidecars (trigger rules, cron jobs, loop state)
//!
//! The archive is a `.tar.gz` file that can be imported back into ragent
//! or inspected manually.

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::message;
use crate::trigger::dynamic::DynamicTriggerEngine;
use ragent_storage::storage::{CronEventRow, Storage};
use ragent_types::cron::{CronEvent, CronSchedule};
use ragent_types::trigger::TriggerRule;

/// Manifest version for the archive format.
const MANIFEST_VERSION: u32 = 1;

/// Archive manifest structure.
///
/// Contains metadata about the archive and SHA-256 checksums for all
/// included files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveManifest {
    /// Format version for the manifest.
    pub manifest_version: u32,
    /// Session ID that was exported.
    pub session_id: String,
    /// Session title.
    pub session_title: String,
    /// Session working directory.
    pub session_directory: String,
    /// Timestamp when the archive was created (ISO-8601).
    pub created_at: String,
    /// Number of messages in the transcript.
    pub message_count: usize,
    /// Number of trigger rules exported.
    pub trigger_count: usize,
    /// Number of cron jobs exported.
    pub cron_job_count: usize,
    /// Number of loop state files exported.
    pub loop_state_count: usize,
    /// SHA-256 checksums of all files in the archive.
    pub files: HashMap<String, String>,
    /// Sensitivity warning about transcript content.
    pub sensitivity_warning: String,
}

/// Session archive export configuration.
#[derive(Debug, Clone)]
pub struct ArchiveConfig {
    /// Include trigger rules in the export.
    pub include_triggers: bool,
    /// Include cron jobs in the export.
    pub include_cron: bool,
    /// Include loop state files in the export.
    pub include_loop_state: bool,
    /// Include run-cost summaries in the export.
    pub include_cost: bool,
}

impl Default for ArchiveConfig {
    fn default() -> Self {
        Self {
            include_triggers: true,
            include_cron: true,
            include_loop_state: true,
            include_cost: false,
        }
    }
}

/// Export a session to a portable archive file.
///
/// Creates a `.tar.gz` archive containing:
/// - `manifest.json` — archive metadata and SHA-256 checksums
/// - `transcript.json` — session messages as JSON array
/// - `triggers.json` — trigger rules (if enabled)
/// - `cron_jobs.json` — cron job definitions (if enabled)
/// - `loop-state/` — loop state files for stateful cron jobs (if enabled)
///
/// # Arguments
///
/// * `storage` — Storage backend to read messages and cron jobs
/// * `trigger_engine` — Optional trigger engine to export trigger rules
/// * `session_id` — ID of the session to export
/// * `output_path` — Path where the archive file will be written
/// * `config` — Export configuration options
///
/// # Errors
///
/// Returns an error if:
/// - The session is not found
/// - File I/O operations fail
/// - Archive creation fails
///
/// # Examples
///
/// ```no_run
/// use ragent_agent::session::archive::{export_session_archive, ArchiveConfig};
/// use ragent_storage::Storage;
/// use std::sync::Arc;
///
/// let storage = Arc::new(Storage::open_in_memory().unwrap());
/// let config = ArchiveConfig::default();
/// export_session_archive(
///     &storage,
///     None,
///     "session-id",
///     "/path/to/archive.tar.gz",
///     &config,
/// ).unwrap();
/// ```
pub fn export_session_archive(
    storage: &Storage,
    trigger_engine: Option<&DynamicTriggerEngine>,
    session_id: &str,
    output_path: &Path,
    config: &ArchiveConfig,
) -> Result<PathBuf> {
    // Get session metadata
    let session = storage
        .get_session(session_id)
        .context("Failed to get session")?
        .ok_or_else(|| anyhow!("Session not found: {}", session_id))?;

    // Get messages
    let messages = storage
        .get_messages(session_id)
        .context("Failed to get messages")?;

    // Get trigger rules if enabled
    let triggers: Vec<TriggerRule> = if config.include_triggers {
        trigger_engine.map(|te| te.list_rules()).unwrap_or_default()
    } else {
        Vec::new()
    };

    // Get cron jobs if enabled
    let cron_jobs: Vec<CronEventRow> = if config.include_cron {
        storage
            .list_cron_events()
            .context("Failed to list cron jobs")?
    } else {
        Vec::new()
    };

    // Create temporary directory for archive contents
    let temp_dir = tempfile::tempdir().context("Failed to create temp directory")?;
    let temp_path = temp_dir.path();

    // Write transcript.json
    let transcript_path = temp_path.join("transcript.json");
    let transcript_json =
        serde_json::to_string_pretty(&messages).context("Failed to serialize messages")?;
    fs::write(&transcript_path, &transcript_json).context("Failed to write transcript.json")?;

    // Write triggers.json if enabled
    let trigger_count = triggers.len();
    if config.include_triggers {
        let triggers_path = temp_path.join("triggers.json");
        let triggers_json =
            serde_json::to_string_pretty(&triggers).context("Failed to serialize triggers")?;
        fs::write(&triggers_path, &triggers_json).context("Failed to write triggers.json")?;
    }

    // Write cron_jobs.json if enabled
    let cron_job_count = cron_jobs.len();
    let mut loop_state_count = 0;
    if config.include_cron {
        // Serialize cron jobs to JSON (skip non-serializable fields)
        #[derive(Serialize)]
        struct CronJobExport {
            id: String,
            agent_type: String,
            prompt: String,
            schedule_form: String,
            start_at: Option<String>,
            duration_secs: Option<i64>,
            schedule_raw: String,
            enabled: bool,
            next_due: String,
            created_at: String,
            last_fired: Option<String>,
            stateful: bool,
        }

        let cron_exports: Vec<CronJobExport> = cron_jobs
            .iter()
            .map(|c| CronJobExport {
                id: c.id.clone(),
                agent_type: c.agent_type.clone(),
                prompt: c.prompt.clone(),
                schedule_form: c.schedule_form.clone(),
                start_at: c.start_at.clone(),
                duration_secs: c.duration_secs,
                schedule_raw: c.schedule_raw.clone(),
                enabled: c.enabled,
                next_due: c.next_due.clone(),
                created_at: c.created_at.clone(),
                last_fired: c.last_fired.clone(),
                stateful: c.stateful,
            })
            .collect();

        let cron_path = temp_path.join("cron_jobs.json");
        let cron_json =
            serde_json::to_string_pretty(&cron_exports).context("Failed to serialize cron jobs")?;
        fs::write(&cron_path, &cron_json).context("Failed to write cron_jobs.json")?;

        // Copy loop state files if enabled
        if config.include_loop_state {
            let loop_state_dir = temp_path.join("loop-state");
            fs::create_dir_all(&loop_state_dir).context("Failed to create loop-state directory")?;

            let data_dir = dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("ragent");
            let loop_state_source = data_dir.join("loop-state");

            if loop_state_source.exists() {
                for cron_job in &cron_exports {
                    if cron_job.stateful {
                        let source_file = loop_state_source.join(format!("{}.txt", cron_job.id));
                        if source_file.exists() {
                            let dest_file = loop_state_dir.join(format!("{}.txt", cron_job.id));
                            fs::copy(&source_file, &dest_file).with_context(|| {
                                format!("Failed to copy loop state file: {}", source_file.display())
                            })?;
                            loop_state_count += 1;
                        }
                    }
                }
            }
        }
    }

    // Compute SHA-256 checksums and build file list
    let mut file_checksums: HashMap<String, String> = HashMap::new();

    // Hash transcript.json
    let transcript_hash = sha256_file(&transcript_path)?;
    file_checksums.insert("transcript.json".to_string(), transcript_hash);

    // Hash triggers.json if present
    if config.include_triggers {
        let triggers_path = temp_path.join("triggers.json");
        let triggers_hash = sha256_file(&triggers_path)?;
        file_checksums.insert("triggers.json".to_string(), triggers_hash);
    }

    // Hash cron_jobs.json if present
    if config.include_cron {
        let cron_path = temp_path.join("cron_jobs.json");
        let cron_hash = sha256_file(&cron_path)?;
        file_checksums.insert("cron_jobs.json".to_string(), cron_hash);

        // Hash loop state files if present
        if config.include_loop_state {
            let loop_state_dir = temp_path.join("loop-state");
            if loop_state_dir.exists() {
                for entry in fs::read_dir(&loop_state_dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_file() {
                        let file_name =
                            format!("loop-state/{}", path.file_name().unwrap().to_string_lossy());
                        let hash = sha256_file(&path)?;
                        file_checksums.insert(file_name, hash);
                    }
                }
            }
        }
    }

    // Create manifest
    let manifest = ArchiveManifest {
        manifest_version: MANIFEST_VERSION,
        session_id: session.id.clone(),
        session_title: session.title.clone(),
        session_directory: session.directory,
        created_at: Utc::now().to_rfc3339(),
        message_count: messages.len(),
        trigger_count,
        cron_job_count,
        loop_state_count,
        files: file_checksums,
        sensitivity_warning: "This archive contains the full session transcript which may include sensitive information such as API keys, credentials, or proprietary code. Handle with appropriate security measures.".to_string(),
    };

    // Write manifest.json
    let manifest_path = temp_path.join("manifest.json");
    let manifest_json =
        serde_json::to_string_pretty(&manifest).context("Failed to serialize manifest")?;
    fs::write(&manifest_path, &manifest_json).context("Failed to write manifest.json")?;

    // Create tar.gz archive
    create_tarball(temp_path, output_path)?;

    Ok(output_path.to_path_buf())
}

/// Import configuration for session archives.
#[derive(Debug, Clone)]
pub struct ImportConfig {
    /// Activate trigger rules from the archive.
    pub activate_triggers: bool,
    /// Activate cron jobs from the archive.
    pub activate_cron: bool,
    /// Restore loop state files from the archive.
    pub restore_loop_state: bool,
    /// Verify SHA-256 checksums before import (fail on mismatch).
    pub verify_checksums: bool,
    /// Import trigger rules from the archive.
    pub import_triggers: bool,
}

impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            activate_triggers: false,
            activate_cron: false,
            restore_loop_state: true,
            verify_checksums: true,
            import_triggers: false,
        }
    }
}

/// Result of importing a session archive.
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// Session ID that was created/updated.
    pub session_id: String,
    /// Number of messages imported.
    pub messages_imported: usize,
    /// Number of trigger rules imported.
    pub triggers_imported: usize,
    /// Number of cron jobs imported.
    pub cron_jobs_imported: usize,
    /// Number of loop state files restored.
    pub loop_state_files_restored: usize,
    /// Checksum verification was performed.
    pub checksums_verified: bool,
}

/// Import a session from a portable archive file.
///
/// Extracts a `.tar.gz` archive created by `export_session_archive` and
/// imports the session into the current ragent instance.
///
/// # Arguments
///
/// * `storage` — Storage backend to write messages and cron jobs
/// * `trigger_engine` — Optional trigger engine to import trigger rules
/// * `archive_path` — Path to the archive file to import
/// * `config` — Import configuration options
///
/// # Errors
///
/// Returns an error if:
/// - The archive file cannot be read or extracted
/// - The manifest is missing or invalid
/// - Checksum verification fails (if enabled)
/// - The transcript cannot be deserialized
/// - Database writes fail
///
/// # Examples
///
/// ```no_run
/// # use ragent_agent::session::archive::{import_session_archive, ImportConfig};
/// # use ragent_storage::Storage;
/// # use std::sync::Arc;
/// # async fn example() -> anyhow::Result<()> {
/// let storage = Arc::new(Storage::open_in_memory()?);
/// let config = ImportConfig {
///     verify_checksums: true,
///     activate_triggers: false,
///     ..Default::default()
/// };
/// let result = import_session_archive(&storage, None, "session.tar.gz", &config).await?;
/// println!("Imported {} messages", result.messages_imported);
/// # Ok(())
/// # }
/// ```
pub async fn import_session_archive(
    storage: &Arc<Storage>,
    trigger_engine: Option<&DynamicTriggerEngine>,
    archive_path: &Path,
    config: &ImportConfig,
) -> Result<ImportResult> {
    use flate2::read::GzDecoder;
    use tar::Archive;
    use tempfile::tempdir;

    // Open and extract archive to temp directory
    let archive_file = File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;
    let tar = GzDecoder::new(archive_file);
    let mut archive = Archive::new(tar);

    let temp_dir = tempdir().context("Failed to create temp directory")?;
    archive
        .unpack(temp_dir.path())
        .context("Failed to extract archive")?;

    // Read and validate manifest
    let manifest_path = temp_dir.path().join("manifest.json");
    let manifest_content =
        fs::read_to_string(&manifest_path).context("Failed to read manifest.json")?;
    let manifest: ArchiveManifest =
        serde_json::from_str(&manifest_content).context("Failed to parse manifest.json")?;

    // Verify checksums if enabled
    if config.verify_checksums {
        verify_archive_checksums(temp_dir.path(), &manifest)?;
    }

    // Read transcript
    let transcript_path = temp_dir.path().join("transcript.json");
    let transcript_content =
        fs::read_to_string(&transcript_path).context("Failed to read transcript.json")?;
    let messages: Vec<message::Message> =
        serde_json::from_str(&transcript_content).context("Failed to parse transcript.json")?;

    // Create new session in the manifest's directory (or current dir if unavailable)
    let session_dir = if manifest.session_directory.is_empty() {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    } else {
        PathBuf::from(&manifest.session_directory)
    };

    // Ensure directory exists
    if !session_dir.exists() {
        fs::create_dir_all(&session_dir).with_context(|| {
            format!(
                "Failed to create session directory: {}",
                session_dir.display()
            )
        })?;
    }

    // Generate a new session ID and create it
    let session_id = uuid::Uuid::new_v4().to_string();
    storage.create_session(&session_id, &session_dir.to_string_lossy())?;

    let mut imported_count = 0u64;
    for msg in &messages {
        // Re-parent each message into the new session with a fresh ID
        let imported_msg = message::Message {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: session_id.clone(),
            role: msg.role.clone(),
            parts: msg.parts.clone(),
            created_at: msg.created_at,
            updated_at: msg.updated_at,
        };
        storage.create_message(&imported_msg)?;
        imported_count += 1;
    }

    // Import trigger rules if present and enabled
    let mut triggers_imported = 0;
    if config.import_triggers {
        let triggers_path = temp_dir.path().join("triggers.json");
        if triggers_path.exists() {
            let triggers_content =
                fs::read_to_string(&triggers_path).context("Failed to read triggers.json")?;
            let triggers: Vec<TriggerRule> =
                serde_json::from_str(&triggers_content).context("Failed to parse triggers.json")?;

            if let Some(engine) = trigger_engine {
                for trigger in triggers {
                    let _ = engine.runtime().add_rule(trigger);
                    triggers_imported += 1;
                }
            }
        }
    }

    // Import cron jobs if present and enabled
    let mut cron_jobs_imported = 0;
    if config.activate_cron {
        let cron_path = temp_dir.path().join("cron_jobs.json");
        if cron_path.exists() {
            let cron_content =
                fs::read_to_string(&cron_path).context("Failed to read cron_jobs.json")?;
            let cron_exports: Vec<CronEventExport> =
                serde_json::from_str(&cron_content).context("Failed to parse cron_jobs.json")?;

            for cron_export in cron_exports {
                let schedule = CronSchedule::repeat_now(cron_export.duration_secs.unwrap_or(300));
                let cron_event = CronEvent::new(
                    uuid::Uuid::new_v4().to_string(),
                    cron_export.agent_type,
                    cron_export.prompt,
                    schedule,
                    cron_export.schedule_raw,
                    chrono::Utc::now(),
                );
                storage.insert_cron_event(&cron_event)?;
                cron_jobs_imported += 1;
            }
        }
    }

    // Restore loop state files if enabled
    let mut loop_state_restored = 0;
    if config.restore_loop_state {
        let loop_state_dir = temp_dir.path().join("loop-state");
        if loop_state_dir.exists() {
            let data_dir = dirs::data_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("ragent");
            let loop_state_target = data_dir.join("loop-state");
            fs::create_dir_all(&loop_state_target)
                .context("Failed to create loop-state directory")?;

            for entry in fs::read_dir(&loop_state_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().unwrap();
                    let target_path = loop_state_target.join(file_name);
                    fs::copy(&path, &target_path).with_context(|| {
                        format!("Failed to restore loop state file: {}", path.display())
                    })?;
                    loop_state_restored += 1;
                }
            }
        }
    }

    Ok(ImportResult {
        session_id,
        messages_imported: imported_count as usize,
        triggers_imported,
        cron_jobs_imported,
        loop_state_files_restored: loop_state_restored,
        checksums_verified: config.verify_checksums,
    })
}

/// Verify SHA-256 checksums of all files in an extracted archive.
///
/// # Errors
///
/// Returns an error if any file's checksum does not match the manifest.
fn verify_archive_checksums(extract_dir: &Path, manifest: &ArchiveManifest) -> Result<()> {
    use sha2::{Digest, Sha256};

    for (file_path, expected_hash) in &manifest.files {
        let full_path = extract_dir.join(file_path);
        if !full_path.exists() {
            return Err(anyhow!(
                "Checksum verification failed: missing file '{}'",
                file_path
            ));
        }

        let file = File::open(&full_path)
            .with_context(|| format!("Failed to open file: {}", full_path.display()))?;
        let mut reader = BufReader::new(file);
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }

        let actual_hash = format!("{:x}", hasher.finalize());
        if actual_hash != *expected_hash {
            return Err(anyhow!(
                "Checksum verification failed for '{}': expected {}, got {}",
                file_path,
                expected_hash,
                actual_hash
            ));
        }
    }

    Ok(())
}

/// Cron job export structure for import.
#[derive(Debug, Clone, Deserialize)]
struct CronEventExport {
    // reason: id is deserialised for import validation but not consumed by the archive step.
    #[allow(dead_code)]
    id: String,
    agent_type: String,
    prompt: String,
    // reason: schedule_form is parsed to preserve round-trip fidelity but unused here.
    #[allow(dead_code)]
    schedule_form: String,
    // reason: start_at is read from export payloads but not needed for re-import.
    #[allow(dead_code)]
    start_at: Option<String>,
    duration_secs: Option<i64>,
    schedule_raw: String,
    // reason: enabled is captured for completeness but not applied during import.
    #[allow(dead_code)]
    enabled: bool,
    // reason: next_due is informational from the export and not used on import.
    #[allow(dead_code)]
    next_due: String,
    // reason: created_at is preserved for audit traceability but not consumed.
    #[allow(dead_code)]
    created_at: String,
    // reason: last_fired is historical metadata that is not re-imported.
    #[allow(dead_code)]
    last_fired: Option<String>,
    // reason: stateful flag is read to preserve semantics but not used in archive.
    #[allow(dead_code)]
    stateful: bool,
}

/// Compute SHA-256 hash of a file.
fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};

    let file =
        File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();

    let mut buffer = [0u8; 8192];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

/// Create a tar.gz archive from a directory.
fn create_tarball(source_dir: &Path, output_path: &Path) -> Result<()> {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tar::Builder;

    let file = File::create(output_path)
        .with_context(|| format!("Failed to create archive: {}", output_path.display()))?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut tar = Builder::new(encoder);

    // Add all files and directories
    for entry in walkdir::WalkDir::new(source_dir) {
        let entry = entry?;
        let path = entry.path();
        let relative_path = path.strip_prefix(source_dir)?;

        // Skip the root directory itself
        if relative_path.as_os_str().is_empty() {
            continue;
        }

        let relative_str = relative_path.to_string_lossy().to_string();

        if path.is_file() {
            tar.append_path_with_name(path, &relative_str)?;
        } else if path.is_dir() {
            tar.append_dir_all(&relative_str, path)?;
        }
    }

    tar.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_manifest_serialization() {
        let mut files = HashMap::new();
        files.insert("transcript.json".to_string(), "abc123".to_string());

        let manifest = ArchiveManifest {
            manifest_version: MANIFEST_VERSION,
            session_id: "test-session".to_string(),
            session_title: "Test Session".to_string(),
            session_directory: "/tmp/test".to_string(),
            created_at: Utc::now().to_rfc3339(),
            message_count: 10,
            trigger_count: 2,
            cron_job_count: 1,
            loop_state_count: 0,
            files,
            sensitivity_warning: "Warning".to_string(),
        };

        let json = serde_json::to_string(&manifest).unwrap();
        let _back: ArchiveManifest = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_sha256_file_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("empty.txt");
        fs::write(&file_path, b"").unwrap();

        let hash = sha256_file(&file_path).unwrap();
        // SHA-256 of empty string
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_sha256_file_content() {
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, b"hello world").unwrap();

        let hash = sha256_file(&file_path).unwrap();
        // SHA-256 of "hello world"
        assert_eq!(
            hash,
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }

    #[test]
    fn test_archive_config_defaults() {
        let config = ArchiveConfig::default();
        assert!(config.include_triggers);
        assert!(config.include_cron);
        assert!(config.include_loop_state);
        assert!(!config.include_cost);
    }

    #[test]
    fn test_import_config_defaults() {
        let config = ImportConfig::default();
        assert!(!config.activate_triggers);
        assert!(!config.activate_cron);
        assert!(config.restore_loop_state);
        assert!(config.verify_checksums);
        assert!(!config.import_triggers);
    }
}
