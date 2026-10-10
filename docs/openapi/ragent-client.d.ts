// AUTO-GENERATED from crates/ragent-server/src/routes/openapi.rs
// Do not edit by hand. Regenerate with `ragent openapi --client typescript`.

export interface RagentClientOptions {
  baseUrl: string;
  token?: string;
  fetch?: typeof fetch;
}

export class RagentClient {
  private readonly options: RagentClientOptions;

  constructor(options: RagentClientOptions) {
    this.options = options;
  }

  private async request<T>(
    method: string,
    path: string,
    body?: unknown,
    auth = true,
  ): Promise<T> {
    const doFetch = this.options.fetch ?? fetch;
    const headers: Record<string, string> = { Accept: 'application/json' };
    if (body !== undefined) headers['Content-Type'] = 'application/json';
    if (auth && this.options.token) headers['Authorization'] = `Bearer ${this.options.token}`;
    const response = await doFetch(`${this.options.baseUrl}${path}`, {
      method,
      headers,
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    if (!response.ok) {
      throw new Error(`${method} ${path} failed with ${response.status}`);
    }
    return (await response.json()) as T;
  }

  /** GET /health - Liveness probe */
  async getHealth(): Promise<unknown> {
    return this.request<unknown>('GET', `/health`, false);
  }

  /** GET /openapi.json - OpenAPI 3.1 document for this API */
  async getOpenapijson(): Promise<unknown> {
    return this.request<unknown>('GET', `/openapi.json`, false);
  }

  /** GET /config - Resolved configuration (credentials redacted) */
  async getConfig(): Promise<unknown> {
    return this.request<unknown>('GET', `/config`, true);
  }

  /** GET /providers - Registered LLM providers and their models */
  async getProviders(): Promise<unknown> {
    return this.request<unknown>('GET', `/providers`, true);
  }

  /** GET /sessions - List sessions */
  async getSessions(): Promise<unknown> {
    return this.request<unknown>('GET', `/sessions`, true);
  }

  /** POST /sessions - Create a session for a project directory */
  async postSessions(body: unknown): Promise<unknown> {
    return this.request<unknown>('POST', `/sessions`, body, true);
  }

  /** GET /sessions/{id} - Fetch one session */
  async getSessionsId(id: string): Promise<unknown> {
    return this.request<unknown>('GET', `/sessions/${encodeURIComponent(id)}`, true);
  }

  /** DELETE /sessions/{id} - Archive a session */
  async deleteSessionsId(id: string): Promise<unknown> {
    return this.request<unknown>('DELETE', `/sessions/${encodeURIComponent(id)}`, true);
  }

  /** GET /sessions/{id}/messages - List a session's messages */
  async getSessionsIdMessages(id: string): Promise<unknown> {
    return this.request<unknown>('GET', `/sessions/${encodeURIComponent(id)}/messages`, true);
  }

  /** POST /sessions/{id}/messages - Send a message and run one agent turn */
  async postSessionsIdMessages(id: string, body: unknown): Promise<unknown> {
    return this.request<unknown>('POST', `/sessions/${encodeURIComponent(id)}/messages`, body, true);
  }

  /** POST /sessions/{id}/abort - Abort the in-flight turn for a session */
  async postSessionsIdAbort(id: string): Promise<unknown> {
    return this.request<unknown>('POST', `/sessions/${encodeURIComponent(id)}/abort`, true);
  }

  /** POST /sessions/{id}/permission/{req_id} - Answer a pending permission request */
  async postSessionsIdPermissionReqId(id: string, req_id: string, body: unknown): Promise<unknown> {
    return this.request<unknown>('POST', `/sessions/${encodeURIComponent(id)}/permission/${encodeURIComponent(req_id)}`, body, true);
  }

  /** GET /sessions/{id}/tasks - List sub-agent tasks for a session */
  async getSessionsIdTasks(id: string): Promise<unknown> {
    return this.request<unknown>('GET', `/sessions/${encodeURIComponent(id)}/tasks`, true);
  }

  /** POST /sessions/{id}/tasks - Spawn a background sub-agent task */
  async postSessionsIdTasks(id: string, body: unknown): Promise<unknown> {
    return this.request<unknown>('POST', `/sessions/${encodeURIComponent(id)}/tasks`, body, true);
  }

  /** GET /sessions/{id}/tasks/{tid} - Fetch one sub-agent task */
  async getSessionsIdTasksTid(id: string, tid: string): Promise<unknown> {
    return this.request<unknown>('GET', `/sessions/${encodeURIComponent(id)}/tasks/${encodeURIComponent(tid)}`, true);
  }

  /** DELETE /sessions/{id}/tasks/{tid} - Cancel a sub-agent task */
  async deleteSessionsIdTasksTid(id: string, tid: string): Promise<unknown> {
    return this.request<unknown>('DELETE', `/sessions/${encodeURIComponent(id)}/tasks/${encodeURIComponent(tid)}`, true);
  }

  /** GET /events - Server-sent event stream of runtime events */
  async getEvents(): Promise<unknown> {
    return this.request<unknown>('GET', `/events`, true);
  }

  /** GET /memory/search - Search stored memories */
  async getMemorySearch(): Promise<unknown> {
    return this.request<unknown>('GET', `/memory/search`, true);
  }

  /** POST /memory/store - Store a memory */
  async postMemoryStore(body: unknown): Promise<unknown> {
    return this.request<unknown>('POST', `/memory/store`, body, true);
  }

  /** DELETE /memory/{id} - Forget a memory by id */
  async deleteMemoryId(id: string): Promise<unknown> {
    return this.request<unknown>('DELETE', `/memory/${encodeURIComponent(id)}`, true);
  }

  /** GET /memory/visualisation - Memory visualisation dataset */
  async getMemoryVisualisation(): Promise<unknown> {
    return this.request<unknown>('GET', `/memory/visualisation`, true);
  }

  /** GET /memory/visualisation/graph - Memory knowledge graph */
  async getMemoryVisualisationGraph(): Promise<unknown> {
    return this.request<unknown>('GET', `/memory/visualisation/graph`, true);
  }

  /** GET /memory/visualisation/tags - Memory tag frequency */
  async getMemoryVisualisationTags(): Promise<unknown> {
    return this.request<unknown>('GET', `/memory/visualisation/tags`, true);
  }

  /** GET /memory/visualisation/heatmap - Memory activity heatmap */
  async getMemoryVisualisationHeatmap(): Promise<unknown> {
    return this.request<unknown>('GET', `/memory/visualisation/heatmap`, true);
  }

  /** GET /research/ - List research items */
  async getResearch(): Promise<unknown> {
    return this.request<unknown>('GET', `/research/`, true);
  }

  /** POST /research/ - Create a research item */
  async postResearch(body: unknown): Promise<unknown> {
    return this.request<unknown>('POST', `/research/`, body, true);
  }

  /** GET /research/{name} - Show one research item */
  async getResearchName(name: string): Promise<unknown> {
    return this.request<unknown>('GET', `/research/${encodeURIComponent(name)}`, true);
  }

  /** PUT /research/{name} - Update a research item */
  async putResearchName(name: string, body: unknown): Promise<unknown> {
    return this.request<unknown>('PUT', `/research/${encodeURIComponent(name)}`, body, true);
  }

  /** DELETE /research/{name} - Delete a research item */
  async deleteResearchName(name: string): Promise<unknown> {
    return this.request<unknown>('DELETE', `/research/${encodeURIComponent(name)}`, true);
  }

  /** GET /research/{name}/events - Server-sent progress events for a research run */
  async getResearchNameEvents(name: string): Promise<unknown> {
    return this.request<unknown>('GET', `/research/${encodeURIComponent(name)}/events`, true);
  }

  /** GET /v1/models - OpenAI-compatible model list */
  async getV1Models(): Promise<unknown> {
    return this.request<unknown>('GET', `/v1/models`, true);
  }

  /** POST /v1/chat/completions - OpenAI-compatible chat completion (stream:false JSON; stream:true SSE) */
  async postV1ChatCompletions(body: unknown): Promise<unknown> {
    return this.request<unknown>('POST', `/v1/chat/completions`, body, true);
  }

  /** POST /orchestrator/start - Start an orchestration job */
  async postOrchestratorStart(body: unknown): Promise<unknown> {
    return this.request<unknown>('POST', `/orchestrator/start`, body, true);
  }

  /** GET /orchestrator/jobs/{id} - Fetch an orchestration job */
  async getOrchestratorJobsId(id: string): Promise<unknown> {
    return this.request<unknown>('GET', `/orchestrator/jobs/${encodeURIComponent(id)}`, true);
  }

  /** GET /automation - List configured automations and their next-due times */
  async getAutomation(): Promise<unknown> {
    return this.request<unknown>('GET', `/automation`, true);
  }

  /** GET /automation/runs/{id} - List an automation's run history */
  async getAutomationRunsId(id: string): Promise<unknown> {
    return this.request<unknown>('GET', `/automation/runs/${encodeURIComponent(id)}`, true);
  }

  /** POST /automation/{id}/run - Enqueue a manual automation run */
  async postAutomationIdRun(id: string): Promise<unknown> {
    return this.request<unknown>('POST', `/automation/${encodeURIComponent(id)}/run`, true);
  }

  /** POST /auto/{id} - Automation webhook ingress */
  async postAutoId(id: string, body: unknown): Promise<unknown> {
    return this.request<unknown>('POST', `/auto/${encodeURIComponent(id)}`, body, false);
  }
}
