//! Sandboxed JavaScript runtime for plugin execution (spec `plugins`
//! T-004/T-007; FR-003, FR-015, FR-017, FR-018, FR-026).
//!
//! # Engine-choice spike results (T-004)
//!
//! Both candidate engines were evaluated on the three spike criteria
//! (scratch crate `target/temp/plugin-spike/`, since deleted):
//!
//! | Criterion                     | rquickjs 0.10                                   | boa 0.21                                       |
//! | ----------------------------- | ----------------------------------------------- | ---------------------------------------------- |
//! | (a) deadline interruption     | `Runtime::set_interrupt_handler` aborts `while(true){}` 200 ms after the deadline was set to now+200 ms (interrupted=true, elapsed=199.8 ms — fires within ~50 ms of the deadline). | No deadline hook; only a loop-*iteration* budget via `RuntimeLimits`, which cannot express a wall-clock budget (FR-017). |
//! | (b) memory ceiling            | `Runtime::set_memory_limit(8 MiB)` contained an unbounded allocation loop; host process intact. | No allocation-budget API.                        |
//! | (c) JSON marshalling          | No serde integration in 0.10; JSON crosses as strings via `JSON.parse`/`JSON.stringify` + `serde_json`. Adequate for host API v1. | Equivalent ergonomics (string round-trip).       |
//!
//! **Decision: rquickjs** (QuickJS), per the plan's default assumption.
//!
//! # Runtime design (T-007)
//!
//! - [`RuntimePool`] owns engine runtimes; [`RuntimePool::checkout`] returns a
//!   fresh [`SandboxContext`] pre-configured with the configured memory ceiling
//!   and an interrupt handler armed against a caller-supplied deadline.
//! - Deadline interruption maps to [`PluginError::Timeout`]; allocation-failure
//!   aborts map to [`PluginError::MemoryLimit`]; uncaught JavaScript
//!   exceptions map to [`PluginError::Script`]. No panic path escapes
//!   (FR-026) and no `unsafe` code appears outside the engine crate itself.
//! - The context exposes **no globals beyond what `host_api` installs**
//!   (FR-018): QuickJS's `std`/`os` modules are not registered and `print` is
//!   absent. `Runtime::new()`/bare `Context` provides no host bindings by
//!   default in rquickjs, so the sandbox starts empty and `host_api` fills it.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::error::PluginError;
use crate::host_api::HostApiInstall;

/// Per-tool and per-entry budgets for one plugin execution, derived from
/// `plugins.max_execution_ms` / `plugins.max_entry_ms` /
/// `plugins.max_memory_mb` (FR-017).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxBudget {
    /// Memory ceiling per script context, in bytes.
    pub memory_bytes: usize,
    /// Maximum wall-clock time for this execution.
    pub deadline: Duration,
    /// QuickJS stack size ceiling in bytes (a tighter cap than the memory
    /// budget so deep recursion trips stack before heap).
    pub stack_bytes: usize,
}

impl SandboxBudget {
    /// Build a budget from the `plugins` configuration block.
    #[must_use]
    pub fn from_config(config: &ragent_config::PluginsConfig) -> Self {
        Self {
            memory_bytes: (config.max_memory_mb.max(1)) as usize * 1024 * 1024,
            deadline: Duration::from_millis(config.max_execution_ms.max(1)),
            // 1 MiB stack: the engine default (256 KiB quickjs stack inside
            // the interpreter loop) plus headroom for host-API recursion;
            // plugin code should not be able to overflow the host stack via
            // JS recursion (FR-026).
            stack_bytes: 1024 * 1024,
        }
    }

    /// A long-running deadline for entry-point execution (entry budget).
    #[must_use]
    pub const fn with_deadline(self, deadline: Duration) -> Self {
        Self { deadline, ..self }
    }
}

impl Default for SandboxBudget {
    fn default() -> Self {
        Self::from_config(&ragent_config::PluginsConfig::default())
    }
}

/// Owns plugin engine runtimes and hands out configured sandbox contexts
/// (FR-003, FR-017).
#[derive(Debug, Default)]
pub struct RuntimePool;

impl RuntimePool {
    /// Create a new pool. Runtimes are allocated lazily per checkout so a
    /// crashed plugin cannot taint a pooled runtime (FR-026).
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Allocate a fresh sandbox context configured with `budget`.
    ///
    /// The interrupt handler is armed against `budget.deadline` measured from
    /// this call; JavaScript starts on the returned context's first `eval`.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::Engine`] when the engine runtime or context
    /// cannot be allocated.
    pub fn checkout(&self, budget: SandboxBudget) -> Result<SandboxContext, PluginError> {
        SandboxContext::new(budget)
    }
}

