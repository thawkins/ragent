# Kimi K2.6 Agent Swarm — Integration Research for ragent

> **Research Date:** 2026-05-23  
> **Sources:** Moonshot AI Official Website, Kimi Platform Documentation, Verdent AI Analysis, MarkTechPost, Till Freitag Blog, Lushbinary Developer Guide  
> **Researcher:** Rust Agent (ragent project)

---

## 1. Executive Summary

**Kimi K2.6 Agent Swarm** is Moonshot AI's horizontally-scaled, model-native multi-agent system that coordinates up to **300 parallel sub-agents** across **4,000 coordinated steps** within a single autonomous run. Unlike traditional multi-agent systems that rely on hand-coded orchestration frameworks (CrewAI, LangGraph, AutoGen), Kimi's swarm is **absorbed into the model itself** — the orchestration is a first-party architectural feature, not infrastructure bolted on top.

**Critical Integration Insight:** K2.6's API is **fully OpenAI-compatible** (`https://api.moonshot.ai/v1`). This means ragent can integrate Kimi K2.6 immediately via the existing generic OpenAI provider path, or more robustly via a dedicated Moonshot provider. The true integration opportunity is not just "using Kimi as an LLM backend," but **adopting Kimi's swarm design patterns** to enhance ragent's existing `ragent-team` and `/swarm` infrastructure.

**Key K2.6 Stats vs. K2.5:**

| Capability | K2.5 | K2.6 | Change |
|-----------|------|------|--------|
| Max Parallel Sub-agents | 100 | 300 | 3× |
| Max Coordinated Steps | 1,500 | 4,000 | 2.7× |
| Video Input | No | Yes | Added |
| Claw Groups (heterogeneous swarms) | No | Research preview | Added |
| Document-to-Skill Conversion | No | Yes | Added |
| SWE-Bench Pro | 50.7 | 58.6 | +15.6% |
| SWE-Bench Verified | — | 80.2 | SOTA open-source |

---

## 2. What Is Kimi K2.6 Agent Swarm?

### 2.1 Core Philosophy: Scale Out, Not Just Up

Moonshot AI's central thesis is that **the single-agent sequential execution model hits a structural ceiling**. As context windows fill during long-horizon tasks, systems fall back to lossy summarization. This is not a bug — it is a **fundamental architectural constraint**.

**Kimi's solution:** Horizontal scaling via self-organizing agent networks. The model itself decomposes tasks, spawns specialized sub-agents, routes work based on skill profiles, and synthesizes results through a shared state coordinator.

| Aspect | Vertical Scaling (Single Agent) | Horizontal Scaling (Kimi Swarm) |
|--------|--------------------------------|--------------------------------|
| Analogy | One brain, bigger | Many brains, networked |
| Unit | Single model | Self-organizing network |
| Organization | Expert individual | Company, laboratory, agency |
| Ceiling | Physical, economic, intellectual | Near-unbounded with parallelism |
| Coordination | Implicit (single context) | Explicit (shared operational space) |

### 2.2 Model-Native vs. Framework Orchestration

Three architectures have emerged by Q2 2026:

| Approach | Example | Philosophy | Tradeoff |
|----------|---------|-----------|----------|
| **Model-native** | Kimi K2.6 | The model *is* the orchestrator | Zero config, black box |
| **Platform** | Airtable Superagent | Platform orchestrates agents | Business-ready, vendor lock-in |
| **Framework** | CrewAI, LangGraph | Developers build orchestration | Full control, high complexity |

**Kimi's model-native approach** means swarm behavior emerges through prompting — no framework needed. The coordinator agent (K2.6 itself) was specifically trained using Parallel-Agent RL to manage delegation, conflict resolution, and synthesis.

### 2.3 Heterogeneous Decomposition

K2.6 uses **heterogeneous decomposition** rather than uniform parallelism. When a complex task arrives, the coordinator analyzes the task structure and assigns subtasks based on **skill profiles**:

