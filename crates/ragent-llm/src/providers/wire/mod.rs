//! Declared tool-calling wire contract per provider family (FR-002).
//!
//! Every provider client declares a `ToolWireProfile` describing how its
//! provider carries a tool definition, a tool result, a tool-call id, the
//! argument JSON, and a streamed tool-call delta. The profile is the single
//! source of truth the encoders and the stream accumulator dispatch on, so a
//! provider cannot invent its own envelope (FR-003), its own accumulation rule
//! (FR-004) or its own result carrier (FR-006).
//!
//! A profile is a `const` per wire family
//! ([`OPENAI_CHAT`](wire::profile_table::OPENAI_CHAT),
//! [`ANTHROPIC_MESSAGES`](wire::profile_table::ANTHROPIC_MESSAGES),
//! [`GEMINI`](wire::profile_table::GEMINI),
//! [`BEDROCK_CONVERSE`](wire::profile_table::BEDROCK_CONVERSE),
//! [`OPENAI_RESPONSES`](wire::profile_table::OPENAI_RESPONSES),
//! [`OLLAMA_NATIVE`](wire::profile_table::OLLAMA_NATIVE)), the `FAMILIES` list
//! names every one of them, and every registered provider id names the contract
//! it encodes its requests with in the
//! [`PROVIDER_WIRE`](wire::profile_table::PROVIDER_WIRE) table, looked up
//! by [`profile_for`](wire::profile_table::profile_for). The constants and the
//! two tables live in the [`profile_table`](wire::profile_table) submodule; this
//! module holds the shape of the contract itself. An id that is absent from the
//! table has no declared contract, and the coverage assertion in
//! `tests/test_wire_profile_table.rs` fails the build, so a provider cannot be
//! registered without stating its tool-calling terms (FR-002). There is
//! deliberately no `Default` impl for
//! [`ToolWireProfile`](wire::ToolWireProfile) - a defaulted profile would be a
//! silent, wrong contract for every provider that forgot to declare one.
//!
//! Those links carry an explicit `wire::` target because rustdoc attributes this
//! header to the `pub mod wire;` declaration in `providers` and so resolves them
//! from that module's scope rather than from this one.
//!
//! Every label here is ASCII (NFR-005) because the wire matrix (FR-016) prints
//! these values verbatim and no cell may be blank.

/// The one stream tool-call accumulator every provider family shares (FR-004).
pub mod decode;
/// The one tool-definition encoder every provider family shares (FR-003).
pub mod encode;
/// The FR-016 wire matrix: the declared profile table, printable without
/// contacting a provider.
pub mod matrix;
pub mod profile_table;
/// The opt-in payload trace (FR-017): each step's redacted request body under
/// `target/temp/wire-trace/`, inert until `tools.wire_trace` enables it.
pub mod trace;

/// The JSON envelope a provider wraps a tool *definition* in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolEnvelope {
    /// `{"type":"function","function":{name, description, parameters}}` - the
    /// OpenAI chat-completions shape and every compatible endpoint that copies
    /// it.
    OpenAiFunction,
    /// `{"name", "description", "input_schema"}` - the Anthropic Messages
    /// shape (also used by Azure Resource when it speaks Anthropic).
    AnthropicTool,
    /// `{"name", "description", "parameters"}` - one entry of Gemini's
    /// `tools[].functionDeclarations[]`.
    GeminiFunctionDeclaration,
    /// `{"toolSpec":{name, description, inputSchema:{json:...}}}` - the Bedrock
    /// Converse shape.
    BedrockToolSpec,
    /// `{"type":"function","name","description","parameters"}` - the flattened
    /// Responses-API function shape, which has no nested `function` object.
    ResponsesFlatFunction,
}

impl ToolEnvelope {
    /// A stable ASCII label for this envelope, used by the wire matrix (FR-016)
    /// and by trace output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiFunction => "openai_function",
            Self::AnthropicTool => "anthropic_tool",
            Self::GeminiFunctionDeclaration => "gemini_function_declaration",
            Self::BedrockToolSpec => "bedrock_tool_spec",
            Self::ResponsesFlatFunction => "responses_flat_function",
        }
    }
}

/// The carrier a provider expects a tool *result* in when it is appended to
/// history (FR-006).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResultCarrier {
    /// A `role: "tool"` message carrying the call id in the profile's
    /// [`ToolWireProfile::id_field`], plus `tool_name` when the provider wants
    /// it.
    ToolRoleMessage,
    /// A user-role message containing a `tool_result` block with
    /// `tool_use_id` and `content`.
    UserToolResultBlock,
    /// A `functionResponse` part, which names the tool and carries no call id.
    GeminiFunctionResponse,
    /// A `function_call_output` input item keyed by `call_id`.
    ResponsesFunctionCallOutput,
}

impl ResultCarrier {
    /// A stable ASCII label for this carrier, used by the wire matrix (FR-016).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ToolRoleMessage => "tool_role_message",
            Self::UserToolResultBlock => "user_tool_result_block",
            Self::GeminiFunctionResponse => "gemini_function_response",
            Self::ResponsesFunctionCallOutput => "responses_function_call_output",
        }
    }
}

