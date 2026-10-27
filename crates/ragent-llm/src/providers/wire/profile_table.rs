//! One declared profile per provider family, and the id-to-profile table
//! (FR-002).
//!
//! [`crate::provider::wire`] defines the *shape* of a tool-calling contract;
//! this module states the values. There is one [`ToolWireProfile`] `const` per
//! wire family, [`FAMILIES`] names every one of them, and a [`PROVIDER_WIRE`]
//! table names, for every registered provider id, the family that provider
//! speaks. [`profile_for`] is the lookup the encoders (T-004 onward), the stream
//! accumulator (T-008), the parallel gate (T-015), the error-flag encoder (T-017)
//! and the wire matrix (T-022) share.
//!
//! The coverage assertion lives in `tests/test_wire_profile_table.rs` and fails
//! when a provider is registered without a row, so a provider cannot be added
//! to the build without stating its tool-calling terms (FR-002). There is no
//! catch-all: [`profile_for`] returns `None` for an unknown id rather than a
//! defaulted contract, because a defaulted profile would be a silent, wrong
//! answer for exactly the case the assertion exists to catch.
//!
//! ## Six families, fifteen providers
//!
//! The families are the six wire formats the tree actually implements:
//! `OPENAI_CHAT` (chat completions), `ANTHROPIC_MESSAGES`, `GEMINI`,
//! `BEDROCK_CONVERSE`, `OPENAI_RESPONSES` and `OLLAMA_NATIVE` (the native
//! `/api/chat` shape). [`FAMILIES`] lists each one exactly once, under its
//! `const` name in lower case, which is what turns "one profile per family" into
//! an assertion: the table is many-to-one, so on its own it cannot tell a shared
//! declared contract from fifteen literals that happen to be equal today. Several
//! providers share a family because they speak the same wire format: `openai`,
//! `azure_foundry`, `copilot`, `openrouter`, `xai`, `generic_openai`,
//! `huggingface` and local `ollama` all build their request body through
//! `OpenAiCompat`, and `huggingface`'s `t_` tool-name prefix is a name
//! transformation inside the same envelope, not a different contract.
//!
//! ## Two providers have a second client behind the same id
//!
//! `bedrock` and `azure_resource` each construct one of two clients depending
//! on the model, so one table row cannot describe both paths:
//!
//! - `bedrock` is [`BEDROCK_CONVERSE`], the Converse API. A Bedrock *Anthropic*
//!   model is served by `BedrockAnthropicClient`, which speaks
//!   [`ANTHROPIC_MESSAGES`].
//! - `azure_resource` is [`OPENAI_CHAT`], the file's default `api_type`. An
//!   entry whose `api_type` is `"anthropic"` is served by
//!   `AzureAnthropicClient`, which speaks [`ANTHROPIC_MESSAGES`].
//!
//! The row states the family the provider id resolves to when nothing else
//! selects a path. Each client still holds its own [`ToolWireProfile`] - that
//! is what FR-002 requires of a *client* - so selecting the second path means
//! selecting the second family, not re-deriving a contract.
//!
//! ## The router row is nominal
//!
//! `router` is registered like any other provider, so the coverage assertion
//! requires a row for it. It is the one row that describes no encoding of its
//! own: `RouterClient` picks a downstream provider per request and forwards the
//! canonical `ChatRequest` to that provider's client, which encodes with *its*
//! profile. The row therefore names [`OPENAI_CHAT`] - the family of the
//! canonical request the router forwards unchanged - and any consumer that
//! needs the effective contract for a router session must resolve the
//! downstream provider rather than read this row. Stating the row here keeps
//! the wire matrix populated (every cell printable, TC-002) instead of
//! special-casing one provider out of the table.
//!
//! FR-010's gate (T-015) reads this row like any other, so it demotes a router
//! step only if the row itself says `supports_parallel == false`. A downstream
//! provider that declares `false` is not honoured through the gate: the
//! downstream selection happens inside `RouterClient::chat`, after the agent
//! loop has already chosen sequential or concurrent for the step. That gap is
//! recorded rather than hidden - see
//! `specs/uniformtools/T-015-PARALLEL-GATE.md`.

use super::{ArgsEncoding, IdField, ResultCarrier, StreamKeying, ToolEnvelope, ToolWireProfile};

