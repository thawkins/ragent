---
title: "PlanEnterTool"
entity_type: "technology"
type: entity
generated: "2026-04-19T16:13:14.934265555+00:00"
---

# PlanEnterTool

**Type:** technology

### From: plan

The `PlanEnterTool` is a concrete implementation of the `Tool` trait that serves as the entry point for delegating tasks to a specialized planning agent within the ragent-core framework. When instantiated and executed, this tool performs validation on the incoming task parameter to ensure non-empty, meaningful task descriptions are provided for the planning agent. Upon successful validation, it constructs and publishes an `AgentSwitchRequested` event to the system's event bus, which acts as the signaling mechanism for the session processor to initiate an agent context switch. The tool's architecture reflects a careful separation of concerns: parameter validation occurs at the tool level, while the actual agent lifecycle management is delegated to the broader session management infrastructure through events. The tool returns structured metadata including an `agent_switch` field set to "plan", which serves as a contract between the tool execution layer and the session processor. This metadata-driven approach allows the system to maintain loose coupling while enabling precise coordination between tool execution outcomes and system state transitions. The tool's permission category of "plan" categorizes it within the broader security and permission framework of the system, potentially enabling fine-grained access control policies.

## Diagram

```mermaid
flowchart TD
    subgraph InputValidation["Input Validation Phase"]
        node1["Receive JSON Input"] --> node2["Extract 'task' Parameter"]
        node2 --> node3["Validate Non-Empty Task"]
        node3 --> node4["Extract Optional 'context'"]
    end
    
    subgraph EventPublication["Event Publication Phase"]
        node4 --> node5["Create AgentSwitchRequested Event"]
        node5 --> node6["Publish to Event Bus"]
    end
    
    subgraph OutputGeneration["Output Generation Phase"]
        node6 --> node7["Format Response Content"]
        node7 --> node8["Construct ToolOutput with Metadata"]
        node8 --> node9["Return Ok Result"]
    end
    
    node3 -.->|Validation Fails| node10["Return Error via bail!"]
    node2 -.->|Missing Parameter| node10
```

## External Resources

- [async-trait crate documentation for understanding the async trait implementation pattern used](https://docs.rs/async-trait/latest/async_trait/) - async-trait crate documentation for understanding the async trait implementation pattern used
- [Serde serialization framework documentation for JSON schema handling](https://serde.rs/) - Serde serialization framework documentation for JSON schema handling

## Sources

- [plan](../sources/plan.md)
