use ragent_aiwiki::sync::sources::{make_ref_key, parse_ref_key, resolve_file_path};
use std::path::Path;

fn main() {
    // Test single file source key creation and parsing
    let source = "SPEC.md";
    let key = make_ref_key(source, "");
    println!("Key for single file '{}': {}", source, key);
    
    if let Some((src, file)) = parse_ref_key(&key) {
        println!("Parsed: source='{}', file='{}'", src, file);
    }
    
    // Test resolve_file_path
    let root = Path::new("/home/project");
    let raw_dir = Path::new("/home/project/aiwiki/raw");
    let resolved = resolve_file_path(root, raw_dir, &key);
    println!("Resolved path: {}", resolved.display());
}
