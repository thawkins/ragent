//! OpenAPI 3.1 document and generated typed client for the ragent REST API
//! (spec `openhands` FR-007).
//!
//! The document is **derived from a single route table** ([`ROUTES`]) rather than
//! hand-maintained prose. Every entry carries the path, method, auth
//! requirement, and the request/response schema names, and both the JSON
//! document ([`document`]) and the generated TypeScript client
//! ([`typescript_client`]) are rendered from that one table. The companion
//! integration tests in `tests/test_openapi_document.rs` close the drift gap in
//! both directions:
//!
//! - **documented -> served:** every path in [`ROUTES`] is probed through the
//!   live router and must not answer `404 Not Found`.
//! - **served -> documented:** the route literals registered across
//!   `src/routes/` are scraped and compared to the documented paths, so a route
//!   added to [`super::router`] without a matching entry here fails the test.
//!
//! The document contains no secrets and no runtime data: it is a static shape
//! description, which is why it is served **without** the bearer-token check
//! (FR-035 forbids secrets in the document; there are none to leak). The
//! protected operations declare `bearerAuth` so a client knows to authenticate.

use axum::{Json, Router, extract::State, routing::get};
use serde_json::{Value, json};

use super::AppState;

/// One documented REST operation.
///
/// This is the single source of truth for the OpenAPI document and the
/// generated client. Keep it in sync with the route registrations in
/// [`super::router`]: the served/documented drift is asserted by
/// `tests/test_openapi_document.rs`.
#[derive(Debug, Clone, Copy)]
pub struct Operation {
    /// Absolute request path, with `{param}` placeholders for path parameters.
    pub path: &'static str,
    /// Uppercase HTTP method (`GET`, `POST`, `DELETE`, `PUT`).
    pub method: &'static str,
    /// Short operation summary used as the OpenAPI `summary`.
    pub summary: &'static str,
    /// OpenAPI tag grouping related operations.
    pub tag: &'static str,
    /// `true` when the operation requires the native bearer token (FR-024).
    pub auth: bool,
    /// Name of the request-body schema in `components.schemas`, if any.
    pub body_schema: Option<&'static str>,
    /// Name of the success-response schema in `components.schemas`, if any.
    pub response_schema: Option<&'static str>,
    /// `true` when the operation answers with a `text/event-stream`.
    pub sse: bool,
}

