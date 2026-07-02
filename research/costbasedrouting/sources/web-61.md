# Web source

- URL: https://learn.microsoft.com/en-us/azure/foundry/concepts/foundry-models-overview
- Title: [ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]
- Captured (UTC): 2026-06-29T15:43:28.620172819+00:00

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

# Microsoft Foundry Models overview

Feedback
Summarize this article for me

## In this article

Microsoft Foundry Models is your one-stop destination for discovering, evaluating, and deploying powerful AI
models—whether you're building a custom copilot, an agent, enhancing an existing application, or exploring new AI
capabilities.

With Foundry Models, you can:
* Explore a rich catalog of cutting-edge models from Microsoft, OpenAI, DeepSeek, Hugging Face, Meta, and more.
* Compare and evaluate models side-by-side using real-world tasks and your own data.
* Deploy with confidence, thanks to built-in tools for fine-tuning, observability, and responsible AI.
* Choose your path—bring your own model, use a hosted one, or integrate seamlessly with Azure services.

Whether you're a developer, data scientist, or enterprise architect, Foundry Models gives you the flexibility and
control to build AI solutions that scale—securely, responsibly, and fast.

Foundry offers a comprehensive catalog of AI models. There are over 1,900 models that range from foundation models,
reasoning models, small language models, multimodal models, domain-specific models, and industry models.

The model catalog is organized into two main categories:
* [Foundry Models sold by Azure][8]
* [Models from partners and community][9]

Understanding the distinction between these categories helps you choose the right models for your specific requirements
and strategic goals.

Note

For all models, customers remain responsible for:
* Complying with the law in their use of any model or system
* Reviewing model descriptions in the model catalog, model cards made available by the model provider, and other
  relevant documentation
* Selecting an appropriate model for their use case
* Implementing appropriate measures (including use of Azure AI Content Safety) to ensure customers' use of the Foundry
  Tools complies with the Acceptable Use Policy in Microsoft's Product Terms and the Microsoft Enterprise AI Services
  Code of Conduct.

## Models sold by Azure

Also referred to as *Azure Direct models* or *Direct from Azure models*, these models are hosted and sold by Microsoft
under Microsoft Product Terms. Microsoft has evaluated these models, and they're deeply integrated into Azure's AI
ecosystem. The models come from a variety of providers and offer enhanced integration, optimized performance, and direct
Microsoft support, including enterprise-grade service level agreements (SLAs).

Characteristics of Models sold by Azure:
* Support available from Microsoft.
* High level of integration with Azure services and infrastructure.
* Subject to internal review based on Microsoft's Responsible AI standards.
* Model documentation and transparency reports provide customer visibility to model risks, mitigations, and limitations.
* Enterprise-grade scalability, reliability, and security.

Some of these models also offer fungible provisioned throughput, meaning you can flexibly use your quota and
reservations across any of these models. To learn how Foundry handles the data you provide to Foundry Models sold by
Azure, see [Data, privacy, and security for Models sold by Azure in Microsoft Foundry][10].

## Models from partners and community

These models constitute the vast majority of the Foundry Models and are provided by trusted third-party organizations,
partners, research labs, and community contributors. These models offer specialized and diverse AI capabilities,
covering a wide array of scenarios, industries, and innovations. Examples of Models from partners and community are the
family of large language models developed by **Anthropic** and **Open models from the Hugging Face hub**.

Anthropic includes the Claude family of state-of-the-art large language models that support text and image input, text
output, multilingual capabilities, and vision. For help with Anthropic models, use [Microsoft Support][11]. To learn
more about privacy, see [Data, privacy, and security for Claude models in Microsoft Foundry (preview)][12]. For terms
that govern data processing, see [Anthropic's Data processing Addendum][13] and [Anthropic's Commercial Terms of
Service][14]. To learn how to work with Anthropic models, see [Deploy and use Claude models in Microsoft Foundry][15].

Hugging Face hub includes hundreds of models for real-time inference with managed compute. Hugging Face creates and
maintains models listed in this collection. For help with the Hugging Face models, use the [Hugging Face forum][16] or
[Hugging Face support][17]. Learn how to deploy Hugging Face models in [How to deploy and infer with a managed compute
deployment (classic)][18].

Important

To work with models that are deployable on managed computes, such as Hugging Face models, use a hub-based project in the
Foundry portal (classic). To learn more about the available Foundry portals, see [What is Microsoft Foundry?][19].

