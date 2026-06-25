# Web source

- URL: https://docs.swarms.ai/docs/documentation/multi-agent/agent_rearrange
- Title: > ## Documentation Index
- Captured (UTC): 2026-06-25T05:52:46.009340815+00:00

```text
> ## Documentation Index
> 
> Fetch the complete documentation index at: [/llms.txt][1]
> 
> Use this file to discover all available pages before exploring further.

[Skip to main content][2]
[Swarms API Documentation home page[light logo][dark logo]][3]
Search...
⌘K
* [Get API Key][4]
* [Account Management][5]
* [Support][6]
* [Status][7]
Search...
Navigation
[Introduction][8][Swarms API][9][Marketplace][10][API Reference][11][Examples][12][Changelog][13]
* [
  Get Your API Key][14]
* [
  Account Management][15]
* [
  Discord][16]
* [
  Technical Support][17]

### Getting Started
* [
  Welcome
  ][18]
* [
  FAQ
  ][19]
* [
  Quickstart
  ][20]
* [
  API Key Setup
  ][21]
* [
  Setup & Configuration
  ][22]
* [
  API Architecture
  ][23]

### Clients
* [
  Official Client Libraries
  ][24]
* [
  Python Client
  ][25]
* [
  Swarms API MCP Server
  ][26]
* [
  Docs MCP and LLMs txt
  ][27]

### Agent Completions
* [
  Agent Completions Reference
  ][28]
* [
  OpenAI-Compatible Endpoint
  ][29]

### Multi-Agent
* [
  Overview
  ][30]
* [
  Available architectures
  ][31]
* [
  Sequential workflow
  ][32]
* [
  Concurrent workflow
  ][33]
* [
  Multi agent router
  ][34]
* [
  Mixture of agents
  ][35]
* [
  Group chat
  ][36]
* [
  Majority voting
  ][37]
* [
  Hierarchical swarm
  ][38]
* [
  Agent rearrange
  ][39]
* [
  Graph workflow
  ][40]
* [
  Debate with judge
  ][41]
* [
  Heavy swarm
  ][42]
* [
  Round robin
  ][43]

### Batch
* [
  Batch Agent Completions (Single Agent)
  ][44]
* [
  Batch Agent Completions at Scale
  ][45]
* [
  Batch Swarm Completions (Multi-Agent)
  ][46]
* [
  Batch Swarm Completions for Overnight Reports
  ][47]
* [
  Batched grid workflow
  ][48]

### Capabilities
* [
  Structured Outputs
  ][49]
* [
  Sub-Agent Delegation
  ][50]
* [
  MCP Integration
  ][51]
* [
  Streaming
  ][52]
* [
  Fetch Previously Created Agents
  ][53]

### Resources
* [
  Overview
  ][54]
* [
  Rate Limits
  ][55]
* [
  Rate Limit Headers
  ][56]
* [
  Pricing
  ][57]
* [
  Usage Report
  ][58]
* [
  Security
  ][59]
* [
  Response Compression
  ][60]
* [
  Premium Endpoints
  ][61]
* [
  Priority Processing
  ][62]
* [
  Global Availability
  ][63]
* [
  Community
  ][64]
* [
  Technical Support
  ][65]
* [
  Referral Program
  ][66]
* [
  Contributors
  ][67]

## On this page
* [Overview][68]
* [Use Cases][69]
* [API Usage][70]
  * [Basic AgentRearrange Example][71]
* [Best Practices][72]

Multi-Agent

# Agent rearrange

Copy page

Dynamic swarm architecture that can reorganize agent roles and responsibilities based on task requirements and
performance

Copy page
**Swarm Type**: `AgentRearrange`

## [
## ][73]
## Overview

The AgentRearrange swarm type implements a dynamic architecture where agents can be reassigned to different roles and
responsibilities based on task requirements, performance metrics, or changing circumstances. This flexibility allows the
swarm to adapt to different scenarios and optimize performance through intelligent role reallocation. Key features:
* **Dynamic Role Assignment**: Agents can switch roles based on task needs
* **Performance-Based Reorganization**: Roles adjusted based on agent performance
* **Adaptive Architecture**: Swarm structure evolves with changing requirements
* **Flexible Resource Allocation**: Optimal use of agent capabilities

## [
## ][74]
## Use Cases
* Dynamic project management with changing requirements
* Adaptive content creation workflows
* Performance optimization in multi-agent systems
* Flexible task allocation based on agent strengths

## [
## ][75]
## API Usage

### [
### ][76]
### Basic AgentRearrange Example
* Shell (curl)
* Python (requests)
* JavaScript (fetch)
* Go
* Rust

`curl -X POST "https://api.swarms.world/v1/swarm/completions" \
  -H "x-api-key: $SWARMS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Adaptive Content Creation",
    "description": "Dynamic content creation with flexible agent role assignment",
    "swarm_type": "AgentRearrange",
    