/// Every REST operation the ragent server serves.
///
/// `// KEEP IN SYNC WITH super::router` - the drift test asserts this both ways.
pub const ROUTES: &[Operation] = &[
    Operation {
        path: "/health",
        method: "GET",
        summary: "Liveness probe",
        tag: "system",
        auth: false,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/openapi.json",
        method: "GET",
        summary: "OpenAPI 3.1 document for this API",
        tag: "system",
        auth: false,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/config",
        method: "GET",
        summary: "Resolved configuration (credentials redacted)",
        tag: "config",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/providers",
        method: "GET",
        summary: "Registered LLM providers and their models",
        tag: "config",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/sessions",
        method: "GET",
        summary: "List sessions",
        tag: "sessions",
        auth: true,
        body_schema: None,
        response_schema: Some("SessionList"),
        sse: false,
    },
    Operation {
        path: "/sessions",
        method: "POST",
        summary: "Create a session for a project directory",
        tag: "sessions",
        auth: true,
        body_schema: Some("CreateSessionRequest"),
        response_schema: Some("Session"),
        sse: false,
    },
    Operation {
        path: "/sessions/{id}",
        method: "GET",
        summary: "Fetch one session",
        tag: "sessions",
        auth: true,
        body_schema: None,
        response_schema: Some("Session"),
        sse: false,
    },
    Operation {
        path: "/sessions/{id}",
        method: "DELETE",
        summary: "Archive a session",
        tag: "sessions",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/sessions/{id}/messages",
        method: "GET",
        summary: "List a session's messages",
        tag: "sessions",
        auth: true,
        body_schema: None,
        response_schema: Some("MessageList"),
        sse: false,
    },
    Operation {
        path: "/sessions/{id}/messages",
        method: "POST",
        summary: "Send a message and run one agent turn",
        tag: "sessions",
        auth: true,
        body_schema: Some("SendMessageRequest"),
        response_schema: Some("Message"),
        sse: false,
    },
    Operation {
        path: "/sessions/{id}/abort",
        method: "POST",
        summary: "Abort the in-flight turn for a session",
        tag: "sessions",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/sessions/{id}/permission/{req_id}",
        method: "POST",
        summary: "Answer a pending permission request",
        tag: "sessions",
        auth: true,
        body_schema: Some("PermissionReply"),
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/sessions/{id}/tasks",
        method: "GET",
        summary: "List sub-agent tasks for a session",
        tag: "tasks",
        auth: true,
        body_schema: None,
        response_schema: Some("TaskList"),
        sse: false,
    },
    Operation {
        path: "/sessions/{id}/tasks",
        method: "POST",
        summary: "Spawn a background sub-agent task",
        tag: "tasks",
        auth: true,
        body_schema: Some("SpawnTaskRequest"),
        response_schema: Some("Task"),
        sse: false,
    },
    Operation {
        path: "/sessions/{id}/tasks/{tid}",
        method: "GET",
        summary: "Fetch one sub-agent task",
        tag: "tasks",
        auth: true,
        body_schema: None,
        response_schema: Some("Task"),
        sse: false,
    },
    Operation {
        path: "/sessions/{id}/tasks/{tid}",
        method: "DELETE",
        summary: "Cancel a sub-agent task",
        tag: "tasks",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/events",
        method: "GET",
        summary: "Server-sent event stream of runtime events",
        tag: "events",
        auth: true,
        body_schema: None,
        response_schema: Some("Event"),
        sse: true,
    },
    Operation {
        path: "/memory/search",
        method: "GET",
        summary: "Search stored memories",
        tag: "memory",
        auth: true,
        body_schema: None,
        response_schema: Some("MemoryList"),
        sse: false,
    },
    Operation {
        path: "/memory/store",
        method: "POST",
        summary: "Store a memory",
        tag: "memory",
        auth: true,
        body_schema: Some("MemoryStoreRequest"),
        response_schema: Some("Memory"),
        sse: false,
    },
    Operation {
        path: "/memory/{id}",
        method: "DELETE",
        summary: "Forget a memory by id",
        tag: "memory",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/memory/visualisation",
        method: "GET",
        summary: "Memory visualisation dataset",
        tag: "memory",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/memory/visualisation/graph",
        method: "GET",
        summary: "Memory knowledge graph",
        tag: "memory",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/memory/visualisation/tags",
        method: "GET",
        summary: "Memory tag frequency",
        tag: "memory",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/memory/visualisation/heatmap",
        method: "GET",
        summary: "Memory activity heatmap",
        tag: "memory",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/research/",
        method: "GET",
        summary: "List research items",
        tag: "research",
        auth: true,
        body_schema: None,
        response_schema: Some("ResearchList"),
        sse: false,
    },
    Operation {
        path: "/research/",
        method: "POST",
        summary: "Create a research item",
        tag: "research",
        auth: true,
        body_schema: Some("ResearchCreateRequest"),
        response_schema: Some("Research"),
        sse: false,
    },
    Operation {
        path: "/research/{name}",
        method: "GET",
        summary: "Show one research item",
        tag: "research",
        auth: true,
        body_schema: None,
        response_schema: Some("Research"),
        sse: false,
    },
    Operation {
        path: "/research/{name}",
        method: "PUT",
        summary: "Update a research item",
        tag: "research",
        auth: true,
        body_schema: Some("ResearchCreateRequest"),
        response_schema: Some("Research"),
        sse: false,
    },
    Operation {
        path: "/research/{name}",
        method: "DELETE",
        summary: "Delete a research item",
        tag: "research",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/research/{name}/events",
        method: "GET",
        summary: "Server-sent progress events for a research run",
        tag: "research",
        auth: true,
        body_schema: None,
        response_schema: Some("Event"),
        sse: true,
    },
    Operation {
        path: "/v1/models",
        method: "GET",
        summary: "OpenAI-compatible model list",
        tag: "openai",
        auth: true,
        body_schema: None,
        response_schema: Some("OpenAiModelList"),
        sse: false,
    },
    Operation {
        path: "/v1/chat/completions",
        method: "POST",
        summary: "OpenAI-compatible chat completion (stream:false JSON; stream:true SSE)",
        tag: "openai",
        auth: true,
        body_schema: Some("OpenAiChatCompletionRequest"),
        response_schema: Some("OpenAiChatCompletion"),
        sse: false,
    },
    Operation {
        path: "/orchestrator/start",
        method: "POST",
        summary: "Start an orchestration job",
        tag: "orchestrator",
        auth: true,
        body_schema: Some("OrchestrateRequest"),
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/orchestrator/jobs/{id}",
        method: "GET",
        summary: "Fetch an orchestration job",
        tag: "orchestrator",
        auth: true,
        body_schema: None,
        response_schema: None,
        sse: false,
    },
    Operation {
        path: "/automation",
        method: "GET",
        summary: "List configured automations and their next-due times",
        tag: "automation",
        auth: true,
        body_schema: None,
        response_schema: Some("AutomationList"),
        sse: false,
    },
    Operation {
        path: "/automation/runs/{id}",
        method: "GET",
        summary: "List an automation's run history",
        tag: "automation",
        auth: true,
        body_schema: None,
        response_schema: Some("AutomationRunList"),
        sse: false,
    },
    Operation {
        path: "/automation/{id}/run",
        method: "POST",
        summary: "Enqueue a manual automation run",
        tag: "automation",
        auth: true,
        body_schema: None,
        response_schema: Some("AutomationRun"),
        sse: false,
    },
    Operation {
        path: "/auto/{id}",
        method: "POST",
        summary: "Automation webhook ingress",
        tag: "automation",
        auth: false,
        body_schema: Some("AutomationWebhookRequest"),
        response_schema: Some("AutomationRun"),
        sse: false,
    },
];