- **Code refactoring** → code agent with dev tools
- **Research synthesis** → research agent with web search
- **Documentation generation** → writing agent with doc tools
- **Fact validation** → fact-checker agent with search tools
- **UI generation** → design agent with coding-driven design skills

> *"K2.6 adaptively coordinates tasks based on agent skill profiles — dynamically matching tasks to agents based on their specific skill profiles and available tools."* — Moonshot AI

The 4,000-step budget is **swarm-wide**, not per-agent. A 300-agent swarm averages ~13 steps per agent, favoring many shallow parallel subtasks over deep individual runs.

### 2.4 Document-to-Skill Conversion (K2.6 New)

A distinctive K2.6 capability: **PDFs, spreadsheets, slides, and Word documents can be converted into reusable agent skills**. The model captures the document's structural and stylistic DNA, allowing sub-agents to reproduce the same quality and format in future tasks.

**Example:** Upload an astrophysics paper → K2.6 creates an "academic writing" skill → future agents produce 40-page research papers in the same style without re-uploading the source.

**Integration angle:** ragent's existing skills system (`/skills`) could adopt this pattern — allowing users to convert any uploaded document into a skill template that future agents inherit.

### 2.5 Claw Groups — Heterogeneous Swarms (Research Preview)

**Claw Groups** extends swarm coordination beyond Kimi-only agents. It allows humans and agents from **any model, any device** to participate in the same swarm, with K2.6 coordinating across the mixed pool.

**Integration angle:** ragent's existing team system already supports heterogeneous agents (different models per teammate). Claw Groups validates this architecture and suggests enhancing it with cross-model swarm coordination.

### 2.6 Structural Anti-Groupthink

A unique feature of Kimi's swarm is **productive disagreement**:

1. Independent agents analyze the same problem from different angles
2. Agents arrive at different conclusions naturally
3. A **forced reconciliation phase** triggers when contradictions exceed a threshold
4. Agents must defend or revise their conclusions
5. Final synthesis incorporates the reconciled, multi-perspective view

This is structurally designed to avoid the echo-chamber effect common in single-agent or tightly-coupled multi-agent systems.

---

## 3. Kimi K2.6 Model Specifications

### 3.1 Architecture

| Spec | Value |
|------|-------|
| Architecture | Mixture-of-Experts (MoE) |
| Total Parameters | 1 Trillion |
| Activated Parameters | 32 Billion per token |
| Layers (incl. Dense) | 61 |
| Attention Heads | 64 |
| Experts / Selected per Token | 384 / 8 + 1 shared |
| Context Length | 256K tokens |
| Vocabulary Size | 160K |
| Attention Mechanism | Multi-head Latent Attention (MLA) |
| Activation Function | SwiGLU |
| Vision Encoder | MoonViT (400M params) |
| Quantization | Native INT4 (QAT) |
| License | Modified MIT |

**MLA (Multi-head Latent Attention)** reduces KV cache memory by compressing key-value pairs into a lower-dimensional latent space. Combined with SwiGLU and 384 routed experts, K2.6 achieves strong quality-throughput balance.

### 3.2 Model Variants

| Model | Context | Thinking | Best For |
|-------|---------|----------|----------|
| `kimi-k2.6` | 256K | Optional (toggleable) | Long-horizon coding, swarm orchestration |
| `kimi-k2.5` | 256K | Optional | Visual + text, agent loops |
| `kimi-k2` | 256K | No | General reasoning, coding, MoE base |
| `kimi-k2-thinking` | 256K | Required | Complex reasoning, math, multi-step |

### 3.3 Pricing (as of 2026-05)

| Model | Cache Hit | Input | Output |
|-------|-----------|-------|--------|
| `kimi-k2.6` | $0.16 / MTok | $0.95 / MTok | $4.00 / MTok |
| `kimi-k2.5` | $0.10 / MTok | $0.60 / MTok | $3.00 / MTok |
| `kimi-k2` | $0.15 / MTok | $0.60 / MTok | $2.50 / MTok |

