# Web source

- URL: https://learn.microsoft.com/en-us/azure/foundry/how-to/high-availability-resiliency
- Title: [ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]
- Captured (UTC): 2026-06-29T15:44:02.470696368+00:00

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

# High availability and resiliency for Microsoft Foundry projects and Agent Services

Feedback
Summarize this article for me

## In this article

Important

Items marked (preview) in this article are currently in public preview. This preview is provided without a service-level
agreement, and we don't recommend it for production workloads. Certain features might not be supported or might have
constrained capabilities. For more information, see [Supplemental Terms of Use for Microsoft Azure Previews][8].

Plan ahead to maintain business continuity and prepare for disaster recovery with [Microsoft Foundry][9].

Microsoft strives to ensure that Azure services are always available. However, unplanned service outages might occur.
This article walks you through configuring multi-region deployments, hardening infrastructure resources, designing model
deployment resiliency, and preparing failover procedures for Foundry projects and Agent Services.

Important

Foundry itself doesn't provide automatic failover or disaster recovery.

## Prerequisites
* An Azure subscription. If you don't have one, create a [free account][10].
* A Microsoft Foundry account and project. For more information, see the [Microsoft Foundry Quickstart][11].
* Azure CLI installed (optional, for applying resource locks via command line).
* Appropriate RBAC roles:
  * **Owner** or **Contributor** on the resource group to deploy and configure resources.
  * **User Access Administrator** to assign RBAC roles to managed identities.
  * **Cosmos DB Operator** for Azure Cosmos DB configuration.
  * **Search Service Contributor** for Azure AI Search configuration.
  * **Storage Account Contributor** for Azure Storage configuration.

Important

Microsoft and you jointly operate the Foundry Agent Service. Microsoft runs the control plane and capability host
platform. You own the durability of stateful dependencies (Azure Cosmos DB, Azure AI Search, Azure Storage) when you use
Standard agent deployment mode. In Basic mode, Microsoft manages those data components and recovery options are limited.
This shared responsibility model means your HA/DR design must cover each customer-managed component individually.

## Identify Azure services for Foundry

Foundry is an Azure native service with fewer implicit dependencies than the earlier workspace model. Foundry projects
can attach resources based on workload patterns, such as retrieval, orchestration, monitoring, and integration. Treat
attached resources as optional unless your workload requires them.

Service categories include:
* **Platform infrastructure (Microsoft-managed)**: Control plane and project metadata service components that Microsoft
  operates regionally.
* **Optional workload and integration resources (customer-managed)**: Azure Storage, Azure Key Vault, Azure Container
  Registry (ACR), Application Insights, Azure Logic Apps, Azure Functions, Azure AI Search, Azure Cosmos DB, Azure Event
  Grid, SharePoint, Microsoft Purview (explicit connection), and other connection targets.
* **Connections**: Configuration objects that reference external Azure or SaaS services. You own their high availability
  configuration.

None of these optional resources, such as Key Vault, Storage, ACR, and Application Insights, are hard dependencies of
the Foundry resource model itself, though your solution might require them. Design per workload and avoid assuming a
fixed mandatory set.

────────────────────────┬─────────────────────────────────┬───────┬─────────────────────────────────────────────────────
Resource type           │Example services                 │Managed│Notes on availability                                
                        │                                 │by     │                                                     
────────────────────────┼─────────────────────────────────┼───────┼─────────────────────────────────────────────────────
Platform infrastructure │Foundry control plane, project   │Microso│Regional; no customer action for zone configuration. 
                        │metadata                         │ft     │                                                     
────────────────────────┼─────────────────────────────────┼───────┼─────────────────────────────────────────────────────
State stores (Standard  │Azure Cosmos DB, Azure AI Search,│You    │Configure redundancy, backup, replication.           
agent mode)             │Azure Storage                    │       │                                                     
────────────────────────┼─────────────────────────────────┼───────┼─────────────────────────────────────────────────────
Security and secrets    │Azure Key Vault                  │You    │Automatic zone redundancy when supported; configure  
                        │                                 │       │RBAC and purge protection.                           