/// The JSON key the provider expects a tool-call id under.
///
/// The result carrier (FR-006) writes the id into exactly this position, and
/// the id written must be byte-identical to the id the model returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdField {
    /// The provider returns an id under this JSON key (`tool_call_id`,
    /// `tool_use_id`, `call_id`, ...).
    Key(&'static str),
    /// The provider returns no id at all (Gemini), so one must be synthesised
    /// deterministically by the harness (FR-019).
    Synthesised,
}

impl IdField {
    /// The JSON key, or `synthesised` when the provider supplies no id.
    ///
    /// Never empty: the wire matrix (FR-016) has no blank cell (NFR-005).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Key(key) => key,
            Self::Synthesised => "synthesised",
        }
    }
}

/// How a provider encodes the argument JSON of a tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArgsEncoding {
    /// A JSON *string* holding the object (OpenAI and its copies): the
    /// accumulated delta fragments are escaped inside a string field.
    JsonString,
    /// A JSON *object* (Anthropic, Gemini, Bedrock).
    JsonObject,
}

impl ArgsEncoding {
    /// A stable ASCII label for this encoding, used by the wire matrix
    /// (FR-016).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JsonString => "json_string",
            Self::JsonObject => "json_object",
        }
    }
}

/// The key the stream accumulator attributes an incoming
/// [`crate::llm::StreamEvent::ToolCallDelta`] by (FR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamKeying {
    /// The provider supplies a content-block / choice index and deltas belong
    /// to the call opened at that index (Anthropic, OpenAI chat-completions).
    ByIndex,
    /// The provider supplies the call id on every delta (OpenAI Responses),
    /// so deltas are attributed by id.
    ByCallId,
}

impl StreamKeying {
    /// A stable ASCII label for this keying, used by the wire matrix (FR-016).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ByIndex => "by_index",
            Self::ByCallId => "by_call_id",
        }
    }
}

/// The tool-calling wire contract one provider family declares (FR-002).
///
/// Nine fields, one per divergent decision the current provider modules each
/// make for themselves. A client takes a profile by value (it is [`Copy`]) and
/// dispatches its encoders and its stream accumulator on it, so adding a
/// provider means declaring a profile rather than re-deriving the contract.
///
/// Every field is a required decision, and the type has **no `Default` impl and
/// no optional field**: a client constructed without a profile fails to compile,
/// and there is no fallback contract one could be silently built with. That is
/// what makes the coverage assertion on
/// [`PROVIDER_WIRE`](wire::profile_table::PROVIDER_WIRE) a build-time guarantee
/// rather than a convention.
///
/// The nine decisions, and the one place each is read (or the task that will
/// read it, where the consumer is not yet written - the field is declared here
/// first, because a profile is the single statement of a provider's terms and a
/// decision added later would mean a client encoded requests it never
/// described):
///
/// | Field | Consumer |
/// |---|---|
/// | [`tool_envelope`](Self::tool_envelope) | `wire::encode::encode_tools` (FR-003, T-004) |
/// | [`result_carrier`](Self::result_carrier) | `wire::encode::encode_message` (FR-006, T-006) |
/// | [`id_field`](Self::id_field) | `wire::encode::encode_message`, the id-keyed carriers (FR-006, T-006) |
/// | [`args_encoding`](Self::args_encoding) | `wire::encode`'s `args_json` (FR-007, T-004/T-006) |
/// | [`stream_keying`](Self::stream_keying) | `wire::decode::ToolCallAccumulator::new` (FR-004, T-008) |
/// | [`supports_parallel`](Self::supports_parallel) | the agent loop's per-step parallel gate (FR-010, T-015) |
/// | [`supports_is_error`](Self::supports_is_error) | `wire::encode`'s `mark_error` (FR-014, T-017) |
/// | [`echo_reasoning`](Self::echo_reasoning) | the reasoning replay path (FR-011, T-018) |
/// | [`synthetic_ids`](Self::synthetic_ids) | the accumulator's id minting (FR-019, T-010) |
///
/// A field whose consumer is a later task is still declared here, and the wire
/// matrix (FR-016) prints every one of them, so the table states the whole
/// contract from the day the type exists rather than growing a column per task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToolWireProfile {
    /// The envelope a tool definition is wrapped in (FR-003).
    pub tool_envelope: ToolEnvelope,
    /// The carrier a tool result is emitted in (FR-006).
    pub result_carrier: ResultCarrier,
    /// The JSON key the tool-call id occupies, or [`IdField::Synthesised`]
    /// (FR-006, FR-019).
    pub id_field: IdField,
    /// How tool-call argument JSON is encoded on the wire.
    pub args_encoding: ArgsEncoding,
    /// The key streamed tool-call deltas are attributed by (FR-004).
    pub stream_keying: StreamKeying,
    /// Whether the loop may dispatch a step's tool calls concurrently
    /// (FR-010). `false` forces sequential execution: the agent loop demotes
    /// the user's `experimental.parallel_tool_calls` opt-in for the step and
    /// logs the demotion once. `true` means concurrency is *permitted*, never
    /// that it is switched on - only the user's own opt-in does that.
    pub supports_parallel: bool,
    /// Whether the provider transmits an error flag on a tool result, or needs
    /// the `ERROR: ` textual marker instead (FR-014).
    pub supports_is_error: bool,
    /// Whether signed reasoning must be replayed ahead of the tool-use block
    /// (FR-011).
    pub echo_reasoning: bool,
    /// Whether the provider omits tool-call ids, so the harness must generate
    /// them (FR-019).
    pub synthetic_ids: bool,
}