/// Build the OpenAPI 3.1 document for this server.
///
/// The document is rendered from [`ROUTES`] and the static schema table in
/// [`schemas`]; it carries no runtime configuration and therefore cannot leak a
/// credential (FR-035).
#[must_use]
pub fn document() -> Value {
    let mut paths = serde_json::Map::new();

    for op in ROUTES {
        let entry = paths
            .entry(op.path.to_string())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        if let Value::Object(methods) = entry {
            methods.insert(op.method.to_lowercase(), operation_object(op));
        }
    }

    let mut ordered_paths = serde_json::Map::new();
    let mut keys: Vec<&String> = paths.keys().collect();
    keys.sort();
    for key in keys {
        if let Some(value) = paths.get(key) {
            ordered_paths.insert(key.clone(), value.clone());
        }
    }

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "ragent REST API",
            "summary": "Native REST + SSE API of the ragent coding agent",
            "description": "The canonical ragent HTTP surface, plus the \
                OpenAI-compatible adapter under `/v1`. Every operation marked \
                with the `bearerAuth` security scheme requires \
                `Authorization: Bearer <token>`.",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "servers": [{ "url": "/" }],
        "paths": ordered_paths,
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "Native API token issued at server start.",
                }
            },
            "schemas": schemas(),
        },
        "tags": tag_list(),
    })
}