────────────────────────┼─────────────────────────────────┼───────┼─────────────────────────────────────────────────────
Monitoring              │Application Insights             │You    │Consider multiregion instances or failover strategy. 
────────────────────────┼─────────────────────────────────┼───────┼─────────────────────────────────────────────────────
Image and artifact      │Azure Container Registry         │You    │Use geo-replication as needed.                       
registry                │                                 │       │                                                     
────────────────────────┼─────────────────────────────────┼───────┼─────────────────────────────────────────────────────
Integration and workflow│Logic Apps, Functions, Event Grid│You    │Align region and DR strategy with agent dependencies.
────────────────────────┼─────────────────────────────────┼───────┼─────────────────────────────────────────────────────
Compliance and data     │Microsoft Purview (connected)    │You    │Enable continuity for eDiscovery scenarios.          
mapping                 │                                 │       │                                                     
────────────────────────┼─────────────────────────────────┼───────┼─────────────────────────────────────────────────────
Other knowledge and tool│SharePoint, custom APIs          │You    │Configure per service HA.                            
sources                 │                                 │       │                                                     
────────────────────────┴─────────────────────────────────┴───────┴─────────────────────────────────────────────────────

The rest of this article explains how to make each component highly available.

## Prevent disasters and data loss

Prevention is the primary defense against outages. Apply these recommendations to reduce the likelihood of incidents and
design resiliency into your workload. For more information, see [Design for resiliency][12].

### Prevent resource deletion

To prevent most accidental deletions, apply delete [resource locks][13] to critical resources. Locks protect against
resource-level deletion but not data plane operations. Apply delete locks to these resources.

The following table describes the protections and limitations for each resource:

────────────┬───────────────────────────────────────────────┬───────────────────────────────────────────────────────────
Resource    │Protection provided                            │Limitations                                                
────────────┼───────────────────────────────────────────────┼───────────────────────────────────────────────────────────
Foundry     │Prevents deletion of account, projects, models,│Doesn't protect individual agents or threads.              
account     │connections, and agent capability hosts.       │                                                           
────────────┼───────────────────────────────────────────────┼───────────────────────────────────────────────────────────
Azure Cosmos│Prevents deletion of account,                  │Doesn't protect data within containers.                    
DB account  │`enterprise_memory` database, and containers.  │                                                           
────────────┼───────────────────────────────────────────────┼───────────────────────────────────────────────────────────
Azure AI    │Prevents deletion of the search service        │Doesn't protect indexes or data within indexes.            
Search      │instance.                                      │                                                           
service     │                                               │                                                           
────────────┼───────────────────────────────────────────────┼───────────────────────────────────────────────────────────
Azure       │Prevents deletion of account and blob          │Doesn't protect individual blobs. Users with the **Owner** 
Storage     │containers.                                    │role can remove the lock before deleting a container.      
account     │                                               │                                                           
────────────┴───────────────────────────────────────────────┴───────────────────────────────────────────────────────────

For resilience in depth, combine resource locks with the Azure Policy [`denyAction` effect][14] to block resource
provider delete requests. This layered approach strengthens protection regardless of each resource's recovery
capabilities.

The following Azure CLI command applies a delete lock to a Foundry account:

`az lock create \
  --name "FoundryAccountLock" \
  --lock-type CanNotDelete \
  --resource-group "<your-resource-group>" \
  --resource-name "<your-foundry-account>" \
  --resource-type "Microsoft.CognitiveServices/accounts"
`

To verify the lock was applied:

`az lock list --resource-group "<your-resource-group>" --output table
`

Expected output shows the lock name and type for your resources.

**Reference:** [az lock create][15]

### Implement least privilege access

Use Azure role-based access control (RBAC) to limit access to control and data planes. Grant only required permissions
and audit them regularly.

In production, don't grant standing delete permissions on these resources to any principal. For data plane access to
state stores, only the project's managed identity should have standing write permissions.

You can also destroy data through Agent Service REST APIs. Built-in AI roles like [Foundry User][16] can delete
operational data by using these APIs or the Foundry portal. Accidents or abuse of these APIs can create recovery needs.
No built-in AI role is read only for these data plane operations. For more information, see [Azure AI Foundry REST API
reference][17]. Create [custom roles][18] to limit access to these `Microsoft.CognitiveServices/*/write` data actions.

