# Web source

- URL: https://learn.microsoft.com/en-us/azure/foundry/concepts/manage-costs
- Title: [ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]
- Captured (UTC): 2026-06-29T15:42:18.197891261+00:00

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

# Plan and manage costs for Microsoft Foundry

Feedback
Summarize this article for me

## In this article

This article shows you how to estimate expenses before deployment, track spending in real time, and set up alerts to
avoid budget surprises.

## Prerequisites

Before you begin, ensure you have:
* **Azure subscription:** An active Azure subscription with the resources you want to monitor.
* **Role-based access control (RBAC):** One or both of the following roles at the subscription or resource group scope:
  * [**Cost Management Reader**][8] – View costs and usage data.
  * [**Foundry User**][9] – View Foundry resource data and usage context.
    
    Important
    
    The Foundry RBAC roles were recently renamed. **Foundry User**, **Foundry Owner**, **Foundry Account Owner**, and
    **Foundry Project Manager** were previously named Azure AI User, Azure AI Owner, Azure AI Account Owner, and Azure
    AI Project Manager. You might still see the previous names in some places while the rename rolls out. The role IDs
    and core permissions are unchanged by the rename.
* **Supported Azure account type:** One of the [supported account types for Cost Management][10].
* **Region and model availability check:** Confirm required model and feature availability in your target regions before
  deployment. For details, see [Feature availability across cloud regions][11].
* **Resource topology awareness:** Know whether your cost views are scoped to subscription, resource group, or resource,
  and keep the same scope when you compare estimate versus actual cost.
* **Reporting latency expectation:** Cost and usage records can appear with delay depending on service ingestion timing.
  Use trend windows instead of minute-by-minute comparisons for reconciliation.

If you need to grant these roles to team members, see [Assign access to Cost Management data][12] and [Foundry RBAC
roles][13].

Use this task-to-role mapping as a starting point:
* **View Cost Management data:** [Cost Management Reader][14].
* **View Foundry resources and related usage context:** [Foundry User][15].
* **Create or modify custom roles:** **Owner** at the target scope.

Note

Foundry doesn't have a dedicated page in the Azure pricing calculator because Foundry is composed of several optional
Azure services. This article shows how to use the calculator to estimate costs for these services.

## Estimate costs before using Foundry

Use the [Azure pricing calculator][16] to estimate costs before you add Foundry resources.
1. Go to the [Azure pricing calculator][17].
2. Search for and select a product, such as Azure Speech in Foundry or Azure Language in Foundry.
3. Select additional products to estimate costs for multiple services. For example, add Azure AI Search to include
   potential search costs.
4. As you add resources to your project, return to the calculator and update estimates.

## Validate your cost plan before rollout

Before rolling out to production, validate the following:
1. Required models and services are available in your target regions. See [Feature availability across cloud
   regions][18].
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

**Reference:** [Azure pricing calculator][19]

## Costs associated with Foundry

When you create a Foundry resource, you pay for the Azure services you use, such as Azure OpenAI, Azure Speech in
Foundry, Content Safety, Azure Vision in Foundry, Azure Document Intelligence, and Azure Language in Foundry. Costs vary
by service and feature. For details, see the [Foundry Tools pricing page][20].

## Understand billing models for Foundry

Foundry resources run on Azure infrastructure and accrue costs when deployed. When you create or use Foundry resources,
you're charged based on the services you use.

Common billing approaches include:
* **Pay-as-you-go (Serverless API):** You're billed according to your usage of each Azure service.
* **Commitment tiers:** You commit to using service features for a fixed fee, providing predictable costs. For details,
  see [Commitment tier pricing][21].

Note

If you use the resource above the quota provided by the commitment plan, you pay for the extra usage as described in the
overage amount in the Azure portal when you buy a commitment plan.

## Understand the billing model for Foundry Models

### Token-based pricing

Language and vision models process inputs by breaking them down into tokens. Text, image, and audio workloads can all
use token-based metering. The billing unit and rate can vary by model, deployment type, and meter. Check the pricing
page for the exact meter names and units for your deployment. For current rates, see the [Azure OpenAI pricing
page][22].

### Foundry Models sold by Azure

Models sold by Azure (including Azure OpenAI) are billed by Microsoft. In Cost Management, these charges typically
appear as model-related meters associated with your deployed resources.

### Fine-tuned models

