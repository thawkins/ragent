# ragent-specs

Spec management system for ragent: EARS notation specs, validation, lifecycle
tracking, plan parsing, constitution parsing, and spec implementation runner.
A fully self-contained crate with zero internal workspace dependencies.

## Workspace Dependencies

None. `ragent-specs` is fully self-contained.

## External Dependencies

- serde (derive)
- thiserror
- regex
- tokio (fs, io-util)
- tracing

Dev-dependencies: tokio (full), tempfile.

## Public API (crate root)

### Re-exported types

- **SpecCommand** (enum) — Parsed `/spec` subcommand (Help, Create, Validate, List, Search, Status, Task, Activate, Deactivate, Coverage, Impl, Add, Update, Delete, Jtbd).
- **Amendment** / **AmendmentIssue** / **AmendmentRequest** / **Article** / **Constitution** (structs) — Constitution artifact types.
- **SpecError** (enum) — Spec management errors.
- **BranchResult** (enum) — Branch-creation outcome.
- **ImplOptions** / **ImplResult** / **MilestoneGroup** / **SpecImplRunner** (structs) — Spec implementation runner.
- **SpecIo** (struct) — I/O helper for spec management.
- **SpecManager** / **SpecFilter** / **SpecSearchResult** / **SortBy** (structs/enums) — Spec lifecycle manager.
- **Effort** / **Milestone** / **PhaseMinusOneGate** / **PhaseMinusOneGates** / **PlanParser** / **PlanTask** / **Priority** (structs/enums) — PLAN.md parsing.
- **Plan** / **Requirement** / **Spec** / **SpecId** / **SpecStatus** / **Task** / **TaskStatus** / **EarsTemplate** (structs/enums) — Core spec data structures.
- **ConstitutionTemplate** / **FeedbackTemplate** / **PlanTemplate** / **SpecTemplate** (structs) — Default file templates.
- **AmbiguityIssue** / **AmbiguityKind** / **Category** / **ClarificationMarker** / **ContradictionIssue** / **ContradictionKind** / **GapIssue** / **GapKind** / **Issue** / **ParsedRequirement** / **Report** / **SddFlags** / **Severity** (structs/enums) — Validation.

### Re-exported functions

- **parse_constitution** — Parse a CONSTITUTION.md string into a `Constitution`.
- **create_spec_branch** / **spec_branch_name** — Git branch-per-spec workflow.
- **is_valid_transition** / **next_statuses** — Status lifecycle graph.
- **detect_ambiguity** / **detect_clarification_markers** / **detect_contradictions** / **detect_ears_template** / **detect_gaps** / **parse_requirements** — Validation helpers.
- **validate** / **validate_clarifications** / **validate_consistency** / **validate_with_flags** — Validation entry points.

### Constants

- **REQUIRED_GATE_NAMES** — The three required Phase -1 gate names: `["Simplicity", "Anti-Abstraction", "Integration-First"]`.

## Module: commands

- **SpecCommand** (enum) — Parsed `/spec` subcommand with structured fields per variant.

## Module: constitution

- **Article** / **Amendment** / **AmendmentRequest** / **AmendmentIssue** / **Constitution** (structs).
- **parse_constitution** (fn).
- `Constitution` methods: `empty`, `is_empty`, `article_by_number`, `article_by_title`, `path`, `apply_amendment`, `validate_amendments`.

## Module: error

- **SpecError** (enum) — `AlreadyExists`, `NotFound`, `Io`, `InvalidSpecId`, `InvalidStatusTransition`, `Validation`, `InvalidStructure`, `UnknownId`, `DependencyCycle`, `PlanParse`, `AlreadyImplemented`, `UnresolvedClarifications`, `UncheckedPhaseGates`, `AmendmentError`.

## Module: git

- **BranchResult** (enum) — `Created`, `NotARepo`, `AlreadyExists`, `Failed`.
- **spec_branch_name(specname)** / **create_spec_branch(specname, working_dir)** (fns).

## Module: id_scanner

- **highest_id** / **highest_fr** / **highest_nfr** / **highest_task** (fns) — Find highest numeric IDs.
- **extract_fr_ids** / **extract_nfr_ids** / **extract_task_ids** (fns) — Extract all IDs.

## Module: impl_runner

