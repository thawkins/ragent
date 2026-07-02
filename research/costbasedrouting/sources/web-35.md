# Web source

- URL: https://github.com/MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/includes/concepts-manage-costs-1.md
- Title: [Skip to content][1]
- Captured (UTC): 2026-06-29T15:42:19.406228433+00:00

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
4. /[includes][90]

/

# concepts-manage-costs-1.md

Copy path
BlameMore file actions
BlameMore file actions

## Latest commit

## History

[History][91]
[][92]History
96 lines (66 loc) · 6 KB
 main

## Breadcrumbs
1. [azure-ai-docs][93]
2. /[articles][94]
3. /[foundry][95]
4. /[includes][96]

/

# concepts-manage-costs-1.md

Copy path
Top

## File metadata and controls
* Preview
* Code
* Blame

96 lines (66 loc) · 6 KB
[Raw][97]
Copy raw file
Download raw file
Outline
Edit and raw actions

───────────┬─────────────────
title      │Include file     
───────────┼─────────────────
description│Include file     
───────────┼─────────────────
author     │sdgilley         
───────────┼─────────────────
ms.reviewer│aashishb         
───────────┼─────────────────
ms.author  │sgilley          
───────────┼─────────────────
ms.service │microsoft-foundry
───────────┼─────────────────
ms.topic   │include          
───────────┼─────────────────
ms.date    │03/20/2026       
───────────┼─────────────────
ms.custom  │include          
───────────┴─────────────────

This article shows you how to estimate expenses before deployment, track spending in real time, and set up alerts to
avoid budget surprises.

## Prerequisites

Before you begin, ensure you have:
* **Azure subscription:** An active Azure subscription with the resources you want to monitor.
* **Role-based access control (RBAC):** One or both of the following roles at the subscription or resource group scope:
  * [**Cost Management Reader**][98] – View costs and usage data.
  * [**Foundry User**][99] – View Foundry resource data and usage context.
    
    [!INCLUDE [role-rename-note][100]]
* **Supported Azure account type:** One of the [supported account types for Cost Management][101].
* **Region and model availability check:** Confirm required model and feature availability in your target regions before
  deployment. For details, see [Feature availability across cloud regions][102].
* **Resource topology awareness:** Know whether your cost views are scoped to subscription, resource group, or resource,
  and keep the same scope when you compare estimate versus actual cost.
* **Reporting latency expectation:** Cost and usage records can appear with delay depending on service ingestion timing.
  Use trend windows instead of minute-by-minute comparisons for reconciliation.

If you need to grant these roles to team members, see [Assign access to Cost Management data][103] and [Foundry RBAC
roles][104].

Use this task-to-role mapping as a starting point:
* **View Cost Management data:** [Cost Management Reader][105].
* **View Foundry resources and related usage context:** [Foundry User][106].
* **Create or modify custom roles:** **Owner** at the target scope.

Note

Foundry doesn't have a dedicated page in the Azure pricing calculator because Foundry is composed of several optional
Azure services. This article shows how to use the calculator to estimate costs for these services.

## Estimate costs before using Foundry

Use the [Azure pricing calculator][107] to estimate costs before you add Foundry resources.
1. Go to the [Azure pricing calculator][108].
2. Search for and select a product, such as Azure Speech in Foundry or Azure Language in Foundry.
3. Select additional products to estimate costs for multiple services. For example, add Azure AI Search to include
   potential search costs.
4. As you add resources to your project, return to the calculator and update estimates.

## Validate your cost plan before rollout

Before rolling out to production, validate the following:
1. Required models and services are available in your target regions. See [Feature availability across cloud
   regions][109].
2. The same resource scopes used in your estimates (subscription, resource group, and resource) are used in Cost
   Management views.
3. Meter-level cost breakdowns map to expected services and deployments in your architecture.
4. Built-in roles or custom roles required for cost visibility are assigned to operations and finance users.

### Worked example: estimate and verify

Use this lightweight workflow to reduce billing surprises:
1. Build an estimate in the Azure pricing calculator for the services in your architecture.
2. Deploy a small test workload and generate representative traffic.
3. In Cost Management, group costs by **Resource** and then by **Meter**.
4. Compare actual meter charges to your estimate assumptions, and adjust your baseline budget.

Expected result: You can map each major estimate assumption to one or more observed billing meters, and explain any
material variance before production rollout.

### Reconcile estimates with actual costs