Azure OpenAI fine-tuned models are charged in three ways:
* **Training:** Charged per token or per hour, depending on the model.
* **Hosting:** Hourly cost per deployed model (applies even if the model is unused).
* **Inference:** Per 1,000 tokens (input and output) when the model is called.

Monitor hosted fine-tuned model costs closely to avoid unexpected charges. For current rates, see the [Azure OpenAI
pricing page][23].

Important

Fine-tuned deployments incur hosting charges while deployed, even during low usage periods. Remove or scale down
deployments that you don't need. For details on deployment lifecycle and cleanup policies, see the [fine-tuning
documentation][24].

### HTTP Error response code and billing status

HTTP status codes alone don't determine whether usage is billed. Charges depend on whether billable processing occurred
for the request and on the specific meter behavior.

Use Cost Management meter data and service metrics to reconcile billed usage, and treat your invoice and meter records
as the source of truth.

## Monitor costs

Track your Foundry spending using cost analysis tools. You can view costs by day, month, or year, compare against
budgets, and identify spending trends.

Access cost information from the [Microsoft Foundry][25] portal or the [Azure portal][26]. **Reference:** [Cost
analysis][27]

Important

Your Foundry costs are only a subset of your overall application or solution costs. You need to monitor costs for all
Azure resources used in your application or solution.

### Configure permissions to view costs

To view Foundry costs, assign roles based on the task and scope. For cost reporting, assign the [Cost Management Reader
role][28] at the required scope. Assign the [Foundry User role][29] when users also need to inspect Foundry resources
and usage context.

Important

The Foundry RBAC roles were recently renamed. **Foundry User**, **Foundry Owner**, **Foundry Account Owner**, and
**Foundry Project Manager** were previously named Azure AI User, Azure AI Owner, Azure AI Account Owner, and Azure AI
Project Manager. You might still see the previous names in some places while the rename rolls out. The role IDs and core
permissions are unchanged by the rename.

If built-in roles don't meet your needs, you can create a custom role with least-privilege permissions. Validate role
actions in your environment because available actions can evolve over time.

Example read permissions:
* `Microsoft.Consumption/*/read`
* `Microsoft.CostManagement/*/read`
* `Microsoft.Resources/subscriptions/read`
* `Microsoft.CognitiveServices/accounts/AIServices/usage/read`

Note

You need the **Owner** role at the subscription or resource group scope to create custom roles in that scope.

To create a custom role, use one of the following articles:
* [Azure portal][30]
* [Azure CLI][31]
* [Azure PowerShell][32]

For more information about custom roles, see [Azure custom roles][33].

To create a custom role, construct a role definition JSON file that specifies permissions and scope for the role. The
following example is an illustrative starting point for a custom Foundry Cost Reader role:

`{
    "Name": "Foundry Cost Reader",
    "IsCustom": true,
    "Description": "Can see cost metrics in Foundry",
    "Actions": [
        "Microsoft.Consumption/*/read",
        "Microsoft.CostManagement/*/read",
        "Microsoft.Resources/subscriptions/read",
        "Microsoft.CognitiveServices/accounts/AIServices/usage/read"
    ],
    "NotActions": [],
    "DataActions": [],
    "NotDataActions": [],
    "AssignableScopes": [
        
"/subscriptions/<subscriptionId>/resourceGroups/<resourceGroupName>/providers/Microsoft.CognitiveServices/accounts/<foun
dryResourceName>"
    ]
}
`

Replace `<subscriptionId>`, `<resourceGroupName>`, and `<foundryResourceName>` with your actual values.

Note

Validate custom role definitions in a nonproduction environment before broad rollout, and verify each action against
your tenant's supported resource provider operations.

Note

This custom role example doesn't grant access to Foundry resources by itself. Assign an additional role such as [Foundry
User][34] if users also need Foundry resource visibility.

## Monitor in Foundry portal
1. Sign in to [Microsoft Foundry][35]. Make sure the **New Foundry** toggle is on. These steps refer to **Foundry
   (new)**.
2. Use the sections below to monitor costs.

Note

Foundry portal labels and navigation can vary slightly by tenant and release wave. If you don't see an exact label in
this article, use equivalent cost views in the same project scope.

Note

Estimates do not reflect discounts or contracted pricing that may appear on your final bill. Estimates cover standard
deployment costs only, not [provisioned throughput][36].

