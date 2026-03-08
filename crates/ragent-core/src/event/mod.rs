use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    ToolUse,
    Length,
    ContentFilter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    SessionCreated {
        session_id: String,
    },
    SessionUpdated {
        session_id: String,
    },
    MessageStart {
        session_id: String,
        message_id: String,
    },
    TextDelta {
        session_id: String,
        text: String,
    },
    ReasoningDelta {
        session_id: String,
        text: String,
    },
    ToolCallStart {
        session_id: String,
        call_id: String,
        tool: String,
    },
    ToolCallEnd {
        session_id: String,
        call_id: String,
        tool: String,
        error: Option<String>,
        duration_ms: u64,
    },
    MessageEnd {
        session_id: String,
        message_id: String,
        reason: FinishReason,
    },
    PermissionRequested {
        session_id: String,
        request_id: String,
        permission: String,
        description: String,
    },
    PermissionReplied {
        session_id: String,
        request_id: String,
        allowed: bool,
    },
    AgentSwitched {
        session_id: String,
        from: String,
        to: String,
    },
    AgentError {
        session_id: String,
        error: String,
    },
    McpStatusChanged {
        server_id: String,
        status: String,
    },
    TokenUsage {
        session_id: String,
        input_tokens: u64,
        output_tokens: u64,
    },
}

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<Event>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.sender.subscribe()
    }

    pub fn publish(&self, event: Event) {
        // Ignore error when there are no subscribers
        let _ = self.sender.send(event);
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}