/// Render one operation object for the `paths` map.
fn operation_object(op: &Operation) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("summary".to_string(), json!(op.summary));
    obj.insert("operationId".to_string(), json!(operation_id(op)));
    obj.insert("tags".to_string(), json!([op.tag]));

    let params: Vec<Value> = path_params(op.path)
        .into_iter()
        .map(|name| {
            json!({
                "name": name,
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
            })
        })
        .collect();
    if !params.is_empty() {
        obj.insert("parameters".to_string(), Value::Array(params));
    }

    if op.auth {
        obj.insert("security".to_string(), json!([{ "bearerAuth": [] }]));
    }

    if let Some(schema) = op.body_schema {
        obj.insert(
            "requestBody".to_string(),
            json!({
                "required": true,
                "content": {
                    "application/json": { "schema": { "$ref": format!("#/components/schemas/{schema}") } }
                }
            }),
        );
    }

    let (status, media) = if op.sse {
        (
            "200",
            json!({ "text/event-stream": { "schema": { "type": "string" } } }),
        )
    } else if let Some(schema) = op.response_schema {
        (
            "200",
            json!({
                "application/json": { "schema": { "$ref": format!("#/components/schemas/{schema}") } }
            }),
        )
    } else {
        (
            "200",
            json!({ "application/json": { "schema": { "type": "object" } } }),
        )
    };

    let mut responses = serde_json::Map::new();
    if op.sse {
        responses.insert(
            status.to_string(),
            json!({ "description": "Successful response", "content": media }),
        );
    } else if op.path == "/v1/chat/completions" {
        // `POST /v1/chat/completions` answers either an `application/json`
        // `chat.completion` (stream:false, FR-011) or a `text/event-stream` of
        // `chat.completion.chunk` events (stream:true, FR-012), so both media
        // types are advertised.
        responses.insert(
            status.to_string(),
            json!({
                "description": "OpenAI completion (JSON) or chat.completion.chunk SSE, \
                    terminated by data: [DONE], when the request sets stream:true",
                "content": {
                    "application/json": { "schema": { "$ref": "#/components/schemas/OpenAiChatCompletion" } },
                    "text/event-stream": { "schema": { "type": "string" } }
                }
            }),
        );
    } else {
        responses.insert(
            status.to_string(),
            json!({ "description": "Successful response", "content": media }),
        );
    }
    if op.auth {
        responses.insert(
            "401".to_string(),
            json!({
                "description": "Missing or invalid bearer token",
                "content": {
                    "application/json": { "schema": { "$ref": "#/components/schemas/Error" } }
                }
            }),
        );
    }
    obj.insert("responses".to_string(), Value::Object(responses));

    Value::Object(obj)
}

/// The OpenAPI `tags` array: one entry per distinct tag used by [`ROUTES`],
/// in sorted order.
#[must_use]
pub fn tag_list() -> Value {
    let mut tags: Vec<&str> = ROUTES.iter().map(|op| op.tag).collect();
    tags.sort_unstable();
    tags.dedup();
    json!(
        tags.into_iter()
            .map(|name| json!({ "name": name }))
            .collect::<Vec<_>>()
    )
}

/// Extract `{param}` names from a path, in order of appearance.
#[must_use]
pub fn path_params(path: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut rest = path;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) => {
                params.push(after[..end].to_string());
                rest = &after[end + 1..];
            }
            None => break,
        }
    }
    params
}

/// Stable operation id: lowercased method followed by the path segments in
/// PascalCase (`GET /sessions/{id}/messages` -> `getSessionsIdMessages`).
#[must_use]
pub fn operation_id(op: &Operation) -> String {
    let mut id = op.method.to_lowercase();
    for segment in op.path.split('/').filter(|s| !s.is_empty()) {
        id.push_str(&pascal_case(segment));
    }
    id
}