Use this checklist after each test cycle:
1. Confirm the evaluation scope (subscription, resource group, or resource) matches the scope used in your estimate.
2. Export or view meter-level charges for the same date range used during test traffic.
3. Verify that required tags are present and consistently applied to participating resources.
4. Compare estimate assumptions to observed meters, and record variance by service.
5. Update budgets and alert thresholds only after you validate at least one full billing cycle trend.

**Reference:** [Azure pricing calculator][110]

## Costs associated with Foundry

When you create a Foundry resource, you pay for the Azure services you use, such as Azure OpenAI, Azure Speech in
Foundry, Content Safety, Azure Vision in Foundry, Azure Document Intelligence, and Azure Language in Foundry. Costs vary
by service and feature. For details, see the [Foundry Tools pricing page][111].

## Understand billing models for Foundry

Foundry resources run on Azure infrastructure and accrue costs when deployed. When you create or use Foundry resources,
you're charged based on the services you use.

Common billing approaches include:
* **Pay-as-you-go (Serverless API):** You're billed according to your usage of each Azure service.
* **Commitment tiers:** You commit to using service features for a fixed fee, providing predictable costs. For details,
  see [Commitment tier pricing][112].

Note

If you use the resource above the quota provided by the commitment plan, you pay for the extra usage as described in the
overage amount in the Azure portal when you buy a commitment plan.

## Footer

© 2026 GitHub, Inc.

### Footer navigation
* [Terms][113]
* [Privacy][114]
* [Security][115]
* [Status][116]
* [Community][117]
* [Docs][118]
* [Contact][119]
* Manage cookies
* Do not share my personal information

You can’t perform that action at this time.

[1]: #start-of-content
[2]: /login?return_to=https%3A%2F%2Fgithub.com%2FMicrosoftDocs%2Fazure-ai-docs%2Fblob%2Fmain%2Farticles%2Ffoundry%2Fincl
udes%2Fconcepts-manage-costs-1.md
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
[66]: /login?return_to=https%3A%2F%2Fgithub.com%2FMicrosoftDocs%2Fazure-ai-docs%2Fblob%2Fmain%2Farticles%2Ffoundry%2Finc
ludes%2Fconcepts-manage-costs-1.md
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
[90]: /MicrosoftDocs/azure-ai-docs/tree/main/articles/foundry/includes
[91]: /MicrosoftDocs/azure-ai-docs/commits/main/articles/foundry/includes/concepts-manage-costs-1.md
[92]: /MicrosoftDocs/azure-ai-docs/commits/main/articles/foundry/includes/concepts-manage-costs-1.md
[93]: /MicrosoftDocs/azure-ai-docs/tree/main
[94]: /MicrosoftDocs/azure-ai-docs/tree/main/articles
[95]: /MicrosoftDocs/azure-ai-docs/tree/main/articles/foundry
[96]: /MicrosoftDocs/azure-ai-docs/tree/main/articles/foundry/includes
[97]: https://github.com/MicrosoftDocs/azure-ai-docs/raw/refs/heads/main/articles/foundry/includes/concepts-manage-costs
-1.md
[98]: /MicrosoftDocs/azure-ai-docs/blob/main/azure/role-based-access-control/built-in-roles/management-and-governance#co
st-management-reader
[99]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/concepts/rbac-foundry.md#built-in-roles
[100]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/includes/role-rename-note.md
[101]: /MicrosoftDocs/azure-ai-docs/blob/main/azure/cost-management-billing/costs/understand-cost-mgt-data
[102]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/reference/region-support.md
[103]: /MicrosoftDocs/azure-ai-docs/blob/main/azure/cost-management-billing/costs/assign-access-acm-data
[104]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/concepts/rbac-foundry.md
[105]: /MicrosoftDocs/azure-ai-docs/blob/main/azure/role-based-access-control/built-in-roles/management-and-governance#c
ost-management-reader
[106]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/concepts/rbac-foundry.md#built-in-roles
[107]: https://azure.microsoft.com/pricing/calculator/
[108]: https://azure.microsoft.com/pricing/calculator/
[109]: /MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/reference/region-support.md
[110]: https://azure.microsoft.com/pricing/calculator/
[111]: https://azure.microsoft.com/pricing/details/cognitive-services/
[112]: /MicrosoftDocs/azure-ai-docs/blob/main/azure/ai-services/commitment-tier
[113]: https://docs.github.com/site-policy/github-terms/github-terms-of-service
[114]: https://docs.github.com/site-policy/privacy-policies/github-privacy-statement
[115]: https://github.com/security
[116]: https://www.githubstatus.com/
[117]: https://github.community/
[118]: https://docs.github.com/
[119]: https://support.github.com?tags=dotcom-footer
```
