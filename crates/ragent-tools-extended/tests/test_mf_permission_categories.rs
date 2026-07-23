//! Integration test verifying masterfetch tool permission categories (T-039,
//! FR-022, NFR-003).
//!
//! FR-022 requires:
//! - `mf_fetch`, `mf_crawl`, `mf_search`, `mf_screenshot` → `"web"`
//!   (they make outbound network calls)
//! - `mf_cache_clear`, `mf_version` → `"system"`
//!   (they do not make outbound network calls)
//!
//! This test verifies every tool individually and also checks the full set
//! registered in `create_extended_registry()`.

use ragent_tools_extended::masterfetch::tools::cache_clear::MfCacheClearTool;
use ragent_tools_extended::masterfetch::tools::crawl_tool::MfCrawlTool;
use ragent_tools_extended::masterfetch::tools::fetch::MfFetchTool;
use ragent_tools_extended::masterfetch::tools::screenshot::MfScreenshotTool;
use ragent_tools_extended::masterfetch::tools::search_tool::MfSearchTool;
use ragent_tools_extended::masterfetch::tools::version::MfVersionTool;
use ragent_tools_extended::{Tool, create_extended_registry};

// ---------------------------------------------------------------------------
// Per-tool permission_category() checks
// ---------------------------------------------------------------------------

/// Helper: assert a tool returns the expected permission category.
fn assert_category(tool: &dyn Tool, expected: &str) {
    let actual = tool.permission_category();
    assert_eq!(
        actual,
        expected,
        "tool '{}' should return permission_category '{}' but got '{}'",
        tool.name(),
        expected,
        actual,
    );
}

// --- Network tools → "web" (FR-022) ----------------------------------------

#[test]
fn test_mf_fetch_permission_category_is_web() {
    let tool = MfFetchTool;
    assert_category(&tool, "web");
}

#[test]
fn test_mf_crawl_permission_category_is_web() {
    let tool = MfCrawlTool;
    assert_category(&tool, "web");
}

#[test]
fn test_mf_search_permission_category_is_web() {
    let tool = MfSearchTool;
    assert_category(&tool, "web");
}

#[test]
fn test_mf_screenshot_permission_category_is_web() {
    let tool = MfScreenshotTool;
    assert_category(&tool, "web");
}

// --- Non-network tools → "system" (FR-022) ---------------------------------

#[test]
fn test_mf_cache_clear_permission_category_is_system() {
    let tool = MfCacheClearTool;
    assert_category(&tool, "system");
}

#[test]
fn test_mf_version_permission_category_is_system() {
    let tool = MfVersionTool;
    assert_category(&tool, "system");
}

// ---------------------------------------------------------------------------
// Struct-level: all six tools have the expected category
// ---------------------------------------------------------------------------

/// Names of the four network tools that must return `"web"`.
const WEB_TOOLS: &[&str] = &["mf_fetch", "mf_crawl", "mf_search", "mf_screenshot"];

/// Names of the two non-network tools that must return `"system"`.
const SYSTEM_TOOLS: &[&str] = &["mf_cache_clear", "mf_version"];

#[test]
fn test_all_web_tools_return_web_from_struct() {
    let tools: [(&str, &dyn Tool); 4] = [
        ("mf_fetch", &MfFetchTool as &dyn Tool),
        ("mf_crawl", &MfCrawlTool as &dyn Tool),
        ("mf_search", &MfSearchTool as &dyn Tool),
        ("mf_screenshot", &MfScreenshotTool as &dyn Tool),
    ];
    for (name, tool) in tools {
        assert_eq!(
            tool.permission_category(),
            "web",
            "tool '{name}' should return permission_category \"web\""
        );
    }
}

#[test]
fn test_all_system_tools_return_system_from_struct() {
    let tools: [(&str, &dyn Tool); 2] = [
        ("mf_cache_clear", &MfCacheClearTool as &dyn Tool),
        ("mf_version", &MfVersionTool as &dyn Tool),
    ];
    for (name, tool) in tools {
        assert_eq!(
            tool.permission_category(),
            "system",
            "tool '{name}' should return permission_category \"system\""
        );
    }
}

// ---------------------------------------------------------------------------
// Exhaustive: verify no mf_ tool has an unexpected category
// ---------------------------------------------------------------------------

#[test]
fn test_all_mf_tools_have_valid_permission_category() {
    let all_tools: [(&str, &dyn Tool); 6] = [
        ("mf_fetch", &MfFetchTool as &dyn Tool),
        ("mf_crawl", &MfCrawlTool as &dyn Tool),
        ("mf_search", &MfSearchTool as &dyn Tool),
        ("mf_screenshot", &MfScreenshotTool as &dyn Tool),
        ("mf_cache_clear", &MfCacheClearTool as &dyn Tool),
        ("mf_version", &MfVersionTool as &dyn Tool),
    ];

    for (name, tool) in all_tools {
        let cat = tool.permission_category();
        assert!(
            cat == "web" || cat == "system",
            "tool '{name}' has unexpected permission_category '{cat}' (expected \"web\" or \"system\")"
        );
    }
}

// ---------------------------------------------------------------------------
// Count: exactly 4 web + 2 system = 6 total
// ---------------------------------------------------------------------------

#[test]
fn test_exact_count_of_web_and_system_mf_tools() {
    let all_tools: [(&str, &dyn Tool); 6] = [
        ("mf_fetch", &MfFetchTool as &dyn Tool),
        ("mf_crawl", &MfCrawlTool as &dyn Tool),
        ("mf_search", &MfSearchTool as &dyn Tool),
        ("mf_screenshot", &MfScreenshotTool as &dyn Tool),
        ("mf_cache_clear", &MfCacheClearTool as &dyn Tool),
        ("mf_version", &MfVersionTool as &dyn Tool),
    ];

    let web_count = all_tools
        .iter()
        .filter(|(_, t)| t.permission_category() == "web")
        .count();
    let system_count = all_tools
        .iter()
        .filter(|(_, t)| t.permission_category() == "system")
        .count();

    assert_eq!(
        web_count, 4,
        "expected exactly 4 web tools, got {web_count}"
    );
    assert_eq!(
        system_count, 2,
        "expected exactly 2 system tools, got {system_count}"
    );
    assert_eq!(
        web_count + system_count,
        6,
        "expected exactly 6 total mf_ tools"
    );
}

// ---------------------------------------------------------------------------
// Registry registration: all six tools are registered in create_extended_registry
// ---------------------------------------------------------------------------

#[test]
fn test_all_six_mf_tools_registered_with_correct_categories() {
    let registry = create_extended_registry();

    // Verify all six tools are present and have correct categories.
    for name in WEB_TOOLS {
        let tool = registry.get(name);
        assert!(
            tool.is_some(),
            "tool '{name}' should be registered in create_extended_registry()"
        );
        if let Some(ref t) = tool {
            assert_eq!(
                t.permission_category(),
                "web",
                "registered tool '{name}' should return permission_category \"web\""
            );
        }
    }
    for name in SYSTEM_TOOLS {
        let tool = registry.get(name);
        assert!(
            tool.is_some(),
            "tool '{name}' should be registered in create_extended_registry()"
        );
        if let Some(ref t) = tool {
            assert_eq!(
                t.permission_category(),
                "system",
                "registered tool '{name}' should return permission_category \"system\""
            );
        }
    }
}
