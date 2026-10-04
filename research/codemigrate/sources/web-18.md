# Web source

- URL: https://act101.ai/docs/porting
- Title: Porting — act101
- Author(s): act101 LLC, @act101ai
- Language: English
- Published (UTC): -
- Captured (UTC): 2026-10-02T21:25:35.420682505+00:00
- Relevance: Medium - partial query match


```text
act101’s porting documentation describes a 10-operation, Enterprise-licensed workflow for AI agents to perform full source-to-target language migration across any two of act101’s 163 supported grammars via a contract/inventory/ordering/manifest state machine, designed to be structured, incremental, auditable, and reversible. The prepare phase uses port_contract to extract behavioral contracts (signatures, error paths, guard clauses, side effects, purity, complexity), port_inventory to enumerate symbols/modules/dependencies, and port_order to compute a topological sequence; the execute phase uses port_manifest_init/add/update/remove/note to track mappings and progress, while generate_migration_shim and generate_adapter bridge ported and not-yet-ported code (both support TypeScript and Python; other targets return supported:false). Verification combines verify_port_parity (differential execution) and port-scope verify_behavioral_equivalence via the /port-verify skill into one port-correctness verdict; operations are idempotent, and the manifest (.act/port-manifest.json) is the single source of truth to commit with code so collaborators and CI can observe port status without re-running analysis.
```
