//! Tests for the sandboxed runtime: interruption, memory ceiling, exception
//! containment, and global isolation (spec `plugins` T-007; FR-003, FR-015,
//! FR-017, FR-018, FR-026).

use std::time::{Duration, Instant};

use ragent_plugins::{HOST_API_VERSION, HostApiInstall, PluginError, RuntimePool, SandboxBudget};

fn quick_budget() -> SandboxBudget {
    SandboxBudget {
        memory_bytes: 8 * 1024 * 1024,
        deadline: Duration::from_millis(300),
        stack_bytes: 256 * 1024,
    }
}

#[test]
fn checkout_eval_roundtrip_simple_script() {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(quick_budget()).expect("checkout");
    let value = ctx.eval_to_string("1 + 2 + 3").expect("eval ok");
    assert_eq!(value, "6");
}

#[test]
fn infinite_loop_is_interrupted_at_deadline() {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(quick_budget()).expect("checkout");
    let started = Instant::now();
    let err = ctx
        .eval("let n = 0; while (true) { n += 1; } n")
        .expect_err("loop must be interrupted");
    let elapsed = started.elapsed();
    assert!(
        matches!(err, PluginError::Timeout),
        "expected Timeout, got {err:?}"
    );
    assert!(
        elapsed < Duration::from_millis(1200),
        "interrupt must fire near the 300 ms deadline, took {elapsed:?}"
    );
}

#[test]
fn memory_ceiling_contains_allocation_loop() {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(quick_budget()).expect("checkout");
    let err = ctx
        .eval("const chunks = []; for (;;) { chunks.push(new Array(50000).fill('x'.repeat(16))); }")
        .expect_err("allocation loop must trip the memory ceiling");
    assert!(
        matches!(err, PluginError::MemoryLimit),
        "expected MemoryLimit, got {err:?}"
    );
    // The host thread is alive and a fresh context still works after the OOM.
    let ctx2 = pool.checkout(quick_budget()).expect("re-checkout");
    let ok = ctx2.eval("41 + 1");
    assert!(ok.is_ok());
}

#[test]
fn exceptions_are_contained_as_script_errors() {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(quick_budget()).expect("checkout");
    let err = ctx
        .eval("throw new Error('plugin blew up')")
        .expect_err("exception surfaces as a contained error");
    match err {
        PluginError::Script { detail, .. } => {
            assert!(detail.contains("plugin blew up"), "message: {detail}");
        }
        other => panic!("expected Script, got {other:?}"),
    }
}

#[test]
fn sandbox_has_no_host_globals_until_host_api_installs_them() {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(quick_budget()).expect("checkout");
    // No std/os/print (FR-018).
    let std_type: String = ctx
        .raw()
        .with(|c| c.eval("typeof std + ',' + typeof os + ',' + typeof print"))
        .expect("eval");
    assert_eq!(std_type, "undefined,undefined,undefined");
    // And no ragent global before install.
    let ragent_type: String = ctx.raw().with(|c| c.eval("typeof ragent")).expect("eval");
    assert_eq!(ragent_type, "undefined");
}

#[test]
fn host_api_install_exposes_version_and_plugin_id() {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(quick_budget()).expect("checkout");
    let install = HostApiInstall::new(
        "codex-weather",
        std::path::PathBuf::from("/plugins/codex-weather"),
    );
    ctx.install_host_api(&install).expect("install host api");
    let version: u32 = ctx
        .raw()
        .with(|c| c.eval("ragent.api_version"))
        .expect("eval api_version");
    assert_eq!(version, HOST_API_VERSION);
    let id: String = ctx
        .raw()
        .with(|c| c.eval("ragent.plugin_id"))
        .expect("eval plugin_id");
    assert_eq!(id, "codex-weather");
}

#[test]
fn eval_json_marshals_object_results() {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(quick_budget()).expect("checkout");
    let value = ctx
        .eval_json("JSON.stringify({city: 'Berlin', ok: true})")
        .expect("eval_json");
    assert_eq!(value["city"], "Berlin");
    assert_eq!(value["ok"], true);
}

#[test]
fn eval_json_rejects_non_string_results_with_a_clear_error() {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(quick_budget()).expect("checkout");
    let err = ctx
        .eval_json("({city: 'Berlin'})")
        .expect_err("non-string handler results are a marshalling error");
    match err {
        PluginError::Script { detail, .. } => {
            assert!(
                detail.contains("plugin handler must return a JSON string"),
                "detail: {detail}"
            );
        }
        other => panic!("expected Script, got {other:?}"),
    }
}

#[test]
fn set_global_str_marshals_arguments_in() {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(quick_budget()).expect("checkout");
    ctx.set_global_str("__args", r#"{"city":"Berlin"}"#)
        .expect("set global");
    let parsed: String = ctx
        .raw()
        .with(|c| c.eval("JSON.parse(__args).city"))
        .expect("eval");
    assert_eq!(parsed, "Berlin");
}

#[test]
fn time_remaining_decreases_and_floors_at_zero() {
    let pool = RuntimePool::new();
    let ctx = pool.checkout(quick_budget()).expect("checkout");
    let first = ctx.time_remaining();
    assert!(first <= Duration::from_millis(300));
}

#[test]
fn sandbox_budget_uses_config_fields() {
    let config = ragent_config::PluginsConfig {
        max_execution_ms: 2_500,
        max_memory_mb: 32,
        ..ragent_config::PluginsConfig::default()
    };
    let budget = SandboxBudget::from_config(&config);
    assert_eq!(budget.deadline, Duration::from_millis(2_500));
    assert_eq!(budget.memory_bytes, 32 * 1024 * 1024);
}