/// `id` -> `Id`; `{req_id}` -> `ReqId`; `v1` -> `V1`.
fn pascal_case(segment: &str) -> String {
    let cleaned: String = segment
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    let mut out = String::new();
    for part in cleaned.split('_').filter(|p| !p.is_empty()) {
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

/// The static JSON-Schema component table.
fn schemas() -> Value {
    json!({
        "Error": {
            "type": "object",
            "properties": { "error": { "type": "string" } },
            "required": ["error"]
        },
        "Session": {
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "title": { "type": "string" },
                "directory": { "type": "string" },
                "created_at": { "type": "string" },
                "updated_at": { "type": "string" },
                "summary": { "type": ["string", "null"] }
            },
            "required": ["id", "title", "directory", "created_at", "updated_at"]
        },
        "SessionList": { "type": "array", "items": { "$ref": "#/components/schemas/Session" } },
        "Message": {
            "type": "object",
            "properties": {
                "role": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["role", "content"]
        },
        "MessageList": { "type": "array", "items": { "$ref": "#/components/schemas/Message" } },
        "CreateSessionRequest": {
            "type": "object",
            "properties": { "directory": { "type": "string" } },
            "required": ["directory"]
        },
        "SendMessageRequest": {
            "type": "object",
            "properties": { "content": { "type": "string" } },
            "required": ["content"]
        },
        "PermissionReply": {
            "type": "object",
            "properties": {
                "decision": { "type": "string", "enum": ["allow", "deny"] },
                "reason": { "type": "string" }
            },
            "required": ["decision"]
        },
        "SpawnTaskRequest": {
            "type": "object",
            "properties": {
                "agent": { "type": "string" },
                "prompt": { "type": "string" }
            },
            "required": ["agent", "prompt"]
        },
        "Task": {
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "agent": { "type": "string" },
                "status": { "type": "string" },
                "background": { "type": "boolean" }
            }
        },
        "TaskList": { "type": "array", "items": { "$ref": "#/components/schemas/Task" } },
        "Event": {
            "type": "object",
            "description": "A serialized runtime event; the schema is open.",
            "additionalProperties": true
        },
        "MemoryStoreRequest": {
            "type": "object",
            "properties": {
                "content": { "type": "string" },
                "category": { "type": "string" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "confidence": { "type": "number" }
            },
            "required": ["content", "category"]
        },
        "Memory": {
            "type": "object",
            "properties": {
                "id": { "type": "integer" },
                "content": { "type": "string" },
                "category": { "type": "string" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "confidence": { "type": "number" }
            }
        },
        "MemoryList": { "type": "array", "items": { "$ref": "#/components/schemas/Memory" } },
        "ResearchCreateRequest": {
            "type": "object",
            "description": "Research creation/update payload; optional fields omitted.",
            "additionalProperties": true
        },
        "Research": {
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "status": { "type": "string" }
            }
        },
        "ResearchList": { "type": "array", "items": { "$ref": "#/components/schemas/Research" } },
        "OrchestrateRequest": {
            "type": "object",
            "properties": { "prompt": { "type": "string" } },
            "required": ["prompt"]
        },
        "Automation": {
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "name": { "type": "string" },
                "trigger": { "type": "string", "enum": ["webhook", "schedule"] },
                "schedule": { "type": ["string", "null"] },
                "backend": { "type": "string" },
                "enabled": { "type": "boolean" },
                "next_due": { "type": ["string", "null"] },
                "running": { "type": "boolean" }
            },
            "required": ["id", "trigger", "backend", "enabled"]
        },
        "AutomationList": {
            "type": "array",
            "items": { "$ref": "#/components/schemas/Automation" }
        },
        "AutomationRun": {
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "automation_id": { "type": "string" },
                "trigger": { "type": "string", "enum": ["webhook", "schedule", "manual"] },
                "started_at": { "type": "string" },
                "ended_at": { "type": ["string", "null"] },
                "outcome": { "type": ["string", "null"], "enum": ["success", "error", "cancelled", null] },
                "backend": { "type": "string" },
                "output_ref": { "type": ["string", "null"] }
            },
            "required": ["id", "automation_id", "trigger", "started_at", "backend"]
        },
        "AutomationRunList": {
            "type": "array",
            "items": { "$ref": "#/components/schemas/AutomationRun" }
        },
        "AutomationWebhookRequest": {
            "type": "object",
            "description": "Arbitrary JSON webhook payload; supplied to the run as prompt context.",
            "additionalProperties": true
        },
        "OpenAiChatCompletionRequest": {
            "type": "object",
            "properties": {
                "model": { "type": "string" },
                "stream": { "type": "boolean", "default": false },
                "messages": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "role": { "type": "string", "enum": ["system", "user", "assistant"] },
                            "content": {
                                "oneOf": [
                                    { "type": "string" },
                                    {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "properties": {
                                                "type": { "type": "string" },
                                                "text": { "type": "string" }
                                            }
                                        }
                                    }
                                ]
                            }
                        },
                        "required": ["role", "content"]
                    }
                }
            },
            "required": ["messages"]
        },
        "OpenAiChatCompletion": {
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "object": { "type": "string", "const": "chat.completion" },
                "created": { "type": "integer" },
                "model": { "type": "string" },
                "choices": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "index": { "type": "integer" },
                            "message": { "$ref": "#/components/schemas/Message" },
                            "finish_reason": { "type": "string" }
                        }
                    }
                },
                "usage": { "$ref": "#/components/schemas/OpenAiUsage" }
            }
        },
        "OpenAiUsage": {
            "type": "object",
            "properties": {
                "prompt_tokens": { "type": "integer" },
                "completion_tokens": { "type": "integer" },
                "total_tokens": { "type": "integer" }
            }
        },
        "OpenAiModelList": {
            "type": "object",
            "properties": {
                "object": { "type": "string", "const": "list" },
                "data": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "object": { "type": "string", "const": "model" },
                            "created": { "type": "integer" },
                            "owned_by": { "type": "string" }
                        }
                    }
                }
            }
        }
    })
}