Important

The Foundry RBAC roles were recently renamed. **Foundry User**, **Foundry Owner**, **Foundry Account Owner**, and
**Foundry Project Manager** were previously named Azure AI User, Azure AI Owner, Azure AI Account Owner, and Azure AI
Project Manager. You might still see the previous names in some places while the rename rolls out. The role IDs and core
permissions are unchanged by the rename.

### Implement the single responsibility principle

Dedicate your Azure Cosmos DB account, Azure AI Search service, and Azure Storage account exclusively to your workload's
AI Agent Service. Sharing these resources with other Foundry accounts or workload components increases risk through
broader permission surfaces and a larger blast radius. Unrelated operations from one workload should never remove or
corrupt agent state in another workload. This separation also allows you to make per‑workload recovery decisions without
needing to take an all-or-nothing approach.

### Use zone-redundant configurations

Use zone redundant configurations for your Azure Cosmos DB account, Azure AI Search service, and Azure Storage account.
This setup protects against zone failures within a region. Zone redundant configurations don't protect against full
regional outages or human or automation errors. The Microsoft-hosted components of the Agent Service are zone redundant.

## Configure resources to support recovery

Configure these resources before an incident happens. The recovery steps in this guide assume you applied the following
settings.

────────┬────────────────────────────────────────────────────────┬──────────────────────────────────────────────────────
Resource│Recommended configurations                              │Purpose                                               
────────┼────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────
Foundry │Establish an [explicit connection to Microsoft          │Supports data continuity for compliance scenarios like
account │Purview][19].                                           │eDiscovery requests after thread data is lost.        
────────┼────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────
Foundry │Use a user-assigned managed identity, not a             │Supports restoration of access to agent dependencies  
project │system-assigned managed identity.                       │without reapplying role assignments.                  
────────┼────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────
Foundry │Use the [Standard agent deployment mode][20].           │Provides more recovery capabilities than Basic mode,  
Agent   │                                                        │which has almost no recovery options for resource     
Service │                                                        │loss.                                                 
────────┼────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────
Azure   │Enable [Continuous backup with point-in-time            │Recover from accidental deletion of the               
Cosmos  │restore][21]. Select the 7-day or 30-day retention tier │`enterprise_memory` database, its containers, or the  
DB      │based on your recovery requirements.                    │whole account.                                        
────────┼────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────
Azure   │Use a unique, organization-specific name (for example,  │During point-in-time restore, Cosmos DB creates a new 
Cosmos  │`contoso-agents-cosmosdb`).                             │account with the original name. If that name is       
DB      │                                                        │already taken, the restore fails.                     
────────┼────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────
Azure   │Enable read replication to your designated failover     │Enables the Cosmos DB service to switch the write     
Cosmos  │region, and enable [Service-Managed Failover][22].      │region from the primary region to the secondary region
DB      │                                                        │during a prolonged regional outage.                   
────────┼────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────
Azure AI│Use a unique, organization-specific name (for example,  │During restoration, a new service is created with the 
Search  │`contoso-agents-search`).                               │original name. If that name is already taken, the     
        │                                                        │restore fails.                                        
────────┼────────────────────────────────────────────────────────┼──────────────────────────────────────────────────────
Azure   │Use geo-zone-redundant storage (GZRS). Your workload's  │Allows customer-managed failover to be initiated to   
Storage │recovery region can be the secondary region for this    │the predetermined region.                             
account │Storage account, but it's not required.                 │                                                      
────────┴────────────────────────────────────────────────────────┴──────────────────────────────────────────────────────

**References:**
* [Azure Cosmos DB high availability][23]
* [Azure AI Search service reliability][24]
* [Azure Storage redundancy][25]

### Deployment modes and recovery implications

In [Standard deployment mode][26], you host agent state in your own Azure Cosmos DB, Azure AI Search, and Azure Storage
accounts. This topology increases incident risk (for example, direct data deletion) but gives you control over recovery
procedures. Basic mode provides almost no recovery capabilities for human or automation-based resource loss.

Tip

Agent Service has no availability or state durability Service Level Agreement (SLA). Standard mode offloads SLAs and
data durability assurances to the underlying storage components.