### 3.4 Benchmarks vs. Frontier Models

#### Agentic Benchmarks

| Benchmark | K2.6 | GPT-5.4 | Claude Opus 4.6 | Gemini 3.1 Pro |
|-----------|------|---------|-----------------|----------------|
| HLE-Full (w/ tools) | **54.0** | 52.1 | 53.0 | 51.4 |
| BrowseComp | 83.2 | 82.7 | 83.7 | **85.9** |
| BrowseComp (Swarm) | **86.3** | 78.4 | — | — |
| DeepSearchQA (f1) | **92.5** | 78.6 | 91.3 | 81.9 |
| OSWorld-Verified | 73.1 | **75.0** | 72.7 | — |

#### Coding Benchmarks

| Benchmark | K2.6 | GPT-5.4 | Claude Opus 4.6 | Gemini 3.1 Pro |
|-----------|------|---------|-----------------|----------------|
| SWE-Bench Verified | 80.2 | — | **80.8** | 80.6 |
| SWE-Bench Pro | **58.6** | 57.7 | 53.4 | 54.2 |
| Terminal-Bench 2.0 | 66.7 | 65.4 | 65.4 | **68.5** |
| LiveCodeBench (v6) | 89.6 | — | 88.8 | **91.7** |
| SWE-Bench Multilingual | 76.7 | — | **77.8** | 76.9 |

**Key takeaway:** K2.6 leads on agentic benchmarks (HLE-Full, DeepSearchQA, BrowseComp Swarm) and SWE-Bench Pro, making it the strongest open-source choice for agentic coding workloads.

---

## 4. API Compatibility & Integration Surface

### 4.1 OpenAI-Compatible API

Kimi provides **full OpenAI SDK compatibility**:

```python
from openai import OpenAI

client = OpenAI(
    api_key=os.environ.get("MOONSHOT_API_KEY"),
    base_url="https://api.moonshot.ai/v1",  # or api.moonshot.cn for China
)

completion = client.chat.completions.create(
    model="kimi-k2.6",
    messages=[...],
    tools=[...],           # Tool calling supported
    stream=True,           # Streaming supported
)
```

**Supported features:**
- Standard chat completions (`/v1/chat/completions`)
- Tool/function calling (up to 300+ calls in single-agent mode, 4,000 across swarm)
- Streaming responses (SSE)
- Multi-modal input (text, image via `image_url`, video via `video_url`)
- JSON mode
- Thinking mode toggle via `extra_body={"thinking": {"type": "disabled"}}`

### 4.2 Official Tools

Kimi provides **plug-and-play official tools** via the API:

| Tool | Purpose |
|------|---------|
| `web_search` | Web search with citation support |
| `rethink` | Intelligent idea organization |
| `random_choice` | Random selection |
| `memory` | Conversation history persistence |
| `excel` | Excel/CSV analysis |
| `code_runner` | Python code execution |
| `quick_js` | Safe JavaScript execution (QuickJS) |
| `date` | Date/time processing |
| `fetch` | URL content extraction |
| `convert` | Unit/currency conversion |
| `base64` | Encoding/decoding |

**Integration angle:** ragent could expose these as additional tool options when the Moonshot provider is active, complementing ragent's existing 111 built-in tools.

### 4.3 Thinking Mode Control

K2.6 supports **thinking and non-thinking modes**:

```python
# Enable thinking (default for complex tasks)
response = client.chat.completions.create(
    model="kimi-k2.6",
    messages=[...],
)

# Disable thinking (faster, cheaper for simple tasks)
response = client.chat.completions.create(
    model="kimi-k2.6",
    messages=[...],
    extra_body={"thinking": {"type": "disabled"}},
    max_tokens=1024*32,
)
```

**ragent integration:** Map to ragent's existing `/thinking` command terminology (`auto`, `off`, `low`, `medium`, `high`). Kimi's `disabled` maps to `off`, and default behavior maps to `auto`.

---

## 5. Real-World Execution Cases

### 5.1 12-Hour Zig Optimization (Qwen3.5-0.8B)

