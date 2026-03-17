AGENT_TEAMS.md

Summary: Strategies for scheduling multi-agent teams

This document summarizes centralized strategies, modelling patterns, solver technologies, heuristics, graph/flow methods, and practical engineering recommendations for scheduling and task assignment in multi-agent systems (robots, machines, compute agents, vehicles). It is intended as a concise reference for engineers deciding how to model, prototype and deploy scheduling solutions.

Contents
- Executive summary
- Problem statement & objectives
- Centralized approaches overview
  - Exact/optimization-based (MILP/ILP)
  - Constraint programming (CP & CP-SAT)
  - Heuristics & metaheuristics
  - Graph-based & flow methods
  - Decomposition & hybrid methods
  - Online, auction, and market-based methods
- Practical modelling tips
- Solver & library recommendations
- Selection guide: which approach to use when
- Key takeaways
- Example engineering checklist
- References and further reading

Executive summary
Centralized scheduling methods trade-off optimality guarantees against expressiveness, runtime, and scalability. Exact methods (MILP/ILP, CP) can produce optimal or high-quality schedules for small-to-medium instances with complex constraints. Heuristics and metaheuristics scale to large instances and are essential for online, anytime, or real-time multi-agent task allocation. Graph and flow-based approaches are efficient and exact for assignment-like problems but require extensions when ordering/time/resource constraints appear. Hybrid and decomposition strategies often give the best pragmatic balance for real-world systems.

Problem statement & objectives
- Input: tasks (jobs, missions) with attributes (duration, release/time windows, deadlines, priorities, resource needs, precedence), agents with capabilities and availability, global constraints (shared resources, collision avoidance), and objectives (minimize makespan, total weighted completion, latency, energy, SLA violations, or multi-objective trade-offs).
- Output: assignment and schedule mapping tasks to agents over time, including start times and ordering where necessary.
- Requirements: offline vs online decisions, re-planning frequency, optimality vs timeliness, robustness to uncertainty, and scale.

Centralized approaches overview
1) Exact/Optimization-based (MILP/ILP)
- Typical formulations: assignment/flow formulations, time-indexed formulations, disjunctive (pairwise ordering) formulations, set-partitioning/vehicle-routing-like formulations.
- Strengths: mature solvers (Gurobi, CPLEX, CBC), strong modelling for linear objectives and constraints, can provide optimality bounds and proofs.
- Weaknesses: scalability degrades as binary variables and coupling constraints grow (ordering, timing, big-M). Time-indexed models can explode with long horizons or fine resolution.
- When to use: small-to-medium tightly-constrained instances where optimality or bounds matter, or when the problem can be compactly encoded (assignment without complex sequencing).

2) Constraint Programming (CP & CP-SAT — e.g., Google OR-Tools CP-SAT)
- Key modelling primitives: interval/optional-interval variables, no-overlap, cumulative/global resource constraints, precedence constraints, and rich boolean/int logic.
- Strengths: expressive modelling of complex temporal/logical constraints, strong propagation and constraint filtering, often faster than MILP on highly combinatorial scheduling with global constraints.
- Weaknesses: performance depends heavily on modelling choices and search strategy; proving optimality may still be costly for large instances.
- When to use: problems with rich temporal/resource constraints (cumulative resources, optional tasks, complex precedences), or when rapid prototyping of scheduling logic is needed.

3) Heuristics & Metaheuristics
- Families: greedy/priority rules, list scheduling, local search, hill-climbing, simulated annealing, tabu search, genetic algorithms, ant colony optimization, large neighborhood search (LNS), and hybrids (heuristic init + LNS/CP for improvement).
- Strengths: scale to large instances, fast or anytime behaviour, easier to adapt to dynamic problems and online re-planning.
- Weaknesses: no global optimality guarantees, require tuning, quality varies by instance.
- When to use: large-scale systems, online/adaptive scheduling, or where rapid feasible solutions are required.

4) Graph-based & Flow methods
- Techniques: bipartite matching (assignment), Hungarian algorithm for assignment, min-cost max-flow for time-expanded assignment graphs, multi-commodity flows for more complex resource-aware allocations.
- Strengths: polynomial-time exact solutions for pure assignment or flow formulations, efficient libraries available.
- Weaknesses: adding sequencing, timing, or complicated resource/contention constraints typically requires MILP/CP layers.
- When to use: assignment problems without complex sequencing (task→agent matching, time-windowed assignment with small discretization), or as subroutines in decomposition.

5) Decomposition & Hybrid methods
- Ideas: combine approaches — e.g., use an assignment solver (flow) to allocate tasks to agents, then per-agent sequencing via CP or local search; or use MILP/CP on a rolling horizon; or heuristics for large-scale partitioning and exact solvers for subproblems.
- Strengths: exploit structure, improve scalability while retaining higher-quality solutions on subproblems.

6) Online, Auction & Market-based methods
- Auction algorithms (e.g., Bertsekas’ auction) and market-inspired allocation provide decentralized but controllable centralized-style assignments; they excel for dynamic, asynchronous task arrivals and heterogeneous agent capabilities.
- Strengths: naturally distributed, fast, anytime, and robust to changing availability.
- Weaknesses: may require repeated local negotiation and may not handle complex temporal constraints without additional planning.