### Agent costs
1. Select **Operate** in the upper-right navigation.
2. Select **Overview** in the left pane.
3. At the top of the page, select the subscription, one or more projects, and a date range.
4. The **Estimated cost** tile shows estimates of all the agents for the selected project(s) for the selected dates.
   These estimates don't include prompt agent and non-Foundry agent costs.

[ [Screenshot of the Agents tab under Assets, showing the Estimated costs column with monthly cost estimates for each
agent based on configuration and usage.] ][37]

For individual agent estimates:
1. Select **Assets** in the left pane.
2. Select the **Agents** tab.
3. The **Estimated costs** column shows monthly estimates based on agent configuration and usage patterns.

**Reference:** [Agent concepts][38]

[ [Screenshot of the Agents tab showing a list of agents with columns for Name, Status, and Estimated costs. The
Estimated costs column displays monthly values.] ][39]

To view detailed agent costs:
1. Select **Build** in the upper-right navigation.
2. Select **Agents** in the left pane.
3. Select an agent.
4. Select the **Monitor** tab.
5. Set the date range in the upper-right corner.
6. View token costs and usage metrics for the selected range.

**Reference:** [Monitor agent metrics][40]

[ [Screenshot of the Monitor tab for an agent, showing operational metrics including total token cost, token usage,
average inference latency, agent runs chart, and runs and token metrics.] ][41]

### Model deployment costs
1. Select **Build** in the upper-right navigation.
2. Select **Models** in the left pane.
3. Select a model.
4. Select the **Monitor** tab.
5. Set the date range in the upper-right corner. You see total cost and an estimated cost chart for the selected range.

**Reference:** [Monitor models][42]

[ [Screenshot of Azure portal showing the Monitor tab with total cost and estimated cost chart for a selected model and
date range.] ][43]

When you select **View More Details** or **Azure Cost Management**, you're directed to the Azure portal's **Cost
Management** section. Azure portal costs can show aggregated charges for the related account scope, not only individual
models.

Note

Token and request charts can temporarily differ from **Estimated cost** because of ingestion timing and aggregation
differences. Use **Estimated cost** for near-real-time monitoring, and use Microsoft Cost Management and invoiced
charges for financial reconciliation.

## Monitor in Azure portal
1. Sign in to the [Azure portal][44].
2. View costs for your resource group or individual Foundry resource.
   
   Tip
   
   To open your Foundry resource in Azure portal:
   1. Sign in to [Microsoft Foundry][45]. Make sure the **New Foundry** toggle is on. These steps refer to **Foundry
      (new)**.
   2. Select **Operate** from the upper-right navigation.
   3. Select **Admin**.
   4. Select the link for the parent resource in the second column.
   5. Select **Manage this resource in the Azure portal** under the **View resource** heading in the upper-right.
3. In the Azure portal, select **Cost analysis** under **Cost Management** (for your resource group or Foundry
   resource).
4. View the cost overview. Optionally, add filters (deployment tags, user-defined tags) to segment costs by model
   deployment:
   
   [ [Screenshot of cost overview showing deployment-level tags filter.] ][46]
5. Select **Costs by resource** > **Resources** to see your Foundry resource cost split across model deployments:
   
   [ [Screenshot of split of Foundry resource cost across model deployments.] ][47]

### Understand cost breakdown by meter

Use the **Cost Analysis** tool to view costs grouped by billing meter:
1. Sign in to the [Azure portal][48] and select your resource group.
2. Select **Cost analysis** under **Cost Management**.
3. By default, cost analysis is scoped to the selected resource group.
   
   Important
   
   Scope *Cost Analysis* to the resource group where you deployed the Foundry resource. The cost meters associated with
   Models from partners and community display under the resource group instead of the Foundry resource.
4. Modify **Group by** to **Meter**. You can now see that for this particular resource group, the source of the costs
   comes from different model series.
   
   [ [Screenshot of how to see the cost by each meter in the resource group.] ][49]

#### Models sold by Azure

Models sold by Azure (including Azure OpenAI) are billed directly by Microsoft. When you inspect your bill, you
typically see meters that account for model input and output usage.

[ [Screenshot of cost analysis dashboard scoped to the resource group where the Foundry resource is deployed,
highlighting the meters for Azure OpenAI and Phi models. Cost is group by meter.] ][50]

