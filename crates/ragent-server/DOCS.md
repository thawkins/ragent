# ragent-server

Axum-based HTTP server and SSE streaming API for ragent. Exposes REST endpoints
and server-sent events so any frontend can drive the agent.

## Workspace Dependencies

- ragent-agent
- ragent-research
- ragent-llm
- ragent-types
- ragent-config
- ragent-storage
- ragent-tools-core
- ragent-tools-extended
- ragent-tools-vcs
- ragent-prompt_opt
- ragent-telemetry

## External Dependencies

- axum
- tokio
- serde
- serde_json
- anyhow
- tracing
- tower
- tower-http
- hyper

## Public API (crate root)

### Modules

- **routes** — HTTP route handlers (session, message, model, config, research, spec, etc.).
- **sse** — Server-sent event streaming for real-time session updates.
- **state** — Shared server state (`AppState`) holding the agent manager, config, and event bus.
- **auth** — Bearer token authentication middleware.
- **error** — HTTP error response types.

### Crate-root items

- **AppState** (struct) — Shared server state holding the agent manager, provider registry, config, storage, and event bus.
- **ServerConfig** (struct) — Server configuration (port, host, auth token, CORS).
- **run_server** (async fn) — Start the HTTP server with the given config and state.
- **create_router** (fn) — Build the Axum router with all routes registered.

### Module: routes

REST endpoint handlers organized by resource:

- **Session routes** — `POST /sessions`, `GET /sessions`, `GET /sessions/:id`, `DELETE /sessions/:id`.
- **Message routes** — `POST /sessions/:id/messages`, `GET /sessions/:id/messages`.
- **Model routes** — `GET /models`, `GET /providers`.
- **Config routes** — `GET /config`, `PUT /config`.
- **Tool routes** — `GET /tools`, `POST /tools/:name`.
- **Research routes** — `POST /research`, `GET /research`, `GET /research/:id`, `DELETE /research/:id`.
- **Spec routes** — `GET /specs`, `GET /specs/:id`.
- **Opt routes** — `POST /opt` — Prompt optimization.
- **Health routes** — `GET /health`.

### Module: sse

- **SseStream** (struct) — Server-sent event stream handler.
- **stream_session_events** (fn) — Stream session events to a client via SSE.

### Module: state

- **AppState** (struct) — Shared server state; holds `Arc<AgentManager>`, `Arc<ProviderRegistry>`, `Config`, `Storage`, `EventBus`.

### Module: auth

- **AuthMiddleware** (struct) — Bearer token authentication middleware.
- **require_auth** (fn) — Extractor requiring a valid Bearer token.

### Module: error

- **ApiError** (enum) — HTTP API error types.
- **error_response** (fn) — Convert an error to an HTTP response.