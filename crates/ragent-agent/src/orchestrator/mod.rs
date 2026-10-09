//! Multi-agent orchestration primitives (MVP)
//!
//! Provides an in-process `AgentRegistry`, an `InProcessRouter` (actor-style
//! inboxes), and a Coordinator that can start jobs synchronously and
//! asynchronously with basic negotiation and aggregation strategies.
//!
//! ## Submodules (Milestone 5 extensions)
//! - [`transport`] - pluggable transport adapters (`HttpRouter`, `RouterComposite`)
//! - [`leader`]    - in-process leader election and `CoordinatorCluster`
//! - [`policy`]    - conflict resolution policies and human-in-the-loop fallbacks

/// Coordination layer for orchestrating multi-agent workflows.
pub mod coordinator;
pub mod leader;
pub mod policy;
/// Agent registry for managing agent instances.
pub mod registry;
/// Routing infrastructure for message passing.
pub mod router;
pub mod transport;

// Re-export common orchestrator types for backwards compatibility.
pub use coordinator::{
    Coordinator, JobDescriptor, JobEvent, MetricsSnapshot, OrchestrationMessage,
};
pub use registry::OrchestrationRequest;
pub use registry::{AgentEntry, AgentId, AgentRegistry, Responder};
pub use router::{InProcessRouter, Router};

#[cfg(test)]
#[path = "../../tests/inline/orchestrator_mod_tests.rs"]
mod tests;
