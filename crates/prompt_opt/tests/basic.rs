use async_trait::async_trait;
use prompt_opt::{Completer, OptMethod, optimize, system_prompt};

struct MockCompleter;

#[async_trait]
impl Completer for MockCompleter {
    async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String> {
        Ok(format!("[mock] sys_len={} user={}", system.len(), user))
    }
}

#[tokio::test]
async fn test_all_methods_smoke() {
    let c = MockCompleter;
    for method in OptMethod::all() {
        let result = optimize(*method, "Write a hello-world program in Rust", &c)
            .await
            .unwrap();
        assert!(
            result.contains("Write a hello-world program in Rust"),
            "method {} output missing user input",
            method.name()
        );
    }
}

#[tokio::test]
async fn test_system_prompts_non_empty() {
    for method in OptMethod::all() {
        assert!(
            !system_prompt(*method).is_empty(),
            "system_prompt for {} is empty",
            method.name()
        );
    }
}

#[test]
fn test_help_table() {
    let table = OptMethod::help_table();
    assert!(table.contains("co_star"));
    assert!(table.contains("claude"));
    assert!(table.contains("microsoft"));
}

#[test]
fn test_from_str_round_trip() {
    use std::str::FromStr;
    for method in OptMethod::all() {
        let parsed = OptMethod::from_str(method.name()).ok();
        assert_eq!(
            parsed,
            Some(*method),
            "from_str({}) round-trip failed",
            method.name()
        );
    }
}