- Downloaded and deployed Qwen3.5-0.8B locally on a Mac
- Implemented and optimized model inference in **Zig** (niche language)
- **4,000+ tool calls, 12+ hours continuous execution, 14 iterations**
- Improved throughput from ~15 to ~193 tokens/sec
- Achieved speeds ~20% faster than LM Studio

### 5.2 13-Hour exchange-core Refactor

- Overhauled 8-year-old open-source financial matching engine
- 12 optimization strategies, 1,000+ tool calls, 4,000+ lines modified
- Analyzed CPU and allocation flame graphs
- Reconfigured thread topology (4ME+2RE → 2ME+1RE)
- **185% medium throughput leap** (0.43 → 1.24 MT/s)
- **133% performance throughput gain** (1.23 → 2.86 MT/s)

### 5.3 Agent Swarm Demonstrations

- **CV Matching:** 100 sub-agents matched one CV against 100 California roles → 100 customized resumes
- **Store Landing Pages:** 30 retail stores without websites identified from Google Maps → 30 landing pages generated
- **Academic Paper:** Astrophysics paper converted to reusable skill → 40-page, 7,000-word research paper produced

---

## 6. Integration Strategy for ragent

### 6.1 Phase 1: Add Kimi as a First-Class Provider (Immediate — 1-2 days)

**Option A: Generic OpenAI Provider (Works Today)**

Users can already configure Kimi via ragent's `generic_openai` provider:

```json
{
  "provider": {
    "generic_openai": {
      "env": ["MOONSHOT_API_KEY"],
      "api": {
        "base_url": "https://api.moonshot.ai/v1",
        "default_model": "kimi-k2.6"
      }
    }
  }
}
```

**Pros:** Zero code changes required  
**Cons:** Missing Kimi-specific features (thinking toggle, multi-modal, official tools)

**Option B: Dedicated Moonshot Provider (Recommended)**

Add a proper `moonshot` provider to `ragent-llm`:

```rust
// crates/ragent-llm/src/providers/moonshot.rs
pub struct MoonshotProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,     // default: https://api.moonshot.ai/v1
    model: String,        // default: kimi-k2.6
    thinking: ThinkingMode, // auto, off, low, medium, high
}

impl LLMProvider for MoonshotProvider {
    // OpenAI-compatible request/response format
    // Kimi-specific: thinking mode control, multi-modal content parts
}
```

**Features to support:**
- Native thinking mode control (`/thinking` command integration)
- Multi-modal input (image_url, video_url content parts)
- Kimi official tool integrations (web_search, code_runner, etc.)
- Document-to-skill conversion API (if exposed)

**Implementation:** Model after `crates/ragent-llm/src/providers/openai.rs` — Kimi API is a drop-in replacement with the same request/response format.

### 6.2 Phase 2: Swarm Architecture Enhancement (Medium-term — 1-2 weeks)

#### Current ragent Swarm Limitations

ragent's existing `/swarm <prompt>` command:
- Spawns a fixed number of teammates (typically 3-5)
- Each teammate gets the same prompt with minor variations
- Results are collected and summarized by the lead
- **Static** — the swarm structure is predetermined
- **No skill-based routing** — all agents are generic

#### Kimi-Inspired Enhancements

**A. Dynamic Role Assignment**

Implement **adaptive role creation** based on task analysis:

```rust
pub enum SwarmRole {
    Ceo,          // Decomposes tasks, hires agents, synthesizes results
    Researcher,   // Gathers information via search/web
    Analyst,      // Processes and synthesizes data
    FactChecker,  // Validates claims against sources
    Writer,       // Produces structured output
    Critic,       // Identifies flaws and contradictions
    Coder,        // Writes/refactors code
    DevOps,       // Infrastructure and deployment
    Custom(String),
}
```

The lead agent (CEO) analyzes the prompt and dynamically determines which roles are needed:

