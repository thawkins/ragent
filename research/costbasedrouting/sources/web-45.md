# Web source

- URL: https://ai.azure.com/catalog/models/model-router
- Title: [[Microsoft Foundry Logo]Microsoft Foundry][1]/[Catalog][2]/[Models][3]/model-router
- Captured (UTC): 2026-06-29T15:42:37.193150630+00:00

```text
[[Microsoft Foundry Logo]Microsoft Foundry][1]/[Catalog][2]/[Models][3]/model-router
Models
Providers
[model-router]

# model-router

DetailsLicenseLicense
Model router is a deployable AI model that is trained to select the most suitable large language model (LLM) for a given
prompt.
Microsoft
Direct from Azure
Version: 2025-11-18

Welcome to the **`2025-11-18`** version of model router.

Model Router dynamically selects the optimal large language model (LLM) for a specific query or task in real time. By
evaluating factors like query complexity, cost, and performance, it efficiently routes requests to the most suitable
model, ensuring high quality results while minimizing costs.

This version adds several new capabilities:
1. Support Global Standard and Data Zone Standard deployments.
2. Adds support for new models: `grok-4`, `grok-4-1-fast-reasoning`, `DeepSeek-V3.1`, `DeepSeek-V3.2`, `gpt-oss-120b`,
   `Llama-4-Maverick-17B-128E-Instruct-FP8`, `claude-haiku-4-5`, `claude-sonnet-4-5`, `claude-opus-4-1`,
   `claude-opus-4-6`, `claude-opus-4-7`, `gpt-4o`, `gpt-4o-mini`, `gpt-5.2`, `gpt-5.2-chat`, `gpt-5.3-chat`,
   `gpt-5.4-nano`, `gpt-5.4-mini`, `gpt-5.4` and `gpt-5.5`.
3. Support for agentic scenarios including tools so you can now use it in the Foundry Agent service.
4. Quick deploy or Custom deploy with routing mode and model subset selections.
5. Routing mode: Optimize the routing logic for your needs. Supported options: Quality, Cost, Balanced (default).
6. Model subset: Select your models to create your model subset for routing.

For the latest information, reference the [Model router documentation ][4]

#### Supported models

──────────────┬────────────────────────────────────────┬─────────────
Router Version│Model                                   │Model version
──────────────┼────────────────────────────────────────┼─────────────
2025-11-18    │gpt-5*                                  │2025-08-07   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-5-mini                              │2025-08-07   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-5-nano                              │2025-08-07   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-5-chat                              │2025-08-07   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-5.2                                 │2025-12-11   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-5.2-chat                            │2025-12-11   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-5.3-chat                            │2026-03-03   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-5.4-nano                            │2026-03-17   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-5.4-mini                            │2026-03-17   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-5.4                                 │2026-03-05   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-5.5                                 │2026-04-24   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-4.1                                 │2025-04-14   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-4.1-mini                            │2025-04-14   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-4.1-nano                            │2025-04-14   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-4o                                  │2024-11-20   
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-4o-mini                             │2024-07-18   
──────────────┼────────────────────────────────────────┼─────────────
              │o4-mini                                 │2025-04-16   
──────────────┼────────────────────────────────────────┼─────────────
              │grok-4**                                │1            
──────────────┼────────────────────────────────────────┼─────────────
              │grok-4-1-fast-reasoning**               │1            
──────────────┼────────────────────────────────────────┼─────────────
              │DeepSeek-V3.1**                         │1            
──────────────┼────────────────────────────────────────┼─────────────
              │DeepSeek-V3.2**                         │1            
──────────────┼────────────────────────────────────────┼─────────────
              │gpt-oss-120b**                          │1            
──────────────┼────────────────────────────────────────┼─────────────
              │Llama-4-Maverick-17B-128E-Instruct-FP8**│1            
──────────────┼────────────────────────────────────────┼─────────────
              │claude-haiku-4-5**                      │20251001     
──────────────┼────────────────────────────────────────┼─────────────
              │claude-sonnet-4-5**                     │20250929     
──────────────┼────────────────────────────────────────┼─────────────
              │claude-opus-4-1**                       │20250805     
──────────────┼────────────────────────────────────────┼─────────────
              │claude-opus-4-6**                       │1            
──────────────┼────────────────────────────────────────┼─────────────
              │claude-opus-4-7**                       │1            
──────────────┴────────────────────────────────────────┴─────────────

* Requires registration. Please refer to the particular model documentation for the latest information.
** Model router support for this model is in preview.

## Quick facts

Model providerMicrosoft
TypeChat completion
LifecycleGenerally available (GA)
Input typetext, image
Output typetext
Context window1048.576k
Token limits32768 output
Pricing[View pricing ][5]

## Quick start

[Sign in to get started][6][View documentation][7][Azure support][8]
© 2026 Microsoft Corporation. All rights reserved.
[System status ][9]
[Terms ][10][Privacy ][11]

[1]: https://ai.azure.com/
[2]: /catalog
[3]: /catalog/models
[4]: https://aka.ms/model-router-docs
[5]: https://aka.ms/models-pricing
[6]: https://ai.azure.com/public-catalog-sign-in
[7]: https://learn.microsoft.com/en-us/azure/ai-foundry/
[8]: https://azure.microsoft.com/support/
[9]: https://status.ai.azure.com
[10]: https://azure.microsoft.com/en-us/support/legal/
[11]: https://www.microsoft.com/en-us/privacy/privacystatement
```
