use std::sync::Arc;

use crossterm::event::KeyEvent;

use ragent_core::{
    event::{Event, EventBus},
    message::{Message, MessagePart, Role},
    permission::PermissionRequest,
};

use crate::input::{self, InputAction};

pub struct App {
    pub messages: Vec<Message>,
    pub input: String,
    pub scroll_offset: u16,
    pub is_running: bool,
    pub event_bus: Arc<EventBus>,
    pub session_id: Option<String>,
    pub agent_name: String,
    pub status: String,
    pub permission_pending: Option<PermissionRequest>,
    pub token_usage: (u64, u64),
}

impl App {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            scroll_offset: 0,
            is_running: true,
            event_bus,
            session_id: None,
            agent_name: "general".to_string(),
            status: "ready".to_string(),
            permission_pending: None,
            token_usage: (0, 0),
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) {
        if let Some(action) = input::handle_key(self, key) {
            match action {
                InputAction::SendMessage(text) => {
                    if let Some(ref sid) = self.session_id {
                        let msg = Message::user_text(sid, &text);
                        self.messages.push(msg);
                    }
                    self.input.clear();
                    self.status = "processing...".to_string();
                }
                InputAction::Quit => {
                    self.is_running = false;
                }
                InputAction::ScrollUp => {
                    self.scroll_offset = self.scroll_offset.saturating_add(3);
                }
                InputAction::ScrollDown => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(3);
                }
                InputAction::SwitchAgent => {
                    // Cycle through agents — placeholder
                }
                InputAction::SlashCommand(_cmd) => {
                    // Handle slash commands — placeholder
                }
            }
        }
    }

    pub fn handle_event(&mut self, event: Event) {
        match event {
            Event::SessionCreated { ref session_id } => {
                if self.session_id.is_none() {
                    self.session_id = Some(session_id.clone());
                }
            }
            Event::TextDelta {
                ref session_id,
                ref text,
            } => {
                if self.is_current_session(session_id) {
                    self.append_assistant_text(text);
                }
            }
            Event::ReasoningDelta {
                ref session_id,
                ref text,
            } => {
                if self.is_current_session(session_id) {
                    self.append_reasoning_text(text);
                }
            }
            Event::ToolCallStart {
                ref session_id,
                ref call_id,
                ref tool,
            } => {
                if self.is_current_session(session_id) {
                    self.add_tool_call_part(tool, call_id);
                    self.status = format!("running: {}", tool);
                }
            }
            Event::ToolCallEnd {
                ref session_id,
                ref call_id,
                ref error,
                ..
            } => {
                if self.is_current_session(session_id) {
                    self.update_tool_call_status(call_id, error.is_none());
                }
            }
            Event::MessageStart {
                ref session_id, ..
            } => {
                if self.is_current_session(session_id) {
                    self.status = "thinking...".to_string();
                }
            }
            Event::MessageEnd {
                ref session_id, ..
            } => {
                if self.is_current_session(session_id) {
                    self.status = "ready".to_string();
                }
            }
            Event::PermissionRequested {
                ref session_id,
                ref request_id,
                ref permission,
                ref description,
            } => {
                if self.is_current_session(session_id) {
                    self.permission_pending = Some(PermissionRequest {
                        id: request_id.clone(),
                        session_id: session_id.clone(),
                        permission: permission.clone(),
                        patterns: vec![description.clone()],
                        metadata: serde_json::Value::Null,
                        tool_call_id: None,
                    });
                    self.status = "awaiting permission".to_string();
                }
            }
            Event::PermissionReplied {
                ref session_id, ..
            } => {
                if self.is_current_session(session_id) {
                    self.permission_pending = None;
                    self.status = "processing...".to_string();
                }
            }
            Event::AgentSwitched {
                ref session_id,
                ref to,
                ..
            } => {
                if self.is_current_session(session_id) {
                    self.agent_name = to.clone();
                }
            }
            Event::AgentError {
                ref session_id,
                ref error,
            } => {
                if self.is_current_session(session_id) {
                    self.status = format!("error: {}", error);
                }
            }
            Event::TokenUsage {
                ref session_id,
                input_tokens,
                output_tokens,
            } => {
                if self.is_current_session(session_id) {
                    self.token_usage.0 += input_tokens;
                    self.token_usage.1 += output_tokens;
                }
            }
            _ => {}
        }
    }

    fn is_current_session(&self, session_id: &str) -> bool {
        self.session_id.as_deref() == Some(session_id)
    }

    fn append_assistant_text(&mut self, text: &str) {
        if let Some(last) = self.messages.last_mut()
            && last.role == Role::Assistant
        {
            // Append to the last text part if it exists
            for part in last.parts.iter_mut().rev() {
                if let MessagePart::Text { text: t } = part {
                    t.push_str(text);
                    return;
                }
            }
            // No text part yet, add one
            last.parts.push(MessagePart::Text {
                text: text.to_string(),
            });
            return;
        }
        // Create new assistant message
        if let Some(ref sid) = self.session_id {
            let msg = Message::new(
                sid,
                Role::Assistant,
                vec![MessagePart::Text {
                    text: text.to_string(),
                }],
            );
            self.messages.push(msg);
        }
    }

    fn append_reasoning_text(&mut self, text: &str) {
        if let Some(last) = self.messages.last_mut()
            && last.role == Role::Assistant
        {
            for part in last.parts.iter_mut().rev() {
                if let MessagePart::Reasoning { text: t } = part {
                    t.push_str(text);
                    return;
                }
            }
            last.parts.push(MessagePart::Reasoning {
                text: text.to_string(),
            });
            return;
        }
        if let Some(ref sid) = self.session_id {
            let msg = Message::new(
                sid,
                Role::Assistant,
                vec![MessagePart::Reasoning {
                    text: text.to_string(),
                }],
            );
            self.messages.push(msg);
        }
    }

    fn add_tool_call_part(&mut self, tool: &str, call_id: &str) {
        use ragent_core::message::{ToolCallState, ToolCallStatus};

        if let Some(last) = self.messages.last_mut()
            && last.role == Role::Assistant
        {
            last.parts.push(MessagePart::ToolCall {
                tool: tool.to_string(),
                call_id: call_id.to_string(),
                state: ToolCallState {
                    status: ToolCallStatus::Running,
                    input: serde_json::Value::Null,
                    output: None,
                    error: None,
                    duration_ms: None,
                },
            });
            return;
        }
        if let Some(ref sid) = self.session_id {
            let msg = Message::new(
                sid,
                Role::Assistant,
                vec![MessagePart::ToolCall {
                    tool: tool.to_string(),
                    call_id: call_id.to_string(),
                    state: ToolCallState {
                        status: ToolCallStatus::Running,
                        input: serde_json::Value::Null,
                        output: None,
                        error: None,
                        duration_ms: None,
                    },
                }],
            );
            self.messages.push(msg);
        }
    }

    fn update_tool_call_status(&mut self, call_id: &str, success: bool) {
        use ragent_core::message::ToolCallStatus;

        for msg in self.messages.iter_mut().rev() {
            for part in msg.parts.iter_mut() {
                if let MessagePart::ToolCall {
                    call_id: cid,
                    state,
                    ..
                } = part
                    && cid == call_id
                {
                    state.status = if success {
                        ToolCallStatus::Completed
                    } else {
                        ToolCallStatus::Error
                    };
                    return;
                }
            }
        }
    }
}
