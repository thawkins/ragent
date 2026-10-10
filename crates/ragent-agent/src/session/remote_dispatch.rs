//! Remote-backend turn dispatch (spec `openhands` T-003; FR-001, FR-004, FR-009,
//! FR-021, FR-031, FR-034).
//!
//! When the active execution backend is `remote`, the whole turn is relayed to
//! that backend's ragent server over its REST+SSE API instead of being run
//! locally: the session processor opens a turn against the remote server and
//! mirrors the server's event stream back onto the **local** event bus (FR-021)
//! so the TUI renders a remote turn exactly like a local one. No tool from the
//! turn is executed on this host, and a failed relay never re-runs it locally
//! (FR-031, FR-034).
//!
//! The transport and stream decoder live in [`crate::backend::remote`](crate::backend::remote);
//! this module owns detection and the bridge from a
//! [`RemoteUpdate`](crate::backend::RemoteUpdate) onto the local bus.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use ragent_config::BackendConfig;

use crate::backend::BackendError;
use crate::backend::remote::{self, RemoteError, RemoteUpdate};
use crate::event::{Event, EventBus, FinishReason};
use crate::message::{Message, MessagePart, Role};
use crate::session::processor::SessionProcessor;

/// Resolve the remote backend descriptor the active session is configured for.
///
/// Returns `Some` only when the effective execution backend is `remote` (FR-001);
/// a bare `remote` label with no registered entry yields a default descriptor (no
/// URL), which the relay fails at provisioning rather than falling back to host
/// execution (FR-031).
#[must_use]
pub fn remote_backend_for_config(config: &ragent_config::Config) -> Option<BackendConfig> {
    remote::remote_backend_config(config)
}

/// Translate one streamed remote update into a local event-bus publication.
///
/// Every event is re-published under the **local** session id, because the TUI
/// filters on it; the remote session id is an implementation detail of the
/// transport. `MessageEnd` is handled by the caller, which knows the assistant
/// placeholder id, so it is a no-op here.
fn publish_update(bus: &EventBus, session_id: &str, update: RemoteUpdate) {
    match update {
        RemoteUpdate::SessionCreated => {}
        RemoteUpdate::MessageStart { message_id } => {
            bus.publish(Event::MessageStart {
                session_id: session_id.to_string(),
                message_id,
            });
        }
        RemoteUpdate::TextDelta { text } => {
            bus.publish(Event::TextDelta {
                session_id: session_id.to_string(),
                text,
            });
        }
        RemoteUpdate::ReasoningDelta { text } => {
            bus.publish(Event::ReasoningDelta {
                session_id: session_id.to_string(),
                text,
            });
        }
        RemoteUpdate::ToolCallStart { call_id, tool } => {
            bus.publish(Event::ToolCallStart {
                session_id: session_id.to_string(),
                call_id,
                tool,
            });
        }
        RemoteUpdate::ToolCallEnd {
            call_id,
            tool,
            error,
            duration_ms,
        } => {
            bus.publish(Event::ToolCallEnd {
                session_id: session_id.to_string(),
                call_id,
                tool,
                error,
                duration_ms,
            });
        }
        RemoteUpdate::ToolResult {
            call_id,
            tool,
            content,
            content_line_count,
            metadata,
            success,
        } => {
            bus.publish(Event::ToolResult {
                session_id: session_id.to_string(),
                call_id,
                tool,
                content,
                content_line_count,
                metadata,
                success,
            });
        }
        RemoteUpdate::AgentNotice { message } => {
            bus.publish(Event::AgentNotice {
                session_id: session_id.to_string(),
                message,
            });
        }
        RemoteUpdate::AgentError { error } => {
            bus.publish(Event::AgentError {
                session_id: session_id.to_string(),
                error,
            });
        }
        RemoteUpdate::PermissionRequested {
            request_id,
            permission,
            description,
            options,
        } => {
            bus.publish(Event::PermissionRequested {
                session_id: session_id.to_string(),
                request_id,
                permission,
                description,
                options,
            });
        }
        RemoteUpdate::QuestionRequested {
            request_id,
            question,
            options,
        } => {
            bus.publish(Event::QuestionRequested {
                session_id: session_id.to_string(),
                request_id,
                question,
                options,
            });
        }
        RemoteUpdate::MessageEnd { .. } => {}
        RemoteUpdate::Other { .. } => {}
    }
}

/// Map a remote terminal reason label onto a [`FinishReason`].
fn finish_reason_for(label: &str) -> FinishReason {
    FinishReason::from_label(label)
}

impl SessionProcessor {
    /// Whether the active session should be driven against a remote backend.
    ///
    /// The session's execution-backend override is applied first (FR-033), so an
    /// automation run confined to a `remote` backend relays its turn even when
    /// the global configuration does not select `remote`.
    #[must_use]
    pub(crate) fn remote_backend_for_active(&self, session_id: &str) -> Option<BackendConfig> {
        let config = crate::automation::apply_session_backend_override(
            &self.load_config_cached(),
            session_id,
        );
        remote_backend_for_config(&config)
    }

