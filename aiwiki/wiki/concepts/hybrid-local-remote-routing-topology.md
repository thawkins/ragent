---
title: "Hybrid Local-Remote Routing Topology"
type: concept
generated: "2026-04-19T20:52:42.207225163+00:00"
---

# Hybrid Local-Remote Routing Topology

### From: transport

The hybrid routing topology concept enables unified treatment of co-located and distributed agents through prioritized fallback chains, optimizing for both performance and availability. The `RouterComposite` pattern with `InProcessRouter` primary and `HttpRouter` secondary exemplifies this approach, where the system attempts zero-copy in-memory message delivery before incurring network serialization overhead and latency. This topology reflects the reality of modern distributed systems where edge computing deployments, microservice architectures, and multi-region applications create mixed locality environments within single logical applications.

Performance characteristics vary dramatically between routing strategies. In-process routing achieves sub-microsecond latency through shared memory channels, while HTTP routing incurs millisecond-scale penalties from TCP handshake, TLS negotiation, serialization, and network transmission. The hybrid approach automatically exploits data locality—when agent and coordinator coexist in the same process, they communicate with minimal overhead; when agents are externalized for scaling, isolation, or capability reasons, the system transparently adapts. This transparency is crucial for developer experience, allowing agents to migrate between deployment modes without consumer code changes.

The topology also enables evolutionary system architectures where components begin as in-process libraries and extract to services as scalability demands increase. Early-stage applications benefit from simple deployment models with all agents in-process; as user growth demands horizontal scaling, specific agents can be externalized to dedicated hosts or serverless functions, with the `RouterComposite` automatically routing appropriate traffic to the new deployment. This pattern appears in industry systems like Netflix's Ribbon client-side load balancer with local fallback and Kubernetes' service proxy with endpoint subset prioritization. The ragent implementation's explicit priority ordering (first-success-wins) supports deterministic behavior for debugging while allowing runtime reconfiguration through router ordering changes.

## External Resources

- [Netflix Ribbon: Load balancing with Eureka](https://netflixtechblog.com/ribbon-load-balancing-with-eureka-d82d1d5241be) - Netflix Ribbon: Load balancing with Eureka
- [AWS modern application development: Hybrid architectures](https://docs.aws.amazon.com/whitepapers/latest/modern-application-development-on-aws/the-elastic-compute-cloud-Amazon-ec2-2.html) - AWS modern application development: Hybrid architectures

## Sources

- [transport](../sources/transport.md)