### Monitor costs by resource

You can get more detailed billing information by grouping costs by resource:
1. In **Cost Analysis**, select **View** > **Cost by resource**.
   
   [ [Screenshot of how to see the cost by each resource in the resource group.] ][51]
2. Now you can see the resources generating each of the billing meters. To understand the breakdown of what makes up
   that cost, it can help to modify **Group by** to **Meter** and switching the chart type to **Line**.
3. Azure OpenAI models and Microsoft models are displayed as meters under each Foundry resource.
4. Some providers' models are displayed as meters under Global resources. The word *Global* **isn't** related to the SKU
   of the model deployment (for instance, *Global standard*). If you have multiple Foundry resources, your bill contains
   one entry **for each model for each Foundry resource**. The resource meters have the format *model-name-GUID* where
   the GUID is an identifier associated with a given Foundry resource. You notice billing meters accounting for inputs
   and outputs for each model you consumed.
   
   [ [Screenshot of cost analysis dashboard scoped to the resource group where the Foundry resource is deployed,
   highlighting the meters for models billed throughout Azure Marketplace. Cost is group by resource.] ][52]

## Chargeback with project-level cost attribution (Preview)

Microsoft Foundry supports chargeback at the project level, so FinOps teams and admins can allocate shared Foundry spend
back to the business unit, team, or workload that incurred it. Project-level attribution is useful when multiple
projects share the same Foundry resource and you need to split the bill accurately.

Every Foundry project is automatically tagged with a `project` tag on its underlying usage. In Cost Management, filter
the cost analysis view by the `project` tag to see spend broken down per project. You don't need to add tags manually.

Note

Project-level cost attribution is currently supported for Models sold by Azure (Azure Direct models, including Azure
OpenAI). It isn't yet supported for models served through Azure Marketplace.

### View costs by project
1. Sign in to the [Azure portal][53] and open your Foundry resource.
2. Select **Cost analysis** under **Resource Management** in the left navigation.
3. In the filter bar, select **Add filter**, choose **Tag**, then choose `project`.
4. Select one or more projects to view their attributed cost over the selected time range.
   
   [ [Screenshot of Cost Management Cost analysis view filtered by the project tag, showing accumulated cost over time
   for a selected Foundry project.] ][54]

### What you can do with project-level attribution
* Allocate shared Foundry resource costs back to individual projects for chargeback or showback.
* Track project-level spend trends over time.

It's important to understand scope when you evaluate costs associated with Foundry resources. If your resources are part
of the same resource group, you can scope Cost Analysis at that level to understand the effect on costs. If your
resources are spread across multiple resource groups, you can scope to the subscription level.

When scoped at a higher level, you often need to add more filters to focus on Azure OpenAI usage. When scoped at the
subscription level, you see many other resources that you might not care about in the context of Azure OpenAI cost
management. When you scope at the subscription level, navigate to the full **Cost analysis tool** under the **Cost
Management** service.

Here's an example of how to use the **Cost analysis tool** to see your accumulated costs for a subscription or resource
group:
1. Search for *Cost Management* in the top Azure search bar to navigate to the full service experience, which includes
   more options such as creating budgets.
2. If necessary, select **change** if the **Scope:** isn't pointing to the resource group or subscription you want to
   analyze.
3. On the left, select **Reporting + analytics** > **Cost analysis**.
4. On the **All views** tab, select **Accumulated costs**.

[ [Screenshot of cost analysis dashboard showing how to access accumulated costs.] ][55]

The cost analysis dashboard shows the accumulated costs that are analyzed depending on what you specified for **Scope**.

[ [Screenshot of cost analysis dashboard with scope set to subscription.] ][56]

If you try to add a filter by service, you can't find Azure OpenAI in the list. This situation occurs because Azure
OpenAI usage appears under the broader **Cognitive Services** service classification in Cost Management. If you want to
focus on Azure OpenAI usage across a subscription, use **Service tier: Azure OpenAI**:

[ [Screenshot of cost analysis dashboard with service tier highlighted.] ][57]

## Create budgets

**Prevent cost overruns with automated alerts.** [Create budgets][58] that track your spending limits and [set up
alerts][59] to notify you when costs approach or exceed thresholds.

**Best practice:** Create budgets and alerts for Azure subscriptions and resource groups as part of an overall cost
monitoring strategy.

