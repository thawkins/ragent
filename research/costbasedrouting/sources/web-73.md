# Web source

- URL: https://github.com/MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/concepts/limits-quotas-regions.md
- Title: [Skip to content][1]
- Captured (UTC): 2026-06-29T15:43:57.035148713+00:00

```text
[Skip to content][1]

## Navigation Menu

Toggle navigation
[ Sign in ][2]
Appearance settings
* Platform
  * AI CODE CREATION
    * [
      GitHub CopilotWrite better code with AI
      ][3]
    * [
      GitHub Copilot appDirect agents from issue to merge
      ][4]
    * [
      MCP Registry^{New}Integrate external tools
      ][5]
  * DEVELOPER WORKFLOWS
    * [
      ActionsAutomate any workflow
      ][6]
    * [
      CodespacesInstant dev environments
      ][7]
    * [
      IssuesPlan and track work
      ][8]
    * [
      Code ReviewManage code changes
      ][9]
  * APPLICATION SECURITY
    * [
      GitHub Advanced SecurityFind and fix vulnerabilities
      ][10]
    * [
      Code securitySecure your code as you build
      ][11]
    * [
      Secret protectionStop leaks before they start
      ][12]
  * EXPLORE
    * [Why GitHub][13]
    * [Documentation][14]
    * [Blog][15]
    * [Changelog][16]
    * [Marketplace][17]
  [View all features][18]
* Solutions
  * BY COMPANY SIZE
    * [Enterprises][19]
    * [Small and medium teams][20]
    * [Startups][21]
    * [Nonprofits][22]
  * BY USE CASE
    * [App Modernization][23]
    * [DevSecOps][24]
    * [DevOps][25]
    * [CI/CD][26]
    * [View all use cases][27]
  * BY INDUSTRY
    * [Healthcare][28]
    * [Financial services][29]
    * [Manufacturing][30]
    * [Government][31]
    * [View all industries][32]
  [View all solutions][33]
* Resources
  * EXPLORE BY TOPIC
    * [AI][34]
    * [Software Development][35]
    * [DevOps][36]
    * [Security][37]
    * [View all topics][38]
  * EXPLORE BY TYPE
    * [Customer stories][39]
    * [Events & webinars][40]
    * [Ebooks & reports][41]
    * [Business insights][42]
    * [GitHub Skills][43]
  * SUPPORT & SERVICES
    * [Documentation][44]
    * [Customer support][45]
    * [Community forum][46]
    * [Trust center][47]
    * [Partners][48]
  [View all resources][49]
* Open Source
  * COMMUNITY
    * [
      GitHub SponsorsFund open source developers
      ][50]
  * PROGRAMS
    * [Security Lab][51]
    * [Maintainer Community][52]
    * [Accelerator][53]
    * [GitHub Stars][54]
    * [Archive Program][55]
  * REPOSITORIES
    * [Topics][56]
    * [Trending][57]
    * [Collections][58]
* Enterprise
  * ENTERPRISE SOLUTIONS
    * [
      Enterprise platformAI-powered developer platform
      ][59]
  * AVAILABLE ADD-ONS
    * [
      GitHub Advanced SecurityEnterprise-grade security features
      ][60]
    * [
      Copilot for BusinessEnterprise-grade AI features
      ][61]
    * [
      Premium SupportEnterprise-grade 24/7 support
      ][62]
* [Pricing][63]
Search or jump to...

# Search code, repositories, users, issues, pull requests...

Search
Clear
[Search syntax tips][64]

# Provide feedback

We read every piece of feedback, and take your input very seriously.

Include my email address so I can be contacted
Cancel Submit feedback

# Saved searches

## Use saved searches to filter your results more quickly

Name
Query

To see all available qualifiers, see our [documentation][65].

Cancel Create saved search
[ Sign in ][66]
[ Sign up ][67]
Appearance settings
Resetting focus
You signed in with another tab or window. [Reload][68] to refresh your session. You signed out in another tab or window.
[Reload][69] to refresh your session. You switched accounts on another tab or window. [Reload][70] to refresh your
session. Dismiss alert

### Uh oh!


There was an error while loading. [Please reload this page][71].

[ MicrosoftDocs ][72] / ** [azure-ai-docs][73] ** Public
* [ Notifications ][74] You must be signed in to change notification settings
* [ Fork 466 ][75]
* [ Star 162 ][76]
* [ Code ][77]
* [ Pull requests 8 ][78]
* [ Actions ][79]
* [ Security and quality 0 ][80]
* [ Insights ][81]
Additional navigation options
* [ Code ][82]
* [ Pull requests ][83]
* [ Actions ][84]
* [ Security and quality ][85]
* [ Insights ][86]

## FilesExpand file tree

main

## Breadcrumbs
1. [azure-ai-docs][87]
2. /[articles][88]
3. /[foundry][89]
4. /[agents][90]
5. /[concepts][91]

/

# limits-quotas-regions.md

Copy path
BlameMore file actions
BlameMore file actions

## Latest commit

## History

[History][92]
[][93]History
150 lines (104 loc) · 9.5 KB
 main

## Breadcrumbs
1. [azure-ai-docs][94]
2. /[articles][95]
3. /[foundry][96]
4. /[agents][97]
5. /[concepts][98]

/

# limits-quotas-regions.md

Copy path
Top

## File metadata and controls
* Preview
* Code
* Blame

150 lines (104 loc) · 9.5 KB
[Raw][99]
Copy raw file
Download raw file
Outline
Edit and raw actions

─────────┬──────────────────────────────────────────────────────────────────────────────────────────────────────────────
title    │Quotas and limits for Microsoft Foundry Agent Service                                                         
─────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────────────
descripti│Review default limits for Foundry Agent Service, including file sizes, vector stores, messages, tools, error  
on       │handling, supported regions, and compatible models.                                                           
─────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────────────
manager  │nitinme                                                                                                       
─────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────────────
author   │aahill                                                                                                        
─────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────────────
ms.author│aahi                                                                                                          
─────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────────────
ms.servic│microsoft-foundry                                                                                             
e        │                                                                                                              
─────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────────────
ms.subser│foundry-agent-service                                                                                         
vice     │                                                                                                              
─────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────────────
ms.topic │concept-article                                                                                               
─────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────────────
ms.date  │04/03/2026                                                                                                    
─────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────────────
ms.custom│azure-ai-agents, pilot-ai-workflow-jan-2026, references_regions, doc-kit-assisted                             
─────────┼──────────────────────────────────────────────────────────────────────────────────────────────────────────────
ai-usage │ai-assisted                                                                                                   
─────────┴──────────────────────────────────────────────────────────────────────────────────────────────────────────────

# Foundry Agent Service limits, quotas, and regional support

Foundry Agent Service enforces quotas and limits on agent artifacts, file uploads, messages, and tool registrations.
Understanding these limits helps you design applications that scale without hitting service boundaries. This article
lists default limits, supported regions, compatible models, and guidance for handling limit errors.

Note

Foundry Agent Service is generally available (GA). Some sub-features, such as [Hosted agents][100], are in public
preview and might have different constraints.

## Prerequisites
* An Azure subscription.
* A [Microsoft Foundry project][101].
* A deployed model compatible with Agent Service. Model and region availability can vary.

## Supported regions

Foundry agent service is only available for Foundry projects created in regions that support the [Azure OpenAI Responses
API][102]. Your Foundry project must be in one of these regions to use Agent Service. Some Azure OpenAI models may not
be available in the same regions. See [Region availability for Foundry Models sold by Azure][103] for details.

Important

Not all tools are available in every region. For example, file search isn't available in Italy North and Brazil South.
For the full tool-by-region matrix, see [Tool support by region and model][104].

In addition to Azure OpenAI models, Agent Service supports models from the Foundry model catalog. These models are
deployed and managed through Foundry and follow separate quotas. The following models are available for your agents to
use.

[!INCLUDE [agent-service-models-support-list][105]]

Tip

Model availability can change over time. To verify what you can deploy for your project and region, use the Foundry
portal model experience.

## Troubleshooting

### A model or version isn't available in your region
* Confirm you selected the right tab for your deployment type (global standard vs. provisioned).
* Try a different region that supports the [model and Responses API][106].
* If you're using gpt-5 models, [registration][107] is required. Access is granted according to Microsoft's eligibility
  criteria.

### A tool isn't available in your region
* Not all tools are supported in every region. For example, file search isn't available in Italy North and Brazil South,
  and code interpreter isn't available in all regions.
* Check the [tool support by region and model][108] table to confirm availability before you deploy.
* If a tool isn't available, choose a supported region or use a different tool.

### Provisioned throughput deployment fails
* Confirm you have enough PTUs available in the region.
* Review [Provisioned throughput][109] and [Spillover traffic management][110].

### Agent receives rate-limit (429) errors
* Implement exponential backoff with jitter in your application retry logic.
* For sustained high-throughput workloads, consider provisioned throughput deployments.
* Review [Azure OpenAI quotas and limits][111] for your deployment's tokens-per-minute and requests-per-minute caps.

## Quotas and limits

Foundry Agent Service enforces limits in two places:
* **Agent Service limits.** Limits for agent and thread artifacts, such as file uploads, vector store attachments,
  message counts, and tool registration.
* **Model limits.** Quotas and rate limits for the model deployments your agents call.

If you're using threads and messages, see [Threads, runs, and messages in Foundry Agent Service][112]. If you're using
file search, see [Vector stores for file search][113].

## Default quotas and limits for the service

The following table lists default limits enforced by the Agent Service. These limits apply to all Foundry projects
regardless of subscription type or region.

───────────────────────────────────────────────────────────┬────────────────────
Limit name                                                 │Limit value         
───────────────────────────────────────────────────────────┼────────────────────
Maximum number of files per agent/thread                   │10,000              
───────────────────────────────────────────────────────────┼────────────────────
Maximum file size for agents                               │512 MB              
───────────────────────────────────────────────────────────┼────────────────────
Maximum size for all uploaded files for agents             │300 GB              
───────────────────────────────────────────────────────────┼────────────────────
Maximum file size in tokens for attaching to a vector store│2,000,000 tokens    
───────────────────────────────────────────────────────────┼────────────────────
Maximum number of messages per thread                      │100,000             
───────────────────────────────────────────────────────────┼────────────────────
Maximum size of `text` content per message                 │1,500,000 characters
───────────────────────────────────────────────────────────┼────────────────────
Maximum number of tools registered per agent               │128                 
───────────────────────────────────────────────────────────┴────────────────────

The Agent Service limits in this table are fixed and apply uniformly across all subscription types. Agent Service
doesn't impose separate rate limits on API calls. Rate limiting is applied at the model deployment level. See [Azure
OpenAI quotas and limits][114] for model-specific rate limits.

## Limit error reference

When you exceed a limit, the Agent Service returns an error. Handle these errors gracefully in your application.

─────────────────────────┬───────────┬────────────────────────┬──────────────────────────────────
Error scenario           │HTTP status│Error code              │Recommended action                
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
File too large           │400        │`file_size_exceeded`    │Split content into smaller files  
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
Vector store token limit │400        │`token_limit_exceeded`  │Reduce file content or split files
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
Thread message cap       │400        │`message_limit_exceeded`│Create a new thread               
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
Message content too large│400        │`content_size_exceeded` │Use file search for large content 
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
Too many tools           │400        │`tool_limit_exceeded`   │Remove unused tools               
─────────────────────────┼───────────┼────────────────────────┼──────────────────────────────────
Rate limit exceeded      │429        │`rate_limit_exceeded`   │Implement exponential backoff     
─────────────────────────┴───────────┴────────────────────────┴──────────────────────────────────

For example:
* **File exceeds the maximum size.** Uploading the file fails. Split the content into smaller files or reduce file size
  before you upload.
* **Vector store token limit.** Attaching a file to a vector store fails if the file exceeds the token limit. Reduce the
  file content or split it into multiple files.
* **Thread message cap.** Adding messages can fail after a thread reaches the message limit. Create a new thread for a
  new conversation session, or archive and rotate threads as part of your application design.
* **Message content size.** Creating a message can fail if the `text` content is too large. Send smaller messages, or
  move large content into files and use file search.
* **Tool registration cap.** Creating or updating an agent can fail if you register too many tools. Register only the
  tools you need, and prefer fewer, reusable tools.
* **Rate limit exceeded.** API calls to the model deployment are throttled. Implement exponential backoff with jitter.

For file search scenarios, see [Vector stores for file search][115] for guidance on managing vector store growth.

## Best practices to stay within limits

Use the following practices to reduce limit-related failures:
* **Keep files small and focused.** Prefer multiple smaller documents over a single large document.
* **Avoid very large messages.** Put long content in uploaded files and query it by using file search.
* **Plan for long conversations.** Treat threads as session state and rotate to new threads when conversations become
  very long.
* **Register only required tools.** Remove unused tools from agent definitions.
* **Monitor usage trends.** Track agent activity by using [Foundry Agent Service metrics][116] to identify growth before
  you hit limits.

## Quotas and limits for models

Agents follow the quotas and rate limits for the model deployments they use.

For current model quotas and limits, see:
* [Azure OpenAI quotas and limits][117].
* [Microsoft Foundry Models quotas and limits][118].

To view or request more model quota, see [Manage and increase quotas for resources with Microsoft Foundry (Foundry
projects)][119].

## Request a limit increase

The limits in this article are default values for Foundry Agent Service. If your workload requires higher limits:
* **Model quotas.** You can request increases for model deployment quotas. See [Manage and increase quotas for resources
  with Microsoft Foundry][120].
* **Agent Service limits.** The file, message, and tool limits listed in this article are fixed service limits and can't
  be increased. Design your application to work within these constraints by using the best practices described earlier.

## Related content
* [Threads, runs, and messages in Foundry Agent Service][121]
* [Tool support by region and model][122]
* [Vector stores for file search][123]
* [Monitor Foundry Agent Service][124]
* [Azure OpenAI quotas and limits][125]
* [Manage and increase quotas for resources with Microsoft Foundry][126]

## Footer

© 2026 GitHub, Inc.

### Footer navigation
* [Terms][127]
* [Privacy][128]
* [Security][129]
* [Status][130]
* [Community][131]
* [Docs][132]
* [Contact][133]
* Manage cookies
* Do not share my personal information

You can’t perform that action at this time.

[1]: #start-of-content
[2]: /login?return_to=https%3A%2F%2Fgithub.com%2FMicrosoftDocs%2Fazure-ai-docs%2Fblob%2Fmain%2Farticles%2Ffoundry%2Fagen
ts%2Fconcepts%2Flimits-quotas-regions.md
[3]: https://github.com/features/copilot
[4]: https://github.com/features/ai/github-app
[5]: https://github.com/mcp
[6]: https://github.com/features/actions
[7]: https://github.com/features/codespaces
[8]: https://github.com/features/issues
[9]: https://github.com/features/code-review
[10]: https://github.com/security/advanced-security
[11]: https://github.com/security/advanced-security/code-security
[12]: https://github.com/security/advanced-security/secret-protection
[13]: https://github.com/why-github
[14]: https://docs.github.com
[15]: https://github.blog
[16]: https://github.blog/changelog
[17]: https://github.com/marketplace
[18]: https://github.com/features
[19]: https://github.com/enterprise
[20]: https://github.com/team
[21]: https://github.com/enterprise/startups
[22]: https://github.com/solutions/industry/nonprofits
[23]: https://github.com/solutions/use-case/app-modernization
[24]: https://github.com/solutions/use-case/devsecops
[25]: https://github.com/solutions/use-case/devops
[26]: https://github.com/solutions/use-case/ci-cd
[27]: https://github.com/solutions/use-case
[28]: https://github.com/solutions/industry/healthcare
[29]: https://github.com/solutions/industry/financial-services
[30]: https://github.com/solutions/industry/manufacturing
[31]: https://github.com/solutions/industry/government
[32]: https://github.com/solutions/industry
[33]: https://github.com/solutions
[34]: https://github.com/resources/articles?topic=ai
[35]: https://github.com/resources/articles?topic=software-development
[36]: https://github.com/resources/articles?topic=devops
[37]: https://github.com/resources/articles?topic=security
[38]: https://github.com/resources/articles
[39]: https://github.com/customer-stories
[40]: https://github.com/resources/events
[41]: https://github.com/resources/whitepapers
[42]: https://github.com/solutions/executive-insights
[43]: https://skills.github.com
[44]: https://docs.github.com
[45]: https://support.github.com
[46]: https://github.com/orgs/community/discussions
[47]: https://github.com/trust-center
[48]: https://github.com/partners
[49]: https://github.com/resources
[50]: https://github.com/sponsors
[51]: https://securitylab.github.com
[52]: https://maintainers.github.com
[53]: https://github.com/accelerator
[54]: https://stars.github.com
[55]: https://archiveprogram.github.com
[56]: https://github.com/topics
[57]: https://github.com/trending
[58]: https://github.com/collections
[59]: https://github.com/enterprise
[60]: https://github.com/security/advanced-security
[61]: https://github.com/features/copilot/copilot-business
[62]: https://github.com/premium-support
[63]: https://github.com/pricing
[64]: https://docs.github.com/search-github/github-code-search/understanding-github-code-search-syntax
[65]: https://docs.github.com/search-github/github-code-search/understanding-github-code-search-syntax
[66]: /login?return_to=https%3A%2F%2Fgithub.com%2FMicrosoftDocs%2Fazure-ai-docs%2Fblob%2Fmain%2Farticles%2Ffoundry%2Fage
nts%2Fconcepts%2Flimits-quotas-regions.md
[67]: /signup?ref_cta=Sign+up&ref_loc=header+logged+out&ref_page=%2F%3Cuser-name%3E%2F%3Crepo-name%3E%2Fblob%2Fshow&sour
ce=header-repo&source_repo=MicrosoftDocs%2Fazure-ai-docs
[68]: 
[69]: 
[70]: 
[71]: 
[72]: /MicrosoftDocs
[73]: /MicrosoftDocs/azure-ai-docs
[74]: /login?return_to=%2FMicrosoftDocs%2Fazure-ai-docs
[75]: /login?return_to=%2FMicrosoftDocs%2Fazure-ai-docs
[76]: /login?return_to=%2FMicrosoftDocs%2Fazure-ai-docs
[77]: /MicrosoftDocs/azure-ai-docs
[78]: /MicrosoftDocs/azure-ai-docs/pulls
[79]: /MicrosoftDocs/azure-ai-docs/actions
[80]: /MicrosoftDocs/azure-ai-docs/security
[81]: /MicrosoftDocs/azure-ai-docs/pulse
[82]: /MicrosoftDocs/azure-ai-docs
[83]: /MicrosoftDocs/azure-ai-docs/pulls
[84]: /MicrosoftDocs/azure-ai-docs/actions
[85]: /MicrosoftDocs/azure-ai-docs/security
[86]: /MicrosoftDocs/azure-ai-docs/pulse
[87]: /MicrosoftDocs/azure-ai-docs/tree/main
[88]: /MicrosoftDocs/azure-ai-docs/tree/main/articles
[89]: /MicrosoftDocs/azure-ai-docs/tree/main/articles/foundry
[90]: /MicrosoftDocs/azure-ai-docs/tree/main/articles/foundry/agents
[91]: /MicrosoftDocs/azure-ai-docs/tree/main/articles/foundry/agents/concepts
[92]: /MicrosoftDocs/azure-ai-docs/commits/main/articles/foundry/agents/concepts/limits-quotas-regions.md
[93]: /MicrosoftDocs/azure-ai-docs/commits/main/articles/foundry/agents/concepts/limits-quotas-regions.md
[94]: /MicrosoftDocs/azure-ai-docs/tree/main
[95]: /MicrosoftDocs/azure-ai-docs/tree/main/articles
[96]: /MicrosoftDocs/azure-ai-docs/tree/main/articles/foundry
[97]: /MicrosoftDocs/azure-ai-docs/tree/main/articles/foundry/agents
[98]: /MicrosoftDocs/azure-ai-docs/tree/main/articles/foundry/agents/concepts
[99]: https://github.com/MicrosoftDocs/azure-ai-docs/raw/refs/heads/main/articles/foundry/agents/concepts/limits-quotas-
regions.md
[100]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/concepts/hosted-agents.md
[101]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/how-to/create-projects.md
[102]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/openai/how-to/responses.md#supported-regions
[103]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/foundry-models/concepts/models-sold-directly-by-azure-reg
ion-availability.md
[104]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/concepts/tool-best-practice.md#tool-support-by-reg
ion-and-model
[105]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/includes/agent-service-models-support-list.md
[106]: #supported-regions
[107]: https://aka.ms/openai/gpt-5/2025-08-07
[108]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/concepts/tool-best-practice.md#tool-support-by-reg
ion-and-model
[109]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/openai/concepts/provisioned-throughput.md
[110]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/openai/how-to/spillover-traffic-management.md
[111]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/openai/quotas-limits.md
[112]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/concepts/runtime-components.md
[113]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/concepts/vector-stores.md
[114]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/openai/quotas-limits.md
[115]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/concepts/vector-stores.md
[116]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/observability/how-to/how-to-monitor-agents-dashboard.md
[117]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/openai/quotas-limits.md
[118]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/foundry-models/quotas-limits.md
[119]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/how-to/quota.md
[120]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/how-to/quota.md
[121]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/concepts/runtime-components.md
[122]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/concepts/tool-best-practice.md#tool-support-by-reg
ion-and-model
[123]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/agents/concepts/vector-stores.md
[124]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/observability/how-to/how-to-monitor-agents-dashboard.md
[125]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/openai/quotas-limits.md
[126]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/how-to/quota.md
[127]: https://docs.github.com/site-policy/github-terms/github-terms-of-service
[128]: https://docs.github.com/site-policy/privacy-policies/github-privacy-statement
[129]: https://github.com/security
[130]: https://www.githubstatus.com/
[131]: https://github.community/
[132]: https://docs.github.com/
[133]: https://support.github.com?tags=dotcom-footer
```