/// A single sandboxed JavaScript execution context for one plugin
/// invocation: fresh runtime, memory ceiling, deadline interrupt, empty
/// globals (FR-017, FR-018).
pub struct SandboxContext {
    /// Kept alive for the lifetime of the context (`rquickjs::Context` borrows
    /// its runtime). Field order matters: `runtime` declared before `ctx` so it
    /// drops last, never invalidating the context.
    #[allow(dead_code)]
    runtime: rquickjs::Runtime,
    /// The engine context the host API is installed into.
    ctx: rquickjs::Context,
    /// Per-execution budget applied to [`rearm_deadline`](Self::rearm_deadline).
    execution_budget: Duration,
    /// Interrupt bookkeeping. The handler checks `expired` first and then the
    /// `Instant` deadline; clearing+rearming between executions is cheap
    /// because the handler closure only reads atomics.
    deadline_at: Arc<std::sync::RwLock<Instant>>,
    /// Kill switch flipped when any execution exceeds its budget. Once tripped
    /// every subsequent interrupt returns `true` until [`reset_interrupt`].
    expired: Arc<AtomicBool>,
}

impl std::fmt::Debug for SandboxContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SandboxContext").finish_non_exhaustive()
    }
}

impl SandboxContext {
    fn new(budget: SandboxBudget) -> Result<Self, PluginError> {
        let runtime = rquickjs::Runtime::new().map_err(engine_err)?;
        runtime.set_memory_limit(budget.memory_bytes);
        runtime.set_max_stack_size(budget.stack_bytes);

        let deadline_at = Arc::new(std::sync::RwLock::new(Instant::now() + budget.deadline));
        let expired = Arc::new(AtomicBool::new(false));
        let handler_deadline = Arc::clone(&deadline_at);
        let handler_expired = Arc::clone(&expired);
        runtime.set_interrupt_handler(Some(Box::new(move || {
            if handler_expired.load(Ordering::Relaxed) {
                return true;
            }
            let deadline = handler_deadline
                .read()
                .map(|instant| *instant)
                .unwrap_or_else(|_| Instant::now());
            if Instant::now() >= deadline {
                handler_expired.store(true, Ordering::Relaxed);
                return true;
            }
            false
        })));

        let ctx = rquickjs::Context::full(&runtime).map_err(engine_err)?;
        Ok(Self {
            runtime,
            ctx,
            execution_budget: budget.deadline,
            deadline_at,
            expired,
        })
    }

    /// Install the host API into this sandbox (FR-004, FR-018).
    ///
    /// Idempotent per context; calling twice replaces the previous `ragent`
    /// global.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::Engine`] when the host object cannot be built.
    pub fn install_host_api(&self, install: &HostApiInstall) -> Result<(), PluginError> {
        self.ctx.with(|ctx| install.install(ctx))
    }

    /// Evaluate JavaScript source in this sandbox, discarding the result and
    /// mapping every engine outcome to a [`PluginError`] variant (FR-026).
    /// For a result-carrying evaluation use [`SandboxContext::eval_to_string`]
    /// or [`SandboxContext::eval_json`] — a raw `rquickjs::Value` cannot
    /// escape its context's lifetime.
    ///
    /// Timeout and memory-limit aborts are distinguished from ordinary
    /// exceptions by the error name QuickJS reports.
    ///
    /// # Errors
    ///
    /// - [`PluginError::Timeout`] when the deadline fired (FR-017).
    /// - [`PluginError::MemoryLimit`] when the allocation ceiling tripped.
    /// - [`PluginError::Script`] for any other JavaScript exception.
    /// - [`PluginError::Engine`] for engine-internal failures.
    pub fn eval(&self, source: &str) -> Result<(), PluginError> {
        self.ctx.with(|ctx| {
            ctx.eval::<rquickjs::Value<'_>, _>(source)
                .map_err(|e| classify_error(&ctx, e))?;
            Ok(())
        })
    }

    /// Evaluate JavaScript and coerce the result to a display string. Numbers
    /// and booleans are formatted; strings pass through; objects are
    /// JSON-stringified by the sandbox.
    ///
    /// # Errors
    ///
    /// See [`SandboxContext::eval`].
    pub fn eval_to_string(&self, source: &str) -> Result<String, PluginError> {
        let wrapped = format!(
            "(function(){{ const __out = eval({source_lit}); \
             if (typeof __out === 'undefined' || __out === null) return ''; \
             if (typeof __out === 'object') {{ try {{ return JSON.stringify(__out); }} catch(e) {{ return String(__out); }} }} \
             return String(__out); }})()",
            source_lit = js_string_literal(source)
        );
        self.ctx
            .with(|ctx| ctx.eval(wrapped).map_err(|e| classify_error(&ctx, e)))
    }

