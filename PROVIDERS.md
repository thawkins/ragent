# LLM Provider Comparison — OpenCode vs Codex CLI vs Claude Code vs GitHub Copilot vs ragent

> Last updated: 2025-07

This document compares the LLM provider support across five AI coding agents
for the terminal.

## Summary

| Feature | OpenCode | Codex CLI | Claude Code | GitHub Copilot CLI | ragent |
|---|---|---|---|---|---|
| **Total providers** | 75+ | 1 (OpenAI) | 1 (Anthropic) | 3 | 11 |
| **Extensible** | ✅ (AI SDK) | ❌ | ❌ | ✅ (BYOK, plugins) | ✅ (Generic OpenAI) |
| **Local models** | ✅ (Ollama, llama.cpp, LM Studio) | ✅ (via OpenRouter/Ollama proxy) | ❌ | ✅ (local model support) | ✅ (Ollama) |
| **Cloud-only models** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Custom/OpenAI-compatible** | ✅ (baseURL override) | ✅ (via proxy) | ❌ | ✅ (BYOK) | ✅ (Generic OpenAI) |
| **Open source** | ✅ | ✅ | ❌ | ❌ | ✅ |

## Provider Matrix

| Provider | OpenCode | Codex CLI | Claude Code | GitHub Copilot CLI | ragent |
|---|:---:|:---:|:---:|:---:|:---:|
| **Anthropic** | ✅ | ❌ | ✅ (native) | ✅ | ✅ |
| **OpenAI** | ✅ | ✅ (native) | ❌ | ✅ | ✅ |
| **Google Gemini** | ✅ | ��� | ❌ | ✅ | ✅ |
| **Ollama (local)** | ✅ | ❌* | ❌ | ✅ | ✅ |
| **Ollama Cloud** | ✅ | ❌ | ❌ | ❌ | ✅ |
| **Hugging Face** | ✅ | ❌ | ❌ | ❌ | ✅ |
| **GitHub Copilot** | ✅ | ❌ | ❌ | ✅ (native) | ✅ |
| **Generic OpenAI-compatible** | ✅ (baseURL) | ✅ (baseURL) | ❌ | ✅ (BYOK) | ✅ (native) |
| **Azure AI Foundry** | ❌ | ❌ | ❌ | ✅ | ✅ |
| **Azure Resource (File)** | ❌ | ❌ | ❌ | ❌ | ✅ |
| **Amazon Bedrock** | ❌ | ✅ | ❌ | ✅ | ✅ |
| **Google Vertex AI** | ❌ | ❌ | ✅ | ❌ | ❌ |
| **Azure OpenAI** | ✅ | ❌ | ❌ | ✅ | ❌ |
| **DeepSeek** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Groq** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Cerebras** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Together AI** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Fireworks AI** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **NVIDIA** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **xAI (Grok)** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **OpenRouter** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **llama.cpp** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **LM Studio** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Cloudflare Workers AI** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Scaleway** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **DigitalOcean** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Moonshot AI** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **MiniMax** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **302.AI** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **SAP AI Core** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Vercel AI Gateway** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Venice AI** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **IO.NET** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **GitLab Duo** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **FrogBot** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Baseten** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Helicone** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Cortecs** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **OVHcloud AI Endpoints** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **STACKIT** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Nebius Token Factory** | ✅ | ❌ | ❌ | ❌ | ❌ |
| **Z.AI** | ✅ | ❌ | ❌ | ❌ | ❌ |

\* Codex CLI can work with Ollama via a local OpenAI-compatible proxy, but has no
native Ollama integration.

## Detailed Agent Breakdown

### OpenCode