Create budgets with filters for specific resources or services in Azure if you want more granularity in your monitoring.
Filters help ensure that you don't accidentally create new resources that cost more money. For more about filter options
when you create a budget, see [Group and filter options][60].

Important

While OpenAI has an option for hard limits that prevent you from going over your budget, Azure OpenAI doesn't currently
provide this functionality. You can start automation from action groups as part of your budget notifications to take
more advanced actions, but this functionality requires additional custom development.

## Export cost data

You can [export your cost data][61] to a storage account. Exporting data is helpful when you or others need to do
additional data analysis for costs. For example, finance teams can analyze the data by using Excel or Power BI. You can
export your costs on a daily, weekly, or monthly schedule and set a custom date range. Exporting cost data is the
recommended way to retrieve cost datasets.

## Other costs that might accrue

Enabling capabilities such as sending data to Azure Monitor Logs and alerting incur extra costs for those services.
These costs are visible under those other services and at the subscription level, but aren't visible when scoped just to
your Foundry resource.

### Using Azure Prepayment

You can pay for Models sold by Azure charges with your Azure Prepayment (previously called monetary commitment) credit.
However, you can't use Azure Prepayment credit to pay for charges for other provider models because they're billed
through Azure Marketplace.

For more information, see [Azure pricing calculator][62].

## Troubleshoot common cost analysis issues
* **Costs don't match your estimate:** Confirm that all dependent resources (for example, storage, networking, and
  Marketplace resources) are included in your Cost Management scope.
* **Can't see cost data:** Confirm you have both cost visibility permissions and Foundry access permissions at the
  correct scope.
* **Unexpected meter charges:** Group by **Meter** and **Resource** to identify which service generated the charge, then
  compare with deployment and traffic patterns.
* **Region rollout cost variance:** Validate region/model availability before deployment and recheck assumptions if you
  deploy in different regions.
* **Tag filters return incomplete results:** Verify required tags are applied to all participating resources and
  inherited consistently from your deployment process.
* **Budget alerts are noisy or delayed:** Recalibrate alert thresholds after observing normal usage for a full trend
  window, then separate warning and critical thresholds.
* **Policy or scope drift changes cost visibility:** Confirm your selected scope and policy assignments still include
  all resources used by the workload.
* **Data appears delayed after test runs:** Wait for ingestion latency, then recheck the same time window before
  concluding there is a billing discrepancy.

## Related content
* [Instant access to models in Microsoft Foundry (preview)][63]
* [Microsoft Foundry pricing][64]
* Learn [how to optimize your cloud investment with cost management][65].
* Learn more about managing costs with [cost analysis][66].
* Learn about how to [prevent unexpected costs][67].
* Take the [Cost Management][68] guided learning course.

## Feedback

Was this page helpful?

Yes No No

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?

## Additional resources
* Last updated on 2026-05-14

### In this article

Was this page helpful?

Need help with this topic?

Want to try using Ask Learn to clarify or guide you through this topic?

Ask Learn Ask Learn
Suggest a fix?
[en-us][69]
[ Your Privacy Choices][70]
Theme
* Light
* Dark
* High contrast
* [AI Disclaimer][71]
* [Previous Versions][72]
* [Blog][73]
* [Contribute][74]
* [Privacy][75]
* [Consumer Health Privacy][76]
* [Terms of Use][77]
* [Trademarks][78]
* © Microsoft 2026

