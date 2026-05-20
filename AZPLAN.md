# AZPLAN — Azure Model Info Discovery Plan

## Objective
Determine how to get the proper model name (not just deployment ID) from the Azure OpenAI Service endpoint used in `scripts/getresult.sh`.

## Current Endpoint Analysis

The script uses:
```
POST https://a1a-52048-dev-ais-shr1-eus2-1.openai.azure.com/openai/v1/chat/completions
```

This is an **Azure OpenAI Service** resource endpoint (not Azure AI Foundry). The pattern `*.openai.azure.com` indicates the classic Azure OpenAI Service deployment model where:
- You deploy a model with a deployment name (e.g., `Kimi-K2.5`)
- The deployment name is NOT the actual model name
- The actual model could be something like `gpt-4o`, `gpt-4o-mini`, etc.

## Finding: `/openai/info` Does NOT Exist

### Research Results
- ❌ **No `/openai/info` endpoint** exists in Azure OpenAI Service or Azure AI Foundry APIs
- ❌ Not documented in Microsoft Learn Azure OpenAI REST API reference
- ❌ Not part of the OpenAI-compatible endpoint specification

### What Actually Exists

#### 1. Azure OpenAI Service — `/models` Endpoint
```
GET https://{resource-name}.openai.azure.com/openai/models?api-version=2024-10-21
```

**Headers:**
- `api-key: {your-api-key}` (or `Authorization: Bearer {token}`)

**Response:**
```json
{
  "data": [
    {
      "id": "gpt-4o",
      "object": "model",
      "created": 1686935002,
      "owned_by": "system"
    },
    {
      "id": "gpt-4o-mini",
      "object": "model",
      "created": 1721172741,
      "owned_by": "system"
    }
  ]
}
```

⚠️ **Limitation:** The `/models` endpoint returns the **available models** for the service, not the **deployed models** with their deployment names.

#### 2. Azure OpenAI Service — List Deployments
To get deployment names mapped to actual model names, use the Azure Management API:
```
GET https://management.azure.com/subscriptions/{subscription-id}/resourceGroups/{resource-group}/providers/Microsoft.CognitiveServices/accounts/{account-name}/deployments?api-version=2024-10-01
```

**Response includes:**
```json
{
  "value": [
    {
      "id": "/subscriptions/.../deployments/Kimi-K2.5",
      "name": "Kimi-K2.5",
      "properties": {
        "model": {
          "format": "OpenAI",
          "name": "gpt-4o",        ← Actual model name
          "version": "2024-05-13"
        }
      }
    }
  ]
}
```

⚠️ **Requires:** Azure Management API access (different from data plane API key).

#### 3. Azure AI Foundry — Different Endpoint Pattern
Azure AI Foundry uses:
```
https://services.ai.azure.com/models
```
Not `*.openai.azure.com`.

## Recommended Approach

### Option A: Query Available Models (Simplest)
Use the user's endpoint to list available models:

```bash
#!/bin/sh
# getmodelinfo.sh — Query Azure OpenAI Service models

  ENDPOINT="https://a1a-52048-dev-ais-shr1-eus2-1.openai.azure.com"
  API_KEY="***REDACTED***"
curl -s "${ENDPOINT}/openai/models?api-version=2024-10-21" \
  -H "api-key: ${API_KEY}" \
  | jq -r '.data[] | "\(.id): owned by \(.owned_by)"'
```

**Limitation:** Shows available models, not which model is behind the `Kimi-K2.5` deployment.

### Option B: Infer from Deployment Name
The deployment name `Kimi-K2.5` suggests the actual model is likely:
- **Moonshot AI Kimi K2.5** — a third-party model available through Azure AI Foundry (not Azure OpenAI Service)

This indicates the endpoint may actually be an Azure AI Foundry model catalog endpoint masquerading as an OpenAI-compatible endpoint.

### Option C: Check Model via Chat Response Headers
Some Azure OpenAI deployments return model info in response headers:

```bash
curl -s -D - -X POST "${ENDPOINT}/openai/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -H "api-key: ${API_KEY}" \
  -d '{
    "model": "Kimi-K2.5",
    "messages": [{"role":"user","content":"hello"}],
    "max_tokens": 1
  }' 2>&1 | grep -i model
```

### Option D: Use Azure AI Foundry /models Endpoint (If Applicable)
If this is actually an Azure AI Foundry endpoint:
```bash
curl -s "https://services.ai.azure.com/models" \
  -H "api-key: ${API_KEY}"
```

## Plan for `ragent` Integration

1. **Add endpoint detection** — Determine if the user is using Azure OpenAI Service (`*.openai.azure.com`) or Azure AI Foundry (`services.ai.azure.com`)

2. **Try `/models` first** — Query the models endpoint to list available models

3. **Cache deployment → model mapping** — Store discovered mappings in ragent's settings store

4. **Warn about deployment name vs model name** — When using Azure OpenAI Service, the deployment name may differ from the actual model name

## Files to Modify

| File | Change |
|------|--------|
| `crates/ragent-llm/src/providers/azure_foundry.rs` | Add `/models` discovery endpoint call |
| `crates/ragent-llm/src/providers/openai.rs` | Reuse model discovery for Azure OpenAI Service |
| `crates/ragent-tui/src/app.rs` | Display actual model name alongside deployment name |

## Decision

❌ **Do NOT implement `/openai/info`** — This endpoint does not exist.

✅ **Implement Option A** — Query `/openai/models` endpoint to discover available models.

✅ **Document Option B** — If deployment name ≠ model name, warn the user.