### Use user-assigned managed identities

When a component uses a managed identity to access a dependency, grant that identity the required role assignments. With
a system-assigned managed identity, recreating the faulted resource generates a new principal ID. You must then reapply
all role assignments on every dependency and delete orphaned assignments. Some dependencies might be owned by other
teams, which adds coordination and delay during recovery.

A user-assigned managed identity avoids this effort. After you restore the faulted resource, reattach the existing
user-assigned managed identity. Existing role assignments remain valid.

Important

Avoid treating a single user-assigned managed identity as a universal identity for multiple unrelated uses.

For example, assign a dedicated user-assigned managed identity per project. Even if two projects have identical role
assignments today, treat that situation as temporary. Future divergence can grant unnecessary permissions to one project
if they share an identity, violating least privilege. Separate identities also let dependency logs distinguish activity
per project.

### Use repeatable deployment techniques

Define the account, projects, capability host, and dependencies in infrastructure as code (IaC) such as Bicep or
Terraform. Some recovery steps require redeploying resources exactly as they were. Treat IaC as the source of truth to
reproduce configuration and role assignments quickly. Build your IaC modular so that you can independently deploy each
project.

Make agents redeployable. For ephemeral agents, existing application code is usually sufficient. For long‑lived agents,
store their JSON definitions and knowledge or tool bindings in source control and automate deployment via pipeline calls
to the Foundry APIs. Automatically update client configuration for new agent IDs. This process rehydrates agent
definitions, knowledge files, and tool connections.

Avoid untracked changes made directly in the Foundry portal or Azure portal. Untracked production changes make recovery
slower and error-prone.

If you still choose system-assigned identities (contrary to the recommendation to [use user-assigned managed
identities][27]), design IaC to recreate, not mutate, each role assignment that references the project's principal ID.
The principal ID on role assignments is immutable and can't be updated to a new value. Use a `guid()` expression that
incorporates the principal ID so a regenerated identity produces a distinct role assignment name.

The following Bicep snippet shows a minimal template for Cosmos DB with continuous backup and a resource lock. Extend
this template with Azure AI Search, Azure Storage (GZRS), and role assignments for your managed identity.

`param location string = resourceGroup().location
param cosmosDbName string

resource cosmosDb 'Microsoft.DocumentDB/databaseAccounts@2024-05-15' = {
  name: cosmosDbName
  location: location
  properties: {
    databaseAccountOfferType: 'Standard'
    locations: [
      { locationName: location, failoverPriority: 0, isZoneRedundant: true }
    ]
    backupPolicy: {
      type: 'Continuous'
      continuousModeProperties: { tier: 'Continuous7Days' }
    }
  }
}

resource lock 'Microsoft.Authorization/locks@2020-05-01' = {
  name: '${cosmosDbName}-lock'
  scope: cosmosDb
  properties: {
    level: 'CanNotDelete'
    notes: 'Protect Cosmos DB from accidental deletion.'
  }
}
`

### Minimize treating Azure AI Search as a primary data store

Azure AI Search is designed to hold a derived, query‑optimized projection of authoritative content that you store
elsewhere. Don't rely on it as the only location of knowledge assets. During recovery you must be able to recreate
agents that reference file-based knowledge in either the production or recovery environment.

User‑uploaded files attached within conversation threads generally can't be recovered because they're not registered or
persisted outside the thread context. Set expectations that these attachments are transient and are lost in a disaster.

## Back up and restore agent data

Conversation thread history durability depends on the underlying Standard mode state stores: Cosmos DB
`enterprise_memory` database, Azure AI Search indexes, and Storage blobs for attachments. There's no built-in one-click
export or import feature for complete conversation histories.

### Back up agent definitions

Store agent JSON definitions and knowledge source references in source control. Use the Foundry REST API to periodically
export agent configurations:
1. List your agents by calling the [Agents API][28] to retrieve agent IDs, names, tool bindings, and knowledge source
   configurations.
2. Save each agent definition as a JSON file in your version control system.
3. Include tool bindings, knowledge file references, and connection configurations alongside each agent definition.
4. Automate this process in a CI/CD pipeline on a regular schedule (for example, daily or after each deployment).

