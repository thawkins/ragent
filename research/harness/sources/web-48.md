# Web source

- URL: https://github.com/NousResearch/hermes-agent/blob/main/website/docs/integrations/providers.md
- Title: hermes-agent/website/docs/integrations/providers.md at main · NousResearch/hermes-agent
- Author(s): —
- Language: English
- Published (UTC): —
- Captured (UTC): 2026-09-05T10:27:23.952037014+00:00
- Relevance: High — title + snippet match query


```text
1621 lines (1184 loc) · 83.6 KBtitleLLM and Model Providerssidebar_labelAI Providerssidebar_position1This page covers setting up inference providers for Hermes Agent — from cloud APIs like OpenRouter and Anthropic, to self-hosted endpoints like Ollama and vLLM, to advanced routing and fallback configurations. You need at least one provider configured to use Hermes.
You need at least one way to connect to an LLM. Use hermes model to switch providers and models interactively, or configure directly:
ProviderSetupNous Portalhermes model (OAuth, subscription-based)OpenAI Codexhermes model → ChatGPT or Codex Subscription (ChatGPT OAuth, uses Codex models)GitHub Copilothermes model (OAuth device code flow, COPILOT_GITHUB_TOKEN, GH_TOKEN, or gh auth token)GitHub Copilot ACPhermes model (spawns local copilot --acp --stdio)Anthropichermes model (Claude Max + extra usage credits via OAuth; also supports Anthropic API key or manual setup-token — see note below)OpenRouterOPENROUTER_API_KEY in ~/.hermes/.envRamp RouterRAMP_ROUTER_API_KEY in ~/.hermes/.env (provider: router; aliases: ramp-router, ramp, router.com; Responses-native gateway, live account-scoped catalog)Fireworks AIFIREWORKS_API_KEY in ~/.hermes/.env (provider: fireworks; aliases: fireworks-ai, fw)NovitaAINOVITA_API_KEY in ~/.hermes/.env (provider: novita, 200+ models, Model API, Agent Sandbox, GPU Cloud)AI GatewayAI_GATEWAY_API_KEY in ~/.hermes/.env (provider: ai-gateway)z.ai / GLMGLM_API_KEY in ~/.hermes/.env (provider: zai)Kimi / MoonshotKIMI_API_KEY in ~/.hermes/.env (provider: kimi-coding)Kimi / Moonshot (China)KIMI_CN_API_KEY in ~/.hermes/.env (provider: kimi-coding-cn; aliases: kimi-cn, moonshot-cn)Arcee AIARCEEAI_API_KEY in ~/.hermes/.env (provider: arcee; aliases: arcee-ai, arceeai)GMI CloudGMI_API_KEY in ~/.hermes/.env (provider: gmi; aliases: gmi-cloud, gmicloud)Nebius Token FactoryNEBIUS_API_KEY in ~/.hermes/.env (provider: nebius-token-factory; aliases: nebius, nebius-tf, tokenfactory)Actual ComputerACTUAL_API_KEY in ~/.hermes/.env for the hosted relay, or ACTUAL_BASE_URL=http://127.0.0.1:8080 for the local daemon — no key needed on loopback (provider: actual; aliases: actual-computer, actualcomputer, aci)MiniMaxMINIMAX_API_KEY in ~/.hermes/.env (provider: minimax)MiniMax ChinaMINIMAX_CN_API_KEY in ~/.hermes/.env (provider: minimax-cn)xAI (Grok) — Responses APIXAI_API_KEY in ~/.hermes/.env (provider: xai)xAI Grok OAuth (SuperGrok)hermes model → "xAI Grok OAuth (SuperGrok / Premium+)" — browser login, no API key. See guideQwen Cloud (Alibaba DashScope)DASHSCOPE_API_KEY in ~/.hermes/.env (provider: alibaba; mainland-China endpoint: alibaba-cn)Alibaba Cloud (Coding Plan)ALIBABA_CODING_PLAN_API_KEY (falls back to DASHSCOPE_API_KEY) (provider: alibaba-coding-plan, alias: alibaba_coding; mainland-China endpoint: alibaba-coding-plan-cn with ALIBABA_CODING_PLAN_CN_API_KEY, falling back to the shared keys) — separate billing SKU, different endpointAlibaba Cloud (Token Plan)ALIBABA_TOKEN_PLAN_API_KEY in ~/.hermes/.env (provider: alibaba-token-plan; mainland-China endpoint: alibaba-token-plan-cn with ALIBABA_TOKEN_PLAN_CN_API_KEY, falling back to the shared key) — Model Studio flat-token tierKilo CodeKILOCODE_API_KEY in ~/.hermes/.env (provider: kilocode)Xiaomi MiMoXIAOMI_API_KEY in ~/.hermes/.env (provider: xiaomi, aliases: mimo, xiaomi-mimo)Tencent TokenHubTOKENHUB_API_KEY in ~/.hermes/.env (provider: tencent-tokenhub, aliases: tencent, tokenhub, tencentmaas)Tencent TokenPlanTOKENPLAN_API_KEY in ~/.hermes/.env (provider: tencent-tokenplan, aliases: tokenplan, tencent-lkeap; Anthropic Messages endpoint)OpenCode ZenOPENCODE_ZEN_API_KEY in ~/.hermes/.env (provider: opencode-zen)CommandCodeCOMMANDCODE_API_KEY in ~/.hermes/.env (provider: commandcode, alias: commandcode-chat; Claude models via commandcode-anthropic, alias: commandcode-claude). Works with GOAT/Pro/Max/Provider plans (not the $1 Go plan — no API access).OpenCode GoOPENCODE_GO_API_KEY in ~/.hermes/.env (provider: opencode-go)OpenCode FreeKeyless — no API key or account needed (provider: opencode-free, aliases: free, opencode_free). Select via hermes model or /model free; requests are sent anonymously. The model list refreshes automatically from OpenCode's live catalog, so rotating free promotions appear (and delisted ones disappear) without a Hermes updateDeepSeekDEEPSEEK_API_KEY in ~/.hermes/.env (provider: deepseek)Hugging FaceHF_TOKEN in ~/.hermes/.env (provider: huggingface, aliases: hf)Google / GeminiGOOGLE_API_KEY (or GEMINI_API_KEY) in ~/.hermes/.env (provider: gemini)Google Vertex AIhermes model → "Google Vertex AI" (provider: vertex; OAuth2 via service-account JSON or ADC, GCP billing)OpenAI API (direct)OPENAI_API_KEY in ~/.hermes/.env (provider: openai-api, optional OPENAI_BASE_URL)Azure AI Foundryhermes model → "Azure AI Foundry" (provider: azure-foundry; uses Azure OpenAI / Foundry endpoint and key)AWS Bedrockhermes model → "AWS Bedrock" (provider: bedrock; standard AWS credentials chain via boto3)NVIDIA BuildNVIDIA_API_KEY in ~/.hermes/.env (provider: nvidia; NIM-hosted models on build.nvidia.com)Ollama Cloudhermes model → "Ollama Cloud" (provider: ollama-cloud; cloud-hosted Ollama API)Qwen OAuthhermes model → "Qwen OAuth" (provider: qwen-oauth; browser PKCE login)MiniMax OAuthhermes model → "MiniMax (OAuth)" (provider: minimax-oauth; browser PKCE login)StepFunSTEPFUN_API_KEY in ~/.hermes/.env (provider: stepfun)LM Studiohermes model → "LM Studio" (provider: lmstudio, optional LM_API_KEY)Custom Endpointhermes model → choose "Custom endpoint" (saved in config.yaml)All three OpenCode providers send an opaque, per-conversation x-opencode-session header on every request (main turns on every transport plus auxiliary calls such as compression and titles). OpenCode uses it to pin a conversation to one backend so its prompt cache stays warm; the value is derived from the Hermes session id and carries no personal data.
For the official API-key path, see the dedicated Google Gemini guide.
:::tip Model key alias
In the model: config section, you can use either default: or model: as the key name for your model ID. Both model: { default: my-model } and model: { model: my-model } work identically.
:::
Nous Portal is Nous Research's unified subscription gateway and the recommended way to run Hermes Agent. One OAuth login covers 300+ frontier agentic models (Claude, GPT, Gemini, DeepSeek, Qwen, Kimi, GLM, MiniMax, Grok, ...) plus the Tool Gateway (web search, image generation, TTS, browser automation) — billed against your Nous subscription instead of separate per-provider accounts.
hermes setup --portal     # fresh install — OAuth + provider + gateway in one command
hermes model              # existing install — pick "Nous Portal" from the list
hermes portal info        # inspect login + routing at any timeDon't have a subscription yet? Get one at portal.nousresearch.com/manage-subscription.
For full details: see the dedicated Nous Portal integration page (what's in the subscription, model catalog, troubleshooting) and the step-by-step Run Hermes Agent with Nous Portal guide.
Client identification. Every Portal request from Hermes Agent carries a client=hermes-client-v<version> tag (e.g. client=hermes-client-v0.13.0) auto-aligned to your installed release. This is sent on all Portal pathways — main chat loop, auxiliary calls, compression summarizer, web extraction — and lets Portal-side telemetry distinguish Hermes traffic from other clients. No config required; the tag updates automatically when you hermes update.
JWT auth (automatic). Hermes prefers scoped inference:invoke JWTs for Portal requests with the legacy opaque session-key path as a fallback. No configuration is required — credentials are managed by the OAuth flow and rotate transparently. Revoked refresh tokens are quarantined to avoid replay loops.
:::info Codex Note
The OpenAI Codex provider authenticates via device code (open a URL, enter a code). Hermes stores the resulting credentials in its own auth store under ~/.hermes/auth.json and can import existing Codex CLI credentials from ~/.codex/auth.json when present. No Codex CLI installation is required.
If a token refresh fails with a terminal error (HTTP 4xx, invalid_grant, revoked grant, etc.), Hermes marks the refresh token as dead and stops replaying it so you don't see a flood of identical auth failures. The next request surfaces a typed re-auth message instead. Run hermes auth add openai-codex (or hermes model → ChatGPT or Codex Subscription) to start a fresh device-code login; the quarantine clears on the next successful exchange.
:::
:::warning
Even when using Nous Portal, Codex, or a custom endpoint, some tools (vision, web summarization, MoA) use a separate "auxiliary" model. By default (auxiliary.*.provider: "auto"), Hermes routes these tasks to your main chat model — the same model you picked in hermes model. You can override each task individually to route it to a cheaper/faster model (e.g. Gemini Flash on OpenRouter) — see Auxiliary Models.
:::
:::tip Nous Tool Gateway
Paid Nous Portal subscribers also get access to the Tool Gateway — web search, image generation, TTS, and browser automation routed through your subscription. No extra API keys needed. On a fresh install, hermes setup --portal logs you in, sets Nous as your provider, and turns the gateway on in one command. Existing users can enable it from hermes model or per-tool from hermes tools. Inspect routing at any time with hermes portal info.
:::
Two Commands for Model ManagementHermes has two model commands that serve different purposes:
CommandWhere to runWhat it doeshermes modelYour terminal (outside any session)Full setup wizard — add providers, run OAuth, enter API keys, configure endpoints/modelInside a Hermes chat sessionQuick switch between already-configured providers and modelsIf you're trying to switch to a provider you haven't set up yet (e.g. you only have OpenRouter configured and want to use Anthropic), you need hermes model, not /model. Exit your session first (Ctrl+C or /quit), run hermes model, complete the provider setup, then start a new session.
Subscription plans: what your plan pays forSeveral providers let you sign in to Hermes with a consumer subscription (Claude Max, ChatGPT, SuperGrok / X Premium+, …) instead of an API key. What that subscription actually pays for — and what it doesn't — differs per provider, and it's the single most common source of billing surprises. The table below is the short version; each provider's own section has the details.
Cells marked not currently documented mean exactly that: Hermes docs do not yet specify the behavior. Don't assume — check your provider's billing dashboard, and treat these as open questions.Plan / pathCan Hermes use it?What gets consumedWhat does NOT get consumedCommon surpriseAnthropic — Claude Max + OAuth✅ Yes — hermes model → Anthropic OAuth. Requires Max and purchased extra usage creditsThe extra/overage credits you've added on top of the Max planThe base Max plan allowance (the usage included in Claude Code by default)All Hermes usage bills as "extra usage" even while your included Max allowance sits untouchedAnthropic — Claude Pro❌ No — Pro subscribers cannot use the OAuth pathNothing (path unavailable)Your Pro subscriptionPro looks like it should work; it doesn't. Use an ANTHROPIC_API_KEY instead (pay-per-token, independent of any Claude subscription)OpenAI Codex — ChatGPT plan OAuth✅ Yes — hermes model → ChatGPT or Codex Subscription (ChatGPT OAuth device-code login, uses Codex models)Not currently documentedNot currently documentedDocs cover auth and token refresh only; plan-quota semantics are not yet documentedxAI — SuperGrok / X Premium+ OAuth✅ Yes — browser OAuth, no API key neededYour subscription quota (documented explicitly for X Search: OAuth is preferred over an API key and "uses your subscription quota instead of API spend"). Inference quota semantics beyond that: not currently documentedXAI_API_KEY / pay-per-token API spend, when OAuth credentials are configured and preferredHTTP 403 after a successful login — xAI has restricted OAuth API access to specific SuperGrok tiers despite an active in-app subscriptionGoogle — Gemini consumer plan (Google AI Pro / Ultra)❌ No documented path — the gemini provider is API-key only (GOOGLE_API_KEY / GEMINI_API_KEY); Vertex AI uses GCP billingYour API key's quota (free tier or billing-enabled Google Cloud project) — consumer-plan consumption not currently documentedNot currently documentedFree-tier keys can be exhausted after a handful of agent turns, because Hermes may make several model calls per user turnAnthropic. The OAuth path routes as Claude Code against your Anthropic account and only works on a Claude Max plan with purchased extra usage credits — the base Max allowance is never consumed by Hermes, only the extra/overage credits on top. Claude Pro subscribers cannot use this path; the supported alternative is an ANTHROPIC_API_KEY, billed pay-per-token against that key's organization at standard API pricing. See Anthropic (Native) below.
OpenAI Codex. Hermes authenticates via ChatGPT device-code OAuth, stores credentials in ~/.hermes/auth.json, and can import existing Codex CLI credentials from ~/.codex/auth.json. Which ChatGPT plan tiers are eligible, and how Hermes usage counts against your plan's Codex limits, are not currently documented — the Codex note under Nous Portal covers authentication and token-refresh behavior only.
xAI (SuperGrok / X Premium+). Browser OAuth works with either an active SuperGrok subscription or an X Premium+ subscription on the linked X account, and the same bearer token is reused by direct-to-xAI tools (TTS, image gen, video gen, transcription, X Search). If inference returns HTTP 403 after a successful login, that's a tier/entitlement restriction on xAI's side, not a stale token — the workaround is switching to an XAI_API_KEY. See xAI (Grok) below and the xAI Grok OAuth guide.
Google Gemini. There is currently no way to sign in to Hermes with a consumer Gemini subscription — the gemini provider takes an API key, and Google Vertex AI bills to your GCP project. A billing-enabled Google Cloud project is recommended for agent use; free-tier quotas are too small for long-running agent sessions. See the Google Gemini guide.
:::tip One subscription instead of five
If you'd rather not track per-provider plan semantics at all, Nous Portal covers 300+ models under a single subscription with one OAuth login.
:::
Use Claude models directly through the Anthropic API — no OpenRouter proxy needed. Supports three auth methods:
:::caution Requires Claude Max "extra usage" credits
When you authenticate via hermes model → Anthropic OAuth (or via hermes auth add anthropic --type oauth), Hermes routes as Claude Code against your Anthropic account. It only works if you're on a Claude Max plan and have purchased extra usage credits. The base Max plan allowance (the usage included in Claude Code by default) is not consumed by Hermes — only the extra/overage credits you've added on top are. Claude Pro subscribers cannot use this path.
If you don't have Max + extra credits, use an ANTHROPIC_API_KEY instead — requests are billed pay-per-token against that key's organization (standard API pricing, independent of any Claude subscription).
:::
# With an API key (pay-per-token)export ANTHROPIC_API_KEY=***
hermes chat --provider anthropic --model claude-sonnet-4-6

# Preferred: authenticate through `hermes model`# Hermes will use Claude Code's credential store directly when available
hermes model

# Manual override with a setup-token (fallback / legacy)export ANTHROPIC_TOKEN=***# setup-token or manual OAuth token
hermes chat --provider anthropic

# Auto-detect Claude Code credentials (if you already use Claude Code)
hermes chat --provider anthropic  # reads Claude Code credential files automaticallyWhen you choose Anthropic OAuth through hermes model, Hermes prefers Claude Code's own credential store over copying the token into ~/.hermes/.env. That keeps refreshable Claude credentials refreshable.
Or set it permanently:
model:
  provider: "anthropic"default: "claude-sonnet-4-6":::tip Aliases
--provider claude and --provider claude-code also work as shorthand for --provider anthropic.
:::
Hermes supports GitHub Copilot as a first-class provider with two modes:
copilot — Direct Copilot API (recommended). Uses your GitHub Copilot subscription to access GPT-5.x, Claude, Gemini, and other models through the Copilot API.
hermes chat --provider copilot --model gpt-5.4Authentication options (checked in this order):
COPILOT_GITHUB_TOKEN environment variableGH_TOKEN environment variableGITHUB_TOKEN environment variablegh auth token CLI fallbackIf no token is found, hermes model offers an OAuth device code login — the same flow used by the Copilot CLI and opencode.
:::warning Token types
The Copilot API does not support classic Personal Access Tokens (ghp_*). Supported token types:
TypePrefixHow to getOAuth tokengho_hermes model → GitHub Copilot → Login with GitHubFine-grained PATgithub_pat_GitHub Settings → Developer settings → Fine-grained tokens (needs Copilot Requests permission)GitHub App tokenghu_Via GitHub App installationIf your gh auth token returns a ghp_* token, use hermes model to authenticate via OAuth instead.
:::
:::info Copilot auth behavior in Hermes
Hermes sends a supported GitHub token (gho_*, github_pat_*, or ghu_*) directly to api.githubcopilot.com and includes Copilot-specific headers (Editor-Version, Copilot-Integration-Id, Openai-Intent, x-initiator).
On HTTP 401, Hermes now performs a one-shot credential recovery before fallback:
Re-resolve token via the normal priority chain (COPILOT_GITHUB_TOKEN → GH_TOKEN → GITHUB_TOKEN → gh auth token)Rebuild the shared OpenAI client with refreshed headersRetry the request onceSome older community proxies use api.github.com/copilot_internal/v2/token exchange flows. That endpoint can be unavailable for some account types (returns 404). Hermes therefore keeps direct-token auth as the primary path and relies on runtime credential refresh + retry for robustness.
:::
API routing: GPT-5+ models (except gpt-5-mini) automatically use the Responses API. All other models (GPT-4o, Claude, Gemini, etc.) use Chat Completions. Models are auto-detected from the live Copilot catalog.
copilot-acp — Copilot ACP agent backend. Spawns the local Copilot CLI as a subprocess:
hermes chat --provider copilot-acp --model copilot-acp
# Requires the GitHub Copilot CLI in PATH and an existing `copilot login` sessionPermanent config:
model:
  provider: "copilot"default: "gpt-5.4"Environment variableDescriptionCOPILOT_GITHUB_TOKENGitHub token for Copilot API (first priority)HERMES_COPILOT_ACP_COMMANDOverride the Copilot CLI binary path (default: copilot)HERMES_COPILOT_ACP_ARGSOverride ACP args (default: --acp --stdio)First-Class API-Key ProvidersThese providers have built-in support with dedicated provider IDs. Set the API key and use --provider to select:
# Fireworks AI
hermes chat --provider fireworks --model accounts/fireworks/models/kimi-k2p6
# Requires: FIREWORKS_API_KEY in ~/.hermes/.env# NovitaAI Model API
hermes chat --provider novita --model moonshotai/kimi-k2.5
# Requires: NOVITA_API_KEY in ~/.hermes/.env# Ramp Router (model IDs come from your account's live catalog)
hermes chat --provider router --model gpt-5.4-mini
# Requires: RAMP_ROUTER_API_KEY in ~/.hermes/.env# z.ai / ZhipuAI GLM
hermes chat --provider zai --model glm-5
# Requires: GLM_API_KEY in ~/.hermes/.env# Kimi / Moonshot AI (international: api.moonshot.ai)
hermes chat --provider kimi-coding --model kimi-for-coding
# Requires: KIMI_API_KEY in ~/.hermes/.env# Kimi / Moonshot AI (China: api.moonshot.cn)
hermes chat --provider kimi-coding-cn --model kimi-k2.5
# Requires: KIMI_CN_API_KEY in ~/.hermes/.env# MiniMax (global endpoint)
hermes chat --provider minimax --model MiniMax-M2.7
# Requires: MINIMAX_API_KEY in ~/.hermes/.env# MiniMax (China endpoint)
hermes chat --provider minimax-cn --model MiniMax-M2.7
# Requires: MINIMAX_CN_API_KEY in ~/.hermes/.env# Qwen Cloud / DashScope (Qwen models)
hermes chat --provider alibaba --model qwen3.5-plus
# Requires: DASHSCOPE_API_KEY in ~/.hermes/.env# Xiaomi MiMo
hermes chat --provider xiaomi --model mimo-v2-pro
# Requires: XIAOMI_API_KEY in ~/.hermes/.env# Tencent TokenHub (Hy4 preview)
hermes chat --provider tencent-tokenhub --model hy4-preview
# Requires: TOKENHUB_API_KEY in ~/.hermes/.env# Tencent TokenPlan (Hy4 preview via Anthropic Messages endpoint)
hermes chat --provider tencent-tokenplan --model hy4-preview
# Requires: TOKENPLAN_API_KEY in ~/.hermes/.env# Arcee AI (Trinity models)
hermes chat --provider arcee --model trinity-large-thinking
# Requires: ARCEEAI_API_KEY in ~/.hermes/.env# Meta Model API (Muse Spark family)
hermes chat --provider meta-ai --model muse-spark-1.2
# Requires: MODEL_API_KEY in ~/.hermes/.env# GMI Cloud# Use the exact model ID returned by GMI's /v1/models endpoint.
hermes chat --provider gmi --model zai-org/GLM-5.1-FP8
# Requires: GMI_API_KEY in ~/.hermes/.env# Nebius Token Factory
hermes chat --provider nebius --model deepseek-ai/DeepSeek-V4-Pro
# Requires: NEBIUS_API_KEY in ~/.hermes/.envFireworks uses its native slash-form catalog IDs, such as accounts/fireworks/models/kimi-k2p6. Run hermes model, choose Fireworks AI, and select from the live catalog or enter another Fireworks model ID. The default endpoint is https://api.fireworks.ai/inference/v1; configure a different endpoint through model.base_url in config.yaml, not .env.
Or set the provider permanently in config.yaml:
model:
  provider: "gmi"default: "zai-org/GLM-5.1-FP8"Base URLs can be overridden with NOVITA_BASE_URL, GLM_BASE_URL, KIMI_BASE_URL, MINIMAX_BASE_URL, MINIMAX_CN_BASE_URL, DASHSCOPE_BASE_URL, XIAOMI_BASE_URL, GMI_BASE_URL, META_BASE_URL, or TOKENHUB_BASE_URL environment variables.
:::note Meta contributor tier
muse-spark-1.2-contributor and muse-spark-1.3-contributor are Meta's contributor tiers — Meta may train on your prompts and completions, so interactive model selection asks for confirmation before using either. For current pricing and rate limits, see Meta Model API pricing and rate limits. Use the standard muse-spark-1.2 / muse-spark-1.3 (no training) for confidential work.
:::
:::note Z.AI Endpoint Auto-Detection
When using the Z.AI / GLM provider, Hermes automatically probes multiple endpoints (global, China, coding variants) to find one that accepts your API key. You don't need to set GLM_BASE_URL manually — the working endpoint is detected and cached automatically.
:::
xAI (Grok) — Responses API + Prompt CachingxAI is wired through the Responses API (codex_responses transport) for automatic reasoning support on Grok 4 models — no reasoning_effort parameter needed, the server reasons by default. Set XAI_API_KEY in ~/.hermes/.env and pick xAI in hermes model, or drop grok as a shortcut into /model grok-4-fast-reasoning.
SuperGrok and X Premium+ subscribers can sign in with browser OAuth instead of using an API key — pick xAI Grok OAuth (SuperGrok / Premium+) in hermes model, or run hermes auth add xai-oauth. The same OAuth bearer token is automatically reused by direct-to-xAI tools (TTS, image gen, video gen, transcription). See the xAI Grok OAuth guide for the full flow — and if Hermes runs on a remote host, also see OAuth over SSH / Remote Hosts for the required ssh -L tunnel.
When using xAI as a provider (any base URL containing x.ai), Hermes automatically enables prompt caching by sending the x-grok-conv-id header with every API request. This routes requests to the same server within a conversation session, allowing xAI's infrastructure to reuse cached system prompts and conversation history.
No configuration is needed — caching activates automatically when an xAI endpoint is detected and a session ID is available. This reduces latency and cost for multi-turn conversations.
xAI also ships a dedicated TTS endpoint (/v1/tts). Select xAI TTS in hermes tools → Voice & TTS, or see the Voice & TTS page for config.
Retired xAI model migration (May 15, 2026): xAI is retiring grok-4*, grok-3, grok-code-fast-1, and grok-imagine-image-pro on 2026-05-15. hermes doctor and hermes chat startup both detect any config still pointing at a retired ref and print the recommended replacement. Use hermes migrate xai for a one-shot config rewrite — dry-run by default, add --apply to write changes (a timestamped config.yaml.bak-pre-migrate-xai-* backup is created automatically).
hermes migrate xai          # preview replacements
hermes migrate xai --apply  # rewrite ~/.hermes/config.yaml in placexAI Web Search backend. When the Web Search toolset is enabled, web.backend: xai routes search through xAI's hosted search endpoint using the same XAI_API_KEY / OAuth credentials. No additional setup required if xAI is already configured as a provider.
NovitaAI is the AI-native cloud for builders and agents. Its three product lines are Model API for 200+ models, Agent Sandbox for building and running AI agents, and GPU Cloud for scalable compute, all available from one platform.
# Use any available model
hermes chat --provider novita --model moonshotai/kimi-k2.5
# Requires: NOVITA_API_KEY in ~/.hermes/.env# Short alias
hermes chat --provider novita-ai --model deepseek/deepseek-v3-0324Or set it permanently in config.yaml:
model:
  provider: "novita"default: "moonshotai/kimi-k2.5"base_url: "https://api.novita.ai/openai/v1"Get your API key at novita.ai/settings/key-management. The base URL can be overridden with NOVITA_BASE_URL.
Ollama Cloud — Managed Ollama Models, OAuth + API KeyOllama Cloud hosts the same open-weight catalog as local Ollama but without the GPU requirement. Pick it in hermes model as Ollama Cloud, paste your API key from ollama.com/settings/keys, and Hermes auto-discovers the available models.
hermes model
# → pick "Ollama Cloud"# → paste your OLLAMA_API_KEY# → select from discovered models (gpt-oss:120b, glm-4.6:cloud, qwen3-coder:480b-cloud, etc.)Or config.yaml directly:
model:
  provider: "ollama-cloud"default: "gpt-oss:120b"The model catalog is fetched dynamically from ollama.com/v1/models and cached for one hour. model:tag notation (e.g. qwen3-coder:480b-cloud) is preserved through normalization — don't use dashes.
:::tip Ollama Cloud vs local Ollama
Both speak the same OpenAI-compatible API. Cloud is a first-class provider (--provider ollama-cloud, OLLAMA_API_KEY); local Ollama is reached via the Custom Endpoint flow (base URL http://localhost:11434/v1, no key). Use cloud for large models you can't run locally; use local for privacy or offline work.
:::
Anthropic Claude, Amazon Nova, DeepSeek v3.2, Meta Llama 4, and other models via AWS Bedrock. Uses the AWS SDK (boto3) credential chain — no API key, just standard AWS auth.
# Simplest — named profile in ~/.aws/credentials
hermes chat --provider bedrock --model us.anthropic.claude-sonnet-4-6

# Or with explicit env vars
AWS_PROFILE=myprofile AWS_REGION=us-east-1 hermes chat --provider bedrock --model us.anthropic.claude-sonnet-4-6Or permanently in config.yaml:
model:
  provider: "bedrock"default: "us.anthropic.claude-sonnet-4-6"bedrock:
  region: "us-east-1"# or set AWS_REGION# profile: "myprofile"       # or set AWS_PROFILE# discovery: true            # auto-discover region from IAM# guardrail:                 # optional Bedrock Guardrails#   guardrail_identifier: "your-guardrail-id"#   guardrail_version: "DRAFT"Authentication uses the standard boto3 chain: explicit AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY, AWS_PROFILE from ~/.aws/credentials, IAM role on EC2/ECS/Lambda, IMDS, or SSO. No env var is required if you're already authenticated with the AWS CLI.
Bedrock uses the Converse API under the hood — requests are translated to Bedrock's model-agnostic shape, so the same config works for Claude, Nova, DeepSeek, and Llama models. Set BEDROCK_BASE_URL only if you're calling a non-default regional endpoint.
See the AWS Bedrock guide for a walkthrough of IAM setup, region selection, and cross-region inference.
Gemini models on Google Cloud Vertex AI via Vertex's OpenAI-compatible endpoint. Authentication is OAuth2 — a short-lived access token (~1 hour) minted from a service-account JSON or Application Default Credentials (ADC). There is no static API key; Hermes mints and auto-refreshes the token for you, including re-minting on a mid-session 401.
# Service account JSON (recommended for servers / gateways)echo"VERTEX_CREDENTIALS_PATH=/path/to/service-account.json">>~/.hermes/.env
# or Application Default Credentials
gcloud auth application-default login

hermes model   # → "Google Vertex AI" → project → region → modelOr in config.yaml (project/region are non-secret and live here; the credential path stays in .env):
model:
  provider: "vertex"default: "google/gemini-3-flash-preview"# Vertex requires the google/ prefixvertex:
  project_id: "my-gcp-project"# blank → use the project embedded in the credentialsregion: "global"# required for the Gemini 3.x previewsVERTEX_PROJECT_ID / VERTEX_REGION env vars override the config.yaml values. Hermes lazy-installs google-auth on first use; run hermes setup if the managed install needs repair. See the Google Vertex AI guide for the full walkthrough, and the Google Gemini guide for the static-API-key AI Studio path instead.
Alibaba's Qwen Portal with browser-based OAuth login. Pick Qwen OAuth (Portal) in hermes model, sign in through the browser, and Hermes persists the refresh token.
hermes model
# → pick "Qwen OAuth (Portal)"# → browser opens; sign in with your Alibaba account# → confirm — credentials are saved to ~/.hermes/auth.json

hermes chat   # uses portal.qwen.ai/v1 endpointOr configure config.yaml:
model:
  provider: "qwen-oauth"default: "qwen3-coder-plus"Set HERMES_QWEN_BASE_URL only if the portal endpoint relocates (default: https://portal.qwen.ai/v1).
:::tip Qwen OAuth vs Qwen Cloud (Alibaba DashScope)
qwen-oauth uses the consumer-facing Qwen Portal with OAuth login — ideal for individual users. The alibaba provider uses Qwen Cloud (Alibaba DashScope) with a DASHSCOPE_API_KEY — ideal for programmatic / production workloads. Both route to Qwen-family models but live at different endpoints.
:::
Alibaba Cloud (Coding Plan)If you're subscribed to Alibaba's Coding Plan (a pricing SKU separate from standard DashScope API access), Hermes exposes it as its own first-class provider: alibaba-coding-plan. Endpoint: https://coding-intl.dashscope.aliyuncs.com/v1. It's OpenAI-compatible like the regular alibaba provider but with a different base URL and billing surface.
model:
  provider: alibaba_coding     # alias for alibaba-coding-planmodel: qwen3-coder-plusOr from the CLI:
hermes chat --provider alibaba_coding --model qwen3-coder-plusalibaba_coding uses the same DASHSCOPE_API_KEY your alibaba entry already uses — no separate key needed, just a different routing target. Before this provider was registered, users who set provider: alibaba_coding in config.yaml silently fell through to OpenRouter routing.
For the mainland-China endpoint (alibaba-coding-plan-cn, https://coding.dashscope.aliyuncs.com/v1) set ALIBABA_CODING_PLAN_CN_API_KEY. The CN provider still falls back to ALIBABA_CODING_PLAN_API_KEY / DASHSCOPE_API_KEY, but with only the shared key set the /model picker lists just the international row — set the CN key (or provider: alibaba-coding-plan-cn in config.yaml) to surface the CN one. The same applies to alibaba-token-plan-cn with ALIBABA_TOKEN_PLAN_CN_API_KEY.
MiniMax-M2.7 via browser OAuth login — no API key needed. Pick MiniMax (OAuth) in hermes model, sign in through the browser, and Hermes persists the access + refresh tokens. Uses the Anthropic Messages-compatible endpoint (/anthropic) under the hood.
hermes model
# → pick "MiniMax (OAuth)"# → browser opens; sign in with your MiniMax account (global or CN region)# → confirm — credentials are saved to ~/.hermes/auth.json

hermes chat   # uses api.minimax.io/anthropic endpointOr configure config.yaml:
model:
  provider: "minimax-oauth"default: "MiniMax-M2.7"Supported models: MiniMax-M2.7 (main) and MiniMax-M2.7-highspeed (wired as the default auxiliary model). The OAuth path ignores MINIMAX_API_KEY / MINIMAX_BASE_URL.
:::tip MiniMax OAuth vs API key
minimax-oauth uses MiniMax's consumer-facing portal with OAuth login — no billing setup required. The minimax and minimax-cn providers use MINIMAX_API_KEY / MINIMAX_CN_API_KEY — for programmatic access. See the MiniMax OAuth guide for a full walkthrough.
:::
Nemotron and other open source models via build.nvidia.com (free API key) or a local NIM endpoint.
# Cloud (build.nvidia.com)
hermes chat --provider nvidia --model nvidia/nemotron-3-super-120b-a12b
# Requires: NVIDIA_API_KEY in ~/.hermes/.env# Local NIM endpoint — override base URL
NVIDIA_BASE_URL=http://localhost:8000/v1 hermes chat --provider nvidia --model nvidia/nemotron-3-super-120b-a12bOr set it permanently in config.yaml:
model:
  provider: "nvidia"default: "nvidia/nemotron-3-super-120b-a12b":::tip Local NIM
For on-prem deployments (DGX Spark, local GPU), set NVIDIA_BASE_URL=http://localhost:8000/v1. NIM exposes the same OpenAI-compatible chat completions API as build.nvidia.com, so switching between cloud and local is a one-line env-var change.
:::
Hermes automatically attaches the NIM billing-origin header on every request to build.nvidia.com — no configuration needed. This routes consumption against the correct origin in NVIDIA's billing dashboard.
Open and reasoning models via GMI Cloud — OpenAI-compatible API, API key authentication.
# GMI Cloud
hermes chat --provider gmi --model deepseek-ai/DeepSeek-V3.2
# Requires: GMI_API_KEY in ~/.hermes/.envOr set it permanently in config.yaml:
model:
  provider: "gmi"default: "deepseek-ai/DeepSeek-V3.2"The base URL can be overridden with GMI_BASE_URL (default: https://api.gmi-serving.com/v1).
Your own hardware as a private inference cluster via Actual Computer. Two serving modes, both OpenAI-compatible (Hermes uses the Responses API transport):
Hosted relay — https://api.actual.inc, end-to-end encrypted, routes to your cluster. Authenticate with an ac_ inference key from actual.inc/user/keys.Local daemon — on-device at http://127.0.0.1:8080, fully offline. No API key needed: Hermes detects the loopback base URL and authenticates with an internal placeholder automatically.# Hosted relay (ACTUAL_API_KEY in ~/.hermes/.env)
hermes chat --provider actual --model <model-id-from-your-cluster># Local daemon (ACTUAL_BASE_URL=http://127.0.0.1:8080 in ~/.hermes/.env, no key)
hermes chat --provider actual --model <installed-model-name>Or set it permanently in config.yaml:
model:
  provider: "actual"default: "<model-id>"Notes:
Model IDs come from your cluster's GET /v1/models — discover with hermes model or curl -s https://api.actual.inc/v1/models -H "Authorization: Bearer $ACTUAL_API_KEY".Bare hosts are normalized: ACTUAL_BASE_URL=http://127.0.0.1:8080 becomes http://127.0.0.1:8080/v1 automatically.Reasoning effort is clamped to Actual's supported range (none/low/medium/high/max) — a global xhigh/ultra setting will not 400 requests.Small local models: Hermes' full default toolset plus the system prompt can exceed a 32k context window, producing an empty-stream error from llama.cpp-family servers. Restrict the toolset (-t file,web) or load the model with a larger context. The optional actual-setup skill (hermes skills install official/devops/actual-setup) covers setup and troubleshooting in detail.Aliases: actual-computer, actualcomputer, aci.Step-series models via StepFun — OpenAI-compatible API, API key authentication.
# StepFun
hermes chat --provider stepfun --model step-3.5-flash
# Requires: STEPFUN_API_KEY in ~/.hermes/.envOr set it permanently in config.yaml:
model:
  provider: "stepfun"default: "step-3.5-flash"The base URL can be overridden with STEPFUN_BASE_URL (default: https://api.stepfun.com/v1).
Hugging Face Inference ProvidersHugging Face Inference Providers routes to 20+ open models through a unified OpenAI-compatible endpoint (router.huggingface.co/v1). Requests are automatically routed to the fastest available backend (Groq, Together, SambaNova, etc.) with automatic failover.
# Use any available model
hermes chat --provider huggingface --model Qwen/Qwen3.5-397B-A17B
# Requires: HF_TOKEN in ~/.hermes/.env# Short alias
hermes chat --provider hf --model deepseek-ai/DeepSeek-V3.2Or set it permanently in config.yaml:
model:
  provider: "huggingface"default: "Qwen/Qwen3.5-397B-A17B"Get your token at huggingface.co/settings/tokens — make sure to enable the "Make calls to Inference Providers" permission. Free tier included ($0.10/month credit, no markup on provider rates).
You can append routing suffixes to model names: :fastest (default), :cheapest, or :provider_name to force a specific backend.
The base URL can be overridden with HF_BASE_URL.
Custom & Self-Hosted LLM ProvidersHermes Agent works with any OpenAI-compatible API endpoint. If a server implements /v1/chat/completions, you can point Hermes at it. This means you can use local models, GPU inference servers, multi-provider routers, or any third-party API.
Three ways to configure a custom endpoint:
Interactive setup (recommended):
hermes model
# Select "Custom endpoint (self-hosted / VLLM / etc.)"# Enter: API base URL, API key, Model nameManual config (config.yaml):
# In ~/.hermes/config.yamlmodel:
  default: your-model-nameprovider: custombase_url: http://localhost:8000/v1api_key: your-key-or-leave-empty-for-local:::warning Legacy env vars
LLM_MODEL in .env is removed — config.yaml is the single source of truth for model and endpoint configuration. OPENAI_BASE_URL is still honored, but only for the openai-api provider (it overrides the OpenAI endpoint for direct API-key access). For other providers and custom endpoints, use hermes model or set model.base_url in config.yaml directly. If you have stale entries in your .env, they are automatically cleared on the next hermes setup or config migration.
:::
Both approaches persist to config.yaml, which is the source of truth for model, provider, and base URL.
Switching Models with /model:::warning hermes model vs /model
hermes model (run from your terminal, outside any chat session) is the full provider setup wizard. Use it to add new providers, run OAuth flows, enter API keys, and configure custom endpoints.
/model (typed inside an active Hermes chat session) can only switch between providers and models you've already set up. It cannot add new providers, run OAuth, or prompt for API keys. If you've only configured one provider (e.g. OpenRouter), /model will only show models for that provider.
To add a new provider: Exit your session (Ctrl+C or /quit), run hermes model, set up the new provider, then start a new session.
:::
Once you have at least one custom endpoint configured, you can switch models mid-session:
/model custom:qwen-2.5          # Switch to a model on your custom endpoint
/model custom                    # Auto-detect the model from the endpoint
/model openrouter:claude-sonnet-4 # Switch back to a cloud provider
If you have named custom providers configured (see below), use the triple syntax:
/model custom:local:qwen-2.5    # Use the "local" custom provider with model qwen-2.5
/model custom:work:llama3       # Use the "work" custom provider with llama3
When switching providers, Hermes persists the base URL and provider to config so the change survives restarts. When switching away from a custom endpoint to a built-in provider, the stale base URL is automatically cleared.
:::tip
/model custom (bare, no model name) queries your endpoint's /models API and auto-selects the model if exactly one is loaded. Useful for local servers running a single model.
:::
Everything below follows this same pattern — just change the URL, key, and model name.
```