"task": "Create a comprehensive technical blog post about machine learning in finance, with the ability to reassign agen
t roles based on content needs",
    "agents": [
      {
        "agent_name": "Research Specialist",
        "description": "Conducts research and gathers information",
        
"system_prompt": "You are a research specialist. Gather comprehensive information on machine learning applications in fi
nance, including current trends, use cases, and future prospects.",
        "model_name": "gpt-4.1",
        "max_loops": 1,
        "temperature": 0.3
      },
      {
        "agent_name": "Technical Writer",
        "description": "Creates technical content and explanations",
        
"system_prompt": "You are a technical writer specializing in machine learning and finance. Create clear, engaging techni
cal content that explains complex concepts in accessible terms.",
        "model_name": "gpt-4.1",
        "max_loops": 1,
        "temperature": 0.5
      },
      {
        "agent_name": "Finance Expert",
        "description": "Provides financial domain expertise",
        
"system_prompt": "You are a finance expert with knowledge of machine learning applications. Ensure accuracy in financial
 concepts, market analysis, and industry insights.",
        "model_name": "gpt-4.1",
        "max_loops": 1,
        "temperature": 0.3
      },
      {
        "agent_name": "Editor",
        "description": "Reviews and polishes content",
        
"system_prompt": "You are a professional editor. Review content for clarity, flow, accuracy, and overall quality. Make i
mprovements while maintaining technical accuracy.",
        "model_name": "gpt-4.1",
        "max_loops": 1,
        "temperature": 0.4
      }
    ],
    "max_loops": 1
  }'
`

`import requests
import json

API_BASE_URL = "https://api.swarms.world"
API_KEY = "your_api_key_here"

headers = {
    "x-api-key": API_KEY,
    "Content-Type": "application/json"
}

swarm_config = {
    "name": "Adaptive Content Creation",
    "description": "Dynamic content creation with flexible agent role assignment",
    "swarm_type": "AgentRearrange",
    
"task": "Create a comprehensive technical blog post about machine learning in finance, with the ability to reassign agen
t roles based on content needs",
    "agents": [
        {
            "agent_name": "Research Specialist",
            "description": "Conducts research and gathers information",
            
"system_prompt": "You are a research specialist. Gather comprehensive information on machine learning applications in fi
nance, including current trends, use cases, and future prospects.",
            "model_name": "gpt-4.1",
            "max_loops": 1,
            "temperature": 0.3
        },
        {
            "agent_name": "Technical Writer",
            "description": "Creates technical content and explanations",
            
"system_prompt": "You are a technical writer specializing in machine learning and finance. Create clear, engaging techni
cal content that explains complex concepts in accessible terms.",
            "model_name": "gpt-4.1",
            "max_loops": 1,
            "temperature": 0.5
        },
        {
            "agent_name": "Finance Expert",
            "description": "Provides financial domain expertise",
            
"system_prompt": "You are a finance expert with knowledge of machine learning applications. Ensure accuracy in financial
 concepts, market analysis, and industry insights.",
            "model_name": "gpt-4.1",
            "max_loops": 1,
            "temperature": 0.3
        },
        {
            "agent_name": "Editor",
            "description": "Reviews and polishes content",
            
"system_prompt": "You are a professional editor. Review content for clarity, flow, accuracy, and overall quality. Make i
mprovements while maintaining technical accuracy.",
            "model_name": "gpt-4.1",
            "max_loops": 1,
            "temperature": 0.4
        }
    ],
    "max_loops": 1
}