Practical modelling tips
- Start with the simplest model that captures the objective and critical constraints (assignment → extend with time/sequencing only if necessary).
- Use compact formulations where possible (assignment/flow); switch to time-indexed only for short horizons or coarse discretization.
- When using disjunctive/binary ordering variables, avoid naive big-M values; use tight bounds and problem-specific ordering reductions.
- Exploit problem structure: identical agents reduce symmetry; partition by capability or geography; pre-cluster tasks and solve per-cluster.
- Use greedy or priority-rule heuristics to generate initial solutions for MILP/CP or to seed metaheuristics; good initial solutions often reduce solver time.
- For CP/CP-SAT: use interval variables, optional tasks, and global constraints (no-overlap, cumulative); add symmetry-breaking constraints and hint initial assignments where supported.
- For LNS/hybrid: implement large neighborhood operators informed by domain structure (e.g., remove all tasks for one region or all tasks assigned to a bottleneck agent).
- When real-time performance is required, prefer anytime heuristics with bounded re-plan windows or rolling horizon strategies.

Solver & library recommendations
- Optimization (MILP/IP): Gurobi (commercial, fastest in many benchmarks), CPLEX (commercial), SCIP (academic/opensource), CBC (open source but slower); modeling: Pyomo, PuLP, JuMP (Julia), OR-Tools linear solver.
- CP & CP-SAT: Google OR-Tools CP-SAT (state-of-the-art SAT-based integer solver with scheduling primitives), IBM CP Optimizer (commercial), OR-Tools CP model.
- Graph/flow: NetworkX (Python, convenient but not for large heavy flows), Lemon (C++ library), Boost Graph Library, Google OR-Tools min-cost flow.
- Metaheuristics/LNS: implement custom search or use OR-Tools local search framework, or libraries/frameworks in your language of choice.

Selection guide: which approach to use when
- Pure assignment (match tasks→agents, no sequencing/time) → Hungarian / min-cost flow (exact, scalable).
- Assignment with simple time windows, small horizon → time-expanded flow or time-indexed MILP (if horizon small) or min-cost flow on time-expanded graph.
- Rich temporal/resource constraints (cumulative resources, optional tasks) → CP/CP-SAT (expressive global constraints).
- Moderate-size problem where optimality desired and problem compactly formulated → MILP with Gurobi/CPLEX.
- Large-scale / online / dynamic → heuristics, auction-based methods, hybrid approaches; use rolling horizon + quick heuristics.
- Very large but structure exists (clusters, identical agents) → decompose and solve subproblems with exact methods.

Key takeaways (8–12)
1. Choose the simplest model that captures the critical constraints — often assignment + per-agent sequencing is enough.
2. Use exact methods (MILP/CP) for small-medium instances or when you need bounds; prefer CP for complex temporal constraints.
3. Heuristics and metaheuristics are essential for scale and online operation — combine them with exact solvers for improvement (hybrids/LNS).
4. Graph/flow methods are excellent for assignment problems and should be the first choice when sequencing is not central.
5. Time-indexed formulations have strong LP relaxations but scale poorly with horizon; use sparse or compressed time indexing if you must.
6. Avoid naive big-M formulations — use tight bounds and problem-specific reductions to improve MILP performance.
7. Use rolling horizon and anytime planning for dynamic problems; keep planning horizons short and replan frequently with warm-starts.
8. Seed solvers with good initial solutions from greedy heuristics to dramatically reduce solve time.
9. Exploit structure (identical agents, geography, task clusters) to decompose the problem for tractable exact subproblems.
10. For multi-objective trade-offs (latency vs energy vs fairness), either scalarize or use hierarchical optimization (primary constraint, secondary objective) to keep models manageable.
11. Leverage mature libraries: OR-Tools for CP-SAT and flows, Gurobi/CPLEX for MILP, Lemon/Boost for graph algorithms.
12. Measure and benchmark with representative instances (use public benchmarks like Taillard or domain-specific datasets) before committing to a single approach.

Example engineering checklist
- Define objectives and hard vs soft constraints.
- Start with a simple assignment model; measure baseline.
- Add temporal/resource constraints only if necessary; evaluate solver choice (flow vs MILP vs CP).
- Prototype a greedy heuristic and an improvement phase (local search / LNS / CP improvement).
- Run benchmarks: scale, worst-case, and representative dynamic scenarios.
- Add monitoring and fallback: if optimal solver times out use the best heuristic solution.

References and further reading
- Gerkey, B. P., & Mataric, M. J. (2004). A formal analysis and taxonomy of task allocation in multi-robot systems. Proceedings of the International Conference on Intelligent Robots and Systems (IROS). (paper) https://www.cs.utexas.edu/~jeffp/papers/aaai04-gerkey.pdf
- Google OR-Tools (CP-SAT, scheduling guides, flows) — docs and examples: https://developers.google.com/optimization
- Hungarian algorithm / assignment problem (overview): https://en.wikipedia.org/wiki/Hungarian_algorithm
- Min-cost flow / assignment via flows (overview): https://en.wikipedia.org/wiki/Minimum-cost_flow_problem
- Taillard benchmarks for scheduling (classic job-shop benchmarks): http://mistic.heig-vd.ch/taillard/benchmarks.html
- Bertsekas, D. P. — Auction algorithms and assignment methods (see his book "Network Optimization: Continuous and Discrete Models"). Overview: https://people.csail.mit.edu/dimitrib/ (author page)
- Gurobi Optimizer — commercial MILP solver: https://www.gurobi.com
- IBM CPLEX Optimizer — commercial MILP/CP: https://www.ibm.com/products/ilog-cplex-optimization-studio
- Survey / review on multi-robot task allocation (search academic literature for up-to-date domain surveys)

Notes
- This document is a practical synthesis rather than a comprehensive literature survey. Specific domains (e.g., multi-robot motion-coordinated tasking, vehicle routing with time windows, cloud job scheduling) will have tailored algorithms and benchmarks; consult field-specific surveys for depth.


---
Generated by background research agents. If you want, I can:
- Expand any section with more detailed algorithmic descriptions and pseudocode.
- Add concrete code examples in Python (OR-Tools), Rust (bindings), or Julia (JuMP).
- Produce a shorter quick-reference cheat-sheet for engineering teams.