Characteristics of Models from partners and community:
* Developed and supported by external partners and community contributors
* Diverse range of specialized models catering to niche or broad use cases
* Typically validated by providers themselves, with integration guidelines provided by Azure
* Community-driven innovation and rapid availability of cutting-edge models
* Standard Azure AI integration, with support and maintenance managed by the respective providers

Models from partners and community are deployable using *managed compute* or *serverless deployment* options. The model
provider selects how the models are deployable. To learn about the deployment types available under the serverless
deployment option, see [Deployment types for Microsoft Foundry Models][20].

### Request a model to be included in the model catalog

Request that we add a model to the model catalog right from the model catalog page in the Foundry portal.
1. Go to the model catalog page.
2. In the search bar, search for a model that doesn't exist in the catalog, such as *mymodel*.
3. Select **Request a model** to share details about the model you want to request.

## Choosing between Models sold by Azure and Models from partners and community

When selecting which Foundry Models to use, consider the following:
* **Use case and requirements**: Models sold by Azure are ideal for scenarios requiring deep Azure integration,
  guaranteed support, and enterprise SLAs. Models from partners and community excel in specialized use cases and
  innovation-led scenarios.
* **Support expectations**: Models sold by Azure come with robust Microsoft-provided support and maintenance. Partner
  and community models are supported by their providers, with varying levels of SLA and support structures.
* **Innovation and specialization**: Models from partners and community offer rapid access to specialized innovations
  and niche capabilities, often developed by leading research labs and emerging AI providers.

## Overview of model catalog capabilities

The model catalog in Foundry portal is the hub for discovering and using a wide range of models to build generative AI
applications. The model catalog features hundreds of models across model providers like Azure OpenAI, Mistral, Meta,
Cohere, NVIDIA, and Hugging Face, including models that Microsoft trained. Models from providers other than Microsoft
are Non-Microsoft Products as defined in [Microsoft Product Terms][21] and are subject to the terms provided with the
models.

Search and discover models that meet your needs through keyword search and filters. The model catalog also offers the
model performance leaderboard and benchmark metrics for select models. Access them by selecting **View leaderboard** and
**Compare models**. Benchmark data is also available from the model card's **Benchmarks** tab.

Some of the **filters** available in the model catalog are:
* **Collection**: Filter models based on the model provider collection.
* **Industry**: Filter for the models that are trained on industry-specific dataset.
* **Capabilities**: Filter for unique model features like reasoning and tool calling.
* **Inference tasks**: Filter models based on the inference task type.

Some of the details available in the **model card** are:
* **Quick facts**: Key information about the model at a quick glance
* **Details** tab: Detailed information about the model, like description, version info, and supported data type
* **Benchmarks** tab: Performance benchmark metrics for select models
* **Deployments** tab: A list of existing deployments for the model
* **License** tab: Legal information related to model licensing

## Model deployment options: managed compute and serverless deployments

The model catalog offers two distinct options to deploy models for your use: managed compute and serverless deployments.
To learn about data processing with the deployment options, see [Data, privacy, and security for use of models through
the model catalog in Microsoft Foundry portal (classic)][22]. To learn how Foundry handles the data you provide to
Foundry Models sold by Azure, see [Data, privacy, and security for Models sold by Azure in Microsoft Foundry][23].

### Capabilities of model deployment options

The deployment options and features available for each model vary, as described in the following table:

───────┬──────────────────────────────────────────────────────┬─────────────────────────────────────────────────────────
Feature│Managed compute                                       │Serverless deployment                                    
s      │                                                      │                                                         
───────┼──────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
Deploym│Model weights are deployed to dedicated virtual       │Access models through a deployment that provisions an    
ent    │machines with managed compute. A managed compute,     │API. The API provides access to the model that Microsoft 
experie│which can have one or more deployments, makes         │hosts and manages for inference. You're billed for inputs
nce and│available a REST API for inference. You're billed for │and outputs to the APIs, typically in tokens. Pricing    
billing│the virtual machine core hours the deployments use.   │information is provided before you deploy.               
───────┼──────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
API    │Keys and Microsoft Entra authentication.              │Keys and Microsoft Entra authentication.                 
authent│                                                      │                                                         
ication│                                                      │                                                         
───────┼──────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
Content│Use Azure AI Content Safety service APIs.             │Azure AI Content Safety filters are available integrated 
safety │                                                      │with inference APIs. Azure AI Content Safety filters are 
       │                                                      │billed separately.                                       
───────┼──────────────────────────────────────────────────────┼─────────────────────────────────────────────────────────
Network│[Configure a managed network for Microsoft Foundry    │Managed networks follow the public network access (PNA)  
isolati│hubs (classic)][24].                                  │flag setting for your Foundry resource. For more         
on     │                                                      │information, see the [Network isolation for models       
       │                                                      │deployed via serverless deployments][25] section later in
       │                                                      │this article.                                            