response = requests.post(
    f"{API_BASE_URL}/v1/swarm/completions",
    headers=headers,
    json=swarm_config
)

if response.status_code == 200:
    result = response.json()
    print("AgentRearrange swarm completed successfully!")
    print(f"Cost: ${result['metadata']['billing_info']['total_cost']}")
    print(f"Execution time: {result['metadata']['execution_time_seconds']} seconds")
    print(f"Dynamic results: {result['output']}")
else:
    print(f"Error: {response.status_code} - {response.text}")
`

`const API_BASE_URL = "https://api.swarms.world";
const API_KEY = "your_api_key_here";

const headers = {
    "x-api-key": API_KEY,
    "Content-Type": "application/json"
};

const swarmConfig = {
    name: "Adaptive Content Creation",
    description: "Dynamic content creation with flexible agent role assignment",
    swarm_type: "AgentRearrange",
    
task: "Create a comprehensive technical blog post about machine learning in finance, with the ability to reassign agent 
roles based on content needs",
    agents: [
        {
            agent_name: "Research Specialist",
            description: "Conducts research and gathers information",
            
system_prompt: "You are a research specialist. Gather comprehensive information on machine learning applications in fina
nce, including current trends, use cases, and future prospects.",
            model_name: "gpt-4.1",
            max_loops: 1,
            temperature: 0.3
        },
        {
            agent_name: "Technical Writer",
            description: "Creates technical content and explanations",
            
system_prompt: "You are a technical writer specializing in machine learning and finance. Create clear, engaging technica
l content that explains complex concepts in accessible terms.",
            model_name: "gpt-4.1",
            max_loops: 1,
            temperature: 0.5
        },
        {
            agent_name: "Finance Expert",
            description: "Provides financial domain expertise",
            
system_prompt: "You are a finance expert with knowledge of machine learning applications. Ensure accuracy in financial c
oncepts, market analysis, and industry insights.",
            model_name: "gpt-4.1",
            max_loops: 1,
            temperature: 0.3
        },
        {
            agent_name: "Editor",
            description: "Reviews and polishes content",
            
system_prompt: "You are a professional editor. Review content for clarity, flow, accuracy, and overall quality. Make imp
rovements while maintaining technical accuracy.",
            model_name: "gpt-4.1",
            max_loops: 1,
            temperature: 0.4
        }
    ],
    max_loops: 1
};

fetch(`${API_BASE_URL}/v1/swarm/completions`, {
    method: "POST",
    headers: headers,
    body: JSON.stringify(swarmConfig)
})
.then(response => response.json())
.then(result => {
    if (result.status === "success") {
        console.log("AgentRearrange swarm completed successfully!");
        console.log(`Cost: $${result.metadata.billing_info.total_cost}`);
        console.log(`Execution time: ${result.metadata.execution_time_seconds} seconds`);
        console.log("Dynamic results:", result.output);
    }
})
.catch(error => console.error("Error:", error));
`

`package main

import (
    "bytes"
    "encoding/json"
    "fmt"
    "io/ioutil"
    "net/http"
)

type Agent struct {
    AgentName    string  `json:"agent_name"`
    Description  string  `json:"description"`
    SystemPrompt string  `json:"system_prompt"`
    ModelName    string  `json:"model_name"`
    MaxLoops     int     `json:"max_loops"`
    Temperature  float64 `json:"temperature"`
}

