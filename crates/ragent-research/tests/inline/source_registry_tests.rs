//! Inline tests for `source_registry.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[tokio::test]
async fn builtin_registry_lists_sources() {
    let reg = BuiltinSourceRegistry::new();
    let sources = reg.discover().await.unwrap();
    assert_eq!(sources.len(), 3);
    assert!(sources.iter().any(|s| s.id == "web"));
    assert!(sources.iter().any(|s| s.id == "local"));
    assert!(sources.iter().any(|s| s.id == "spec"));
}