- **Total providers**: 75+
- **Architecture**: Uses the [AI SDK](https://sdk.vercel.ai) and
  [Models.dev](https://models.dev) for broad provider coverage
- **Local model support**: Ollama, llama.cpp, LM Studio
- **Custom providers**: Any OpenAI-compatible endpoint via `baseURL` override
- **Proprietary offerings**: OpenCode Zen (curated models) and OpenCode Go
  (low-cost subscription)
- **Configuration**: `opencode.json` with provider-specific sections; API keys
  stored in `~/.local/share/opencode/auth.json` via `/connect` command
- **Notable**: Widest provider coverage of all five agents; supports niche and
  regional providers (302.AI, STACKIT, OVHcloud, Scaleway, etc.)

### OpenAI Codex CLI

- **Total providers**: 1 (OpenAI native)
- **Architecture**: Direct OpenAI API integration
- **Supported models**: GPT-5.4 (default), GPT-5.4 Mini, GPT-5.3-Codex,
  GPT-5.2 Codex, GPT-5.2, o4-mini, Codex Mini
- **Local model support**: None native; works via OpenAI-compatible proxies
  (Ollama, LM Studio, OpenRouter)
- **Custom providers**: Can point to any OpenAI-compatible `baseURL`
- **Configuration**: `--provider` flag and `CODEX_HOME/config.json`
- **Pricing**: Free open-source tool; model costs through OpenAI API or
  ChatGPT subscription tiers (Free → Pro $200/mo)
- **Notable**: Sandboxed execution environment; tightly integrated with OpenAI
  model ecosystem; limited multi-provider support by design

### Claude Code

- **Total providers**: 1 (Anthropic native) + 2 cloud routes
- **Architecture**: Direct Anthropic API integration
- **Supported models**: Claude Opus 4, Claude Sonnet 4, Claude Haiku 3.5
- **Cloud routes**: Amazon Bedrock, Google Vertex AI (via
  `CLAUDE_CODE_USE_BEDROCK` / `CLAUDE_CODE_USE_VERTEX` env vars)
- **Local model support**: None
- **Custom providers**: None native; third-party proxies (Portkey, OpenRouter)
  can bridge to other providers
- **Configuration**: Environment variables for provider routing
- **Pricing**: Anthropic API pricing or Claude subscription (Pro $20/mo,
  Max $100/mo)
- **Notable**: Best-in-class reasoning with Claude Opus; closed source; strong
  single-provider focus; third-party workarounds exist for multi-provider use

### GitHub Copilot CLI

- **Total providers**: 3 (OpenAI, Anthropic, Google) + BYOK
- **Architecture**: GitHub-hosted model routing with enterprise controls
- **Supported models**: GPT-4o, GPT-4.1, Claude 3.5 Sonnet, Gemini 2.5 Pro,
  Gemini 2.5 Flash, o3-mini, o4-mini, and auto-selection mode
- **BYOK support**: Bring-your-own-key for external providers via plugins
- **Local model support**: Yes, with local LLM endpoints
- **Custom providers**: Plugin system and BYOK for custom endpoints
- **Configuration**: GitHub authentication + settings; enterprise policies for
  model governance
- **Pricing**: Copilot Free / Pro ($19/mo) / Business ($39/mo) / Enterprise
  ($39/user/mo); premium models have request multiplier costs
- **Notable**: Enterprise-ready with FedRAMP support, model governance
  policies, and usage metering; plugin/extension ecosystem for customisation;
  closed source

### ragent

- **Total providers**: 11 (9 cloud + 2 local)
- **Architecture**: Custom `Provider` trait with `ProviderRegistry`; each
  provider is a dedicated Rust module
- **Native providers**:
  1. **Anthropic** — Claude models with thinking/reasoning support
  2. **OpenAI** — GPT models
  3. **Google Gemini** — Gemini models
  4. **Ollama** — Local model inference (any GGUF model)
  5. **Ollama Cloud** — Cloud-hosted Ollama service
  6. **Hugging Face** — HF Inference API models
  7. **GitHub Copilot** — Copilot models (with request multiplier support)
  8. **Generic OpenAI** — Any OpenAI-compatible endpoint (custom `base_url`
     and API key)
  9. **Azure AI Foundry** — Azure-hosted models with dynamic model discovery
  10. **Azure Resource (File)** — Azure file/resource provider with
      `azureresources.json` support
  11. **Amazon Bedrock** — AWS-hosted models (Claude, Nova, Llama, Mistral)
      with AWS SigV4 authentication, dual API support (Anthropic Messages +
      Converse), and dynamic model discovery
- **Local model support**: Ollama (native, first-class)
- **Custom providers**: Generic OpenAI-compatible endpoint with configurable
  `base_url` — covers LM Studio, vLLM, LocalAI, and any OpenAI API-compatible
  server
- **Configuration**: `ragent.json` (or `.jsonc`) with per-provider sections;
  environment variable support (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`,
  `AWS_ACCESS_KEY_ID`, etc.)
- **Pricing**: Free open-source tool; model costs through individual provider
  APIs
- **Notable**: Rust-native performance; thinking/reasoning budget control
  per model; unique Azure AI Foundry and Azure Resource providers; Amazon
  Bedrock with self-contained SigV4 signing (zero AWS SDK dependency);
  compiled as single statically-linked binary with zero runtime dependencies

## Feature Comparison

| Feature | OpenCode | Codex CLI | Claude Code | Copilot CLI | ragent |
|---|:---:|:---:|:---:|:---:|:---:|
| Open source | ✅ | ✅ | ❌ | ❌ | ✅ |
| Single binary | ❌ (Go) | ❌ (Node) | ❌ (Node) | ❌ (Node) | ✅ (Rust) |
| Zero runtime deps | ❌ | ❌ | ❌ | ❌ | ✅ |
| Thinking/reasoning config | ❌ | ❌ | ❌ | ❌ | ✅ |
| MCP client | ✅ | ❌ | ✅ | ✅ | ✅ |
| Local Ollama (native) | ✅ | ❌ | ❌ | ✅ | ✅ |
| Azure AI Foundry | ❌ | ❌ | ❌ | ✅ | ✅ |
| Amazon Bedrock | ✅ | ❌ | ✅ | ❌ | ✅ |
| Google Vertex AI | ❌ | ❌ | ✅ | ❌ | ❌ |
| Generic OpenAI endpoint | ✅ | ✅ | ❌ | ✅ | ✅ |
| Hugging Face | ✅ | ❌ | ❌ | ❌ | ✅ |
| Sandbox execution | ❌ | ✅ | ❌ | ❌ | ❌ |
| Team/swarm agents | ❌ | ❌ | ❌ | ✅ | ✅ |
| Codebase indexing | ❌ | ❌ | ❌ | ✅ | ✅ |

## Key Takeaways

1. **Broadest coverage**: OpenCode leads with 75+ providers via the AI SDK,
   making it ideal for teams that need niche or regional cloud providers.

2. **Deepest single-provider integration**: Claude Code offers the best
   experience for Anthropic models with native Bedrock and Vertex AI routing,
   but is locked to a single model family.

3. **Enterprise governance**: GitHub Copilot CLI provides the strongest
   enterprise story with FedRAMP support, model governance policies, and usage
   metering across its three supported providers.

4. **Balanced multi-provider with local-first**: ragent offers a curated set
   of 11 providers covering all major cloud providers plus native Ollama
   support, with the unique advantage of being a single static binary and
   offering thinking/reasoning budget controls per model. The Amazon Bedrock
   provider adds native AWS integration with self-contained SigV4 signing.

5. **OpenAI ecosystem focus**: Codex CLI is purpose-built for OpenAI models
   with a sandboxed execution model, making it the best choice for teams
   standardised on OpenAI.

6. **Local model support**: OpenCode, Copilot CLI, and ragent all support
   local inference via Ollama. Codex CLI can work through proxies. Claude Code
   has no local model path.