type SwarmConfig struct {
    Name        string   `json:"name"`
    Description string   `json:"description"`
    SwarmType   string   `json:"swarm_type"`
    Task        string   `json:"task"`
    Agents      []Agent  `json:"agents"`
    MaxLoops    int      `json:"max_loops"`
}

func main() {
    API_BASE_URL := "https://api.swarms.world"
    API_KEY := "your_api_key_here"

    swarmConfig := SwarmConfig{
        Name:        "Adaptive Content Creation",
        Description: "Dynamic content creation with flexible agent role assignment",
        SwarmType:   "AgentRearrange",
        
Task:        "Create a comprehensive technical blog post about machine learning in finance, with the ability to reassign
 agent roles based on content needs",
        Agents: []Agent{
            {
                AgentName:    "Research Specialist",
                Description:  "Conducts research and gathers information",
                
SystemPrompt: "You are a research specialist. Gather comprehensive information on machine learning applications in finan
ce, including current trends, use cases, and future prospects.",
                ModelName:    "gpt-4.1",
                MaxLoops:     1,
                Temperature:  0.3,
            },
            {
                AgentName:    "Technical Writer",
                Description:  "Creates technical content and explanations",
                
SystemPrompt: "You are a technical writer specializing in machine learning and finance. Create clear, engaging technical
 content that explains complex concepts in accessible terms.",
                ModelName:    "gpt-4.1",
                MaxLoops:     1,
                Temperature:  0.5,
            },
            {
                AgentName:    "Finance Expert",
                Description:  "Provides financial domain expertise",
                
SystemPrompt: "You are a finance expert with knowledge of machine learning applications. Ensure accuracy in financial co
ncepts, market analysis, and industry insights.",
                ModelName:    "gpt-4.1",
                MaxLoops:     1,
                Temperature:  0.3,
            },
            {
                AgentName:    "Editor",
                Description:  "Reviews and polishes content",
                
SystemPrompt: "You are a professional editor. Review content for clarity, flow, accuracy, and overall quality. Make impr
ovements while maintaining technical accuracy.",
                ModelName:    "gpt-4.1",
                MaxLoops:     1,
                Temperature:  0.4,
            },
        },
        MaxLoops: 1,
    }

    jsonData, _ := json.Marshal(swarmConfig)

    req, _ := http.NewRequest("POST", API_BASE_URL+"/v1/swarm/completions", bytes.NewBuffer(jsonData))
    req.Header.Set("x-api-key", API_KEY)
    req.Header.Set("Content-Type", "application/json")

    client := &http.Client{}
    resp, err := client.Do(req)
    if err != nil {
        fmt.Printf("Error: %v\n", err)
        return
    }
    defer resp.Body.Close()

    body, _ := ioutil.ReadAll(resp.Body)
    fmt.Printf("Response: %s\n", string(body))
}
`

`use reqwest::Client;
use serde_json::{json, Value};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let api_base_url = "https://api.swarms.world";
    let api_key = "your_api_key_here";

    let swarm_config = json!({
        "name": "Adaptive Content Creation",
        "description": "Dynamic content creation with flexible agent role assignment",
        "swarm_type": "AgentRearrange",
        
"task": "Create a comprehensive technical blog post about machine learning in finance, with the ability to reassign agen
t roles based on content needs",
        "agents": [
            {
                "agent_name": "Research Specialist",
                "description": "Conducts research and gathers information",
                
"system_prompt": "You are a research specialist. Gather comprehensive information on machine learning applications in fi
nance, including current trends, use cases, and future prospects.",
                "model_name": "gpt-4.1",
                "max_loops": 1,
                "temperature": 0.3
            },
            {
                "agent_name": "Technical Writer",
                "description": "Creates technical content and explanations",
                
"system_prompt": "You are a technical writer specializing in machine learning and finance. Create clear, engaging techni
cal content that explains complex concepts in accessible terms.",
                "model_name": "gpt-4.1",
                "max_loops": 1,
                "temperature": 0.5
            },
            {
                "agent_name": "Finance Expert",
                "description": "Provides financial domain expertise",
                
"system_prompt": "You are a finance expert with knowledge of machine learning applications. Ensure accuracy in financial
 concepts, market analysis, and industry insights.",
                "model_name": "gpt-4.1",
                "max_loops": 1,
                "temperature": 0.3
            },
            {
                "agent_name": "Editor",
                "description": "Reviews and polishes content",
                
"system_prompt": "You are a professional editor. Review content for clarity, flow, accuracy, and overall quality. Make i
mprovements while maintaining technical accuracy.",
                "model_name": "gpt-4.1",
                "max_loops": 1,
                "temperature": 0.4
            }
        ],
        "max_loops": 1
    });

    let client = Client::new();
    let response = client
        .post(&format!("{}/v1/swarm/completions", api_base_url))
        .header("x-api-key", api_key)
        .header("Content-Type", "application/json")
        .json(&swarm_config)
        .send()
        .await?;

    if response.status().is_success() {
        let result: Value = response.json().await?;
        println!("AgentRearrange swarm completed successfully!");
        println!("Response: {:?}", result);
    } else {
        println!("Error: {}", response.status());
    }

    Ok(())
}
`

**Example Response**:

`{
    "job_id": "swarms-A17nZFDesmLHxCRoeyF3NVYvPaXk",
    "status": "success",
    "swarm_name": "Adaptive Content Creation",
    "description": "Dynamic content creation with flexible agent role assignment",
    "swarm_type": "AgentRearrange",
    "output": [
        {
            "role": "Research Specialist",
            
"content": "My research on machine learning in finance reveals key applications in algorithmic trading, risk assessment,
 fraud detection, and customer service automation..."
        },
        {
            "role": "Technical Writer",
            
"content": "Building on the research, I've created a comprehensive technical blog post that explains machine learning co
ncepts in finance..."
        },
        {
            "role": "Finance Expert",
            
"content": "I've reviewed the technical content to ensure financial accuracy, including proper terminology and market in
sights..."
        },
        {
            "role": "Editor",
            
"content": "I've edited the content for clarity and flow while maintaining technical accuracy and financial precision...
"
        }
    ],
    "agent_rearrangement": "Agents successfully reorganized based on content creation workflow",
    "number_of_agents": 4,
    "service_tier": "standard",
    "execution_time": 38.2,
    "usage": {
        "input_tokens": 45,
        "output_tokens": 2800,
        "total_tokens": 2845,
        "billing_info": {
            "cost_breakdown": {
                "agent_cost": 0.04,
                "input_token_cost": 0.000135,
                "output_token_cost": 0.042,
                "token_counts": {
                    "total_input_tokens": 45,
                    "total_output_tokens": 2800,
                    "total_tokens": 2845
                },
                "num_agents": 4,
                "service_tier": "standard",
                "night_time_discount_applied": true
            },
            "total_cost": 0.082135,
            "discount_active": true,
            "discount_type": "night_time",
            "discount_percentage": 75
        }
    }
}
`

## [
## ][77]
## Best Practices
* Design agents with flexible, transferable skills
* Use for projects with evolving requirements
* Ensure agents can adapt to different roles effectively
* Ideal for dynamic workflows and adaptive systems

[Hierarchical swarm][78][Graph workflow][79]
⌘I
[twitter][80][github][81][linkedin][82][discord][83][youtube][84][medium][85]
[Powered byThis documentation is built and hosted on Mintlify, a developer documentation platform][86]

[1]: /llms.txt
[2]: #content-area
[3]: /
[4]: https://swarms.world/platform/api-keys
[5]: https://swarms.world/platform/account
[6]: https://cal.com/swarms/swarms-technical-support?overlayCalendar=true
[7]: https://status.swarms.ai
[8]: /docs/introduction/overview
[9]: /docs/documentation
[10]: /docs/marketplace
[11]: /api-reference/general/api-root
[12]: /docs/examples/examples/client-setup
[13]: /docs/documentation/changelog
[14]: https://swarms.world/platform/api-keys
[15]: https://swarms.world/platform/account
[16]: https://discord.gg/EamjgSaEQf
[17]: https://cal.com/swarms/swarms-technical-support?overlayCalendar=true
[18]: /docs/documentation
[19]: /docs/documentation/faq
[20]: /docs/documentation/getting-started/quickstart
[21]: /docs/documentation/getting-started/api-key-setup
[22]: /docs/documentation/getting-started/setup
[23]: /docs/documentation/getting-started/architecture
[24]: /docs/documentation/resources/client-libraries
[25]: /docs/documentation/clients/python-client
[26]: /docs/documentation/clients/swarms-api-mcp
[27]: /docs/documentation/clients/swarms-docs-mcp
[28]: /docs/documentation/capabilities/agent
[29]: /docs/documentation/capabilities/openai-compatible
[30]: /docs/documentation/multi-agent/overview
[31]: /docs/documentation/multi-agent/available-architectures
[32]: /docs/documentation/multi-agent/sequential_workflow
[33]: /docs/documentation/multi-agent/concurrent_workflow
[34]: /docs/documentation/multi-agent/multi_agent_router
[35]: /docs/documentation/multi-agent/mixture_of_agents
[36]: /docs/documentation/multi-agent/group_chat
[37]: /docs/documentation/multi-agent/majority_voting
[38]: /docs/documentation/multi-agent/hierarchical_swarm
[39]: /docs/documentation/multi-agent/agent_rearrange
[40]: /docs/documentation/multi-agent/graph_workflow
[41]: /docs/documentation/multi-agent/debate_with_judge
[42]: /docs/documentation/multi-agent/heavy_swarm
[43]: /docs/documentation/multi-agent/round_robin
[44]: /docs/examples/examples/batch-processing
[45]: /docs/examples/examples/batch-agent-scale-tutorial
[46]: /docs/examples/examples/batch-swarm-completions
[47]: /docs/examples/examples/batch-swarm-scale-tutorial
[48]: /docs/documentation/multi-agent/batched_grid_workflow
[49]: /docs/documentation/capabilities/swarms_api_tools
[50]: /docs/documentation/capabilities/sub_agents
[51]: /docs/documentation/capabilities/mcp_integration
[52]: /docs/documentation/capabilities/tools
[53]: /docs/documentation/capabilities/agents_list
[54]: /docs/documentation/resources/resources-overview
[55]: /docs/documentation/resources/ratelimits
[56]: /docs/documentation/resources/rate-limit-headers
[57]: /docs/documentation/resources/pricing
[58]: /docs/documentation/resources/usage-report
[59]: /docs/documentation/resources/security
[60]: /docs/documentation/resources/response-compression
[61]: /docs/documentation/resources/premium-endpoints
[62]: /docs/documentation/resources/priority-processing
[63]: /docs/documentation/resources/global-availability
[64]: /docs/documentation/resources/community
[65]: /docs/documentation/resources/technical-support
[66]: /docs/documentation/resources/referral
[67]: /docs/documentation/resources/contributors
[68]: #overview
[69]: #use-cases
[70]: #api-usage
[71]: #basic-agentrearrange-example
[72]: #best-practices
[73]: #overview
[74]: #use-cases
[75]: #api-usage
[76]: #basic-agentrearrange-example
[77]: #best-practices
[78]: /docs/documentation/multi-agent/hierarchical_swarm
[79]: /docs/documentation/multi-agent/graph_workflow
[80]: https://twitter.com/swarms_corp
[81]: https://github.com/The-Swarm-Corporation/swarms-api-docs
[82]: https://www.linkedin.com/company/the-swarm-corporation
[83]: https://discord.gg/EamjgSaEQf
[84]: https://www.youtube.com/channel/UC9yXyitkbU_WSy7bd_41SqQ
[85]: https://medium.com/@kyeg
[86]: https://www.mintlify.com?utm_campaign=poweredBy&utm_medium=referral&utm_source=swarms
```
