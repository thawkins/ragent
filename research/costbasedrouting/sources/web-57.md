# Web source

- URL: https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/models-from-partners
- Title: [ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]
- Captured (UTC): 2026-06-29T15:43:19.668002909+00:00

```text
[ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]

This browser is no longer supported.

Upgrade to Microsoft Edge to take advantage of the latest features, security updates, and technical support.

[ Download Microsoft Edge ][3] [ More info about Internet Explorer and Microsoft Edge ][4]
Table of contents Exit editor mode
Ask Learn Ask Learn
Reading mode Table of contents [ Read in English ][5] Add Add to plan [ Edit ][6] Copy Markdown Print

Note

Access to this page requires authorization. You can try [signing in][7] or changing directories.

Access to this page requires authorization. You can try changing directories.

# Foundry Models from partners and community

Feedback
Summarize this article for me

## In this article

Microsoft Foundry Models in the model catalog comprise two main categories, namely *Foundry Models sold by Azure* and
*Foundry Models from partners and community*. This article lists a selection of Foundry Models from partners and
community, along with their capabilities, deployment types, and regions of availability, excluding deprecated and
retired models. Most Foundry Model providers are trusted third-party organizations, partners, research labs, and
community contributors.

Important

Models from partners and community that are not sold by Azure are Non-Microsoft Products under the Product Terms.

For a list of Foundry Models sold by Azure, see [Foundry Models sold by Azure][8], and for a list of Foundry Models that
are supported by the Foundry Agent Service, see [Models supported by Agent Service][9].

Foundry Models support several [deployment types][10] to a Foundry resource. Some models in the model catalog require a
hub-based project hosted by a Foundry hub for deployment. Selecting those models in the catalog opens them up in the
[Foundry (classic) portal experience][11].

## Prerequisites
* An Azure subscription. If you don't have one, create a [free account][12].
  
  Important
  
  The following Azure subscriptions can't be used to purchase software as a service (SaaS) offers in Marketplace:
  Student, Visual Studio Enterprise, or Free credit. For more information on purchasing SaaS offers, see [The SaaS
  purchase experience][13].
* A [Microsoft Foundry project][14].

## Permissions required to subscribe to Models from partners and community

[Foundry Models from partners and community][15] available for deployment (for example, Cohere models) require Azure
Marketplace. Model providers define the license terms and set the price for use of their models using Azure Marketplace.

When deploying third-party models, ensure you have the following permissions in your account:
* On the Azure subscription:
  * `Microsoft.MarketplaceOrdering/agreements/offers/plans/read`
  * `Microsoft.MarketplaceOrdering/agreements/offers/plans/sign/action`
  * `Microsoft.MarketplaceOrdering/offerTypes/publishers/offers/plans/agreements/read`
  * `Microsoft.Marketplace/offerTypes/publishers/offers/plans/agreements/read`
  * `Microsoft.SaaS/register/action`
* On the resource group—to create and use the SaaS resource:
  * `Microsoft.SaaS/resources/read`
  * `Microsoft.SaaS/resources/write`

The **Owner** and **Contributor** built-in roles on the Azure subscription include these permissions. If you don't have
the required permissions, ask your subscription administrator to assign you the **Contributor** role, or [create a
custom role][16] that includes the listed actions.

To verify your permissions, go to the [Azure portal][17], open your subscription, select **Access control (IAM)** >
**Check access**, and review your assigned roles.

Tip

`Microsoft.SaaS/register/action` is a one-time registration of the SaaS resource provider on the subscription. After
registration, it doesn't need to be repeated for each deployment.

## Country/region availability

You can access Models from partners and community with pay-as-you-go billing only if your Azure subscription belongs to
a billing account in a country/region where the model offer is available. Availability varies per model provider and
model SKU. For more information, see [Region availability for models][18].

## Anthropic

Anthropic's flagship product is Claude, a frontier AI model trusted by leading enterprises and millions of users
worldwide for complex tasks including coding, agents, financial analysis, research, and office tasks. Claude delivers
exceptional performance while maintaining high safety standards.

For an overview of Claude models, see [Claude models in Microsoft Foundry (preview)][19], and to use them in Foundry,
see [Deploy and use Claude models in Microsoft Foundry][20].

Note

**Claude Mythos 5** and **Claude Mythos Preview** are only available as *gated research preview*. Access to the models
is granted solely at Anthropic's discretion and prioritized for defensive cybersecurity use cases. See the [Claude
Mythos Preview system card][21] and [Claude Mythos 5 system card][22] for responsible use guidance.

#### Subscription type and region support

To use Claude models in Microsoft Foundry, you must have a paid Azure subscription with a billing account in a country
or region where Anthropic offers the models for purchase. For a list of common subscription-related errors, see [Common
error messages and solutions][23]. The following subscription types are currently not supported:
* Enterprise Accounts located in South Korea
* Cloud Solution Provider subscriptions
* Azure subscriptions that don't have an active pay-as-you-go billing method (for example, student, free trial, or
  startup credit–based accounts)
* Sponsored subscriptions that only use Azure credits. ***Note**: If you have an account with a credit card on file, the
  credit card will be charged instead of Azure Credits.*

For a list of supported regions, see [supported geographic locations][24]. Note that, Anthropic's "Supported Regions
Policy" may apply for the availability in your region, check [supported regions][25] for details.

───────┬───┬────────────────────────────────────────────────────────────────────────────────────────────────────────────
Model  │Typ│Capabilities                                                                                                
       │e  │                                                                                                            
───────┼───┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
`claude│Mes│- **Input:** text, image, and code                                                                          
-mythos│sag│- **Output:** text, image, and code (128,000 max tokens)                                                    
-5`    │es │- **Context window:** 1,000,000                                                                             
**Gated│   │- **Languages:** `en`, `fr`, `ar`, `zh`, `ja`, `ko`, `es`, `hi`                                             
researc│   │- **Tool calling:** Yes (file search, code execution, and more)                                             
h      │   │- **Response formats:** Text in various formats (e.g., prose, lists, Markdown tables, JSON, HTML, code in   
preview│   │various programming languages)                                                                              
**     │   │- **Key parameters:**                                                                                       
       │   │`top_p` must be at least 0.99. Requests with `top_p` below this threshold are rejected with a 400 error.    
       │   │When `top_p` is omitted, the default (0.99) is used.                                                        
       │   │`top_k`, `temperature`, `thinking={"type":"enabled"}`, `thinking={"type":"disabled"}`, and `output_format`  
       │   │are **not supported**.                                                                                      
       │   │Minimum cacheable prompt: 512 tokens.                                                                       
───────┼───┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
`claude│Mes│- **Input:** text, image, and code                                                                          
-fable-│sag│- **Output:** text, image, and code (128,000 max tokens)                                                    
5`     │es │- **Context window:** 1,000,000                                                                             
**Previ│   │- **Languages:** `en`, `fr`, `ar`, `zh`, `ja`, `ko`, `es`, `hi`                                             
ew**   │   │- **Tool calling:** Yes (file search, code execution, and more)                                             
       │   │- **Response formats:** Text in various formats (e.g., prose, lists, Markdown tables, JSON, HTML, code in   
       │   │various programming languages)                                                                              
       │   │- **Key parameters:**                                                                                       
       │   │`top_p` must be at least 0.99. Requests with `top_p` below this threshold are rejected with a 400 error.    
       │   │When `top_p` is omitted, the default (0.99) is used.                                                        
       │   │`top_k`, `temperature`, `thinking={"type":"enabled"}`, `thinking={"type":"disabled"}`, and `output_format`  
       │   │are **not supported**.                                                                                      
───────┼───┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
`claude│Mes│- **Input:** text, image, and code                                                                          
-mythos│sag│- **Output:** text, image, and code (128,000 max tokens)                                                    
-previe│es │- **Context window:** 1,000,000                                                                             
w`     │   │- **Languages:** `en`, `fr`, `ar`, `zh`, `ja`, `ko`, `es`, `hi`                                             
**Gated│   │- **Tool calling:** Yes (file search and code execution)                                                    
researc│   │- **Response formats:** Text in various formats (e.g., prose, lists, Markdown tables, JSON, HTML, code in   
h      │   │various programming languages)                                                                              
preview│   │- **Key parameters:**                                                                                       
**     │   │`top_p` must be at least 0.99. Requests with `top_p` below this threshold are rejected with a 400 error.    
       │   │When `top_p` is omitted, the default (0.99) is used.                                                        
       │   │`top_k`and `temperature` are **not supported**.                                                             
       │   │Minimum cacheable prompt: 2048 tokens.                                                                      
───────┼───┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
`claude│Mes│- **Input:** text, image, and code                                                                          
-opus-4│sag│- **Output:** text, image, and code (128,000 max tokens)                                                    
-8`    │es │- **Context window:** 1,000,000                                                                             
**Previ│   │- **Languages:** `en`, `fr`, `ar`, `zh`, `ja`, `ko`, `es`, `hi`                                             
ew**   │   │- **Tool calling:** Yes (file search and code execution)                                                    
       │   │- **Response formats:** Text in various formats (e.g., prose, lists, Markdown tables, JSON, HTML, code in   
       │   │various programming languages)                                                                              
       │   │- **Key parameters:**                                                                                       
       │   │`top_k`, `temperature`, and `thinking={"type":"enabled"}` are **not supported**.                            
       │   │`top_p` must be 0.99. When omitted, the default (0.99) is used.                                             
───────┼───┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
`claude│Mes│- **Input:** text, image, and code                                                                          
-opus-4│sag│- **Output:** text, image, and code (128,000 max tokens)                                                    
-7`    │es │- **Context window:** 1,000,000                                                                             
**Previ│   │- **Languages:** `en`, `fr`, `ar`, `zh`, `ja`, `ko`, `es`, `hi`                                             
ew**   │   │- **Tool calling:** Yes (file search and code execution)                                                    
       │   │- **Response formats:** Text in various formats (e.g., prose, lists, Markdown tables, JSON, HTML, code in   
       │   │various programming languages)                                                                              
       │   │- **Key parameters:**                                                                                       
       │   │`top_k`, `temperature`, and `thinking={"type":"enabled"}` are **not supported**.                            
       │   │`top_p` must be 0.99. When omitted, the default (0.99) is used.                                             
───────┼───┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
`claude│Mes│- **Input:** text, image, and code                                                                          
-opus-4│sag│- **Output:** text, image, and code (128,000 max tokens)                                                    
-6`    │es │- **Context window:** 1,000,000                                                                             
**Previ│   │- **Languages:** `en`, `fr`, `ar`, `zh`, `ja`, `ko`, `es`, `hi`                                             
ew**   │   │- **Tool calling:** Yes (file search and code execution)                                                    
       │   │- **Response formats:** Text in various formats (e.g., prose, lists, Markdown tables, JSON, HTML, code in   
       │   │various programming languages)                                                                              
───────┼───┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
`claude│Mes│- **Input:** text, image, and code                                                                          
-opus-4│sag│- **Output:** text (64,000 max tokens)                                                                      
-5`    │es │- **Context window:** 200,000                                                                               
**Previ│   │- **Languages:** `en`, `fr`, `ar`, `zh`, `ja`, `ko`, `es`, `hi`                                             
ew**   │   │- **Tool calling:** Yes (file search and code execution)                                                    
       │   │- **Response formats:** Text in various formats (e.g., prose, lists, Markdown tables, JSON, HTML, code in   
       │   │various programming languages)                                                                              
───────┼───┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
`claude│Mes│- **Input:** text, image, and code                                                                          
-opus-4│sag│- **Output:** text (32,000 max tokens)                                                                      
-1`    │es │- **Context window:** 200,000                                                                               
**Previ│   │- **Languages:** `en`, `fr`, `ar`, `zh`, `ja`, `ko`, `es`, `hi`                                             
ew**   │   │- **Tool calling:** Yes (file search and code execution)                                                    
       │   │- **Response formats:** Text in various formats (e.g., prose, lists, Markdown tables, JSON, HTML, code in   
       │   │various programming languages)                                                                              
───────┼───┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
`claude│Mes│- **Input:** text, image, and code                                                                          
-sonnet│sag│- **Output:** text, image, and code (128,000 max tokens)                                                    
-4-6`  │es │- **Context window:** 1,000,000                                                                             
**Previ│   │- **Languages:** `en`, `fr`, `ar`, `zh`, `ja`, `ko`, `es`, `hi`                                             
ew**   │   │- **Tool calling:** Yes (file search and code execution)                                                    
       │   │- **Response formats:** Text in various formats (e.g., prose, lists, Markdown tables, JSON, HTML, code in   
       │   │various programming languages)                                                                              
───────┼───┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
`claude│Mes│- **Input:** text, image, and code                                                                          
-sonnet│sag│- **Output:** text (64,000 max tokens)                                                                      
-4-5`  │es │- **Context window:** 200,000                                                                               
**Previ│   │- **Languages:** `en`, `fr`, `ar`, `zh`, `ja`, `ko`, `es`, `hi`                                             
ew**   │   │- **Tool calling:** Yes (file search and code execution)                                                    
       │   │- **Response formats:** Text in various formats (e.g., prose, lists, Markdown tables, JSON, HTML, code in   
       │   │various programming languages)                                                                              
───────┼───┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
`claude│Mes│- **Input:** text and image                                                                                 
-haiku-│sag│- **Output:** text (64,000 max tokens)                                                                      
4-5`   │es │- **Context window:** 200,000                                                                               
**Previ│   │- **Languages:** `en`, `fr`, `ar`, `zh`, `ja`, `ko`, `es`, `hi`                                             
ew**   │   │- **Tool calling:** Yes (file search and code execution)                                                    
       │   │- **Response formats:** Text in various formats (e.g., prose, lists, Markdown tables, JSON, HTML, code in   
       │   │various programming languages)                                                                              
───────┴───┴────────────────────────────────────────────────────────────────────────────────────────────────────────────

## Cohere

The Cohere family of models includes various models optimized for different use cases, including chat completions and
embeddings. Cohere models are optimized for various use cases that include reasoning, summarization, and question
answering.

To deploy Cohere models in Foundry, see [Deploy Microsoft Foundry Models in the Foundry portal][26].

───────────────┬───────┬────────────────────────────────────────────────────────────────────────────────────────────────
Model          │Type   │Capabilities                                                                                    
───────────────┼───────┼────────────────────────────────────────────────────────────────────────────────────────────────
`Cohere-command│chat-co│- **Input:** text (131,072 tokens)                                                              
-r-plus-08-2024│mpletio│- **Output:** text (4,096 tokens)                                                               
`              │n      │- **Languages:** `en`, `fr`, `es`, `it`, `de`, `pt-br`, `ja`, `ko`, `zh-cn`, and `ar`           
               │       │- **Tool calling:** Yes                                                                         
               │       │- **Response formats:** Text, JSON                                                              
───────────────┼───────┼────────────────────────────────────────────────────────────────────────────────────────────────
`Cohere-command│chat-co│- **Input:** text (131,072 tokens)                                                              
-r-08-2024`    │mpletio│- **Output:** text (4,096 tokens)                                                               
               │n      │- **Languages:** `en`, `fr`, `es`, `it`, `de`, `pt-br`, `ja`, `ko`, `zh-cn`, and `ar`           
               │       │- **Tool calling:** Yes                                                                         
               │       │- **Response formats:** Text, JSON                                                              
───────────────┼───────┼────────────────────────────────────────────────────────────────────────────────────────────────
`Cohere-embed-v│embeddi│- **Input:** text and images (512 tokens)                                                       
3-english`     │ngs    │- **Output:** Vector (1024 dim.)                                                                
               │       │- **Languages:** `en`                                                                           
───────────────┼───────┼────────────────────────────────────────────────────────────────────────────────────────────────
`Cohere-embed-v│embeddi│- **Input:** text (512 tokens)                                                                  
3-multilingual`│ngs    │- **Output:** Vector (1024 dim.)                                                                
               │       │- **Languages:** `en`, `fr`, `es`, `it`, `de`, `pt-br`, `ja`, `ko`, `zh-cn`, and `ar`           
───────────────┴───────┴────────────────────────────────────────────────────────────────────────────────────────────────

## Meta

Meta Llama models and tools are a collection of pretrained and fine-tuned generative AI text and image reasoning models.
Meta models range in scale to include:
* Small language models (SLMs) like 1B and 3B Base and Instruct models for on-device and edge inferencing
* Mid-size large language models (LLMs) like 7B, 8B, and 70B Base and Instruct models
* High-performance models like Meta Llama 3.1-405B Instruct for synthetic data generation and distillation use cases.

To deploy Meta Llama models in Foundry, see [Deploy Microsoft Foundry Models in the Foundry portal][27].

────────────────┬───────┬───────────────────────────────────────────────────────────────────────────────────────────────
Model           │Type   │Capabilities                                                                                   
────────────────┼───────┼───────────────────────────────────────────────────────────────────────────────────────────────
`Llama-3.2-11B-V│chat-co│- **Input:** text and image (128,000 tokens)                                                   
ision-Instruct` │mpletio│- **Output:** text (8,192 tokens)                                                              
                │n      │- **Languages:** `en`                                                                          
                │       │- **Tool calling:** No                                                                         
                │       │- **Response formats:** Text                                                                   
────────────────┼───────┼───────────────────────────────────────────────────────────────────────────────────────────────
`Llama-3.2-90B-V│chat-co│- **Input:** text and image (128,000 tokens)                                                   
ision-Instruct` │mpletio│- **Output:** text (8,192 tokens)                                                              
                │n      │- **Languages:** `en`                                                                          
                │       │- **Tool calling:** No                                                                         
                │       │- **Response formats:** Text                                                                   
────────────────┼───────┼───────────────────────────────────────────────────────────────────────────────────────────────
`Meta-Llama-3.1-│chat-co│- **Input:** text (131,072 tokens)                                                             
405B-Instruct`  │mpletio│- **Output:** text (8,192 tokens)                                                              
                │n      │- **Languages:** `en`, `de`, `fr`, `it`, `pt`, `hi`, `es`, and `th`                            
                │       │- **Tool calling:** No                                                                         
                │       │- **Response formats:** Text                                                                   
────────────────┼───────┼───────────────────────────────────────────────────────────────────────────────────────────────
`Meta-Llama-3.1-│chat-co│- **Input:** text (131,072 tokens)                                                             
8B-Instruct`    │mpletio│- **Output:** text (8,192 tokens)                                                              
                │n      │- **Languages:** `en`, `de`, `fr`, `it`, `pt`, `hi`, `es`, and `th`                            
                │       │- **Tool calling:** No                                                                         
                │       │- **Response formats:** Text                                                                   
────────────────┼───────┼───────────────────────────────────────────────────────────────────────────────────────────────
`Llama-4-Scout-1│chat-co│- **Input:** text and image (128,000 tokens)                                                   
7B-16E-Instruct`│mpletio│- **Output:** text (8,192 tokens)                                                              
                │n      │- **Languages:** `en`                                                                          
                │       │- **Tool calling:** No                                                                         
                │       │- **Response formats:** Text                                                                   
────────────────┴───────┴───────────────────────────────────────────────────────────────────────────────────────────────

## Microsoft

Microsoft models include various model groups such as MAI models, Phi models, healthcare AI models, and more.

To deploy Microsoft models in Foundry, see [Deploy Microsoft Foundry Models in the Foundry portal][28].

───────┬──────────┬─────────────────────────────────────────────────────────────────────────────────────────────────────
Model  │Type      │Capabilities                                                                                         
───────┼──────────┼─────────────────────────────────────────────────────────────────────────────────────────────────────
`Phi-4-│chat-compl│- **Input:** text (131,072 tokens)                                                                   
mini-in│etion     │- **Output:** text (4,096 tokens)                                                                    
struct`│          │- **Languages:** `ar`, `zh`, `cs`, `da`, `nl`, `en`, `fi`, `fr`, `de`, `he`, `hu`, `it`, `ja`, `ko`, 
       │          │`no`, `pl`, `pt`, `ru`, `es`, `sv`, `th`, `tr`, and `uk`                                             
       │          │- **Tool calling:** No                                                                               
       │          │- **Response formats:** Text                                                                         
───────┼──────────┼─────────────────────────────────────────────────────────────────────────────────────────────────────
`Phi-4-│chat-compl│- **Input:** text, images, and audio (131,072 tokens)                                                
multimo│etion     │- **Output:** text (4,096 tokens)                                                                    
dal-ins│          │- **Languages:** `ar`, `zh`, `cs`, `da`, `nl`, `en`, `fi`, `fr`, `de`, `he`, `hu`, `it`, `ja`, `ko`, 
truct` │          │`no`, `pl`, `pt`, `ru`, `es`, `sv`, `th`, `tr`, and `uk`                                             
       │          │- **Tool calling:** No                                                                               
       │          │- **Response formats:** Text                                                                         
───────┼──────────┼─────────────────────────────────────────────────────────────────────────────────────────────────────
`Phi-4`│chat-compl│- **Input:** text (16,384 tokens)                                                                    
       │etion     │- **Output:** text (16,384 tokens)                                                                   
       │          │- **Languages:** `en`, `ar`, `bn`, `cs`, `da`, `de`, `el`, `es`, `fa`, `fi`, `fr`, `gu`, `ha`, `he`, 
       │          │`hi`, `hu`, `id`, `it`, `ja`, `jv`, `kn`, `ko`, `ml`, `mr`, `nl`, `no`, `or`, `pa`, `pl`, `ps`, `pt`,
       │          │`ro`, `ru`, `sv`, `sw`, `ta`, `te`, `th`, `tl`, `tr`, `uk`, `ur`, `vi`, `yo`, and `zh`               
       │          │- **Tool calling:** No                                                                               
       │          │- **Response formats:** Text                                                                         
───────┼──────────┼─────────────────────────────────────────────────────────────────────────────────────────────────────
`Phi-4-│chat-compl│- **Input:** text (32,768 tokens)                                                                    
reasoni│etion with│- **Output:** text (32,768 tokens)                                                                   
ng`    │reasoning │- **Languages:** `en`                                                                                
       │content   │- **Tool calling:** No                                                                               
       │          │- **Response formats:** Text                                                                         
───────┼──────────┼─────────────────────────────────────────────────────────────────────────────────────────────────────
`Phi-4-│chat-compl│- **Input:** text (128,000 tokens)                                                                   
mini-re│etion with│- **Output:** text (128,000 tokens)                                                                  
asoning│reasoning │- **Languages:** `en`                                                                                
`      │content   │- **Tool calling:** No                                                                               
       │          │- **Response formats:** Text                                                                         
───────┴──────────┴─────────────────────────────────────────────────────────────────────────────────────────────────────

## Mistral AI

Mistral AI offers models for code generation, general-purpose chat, and multimodal tasks, including Codestral,
Ministral, Mistral Small, and Mistral Medium.

To deploy Mistral AI models in Foundry, see [Deploy Microsoft Foundry Models in the Foundry portal][29].

─────────────────────┬────────┬─────────────────────────────────────────────────────────────────────────────────────────
Model                │Type    │Capabilities                                                                             
─────────────────────┼────────┼─────────────────────────────────────────────────────────────────────────────────────────
`Codestral-2501`     │chat-com│- **Input:** text (262,144 tokens)                                                       
                     │pletion │- **Output:** text (4,096 tokens)                                                        
                     │        │- **Languages:** en                                                                      
                     │        │- **Tool calling:** No                                                                   
                     │        │- **Response formats:** Text                                                             
─────────────────────┼────────┼─────────────────────────────────────────────────────────────────────────────────────────
`Ministral-3B`       │chat-com│- **Input:** text (131,072 tokens)                                                       
                     │pletion │- **Output:** text (4,096 tokens)                                                        
                     │        │- **Languages:** fr, de, es, it, and en                                                  
                     │        │- **Tool calling:** Yes                                                                  
                     │        │- **Response formats:** Text, JSON                                                       
─────────────────────┼────────┼─────────────────────────────────────────────────────────────────────────────────────────
`Mistral-small-2503` │chat-com│- **Input:** text (32,768 tokens)                                                        
                     │pletion │- **Output:** text (4,096 tokens)                                                        
                     │        │- **Languages:** fr, de, es, it, and en                                                  
                     │        │- **Tool calling:** Yes                                                                  
                     │        │- **Response formats:** Text, JSON                                                       
─────────────────────┼────────┼─────────────────────────────────────────────────────────────────────────────────────────
`Mistral-medium-2505`│chat-com│- **Input:** text (128,000 tokens), image                                                
                     │pletion │- **Output:** text (128,000 tokens)                                                      
                     │        │- **Tool calling:** No                                                                   
                     │        │- **Response formats:** Text, JSON                                                       
─────────────────────┼────────┼─────────────────────────────────────────────────────────────────────────────────────────
`mistralai-Mistral-7B│chat-com│- **Input:** text                                                                        
-Instruct-v01`¹      │pletion │- **Output:** text                                                                       
                     │        │- **Languages:** en                                                                      
                     │        │- **Response formats:** Text                                                             
─────────────────────┼────────┼─────────────────────────────────────────────────────────────────────────────────────────
`mistralai-Mistral-7B│chat-com│- **Input:** text                                                                        
-Instruct-v0-2`¹     │pletion │- **Output:** text                                                                       
                     │        │- **Languages:** en                                                                      
                     │        │- **Response formats:** Text                                                             
─────────────────────┼────────┼─────────────────────────────────────────────────────────────────────────────────────────
`mistralai-Mixtral-8x│chat-com│- **Input:** text                                                                        
7B-Instruct-v01`¹    │pletion │- **Output:** text                                                                       
                     │        │- **Languages:** en                                                                      
                     │        │- **Response formats:** Text                                                             
─────────────────────┼────────┼─────────────────────────────────────────────────────────────────────────────────────────
`mistralai-Mixtral-8x│chat-com│- **Input:** text (64,000 tokens)                                                        
22B-Instruct-v0-1`¹  │pletion │- **Output:** text (4,096 tokens)                                                        
                     │        │- **Languages:** fr, it, de, es, en                                                      
                     │        │- **Response formats:** Text                                                             
─────────────────────┴────────┴─────────────────────────────────────────────────────────────────────────────────────────

¹ These models require a hub-based project for deployment. Selecting them in the model catalog opens them up in the
[Foundry (classic) portal experience][30].

## Nixtla

Nixtla's TimeGEN-1 is a generative pretrained forecasting and anomaly detection model for time series data. TimeGEN-1
produces accurate forecasts for new time series without training, using only historical values and exogenous covariates
as inputs.

To deploy TimeGEN-1 in Foundry, see [Deploy Microsoft Foundry Models in the Foundry portal][31].

To perform inferencing, TimeGEN-1 requires you to use Nixtla's custom inference API.

─────┬─────┬───────────────────────────────────────

[Content truncated]
```