Tip

You can automate agent export by using the [Azure AI Projects SDK][29] for Python or the [REST API][30]. The SDK
provides methods to list agents, retrieve their configurations, and serialize them to JSON for version control.

### Restore from Cosmos DB point-in-time backup

If the `enterprise_memory` database or its containers are accidentally deleted:
1. Open the [Azure portal][31] and navigate to your Cosmos DB account.
2. Select **Point in time restore** and choose a restore timestamp before the deletion occurred.
3. Specify a new target account name for the restored data.
4. After the restore completes, update the Foundry Agent Service connection to point to the restored Cosmos DB account.
5. Verify agent functionality by running a test conversation in the restored environment.

You can also initiate a restore by using the Azure CLI:

`az cosmosdb restore \
  --account-name <source-account-name> \
  --target-database-account-name <restored-account-name> \
  --restore-timestamp "2026-01-15T10:00:00Z" \
  --location <region> \
  --resource-group <resource-group>
`

Note

Cosmos DB point-in-time restore creates a new account. You must update the Agent Service connection string and reapply
role assignments if you use system-assigned managed identities. User-assigned managed identities reduce this overhead.

### Preserve compliance data

Connect to Microsoft Purview to preserve lineage and classification metadata even if operational thread data is lost.
This ensures eDiscovery and audit capabilities survive a disaster.

### Rebuild Azure AI Search indexes

If your Azure AI Search service is lost or corrupted, rebuild indexes from your authoritative data sources:
1. Create a new Azure AI Search service in the recovery region, or use the secondary service you provisioned in your
   multiregional deployment.
2. Recreate index definitions from your IaC templates or source-controlled schema files.
3. Repopulate indexes by running your data ingestion pipeline against the original data sources (for example, Azure Blob
   Storage, Azure SQL Database, or Cosmos DB).
4. Update agent knowledge source references to point to the new search service endpoint.
5. Verify index completeness by running representative search queries and comparing results against known baselines.

## Plan for multiregional deployment

A multiregional deployment relies on creating Foundry resources and other infrastructure in two Azure regions. If a
regional outage occurs, switch to the other region. When you plan where to deploy your resources, consider:
* Regional availability: If possible, use a region in the same geographic area, not necessarily the closest one. To
  check regional availability for Foundry, see [Azure products by region][32].
* Azure paired regions: Paired regions coordinate platform updates and prioritize recovery efforts where needed.
  However, not all regions are paired. For more information, see [Azure paired regions][33].
* Service availability: Decide whether to use hot/hot, hot/warm, or hot/cold for your solution's resources.
  * Hot/hot: Both regions are active at the same time, and either region is ready to use immediately.
  * Hot/warm: The primary region is active. The secondary region has critical resources (for example, deployed models)
    ready to start. Deploy noncritical resources manually in the secondary region.
  * Hot/cold: The primary region is active. The secondary region has Foundry and other resources deployed, along with
    the required data. Deploy resources such as models, model deployments, and pipelines manually.

The following table shows approximate recovery targets for each strategy. Actual values depend on your deployment size,
services in use, and data replication configuration.

────┬───────────┬──────────────────────────┬────────────────────────────────┬───────────────────────────────────────────
Stra│Approximate│Approximate RPO           │Relative cost                   │Best for                                   
tegy│RTO        │                          │                                │                                           
────┼───────────┼──────────────────────────┼────────────────────────────────┼───────────────────────────────────────────
Hot/│Minutes    │Near zero                 │Highest: full duplicate         │Production workloads with zero-downtime    
hot │           │                          │resources running in both       │requirements                               
    │           │                          │regions                         │                                           
────┼───────────┼──────────────────────────┼────────────────────────────────┼───────────────────────────────────────────
Hot/│30 minutes │Minutes to hours,         │Moderate: critical resources    │Business-critical workloads that can       
warm│to 2 hours │depending on replication  │running, others on standby      │tolerate brief disruption                  
    │           │lag                       │                                │                                           
────┼───────────┼──────────────────────────┼────────────────────────────────┼───────────────────────────────────────────
Hot/│2 to 8     │Hours, depending on backup│Lowest: resources provisioned   │Development, staging, or cost-sensitive    
cold│hours      │frequency                 │but not active                  │workloads with relaxed recovery targets    
────┴───────────┴──────────────────────────┴────────────────────────────────┴───────────────────────────────────────────

