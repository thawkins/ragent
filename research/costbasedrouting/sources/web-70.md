# Web source

- URL: https://learn.microsoft.com/en-us/azure/foundry/openai/quotas-limits
- Title: [ Skip to main content ][1] [ Skip to Ask Learn chat experience ][2]
- Captured (UTC): 2026-06-29T15:43:53.619737037+00:00

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

# Azure OpenAI in Microsoft Foundry Models quotas and limits

Feedback
Summarize this article for me

## In this article

This article contains a quick reference and a detailed description of the quotas and limits for Azure OpenAI.

## Scope of quota

Quotas and limits aren't enforced at the tenant level. Instead, the highest level of quota restrictions is scoped at the
Azure subscription level.

## Regional quota allocation

Tokens per minute (TPM) and requests per minute (RPM) limits are defined *per region*, *per subscription*, and *per
model or deployment type*.

For example, if the `gpt-4.1` Global Standard model is listed with a quota of *5 million TPM* and *5,000 RPM*, then
*each region* where that [model or deployment type is available][8] has its own dedicated quota pool of that amount for
*each* of your Azure subscriptions. Within a single Azure subscription, it's possible to use a larger quantity of total
TPM and RPM quota for a given model and deployment type, as long as you have resources and model deployments spread
across multiple regions.

## Quota tiers

We are introducing Quota Tiers to improve the Foundry Models experience and reduce friction as workloads scale. Quotas
will now increase automatically with usage, helping avoid rate limit errors while also creating a fairer environment for
all users. Seven tiers will be made available: Free Tier and Tiers 1 through 6 - with Tier 6 offering the highest
quotas. A customer’s initial assigned tier is based on their current usage of that model and their current relationship
with Microsoft, such as Enterprise Agreement (EA or MCA-E) status.

### What’s changing for me?

Previously, Foundry offered only Default and Enterprise quota levels for pay as you go offer type, with a large gap
between each level and a longer process to request increases. With Quota Tiers, all users are assigned a tier with
quotas equal to or higher than their previous levels. Any previously approved quota increases are retained and will not
be reduced. As usage grows, Foundry automatically increases quotas by moving users to higher tiers, and additional quota
can still be requested through the quota form.

### How will a customer automatically move from one tier to another, for example what are the tier change criteria?

Automatic tier upgrades are based primarily on customer consumption trends across Foundry Models over time. If a
customer’s usage increases such that their current quota tier is limiting their ability to use Foundry Models the system
will automatically upgrade the customer to the next higher tier. A customer’s relationship with Microsoft is also taken
into account. Customers with Enterprise relationships (including EA and MCA-E) with Microsoft are assigned higher quota
tiers. In addition, Microsoft will also consider a customer's payment history to determine eligibility for automatic
upgrades.

### Can I opt out of auto upgrades?

Yes, you can opt out of auto upgrades and you'll remain in your current tier regardless of changes in your consumption.
We recognize that some of our customers use quota to manage their billing. This isn't the Azure best practice, however,
we understand that if your system is configured that way we don’t want to break it. You can learn more about billing
management and best practices here: [Cost Management][9].

To opt out, you can set the following flag to `NoAutoUpgrade`:

`curl -X PATCH \
  
"https://management.azure.com/subscriptions/00000000-0000-0000-0000-000000000000/providers/Microsoft.CognitiveServices/q
uotaTiers/default?api-version=2025-10-01-preview" \
  -H "Authorization: Bearer <YOUR_ACCESS_TOKEN>" \
  -H "Content-Type: application/json" \
  -d '{
    "properties": {
      "tierUpgradePolicy": "NoAutoUpgrade"
    }
  }'
`

Note

The opt out feature is preview and may be subject to change/removal in the future.

### Can I request more quota?

Yes, using the [quota request form][10] you can always request more quota. If the request is approved, the current tier
will remain the same, but with more quota assigned.

### How do I check my subscription's quota tier?

You can currently check you quota tier with the [control plane API][11]:
* [Bash][12]
* [Python][13]
* [Output][14]