```
User: "Research the Rust ecosystem for embedded databases"
CEO: "I need 3 researchers (survey different DBs), 2 analysts (compare tradeoffs), 1 fact-checker"
→ Spawns 6 specialized agents with tailored prompts and tool sets
```

**B. Disagreement & Reconciliation Protocol**

Add a **reconciliation phase** to ragent's team workflow:

1. Sub-agents complete tasks independently
2. A dedicated **Critic** agent reviews all outputs for contradictions
3. If contradictions found, a **Reconciliation** round triggers
4. Agents must defend or revise their conclusions with evidence
5. Final synthesis incorporates the reconciled, multi-perspective view

```rust
pub struct SwarmReconciliation {
    pub contradictions: Vec<Contradiction>,
    pub rounds: u32,
    pub threshold: f32,      // agreement threshold (e.g., 0.8)
    pub max_rounds: u32,     // prevent infinite loops
}

pub struct Contradiction {
    pub claim_a: (AgentId, String),
    pub claim_b: (AgentId, String),
    pub severity: f32,       // 0.0-1.0
    pub topic: String,
}
```

**C. Parallel Width Control**

Implement dynamic control over parallelism:

```rust
pub struct SwarmConfig {
    pub max_parallel_agents: u32,      // default: 50 (practical limit)
    pub max_tool_calls: u32,           // default: 1000
    pub decomposition_depth: u32,    // how many levels of sub-task nesting
    pub auto_hire: bool,               // enable dynamic agent creation
    pub disagreement_mode: bool,       // enable critic/reconciliation
    pub skill_inheritance: bool,     // inherit document-derived skills
}
```

**D. Integration Points in ragent Codebase**

| Component | Current | Enhanced |
|-----------|---------|----------|
| `ragent-team/src/lib.rs` | Static teammate spawning | Dynamic role-based spawning with skill profiles |
| `SwarmOrchestrator` | Parallel execution + collection | + decomposition + reconciliation + skill routing |
| `/swarm` slash command | Single prompt broadcast | Adaptive task decomposition with role assignment |
| `team_task_create` | Manual task creation | Auto-generated from CEO decomposition |
| `team_spawn` | Generic agent spawn | Role-specific spawn with tailored tools |

### 6.3 Phase 3: Long-Horizon Agent Loops (Advanced — 2-4 weeks)

Kimi K2.6 demonstrated:
- **4,000+ tool calls** over **12+ hours** of continuous execution
- Self-correction when encountering errors
- Architectural pattern following (maintaining existing code conventions)

**Implementation:** Enhance ragent's session processor to support:

1. **Checkpointing** — Save full session state every N tool calls for crash recovery
2. **Context Compaction** — Intelligent summarization without lossy truncation
3. **Iterative Refinement** — Allow agents to revise their own plans based on intermediate results
4. **Tool Call Budgeting** — Track and limit tool calls per session to prevent runaway execution
5. **Heartbeat / Keep-alive** — Periodic status reports during long runs

```rust
pub struct LongHorizonConfig {
    pub checkpoint_interval: u32,      // tool calls between checkpoints
    pub max_tool_calls: u32,           // hard limit (e.g., 4000)
    pub max_execution_time: Duration,    // wall-clock limit (e.g., 12 hours)
    pub compaction_strategy: CompactionStrategy,
    pub heartbeat_interval: Duration,    // status report frequency
}
```

### 6.4 Phase 4: Document-to-Skill Conversion (Future — 2-3 weeks)

Adopt Kimi's document-to-skill pattern:

1. User uploads a document (PDF, Word, spreadsheet, slide deck)
2. ragent analyzes the document's structure, style, and content patterns
3. Creates a reusable skill template from the analysis
4. Future swarm agents can inherit this skill for consistent output

```rust
pub struct DocumentSkill {
    pub name: String,
    pub source_document: PathBuf,
    pub style_dna: StyleProfile,       // tone, formatting, structure patterns
    pub content_patterns: Vec<Pattern>, // reusable templates
    pub example_outputs: Vec<String>,   // generated samples
}

impl SkillPack for DocumentSkill {
    fn apply_to(&self, agent: &mut Agent) {
        agent.system_prompt += &self.style_dna.to_prompt();
    }
}
```