Tip

Depending on your business requirements, you might treat Foundry services differently.

Foundry builds on other services. Some services replicate to other regions. You must manually create other services in
multiple regions. The following table lists the services, who is responsible for replication, and an overview of the
configuration:

──────┬────┬────────────────────────────────────────────────────────────────────────────────────────────────────────────
Azure │Geo-│Configuration                                                                                               
servic│repl│                                                                                                            
e     │icat│                                                                                                            
      │ed  │                                                                                                            
      │by  │                                                                                                            
──────┼────┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
Foundr│You │Create projects in the selected regions.                                                                    
y     │    │                                                                                                            
projec│    │                                                                                                            
ts    │    │                                                                                                            
──────┼────┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
Key   │Micr│Use the same Azure Key Vault instance with the Foundry project and resources in both regions. Azure Key     
Vault │osof│Vault automatically fails over to a secondary region. For more information, see [Azure Key Vault            
      │t   │availability and redundancy][34].                                                                           
──────┼────┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
Storag│You │Foundry projects don't support default storage account failover using geo-redundant storage (GRS),          
e     │    │geo-zone-redundant storage (GZRS), read-access geo-redundant storage (RA-GRS), or read-access               
accoun│    │geo-zone-redundant storage (RA-GZRS). Configure a storage account according to your needs, and use it for   
t     │    │your project. All subsequent projects use the project's storage account. For more information, see [Azure   
      │    │Storage redundancy][35].                                                                                    
──────┼────┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
Azure │You │Enable geo-replication on your Azure Container Registry instance to the paired region. Use the same instance
Contai│    │for both projects. For more information, see [Geo-replication in Azure Container Registry][36].             
ner   │    │                                                                                                            
Regist│    │                                                                                                            
ry    │    │                                                                                                            
──────┼────┼────────────────────────────────────────────────────────────────────────────────────────────────────────────
Applic│You │Create Application Insights for the project in both regions. To adjust the data retention period and        
ation │    │details, see [Data collection, retention, and storage in Application Insights][37].                         
Insigh│    │                                                                                                            
ts    │    │                                                                                                            
──────┴────┴────────────────────────────────────────────────────────────────────────────────────────────────────────────

Use these development practices to enable fast recovery and restart in the secondary region:
1. Use Azure Resource Manager templates. Templates are infrastructure as code, and they let you quickly deploy services
   in both regions.
2. To avoid drift between the two regions, update your continuous integration and deployment pipelines to deploy to both
   regions.
3. Create role assignments for users in both regions.
4. Create network resources such as Azure virtual networks and private endpoints for both regions. Ensure users can
   access both network environments. For example, configure VPN and DNS for both virtual networks.

## Design for high availability

### Configure availability zones

Some Azure services support availability zones. In regions that support availability zones, if a single zone fails,
services configured for zone redundancy continue to operate. Services that aren't zone-redundant might experience
interruptions. The Microsoft-hosted components of the Agent Service are zone redundant. Verify that your
customer-managed dependencies (Cosmos DB, AI Search, Storage) are also configured for zone redundancy.

Learn more in [Availability zone service support][38].

### Deploy critical components to multiple regions

Decide what level of business continuity you need. The level can differ between components of your solution. For
example, you might use a hot/hot configuration for production pipelines or model deployments, and hot/cold for
development.

Foundry is a regional service that stores data on the service side and in a storage account in your subscription. If a
regional disaster occurs, you can't recover service data. You can recover data that the service stores in the storage
account in your subscription if storage redundancy is enabled. Service-side data is mostly metadata like tags, asset
names, and descriptions. Data in your storage account typically isn't metadata, like uploaded data.

For connections that are essential to business continuity:
1. Create two separate resources in two different regions (for example, two AI Services resources).
2. Create two project connections, one for each regional resource.
3. Verify both connections are active and accessible from your application.
4. Deploy resources for any business-critical projects in both regions.

### Isolate storage for large datasets

