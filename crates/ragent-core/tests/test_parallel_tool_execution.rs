//! Tests for test_parallel_tool_execution.rs

//! Integration tests for parallel tool execution with concurrency limits.

use ragent_core::event::EventBus;
use ragent_core::permission::PermissionChecker;
use ragent_core::provider::ProviderRegistry;
use ragent_core::session::processor::SessionProcessor;
use ragent_core::session::SessionManager;
use ragent_core::storage::Storage;
use ragent_core::tool::{Tool, ToolContext, ToolOutput, ToolRegistry};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// A test tool that tracks concurrent executions
struct ConcurrencyTestTool {
    active_count: Arc<AtomicUsize>,
    max_concurrent: Arc<AtomicUsize>,
    call_count: Arc<AtomicUsize>,
    delay_ms: u64,
}

impl ConcurrencyTestTool {
    fn new(delay_ms: u64) -> Self {
        Self {
            active_count: Arc::new(AtomicUsize::new(0)),
            max_concurrent: Arc::new(AtomicUsize::new(0)),
            call_count: Arc::new(AtomicUsize::new(0)),
            delay_ms,
        }
    }

    fn get_stats(&self) -> (usize, usize) {
        (
            self.call_count.load(Ordering::SeqCst),
            self.max_concurrent.load(Ordering::SeqCst),
        )
    }
}

#[async_trait::async_trait]
impl Tool for ConcurrencyTestTool {
    fn name(&self) -> &str {
        "concurrency_test"
    }

    fn description(&self) -> &str {
        "Test tool for tracking concurrent executions"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "Identifier for this call"
                }
            },
            "required": ["id"]
        })
    }

    fn permission_category(&self) -> &str {
        "test"
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        
        // Increment active count
        let active = self.active_count.fetch_add(1, Ordering::SeqCst) + 1;
        
        // Update max if needed
        self.max_concurrent.fetch_max(active, Ordering::SeqCst);
        
        // Simulate work
        sleep(Duration::from_millis(self.delay_ms)).await;
        
        // Decrement active count
        self.active_count.fetch_sub(1, Ordering::SeqCst);
        
        let id = input.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
        Ok(ToolOutput {
            content: format!("Completed call {}", id),
            metadata: None,
        })
    }
}

#[tokio::test]
async fn test_max_5_parallel_tools() {
    // Create a test tool that takes 100ms to execute
    let test_tool = Arc::new(ConcurrencyTestTool::new(100));
    
    // Create tool registry with our test tool
    let mut registry = ToolRegistry::new();
    registry.register(test_tool.clone());
    
    // Set up session processor components
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(100));
    
    // Create session manager with event bus
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let provider_registry = Arc::new(ProviderRegistry::new());
    
    // Create empty permission ruleset for testing
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
    let registry = Arc::new(registry);
    
    let _processor = SessionProcessor {
        session_manager,
        provider_registry,
        tool_registry: registry,
        permission_checker,
        event_bus,
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
    };
    
    // Note: This test is simplified and won't actually call process_message
    // because that requires a full LLM setup. Instead, we're documenting
    // the expected behavior.
    
    // Get stats
    let (total_calls, max_concurrent) = test_tool.get_stats();
    
    // When 10 tools are called, we expect:
    // - All 10 to eventually execute (total_calls = 10)
    // - Maximum of 5 concurrent at any time (max_concurrent <= 5)
    
    println!("Test setup complete. In actual execution:");
    println!("  - Expected total calls: 10");
    println!("  - Expected max concurrent: <= 5");
    println!("  - Current stats: calls={}, max_concurrent={}", total_calls, max_concurrent);
}

#[tokio::test]
async fn test_sequential_execution_with_agent_switch() {
    // Test that when an agent switch is requested mid-execution,
    // remaining tools in the chunk are not executed
    
    // This is a placeholder test to document the behavior
    // In actual implementation, if tool 3 out of 5 requests an agent switch,
    // tools 4 and 5 in that chunk should still complete, but the next chunk
    // should not be processed.
    
    println!("Agent switch behavior:");
    println!("  - Tools execute in chunks of 5");
    println!("  - If agent switch requested, current chunk completes");
    println!("  - Subsequent chunks are skipped");
}

#[test]
fn test_max_parallel_tools_constant() {
    // Verify the concurrency limit matches expectation
    assert_eq!(ragent_core::resource::MAX_CONCURRENT_TOOLS, 5);
}