/// The OpenAI chat-completions family: `{"type":"function",...}` definitions,
/// `role:"tool"` results keyed by `tool_call_id`, arguments as an escaped JSON
/// string, deltas keyed by choice index.
///
/// `supports_is_error` is `false` because the shape has no error flag on a tool
/// message; FR-014's `ERROR: ` prefix carries it instead. `echo_reasoning` is
/// `false` because the family has no signed-reasoning channel to replay.
pub const OPENAI_CHAT: ToolWireProfile = ToolWireProfile {
    tool_envelope: ToolEnvelope::OpenAiFunction,
    result_carrier: ResultCarrier::ToolRoleMessage,
    id_field: IdField::Key("tool_call_id"),
    args_encoding: ArgsEncoding::JsonString,
    stream_keying: StreamKeying::ByIndex,
    supports_parallel: true,
    supports_is_error: false,
    echo_reasoning: false,
    synthetic_ids: false,
};

/// The Anthropic Messages family: `{"name","description","input_schema"}`
/// definitions, `tool_result` blocks in a user turn keyed by `tool_use_id`,
/// arguments as a JSON object, deltas keyed by content-block index.
///
/// This is the one family that replays signed reasoning (FR-011) and the one
/// mainstream family that transmits `is_error` natively.
pub const ANTHROPIC_MESSAGES: ToolWireProfile = ToolWireProfile {
    tool_envelope: ToolEnvelope::AnthropicTool,
    result_carrier: ResultCarrier::UserToolResultBlock,
    id_field: IdField::Key("tool_use_id"),
    args_encoding: ArgsEncoding::JsonObject,
    stream_keying: StreamKeying::ByIndex,
    supports_parallel: true,
    supports_is_error: true,
    echo_reasoning: true,
    synthetic_ids: false,
};

/// The Google Gemini family: `functionDeclarations` definitions,
/// `functionResponse` results that name the tool and carry no call id,
/// arguments as a JSON object.
///
/// The model supplies no id at all, so the harness must synthesise one (FR-019)
/// - which is why `synthetic_ids` and [`IdField::Synthesised`] agree here.
pub const GEMINI: ToolWireProfile = ToolWireProfile {
    tool_envelope: ToolEnvelope::GeminiFunctionDeclaration,
    result_carrier: ResultCarrier::GeminiFunctionResponse,
    id_field: IdField::Synthesised,
    args_encoding: ArgsEncoding::JsonObject,
    stream_keying: StreamKeying::ByIndex,
    supports_parallel: true,
    supports_is_error: false,
    echo_reasoning: false,
    synthetic_ids: true,
};

/// The Amazon Bedrock Converse family: `toolSpec` definitions under
/// `toolConfig`, `toolResult` blocks keyed by `toolUseId` with a native
/// `status` field, arguments as a JSON object.
///
/// `supports_is_error` is `true` because Converse carries `status: "error"` on
/// a `toolResult`; the encoder writes that `status` and does not add FR-014's
/// text marker beside a native flag (T-017).
pub const BEDROCK_CONVERSE: ToolWireProfile = ToolWireProfile {
    tool_envelope: ToolEnvelope::BedrockToolSpec,
    result_carrier: ResultCarrier::UserToolResultBlock,
    id_field: IdField::Key("toolUseId"),
    args_encoding: ArgsEncoding::JsonObject,
    stream_keying: StreamKeying::ByIndex,
    supports_parallel: true,
    supports_is_error: true,
    echo_reasoning: false,
    synthetic_ids: false,
};

/// The OpenAI Responses family: the flattened function shape (no nested
/// `function` object), `function_call_output` items keyed by `call_id`,
/// arguments as a JSON string, and deltas keyed by call id rather than index.
///
/// This is the only family whose stream is id-keyed (FR-004), and the only one
/// whose tool history the current encoder drops (FR-018).
pub const OPENAI_RESPONSES: ToolWireProfile = ToolWireProfile {
    tool_envelope: ToolEnvelope::ResponsesFlatFunction,
    result_carrier: ResultCarrier::ResponsesFunctionCallOutput,
    id_field: IdField::Key("call_id"),
    args_encoding: ArgsEncoding::JsonString,
    stream_keying: StreamKeying::ByCallId,
    supports_parallel: true,
    supports_is_error: false,
    echo_reasoning: false,
    synthetic_ids: false,
};