[1]: #main
[2]: #
[3]: https://go.microsoft.com/fwlink/p/?LinkID=2092881 
[4]: https://learn.microsoft.com/en-us/lifecycle/faq/internet-explorer-microsoft-edge
[5]: #
[6]: https://github.com/MicrosoftDocs/azure-ai-docs/blob/main/articles/foundry/concepts/manage-costs.md
[7]: #
[8]: /en-us/azure/role-based-access-control/built-in-roles/management-and-governance#cost-management-reader
[9]: rbac-foundry#built-in-roles
[10]: /en-us/azure/cost-management-billing/costs/understand-cost-mgt-data
[11]: ../reference/region-support
[12]: /en-us/azure/cost-management-billing/costs/assign-access-acm-data
[13]: rbac-foundry
[14]: /en-us/azure/role-based-access-control/built-in-roles/management-and-governance#cost-management-reader
[15]: rbac-foundry#built-in-roles
[16]: https://azure.microsoft.com/pricing/calculator/
[17]: https://azure.microsoft.com/pricing/calculator/
[18]: ../reference/region-support
[19]: https://azure.microsoft.com/pricing/calculator/
[20]: https://azure.microsoft.com/pricing/details/cognitive-services/
[21]: /en-us/azure/ai-services/commitment-tier
[22]: https://azure.microsoft.com/pricing/details/azure-openai/
[23]: https://azure.microsoft.com/pricing/details/azure-openai/
[24]: ../openai/how-to/fine-tuning
[25]: https://ai.azure.com/?cid=learnDocs
[26]: https://portal.azure.com/
[27]: /en-us/azure/cost-management-billing/costs/quick-acm-cost-analysis
[28]: /en-us/azure/role-based-access-control/built-in-roles/management-and-governance#cost-management-reader
[29]: rbac-foundry#built-in-roles
[30]: /en-us/azure/role-based-access-control/custom-roles-portal
[31]: /en-us/azure/role-based-access-control/custom-roles-cli
[32]: /en-us/azure/role-based-access-control/custom-roles-powershell
[33]: /en-us/azure/role-based-access-control/custom-roles
[34]: rbac-foundry#built-in-roles
[35]: https://ai.azure.com/?cid=learnDocs
[36]: ../openai/concepts/provisioned-throughput
[37]: media/manage-costs/agent-costs.png#lightbox
[38]: ../agents/concepts/development-lifecycle
[39]: media/manage-costs/agent-list.png#lightbox
[40]: ../observability/how-to/how-to-monitor-agents-dashboard
[41]: media/manage-costs/agent-build-cost.png#lightbox
[42]: ../foundry-models/how-to/monitor-models
[43]: media/manage-costs/model-costs.png#lightbox
[44]: https://portal.azure.com/
[45]: https://ai.azure.com/?cid=learnDocs
[46]: ../media/manage-costs/cost-overview-deployment-tags.png#lightbox
[47]: ../media/manage-costs/azure-foundry-cost-split.png#lightbox
[48]: https://portal.azure.com/
[49]: ../foundry-models/media/manage-cost/cost-by-meter.png#lightbox
[50]: ../foundry-models/media/manage-cost/cost-by-meter-1p.png#lightbox
[51]: ../foundry-models/media/manage-cost/cost-by-resource.png#lightbox
[52]: ../foundry-models/media/manage-cost/cost-by-resource-saas.png#lightbox
[53]: https://portal.azure.com/
[54]: media/manage-costs/cost-analysis-project-tag.png#lightbox
[55]: ../openai/media/manage-costs/cost-analyzer.png#lightbox
[56]: ../openai/media/manage-costs/subscription.png#lightbox
[57]: ../openai/media/manage-costs/service-tier.png#lightbox
[58]: /en-us/azure/cost-management-billing/costs/tutorial-acm-create-budgets
[59]: /en-us/azure/cost-management-billing/costs/cost-mgt-alerts-monitor-usage-spending
[60]: /en-us/azure/cost-management-billing/costs/group-filter
[61]: /en-us/azure/cost-management-billing/costs/tutorial-export-acm-data
[62]: https://azure.microsoft.com/pricing/calculator/
[63]: instant-models
[64]: https://azure.microsoft.com/pricing/details/microsoft-foundry/
[65]: /en-us/azure/cost-management-billing/costs/cost-mgt-best-practices
[66]: /en-us/azure/cost-management-billing/costs/quick-acm-cost-analysis
[67]: /en-us/azure/cost-management-billing/understand/analyze-unexpected-charges
[68]: /en-us/training/paths/control-spending-manage-bills
[69]: #
[70]: https://aka.ms/yourcaliforniaprivacychoices
[71]: https://learn.microsoft.com/en-us/principles-for-ai-generated-content
[72]: https://learn.microsoft.com/en-us/previous-versions/
[73]: https://techcommunity.microsoft.com/t5/microsoft-learn-blog/bg-p/MicrosoftLearnBlog
[74]: https://learn.microsoft.com/en-us/contribute
[75]: https://go.microsoft.com/fwlink/?LinkId=521839
[76]: https://go.microsoft.com/fwlink/?linkid=2259814
[77]: https://learn.microsoft.com/en-us/legal/termsofuse
[78]: https://www.microsoft.com/legal/intellectualproperty/Trademarks/
```