───────┴──────────────────────────────────────────────────────┴─────────────────────────────────────────────────────────

[ [Diagram that shows the service cycle differences between managed compute deployments and serverless deployments in
Microsoft Foundry Models.] ][26]

## Managed compute

The capability to deploy models as managed compute builds on platform capabilities of Azure Machine Learning to enable
seamless integration of the wide collection of models in the model catalog across the entire life cycle of large
language model (LLM) operations.

Important

To work with models that are deployable on managed computes, use a hub-based project in the Foundry portal (classic). To
learn more about the available Foundry portals, see [What is Microsoft Foundry?][27].

[ [Diagram that shows the life cycle of large language model (LLM) operations with managed compute.] ][28]

### Availability of models for deployment as managed compute

The models are made available through [Azure Machine Learning registries][29]. These registries enable a
machine-learning-first approach to [hosting and distributing Azure Machine Learning assets][30]. These assets include
model weights, container runtimes for running the models, pipelines for evaluating and fine-tuning the models, and
datasets for benchmarks and samples.

The registries build on top of a highly scalable and enterprise-ready infrastructure that:
* Delivers low-latency access to model artifacts in all Azure regions with built-in geo-replication.
* Supports enterprise security requirements such as limiting access to models by using Azure Policy and secure
  deployment by using managed virtual networks.

### Deployment of models for inference with managed compute

Models available for deployment to managed compute can be deployed to Azure Machine Learning managed compute for
real-time inference. To deploy to managed compute, you need a virtual machine quota in your Azure subscription for the
specific products to optimally run the model. Some models let you deploy to a temporarily shared quota for model
testing.

Learn more about deploying models:
* To deploy open models to managed compute, see [How to deploy and infer with a managed compute deployment
  (classic)][31].
* To deploy protected Foundry Models to managed compute with pay-as-you-go billing, see [Deploy Microsoft Foundry Models
  to managed compute with pay-as-you-go billing (classic)][32].

### Building generative AI apps with managed compute

The *prompt flow* feature in Azure Machine Learning offers a great experience for prototyping. Use models deployed with
managed compute in prompt flow with the [Open Model LLM tool][33]. You can also use the REST API exposed by managed
compute in popular LLM tools like LangChain with the [Azure Machine Learning extension][34].

### Content safety for models deployed as managed compute

The [Azure AI Content Safety][35] service is available for use with managed compute to screen for various categories of
harmful content, such as sexual content, violence, hate, and self-harm. You can also use the service to screen for
advanced threats such as jailbreak risk detection and protected material text detection.

For reference integration with Azure AI Content Safety for Llama 2, see [this notebook][36]. Or use the **Content Safety
(Text)** tool in prompt flow to pass responses from the model to Azure AI Content Safety for screening. You're billed
separately for such use, as described in [Azure AI Content Safety pricing][37].

## Serverless deployments

Serverless deployments provide a way to consume Foundry Models as APIs without hosting them on your subscription. Models
are hosted in a Microsoft-managed infrastructure, which enables API-based access to the model provider's model.
API-based access can dramatically reduce the cost of accessing a model and simplify the setup experience.

Models that are available for serverless deployments are offered by the model provider, but they're hosted in a
Microsoft-managed Azure infrastructure and accessed via API. Model providers define the license terms and set the price
for use of their models. The Azure Machine Learning service:
* Manages the hosting infrastructure.
* Makes the inference APIs available.
* Acts as the data processor for prompts submitted and content output for serverless deployments.

[ [Diagram that shows the model publisher service cycle for serverless deployments of Microsoft Foundry Models.] ][38]

### Serverless deployment types

The serverless deployment option for Foundry Models offers two main deployment categories: standard (pay-per-token) and
provisioned (reserved capacity). Within each category, you can choose global, data zone, or regional processing based on
your compliance requirements.

The available serverless deployment types include: Global Standard, Global Provisioned, Global Batch, Data Zone
Standard, Data Zone Provisioned, Data Zone Batch, Standard, Regional Provisioned, and Developer. To learn more about
these deployment types and how to choose the right one for your use, see [Deployment types for Microsoft Foundry
Models][39].

### Billing for serverless deployments

The discovery, subscription, and consumption experience for models deployed as serverless deployments is in Foundry
portal and Azure Machine Learning studio. Users accept license terms for use of the models. Pricing information for
consumption is available during deployment.

