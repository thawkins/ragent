# ragent-server

Axum-based HTTP server and SSE streaming API for ragent. Exposes REST endpoints
and server-sent events so any frontend can drive the agent.

## Workspace Dependencies

- ragent-agent
- ragent-research
- ragent-types
- ragent-config

## External Dependencies

- tokio
- serde
- serde_json
- anyhow
- tracing
- axum
- tower-http
- tokio-stream
- futures
- async-stream
- uuid
- blake3
- chrono

## Public API (crate root)

### Modules

- **routes** — Router construction, shared `AppState`, bearer-token auth middleware, and the REST handlers. Submodules: `automation`, `memory`, `openai`, `openapi`, `research`.
- **sse** — Server-sent event encoding for real-time session updates.

### Crate-root items

- **start_server** (async fn, re-export of `routes::start_server`) — Bind `addr` and serve the router for the given `AppState`.
- **DEFAULT_LISTEN_ADDR** (const) — Default bind address for `ragent serve` (`127.0.0.1:3000`).

### Module: routes

- **AppState** (struct) — Shared server state: `event_bus`, `config`, `storage`, `session_processor`, `auth_token`, `rate_limiter`, `coordinator`, `research_runs`, `research_root`.
- **router** (fn) — Build the Axum router with all routes and middleware (auth, timeout, body limit, CORS).
- **constant_time_eq** (fn) — Constant-time token comparison used by the auth middleware.
- **redact_config_secrets** (fn) / **REDACTED** (const) — Secret redaction for `GET /config`.
- **internal_error_response** (fn) — Build a sanitised 500 response.

REST endpoints (all except `/health`, `/openapi.json` and the automation webhook require the bearer token):

- **Sessions** — `GET|POST /sessions`, `GET|DELETE /sessions/{id}`, `GET|POST /sessions/{id}/messages`, `POST /sessions/{id}/abort`, `POST /sessions/{id}/permission/{req_id}`.
- **Tasks** — `GET|POST /sessions/{id}/tasks`, `GET|DELETE /sessions/{id}/tasks/{tid}`.
- **Config / providers** — `GET /config`, `GET /providers`.
- **Events** — `GET /events` (SSE).
- **Memory** (`/memory`) — `GET /search`, `POST /store`, `DELETE /{id}`, `GET /visualisation` (+ `/graph`, `/tags`, `/heatmap`).
- **Research** (`/research`) — `GET|POST /`, `GET|PUT|DELETE /{name}`, `GET /{name}/events`.
- **Automation** — `GET /automation`, `GET /automation/runs/{id}`, `POST /automation/{id}/run`, `POST /auto/{id}` (webhook).
- **OpenAI-compatible** — `GET /v1/models`, `POST /v1/chat/completions`.
- **Orchestrator** — `POST /orchestrator/start`, `GET /orchestrator/jobs/{id}`.
- **Meta** — `GET /health`, `GET /openapi.json`.

### Module: sse

- **event_to_parts** (fn) — Map an `Event` to its SSE event name and JSON payload.
- **event_to_sse** (fn) — Build an Axum SSE `Event` from a ragent `Event`.
- **redacted_event_debug** (fn) — Debug-format an event with secrets redacted.