/// Render a dependency-free, typed TypeScript client for this API.
///
/// The client is generated from [`ROUTES`] so it cannot describe an operation
/// the server does not serve. It uses the global `fetch` and adds no runtime
/// dependency. The checked-in copy lives at
/// `docs/openapi/ragent-client.d.ts` and is guarded against drift by
/// `tests/test_openapi_document.rs`.
#[must_use]
pub fn typescript_client() -> String {
    let mut out = String::new();
    out.push_str("// AUTO-GENERATED from crates/ragent-server/src/routes/openapi.rs\n");
    out.push_str(
        "// Do not edit by hand. Regenerate with `ragent openapi --client typescript`.\n\n",
    );
    out.push_str("export interface RagentClientOptions {\n");
    out.push_str("  baseUrl: string;\n");
    out.push_str("  token?: string;\n");
    out.push_str("  fetch?: typeof fetch;\n");
    out.push_str("}\n\n");
    out.push_str("export class RagentClient {\n");
    out.push_str("  private readonly options: RagentClientOptions;\n\n");
    out.push_str("  constructor(options: RagentClientOptions) {\n");
    out.push_str("    this.options = options;\n");
    out.push_str("  }\n\n");
    out.push_str("  private async request<T>(\n");
    out.push_str("    method: string,\n");
    out.push_str("    path: string,\n");
    out.push_str("    body?: unknown,\n");
    out.push_str("    auth = true,\n");
    out.push_str("  ): Promise<T> {\n");
    out.push_str("    const doFetch = this.options.fetch ?? fetch;\n");
    out.push_str("    const headers: Record<string, string> = { Accept: 'application/json' };\n");
    out.push_str("    if (body !== undefined) headers['Content-Type'] = 'application/json';\n");
    out.push_str("    if (auth && this.options.token) headers['Authorization'] = `Bearer ${this.options.token}`;\n");
    out.push_str("    const response = await doFetch(`${this.options.baseUrl}${path}`, {\n");
    out.push_str("      method,\n");
    out.push_str("      headers,\n");
    out.push_str("      body: body === undefined ? undefined : JSON.stringify(body),\n");
    out.push_str("    });\n");
    out.push_str("    if (!response.ok) {\n");
    out.push_str("      throw new Error(`${method} ${path} failed with ${response.status}`);\n");
    out.push_str("    }\n");
    out.push_str("    return (await response.json()) as T;\n");
    out.push_str("  }\n");

    for op in ROUTES {
        out.push('\n');
        out.push_str(&format!(
            "  /** {} {} - {} */\n",
            op.method, op.path, op.summary
        ));
        let params = path_params(op.path);
        let args = params
            .iter()
            .map(|p| format!("{p}: string"))
            .chain(op.body_schema.map(|_| "body: unknown".to_string()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut template_path = op.path.to_string();
        for p in &params {
            template_path = template_path.replace(
                &format!("{{{p}}}"),
                &format!("${{encodeURIComponent({p})}}"),
            );
        }
        let body_arg = if op.body_schema.is_some() {
            ", body"
        } else {
            ""
        };
        out.push_str(&format!(
            "  async {}({args}): Promise<unknown> {{\n",
            operation_id(op)
        ));
        out.push_str(&format!(
            "    return this.request<unknown>('{}', `{}`{}, {});\n",
            op.method, template_path, body_arg, op.auth
        ));
        out.push_str("  }\n");
    }

    out.push_str("}\n");
    out
}

/// `GET /openapi.json` handler.
async fn openapi_json(State(_state): State<AppState>) -> Json<Value> {
    Json(document())
}

/// Build the OpenAPI sub-router.
///
/// Mounted on the *public* router: the document describes the API shape and
/// contains no runtime data or credentials, so it is safe to serve without the
/// bearer token (a client needs it before it can authenticate).
pub fn openapi_routes() -> Router<AppState> {
    Router::new().route("/openapi.json", get(openapi_json))
}