Foundry Models from partners and community are billed through Azure Marketplace, in accordance with the [Microsoft
Commercial Marketplace Terms of Use][40].

Foundry Models sold by Azure are billed via Azure meters as First Party Consumption Services. As described in the
[Product Terms][41], you purchase First Party Consumption Services by using Azure meters, but they aren't subject to
Azure service terms. Use of these models is subject to the provided license terms.

### Fine-tuning models

Certain models also support fine-tuning. For these models, you can use managed compute or serverless deployments
fine-tuning to tailor the models by using data that you provide. For more information, see [Fine-tune models with
Microsoft Foundry (classic)][42].

### RAG with models deployed as serverless deployments

In the Foundry portal, use vector indexes and retrieval-augmented generation (RAG) with models deployed via serverless
deployments to generate embeddings and inferencing based on custom data. These embeddings and inferencing can then
generate answers specific to your use case. For more information, see [Build and consume vector indexes in Microsoft
Foundry portal (classic)][43].

### Regional availability of offers and models

Pay-per-token billing is available only to users whose Azure subscription belongs to a billing account in a country or
region where the model provider has made the offer available. If the offer is available in the relevant region, the user
must have a project resource in the Azure region where the model is available for deployment or fine-tuning, as
applicable. See [Region availability for models in serverless deployments (classic)][44] for detailed information.

### Content safety for models deployed via serverless deployments

For language models deployed via serverless API, Azure AI implements a default configuration of [Azure AI Content
Safety][45] text moderation filters that detect harmful content such as hate, self-harm, sexual, and violent content. To
learn more about content filtering, see [Guardrails & controls for Models sold by Azure][46].

Tip

Content filtering is not available for certain model types that are deployed via serverless API. These model types
include embedding models and time series models.

Content filtering occurs synchronously as the service processes prompts to generate content. You might be billed
separately according to [Azure AI Content Safety pricing][47] for such use. You can disable content filtering for
individual serverless endpoints either:
* At the time when you first deploy a language model
* Later, by selecting the content filtering toggle on the deployment details page

Suppose you decide to use an API other than the [Model Inference API][48] to work with a model that's deployed via a
serverless API. In such a situation, content filtering isn't enabled unless you implement it separately by using Azure
AI Content Safety.

To get started with Azure AI Content Safety, see [Quickstart: Analyze text content][49]. If you don't use content
filtering when working with models that are deployed via serverless API, you run a higher risk of exposing users to
harmful content.

### Network isolation for models deployed via serverless deployments

Endpoints for models deployed as serverless deployments follow the public network access flag setting of the Foundry hub
that has the project in which the deployment exists. To help secure your serverless deployment, disable the public
network access flag on your Foundry hub. You can help secure inbound communication from a client to your endpoint by
using a private endpoint for the hub.

To set the public network access flag for the Foundry hub:
1. Go to the [Azure portal][50].
2. Search for the resource group to which the hub belongs, and select your Foundry hub from the resources listed for
   this resource group.
3. On the hub overview page, on the left pane, go to **Settings** > **Networking**.
4. On the **Public access** tab, configure settings for the public network access flag.
5. Save your changes. Changes might take up to five minutes to propagate.

#### Limitations
* If you have a Foundry hub with a private endpoint created before July 11, 2024, serverless deployments added to
  projects in this hub won't follow the networking configuration of the hub. Instead, create a new private endpoint for
  the hub and a new serverless deployment in the project so that the new deployments can follow the hub's networking
  configuration.
* If you have a Foundry hub with serverless deployments created before July 11, 2024, and you enable a private endpoint
  on this hub, the existing serverless deployments won't follow the hub's networking configuration. For serverless
  deployments in the hub to follow the hub's networking configuration, create the deployments again.
* Currently, [Azure OpenAI On Your Data][51] support isn't available for serverless deployments in private hubs, because
  private hubs have the public network access flag disabled.
* Any network configuration change (for example, enabling or disabling the public network access flag) might take up to
  five minutes to propagate.

## Model lifecycle: deprecation and retirement

AI models evolve fast, and when a new version or a new model with updated capabilities in the same model family becomes
available, older models might be retired in the Foundry model catalog. To allow for a smooth transition to a newer model
version, some models let users enable automatic updates. To learn about the model lifecycle of different models,
upcoming model retirement dates, and suggested replacement models and versions, see:
* [Azure OpenAI in Microsoft Foundry model deprecations and retirements][52]
* [Model deprecation and retirement for Microsoft Foundry Models][53]