    /// Relay one turn to the configured remote backend and mirror its event
    /// stream into the local TUI (FR-021).
    ///
    /// The user message has already been persisted and announced by the caller.
    /// This stores an assistant placeholder, opens the remote turn, publishes each
    /// mirrored event, then persists the final assistant text and publishes
    /// `MessageEnd`. A relay failure (unreachable server, dropped stream, malformed
    /// frame, timeout) is surfaced as an `AgentError` and returned as an `Err`,
    /// failing the turn with no local re-execution (FR-031, FR-034).
    ///
    /// # Errors
    ///
    /// Returns the underlying [`RemoteError`](crate::backend::RemoteError) wrapped
    /// in an [`anyhow::Error`] when the relay fails, or a storage error when the
    /// turn cannot be persisted.
    pub(crate) async fn process_remote_turn(
        &self,
        session_id: &str,
        descriptor: &BackendConfig,
        user_msg: &Message,
        cancel_flag: Arc<AtomicBool>,
    ) -> anyhow::Result<Message> {
        // 1. Persist an assistant placeholder so the turn is visible/resumable
        //    while the remote server streams.
        let placeholder = Message::new(session_id, Role::Assistant, Vec::new());
        let assistant_msg_id = placeholder.id.clone();
        {
            let stored = placeholder.clone();
            self.storage_op(move |s| s.create_message(&stored)).await?;
        }

        let display_name = descriptor.display_name().to_string();
        self.event_bus.publish(Event::AgentNotice {
            session_id: session_id.to_string(),
            message: format!("relaying turn to remote backend '{display_name}'"),
        });

        // 2. Resolve the session working directory advertised to the remote server.
        let working_dir = self
            .session_manager
            .get_session(session_id)?
            .map_or_else(|| PathBuf::from("."), |session| session.directory);

        // 3. Relay the turn, mirroring each streamed update through the bus. The
        //    bearer key is resolved from the session's encrypted credential store
        //    at spawn time (FR-010) rather than read from the descriptor; a named
        //    credential the store does not hold fails the turn rather than relaying
        //    it unauthenticated (FR-010, FR-031).
        let resolver =
            crate::backend::StorageSecretResolver::new(self.session_manager.storage().clone());
        let backend = match remote::RemoteBackend::resolve_credentials(descriptor, &resolver) {
            Ok(backend) => backend,
            Err(error) => {
                return Err(self.fail_turn(
                    session_id,
                    &assistant_msg_id,
                    error,
                    &display_name,
                    FinishReason::Stop,
                ));
            }
        };
        let prompt = user_msg.text_content();
        let bus = self.event_bus.clone();
        let sid = session_id.to_string();
        let mut on_update = move |update: RemoteUpdate| publish_update(&bus, &sid, update);

        let outcome = remote::relay_turn(
            &backend,
            session_id,
            &working_dir,
            &prompt,
            cancel_flag.as_ref(),
            &mut on_update,
        )
        .await;

        // 4. Persist the final assistant message (the mirrored text).
        let (text, reason, failure): (String, FinishReason, Option<RemoteError>) = match outcome {
            Ok(outcome) => (outcome.text, finish_reason_for(&outcome.stop_reason), None),
            Err(error) => (String::new(), FinishReason::Stop, Some(error)),
        };
        let mut assistant_msg = Message::new(
            session_id,
            Role::Assistant,
            vec![MessagePart::Text { text }],
        );
        assistant_msg.id = assistant_msg_id.clone();
        self.storage_op(move |s| s.update_message(&assistant_msg))
            .await?;

        // 5. Publish the terminal event and, on failure, the error.
        match failure {
            None => {
                self.event_bus.publish(Event::MessageEnd {
                    session_id: session_id.to_string(),
                    message_id: assistant_msg_id,
                    reason,
                });
                Ok(Message::new(session_id, Role::Assistant, Vec::new()))
            }
            Some(error) => {
                // The turn is cancelled, not failed: the user asked to stop, so
                // report it as a cancellation and do not shout an error.
                let reason = if error.kind() == remote::RemoteErrorKind::Cancelled {
                    FinishReason::Cancelled
                } else {
                    FinishReason::Stop
                };
                Err(self.fail_turn(session_id, &assistant_msg_id, error, &display_name, reason))
            }
        }
    }

    /// Publish the `AgentError` + terminal `MessageEnd` for a failed remote relay,
    /// then return the backend error (FR-034).
    fn fail_turn(
        &self,
        session_id: &str,
        assistant_msg_id: &str,
        error: RemoteError,
        display_name: &str,
        reason: FinishReason,
    ) -> anyhow::Error {
        let backend_error = BackendError::from(error);
        self.event_bus.publish(Event::AgentError {
            session_id: session_id.to_string(),
            error: describe(&backend_error, display_name),
        });
        self.event_bus.publish(Event::MessageEnd {
            session_id: session_id.to_string(),
            message_id: assistant_msg_id.to_string(),
            reason,
        });
        backend_error.into()
    }
}

/// Render a remote relay failure with the backend display name, for the TUI.
fn describe(error: &BackendError, display_name: &str) -> String {
    format!("remote backend '{display_name}': {error}")
}
