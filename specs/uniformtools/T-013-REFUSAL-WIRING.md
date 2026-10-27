# T-013 - One place builds a pre-execution refusal, and that place owns `is_error`

Spec `uniformtools`, task T-013. Requirements FR-008 (corrective error on
undecodable arguments), FR-009 (corrective error on schema violation) and FR-014
(error results are marked).

Status: **completed**. Dependency: T-012 (the one argument decoder).

## 1. What the task asks for

> **FR-008** When argument decoding fails, the system **shall not** execute the
> tool, and **shall** append a tool result whose text names the tool, quotes the
> parse error, and instructs the model to resend one complete JSON object, with
> the record's status set to `Error`.
>
> **FR-009** When `validate_required_args` reports a violation against the
> tool's declared `parameters_schema()`, the system **shall not** execute the
> tool, and **shall** append a tool result naming the offending parameter and its
> declared type.
>
> **FR-014** Every tool result **shall** carry an `is_error` flag derived from
> its record status.

The task title says it plainly: *wire the corrective argument-decode error and
the schema-violation error into one place that also sets `is_error`*. Three
requirements collapse into one deliverable - a refusal is a value, and the value
owns its sentence, its record status, and therefore its flag.

## 2. What was actually missing

`crates/ragent-agent/src/session/tool_input_error.rs` already existed
(`d6abcd4a`): `ToolInputError` with `Undecodable` / `SchemaViolation` /
`UnknownTool`, four constructors, `status()`, `is_error()`, `into_anyhow()`, and
one `Display` that quotes the parser and states both the tool and the
non-execution. Its contract file `tests/test_tool_input_error.rs` existed too, 7
cases.

**Neither was reachable.**

1. `crates/ragent-agent/src/session/mod.rs` never declared
   `pub mod tool_input_error;`. The file was dead source: `cargo check` did not
   build it.
2. `crates/ragent-agent/tests/all.rs` never had a `#[path]` line for
   `test_tool_input_error.rs`. The 7 cases had never run.
3. `crates/ragent-types/src/lib.rs` did not re-export `ToolArgDecodeError`, so
   the test file's `use ragent_types::ToolArgDecodeError;` could not resolve.

That is the same orphan-module defect T-030 and T-012 each found, third time in
this spec. Meanwhile the two gates the task names were still hand-rolled, and
they had drifted:

| Gate | Sentence before | Status before | Defect |
|---|---|---|---|
| `processor.rs` decode gate (B1) | `Invalid arguments JSON for tool '<t>': <e>. Arguments must be a single valid JSON object; resend the complete tool call with corrected JSON.` | `ToolCallStatus::Error` literal | its own wording, its own status, and a `format!("Error: {msg}")` content wrapper - three parallel decisions where the constructor has one |
| `processor.rs` in-task schema gate (B3) | `Invalid tool arguments: <schema error>` (wrapped in `anyhow!`) | `Error` derived later by the generic error path | no "the tool was not executed", no instruction to resend, and a *different* prefix from the decode gate's |

So the "one place that also sets `is_error`" existed but neither gate used it,
because the module it lives in was not part of the crate.

A fourth defect surfaced once the contract cases could run:
`schema_refusal_names_the_parameter_and_its_type` failed, because FR-009 asks the
refusal to name the offending parameter **and its declared type**, and
`validate_required_args` said only `missing required parameter 'destination'`.
The type was in the schema it was reading and simply not reported.

## 3. What landed

### 3.1 The module is compiled, and its type is importable

- `pub mod tool_input_error;` in `crates/ragent-agent/src/session/mod.rs`, with
  the module's one-line doc, in alphabetical position between `text_toolcalls`
  and `verification`.
- `#[path = "test_tool_input_error.rs"] mod test_tool_input_error;` in
  `crates/ragent-agent/tests/all.rs`.
- `pub use tool_args::ToolArgDecodeError;` at the `ragent-types` crate root, so
  the refusal's argument type is nameable from the root the rest of the workspace
  already imports.

### 3.2 The decode gate builds a `ToolInputError`

`SessionProcessor::process_message`'s B1 gate no longer formats a sentence. It
decodes through the shared helper (T-012), and on failure builds:

