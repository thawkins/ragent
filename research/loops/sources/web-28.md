# Web source

- URL: https://www.mindstudio.ai/blog/agentic-loop-design-goals-verification-criteria
- Title: Agentic Loop Design: How to Define Goals and Verification Criteria That Actually Work
- Author(s): Luis Chavez-Mattos
- Language: English
- Published (UTC): 2026-06-21T00:00:00+00:00
- Captured (UTC): 2026-09-13T14:47:02.808383053+00:00
- Relevance: Medium — multiple title terms match query


```text
This MindStudio guide argues that most agentic loop failures are goal failures rather than model failures: an agentic loop (the observe–reason–act–check cycle) requires three components—a goal, a verification method, and a stop condition—and fails in two modes, runaway loops or premature termination, when goals aren't verifiable. A verifiable goal describes an observable state that can be confirmed without the agent's own judgment, passing the "second agent test" (an independent agent could confirm completion from the output alone, with no extra context). Stop conditions come in three types—completion-based (the ideal, binary checks such as "all items have status processed or error"), iteration-based (a safety-net cap, with 5–10 iterations as a practical starting point), and quality-threshold (operationally defined criteria rather than "good enough")—and every loop should pair a completion condition with an iteration cap. Verification must be structurally separate from execution because models show "self-serving evaluation bias" when judging their own output; the recommended pattern is a dedicated verification prompt (~50–150 words) returning structured JSON such as `{"passed": false, "failed_criteria": [...], "can_retry": true}` that a loop router can act on. The article catalogs four failure patterns—the "good enough" trap, infinite refinement loops, silent failure loops, and scope creep loops—each fixed by enumerated output schemas, checklist-based "done" definitions, explicit error states, and scope constraints, and notes that MindStudio supports this architecture through discrete workflow steps, conditional branching, and 200+ AI models (enabling cheaper models for verification steps).
```