/// The native Ollama `/api/chat` family: the OpenAI function envelope for
/// definitions, but `arguments` as a JSON object, a `role:"tool"` result that
/// names the tool instead of echoing an id, and deltas keyed by call id.
///
/// It is a separate family from [`OPENAI_CHAT`] - which local `ollama` speaks,
/// over `/v1/chat/completions` - because the argument encoding, the result key
/// and the stream keying all differ. Because the native result carries no id
/// slot, the accumulator synthesises `<provider>_<counter>_<name>` when a frame
/// omits `id` (FR-019), so `synthetic_ids` is `true`.
pub const OLLAMA_NATIVE: ToolWireProfile = ToolWireProfile {
    tool_envelope: ToolEnvelope::OpenAiFunction,
    result_carrier: ResultCarrier::ToolRoleMessage,
    id_field: IdField::Synthesised,
    args_encoding: ArgsEncoding::JsonObject,
    stream_keying: StreamKeying::ByCallId,
    supports_parallel: true,
    supports_is_error: false,
    echo_reasoning: false,
    synthetic_ids: true,
};

/// The complete set of declared wire families: one [`ToolWireProfile`] per
/// family, under the ASCII label the family is known by (FR-002).
///
/// Each label is the family's `const` name in lower case, so the list and the
/// declarations above can be checked against each other mechanically. Two
/// properties make this list load-bearing rather than a convenience:
///
/// - **Every [`PROVIDER_WIRE`] row names one of these.** A row is an
///   id-to-family assignment, never a contract composed at the row, so a
///   provider cannot quietly speak a seventh profile that no family declares.
/// - **Every family `const` in this module is listed here.** A constant that no
///   row and no list names is dead contract, and a family added without being
///   registered would let the wire matrix (FR-016) miss it.
///
/// Both directions are asserted in `tests/test_wire_profile_table.rs`, which is
/// also the only reader: production code reads the family `const`s directly or
/// goes through [`profile_for`], so this list adds no indirection to a request
/// path.
pub const FAMILIES: &[(&str, ToolWireProfile)] = &[
    ("anthropic_messages", ANTHROPIC_MESSAGES),
    ("bedrock_converse", BEDROCK_CONVERSE),
    ("gemini", GEMINI),
    ("ollama_native", OLLAMA_NATIVE),
    ("openai_chat", OPENAI_CHAT),
    ("openai_responses", OPENAI_RESPONSES),
];

/// Every registered provider id and the family it declares (FR-002), ordered by
/// provider id.
///
/// Every row names a member of [`FAMILIES`] - the same contract object, not an
/// equal one built here - which is what makes "one profile per provider family"
/// mean exactly `openai` and `openrouter` speak the same declared terms rather
/// than two identical literals.
///
/// The list is a slice rather than a fixed-size array so registering a provider
/// does not change this type; the coverage assertion in
/// `tests/test_wire_profile_table.rs` is what fails when the two disagree. See
/// the module docs for the `bedrock`, `azure_resource` and `router` rows, which
/// are the three that need a sentence of explanation.
pub const PROVIDER_WIRE: &[(&str, ToolWireProfile)] = &[
    ("anthropic", ANTHROPIC_MESSAGES),
    ("azure_foundry", OPENAI_CHAT),
    ("azure_resource", OPENAI_CHAT),
    ("bedrock", BEDROCK_CONVERSE),
    ("copilot", OPENAI_CHAT),
    ("gemini", GEMINI),
    ("generic_openai", OPENAI_CHAT),
    ("huggingface", OPENAI_CHAT),
    ("ollama", OPENAI_CHAT),
    ("ollama_cloud", OLLAMA_NATIVE),
    ("openai", OPENAI_CHAT),
    ("openai_responses", OPENAI_RESPONSES),
    ("openrouter", OPENAI_CHAT),
    ("router", OPENAI_CHAT),
    ("xai", OPENAI_CHAT),
];

/// Returns the profile the provider registered under `provider_id` declares.
///
/// Returns `None` when no provider with that id is registered in this table -
/// deliberately, so a client cannot inherit a plausible-looking contract it
/// never stated (FR-002). Registering a provider is therefore a two-line
/// change: the provider itself, and its row here.
///
/// # Examples
///
/// ```
/// use ragent_llm::provider::wire::profile_table::{GEMINI, profile_for};
///
/// assert_eq!(profile_for("gemini"), Some(GEMINI));
/// assert_eq!(profile_for("no_such_provider"), None);
/// ```
#[must_use]
pub fn profile_for(provider_id: &str) -> Option<ToolWireProfile> {
    PROVIDER_WIRE
        .iter()
        .find(|(id, _)| *id == provider_id)
        .map(|(_, profile)| *profile)
}