### 6.5 Phase 5: Multi-Modal Agent Support (Future)

K2.6 supports native image and video input. ragent integration:

- Allow image/video content parts in chat messages
- Support vision-based tools (screenshot analysis, diagram understanding)
- Enable coding agents to work from UI mockups or design files

---

## 7. Technical Integration Details

### 7.1 Provider Implementation

```rust
// crates/ragent-llm/src/providers/moonshot.rs
use crate::{ChatRequest, ChatResponse, LLMProvider, Tool};
use reqwest;
use serde_json;

pub struct MoonshotProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    default_model: String,
}

impl LLMProvider for MoonshotProvider {
    async fn chat(&self, request: ChatRequest) -> anyhow::Result<ChatResponse> {
        // Convert ChatRequest to OpenAI-compatible format
        // Kimi-specific: handle thinking mode, multi-modal content parts
        let body = serde_json::json!({
            "model": request.model.unwrap_or_else(|| self.default_model.clone()),
            "messages": self.convert_messages(&request.messages),
            "tools": request.tools.map(|t| self.convert_tools(&t)),
            "stream": request.stream,
            // Kimi-specific: thinking mode
            "thinking": self.get_thinking_config(&request),
        });

        let response = self.client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await?;

        self.parse_response(response).await
    }
}
```

### 7.2 Configuration Schema

```json
{
  "provider": {
    "moonshot": {
      "env": ["MOONSHOT_API_KEY"],
      "api": {
        "base_url": "https://api.moonshot.ai/v1",
        "default_model": "kimi-k2.6",
        "timeout_secs": 120
      },
      "thinking": {
        "enabled": true,
        "level": "auto"
      },
      "models": {
        "kimi-k2.6": {
          "thinking": { "enabled": true, "level": "high" },
          "multimodal": true,
          "max_tool_calls": 4000
        },
        "kimi-k2.5": {
          "thinking": { "enabled": false },
          "multimodal": true,
          "max_tool_calls": 1500
        }
      }
    }
  },
  "swarm": {
    "max_parallel_agents": 50,
    "max_tool_calls": 1000,
    "auto_hire": true,
    "disagreement_mode": true,
    "checkpoint_interval": 100
  }
}
```

### 7.3 Slash Commands

| Command | Action |
|---------|--------|
| `/provider moonshot` | Switch to Moonshot provider |
| `/model kimi-k2.6` | Select K2.6 model |
| `/thinking auto|off|low|medium|high` | Control thinking mode |
| `/swarm <prompt>` | Launch dynamic swarm with Kimi-inspired decomposition |
| `/swarm config` | Show/adjust swarm parameters |
| `/skill from-doc <path>` | Convert document to reusable skill |

---

## 8. Competitive Analysis

### 8.1 How Kimi Agent Swarm Compares to ragent's Current Team System

| Dimension | ragent Team System | Kimi K2.6 Swarm |
|-----------|-------------------|-----------------|
| Orchestration | Explicit (user-defined teams) | Model-native (self-organizing) |
| Max Agents | Limited by user config | 300 (K2.6) |
| Steps | Session-based | 4,000 coordinated |
| Decomposition | Manual (/swarm broadcast) | Automatic (heterogeneous) |
| Disagreement | None | Built-in reconciliation |
| Tool Calls | Per-agent limit | 4,000 swarm-wide |
| Speed | Sequential or limited parallel | 4.5× faster parallel |
| Context | Per-agent context | 256K per agent |
| Duration | Session-based | 12+ hour continuous |
| Skills | YAML skill packs | Document-derived skills |

### 8.2 Positioning

- **ragent** excels at: Local execution, 111 built-in tools, TUI, security layers, GitHub/GitLab integration, code index, memory system, spec management, skills system, persistent storage
- **Kimi Swarm** excels at: Massive parallelism, self-organization, long-horizon execution, multi-modal reasoning, document-to-skill conversion