`curl -X GET \
  
"https://management.azure.com/subscriptions/00000000-0000-0000-0000-000000000000/providers/Microsoft.CognitiveServices/q
uotaTiers?api-version=2025-10-01-preview" \
  
-H "Authorization: Bearer $(az account get-access-token --resource https://management.azure.com --query accessToken -o t
sv)" \
  -H "Content-Type: application/json"
`

`import requests
import json
from azure.identity import DefaultAzureCredential


subscriptionId = "{YOUR-SUBSCRIPTION-ID}"
api_version = "2025-10-01-preview" 
base_url = "https://management.azure.com"

token_credential = DefaultAzureCredential()
token = token_credential.get_token('https://management.azure.com/.default')
headers = {
    'Authorization': 'Bearer ' + token.token,
    'Content-Type': 'application/json'
}


list_url = (
    f"{base_url}/subscriptions/{subscriptionId}"
    f"/providers/Microsoft.CognitiveServices/quotaTiers"
    f"?api-version={api_version}"
)

response = requests.get(list_url, headers=headers)
print(json.dumps(response.json(), indent=2))

`

`{
  "value": [
    {
      "properties": {
        "currentTierName": "Tier 1",
        "assignmentDate": "2025-10-18T05:09:05.6334222Z",
        "tierUpgradePolicy": "OnceUpgradeIsAvailable"
      },
      "id": "/subscriptions/aaaaa-bbbbb-ccccc-dddd-eeeeeee/providers/Microsoft.CognitiveServices/quotaTiers/default",
      "name": "default",
      "type": "Microsoft.CognitiveServices/quotaTiers"
    }
  ]
}
`

### Quota tier reference
* [Tier 1][15]
* [Tier 2][16]
* [Tier 3][17]
* [Tier 4][18]
* [Tier 5][19]
* [Tier 6][20]
* [Tier 0][21]

### Tier 1

────────────────────────────┬────────────────┬─────────────────────────┬───────────────────────
Model Name                  │Deployment Type │Requests Per Minute (RPM)│Tokens Per Minute (TPM)
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
codex-mini                  │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
computer-use-preview        │GlobalStandard  │4,500                    │450,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1                     │DataZoneStandard│300                      │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1                     │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1-mini                │DataZoneStandard│2,000                    │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1-mini                │GlobalStandard  │5,000                    │5,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1-mini                │Standard        │6,000                    │6,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1-nano                │DataZoneStandard│2,000                    │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1-nano                │GlobalStandard  │5,000                    │5,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o                      │DataZoneStandard│300 / 10s                │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-audio-preview        │GlobalStandard  │30000 / 10s              │30,000,000             
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-mini                 │DataZoneStandard│10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-mini                 │GlobalStandard  │20,000                   │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-mini-audio-preview   │GlobalStandard  │30000 / 10s              │30,000,000             
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-mini-realtime-preview│GlobalStandard  │36                       │6,000                  
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-realtime-preview     │GlobalStandard  │36                       │6,000                  
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5                       │DataZoneStandard│3,000                    │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5                       │GlobalStandard  │10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-chat                  │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-codex                 │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-mini                  │DataZoneStandard│300                      │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-mini                  │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-nano                  │DataZoneStandard│2,000                    │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-nano                  │GlobalStandard  │5,000                    │5,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-pro                   │GlobalStandard  │1,600                    │160,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1                     │DataZoneStandard│3,000                    │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1                     │GlobalStandard  │10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1                     │Standard        │3,000                    │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1-chat                │GlobalStandard  │10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1-codex               │DataZoneStandard│3,000                    │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1-codex               │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1-codex-max           │GlobalStandard  │10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1-codex-mini          │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.2                     │DataZoneStandard│3,000                    │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.2                     │GlobalStandard  │10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.2-chat                │GlobalStandard  │10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.3-chat                │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.2-codex               │GlobalStandard  │10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.3-codex               │GlobalStandard  │10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.4                     │DataZoneStandard│300                      │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.4                     │GlobalStandard  │10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.4-pro                 │GlobalStandard  │160                      │160,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.4-mini                │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.4-nano                │DataZoneStandard│2,000                    │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.4-nano                │GlobalStandard  │5,000                    │5,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.5                     │DataZoneStandard│0                        │0                      
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.5                     │GlobalStandard  │0                        │0                      
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-chat-latest             │GlobalStandard  │10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-audio                   │GlobalStandard  │30000 / 10s              │30,000,000             
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-image-1                 │GlobalStandard  │9                        │-                      
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-image-1-mini            │GlobalStandard  │12                       │-                      
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-image-1.5               │DataZoneStandard│3                        │-                      
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-image-1.5               │GlobalStandard  │9                        │-                      
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-image-2                 │DataZoneStandard│2                        │-                      
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-image-2                 │GlobalStandard  │6                        │-                      
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-realtime                │GlobalStandard  │200                      │100,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
model-router                │DataZoneStandard│300                      │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
model-router                │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
o1                          │DataZoneStandard│100                      │600,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
o1                          │GlobalStandard  │500                      │3,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
o3                          │DataZoneStandard│300                      │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
o3                          │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
o3-deep-research            │GlobalStandard  │3,000                    │3,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
o3-mini                     │DataZoneStandard│200                      │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
o3-mini                     │GlobalStandard  │500                      │5,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
o3-pro                      │GlobalStandard  │160                      │1,600,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
o4-mini                     │DataZoneStandard│300 / 10s                │300,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
o4-mini                     │GlobalStandard  │1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
text-embedding-3-large      │DataZoneStandard│1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
text-embedding-3-large      │GlobalStandard  │1000 / 10s               │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
text-embedding-3-small      │DataZoneStandard│1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
text-embedding-3-small      │GlobalStandard  │1000 / 10s               │1,000,000              
────────────────────────────┴────────────────┴─────────────────────────┴───────────────────────