    /// Evaluate JavaScript and expect the result to be a **JSON document
    /// string** (the v1 marshalling contract for tool handlers, FR-005): the
    /// script is responsible for `JSON.stringify`ing its result. The returned
    /// string is parsed into `serde_json::Value` on the Rust side. A script
    /// returning a non-string is an error (FR-005 marshalling rules).
    ///
    /// # Errors
    ///
    /// See [`SandboxContext::eval`]; additionally [`PluginError::Script`] when
    /// the script yields a non-string or the string is not valid JSON.
    /// Evaluate JavaScript and expect the result to be a **JSON document
    /// string** (the v1 marshalling contract for tool handlers, FR-005): the
    /// script is responsible for `JSON.stringify`ing its result. The returned
    /// string is parsed into `serde_json::Value` on the Rust side. A script
    /// returning a non-string is an error (FR-005 marshalling rules).
    ///
    /// # Errors
    ///
    /// See [`SandboxContext::eval`]; additionally [`PluginError::Script`] when
    /// the script yields a non-string or the string is not valid JSON.
    pub fn eval_json(&self, source: &str) -> Result<serde_json::Value, PluginError> {
        let wrapped = format!(
            "(function(){{ const __out = eval({source_lit}); \
             if (typeof __out !== 'string') throw new Error('plugin handler must return a JSON string via JSON.stringify'); \
             return __out; }})()",
            source_lit = js_string_literal(source)
        );
        let text: String = self
            .ctx
            .with(|ctx| ctx.eval(wrapped).map_err(|e| classify_error(&ctx, e)))?;
        serde_json::from_str(&text).map_err(|e| PluginError::Script {
            plugin: String::new(),
            detail: format!("plugin returned invalid JSON: {e}"),
        })
    }

    /// Set a global string value (used by the host API's argument marshalling
    /// to pass a JSON argument document into the sandbox).
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::Engine`] when the global cannot be set.
    pub fn set_global_str(&self, name: &str, value: &str) -> Result<(), PluginError> {
        self.ctx
            .with(|ctx| ctx.globals().set(name, value).map_err(engine_err))
    }

    /// How long remains before the deadline fires (reads the currently-armed
    /// deadline; `0` before the first [`rearm_deadline`](Self::rearm_deadline)
    /// or once a budget has tripped).
    #[must_use]
    pub fn time_remaining(&self) -> Duration {
        self.deadline_at
            .read()
            .map(|instant| instant.saturating_duration_since(Instant::now()))
            .unwrap_or_default()
    }

    /// Clear the interrupt kill switch and arm a fresh deadline of the
    /// context's execution budget measured from now. The session calls this
    /// once before each plugin evaluation (entry, then every tool dispatch)
    /// so each execution gets its own budget window (FR-017): without it the
    /// entry budget would keep firing inside later tool calls.
    pub fn reset_interrupt(&self) {
        self.expired.store(false, Ordering::Relaxed);
        if let Ok(mut guard) = self.deadline_at.write() {
            *guard = Instant::now() + self.execution_budget;
        }
    }

    /// Alias for [`reset_interrupt`](Self::reset_interrupt) with the entry
    /// budget semantics documented in [`SandboxBudget::with_deadline`].
    pub fn rearm_deadline(&self) {
        self.reset_interrupt();
    }

    /// Direct access to the engine context for host-side wiring that needs
    /// to read back sandbox globals (tests, harness).
    #[must_use]
    pub fn raw(&self) -> &rquickjs::Context {
        &self.ctx
    }
}

/// Distinguish timeout and memory-limit aborts from ordinary script errors
/// (FR-017, FR-018). QuickJS surfaces interrupts as an "InternalError:
/// interrupted" exception and OOM as "InternalError: out of memory".
fn classify_error(ctx: &rquickjs::Ctx<'_>, err: rquickjs::Error) -> PluginError {
    let detail = catch_exception_message(ctx, &err);
    if detail.contains("interrupted") {
        PluginError::Timeout
    } else if detail.contains("out of memory") {
        PluginError::MemoryLimit
    } else {
        PluginError::Script {
            plugin: String::new(),
            detail,
        }
    }
}

/// Pull a readable message out of a QuickJS error, preferring the caught
/// exception's `message` property when the error is a thrown exception.
fn catch_exception_message(ctx: &rquickjs::Ctx<'_>, err: &rquickjs::Error) -> String {
    if let rquickjs::Error::Exception = err {
        let caught = ctx.catch();
        if let Some(exc) = caught.as_exception()
            && let Some(msg) = exc.message()
        {
            return msg;
        }
        if let Some(s) = caught.as_string() {
            return s.to_string().unwrap_or_default();
        }
    }
    err.to_string()
}

fn engine_err(e: rquickjs::Error) -> PluginError {
    PluginError::Engine(e.to_string())
}

/// Quote a JavaScript source snippet as a JS string literal so it can be
/// evaluated inside a wrapper function via the sandbox's own `eval`. Uses
/// `JSON.stringify` semantics for escaping (valid JS string literal syntax).
fn js_string_literal(source: &str) -> String {
    serde_json::to_string(source).unwrap_or_else(|_| "\"\"".to_string())
}