If you connect data to customize your AI application, you can use datasets in Azure AI and outside Azure AI. Dataset
volume can be large, so keep this data in a separate storage account to limit blast radius and simplify replication.
1. Create a dedicated storage account for your large datasets, separate from the project's primary storage.
2. Evaluate the data replication strategy (LRS, GRS, GZRS) that makes the most sense for your recovery requirements.
3. In the Foundry portal, create a connection to your data storage account. If you have multiple Foundry instances in
   different regions, you can point to the same storage account. Connections work across regions.

### Monitor for early outage detection

Configure monitoring and alerting so your team detects regional degradation before it affects production workloads:
1. Enable [Azure Service Health alerts][39] for all Azure services in your Foundry workload. Service Health provides
   advance notice of planned maintenance and early warning of unplanned outages.
2. Configure [resource health alerts][40] for your Cosmos DB, Azure OpenAI, and Storage accounts to detect individual
   resource failures.
3. Set up Application Insights [availability tests][41] to probe your agent endpoints continuously from multiple
   geographic locations.
4. Define alert action groups that notify your operations team through email, SMS, or your incident management system so
   failover decisions can be made quickly.

## Configure model deployment resiliency

Azure OpenAI model deployments are a critical component of most Foundry workloads. Design your deployment topology for
resiliency so that a regional outage or capacity constraint doesn't take your application offline. This section covers
Standard and Provisioned deployment strategies, API gateway patterns, and the supporting infrastructure that ties them
together.

### Configure Standard deployments

Standard deployments offer the simplest path to resiliency because Data Zone and Global Standard options distribute
requests across multiple regions automatically.

Note

If your data-residency requirements allow it, prefer Global Standard deployments. Data Zone deployments (US/EU) are the
next best option for organizations that require data processing within a geographic boundary.

Use the following approach for Standard deployments:
1. Default to Data Zone deployments (US or EU options).
2. Deploy two Azure OpenAI resources in the same Azure subscription. Place one resource in your preferred region and the
   other in your secondary (failover) region. Azure OpenAI allocates quota at the subscription-plus-region level, so
   both resources can share a subscription without affecting quota.
3. Create one deployment for each model you plan to use in the primary region, and duplicate those model deployments in
   the secondary region. Allocate the full available quota in each Standard deployment. Full allocation provides higher
   throughput compared to splitting quota across multiple deployments.
4. Select the deployment region based on your network topology. You can deploy an Azure OpenAI resource to any supported
   region and then create a private endpoint for that resource in a region closer to your application.
   * After traffic enters the Azure OpenAI boundary, the service optimizes routing and processing across available
     compute in the data zone.
   * Data Zone routing is more efficient and simpler than self-managed load balancing across multiple regional
     deployments.
5. If a regional outage makes the primary deployment unreachable, route traffic to the secondary deployment in the
   passive region within the same subscription.
   * Because both primary and secondary are Zone deployments, they draw from the same Zone capacity pool across all
     available regions in the Zone. The secondary deployment protects against the primary Azure OpenAI endpoint being
     unreachable.
   * Use a Generative AI Gateway that supports load balancing and the circuit-breaker pattern, such as Azure API
     Management, in front of the Azure OpenAI endpoints to minimize disruption during a regional outage.
   * If the quota in a given subscription is exhausted, deploy a new subscription in the same manner and place its
     endpoint behind the Generative AI Gateway.

### Configure Provisioned deployments

Provisioned Throughput Unit (PTU) deployments guarantee dedicated capacity for latency-sensitive or mission-critical
workloads. Combine an enterprise PTU pool with optional workload-specific deployments for maximum flexibility and
resiliency.

#### Create an enterprise PTU pool
1. For provisioned deployments, create a single Data Zone PTU deployment that serves as an enterprise pool of PTU. Use
   Azure API Management to manage traffic from multiple applications and to set throughput limits, logging, priority,
   and failover logic.
   * Think of the enterprise PTU pool as a "private Standard deployment" that protects against the noisy-neighbor
     problem. When demand is high on Standard deployments, your organization has guaranteed, dedicated access to a
     capacity pool that only you can use.
   * This approach gives you control over which applications experience increased latency first, allowing you to
     prioritize traffic to 

[Content truncated]
```