```rust
let refusal = match schema_violation(&self.tool_registry, &tc.name, &Value::Null) {
    Err(reason) => ToolInputError::undecodable_with_reason(&tc.name, decode_error, &reason),
    Ok(()) => ToolInputError::undecodable(&tc.name, decode_error),
};
let err_msg = refusal.to_string();
```

and then takes **both** the record status and the message from that value:

```rust
refusal.status(),   // was: ToolCallStatus::Error (a literal)
```

The `undecodable_with_reason` arm preserves the pre-existing B3 behaviour - a
non-object payload is *also*, as far as the tool's schema is concerned, missing
every required parameter, so both reasons go into one sentence and the model can
fix the call in a single retry. The type of `parsed_input` changed from
`Result<Value, String>` to `Result<Value, ragent_types::ToolArgDecodeError>`, so
the gate passes the decoder's own error value to the constructor instead of its
`Display` rendering - the constructor quotes it, and nothing re-parses a string
to recover a struct.

The `is_error` flag needed no new code: the loop's single append site already
writes `is_error: status.is_error()`, so once the status comes from the refusal
the flag does too.

### 3.3 The schema gate builds a `ToolInputError`

The in-task B3 gate's bare `anyhow!("Invalid tool arguments: {schema_err}")`
became:

```rust
Err(ToolInputError::schema_violation(&tc_clone.name, &schema_err).into_anyhow())
```

`into_anyhow` exists for exactly this shape: the task's single error exit path
already reformats an error into the result's text and status, and `Display` and
`{:#}` render the identical sentence, so the in-task gate and the pre-dispatch
gate now produce the same words. The prefix `Invalid tool arguments: ` is gone -
it was the second wording FR-008/FR-009 exist to prevent.

### 3.4 FR-009's "declared type" is now actually reported

`crates/ragent-tools-core/src/schema.rs::validate_required_args` reads the
declared type before the presence check and states it when the required
parameter is absent:

```rust
Err(format!(
    "missing required parameter '{field_name}', which is declared as type '{expected_type}'"
))
```

with the old message kept verbatim for a required field that declares no
`type`. The type was already being read one line later for the mismatch case;
this only moves the read above the presence check. No caller's control flow
changes: the function still returns the first violation it finds.

## 4. The cases

`crates/ragent-agent/tests/test_tool_input_error.rs` - **7 existing cases now
run, + 1 new**.

| Case | Pins |
|---|---|
| `undecodable_refusal_names_quotes_and_instructs` | FR-008: names the tool, quotes the decoder verbatim, instructs a resend, says the tool did not run |
| `schema_refusal_names_the_parameter_and_its_type` | FR-009: names the tool, `'destination'`, and the declared type `string`; says it did not run |
| `every_refusal_is_an_error_by_the_status_derivation` | FR-014: both causes report `ToolCallStatus::Error`, and `is_error()` **is** `status().is_error()` - one derivation, not two |
| `anyhow_adapter_preserves_the_sentence` | the in-task adapter renders identical text, including `{:#}`, and survives boxing |
| `refusal_kinds_are_distinct_values` | the two causes are distinguishable by value, not by matching message text |
| `refusals_are_constructed_only_in_the_shared_constructor` | no other agent source file states a refusal sentence (the negative half) |
| `is_error_is_derived_from_the_status_type` | `history.rs`, `compaction/convert.rs`, `processor.rs` never write `is_error: !success` or compare the status to `Error` |
| **`both_gates_build_their_refusal_with_the_shared_constructor`** (new) | the positive half: `processor.rs` calls `undecodable_with_reason`/`undecodable`, calls `schema_violation`, takes its status from `refusal.status()`, and contains none of the three pre-T-013 sentence fragments |

The new case is the one that would have caught the original defect. The negative
scan alone is satisfiable by a gate that never states a sentence at all - it
could drift back to a bare status literal and a `format!` and stay green. The
positive half names the constructor calls the two gates must make.

`crates/ragent-agent/tests/test_processor_more_paths.rs` - **+1 case, +2 new
assertions**:

| Case | Pins |
|---|---|
| `test_non_object_arguments_are_still_refused` (extended) | the refusal now also instructs a resend (FR-008), and the wire form carries `is_error: true` (FR-014) |
| **`test_schema_violation_names_the_parameter_and_is_marked_on_the_wire`** (new) | a `move_file` call missing `destination` produces `status == Error`, a sentence naming `tool 'move_file'` and `'destination'` and its declared type, **not** the stale `Invalid tool arguments:` prefix, and `is_error: true` on the request the provider receives |

The wire assertions read `ContentPart::ToolResult { is_error, .. }` off the
*captured provider request*, not off the rendered message: FR-014 is a statement
about what the provider receives, and only the request proves it.

## 5. Proof

### Red proofs

Each break was applied, the named case observed red, then the file restored and
verified byte-identical with `md5sum` and `touch`ed so cargo could not reuse the
broken object file.

| Break | Failing case | Message observed |
|---|---|---|
| decode gate back to a hand-rolled `format!()` sentence + `ToolCallStatus::Error` literal | `both_gates_build_their_refusal_with_the_shared_constructor` | "the decode gate must build its FR-008 refusal with ToolInputError, not a hand-rolled sentence" |
| in-task schema gate back to `anyhow!("Invalid tool arguments: {schema_err}")` | the same case **and** `test_schema_violation_names_the_parameter_and_is_marked_on_the_wire` | "the in-task schema gate must build its FR-009 refusal with ToolInputError" / "the sentence names the tool (FR-009): Invalid tool arguments: missing required parameter 'destination', which is declared as type 'string'" |
| `validate_required_args` back to the presence-only message | `schema_refusal_names_the_parameter_and_its_type` | "the sentence states the declared type: Invalid arguments for tool 'move_file': missing required parameter 'destination'. ..." |
| the append site's `is_error: status.is_error()` back to `is_error: false` | `test_non_object_arguments_are_still_refused` **and** `test_schema_violation_names_the_parameter_and_is_marked_on_the_wire` | `left: [false], right: [true]` in both |

The third proof is the one that shows the case file is load-bearing: the
contract case had never run before this task, and the moment it did it found a
real FR-009 gap rather than passing vacuously.

### Verification (green)

- `cargo fmt --all -- --check` - clean
- `cargo check -p ragent-agent` - clean
- `cargo clippy -p ragent-agent -p ragent-tools-core -p ragent-types --all-targets`
  - no new warning names any file this task touched (the only diagnostics are the
    pre-existing `pub(crate) ... inside private module` group and provider
  `unnecessary structure name repetition`)
- `cargo test -p ragent-agent --test all test_tool_input_error::` - **8 passed**
  (previously 0 ran)
- `cargo test -p ragent-agent --test all test_processor_more_paths::` -
  **24 passed**
- `cargo test -p ragent-agent --test all` - **1679 passed**, 0 failed, 1 ignored
- `cargo test -p ragent-tools-core --test all` - **394 passed**, 0 failed
- `cargo test -p ragent-tools-extended --test all` - **1806 passed**, 11 ignored
  (the extended tools' own `parameters_schema()` errors are unchanged)
- `cargo test -p ragent-types --test all` - **418 passed**, 0 failed
- `cargo test -p ragent-server --test all` - **192 passed**, 0 failed
- guard scripts: `check-dead-code-reasons`, `check-inline-tests`,
  `check-test-env-isolation`, `check-silent-errors`, `check-security-unwraps`,
  `check-poison-locks`, `check-shared-guards` - all pass

## 6. Notes for the downstream tasks

- **T-014** takes the same `ToolInputError` values into the ordered pipeline.
  `session/dispatch.rs` already builds all three variants at stages 1-3; that
  module is still not declared in `session/mod.rs`, which is T-014's first fix.
  Until then `resolve_call` is the only caller and the loop gates above it.
- **T-017** finishes FR-014's transmit side. `is_error` is now correct from the
  record through to `ContentPart::ToolResult`; what remains is the profile's
  `supports_is_error == false` path, which must prefix `ERROR: `.
- **Ordering** is unchanged here. The in-task schema gate still runs after the
  pre-tool hooks (inside the spawned task), which is the ordering T-014 fixes;
  this task deliberately changed *what a refusal says and what it sets*, not
  *when it is reached*.