## Related content
* [Instant access to models in Microsoft Foundry (preview)][54]
* [Data, privacy, and security for Models sold by Azure in Microsoft Foundry][55]
* [Data, privacy, and security for use of models through the model catalog in Microsoft Foundry portal (classic)][56]

## Feedback

Was this page helpful?

Yes No No

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?

## Additional resources
* Last updated on 2026-04-20

### In this article

Was this page helpful?

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?
[en-us][57]
[ Your Privacy Choices][58]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][59]
* [Previous Versions][60]
* [Blog][61]
* [Contribute][62]
* [Privacy][63]
* [Consumer Health Privacy][64]
* [Terms of Use][65]
* [Trademarks][66]
* © Microsoft 2026

[1]: #main
[2]: #
[3]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[4]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[5]: #
[6]: https://github.com/MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/concepts/foundry-models-overview.md
[7]: #
[8]: #models-sold-by-azure
[9]: #models-from-partners-and-community
[10]: ../responsible-ai/openai/data-privacy
[11]: https://aka.ms/anthropic-maas-support
[12]: ../responsible-ai/claude-models/data-privacy
[13]: https://www.anthropic.com/legal/data-processing-addendum
[14]: https://aka.ms/anthropic_tandc
[15]: ../foundry-models/how-to/use-foundry-models-claude
[16]: https://discuss.huggingface.co
[17]: https://huggingface.co/support
[18]: ../../foundry-classic/how-to/deploy-models-managed
[19]: ../what-is-foundry
[20]: ../foundry-models/concepts/deployment-types
[21]: https://www.microsoft.com/licensing/terms/welcome/welcomepage
[22]: ../../foundry-classic/how-to/concept-data-privacy
[23]: ../responsible-ai/openai/data-privacy
[24]: ../../foundry-classic/how-to/configure-managed-network
[25]: #network-isolation-for-models-deployed-via-serverless-deployments
[26]: ../../foundry-classic/media/explore/platform-service-cycle.png#lightbox
[27]: ../what-is-foundry
[28]: ../../foundry-classic/media/explore/llmops-life-cycle.png#lightbox
[29]: /en-us/azure/machine-learning/concept-machine-learning-registries-mlops
[30]: /en-us/azure/machine-learning/how-to-share-models-pipelines-across-workspaces-with-registries
[31]: ../../foundry-classic/how-to/deploy-models-managed
[32]: ../../foundry-classic/how-to/deploy-models-managed-pay-go
[33]: /en-us/azure/machine-learning/prompt-flow/tools-reference/open-model-llm-tool
[34]: https://python.langchain.com/docs/integrations/chat/azureml_chat_endpoint/
[35]: ../../ai-services/content-safety/overview
[36]: https://github.com/Azure/azureml-examples/blob/main/sdk/python/foundation-models/system/inference/text-generation/
llama-safe-online-deployment.ipynb
[37]: https://azure.microsoft.com/pricing/details/cognitive-services/content-safety/
[38]: ../../foundry-classic/media/explore/model-publisher-cycle.png#lightbox
[39]: ../foundry-models/concepts/deployment-types
[40]: /en-us/legal/marketplace/marketplace-terms
[41]: https://www.microsoft.com/licensing/terms/welcome/welcomepage
[42]: ../../foundry-classic/concepts/fine-tuning-overview
[43]: ../../foundry-classic/how-to/index-add
[44]: ../../foundry-classic/how-to/deploy-models-serverless-availability
[45]: ../../ai-services/content-safety/overview
[46]: ../../foundry-classic/concepts/model-catalog-content-safety
[47]: https://azure.microsoft.com/pricing/details/cognitive-services/content-safety/
[48]: /en-us/azure/ai-studio/reference/reference-model-inference-api
[49]: /en-us/azure/ai-services/content-safety/quickstart-text
[50]: https://ms.portal.azure.com/
[51]: /en-us/azure/ai-foundry/openai/concepts/use-your-data
[52]: ../openai/concepts/model-retirements
[53]: model-lifecycle-retirement
[54]: instant-models
[55]: ../responsible-ai/openai/data-privacy
[56]: ../../foundry-classic/how-to/concept-data-privacy
[57]: #
[58]: https://aka.ms/yourcaliforniaprivacychoices
[59]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[60]: https://learn.microsoft.com/en-us/previous-versions/
[61]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[62]: https://learn.microsoft.com/en-us/contribute
[63]: https://go.microsoft.com/fwlink/?LinkId=521839
[64]: https://go.microsoft.com/fwlink/?linkid=2259814
[65]: https://learn.microsoft.com/en-us/legal/termsofuse
[66]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