### Tier 2

────────────────────────────┬────────────────┬─────────────────────────┬───────────────────────
Model Name                  │Deployment Type │Requests Per Minute (RPM)│Tokens Per Minute (TPM)
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
codex-mini                  │GlobalStandard  │2,000                    │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
computer-use-preview        │GlobalStandard  │20,000                   │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1                     │DataZoneStandard│1,000                    │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1                     │GlobalStandard  │3,000                    │3,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1-mini                │DataZoneStandard│6,000                    │6,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1-mini                │GlobalStandard  │16,000                   │16,000,000             
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1-mini                │Standard        │12,000                   │12,000,000             
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1-nano                │DataZoneStandard│6,000                    │6,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4.1-nano                │GlobalStandard  │16,000                   │16,000,000             
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o                      │DataZoneStandard│1000 / 10s               │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-audio-preview        │GlobalStandard  │30000 / 10s              │30,000,000             
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-mini                 │DataZoneStandard│30,000                   │3,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-mini                 │GlobalStandard  │90,000                   │9,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-mini-audio-preview   │GlobalStandard  │30000 / 10s              │30,000,000             
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-mini-realtime-preview│GlobalStandard  │36                       │6,000                  
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-4o-realtime-preview     │GlobalStandard  │36                       │6,000                  
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5                       │DataZoneStandard│10,000                   │1,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5                       │GlobalStandard  │30,000                   │3,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-chat                  │GlobalStandard  │2,000                    │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-codex                 │GlobalStandard  │2,000                    │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-mini                  │DataZoneStandard│670                      │670,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-mini                  │GlobalStandard  │2,000                    │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-nano                  │DataZoneStandard│6,000                    │6,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-nano                  │GlobalStandard  │16,000                   │16,000,000             
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5-pro                   │GlobalStandard  │3,500                    │350,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1                     │DataZoneStandard│6,700                    │670,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1                     │GlobalStandard  │20,000                   │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1                     │Standard        │6,700                    │670,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1-chat                │GlobalStandard  │20,000                   │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1-codex               │DataZoneStandard│6,700                    │670,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1-codex               │GlobalStandard  │2,000                    │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1-codex-max           │GlobalStandard  │20,000                   │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.1-codex-mini          │GlobalStandard  │2,000                    │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.2                     │DataZoneStandard│6,700                    │670,000                
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.2                     │GlobalStandard  │20,000                   │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.2-chat                │GlobalStandard  │20,000                   │2,000,000              
────────────────────────────┼────────────────┼─────────────────────────┼───────────────────────
gpt-5.3-chat                │GlobalStandard  │2,000                    │2,000,000              
────────────────────────────┼────────────────

[Content truncated]
```
