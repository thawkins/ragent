---
title: "Capability-Based Agent Permissions"
type: concept
generated: "2026-04-19T16:13:14.936037439+00:00"
---

# Capability-Based Agent Permissions

### From: plan

Capability-based agent permissions represent a security and architectural design where agents are granted specific, limited capabilities rather than broad system access, with these capabilities enforced through the tool system and agent activation mechanisms. The plan agent exemplifies this pattern through its explicit read-only constraint: it is delegated tasks for "codebase analysis and architecture planning" with the documented limitation that it "cannot modify files." This constraint is encoded in multiple layers: the tool's description communicates the limitation to LLM planners, the permission category ("plan") enables policy-based enforcement, and the agent implementation presumably lacks access to file-modifying tools. Capability-based design reduces the blast radius of potential errors or prompt injection attacks, as compromised or malfunctioning agents have constrained abilities. The pattern aligns with the principle of least privilege, where agents receive only the permissions necessary for their specific function. In this architecture, capabilities are not merely static properties but are dynamically activated and deactivated through the delegation mechanism—when the plan agent is not active, its capabilities are entirely unavailable to the system. This temporal dimension of capability management provides additional security boundaries. The approach also supports auditability, as each capability activation and deactivation is logged through the event system, creating a complete trace of which agent had which capabilities at any given time.

## External Resources

- [Wikipedia article on capability-based security models](https://en.wikipedia.org/wiki/Capability-based_security) - Wikipedia article on capability-based security models
- [Cornell University survey paper on capability-based systems](https://www.cs.cornell.edu/andru/caps-fckl/paper/caps-why-what.html) - Cornell University survey paper on capability-based systems

## Related

- [Agent Delegation Pattern](agent-delegation-pattern.md)

## Sources

- [plan](../sources/plan.md)