- **ImplOptions** / **ImplResult** / **MilestoneGroup** / **SpecImplRunner** (structs).
- `SpecImplRunner` methods: `new`, `spec_name`, `tasks`, `execution_order`, `total_to_execute`, `milestone_groups`, `task_id_at`, `task_prompt`, `run`, `resolve_requirements`, `build_file_order_warning`.
- Helpers: `build_progress_update`, `build_completion_summary`, `build_cancellation_summary`, `build_blocked_summary`, `find_dependents`, `parse_impl_args`.

## Module: io

- **SpecIo** (struct) — methods: `create_spec_dir`, `atomic_write`, `read_file`, `spec_exists`, `discover_specs`, `read_spec`, `write_spec`, `write_spec_fields`, `extract_title`, `extract_status`.

## Module: manager

- **SpecManager** (struct) — methods: `new`, `root`, `discover_specs`, `read_spec`, `write_spec`, `create_spec`, `delete_spec`, `transition`, `transition_with_flags`, `update_task_status`, `list_specs`, `search_specs`, `search_specs_with_archived`.
- **SpecFilter** (struct) — methods: `new`, `with_status`, `with_id_prefix`, `with_modified_since`, `with_archived`, `with_sort`.
- **SortBy** (enum) — `ModifiedAt`, `Status`, `Id`, `Title`.
- **SpecSearchResult** (struct) — Search result with relevance score and context snippets.
- **is_valid_transition** / **next_statuses** (fns).

## Module: plan_parser

- **Effort** (enum) / **Priority** (enum) — with `parse` and `as_str`.
- **PlanTask** (struct) — Parsed task from PLAN.md.
- **Milestone** (struct) — with `normalise_item`.
- **PhaseMinusOneGate** / **PhaseMinusOneGates** (structs) — Gate parsing; methods: `is_all_checked`, `unchecked_required_gates`, `has_all_required_gates`, `is_empty`.
- **PlanParser** (struct) — methods: `parse`, `parse_phase_minus_one_gates`, `parse_milestones`.
- **REQUIRED_GATE_NAMES** (const).
- **resolve_execution_order** / **filter_for_task** / **filter_for_resume** (fns) — Dependency resolution.

## Module: spec

- **SpecId** (struct) — methods: `new`, `as_str`, `dir_name`.
- **SpecStatus** (enum) — `ALL` const, `as_str`, `parse`.
- **EarsTemplate** (enum) — `as_str`.
- **Requirement** (struct) — A single requirement.
- **TaskStatus** (enum) — `as_str`, `parse`.
- **Task** (struct) — A single implementation task.
- **Spec** (struct) — methods: `new`, `dir_path`, `spec_md_path`, `plan_md_path`, `feedback_md_path`, `coverage_pct`, `transition`.
- **Plan** (struct) — methods: `new`, `path`.

## Module: templates

- **SpecTemplate** / **PlanTemplate** / **FeedbackTemplate** / **ConstitutionTemplate** (structs) — `generate` methods.

## Module: validate

- **SddFlags** (struct) — methods: `all_enabled`, `all_disabled`, `from_bools`.
- **Severity** (enum) / **Category** (enum) / **Issue** (struct) — Validation issue types.
- **Report** (struct) — methods: `new`, `add`, `has_errors`, `has_warnings`, `has_clarifications`, `has_consistency_issues`, `count_by_category`, `has_phase_gate_issues`, `count_by_severity`, `sort`, `format`.
- **ClarificationMarker** / **AmbiguityIssue** / **AmbiguityKind** / **ContradictionIssue** / **ContradictionKind** / **GapIssue** / **GapKind** / **ParsedRequirement** (structs/enums) — Consistency analysis types.
- Functions: `detect_ears_template`, `detect_clarification_markers`, `detect_ambiguity`, `detect_contradictions`, `detect_gaps`, `parse_requirements`, `extract_sections`, `validate`, `validate_with_flags`, `validate_structure`, `validate_ears`, `validate_plan`, `validate_clarifications`, `validate_consistency`, `validate_phase_minus_one_gates`.

## Status Transition Graph

```
Draft -> InReview
InReview -> Draft | Approved
Approved -> InProgress
InProgress -> Implemented
Implemented -> Verified
Verified -> Archived
Archived -> Draft
```