**Integration thesis:** Use Kimi K2.6 as the "brain" for ragent's swarm coordinator, combining ragent's rich tool ecosystem with Kimi's horizontal scaling capabilities. ragent provides the infrastructure; Kimi provides the intelligence for task decomposition and agent coordination.

---

## 9. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| API rate limits on 100+ agents | Medium | High | Implement backoff/retry, use batching, practical limit of 50 agents |
| Cost explosion (300 agents × $0.95/MTok) | High | High | Token budgeting, user confirmation for >N agents, default max 50 |
| Context window overflow in long runs | Medium | Medium | Intelligent compaction, use 256K window fully, checkpointing |
| Network latency (China-based API) | Medium | Low | Support both `api.moonshot.ai` and `api.moonshot.cn`, HTTP/2, connection pooling |
| Vendor lock-in to Moonshot | Low | Medium | Maintain generic OpenAI compatibility as fallback |
| Thinking mode incompatibility | Low | Low | Default thinking=off, enable via explicit user toggle |
| Failure recovery opacity | Medium | Medium | Add explicit checkpointing and resume; don't rely on opaque model behavior |
| State drift in long-horizon runs | Medium | High | Implement explicit state validation checkpoints every N steps |

---

## 10. Recommended Implementation Roadmap

| Phase | Timeline | Deliverables | Effort |
|-------|----------|--------------|--------|
| **Phase 1** | Week 1 | Add `moonshot` provider to `ragent-llm`, config schema, `/provider moonshot` command, thinking mode toggle | 1-2 days |
| **Phase 2** | Weeks 2-3 | Enhance `ragent-team` with `SwarmRole` enum, dynamic decomposition prompt, skill-based routing | 1-2 weeks |
| **Phase 3** | Weeks 4-5 | Add disagreement/reconciliation protocol, `Critic` agent role, `SwarmReconciliation` struct | 1-2 weeks |
| **Phase 4** | Weeks 6-7 | Long-horizon support: checkpointing to SQLite, compaction strategy, tool call budgeting, heartbeat | 2-3 weeks |
| **Phase 5** | Weeks 8-10 | Document-to-skill conversion: upload → analyze → skill template → inheritance | 2-3 weeks |
| **Phase 6** | Week 11+ | Multi-modal content parts (image/video), vision-based tools, UI mockup coding | 2-4 weeks |

---

## 11. References

1. [Kimi K2.6 Agent Swarm: 300 Agents, 4,000 Steps — Verdent AI](https://www.verdent.ai/guides/kimi-k2-6-agent-swarm)
2. [Agent Swarm Architectures Compared — Till Freitag](https://till-freitag.com/blog/agent-swarm-architectures-compared)
3. [Kimi K2.6 Technical Blog — Moonshot AI](https://www.kimi.com/blog/kimi-k2-6)
4. [Kimi K2.6 Quick Start — Kimi Platform](https://platform.kimi.ai/docs/guide/kimi-k2-6-quickstart)
5. [Kimi K2.6 Developer Guide — Lushbinary](https://lushbinary.com/blog/kimi-k2-6-developer-guide-benchmarks-api-agent-swarm)
6. [Moonshot AI Releases Kimi K2.6 — MarkTechPost](https://www.marktechpost.com/2026/04/20/moonshot-ai-releases-kimi-k2-6-with-long-horizon-coding-agent-swarm-scaling-to-300-sub-agents-and-4000-coordinated-steps)
7. [Kimi API Platform](https://platform.moonshot.ai)
8. [Kimi K2.6 on Hugging Face](https://huggingface.co/moonshotai/Kimi-K2.6)
9. [Kimi Code CLI — GitHub](https://github.com/MoonshotAI/kimi-cli)
10. [Kimi Agent SDK — GitHub](https://github.com/MoonshotAI/kimi-agent-sdk)

---

*Document generated by ragent research agent. For updates, see ragent project repository.*
