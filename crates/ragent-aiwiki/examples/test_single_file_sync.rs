use ragent_aiwiki::{
    Aiwiki, AiwikiConfig, AiwikiState, SourceFolder,
    sync::{make_ref_key, parse_ref_key, preview_sync, resolve_file_path, sync},
};
use std::path::Path;
use tempfile::TempDir;

#[tokio::main]
async fn main() {
    // Create temp project structure
    let temp_dir = TempDir::new().unwrap();
    let project_root = temp_dir.path();

    // Create a single file (not in a folder)
    let spec_file = project_root.join("SPEC.md");
    tokio::fs::write(&spec_file, "# SPEC\n\nThis is the spec.")
        .await
        .unwrap();

    // Create aiwiki structure
    let aiwiki_root = project_root.join("aiwiki");
    tokio::fs::create_dir_all(&aiwiki_root.join("wiki"))
        .await
        .unwrap();
    tokio::fs::create_dir_all(&aiwiki_root.join("raw"))
        .await
        .unwrap();
    tokio::fs::create_dir_all(&aiwiki_root.join("static"))
        .await
        .unwrap();

    // Create config with single file source
    let mut config = AiwikiConfig::default();
    let source = SourceFolder::from_file_path("SPEC.md");
    config.sources.push(source);

    // Save config
    let config_path = aiwiki_root.join("config.json");
    let config_json = serde_json::to_string_pretty(&config).unwrap();
    tokio::fs::write(&config_path, config_json).await.unwrap();

    // Create empty state
    let state = AiwikiState::default();
    let state_path = aiwiki_root.join("state.json");
    let state_json = serde_json::to_string_pretty(&state).unwrap();
    tokio::fs::write(&state_path, state_json).await.unwrap();

    // Load wiki and run preview sync
    let wiki = Aiwiki::new(project_root).await.unwrap();

    println!("Running preview_sync...");
    let preview = preview_sync(&wiki).await.unwrap();
    println!(
        "Preview result: new_files={}, modified_files={}, deleted_files={}",
        preview.new_files.len(),
        preview.modified_files.len(),
        preview.deleted_files.len()
    );

    for (key, _size) in &preview.new_files {
        println!("  New file key: {}", key);
        if let Some((src, file)) = parse_ref_key(key) {
            println!("    Parsed: source={}, file={}", src, file);
            let resolved = resolve_file_path(project_root, &aiwiki_root.join("raw"), key);
            println!("    Resolved path: {}", resolved.display());
        }
    }

    // Now run actual sync without extractor
    println!("\nRunning sync (no extractor)...");
    let result = sync(&wiki, false, None, None).await;
    match result {
        Ok(r) => {
            println!("Sync result: {}", r.summary());
            for err in &r.errors {
                println!("  Error: {}", err);
            }
        }
        Err(e) => {
            println!("Sync error: {:?}", e);
        }
    }

    // Check final state
    let state = AiwikiState::load(&aiwiki_root).await.unwrap();
    println!("\nFinal state files:");
    for key in state.files.keys() {
        println!("  {}", key);
    }
}
